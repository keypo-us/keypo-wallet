import XCTest
@testable import KeypoCore

final class BackupBlobTests: XCTestCase {

    // Test 4.1: BackupBlob encode to JSON then decode round-trips all fields
    func testBackupBlobRoundTrip() throws {
        let blob = BackupBlob(
            version: 1,
            createdAt: "2024-01-01T00:00:00Z",
            deviceName: "Test Mac",
            argon2Salt: "AAAAAAAAAAAAAAAAAAAAAA==",
            hkdfSalt: "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
            nonce: "AAAAAAAAAAAAAAA=",
            ciphertext: "dGVzdA==",
            authTag: "AAAAAAAAAAAAAAAAAAAAAA==",
            secretCount: 5,
            vaultNames: ["open", "passcode"]
        )

        let encoder = JSONEncoder()
        let data = try encoder.encode(blob)
        let decoded = try decodeBackupBlob(from: data)

        XCTAssertEqual(decoded.version, 1)
        XCTAssertEqual(decoded.createdAt, "2024-01-01T00:00:00Z")
        XCTAssertEqual(decoded.deviceName, "Test Mac")
        XCTAssertEqual(decoded.argon2Salt, blob.argon2Salt)
        XCTAssertEqual(decoded.hkdfSalt, blob.hkdfSalt)
        XCTAssertEqual(decoded.nonce, blob.nonce)
        XCTAssertEqual(decoded.ciphertext, blob.ciphertext)
        XCTAssertEqual(decoded.authTag, blob.authTag)
        XCTAssertEqual(decoded.secretCount, 5)
        XCTAssertEqual(decoded.vaultNames, ["open", "passcode"])
    }

    // Test 4.2: BackupBlob with version 1 decodes
    func testBackupBlobVersion1Decodes() throws {
        let json = """
        {"version":1,"createdAt":"2024-01-01T00:00:00Z","deviceName":"Mac",
         "argon2Salt":"AA==","hkdfSalt":"AA==","nonce":"AA==",
         "ciphertext":"AA==","authTag":"AA==","secretCount":0,"vaultNames":[]}
        """
        let blob = try decodeBackupBlob(from: Data(json.utf8))
        XCTAssertEqual(blob.version, 1)
    }

    // Test 4.3: BackupBlob with unknown version fails
    func testBackupBlobUnknownVersionFails() {
        let json = """
        {"version":99,"createdAt":"2024-01-01T00:00:00Z","deviceName":"Mac",
         "argon2Salt":"AA==","hkdfSalt":"AA==","nonce":"AA==",
         "ciphertext":"AA==","authTag":"AA==","secretCount":0,"vaultNames":[]}
        """
        XCTAssertThrowsError(try decodeBackupBlob(from: Data(json.utf8))) { error in
            XCTAssertTrue(error is BackupBlobError)
            if case BackupBlobError.unsupportedVersion(let v) = error {
                XCTAssertEqual(v, 99)
            }
        }
    }

    // Test 4.4: BackupPayload with multiple vaults and secrets round-trips
    func testBackupPayloadRoundTrip() throws {
        let payload = BackupPayload(vaults: [
            BackupVault(name: "open", secrets: [
                BackupSecret(name: "API_KEY", value: "secret123", policy: "open",
                           createdAt: "2024-01-01T00:00:00Z", updatedAt: "2024-01-01T00:00:00Z"),
                BackupSecret(name: "DB_PASS", value: "dbpass", policy: "open",
                           createdAt: "2024-01-01T00:00:00Z", updatedAt: "2024-01-02T00:00:00Z"),
            ]),
            BackupVault(name: "passcode", secrets: [
                BackupSecret(name: "SIGNING_KEY", value: "0xdeadbeef", policy: "passcode",
                           createdAt: "2024-01-01T00:00:00Z", updatedAt: "2024-01-01T00:00:00Z"),
            ]),
            BackupVault(name: "biometric", secrets: [
                BackupSecret(name: "MASTER_KEY", value: "masterkey", policy: "biometric",
                           createdAt: "2024-01-01T00:00:00Z", updatedAt: "2024-01-01T00:00:00Z"),
            ]),
        ])

        let encoder = JSONEncoder()
        let data = try encoder.encode(payload)
        let decoded = try JSONDecoder().decode(BackupPayload.self, from: data)

        XCTAssertEqual(decoded.vaults.count, 3)
        XCTAssertEqual(decoded.vaults[0].name, "open")
        XCTAssertEqual(decoded.vaults[0].secrets.count, 2)
        XCTAssertEqual(decoded.vaults[0].secrets[0].name, "API_KEY")
        XCTAssertEqual(decoded.vaults[0].secrets[0].value, "secret123")
        XCTAssertEqual(decoded.vaults[1].secrets[0].name, "SIGNING_KEY")
        XCTAssertEqual(decoded.vaults[2].secrets[0].name, "MASTER_KEY")
        XCTAssertEqual(decoded.vaults[2].secrets[0].value, "masterkey")
    }
}
