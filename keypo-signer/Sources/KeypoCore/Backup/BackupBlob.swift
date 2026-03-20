import Foundation

/// Top-level backup file stored in iCloud Drive.
public struct BackupBlob: Codable {
    public let version: Int                // 1
    public let createdAt: String           // ISO 8601
    public let deviceName: String          // Host.current().localizedName
    public let argon2Salt: String          // base64
    public let hkdfSalt: String            // base64
    public let nonce: String               // base64
    public let ciphertext: String          // base64
    public let authTag: String             // base64
    public let secretCount: Int
    public let vaultNames: [String]

    public init(version: Int, createdAt: String, deviceName: String,
                argon2Salt: String, hkdfSalt: String,
                nonce: String, ciphertext: String, authTag: String,
                secretCount: Int, vaultNames: [String]) {
        self.version = version
        self.createdAt = createdAt
        self.deviceName = deviceName
        self.argon2Salt = argon2Salt
        self.hkdfSalt = hkdfSalt
        self.nonce = nonce
        self.ciphertext = ciphertext
        self.authTag = authTag
        self.secretCount = secretCount
        self.vaultNames = vaultNames
    }
}

/// Errors for backup blob operations.
public enum BackupBlobError: Error, CustomStringConvertible {
    case unsupportedVersion(Int)
    case decodingFailed(String)

    public var description: String {
        switch self {
        case .unsupportedVersion(let v):
            return "unsupported backup version: \(v). This backup was created by a newer version of keypo-signer."
        case .decodingFailed(let msg):
            return "failed to decode backup: \(msg)"
        }
    }
}

/// Decode a BackupBlob with version validation.
public func decodeBackupBlob(from data: Data) throws -> BackupBlob {
    let decoder = JSONDecoder()
    let blob: BackupBlob
    do {
        blob = try decoder.decode(BackupBlob.self, from: data)
    } catch {
        throw BackupBlobError.decodingFailed(error.localizedDescription)
    }
    guard blob.version == 1 else {
        throw BackupBlobError.unsupportedVersion(blob.version)
    }
    return blob
}

/// The decrypted payload inside a backup blob.
public struct BackupPayload: Codable {
    public let vaults: [BackupVault]

    public init(vaults: [BackupVault]) {
        self.vaults = vaults
    }
}

/// A single vault within the backup payload.
public struct BackupVault: Codable {
    public let name: String
    public let secrets: [BackupSecret]

    public init(name: String, secrets: [BackupSecret]) {
        self.name = name
        self.secrets = secrets
    }
}

/// A single secret within a backup vault.
public struct BackupSecret: Codable {
    public let name: String
    public let value: String
    public let policy: String
    public let createdAt: String
    public let updatedAt: String

    public init(name: String, value: String, policy: String,
                createdAt: String, updatedAt: String) {
        self.name = name
        self.value = value
        self.policy = policy
        self.createdAt = createdAt
        self.updatedAt = updatedAt
    }
}
