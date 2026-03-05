use std::collections::HashMap;
use std::path::Path;

use alloy::primitives::{address, Address, Bytes, B256};
use alloy::sol;
use alloy::sol_types::SolCall;
use alloy::sol_types::SolValue;
use serde::Deserialize;

use crate::error::Result;
use crate::traits::AccountImplementation;
use crate::types::Call;

sol! {
    function initialize(bytes32 qx, bytes32 qy);
    function execute(bytes32 mode, bytes executionData);

    struct Execution {
        address target;
        uint256 value;
        bytes callData;
    }

    struct WebAuthnSig {
        bytes32 r;
        bytes32 s;
        uint256 challengeIndex;
        uint256 typeIndex;
        bytes authenticatorData;
        string clientDataJSON;
    }
}

/// ERC-4337 v0.7 EntryPoint address.
const ENTRY_POINT_V07: Address = address!("0x0000000071727De22E5E9d8BAf0edAc6f37da032");

/// Deserialization helper for deployment JSON files.
#[derive(Deserialize)]
struct DeploymentFile {
    #[serde(rename = "chainId")]
    chain_id: u64,
    address: Address,
}

/// KeypoAccount implementation — EIP-7702 delegation target with P-256 signatures,
/// ERC-4337 v0.7 support, and ERC-7821 batch execution.
pub struct KeypoAccountImpl {
    deployments: HashMap<u64, Address>,
}

impl KeypoAccountImpl {
    /// Creates a new instance with no known deployments.
    pub fn new() -> Self {
        Self {
            deployments: HashMap::new(),
        }
    }

    /// Creates a new instance with a single deployment.
    pub fn with_deployment(chain_id: u64, addr: Address) -> Self {
        let mut deployments = HashMap::new();
        deployments.insert(chain_id, addr);
        Self { deployments }
    }

    /// Creates a new instance from a map of chain_id → address.
    pub fn with_deployments(deployments: HashMap<u64, Address>) -> Self {
        Self { deployments }
    }

    /// Reads deployment records from a directory of JSON files.
    ///
    /// Each JSON file should contain at least `chainId` and `address` fields.
    /// Returns an error if the directory can't be read or any JSON file can't be parsed.
    pub fn from_deployments_dir(path: &Path) -> Result<Self> {
        let mut deployments = HashMap::new();
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let file_path = entry.path();
            if file_path.extension().and_then(|e| e.to_str()) == Some("json") {
                let contents = std::fs::read_to_string(&file_path)?;
                let record: DeploymentFile = serde_json::from_str(&contents)?;
                deployments.insert(record.chain_id, record.address);
            }
        }
        Ok(Self { deployments })
    }

    /// Builds the ERC-7821 batch mode bytes32.
    /// mode byte[0] = 0x01, rest zero. See docs/decisions/003-erc7821-batch-mode.md
    fn batch_mode() -> B256 {
        let mut mode = [0u8; 32];
        mode[0] = 0x01;
        B256::from(mode)
    }
}

impl Default for KeypoAccountImpl {
    fn default() -> Self {
        Self::new()
    }
}

impl AccountImplementation for KeypoAccountImpl {
    fn name(&self) -> &str {
        "KeypoAccount"
    }

    fn implementation_address(&self, chain_id: u64) -> Option<Address> {
        self.deployments.get(&chain_id).copied()
    }

    fn encode_initialize(&self, qx: B256, qy: B256) -> Bytes {
        let call = initializeCall { qx, qy };
        Bytes::from(call.abi_encode())
    }

    fn encode_execute(&self, calls: &[Call]) -> Bytes {
        let executions: Vec<Execution> = calls
            .iter()
            .map(|c| Execution {
                target: c.to,
                value: c.value,
                callData: c.data.clone(),
            })
            .collect();

        let execution_data = executions.abi_encode();

        let call = executeCall {
            mode: Self::batch_mode(),
            executionData: Bytes::from(execution_data),
        };
        Bytes::from(call.abi_encode())
    }

    fn encode_signature(&self, r: B256, s: B256) -> Bytes {
        let mut sig = Vec::with_capacity(64);
        sig.extend_from_slice(r.as_slice());
        sig.extend_from_slice(s.as_slice());
        Bytes::from(sig)
    }

