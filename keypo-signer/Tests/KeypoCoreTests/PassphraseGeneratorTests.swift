import XCTest
@testable import KeypoCore

final class PassphraseGeneratorTests: XCTestCase {

    // Test 3.4: generatePassphrase returns 4 words
    func testGeneratePassphraseWordCount() {
        let words = PassphraseGenerator.generatePassphrase()
        XCTAssertEqual(words.count, 4)
    }

    // Test 3.5: Every generated word is in the BIP-39 wordlist
    func testGeneratePassphraseWordsInWordlist() {
        let wordlist = Set(Wordlist.english)
        let words = PassphraseGenerator.generatePassphrase()
        for word in words {
            XCTAssertTrue(wordlist.contains(word), "Generated word '\(word)' is not in the BIP-39 wordlist")
        }
    }

    // Test 3.6: 100 calls produce at least 90 distinct passphrases (randomness check)
    func testGeneratePassphraseRandomness() {
        var passphrases = Set<String>()
        for _ in 0..<100 {
            let words = PassphraseGenerator.generatePassphrase()
            passphrases.insert(words.joined(separator: " "))
        }
        XCTAssertGreaterThanOrEqual(passphrases.count, 90,
            "Only \(passphrases.count) unique passphrases out of 100 — weak randomness")
    }

    // Test 3.7: confirmationIndices returns correct count with valid range
    func testConfirmationIndices() {
        let indices = PassphraseGenerator.confirmationIndices(wordCount: 4, confirmCount: 2)
        XCTAssertEqual(indices.count, 2, "Should return exactly 2 indices")

        // Check all indices are in valid range
        for index in indices {
            XCTAssertGreaterThanOrEqual(index, 0)
            XCTAssertLessThan(index, 4)
        }

        // Check indices are unique
        XCTAssertEqual(Set(indices).count, 2, "Indices should be distinct")

        // Check indices are sorted
        XCTAssertEqual(indices, indices.sorted(), "Indices should be sorted")
    }
}
