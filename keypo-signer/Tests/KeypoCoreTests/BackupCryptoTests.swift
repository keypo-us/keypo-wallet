import XCTest
@testable import KeypoCore

final class BackupCryptoTests: XCTestCase {

    // MARK: - Test Helpers

    private func randomData(_ count: Int) throws -> Data {
        try BackupCrypto.secureRandom(count: count)
    }

    // MARK: - Key Derivation Tests

    // Test 3.8: deriveBackupKey with fresh salts then re-derive with same salts produces same key
    func testDeriveBackupKeyDeterministic() throws {
        let syncedKey = try randomData(32)
        let passphrase = "correct horse battery staple"

        let keys = try BackupCrypto.deriveBackupKey(syncedKey: syncedKey, passphrase: passphrase)
        let reDerived = try BackupCrypto.deriveBackupKey(
            syncedKey: syncedKey,
            passphrase: passphrase,
            argon2Salt: keys.argon2Salt,
            hkdfSalt: keys.hkdfSalt
        )

        XCTAssertEqual(keys.backupKey, reDerived, "Re-derived key should match original")
    }

    // Test 3.9: Two fresh derivations produce different keys (random salts)
    func testDeriveBackupKeyDifferentSalts() throws {
        let syncedKey = try randomData(32)
        let passphrase = "test passphrase"

        let keys1 = try BackupCrypto.deriveBackupKey(syncedKey: syncedKey, passphrase: passphrase)
        let keys2 = try BackupCrypto.deriveBackupKey(syncedKey: syncedKey, passphrase: passphrase)

        XCTAssertNotEqual(keys1.backupKey, keys2.backupKey, "Different salts should produce different keys")
    }

    // Test 3.10: Encrypt then decrypt with correct key
    func testEncryptDecryptRoundTrip() throws {
        let key = try randomData(32)
        let plaintext = Data("hello vault backup".utf8)

        let (nonce, ciphertext, authTag) = try BackupCrypto.encrypt(plaintext: plaintext, key: key)
        let decrypted = try BackupCrypto.decrypt(ciphertext: ciphertext, nonce: nonce, authTag: authTag, key: key)

        XCTAssertEqual(decrypted, plaintext, "Decrypted data should match original plaintext")
    }

    // Test 3.11: Decrypt with wrong key fails
    func testDecryptWrongKeyFails() throws {
        let key = try randomData(32)
        var wrongKey = key
        wrongKey[0] ^= 0x01  // Flip one bit

        let plaintext = Data("secret data".utf8)
        let (nonce, ciphertext, authTag) = try BackupCrypto.encrypt(plaintext: plaintext, key: key)

        XCTAssertThrowsError(
            try BackupCrypto.decrypt(ciphertext: ciphertext, nonce: nonce, authTag: authTag, key: wrongKey)
        ) { error in
            XCTAssertTrue(error is BackupCryptoError, "Should throw BackupCryptoError, got \(type(of: error))")
        }
    }

    // Test 3.12: Decrypt with tampered ciphertext fails
    func testDecryptTamperedCiphertextFails() throws {
        let key = try randomData(32)
        let plaintext = Data("secret data".utf8)

        let (nonce, ciphertext, authTag) = try BackupCrypto.encrypt(plaintext: plaintext, key: key)
        var tampered = ciphertext
        tampered[0] ^= 0xFF

        XCTAssertThrowsError(
            try BackupCrypto.decrypt(ciphertext: tampered, nonce: nonce, authTag: authTag, key: key)
        ) { error in
            XCTAssertTrue(error is BackupCryptoError, "Should throw BackupCryptoError")
        }
    }

    // Test 3.13: Wrong passphrase produces different key
    func testWrongPassphraseProducesDifferentKey() throws {
        let syncedKey = try randomData(32)
        let keys = try BackupCrypto.deriveBackupKey(syncedKey: syncedKey, passphrase: "correct")
        let wrongKey = try BackupCrypto.deriveBackupKey(
            syncedKey: syncedKey,
            passphrase: "wrong",
            argon2Salt: keys.argon2Salt,
            hkdfSalt: keys.hkdfSalt
        )

        XCTAssertNotEqual(keys.backupKey, wrongKey, "Wrong passphrase should produce different key")
    }

    // Test 3.14: Wrong synced key produces different derived key
    func testWrongSyncedKeyProducesDifferentKey() throws {
        let syncedKey = try randomData(32)
        var wrongSyncedKey = syncedKey
        wrongSyncedKey[0] ^= 0x01

        let passphrase = "test passphrase"
        let keys = try BackupCrypto.deriveBackupKey(syncedKey: syncedKey, passphrase: passphrase)
        let wrongDerived = try BackupCrypto.deriveBackupKey(
            syncedKey: wrongSyncedKey,
            passphrase: passphrase,
            argon2Salt: keys.argon2Salt,
            hkdfSalt: keys.hkdfSalt
        )

        XCTAssertNotEqual(keys.backupKey, wrongDerived, "Wrong synced key should produce different key")
    }

    // Test 3.15: Backup key is exactly 32 bytes
    func testBackupKeyLength() throws {
        let syncedKey = try randomData(32)
        let keys = try BackupCrypto.deriveBackupKey(syncedKey: syncedKey, passphrase: "test")
        XCTAssertEqual(keys.backupKey.count, 32)
    }

    // Test 3.16: Argon2 salt is exactly 16 bytes
    func testArgon2SaltLength() throws {
        let syncedKey = try randomData(32)
        let keys = try BackupCrypto.deriveBackupKey(syncedKey: syncedKey, passphrase: "test")
        XCTAssertEqual(keys.argon2Salt.count, 16)
    }

    // Test 3.17: HKDF salt is exactly 32 bytes
    func testHkdfSaltLength() throws {
        let syncedKey = try randomData(32)
        let keys = try BackupCrypto.deriveBackupKey(syncedKey: syncedKey, passphrase: "test")
        XCTAssertEqual(keys.hkdfSalt.count, 32)
    }
}
