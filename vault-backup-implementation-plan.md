# Vault Backup — Implementation Plan

## Task Dependency Graph

```
[1] .app bundle packaging
 └──▶ [2] iCloud Keychain integration
       └──▶ [4] vault backup command
       └──▶ [5] vault restore command
[3] Backup crypto module (Argon2id + HKDF + AES-256-GCM + passphrase gen)
 └──▶ [4] vault backup command
 └──▶ [5] vault restore command
[4] vault backup command
 └──▶ [6] vault backup-reset command
 └──▶ [7] vault backup-info command
 └──▶ [8] Stale backup nudge
[5] vault restore command
[9] Homebrew formula update ← after all commands land
```

---

## Task 1: .app Bundle Packaging

**Goal:** Wrap keypo-signer in a minimal `.app` bundle so it can carry entitlements and a provisioning profile. All existing functionality must continue to work.

**Apple Developer Portal (manual):**
- Register App ID: `com.keypo.signer`
- Enable Keychain Sharing capability
- Create macOS Developer ID provisioning profile

**Code changes:**
- Create `keypo-signer/keypo-signer.entitlements` plist with `keychain-access-groups` and `com.apple.application-identifier`
- Create minimal `keypo-signer/Info.plist` (just `CFBundleIdentifier`, `CFBundleName`, `CFBundleExecutable`)
- Add `scripts/build-app-bundle.sh`:
  - Run `swift build -c release`
  - Create `.app/Contents/MacOS/`, `.app/Contents/` directories
  - Copy binary, Info.plist, provisioning profile
  - Code-sign with `codesign --sign "Developer ID Application: ..." --entitlements keypo-signer.entitlements --options runtime keypo-signer.app`
  - Notarize with `notarytool`
- Update CI workflow (`.github/workflows/`) to use new build script

### Tests

| # | Test | Pass | Fail |
|---|------|------|------|
| 1.1 | `codesign -d --entitlements :- keypo-signer.app` | Output contains `keychain-access-groups` with value `$(TeamID).com.keypo.signer` | Entitlement missing or wrong value |
| 1.2 | `codesign --verify --deep --strict keypo-signer.app` | Exit code 0 | Non-zero exit code or signature invalid |
| 1.3 | Run `keypo-signer create --label test-bundle --policy open` via the `.app` binary | Key created, appears in `keypo-signer list` output | Error or key not created |
| 1.4 | Run `keypo-signer vault init`, `vault set`, `vault get`, `vault exec` via `.app` binary | All commands produce identical output to pre-bundle binary | Any command fails or output differs |
| 1.5 | Run `swift test` in `keypo-signer/` | All existing tests pass | Any test failure |
| 1.6 | `spctl --assess --type execute keypo-signer.app` (after notarization) | "accepted" | "rejected" or Gatekeeper warning |

---

## Task 2: iCloud Keychain Integration

**Goal:** Read and write a synchronizable generic password item to iCloud Keychain.

**New file:** `keypo-signer/Sources/KeypoSigner/Backup/KeychainSync.swift`

**Functions:**
```swift
/// Store a 256-bit key in iCloud Keychain (kSecAttrSynchronizable: true)
func storeSyncedBackupKey(_ keyData: Data) throws

/// Read the synced backup key. Returns nil if not found.
func readSyncedBackupKey() throws -> Data?

/// Delete the synced backup key (for backup-reset).
func deleteSyncedBackupKey() throws

/// Check if the synced key exists without reading it.
func syncedBackupKeyExists() throws -> Bool
```

**Keychain query attributes:**
- `kSecClass`: `kSecClassGenericPassword`
- `kSecAttrService`: `"com.keypo.vault-backup"`
- `kSecAttrAccount`: `"backup-encryption-key"`
- `kSecAttrSynchronizable`: `true`
- `kSecAttrAccessible`: `kSecAttrAccessibleAfterFirstUnlock` (available after first unlock, syncs via iCloud)

**Error handling:**
- `errSecItemNotFound` → return nil (key not synced yet)
- `errSecDuplicateItem` → delete and re-add (for storeSyncedBackupKey)
- `errSecMissingEntitlement` (-34018) → clear error: "keypo-signer is missing required entitlements. Ensure you're running the .app-bundled version."

### Tests

