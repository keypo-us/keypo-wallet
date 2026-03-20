import Foundation

/// A name+policy pair for diff display.
public struct SecretRef: Codable, Equatable {
    public let name: String
    public let policy: String

    public init(name: String, policy: String) {
        self.name = name
        self.policy = policy
    }
}

/// Result of comparing local vault secrets with a backup payload.
public struct RestoreDiff {
    public let localOnly: [SecretRef]
    public let backupOnly: [SecretRef]
    public let inBoth: [SecretRef]

    public init(localOnly: [SecretRef], backupOnly: [SecretRef], inBoth: [SecretRef]) {
        self.localOnly = localOnly
        self.backupOnly = backupOnly
        self.inBoth = inBoth
    }
}

/// Compute a name-level diff between local vault secrets and a backup payload.
///
/// - `localSecrets`: from `VaultStore.allSecretNames()` — no decryption needed
/// - `backupPayload`: the decrypted `BackupPayload` from the backup blob
///
/// Names are globally unique across policies (enforced by `isNameGloballyUnique`).
/// If the same name exists under different policies in local vs backup, it's classified
/// as "inBoth" — local version wins on merge, display uses the local policy.
public func computeRestoreDiff(
    localSecrets: [(name: String, policy: KeyPolicy)],
    backupPayload: BackupPayload
) -> RestoreDiff {
    // Build lookup: name → local policy
    var localMap: [String: KeyPolicy] = [:]
    for entry in localSecrets {
        localMap[entry.name] = entry.policy
    }

    // Build lookup: name → backup policy (from vault name, which is authoritative)
    var backupMap: [String: String] = [:]
    for vault in backupPayload.vaults {
        for secret in vault.secrets {
            backupMap[secret.name] = vault.name
        }
    }

    let localNames = Set(localMap.keys)
    let backupNames = Set(backupMap.keys)

    let localOnlyNames = localNames.subtracting(backupNames)
    let backupOnlyNames = backupNames.subtracting(localNames)
    let inBothNames = localNames.intersection(backupNames)

    let localOnly = localOnlyNames.sorted().map { name in
        SecretRef(name: name, policy: localMap[name]!.rawValue)
    }

    let backupOnly = backupOnlyNames.sorted().map { name in
        SecretRef(name: name, policy: backupMap[name]!)
    }

    // "inBoth" uses the local policy for display (local version wins on merge)
    let inBoth = inBothNames.sorted().map { name in
        SecretRef(name: name, policy: localMap[name]!.rawValue)
    }

    return RestoreDiff(localOnly: localOnly, backupOnly: backupOnly, inBoth: inBoth)
}
