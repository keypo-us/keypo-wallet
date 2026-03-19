import Foundation
import Security

/// Generate and validate BIP-39-based passphrases for vault backup.
public enum PassphraseGenerator {

    /// Generate a random passphrase by selecting words from the BIP-39 English wordlist.
    /// Uses `SecRandomCopyBytes` for cryptographic randomness.
    /// - Parameter wordCount: Number of words to generate (default: 4, yielding ~44 bits entropy).
    /// - Returns: Array of lowercase words from the BIP-39 wordlist.
    public static func generatePassphrase(wordCount: Int = 4) -> [String] {
        let wordlist = Wordlist.english
        precondition(wordlist.count == 2048, "BIP-39 wordlist must contain exactly 2048 entries")

        // 2 bytes per word gives us 16 bits of randomness; we use modulo 2048 (11 bits)
        var randomBytes = [UInt8](repeating: 0, count: wordCount * 2)
        let status = SecRandomCopyBytes(kSecRandomDefault, randomBytes.count, &randomBytes)
        precondition(status == errSecSuccess, "SecRandomCopyBytes failed with status \(status)")

        var words: [String] = []
        for i in 0..<wordCount {
            let high = UInt16(randomBytes[i * 2]) << 8
            let low = UInt16(randomBytes[i * 2 + 1])
            let index = Int((high | low) % 2048)
            words.append(wordlist[index])
        }

        // Zeroize random bytes
        for i in 0..<randomBytes.count { randomBytes[i] = 0 }

        return words
    }

    /// Pick `confirmCount` unique random indices from `0..<wordCount` for passphrase confirmation.
    /// Returns sorted indices for deterministic UX ordering.
    public static func confirmationIndices(wordCount: Int, confirmCount: Int) -> [Int] {
        precondition(confirmCount <= wordCount, "confirmCount must be <= wordCount")
        precondition(wordCount > 0 && confirmCount > 0)

        var indices = Array(0..<wordCount)
        var selected: [Int] = []

        // Fisher-Yates partial shuffle with secure random
        for i in 0..<confirmCount {
            var randomBytes = [UInt8](repeating: 0, count: 4)
            let status = SecRandomCopyBytes(kSecRandomDefault, 4, &randomBytes)
            precondition(status == errSecSuccess)

            let randomValue = UInt32(randomBytes[0]) << 24 | UInt32(randomBytes[1]) << 16 |
                              UInt32(randomBytes[2]) << 8 | UInt32(randomBytes[3])
            let remaining = indices.count - i
            let j = i + Int(randomValue % UInt32(remaining))

            indices.swapAt(i, j)
            selected.append(indices[i])
        }

        return selected.sorted()
    }
}