| # | Test | Pass | Fail |
|---|------|------|------|
| 2.1 | `storeSyncedBackupKey(32 random bytes)` then `readSyncedBackupKey()` | Returned data matches stored data byte-for-byte | Data mismatch, nil returned, or OSStatus error |
| 2.2 | `readSyncedBackupKey()` when no key has been stored | Returns nil | Throws error or returns non-nil |
| 2.3 | `storeSyncedBackupKey()` then `deleteSyncedBackupKey()` then `readSyncedBackupKey()` | Final read returns nil | Key still present after delete |
| 2.4 | `storeSyncedBackupKey()` called twice with different data | Second call succeeds, `readSyncedBackupKey()` returns the second value | `errSecDuplicateItem` thrown, or first value returned |
| 2.5 | `syncedBackupKeyExists()` when key is stored | Returns true | Returns false |
| 2.6 | `syncedBackupKeyExists()` when no key is stored | Returns false | Returns true or throws |
| 2.7 | Call any KeychainSync function from an unsigned binary (no entitlements) | Throws error with message containing "missing required entitlements" | Silent failure or crash |
| 2.8 | *(Manual, local only)* Store key on Mac A, sign into same iCloud on Mac B, call `readSyncedBackupKey()` on Mac B | Returns the key stored on Mac A | Key not found or data mismatch |

---

## Task 3: Backup Crypto Module

**Goal:** Passphrase generation, key derivation, and encrypt/decrypt for the backup blob.

**Dependencies:** None (pure crypto, no iCloud). Can be built in parallel with Tasks 1–2.

**New files:**
- `keypo-signer/Sources/KeypoSigner/Backup/BackupCrypto.swift`
- `keypo-signer/Sources/KeypoSigner/Backup/PassphraseGenerator.swift`
- `keypo-signer/Sources/KeypoSigner/Backup/Wordlist.swift`

### Wordlist

The BIP-39 English wordlist (2048 words):
- Source: https://github.com/bitcoin/bips/blob/master/bip-0039/english.txt
- Embed as a static `[String]` array in `Wordlist.swift`
- Must contain exactly 2048 entries

### PassphraseGenerator

```swift
/// Generate a 4-word passphrase from the BIP-39 English wordlist.
func generatePassphrase() -> [String]

/// Pick n random indices from a 4-word passphrase for user confirmation.
func confirmationIndices(wordCount: Int, confirmCount: Int) -> [Int]
```

Use `SecRandomCopyBytes` for randomness (11 bits per word × 4 = 44 bits entropy).

### BackupCrypto

```swift
struct BackupKeys {
    let argon2Salt: Data    // 16 bytes
    let hkdfSalt: Data      // 32 bytes
    let backupKey: Data     // 32 bytes (derived)
}

/// Derive backup key from synced key + passphrase (new salts generated).
func deriveBackupKey(syncedKey: Data, passphrase: String) throws -> BackupKeys

/// Derive backup key from synced key + passphrase + existing salts (for restore/verify).
func deriveBackupKey(syncedKey: Data, passphrase: String, argon2Salt: Data, hkdfSalt: Data) throws -> Data

/// Encrypt plaintext payload. Returns (nonce, ciphertext, authTag).
func encrypt(plaintext: Data, key: Data) throws -> (nonce: Data, ciphertext: Data, authTag: Data)

/// Decrypt ciphertext. Returns plaintext.
func decrypt(ciphertext: Data, nonce: Data, authTag: Data, key: Data) throws -> Data
```

### Argon2id via swift-sodium

Add to `Package.swift`:
```swift
.package(url: "https://github.com/jedisct1/swift-sodium.git", from: "0.9.1")
```

Use `Sodium().pwHash.hash()` with:
- Algorithm: `.Argon2id13`
- `opsLimit`: 3
- `memLimit`: 67108864 (64 MB)
- Output length: 32 bytes

AES-256-GCM and HKDF-SHA256 use CryptoKit natively (`AES.GCM` and `HKDF<SHA256>`).

### Tests

