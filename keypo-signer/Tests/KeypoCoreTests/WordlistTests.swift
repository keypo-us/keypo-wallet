import XCTest
@testable import KeypoCore

final class WordlistTests: XCTestCase {

    // Test 3.1: Wordlist has exactly 2048 entries
    func testWordlistCount() {
        XCTAssertEqual(Wordlist.english.count, 2048)
    }

    // Test 3.2: Every word is lowercase alpha only
    func testWordlistAllLowercaseAlpha() {
        let pattern = try! NSRegularExpression(pattern: "^[a-z]+$")
        for (index, word) in Wordlist.english.enumerated() {
            let range = NSRange(word.startIndex..., in: word)
            let match = pattern.firstMatch(in: word, range: range)
            XCTAssertNotNil(match, "Word at index \(index) ('\(word)') contains non-alpha characters")
        }
    }

    // Test 3.3: No duplicate words
    func testWordlistNoDuplicates() {
        let uniqueSet = Set(Wordlist.english)
        XCTAssertEqual(uniqueSet.count, 2048, "Wordlist contains duplicate entries")
    }
}
