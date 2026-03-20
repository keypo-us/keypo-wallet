import Foundation

/// Passphrase strength level.
public enum PassphraseStrengthLevel: String, Codable {
    case weak
    case fair
    case good
    case strong
}

/// Result of passphrase strength evaluation.
public struct PassphraseStrengthResult {
    public let level: PassphraseStrengthLevel
    public let estimatedBits: Double
    public let feedback: String

    public init(level: PassphraseStrengthLevel, estimatedBits: Double, feedback: String) {
        self.level = level
        self.estimatedBits = estimatedBits
        self.feedback = feedback
    }
}

/// Evaluate passphrase strength using entropy estimation.
public enum PassphraseStrengthEvaluator {

    /// Evaluate the strength of a passphrase.
    /// - Parameter passphrase: The passphrase string to evaluate.
    /// - Returns: A `PassphraseStrengthResult` with level, estimated bits, and feedback.
    public static func evaluate(_ passphrase: String) -> PassphraseStrengthResult {
        let trimmed = passphrase.trimmingCharacters(in: .whitespacesAndNewlines)

        guard !trimmed.isEmpty else {
            return PassphraseStrengthResult(level: .weak, estimatedBits: 0, feedback: "Passphrase is empty")
        }

        // Check if it looks like a BIP-39 generated passphrase (space-separated words)
        let words = trimmed.lowercased().split(separator: " ").map(String.init)
        let bip39Words = Set(Wordlist.english)

        let allBip39 = words.count >= 2 && words.allSatisfy { bip39Words.contains($0) }

        let bits: Double
        if allBip39 {
            // BIP-39 word-based: ~11 bits per word
            bits = Double(words.count) * 11.0
        } else {
            // Character-based entropy estimation
            bits = characterEntropy(trimmed)
        }

        let level: PassphraseStrengthLevel
        let feedback: String

        switch bits {
        case ..<30:
            level = .weak
            feedback = "Too weak — use a longer passphrase or add more words"
        case 30..<44:
            level = .fair
            feedback = "Fair — consider adding more words or characters"
        case 44..<60:
            level = .good
            feedback = "Good passphrase strength"
        default:
            level = .strong
            feedback = "Strong passphrase"
        }

        return PassphraseStrengthResult(level: level, estimatedBits: bits, feedback: feedback)
    }

    /// Estimate entropy from character composition.
    private static func characterEntropy(_ passphrase: String) -> Double {
        var charsetSize = 0

        let hasLower = passphrase.unicodeScalars.contains { $0.value >= 0x61 && $0.value <= 0x7A }
        let hasUpper = passphrase.unicodeScalars.contains { $0.value >= 0x41 && $0.value <= 0x5A }
        let hasDigit = passphrase.unicodeScalars.contains { $0.value >= 0x30 && $0.value <= 0x39 }
        let hasSymbol = passphrase.unicodeScalars.contains {
            ($0.value >= 0x21 && $0.value <= 0x2F) ||
            ($0.value >= 0x3A && $0.value <= 0x40) ||
            ($0.value >= 0x5B && $0.value <= 0x60) ||
            ($0.value >= 0x7B && $0.value <= 0x7E)
        }
        let hasSpace = passphrase.contains(" ")

        if hasLower { charsetSize += 26 }
        if hasUpper { charsetSize += 26 }
        if hasDigit { charsetSize += 10 }
        if hasSymbol { charsetSize += 32 }
        if hasSpace { charsetSize += 1 }

        // Minimum charset of 26 (assume at least lowercase)
        charsetSize = max(charsetSize, 26)

        let length = Double(passphrase.count)
        return length * log2(Double(charsetSize))
    }

    /// Format strength as a visual bar for CLI display.
    /// - Parameter result: The strength evaluation result.
    /// - Returns: A formatted string like "Strength: ████░░░░ good (~47 bits)"
    public static func formatBar(_ result: PassphraseStrengthResult) -> String {
        let filled: Int
        switch result.level {
        case .weak: filled = 2
        case .fair: filled = 4
        case .good: filled = 6
        case .strong: filled = 8
        }
        let bar = String(repeating: "█", count: filled) + String(repeating: "░", count: 8 - filled)
        return "Strength: \(bar) \(result.level.rawValue) (~\(Int(result.estimatedBits)) bits)"
    }
}