| # | Test | Pass | Fail |
|---|------|------|------|
| 3.1 | `Wordlist.english.count` | Exactly 2048 | Any other count |
| 3.2 | Every word in `Wordlist.english` is lowercase alpha only | All words match `^[a-z]+$` | Any word contains non-alpha characters |
| 3.3 | `Wordlist.english` has no duplicates | `Set(Wordlist.english).count == 2048` | Set count differs from array count |
| 3.4 | `generatePassphrase()` returns 4 words | Array count is 4 | Any other count |
| 3.5 | Every word from `generatePassphrase()` is in `Wordlist.english` | All 4 words found in wordlist | Any word not in wordlist |
| 3.6 | `generatePassphrase()` called 100 times produces at least 90 distinct results | >= 90 unique passphrases | < 90 unique (indicates weak randomness) |
| 3.7 | `confirmationIndices(wordCount: 4, confirmCount: 2)` | Returns 2 distinct indices, each in range 0..<4 | Duplicate indices, out of range, or wrong count |
| 3.8 | `deriveBackupKey(syncedKey, passphrase)` then `deriveBackupKey(syncedKey, passphrase, argon2Salt, hkdfSalt)` with same salts | Both produce identical 32-byte backup key | Keys differ |
| 3.9 | `deriveBackupKey` with same inputs called twice (fresh salts each time) | Two different backup keys (salts are random) | Identical keys (salt reuse) |
| 3.10 | Encrypt then decrypt with correct key | Decrypted plaintext matches original byte-for-byte | Data mismatch |
| 3.11 | Encrypt then decrypt with wrong key (1 bit flipped) | Decrypt throws `CryptoKit.CryptoKitError.authenticationFailure` | Decryption succeeds or different error type |
| 3.12 | Encrypt then decrypt with correct key but tampered ciphertext (1 byte modified) | Decrypt throws authentication failure | Decryption succeeds |
| 3.13 | `deriveBackupKey` with correct synced key but wrong passphrase | Derived key differs from original | Keys match |
| 3.14 | `deriveBackupKey` with correct passphrase but wrong synced key (1 bit flipped) | Derived key differs from original | Keys match |
| 3.15 | `BackupKeys.backupKey` is exactly 32 bytes | `backupKey.count == 32` | Any other length |
| 3.16 | `BackupKeys.argon2Salt` is exactly 16 bytes | `argon2Salt.count == 16` | Any other length |
| 3.17 | `BackupKeys.hkdfSalt` is exactly 32 bytes | `hkdfSalt.count == 32` | Any other length |

---

## Task 4: `vault backup` Command

**Goal:** Full backup flow — decrypt local vault, encrypt with two-factor key, write to iCloud Drive.

**New files:**
- `keypo-signer/Sources/KeypoSigner/Backup/BackupCommand.swift`
- `keypo-signer/Sources/KeypoSigner/Backup/BackupBlob.swift` (Codable structs for blob format)
- `keypo-signer/Sources/KeypoSigner/Backup/iCloudDrive.swift` (read/write/rotate files)

### BackupBlob (Codable)

```swift
struct BackupBlob: Codable {
    let version: Int                // 1
    let createdAt: String           // ISO 8601
    let deviceName: String          // Host.current().localizedName
    let argon2Salt: String          // base64
    let hkdfSalt: String            // base64
    let nonce: String               // base64
    let ciphertext: String          // base64
    let authTag: String             // base64
    let secretCount: Int
    let vaultNames: [String]
}

struct BackupPayload: Codable {
    let vaults: [BackupVault]
}

struct BackupVault: Codable {
    let name: String
    let secrets: [BackupSecret]
}

struct BackupSecret: Codable {
    let name: String
    let value: String
    let policy: String
    let createdAt: String
    let updatedAt: String
}
```

### iCloudDrive

```swift
let backupDir = "~/Library/Mobile Documents/com~apple~CloudDocs/Keypo/"
let currentFile = "vault-backup.json"
let previousFile = "vault-backup.prev.json"

/// Write backup blob, rotating current → prev first.
func writeBackup(_ blob: BackupBlob, isFirstBackup: Bool) throws

/// Read backup blob from iCloud Drive. Returns nil if file not found.
func readBackup(previous: Bool = false) throws -> BackupBlob?

/// Check if backup exists.
func backupExists(previous: Bool = false) -> Bool
```

### BackupCommand flow

**First run (no synced key exists):**
1. Check `readSyncedBackupKey()` → nil
2. Generate 32 random bytes → `storeSyncedBackupKey()`
3. Generate 4-word passphrase → display to stdout
4. Prompt user to type back 2 randomly selected words → validate
5. Decrypt all vault secrets (existing `VaultManager.getAllSecrets()` or similar)
6. Build `BackupPayload` → serialize to JSON → encrypt
7. Build `BackupBlob` with metadata + encrypted data
8. Write to iCloud Drive via `writeBackup()`
9. Initialize `~/.keypo/backup-state.json` with `secrets_since_backup: 0`
10. Output JSON result

