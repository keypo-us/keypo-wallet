import XCTest
@testable import KeypoCore

final class BackupDiffTests: XCTestCase {

    // Helper to build a BackupPayload from a list of (name, policy) tuples
    private func makePayload(_ secrets: [(name: String, policy: String)]) -> BackupPayload {
        var vaultMap: [String: [BackupSecret]] = [:]
        for (name, policy) in secrets {
            let secret = BackupSecret(
                name: name, value: "val", policy: policy,
                createdAt: "2024-01-01T00:00:00Z", updatedAt: "2024-01-01T00:00:00Z"
            )
            vaultMap[policy, default: []].append(secret)
        }
        let vaults = vaultMap.map { BackupVault(name: $0.key, secrets: $0.value) }
        return BackupPayload(vaults: vaults)
    }

    // Test: empty local, non-empty backup → all backup-only
    func testEmptyLocalNonEmptyBackup() {
        let payload = makePayload([
            ("API_KEY", "open"),
            ("DB_PASS", "passcode"),
        ])
        let diff = computeRestoreDiff(localSecrets: [], backupPayload: payload)

        XCTAssertTrue(diff.localOnly.isEmpty)
        XCTAssertEqual(diff.backupOnly.count, 2)
        XCTAssertTrue(diff.inBoth.isEmpty)
        XCTAssertEqual(diff.backupOnly.map(\.name), ["API_KEY", "DB_PASS"])
    }

    // Test: non-empty local, empty backup → all local-only
    func testNonEmptyLocalEmptyBackup() {
        let local: [(name: String, policy: KeyPolicy)] = [
            (name: "SECRET_A", policy: .open),
            (name: "SECRET_B", policy: .biometric),
        ]
        let payload = makePayload([])
        let diff = computeRestoreDiff(localSecrets: local, backupPayload: payload)

        XCTAssertEqual(diff.localOnly.count, 2)
        XCTAssertTrue(diff.backupOnly.isEmpty)
        XCTAssertTrue(diff.inBoth.isEmpty)
        XCTAssertEqual(diff.localOnly.map(\.name), ["SECRET_A", "SECRET_B"])
    }

    // Test: overlapping names → correct "inBoth" classification
    func testOverlappingNames() {
        let local: [(name: String, policy: KeyPolicy)] = [
            (name: "API_KEY", policy: .open),
            (name: "LOCAL_ONLY", policy: .open),
        ]
        let payload = makePayload([
            ("API_KEY", "open"),
            ("BACKUP_ONLY", "passcode"),
        ])
        let diff = computeRestoreDiff(localSecrets: local, backupPayload: payload)

        XCTAssertEqual(diff.localOnly.count, 1)
        XCTAssertEqual(diff.localOnly[0].name, "LOCAL_ONLY")
        XCTAssertEqual(diff.backupOnly.count, 1)
        XCTAssertEqual(diff.backupOnly[0].name, "BACKUP_ONLY")
        XCTAssertEqual(diff.inBoth.count, 1)
        XCTAssertEqual(diff.inBoth[0].name, "API_KEY")
    }

    // Test: same name, different policy → classified as "inBoth", local policy shown
    func testSameNameDifferentPolicy() {
        let local: [(name: String, policy: KeyPolicy)] = [
            (name: "TOKEN", policy: .biometric),
        ]
        let payload = makePayload([
            ("TOKEN", "open"),
        ])
        let diff = computeRestoreDiff(localSecrets: local, backupPayload: payload)

        XCTAssertTrue(diff.localOnly.isEmpty)
        XCTAssertTrue(diff.backupOnly.isEmpty)
        XCTAssertEqual(diff.inBoth.count, 1)
        XCTAssertEqual(diff.inBoth[0].name, "TOKEN")
        // Local policy wins for display
        XCTAssertEqual(diff.inBoth[0].policy, "biometric")
    }

    // Test: both empty → all three arrays empty
    func testBothEmpty() {
        let diff = computeRestoreDiff(localSecrets: [], backupPayload: makePayload([]))

        XCTAssertTrue(diff.localOnly.isEmpty)
        XCTAssertTrue(diff.backupOnly.isEmpty)
        XCTAssertTrue(diff.inBoth.isEmpty)
    }

    // Test: backup is strict subset of local → backupOnly is empty
    func testBackupStrictSubsetOfLocal() {
        let local: [(name: String, policy: KeyPolicy)] = [
            (name: "A", policy: .open),
            (name: "B", policy: .passcode),
            (name: "C", policy: .biometric),
        ]
        let payload = makePayload([
            ("A", "open"),
            ("B", "passcode"),
        ])
        let diff = computeRestoreDiff(localSecrets: local, backupPayload: payload)

        XCTAssertEqual(diff.localOnly.count, 1)
        XCTAssertEqual(diff.localOnly[0].name, "C")
        XCTAssertTrue(diff.backupOnly.isEmpty)
        XCTAssertEqual(diff.inBoth.count, 2)
    }

    // Test: backup-only secrets spanning multiple policies
    func testBackupOnlyMultiplePolicies() {
        let local: [(name: String, policy: KeyPolicy)] = [
            (name: "EXISTING", policy: .open),
        ]
        let payload = makePayload([
            ("EXISTING", "open"),
            ("NEW_OPEN", "open"),
            ("NEW_PASSCODE", "passcode"),
        ])
        let diff = computeRestoreDiff(localSecrets: local, backupPayload: payload)

        XCTAssertTrue(diff.localOnly.isEmpty)
        XCTAssertEqual(diff.backupOnly.count, 2)
        XCTAssertEqual(diff.inBoth.count, 1)

        // Check policies are correct
        let backupOnlyDict = Dictionary(uniqueKeysWithValues: diff.backupOnly.map { ($0.name, $0.policy) })
        XCTAssertEqual(backupOnlyDict["NEW_OPEN"], "open")
        XCTAssertEqual(backupOnlyDict["NEW_PASSCODE"], "passcode")
    }

    // Test: results are sorted by name
    func testResultsSorted() {
        let local: [(name: String, policy: KeyPolicy)] = [
            (name: "ZEBRA", policy: .open),
            (name: "ALPHA", policy: .open),
        ]
        let payload = makePayload([
            ("ZULU", "open"),
            ("BRAVO", "passcode"),
        ])
        let diff = computeRestoreDiff(localSecrets: local, backupPayload: payload)

        XCTAssertEqual(diff.localOnly.map(\.name), ["ALPHA", "ZEBRA"])
        XCTAssertEqual(diff.backupOnly.map(\.name), ["BRAVO", "ZULU"])
    }
}
