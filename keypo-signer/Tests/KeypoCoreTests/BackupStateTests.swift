import XCTest
@testable import KeypoCore

final class BackupStateTests: XCTestCase {

    private var tempDir: URL!
    private var stateManager: BackupStateManager!

    override func setUp() {
        super.setUp()
        tempDir = FileManager.default.temporaryDirectory
            .appendingPathComponent("keypo-test-\(UUID().uuidString)")
        try? FileManager.default.createDirectory(at: tempDir, withIntermediateDirectories: true)
        stateManager = BackupStateManager(configDir: tempDir)
    }

    override func tearDown() {
        try? FileManager.default.removeItem(at: tempDir)
        super.tearDown()
    }

    // Test 8.1: readBackupState when file doesn't exist returns defaults
    func testReadDefaultState() {
        let state = stateManager.read()
        XCTAssertNil(state.lastBackupAt)
        XCTAssertEqual(state.secretsSinceBackup, 0)
    }

    // Test 8.2: write then read round-trips all fields
    func testWriteReadRoundTrip() throws {
        let state = BackupState(lastBackupAt: "2024-01-01T00:00:00Z", secretsSinceBackup: 7)
        try stateManager.write(state)

        let read = stateManager.read()
        XCTAssertEqual(read.lastBackupAt, "2024-01-01T00:00:00Z")
        XCTAssertEqual(read.secretsSinceBackup, 7)
    }

    // Test: incrementAndNudge increments count correctly
    func testIncrementAndNudge() throws {
        try stateManager.incrementAndNudge()
        XCTAssertEqual(stateManager.read().secretsSinceBackup, 1)

        try stateManager.incrementAndNudge(count: 3)
        XCTAssertEqual(stateManager.read().secretsSinceBackup, 4)
    }

    // Test: resetAfterBackup resets count to 0
    func testResetAfterBackup() throws {
        try stateManager.incrementAndNudge(count: 10)
        XCTAssertEqual(stateManager.read().secretsSinceBackup, 10)

        try stateManager.resetAfterBackup()
        let state = stateManager.read()
        XCTAssertEqual(state.secretsSinceBackup, 0)
        XCTAssertNotNil(state.lastBackupAt)
    }
}
