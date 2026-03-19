import XCTest
@testable import KeypoCore

final class iCloudDriveTests: XCTestCase {

    private var tempDir: URL!
    private var driveManager: iCloudDriveManager!

    override func setUp() {
        super.setUp()
        tempDir = FileManager.default.temporaryDirectory
            .appendingPathComponent("keypo-test-\(UUID().uuidString)")
        driveManager = iCloudDriveManager(baseDir: tempDir)
    }

    override func tearDown() {
        try? FileManager.default.removeItem(at: tempDir)
        super.tearDown()
    }

    private func makeBlob(secretCount: Int = 3) -> BackupBlob {
        BackupBlob(
            version: 1,
            createdAt: "2024-01-01T00:00:00Z",
            deviceName: "Test Mac",
            argon2Salt: "AAAAAAAAAAAAAAAAAAAAAA==",
            hkdfSalt: "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
            nonce: "AAAAAAAAAAAAAAA=",
            ciphertext: "dGVzdA==",
            authTag: "AAAAAAAAAAAAAAAAAAAAAA==",
            secretCount: secretCount,
            vaultNames: ["open"]
        )
    }

    // Test 4.5: writeBackup when no previous backup exists
    func testWriteFirstBackup() throws {
        let blob = makeBlob()
        try driveManager.writeBackup(blob, isFirstBackup: true)

        XCTAssertTrue(driveManager.backupExists())
        XCTAssertFalse(driveManager.backupExists(previous: true))
    }

    // Test 4.6: writeBackup when previous backup exists (rotation)
    func testWriteRotatesBackup() throws {
        let blob1 = makeBlob(secretCount: 1)
        try driveManager.writeBackup(blob1, isFirstBackup: true)

        let blob2 = makeBlob(secretCount: 2)
        try driveManager.writeBackup(blob2, isFirstBackup: false)

        XCTAssertTrue(driveManager.backupExists())
        XCTAssertTrue(driveManager.backupExists(previous: true))

        let current = try driveManager.readBackup()
        XCTAssertEqual(current?.secretCount, 2)

        let prev = try driveManager.readBackup(previous: true)
        XCTAssertEqual(prev?.secretCount, 1)
    }

    // Test 4.7: writeBackup when both current and prev exist
    func testWriteOverwritesPrev() throws {
        let blob1 = makeBlob(secretCount: 1)
        try driveManager.writeBackup(blob1, isFirstBackup: true)

        let blob2 = makeBlob(secretCount: 2)
        try driveManager.writeBackup(blob2, isFirstBackup: false)

        let blob3 = makeBlob(secretCount: 3)
        try driveManager.writeBackup(blob3, isFirstBackup: false)

        let current = try driveManager.readBackup()
        XCTAssertEqual(current?.secretCount, 3)

        let prev = try driveManager.readBackup(previous: true)
        XCTAssertEqual(prev?.secretCount, 2)
    }

    // Test 4.8: readBackup reads a file written by writeBackup
    func testReadBackup() throws {
        let blob = makeBlob(secretCount: 5)
        try driveManager.writeBackup(blob, isFirstBackup: true)

        let read = try driveManager.readBackup()
        XCTAssertNotNil(read)
        XCTAssertEqual(read?.secretCount, 5)
        XCTAssertEqual(read?.deviceName, "Test Mac")
        XCTAssertEqual(read?.version, 1)
    }

    // Test 4.9: readBackup returns nil when no file exists
    func testReadBackupNilWhenMissing() throws {
        let result = try driveManager.readBackup()
        XCTAssertNil(result)
    }

    // Test 4.10: readBackup(previous: true) reads the .prev file
    func testReadPreviousBackup() throws {
        let blob1 = makeBlob(secretCount: 10)
        try driveManager.writeBackup(blob1, isFirstBackup: true)

        let blob2 = makeBlob(secretCount: 20)
        try driveManager.writeBackup(blob2, isFirstBackup: false)

        let prev = try driveManager.readBackup(previous: true)
        XCTAssertEqual(prev?.secretCount, 10)
    }

    // Test 4.11: backupExists returns correct booleans
    func testBackupExists() throws {
        XCTAssertFalse(driveManager.backupExists())

        let blob = makeBlob()
        try driveManager.writeBackup(blob, isFirstBackup: true)
        XCTAssertTrue(driveManager.backupExists())
    }

    // Test 4.12: Directory is created if it doesn't exist
    func testDirectoryCreated() throws {
        XCTAssertFalse(FileManager.default.fileExists(atPath: tempDir.path))

        let blob = makeBlob()
        try driveManager.writeBackup(blob, isFirstBackup: true)

        XCTAssertTrue(FileManager.default.fileExists(atPath: tempDir.path))
    }
}