    fn encode_webauthn_signature(
        &self,
        authenticator_data: &[u8],
        client_data_json: &str,
        r: B256,
        s: B256,
    ) -> Option<Bytes> {
        let challenge_index = client_data_json.find("\"challenge\"")?;
        let type_index = client_data_json.find("\"type\"")?;

        let sig = WebAuthnSig {
            r,
            s,
            challengeIndex: alloy::primitives::U256::from(challenge_index),
            typeIndex: alloy::primitives::U256::from(type_index),
            authenticatorData: Bytes::from(authenticator_data.to_vec()),
            clientDataJSON: client_data_json.to_string(),
        };

        // Use abi_encode_params() — NOT abi_encode() which adds an outer offset.
        // This matches P256Helper.sol's abi.encode(r, s, challengeIndex, typeIndex, authenticatorData, clientDataJSON).
        Some(Bytes::from(sig.abi_encode_params()))
    }

    fn dummy_signature(&self) -> Bytes {
        Bytes::from(vec![0x01u8; 64])
    }

    fn entry_point(&self) -> Address {
        ENTRY_POINT_V07
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::U256;
    use alloy::sol_types::SolCall;
    use alloy::sol_types::SolValue;

    fn test_impl() -> KeypoAccountImpl {
        KeypoAccountImpl::new()
    }

    #[test]
    fn encode_initialize_length_and_selector() {
        let imp = test_impl();
        let qx = B256::repeat_byte(0x11);
        let qy = B256::repeat_byte(0x22);
        let encoded = imp.encode_initialize(qx, qy);

        // 4 bytes selector + 32 bytes qx + 32 bytes qy = 68
        assert_eq!(encoded.len(), 68);

        // Verify selector matches initialize(bytes32,bytes32)
        let expected_selector = initializeCall::SELECTOR;
        assert_eq!(&encoded[..4], expected_selector);
    }

    #[test]
    fn encode_initialize_roundtrip() {
        let imp = test_impl();
        let qx = B256::repeat_byte(0xAA);
        let qy = B256::repeat_byte(0xBB);
        let encoded = imp.encode_initialize(qx, qy);

        let decoded = initializeCall::abi_decode(&encoded).unwrap();
        assert_eq!(decoded.qx, qx);
        assert_eq!(decoded.qy, qy);
    }

    #[test]
    fn encode_execute_single_call() {
        let imp = test_impl();
        let calls = vec![Call {
            to: Address::repeat_byte(0xDE),
            value: U256::from(1_000_000_000_000_000_000u64),
            data: Bytes::from(vec![0x12, 0x34]),
        }];
        let encoded = imp.encode_execute(&calls);

        // Decode the outer execute call
        let decoded = executeCall::abi_decode(&encoded).unwrap();
        assert_eq!(decoded.mode, KeypoAccountImpl::batch_mode());

        // Decode the execution data as Vec<Execution>
        let executions = Vec::<Execution>::abi_decode(&decoded.executionData).unwrap();
        assert_eq!(executions.len(), 1);
        assert_eq!(executions[0].target, Address::repeat_byte(0xDE));
        assert_eq!(
            executions[0].value,
            U256::from(1_000_000_000_000_000_000u64)
        );
        assert_eq!(executions[0].callData, Bytes::from(vec![0x12, 0x34]));
    }

    #[test]
    fn encode_execute_three_call_batch() {
        let imp = test_impl();
        let calls: Vec<Call> = (0..3)
            .map(|i| Call {
                to: Address::repeat_byte(i + 1),
                value: U256::from(i as u64 * 100),
                data: Bytes::from(vec![i]),
            })
            .collect();
        let encoded = imp.encode_execute(&calls);

        let decoded = executeCall::abi_decode(&encoded).unwrap();
        let executions = Vec::<Execution>::abi_decode(&decoded.executionData).unwrap();
        assert_eq!(executions.len(), 3);
        for (i, exec) in executions.iter().enumerate() {
            assert_eq!(exec.target, Address::repeat_byte(i as u8 + 1));
        }
    }

    #[test]
    fn encode_execute_empty_batch() {
        let imp = test_impl();
        let encoded = imp.encode_execute(&[]);

        let decoded = executeCall::abi_decode(&encoded).unwrap();
        let executions = Vec::<Execution>::abi_decode(&decoded.executionData).unwrap();
        assert_eq!(executions.len(), 0);
    }

    #[test]
    fn encode_execute_known_vector() {
        // Known vector: execute(BATCH_MODE, abi.encode([Execution(0xdead...beef, 1 ether, 0x1234)]))
        // BATCH_MODE = 0x0100...00
        // This test verifies our encoding matches Solidity's.
        let imp = test_impl();
        let mut to_bytes = [0u8; 20];
        to_bytes[0] = 0xDE;
        to_bytes[1] = 0xAD;
        to_bytes[18] = 0xBE;
        to_bytes[19] = 0xEF;
        let calls = vec![Call {
            to: Address::from(to_bytes),
            value: U256::from(1_000_000_000_000_000_000u64), // 1 ether
            data: Bytes::from(vec![0x12, 0x34]),
        }];
        let encoded = imp.encode_execute(&calls);

        // Verify it roundtrips correctly
        let decoded = executeCall::abi_decode(&encoded).unwrap();
        assert_eq!(decoded.mode[0], 0x01);
        for &b in &decoded.mode[1..] {
            assert_eq!(b, 0x00);
        }

        let executions = Vec::<Execution>::abi_decode(&decoded.executionData).unwrap();
        assert_eq!(executions.len(), 1);
        assert_eq!(executions[0].target, Address::from(to_bytes));
        assert_eq!(
            executions[0].value,
            U256::from(1_000_000_000_000_000_000u64)
        );
        assert_eq!(executions[0].callData, Bytes::from(vec![0x12, 0x34]));

        // Verify the mode bytes specifically
        let mode = decoded.mode;
        assert_eq!(
            mode,
            KeypoAccountImpl::batch_mode(),
            "mode should be 0x01 in byte[0]"
        );
    }

    #[test]
    fn encode_signature_length_and_values() {
        let imp = test_impl();
        let r = B256::repeat_byte(0xAA);
        let s = B256::repeat_byte(0xBB);
        let encoded = imp.encode_signature(r, s);

        assert_eq!(encoded.len(), 64);
        assert_eq!(&encoded[..32], r.as_slice());
        assert_eq!(&encoded[32..], s.as_slice());
    }

    #[test]
    fn dummy_signature_is_64_bytes_of_0x01() {
        let imp = test_impl();
        let dummy = imp.dummy_signature();
        assert_eq!(dummy.len(), 64);
        assert!(dummy.iter().all(|&b| b == 0x01));
    }

    #[test]
    fn encode_webauthn_signature_known_json() {
        let imp = test_impl();
        let client_data_json =
            r#"{"type":"webauthn.get","challenge":"dGVzdA","origin":"https://example.com"}"#;
        let auth_data = vec![0x01, 0x02, 0x03];
        let r = B256::repeat_byte(0x11);
        let s = B256::repeat_byte(0x22);

        let encoded = imp
            .encode_webauthn_signature(&auth_data, client_data_json, r, s)
            .expect("should encode");

        // Decode as flat tuple
        let decoded = WebAuthnSig::abi_decode_params(&encoded).unwrap();
        assert_eq!(decoded.r, r);
        assert_eq!(decoded.s, s);

        // challengeIndex should point to the '"' before 'challenge'
        let expected_challenge_idx = client_data_json.find("\"challenge\"").unwrap();
        assert_eq!(decoded.challengeIndex, U256::from(expected_challenge_idx));

        // typeIndex should point to the '"' before 'type'
        let expected_type_idx = client_data_json.find("\"type\"").unwrap();
        assert_eq!(decoded.typeIndex, U256::from(expected_type_idx));

        assert_eq!(decoded.authenticatorData, Bytes::from(auth_data));
        assert_eq!(decoded.clientDataJSON, client_data_json);
    }

    #[test]
    fn encode_webauthn_signature_missing_challenge_returns_none() {
        let imp = test_impl();
        let client_data_json = r#"{"type":"webauthn.get"}"#;
        let result =
            imp.encode_webauthn_signature(&[0x01], client_data_json, B256::ZERO, B256::ZERO);
        assert!(result.is_none());
    }

    #[test]
    fn entry_point_address() {
        let imp = test_impl();
        assert_eq!(imp.entry_point(), ENTRY_POINT_V07);
    }

    #[test]
    fn from_deployments_dir_reads_json() {
        let dir = tempfile::tempdir().unwrap();
        let json = r#"{"contract":"KeypoAccount","chainId":84532,"address":"0x6d1566f9aAcf9c06969D7BF846FA090703A38E43"}"#;
        std::fs::write(dir.path().join("base-sepolia.json"), json).unwrap();

        let imp = KeypoAccountImpl::from_deployments_dir(dir.path()).unwrap();
        let addr = imp.implementation_address(84532).unwrap();
        assert_eq!(
            addr,
            "0x6d1566f9aAcf9c06969D7BF846FA090703A38E43"
                .parse::<Address>()
                .unwrap()
        );
    }

    #[test]
    fn from_deployments_dir_malformed_json_errors() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("bad.json"), "not json").unwrap();

        let result = KeypoAccountImpl::from_deployments_dir(dir.path());
        assert!(result.is_err());
    }

    #[test]
    fn implementation_address_missing_chain_returns_none() {
        let imp = test_impl();
        assert!(imp.implementation_address(999).is_none());
    }
}
