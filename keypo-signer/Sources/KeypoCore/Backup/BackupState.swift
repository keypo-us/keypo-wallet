import Foundation

/// Tracks backup state to enable stale-backup nudges.
public class BackupStateManager {
    public let configDir: URL

    public init(configDir: URL) {
        self.configDir = configDir
    }

    private var stateFilePath: URL {
        configDir.appendingPathComponent("backup-state.json")
    }

    // MARK: - Read / Write

    /// Read backup state. Returns default state if file doesn't exist.
    public func read() -> BackupState {
        let fm = FileManager.default
        guard fm.fileExists(atPath: stateFilePath.path) else {
            return BackupState(lastBackupAt: nil, secretsSinceBackup: 0)
        }
        do {
            let data = try Data(contentsOf: stateFilePath)
            return try JSONDecoder().decode(BackupState.self, from: data)
        } catch {
            // Corrupt state file — return default rather than crash
            return BackupState(lastBackupAt: nil, secretsSinceBackup: 0)
        }
    }

    /// Write backup state atomically.
    public func write(_ state: BackupState) throws {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        let data = try encoder.encode(state)

        let fm = FileManager.default
        // Ensure config dir exists
        if !fm.fileExists(atPath: configDir.path) {
            try fm.createDirectory(at: configDir, withIntermediateDirectories: true)
        }

        let tempPath = configDir.appendingPathComponent("backup-state.json.tmp")
        try data.write(to: tempPath, options: .atomic)

        if fm.fileExists(atPath: stateFilePath.path) {
            _ = try fm.replaceItemAt(stateFilePath, withItemAt: tempPath)
        } else {
            try fm.moveItem(at: tempPath, to: stateFilePath)
        }
    }

    /// Reset state after a successful backup.
    public func resetAfterBackup() throws {
        let formatter = ISO8601DateFormatter()
        let state = BackupState(
            lastBackupAt: formatter.string(from: Date()),
            secretsSinceBackup: 0
        )
        try write(state)
    }

    // MARK: - Nudge

    /// Increment the secrets-since-backup counter and emit a stderr nudge if >= 5.
    /// Call this after `vault set` or `vault import`.
    public func incrementAndNudge(count: Int = 1) throws {
        var state = read()
        state.secretsSinceBackup += count
        try write(state)

        if state.secretsSinceBackup >= 5 {
            let msg = "Note: \(state.secretsSinceBackup) secrets not included in your latest backup. Run 'vault backup' to update.\n"
            FileHandle.standardError.write(Data(msg.utf8))
        }
    }
}

/// Persisted backup state.
public struct BackupState: Codable {
    public var lastBackupAt: String?
    public var secretsSinceBackup: Int

    public init(lastBackupAt: String?, secretsSinceBackup: Int) {
        self.lastBackupAt = lastBackupAt
        self.secretsSinceBackup = secretsSinceBackup
    }
}
