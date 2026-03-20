import Foundation

/// Read and write backup blobs to iCloud Drive.
public class iCloudDriveManager {
    public let baseDir: URL

    private let currentFile = "vault-backup.json"
    private let previousFile = "vault-backup.prev.json"

    /// Initialize with a base directory.
    /// - Parameter baseDir: Defaults to `~/Library/Mobile Documents/com~apple~CloudDocs/Keypo/`.
    ///   Pass a custom URL for testing.
    public init(baseDir: URL? = nil) {
        if let dir = baseDir {
            self.baseDir = dir
        } else {
            let home = FileManager.default.homeDirectoryForCurrentUser
            self.baseDir = home
                .appendingPathComponent("Library/Mobile Documents/com~apple~CloudDocs/Keypo")
        }
    }

    private var currentPath: URL { baseDir.appendingPathComponent(currentFile) }
    private var previousPath: URL { baseDir.appendingPathComponent(previousFile) }

    // MARK: - Write

    /// Write a backup blob, rotating current → prev first (unless first backup).
    public func writeBackup(_ blob: BackupBlob, isFirstBackup: Bool) throws {
        let fm = FileManager.default

        // Ensure directory exists
        if !fm.fileExists(atPath: baseDir.path) {
            try fm.createDirectory(at: baseDir, withIntermediateDirectories: true)
        }

        // Rotate: current → prev (overwrite prev if exists)
        if !isFirstBackup && fm.fileExists(atPath: currentPath.path) {
            if fm.fileExists(atPath: previousPath.path) {
                try fm.removeItem(at: previousPath)
            }
            try fm.moveItem(at: currentPath, to: previousPath)
        }

        // Write new current via temp file
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        let data = try encoder.encode(blob)

        let tmpPath = baseDir.appendingPathComponent("vault-backup.json.tmp")
        try data.write(to: tmpPath, options: .atomic)

        if fm.fileExists(atPath: currentPath.path) {
            _ = try fm.replaceItemAt(currentPath, withItemAt: tmpPath)
        } else {
            try fm.moveItem(at: tmpPath, to: currentPath)
        }
    }

    // MARK: - Read

    /// Read a backup blob. Returns nil if file not found.
    public func readBackup(previous: Bool = false) throws -> BackupBlob? {
        let path = previous ? previousPath : currentPath
        let fm = FileManager.default

        guard fm.fileExists(atPath: path.path) else {
            return nil
        }

        let data = try Data(contentsOf: path)
        return try decodeBackupBlob(from: data)
    }

    // MARK: - Exists

    /// Check if a backup file exists.
    public func backupExists(previous: Bool = false) -> Bool {
        let path = previous ? previousPath : currentPath
        return FileManager.default.fileExists(atPath: path.path)
    }
}
