import Foundation
import CryptoKit
import Security
import Sodium

/// Holds the derived backup key along with the salts used in derivation.
public struct BackupKeys {
    public let argon2Salt: Data    // 16 bytes
    public let hkdfSalt: Data      // 32 bytes
    public let backupKey: Data     // 32 bytes (derived)

    public init(argon2Salt: Data, hkdfSalt: Data, backupKey: Data) {
        self.argon2Salt = argon2Salt
        self.hkdfSalt = hkdfSalt
        self.backupKey = backupKey
    }
}

/// Backup encryption errors.
public enum BackupCryptoError: Error, CustomStringConvertible {
    case argon2Failed(String)
    case randomGenerationFailed
    case encryptionFailed(String)
    case decryptionFailed(String)

    public var description: String {
        switch self {
        case .argon2Failed(let msg): return "Argon2id key derivation failed: \(msg)"
        case .randomGenerationFailed: return "secure random generation failed"
        case .encryptionFailed(let msg): return "backup encryption failed: \(msg)"
        case .decryptionFailed(let msg): return "backup decryption failed: \(msg)"
        }
    }
}

/// Backup crypto: Argon2id + HKDF-SHA256 + AES-256-GCM.
public enum BackupCrypto {

    // MARK: - Key Derivation (new salts)

    /// Derive a backup key from a synced key and passphrase, generating fresh random salts.
    /// Used for first-time backup and backup-reset.
    public static func deriveBackupKey(syncedKey: Data, passphrase: String) throws -> BackupKeys {
        let argon2Salt = try secureRandom(count: 16)
        let hkdfSalt = try secureRandom(count: 32)

        let backupKey = try deriveBackupKey(
            syncedKey: syncedKey,
            passphrase: passphrase,
            argon2Salt: argon2Salt,
            hkdfSalt: hkdfSalt
        )

        return BackupKeys(argon2Salt: argon2Salt, hkdfSalt: hkdfSalt, backupKey: backupKey)
    }

    // MARK: - Key Derivation (existing salts)

    /// Derive a backup key from a synced key and passphrase using existing salts.
    /// Used for restore and verification.
    public static func deriveBackupKey(syncedKey: Data, passphrase: String,
                                       argon2Salt: Data, hkdfSalt: Data) throws -> Data {
        // Step 1: Argon2id — harden the passphrase
        let sodium = Sodium()
        let passphraseBytes = Array(passphrase.utf8)

        guard let argon2Output = sodium.pwHash.hash(
            outputLength: 32,
            passwd: passphraseBytes,
            salt: Array(argon2Salt),
            opsLimit: 3,
            memLimit: 67_108_864,  // 64 MB
            alg: .Argon2ID13
        ) else {
            throw BackupCryptoError.argon2Failed("Sodium pwHash returned nil")
        }

        // Step 2: HKDF-SHA256 — combine hardened passphrase with synced key
        let ikm = syncedKey + Data(argon2Output)
        let inputKey = SymmetricKey(data: ikm)
        let derivedKey = HKDF<SHA256>.deriveKey(
            inputKeyMaterial: inputKey,
            salt: hkdfSalt,
            info: Data("keypo-vault-backup-v1".utf8),
            outputByteCount: 32
        )

        return derivedKey.withUnsafeBytes { buffer in
            Data(bytes: buffer.baseAddress!, count: buffer.count)
        }
    }

    // MARK: - AES-256-GCM Encryption

    /// Encrypt plaintext with AES-256-GCM.
    /// - Returns: Tuple of (nonce, ciphertext, authTag).
    public static func encrypt(plaintext: Data, key: Data) throws -> (nonce: Data, ciphertext: Data, authTag: Data) {
        let symmetricKey = SymmetricKey(data: key)
        let sealedBox: AES.GCM.SealedBox
        do {
            sealedBox = try AES.GCM.seal(plaintext, using: symmetricKey)
        } catch {
            throw BackupCryptoError.encryptionFailed(error.localizedDescription)
        }
        return (
            nonce: Data(sealedBox.nonce),
            ciphertext: Data(sealedBox.ciphertext),
            authTag: Data(sealedBox.tag)
        )
    }

    // MARK: - AES-256-GCM Decryption

    /// Decrypt ciphertext with AES-256-GCM.
    /// - Returns: Decrypted plaintext.
    public static func decrypt(ciphertext: Data, nonce: Data, authTag: Data, key: Data) throws -> Data {
        let symmetricKey = SymmetricKey(data: key)
        do {
            let gcmNonce = try AES.GCM.Nonce(data: nonce)
            let sealedBox = try AES.GCM.SealedBox(nonce: gcmNonce, ciphertext: ciphertext, tag: authTag)
            return try AES.GCM.open(sealedBox, using: symmetricKey)
        } catch {
            throw BackupCryptoError.decryptionFailed(error.localizedDescription)
        }
    }

    // MARK: - Secure Random

    /// Generate cryptographically secure random bytes.
    public static func secureRandom(count: Int) throws -> Data {
        var bytes = [UInt8](repeating: 0, count: count)
        let status = SecRandomCopyBytes(kSecRandomDefault, count, &bytes)
        guard status == errSecSuccess else {
            throw BackupCryptoError.randomGenerationFailed
        }
        return Data(bytes)
    }
}
