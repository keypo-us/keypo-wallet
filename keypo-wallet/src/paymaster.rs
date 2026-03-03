use alloy::primitives::Address;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// ERC-7677 paymaster client.
pub struct PaymasterClient {
    pub url: String,
    pub context: serde_json::Value,
    client: reqwest::Client,
}

impl PaymasterClient {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            context: serde_json::Value::Null,
            client: reqwest::Client::new(),
        }
    }

    pub fn with_context(url: impl Into<String>, context: serde_json::Value) -> Self {
        Self {
            url: url.into(),
            context,
            client: reqwest::Client::new(),
        }
    }

    /// Calls `pm_getPaymasterStubData` via HTTP.
    pub async fn get_paymaster_stub_data(
        &self,
        user_op: &PaymasterUserOp,
        entry_point: Address,
        chain_id: u64,
    ) -> Result<PaymasterStubResponse> {
        let params = serde_json::json!([
            user_op,
            format!("{:?}", entry_point),
            format!("0x{:x}", chain_id),
            self.context,
        ]);
        let result =
            crate::rpc::json_rpc_post(&self.client, &self.url, "pm_getPaymasterStubData", params)
                .await
                .map_err(|e| Error::Paymaster(e.to_string()))?;
        serde_json::from_value(result).map_err(|e| Error::Paymaster(e.to_string()))
    }

    /// Calls `pm_getPaymasterData` via HTTP.
    pub async fn get_paymaster_data(
        &self,
        user_op: &PaymasterUserOp,
        entry_point: Address,
        chain_id: u64,
    ) -> Result<PaymasterDataResponse> {
        let params = serde_json::json!([
            user_op,
            format!("{:?}", entry_point),
            format!("0x{:x}", chain_id),
            self.context,
        ]);
        let result =
            crate::rpc::json_rpc_post(&self.client, &self.url, "pm_getPaymasterData", params)
                .await
                .map_err(|e| Error::Paymaster(e.to_string()))?;
        serde_json::from_value(result).map_err(|e| Error::Paymaster(e.to_string()))
    }

    /// Builds a `pm_getPaymasterStubData` JSON-RPC request.
    pub fn build_stub_request(
        &self,
        user_op: &PaymasterUserOp,
        entry_point: Address,
        chain_id: u64,
    ) -> PaymasterRequest {
        PaymasterRequest {
            jsonrpc: "2.0".into(),
            id: 1,
            method: "pm_getPaymasterStubData".into(),
            params: serde_json::json!([
                user_op,
                format!("{:?}", entry_point),
                format!("0x{:x}", chain_id),
                self.context,
            ]),
        }
    }

    /// Builds a `pm_getPaymasterData` JSON-RPC request.
    pub fn build_data_request(
        &self,
        user_op: &PaymasterUserOp,
        entry_point: Address,
        chain_id: u64,
    ) -> PaymasterRequest {
        PaymasterRequest {
            jsonrpc: "2.0".into(),
            id: 1,
            method: "pm_getPaymasterData".into(),
            params: serde_json::json!([
                user_op,
                format!("{:?}", entry_point),
                format!("0x{:x}", chain_id),
                self.context,
            ]),
        }
    }
}

/// Unpacked v0.7 UserOp for paymaster JSON-RPC requests.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PaymasterUserOp {
    pub sender: String,
    pub nonce: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub factory: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub factory_data: Option<String>,
    pub call_data: String,
    pub call_gas_limit: String,
    pub verification_gas_limit: String,
    pub pre_verification_gas: String,
    pub max_fee_per_gas: String,
    pub max_priority_fee_per_gas: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paymaster: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paymaster_verification_gas_limit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paymaster_post_op_gas_limit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paymaster_data: Option<String>,
    pub signature: String,
}

/// JSON-RPC request envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymasterRequest {
    pub jsonrpc: String,
    pub id: u64,
    pub method: String,
    pub params: serde_json::Value,
}

/// Response from `pm_getPaymasterStubData`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PaymasterStubResponse {
    pub paymaster: Option<String>,
    pub paymaster_data: Option<String>,
    pub paymaster_verification_gas_limit: Option<String>,
    pub paymaster_post_op_gas_limit: Option<String>,
}

/// Response from `pm_getPaymasterData`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PaymasterDataResponse {
    pub paymaster: Option<String>,
    pub paymaster_data: Option<String>,
    pub paymaster_verification_gas_limit: Option<String>,
    pub paymaster_post_op_gas_limit: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::address;

    fn sample_user_op() -> PaymasterUserOp {
        PaymasterUserOp {
            sender: "0x1111111111111111111111111111111111111111".into(),
            nonce: "0x0".into(),
            factory: None,
            factory_data: None,
            call_data: "0x1234".into(),
            call_gas_limit: "0x5208".into(),
            verification_gas_limit: "0x5208".into(),
            pre_verification_gas: "0x5208".into(),
            max_fee_per_gas: "0x3b9aca00".into(),
            max_priority_fee_per_gas: "0x3b9aca00".into(),
            paymaster: None,
            paymaster_verification_gas_limit: None,
            paymaster_post_op_gas_limit: None,
            paymaster_data: None,
            signature: "0x".into(),
        }
    }

    #[test]
    fn stub_request_serialization() {
        let client = PaymasterClient::new("https://paymaster.example.com");
        let entry_point = address!("0x0000000071727De22E5E9d8BAf0edAc6f37da032");
        let req = client.build_stub_request(&sample_user_op(), entry_point, 84532);

        assert_eq!(req.method, "pm_getPaymasterStubData");
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("pm_getPaymasterStubData"));
        assert!(json.contains("0x14a34")); // chain_id hex
    }

    #[test]
    fn data_request_serialization() {
        let client = PaymasterClient::new("https://paymaster.example.com");
        let entry_point = address!("0x0000000071727De22E5E9d8BAf0edAc6f37da032");
        let req = client.build_data_request(&sample_user_op(), entry_point, 84532);

        assert_eq!(req.method, "pm_getPaymasterData");
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("pm_getPaymasterData"));
    }

    #[test]
    fn stub_response_deserialization() {
        let json = r#"{
            "paymaster": "0x2222222222222222222222222222222222222222",
            "paymasterData": "0xabcd",
            "paymasterVerificationGasLimit": "0x5208",
            "paymasterPostOpGasLimit": "0x0"
        }"#;
        let resp: PaymasterStubResponse = serde_json::from_str(json).unwrap();
        assert_eq!(
            resp.paymaster,
            Some("0x2222222222222222222222222222222222222222".into())
        );
        assert_eq!(resp.paymaster_data, Some("0xabcd".into()));
    }

    #[test]
    fn chain_id_hex_format() {
        let client = PaymasterClient::new("https://paymaster.example.com");
        let entry_point = address!("0x0000000071727De22E5E9d8BAf0edAc6f37da032");
        let req = client.build_stub_request(&sample_user_op(), entry_point, 84532);

        // Verify chain_id is hex-encoded in params
        let params = req.params.as_array().unwrap();
        let chain_id_str = params[2].as_str().unwrap();
        assert_eq!(chain_id_str, "0x14a34");

        // Also test mainnet
        let req_mainnet = client.build_stub_request(&sample_user_op(), entry_point, 1);
        let params = req_mainnet.params.as_array().unwrap();
        assert_eq!(params[2].as_str().unwrap(), "0x1");
    }
}
