import Foundation
import Security

/// Errors specific to iCloud Keychain sync operations.
public enum KeychainSyncError: Error, CustomStringConvertible {
    case missingEntitlement
    case storeFailed(String)
    case deleteFailed(String)
    case readFailed(String)

    public var description: String {
        switch self {
        case .missingEntitlement:
            return "keypo-signer is missing required entitlements. Ensure you're running the .app-bundled version."
        case .storeFailed(let msg):
            return "failed to store synced backup key: \(msg)"
        case .deleteFailed(let msg):
            return "failed to delete synced backup key: \(msg)"
        case .readFailed(let msg):
            return "failed to read synced backup key: \(msg)"
        }
    }
}

/// Read and write a synchronizable generic password item to iCloud Keychain
/// for vault backup encryption key sync across devices.
public enum KeychainSync {

    private static let service = "com.keypo.vault-backup"
    private static let account = "backup-encryption-key"

    // MARK: - Store

    /// Store a 256-bit key in iCloud Keychain (kSecAttrSynchronizable: true).
    /// If a key already exists, it is deleted and re-added.
    public static func storeSyncedBackupKey(_ keyData: Data) throws {
        let query = baseQuery()
        var addQuery = query
        addQuery[kSecValueData as String] = keyData
        addQuery[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlock

        var status = SecItemAdd(addQuery as CFDictionary, nil)

        if status == errSecDuplicateItem {
            // Delete existing and re-add
            let deleteStatus = SecItemDelete(query as CFDictionary)
            if deleteStatus != errSecSuccess && deleteStatus != errSecItemNotFound {
                throw mapError(deleteStatus, operation: "delete before re-store")
            }
            status = SecItemAdd(addQuery as CFDictionary, nil)
        }

        if status == -34018 {
            throw KeychainSyncError.missingEntitlement
        }

        guard status == errSecSuccess else {
            throw KeychainSyncError.storeFailed("OSStatus \(status)")
        }
    }

    // MARK: - Read

    /// Read the synced backup key. Returns nil if not found.
    public static func readSyncedBackupKey() throws -> Data? {
        var query = baseQuery()
        query[kSecReturnData as String] = true
        query[kSecMatchLimit as String] = kSecMatchLimitOne

        var result: AnyObject?
        let status = SecItemCopyMatching(query as CFDictionary, &result)

        if status == errSecItemNotFound {
            return nil
        }

        if status == -34018 {
            throw KeychainSyncError.missingEntitlement
        }

        guard status == errSecSuccess else {
            throw KeychainSyncError.readFailed("OSStatus \(status)")
        }

        guard let data = result as? Data else {
            throw KeychainSyncError.readFailed("unexpected result type")
        }

        return data
    }

    // MARK: - Delete

    /// Delete the synced backup key (for backup-reset).
    public static func deleteSyncedBackupKey() throws {
        let query = baseQuery()
        let status = SecItemDelete(query as CFDictionary)

        if status == errSecItemNotFound {
            return // Already gone, not an error
        }

        if status == -34018 {
            throw KeychainSyncError.missingEntitlement
        }

        guard status == errSecSuccess else {
            throw KeychainSyncError.deleteFailed("OSStatus \(status)")
        }
    }

    // MARK: - Exists

    /// Check if the synced key exists without reading its data.
    public static func syncedBackupKeyExists() throws -> Bool {
        var query = baseQuery()
        query[kSecReturnAttributes as String] = true
        query[kSecMatchLimit as String] = kSecMatchLimitOne

        var result: AnyObject?
        let status = SecItemCopyMatching(query as CFDictionary, &result)

        if status == errSecItemNotFound {
            return false
        }

        if status == -34018 {
            throw KeychainSyncError.missingEntitlement
        }

        guard status == errSecSuccess else {
            throw KeychainSyncError.readFailed("OSStatus \(status)")
        }

        return true
    }

    // MARK: - Private

    private static func baseQuery() -> [String: Any] {
        return [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
            kSecAttrSynchronizable as String: true,
        ]
    }

    private static func mapError(_ status: OSStatus, operation: String) -> KeychainSyncError {
        if status == -34018 {
            return .missingEntitlement
        }
        return .storeFailed("\(operation): OSStatus \(status)")
    }
}