**Subsequent run (synced key exists):**
1. `readSyncedBackupKey()` → found
2. Prompt for passphrase (stdin)
3. Rotate current → prev
4. Decrypt vault, encrypt, write
5. Reset `backup-state.json`

**Synced key missing (was deleted/corrupted):**
1. `readSyncedBackupKey()` → nil, but backup file exists in iCloud Drive
2. Warn user: previous backup will become permanently unrecoverable
3. Prompt for confirmation `[y/N]`
4. On `y`: generate new synced key, new passphrase, full re-backup

### Tests

| # | Test | Pass | Fail |
|---|------|------|------|
| 4.1 | `BackupBlob` encode to JSON then decode | All fields round-trip identically | Any field lost or corrupted |
| 4.2 | `BackupBlob` with `version: 1` decodes | Succeeds | Decode error |
| 4.3 | `BackupBlob` with `version: 99` (unknown) | Fails with version error | Silently accepted |
| 4.4 | `BackupPayload` with 3 vaults, 10 secrets total, encode → JSON → decode | All vault names, secret names, values, policies, timestamps preserved | Any data loss |
| 4.5 | `writeBackup()` when no previous backup exists | Creates `vault-backup.json`, no `.prev` file | File not created, or `.prev` created |
| 4.6 | `writeBackup()` when previous backup exists | Old `vault-backup.json` moved to `vault-backup.prev.json`, new `vault-backup.json` written | Old file lost, or `.prev` not updated |
| 4.7 | `writeBackup()` when both current and `.prev` exist | `.prev` overwritten with current, new current written. Only 2 files in directory. | 3+ backup files, or old `.prev` retained |
| 4.8 | `readBackup()` reads a file written by `writeBackup()` | Parsed `BackupBlob` matches what was written | Parse error or data mismatch |
| 4.9 | `readBackup()` when no file exists | Returns nil | Throws error |
| 4.10 | `readBackup(previous: true)` reads the `.prev` file | Returns the previous backup blob | Returns current or nil |
| 4.11 | `backupExists()` returns true after write, false before any write | Correct boolean in both cases | Wrong value |
| 4.12 | iCloud Drive `Keypo/` directory is created if it doesn't exist | Directory created, file written | Error due to missing directory |
| 4.13 | *(Integration, local only)* Full first-run backup: vault with 3 secrets → backup → file exists in iCloud Drive | `vault-backup.json` exists, parseable, `secretCount` is 3, `vaultNames` matches local vault names | File missing, wrong count, or parse error |
| 4.14 | *(Integration, local only)* Subsequent backup: add 2 secrets → backup again | `.prev` contains old backup, current contains new with `secretCount` incremented by 2 | Rotation failed or count wrong |
| 4.15 | *(Integration, local only)* `backup-state.json` reset to 0 after successful backup | `secretsSinceBackup` is 0 | Non-zero value |

---

## Task 5: `vault restore` Command

**Goal:** Restore from backup onto a clean device.

**New file:** `keypo-signer/Sources/KeypoSigner/Backup/RestoreCommand.swift`

### Flow

1. Check for existing local vault (`VaultManager.vaultExists()` or check `~/.keypo/vault.json`). If exists → abort with local vault error message.
2. `readSyncedBackupKey()` → if nil, print iCloud Keychain sync message, exit.
3. `readBackup()` → if nil, print iCloud Drive sync message, exit. Support `--previous` flag.
4. Prompt for passphrase.
5. Derive backup key from synced key + passphrase + salts from blob.
6. Decrypt ciphertext → parse `BackupPayload`.
7. On decryption failure → "Decryption failed. Check your passphrase and try again."
8. For each vault in payload:
   - For each secret: call existing `VaultManager` to create SE key with matching policy, encrypt value, store.
9. Write `vault.json` and `keys.json`.
10. Initialize `backup-state.json` with `secrets_since_backup: 0`.
11. Output JSON result.

**Key detail:** Restoring creates *new* SE keys on the new device. The key labels/tags won't match the originals. The vault metadata (names, policies) is preserved, but the underlying SE key references are new.

### Tests

