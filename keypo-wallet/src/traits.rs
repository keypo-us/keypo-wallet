use alloy::primitives::{Address, Bytes, B256};

use crate::types::Call;

/// Trait for smart account implementations that define ABI encoding and deployment addresses.
pub trait AccountImplementation: Send + Sync {
    /// Human-readable name of this implementation.
    fn name(&self) -> &str;

    /// Returns the deployed implementation address for the given chain, if known.
    fn implementation_address(&self, chain_id: u64) -> Option<Address>;

    /// ABI-encodes an `initialize(bytes32 qx, bytes32 qy)` call.
    fn encode_initialize(&self, qx: B256, qy: B256) -> Bytes;

    /// ABI-encodes an `execute(bytes32 mode, bytes executionData)` call.
    /// Always uses batch mode 0x01.
    fn encode_execute(&self, calls: &[Call]) -> Bytes;

    /// Encodes a raw P-256 signature as `r || s` (64 bytes).
    fn encode_signature(&self, r: B256, s: B256) -> Bytes;

    /// Encodes a WebAuthn-wrapped P-256 signature.
    /// Returns `None` if the implementation doesn't support WebAuthn or if the
    /// client data JSON is malformed.
    fn encode_webauthn_signature(
        &self,
        _authenticator_data: &[u8],
        _client_data_json: &str,
        _r: B256,
        _s: B256,
    ) -> Option<Bytes> {
        None
    }

    /// Returns a dummy 64-byte signature for gas estimation.
    fn dummy_signature(&self) -> Bytes;

    /// Returns the ERC-4337 EntryPoint address used by this implementation.
    fn entry_point(&self) -> Address;
}
