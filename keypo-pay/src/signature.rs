use alloy::primitives::Address;

use crate::types::{P256PublicKey, P256Signature};

/// Formats a P-256 signature in Tempo's P-256 signature format (type 0x01).
///
/// Total 130 bytes: 0x01 || r(32) || s(32) || pubX(32) || pubY(32) || pre_hash(1)
///
/// Note: The spec's Feature 2 description says pre_hash should be true, but the
/// Tempo Protocol Reference section (lines 280-286) conclusively resolves this as
/// `pre_hash = false` (0x00). keypo-signer signs the raw keccak256 digest without
/// additional SHA-256 hashing, so pre_hash = false is correct.
pub fn format_p256_signature(
    sig: &P256Signature,
    pub_key: &P256PublicKey,
    pre_hash: bool,
) -> Vec<u8> {
    let mut out = Vec::with_capacity(130);
    out.push(0x01); // type ID
    out.extend_from_slice(sig.r.as_slice()); // 32 bytes
    out.extend_from_slice(sig.s.as_slice()); // 32 bytes
    out.extend_from_slice(pub_key.qx.as_slice()); // 32 bytes
    out.extend_from_slice(pub_key.qy.as_slice()); // 32 bytes
    out.push(if pre_hash { 0x01 } else { 0x00 }); // 1 byte
    out
}

/// Formats a Keychain signature in Tempo's Keychain format (type 0x03).
///
/// Total 151 bytes: 0x03 || root_address(20) || inner_p256_signature(130)
///
/// Note: T2.2 validates this format offline. On-chain Keychain signature validation
/// requires the access key to be authorized first (Phase 4).
pub fn format_keychain_signature(root_address: Address, inner_sig: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(1 + 20 + inner_sig.len());
    out.push(0x03); // type ID
    out.extend_from_slice(root_address.as_slice()); // 20 bytes
    out.extend_from_slice(inner_sig); // 130 bytes (P-256 inner)
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::B256;

    fn test_sig() -> P256Signature {
        P256Signature {
            r: B256::repeat_byte(0x11),
            s: B256::repeat_byte(0x22),
        }
    }

    fn test_pub_key() -> P256PublicKey {
        P256PublicKey {
            qx: B256::repeat_byte(0xAA),
            qy: B256::repeat_byte(0xBB),
        }
    }

    #[test]
    fn p256_signature_is_130_bytes() {
        let formatted = format_p256_signature(&test_sig(), &test_pub_key(), false);
        assert_eq!(formatted.len(), 130);
    }

    #[test]
    fn p256_signature_starts_with_0x01() {
        let formatted = format_p256_signature(&test_sig(), &test_pub_key(), false);
        assert_eq!(formatted[0], 0x01);
    }

    #[test]
    fn p256_signature_ends_with_pre_hash_false() {
        let formatted = format_p256_signature(&test_sig(), &test_pub_key(), false);
        assert_eq!(formatted[129], 0x00, "pre_hash should be false (0x00)");
    }

    #[test]
    fn p256_signature_ends_with_pre_hash_true() {
        let formatted = format_p256_signature(&test_sig(), &test_pub_key(), true);
        assert_eq!(formatted[129], 0x01, "pre_hash should be true (0x01)");
    }

    #[test]
    fn p256_signature_contains_correct_fields() {
        let sig = test_sig();
        let pk = test_pub_key();
        let formatted = format_p256_signature(&sig, &pk, false);

        assert_eq!(&formatted[1..33], sig.r.as_slice());
        assert_eq!(&formatted[33..65], sig.s.as_slice());
        assert_eq!(&formatted[65..97], pk.qx.as_slice());
        assert_eq!(&formatted[97..129], pk.qy.as_slice());
    }

    #[test]
    fn keychain_signature_is_151_bytes() {
        let inner = format_p256_signature(&test_sig(), &test_pub_key(), false);
        let root_addr = Address::repeat_byte(0xDD);
        let formatted = format_keychain_signature(root_addr, &inner);
        assert_eq!(formatted.len(), 151);
    }

    #[test]
    fn keychain_signature_starts_with_0x03() {
        let inner = format_p256_signature(&test_sig(), &test_pub_key(), false);
        let root_addr = Address::repeat_byte(0xDD);
        let formatted = format_keychain_signature(root_addr, &inner);
        assert_eq!(formatted[0], 0x03);
    }

    #[test]
    fn keychain_signature_contains_root_address() {
        let inner = format_p256_signature(&test_sig(), &test_pub_key(), false);
        let root_addr = Address::repeat_byte(0xDD);
        let formatted = format_keychain_signature(root_addr, &inner);
        assert_eq!(&formatted[1..21], root_addr.as_slice());
    }

    #[test]
    fn keychain_inner_starts_with_0x01() {
        let inner = format_p256_signature(&test_sig(), &test_pub_key(), false);
        let root_addr = Address::repeat_byte(0xDD);
        let formatted = format_keychain_signature(root_addr, &inner);
        assert_eq!(formatted[21], 0x01, "inner signature should start with P-256 type 0x01");
    }
}