| # | Test | Pass | Fail |
|---|------|------|------|
| 5.1 | Restore when local vault exists (`vault.json` present) | Aborts with error containing "Existing local vault detected" | Proceeds with restore or different error message |
| 5.2 | Restore when iCloud Keychain synced key is missing | Exits with message containing "iCloud Keychain" and "sync" | Proceeds without key or crashes |
| 5.3 | Restore when iCloud Drive backup file is missing | Exits with message containing "iCloud Drive" | Proceeds without file or crashes |
| 5.4 | Restore with wrong passphrase | Exits with "Decryption failed" message | Restores garbage data or crashes |
| 5.5 | Restore with `--previous` flag reads `.prev` file | Restores from `.prev`, not current | Reads current file |
| 5.6 | *(Integration, local only)* Full round-trip: create 5 secrets across 2 vaults → backup → `vault destroy` → restore → `vault list` | All 5 secrets present, correct names and values, in correct vaults | Any secret missing, wrong value, or wrong vault |
| 5.7 | *(Integration, local only)* Restored secrets have correct policy tiers | `vault get` with biometric/passcode secrets requires appropriate auth | Policies not enforced |
| 5.8 | *(Integration, local only)* `vault exec` works after restore | Secrets injected as env vars match originals | Missing or wrong env vars |
| 5.9 | *(Integration, local only)* `keys.json` after restore has new SE key references | Key IDs in `keys.json` differ from pre-backup `keys.json` | Same key IDs (impossible — SE keys are device-bound) |
| 5.10 | *(Integration, local only)* `backup-state.json` initialized after restore | `secretsSinceBackup` is 0 | File missing or non-zero |

---

## Task 6: `vault backup-reset` Command

**Goal:** Regenerate both factors (synced key + passphrase) while device is alive.

**New file:** `keypo-signer/Sources/KeypoSigner/Backup/BackupResetCommand.swift`

### Flow

1. Verify local vault exists (nothing to back up otherwise).
2. `deleteSyncedBackupKey()` — remove old synced key from iCloud Keychain.
3. Generate new 32-byte synced key → `storeSyncedBackupKey()`.
4. Generate new 4-word passphrase → display, confirm.
5. Decrypt all local vault secrets via SE.
6. Derive new backup key, encrypt, rotate, write.
7. Warn: "Your previous passphrase and encryption key have been regenerated. All older backups are now permanently unrecoverable."
8. Reset `backup-state.json`.

### Tests

| # | Test | Pass | Fail |
|---|------|------|------|
| 6.1 | backup-reset when no local vault exists | Aborts with error | Proceeds |
| 6.2 | *(Integration, local only)* After backup-reset, `readSyncedBackupKey()` returns a different key than before reset | Key data differs from pre-reset key | Same key data |
| 6.3 | *(Integration, local only)* After backup-reset, restore with old passphrase fails | Decryption fails | Restore succeeds with old passphrase |
| 6.4 | *(Integration, local only)* After backup-reset, restore with old synced key + new passphrase fails | Decryption fails (old synced key was deleted) | Restore succeeds |
| 6.5 | *(Integration, local only)* After backup-reset, restore with new passphrase succeeds | All secrets restored correctly | Restore fails |
| 6.6 | *(Integration, local only)* Output includes warning about older backups being unrecoverable | stderr contains "permanently unrecoverable" | Warning missing |
| 6.7 | *(Integration, local only)* `.prev` file after reset contains the pre-reset backup | `.prev` parseable and has old `createdAt` timestamp | `.prev` missing or contains reset backup |

---

## Task 7: `vault backup-info` Command

**Goal:** Show backup status without decrypting.

**New file:** `keypo-signer/Sources/KeypoSigner/Backup/BackupInfoCommand.swift`

### Flow

1. Check if backup file exists in iCloud Drive.
2. If exists, read and parse the outer JSON (no decryption needed — metadata is plaintext).
3. Check if `.prev` exists.
4. Check if synced key is available via `syncedBackupKeyExists()`.
5. Read `backup-state.json` for `secrets_since_backup`.
6. Output JSON.

### Tests

| # | Test | Pass | Fail |
|---|------|------|------|
| 7.1 | backup-info when no backup exists | JSON output has `"backup_exists": false` | `true` or error |
| 7.2 | backup-info after a backup | JSON contains correct `created_at`, `device_name`, `secret_count`, `vault_names` matching the backup | Any field wrong or missing |
| 7.3 | backup-info `previous_backup_exists` field | `true` when `.prev` exists, `false` when it doesn't | Wrong value |
| 7.4 | backup-info `synced_key_available` field | `true` when Keychain item exists, `false` when it doesn't | Wrong value |
| 7.5 | backup-info `local_secrets_not_backed_up` field | Matches `secrets_since_backup` from `backup-state.json` | Wrong count |
| 7.6 | backup-info does not prompt for passphrase or decrypt anything | Command completes without any stdin prompt | Prompts for passphrase |

---

## Task 8: Stale Backup Nudge

**Goal:** Warn on `vault set` and `vault import` when >= 5 local secrets are not backed up.

**Changes to existing code:**
- In `VaultSetCommand` and `VaultImportCommand` (existing files), after successful secret storage:
  1. Read `~/.keypo/backup-state.json`
  2. Increment `secrets_since_backup`
  3. Write back
  4. If `secrets_since_backup >= 5`, emit to stderr: `"Note: N secrets not included in your latest backup. Run 'vault backup' to update."`

**New file:** `keypo-signer/Sources/KeypoSigner/Backup/BackupState.swift`

```swift
struct BackupState: Codable {
    var lastBackupAt: String?
    var secretsSinceBackup: Int
}

func readBackupState() -> BackupState
func writeBackupState(_ state: BackupState)
func incrementAndNudge() // read, increment, write, maybe warn
```

### Tests

| # | Test | Pass | Fail |
|---|------|------|------|
| 8.1 | `readBackupState()` when `backup-state.json` doesn't exist | Returns default state with `secretsSinceBackup: 0` | Throws error |
| 8.2 | `writeBackupState()` then `readBackupState()` | Round-trips all fields | Data loss |
| 8.3 | `vault set` increments `secretsSinceBackup` by 1 | Count increases by exactly 1 | Count unchanged or wrong increment |
| 8.4 | `vault import` with 3 secrets increments `secretsSinceBackup` by 3 | Count increases by exactly 3 | Wrong increment |
| 8.5 | `vault set` 4 times (count = 4) | No stderr nudge | Nudge emitted before threshold |
| 8.6 | `vault set` 5 times (count = 5) | stderr contains "5 secrets not included in your latest backup" | No nudge, or wrong count in message |
| 8.7 | `vault set` 10 times (count = 10) | stderr contains "10 secrets not included" | Wrong count |
| 8.8 | `vault backup` resets count, then `vault set` 4 times | No nudge | Nudge appears |
| 8.9 | `vault get` and `vault exec` do not increment count or emit nudge | `secretsSinceBackup` unchanged, no stderr output | Count incremented or nudge emitted |
| 8.10 | Nudge is on stderr, not stdout | stdout is clean JSON output, nudge only on stderr | Nudge on stdout (would break JSON parsing) |

---

## Task 9: Homebrew Formula Update

**Goal:** Install the `.app` bundle and symlink the binary.

**Changes to:** `homebrew/Formula/keypo-signer.rb` and `homebrew/Formula/keypo-wallet.rb`

- Download the notarized `.app` bundle (or build from source and package)
- Install `.app` to `libexec/`
- Symlink `libexec/keypo-signer.app/Contents/MacOS/keypo-signer` → `bin/keypo-signer`
- keypo-wallet formula: since it bundles keypo-signer, same treatment

### Tests

| # | Test | Pass | Fail |
|---|------|------|------|
| 9.1 | `brew install keypo-us/tap/keypo-signer` completes | Exit code 0 | Install failure |
| 9.2 | `which keypo-signer` resolves | Points to Homebrew-managed symlink | Not found |
| 9.3 | `keypo-signer --help` works | Prints help text | Error or not found |
| 9.4 | `keypo-signer vault backup --help` works | Prints backup command help | Command not recognized |
| 9.5 | `codesign -d --entitlements :- $(which keypo-signer)/../../../..` | Shows `keychain-access-groups` | Missing entitlements |
| 9.6 | `brew install keypo-us/tap/keypo-wallet` includes keypo-signer | Both `keypo-wallet` and `keypo-signer` available in PATH | keypo-signer missing |

---

## Suggested Build Order

| Phase | Tasks | Can Parallelize? |
|-------|-------|-----------------|
| **Phase 1** | Task 1 (.app bundle) + Task 3 (crypto module) | Yes — independent |
| **Phase 2** | Task 2 (iCloud Keychain) — depends on Task 1 | No |
| **Phase 3** | Task 4 (backup) + Task 5 (restore) — depend on Tasks 2 & 3 | Yes — independent of each other |
| **Phase 4** | Task 6 (backup-reset) + Task 7 (backup-info) + Task 8 (nudge) | Yes — all independent, depend on Task 4 |
| **Phase 5** | Task 9 (Homebrew) | After all commands land |
