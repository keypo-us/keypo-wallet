use std::time::Duration;

use alloy::primitives::{keccak256, Address, Bytes, B256, U256};
use alloy::providers::{Provider, ProviderBuilder};
use alloy::sol;
use alloy::sol_types::SolValue;

use crate::bundler::{BundlerClient, GasEstimate, UserOp};
use crate::error::{Error, Result};
use crate::paymaster::{PaymasterClient, PaymasterDataResponse, PaymasterStubResponse};
use crate::signer::P256Signer;
use crate::traits::AccountImplementation;
use crate::types::{AccountRecord, Call, ChainDeployment};

/// Default timeout for waiting for a UserOp receipt.
const DEFAULT_RECEIPT_TIMEOUT: Duration = Duration::from_secs(120);

// ---------------------------------------------------------------------------
// Result type
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct ExecuteResult {
    pub user_op_hash: B256,
    pub tx_hash: B256,
    pub success: bool,
}

// ---------------------------------------------------------------------------
// Pure hex-parsing helpers
// ---------------------------------------------------------------------------

pub fn parse_hex_u128(s: &str) -> Result<u128> {
    let stripped = s
        .strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .unwrap_or(s);
    u128::from_str_radix(stripped, 16)
        .map_err(|e| Error::Other(format!("invalid hex u128 '{s}': {e}")))
}

pub fn parse_hex_u256(s: &str) -> Result<U256> {
    let stripped = s
        .strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .unwrap_or(s);
    U256::from_str_radix(stripped, 16)
        .map_err(|e| Error::Other(format!("invalid hex U256 '{s}': {e}")))
}

pub fn parse_hex_bytes(s: &str) -> Result<Vec<u8>> {
    let stripped = s
        .strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .unwrap_or(s);
    hex::decode(stripped).map_err(|e| Error::Other(format!("invalid hex bytes '{s}': {e}")))
}

/// Packs two u128 values into a bytes32: high in [0..16], low in [16..32].
pub fn pack_u128_pair(high: u128, low: u128) -> B256 {
    let mut buf = [0u8; 32];
    buf[..16].copy_from_slice(&high.to_be_bytes());
    buf[16..].copy_from_slice(&low.to_be_bytes());
    B256::from(buf)
}

// ---------------------------------------------------------------------------
// InitCode / PaymasterAndData building
// ---------------------------------------------------------------------------

pub fn build_init_code(user_op: &UserOp) -> Result<Vec<u8>> {
    match (&user_op.factory, &user_op.factory_data) {
        (Some(factory), Some(factory_data)) => {
            let factory_bytes = parse_hex_bytes(factory)?;
            let data_bytes = parse_hex_bytes(factory_data)?;
            let mut init_code = Vec::with_capacity(factory_bytes.len() + data_bytes.len());
            init_code.extend_from_slice(&factory_bytes);
            init_code.extend_from_slice(&data_bytes);
            Ok(init_code)
        }
        (Some(factory), None) => parse_hex_bytes(factory),
        _ => Ok(Vec::new()),
    }
}

pub fn build_paymaster_and_data(user_op: &UserOp) -> Result<Vec<u8>> {
    let paymaster = match &user_op.paymaster {
        Some(pm) => pm,
        None => return Ok(Vec::new()),
    };

    let pm_bytes = parse_hex_bytes(paymaster)?;
    let verif_gas = parse_hex_u128(
        user_op
            .paymaster_verification_gas_limit
            .as_deref()
            .unwrap_or("0x0"),
    )?;
    let post_op_gas = parse_hex_u128(
        user_op
            .paymaster_post_op_gas_limit
            .as_deref()
            .unwrap_or("0x0"),
    )?;
    let pm_data = parse_hex_bytes(user_op.paymaster_data.as_deref().unwrap_or("0x"))?;

    let mut result = Vec::with_capacity(20 + 16 + 16 + pm_data.len());
    result.extend_from_slice(&pm_bytes);
    result.extend_from_slice(&verif_gas.to_be_bytes());
    result.extend_from_slice(&post_op_gas.to_be_bytes());
    result.extend_from_slice(&pm_data);
    Ok(result)
}

// ---------------------------------------------------------------------------
// UserOp skeleton construction
// ---------------------------------------------------------------------------

pub fn build_user_op_skeleton(
    sender: Address,
    nonce: U256,
    call_data: Bytes,
    dummy_sig: Bytes,
) -> UserOp {
    UserOp {
        sender: format!("{:?}", sender),
        nonce: format!("0x{:x}", nonce),
        factory: None,
        factory_data: None,
        call_data: format!("0x{}", hex::encode(&call_data)),
        call_gas_limit: "0x0".into(),
        verification_gas_limit: "0x0".into(),
        pre_verification_gas: "0x0".into(),
        max_fee_per_gas: "0x0".into(),
        max_priority_fee_per_gas: "0x0".into(),
        paymaster: None,
        paymaster_verification_gas_limit: None,
        paymaster_post_op_gas_limit: None,
        paymaster_data: None,
        signature: format!("0x{}", hex::encode(&dummy_sig)),
    }
}

// ---------------------------------------------------------------------------
// Apply helpers (mutate UserOp in-place)
// ---------------------------------------------------------------------------

pub fn apply_gas_estimate(user_op: &mut UserOp, estimate: &GasEstimate) {
    user_op.call_gas_limit = estimate.call_gas_limit.clone();
    user_op.verification_gas_limit = estimate.verification_gas_limit.clone();

    // preVerificationGas: add 10% buffer for L1 data cost fluctuation
    if let Ok(pvg) = parse_hex_u128(&estimate.pre_verification_gas) {
        user_op.pre_verification_gas = format!("0x{:x}", pvg * 11 / 10);
    } else {
        user_op.pre_verification_gas = estimate.pre_verification_gas.clone();
    }

    // Apply paymaster gas limits from estimate if present (overrides stubs)
    if let Some(ref pvgl) = estimate.paymaster_verification_gas_limit {
        user_op.paymaster_verification_gas_limit = Some(pvgl.clone());
    }
    if let Some(ref ppogl) = estimate.paymaster_post_op_gas_limit {
        user_op.paymaster_post_op_gas_limit = Some(ppogl.clone());
    }
}

pub fn apply_gas_prices(user_op: &mut UserOp, max_fee: u128, max_priority_fee: u128) {
    user_op.max_fee_per_gas = format!("0x{:x}", max_fee);
    user_op.max_priority_fee_per_gas = format!("0x{:x}", max_priority_fee);
}

pub fn apply_paymaster_stub(user_op: &mut UserOp, stub: &PaymasterStubResponse) {
    user_op.paymaster = stub.paymaster.clone();
    user_op.paymaster_data = stub.paymaster_data.clone();
    user_op.paymaster_verification_gas_limit = stub.paymaster_verification_gas_limit.clone();
    user_op.paymaster_post_op_gas_limit = stub.paymaster_post_op_gas_limit.clone();
}

pub fn apply_paymaster_data(user_op: &mut UserOp, data: &PaymasterDataResponse) {
    user_op.paymaster = data.paymaster.clone();
    user_op.paymaster_data = data.paymaster_data.clone();
    // Only overwrite gas limits if the paymaster response provides them;
    // otherwise preserve values from the gas estimator.
    if data.paymaster_verification_gas_limit.is_some() {
        user_op.paymaster_verification_gas_limit = data.paymaster_verification_gas_limit.clone();
    }
    if data.paymaster_post_op_gas_limit.is_some() {
        user_op.paymaster_post_op_gas_limit = data.paymaster_post_op_gas_limit.clone();
    }
}

// ---------------------------------------------------------------------------
// Hash computation (ERC-4337 v0.7)
// ---------------------------------------------------------------------------

sol! {
    struct UserOpPack {
        address sender;
        uint256 nonce;
        bytes32 initCodeHash;
        bytes32 callDataHash;
        bytes32 accountGasLimits;
        uint256 preVerificationGas;
        bytes32 gasFees;
        bytes32 paymasterAndDataHash;
    }

    struct UserOpHashEnvelope {
        bytes32 innerHash;
        address entryPoint;
        uint256 chainId;
    }
}

/// Computes the ERC-4337 v0.7 UserOp hash (signature excluded from hash).
pub fn compute_user_op_hash(user_op: &UserOp, entry_point: Address, chain_id: u64) -> Result<B256> {
    // 1. Parse typed fields from hex strings
    let sender: Address = user_op
        .sender
        .parse()
        .map_err(|e| Error::Other(format!("invalid sender: {e}")))?;
    let nonce = parse_hex_u256(&user_op.nonce)?;
    let call_data = parse_hex_bytes(&user_op.call_data)?;
    let verification_gas_limit = parse_hex_u128(&user_op.verification_gas_limit)?;
    let call_gas_limit = parse_hex_u128(&user_op.call_gas_limit)?;
    let pre_verification_gas = parse_hex_u256(&user_op.pre_verification_gas)?;
    let max_priority_fee_per_gas = parse_hex_u128(&user_op.max_priority_fee_per_gas)?;
    let max_fee_per_gas = parse_hex_u128(&user_op.max_fee_per_gas)?;

    // 2. Build initCode and paymasterAndData
    let init_code = build_init_code(user_op)?;
    let paymaster_and_data = build_paymaster_and_data(user_op)?;

    // 3. Pack gas fields
    let account_gas_limits = pack_u128_pair(verification_gas_limit, call_gas_limit);
    let gas_fees = pack_u128_pair(max_priority_fee_per_gas, max_fee_per_gas);

    // 4. Build inner hash
    let pack = UserOpPack {
        sender,
        nonce,
        initCodeHash: keccak256(&init_code),
        callDataHash: keccak256(&call_data),
        accountGasLimits: account_gas_limits,
        preVerificationGas: pre_verification_gas,
        gasFees: gas_fees,
        paymasterAndDataHash: keccak256(&paymaster_and_data),
    };
    // CRITICAL: abi_encode_params(), NOT abi_encode()
    let inner = keccak256(pack.abi_encode_params());

    // 5. Build outer hash
    let envelope = UserOpHashEnvelope {
        innerHash: inner,
        entryPoint: entry_point,
        chainId: U256::from(chain_id),
    };
    // CRITICAL: abi_encode_params(), NOT abi_encode()
    Ok(keccak256(envelope.abi_encode_params()))
}

// ---------------------------------------------------------------------------
// Async RPC helpers
// ---------------------------------------------------------------------------

/// Queries the EntryPoint for the sender's nonce (key = 0).
pub async fn query_nonce(
    provider: &impl Provider,
    sender: Address,
    entry_point: Address,
) -> Result<U256> {
    sol! {
        function getNonce(address sender, uint192 key) external view returns (uint256);
    }

    let call_data = getNonceCall {
        sender,
        key: alloy::primitives::Uint::<192, 3>::ZERO,
    };
    let encoded = alloy::sol_types::SolCall::abi_encode(&call_data);

    let result = provider
        .call(
            alloy::rpc::types::TransactionRequest::default()
                .to(entry_point)
                .input(alloy::rpc::types::TransactionInput::new(Bytes::from(
                    encoded,
                ))),
        )
        .await
        .map_err(|e| Error::Provider(format!("getNonce call failed: {e}")))?;

    // Decode the returned uint256
    let nonce = U256::abi_decode(&result)
        .map_err(|e| Error::Provider(format!("getNonce decode failed: {e}")))?;
    Ok(nonce)
}

/// Gets current gas prices from the standard RPC.
/// Returns `(max_fee_per_gas, max_priority_fee_per_gas)`.
pub async fn get_gas_prices(provider: &impl Provider) -> Result<(u128, u128)> {
    let gas_price = provider
        .get_gas_price()
        .await
        .map_err(|e| Error::Provider(format!("eth_gasPrice failed: {e}")))?;

    let max_fee = gas_price * 3 / 2;

    let max_priority_fee = provider
        .get_max_priority_fee_per_gas()
        .await
        .unwrap_or(100_000_000u128); // 0.1 gwei fallback

    Ok((max_fee, max_priority_fee))
}

fn parse_rpc_url(url: &str) -> Result<url::Url> {
    url.parse()
        .map_err(|e: url::ParseError| Error::Other(format!("invalid RPC URL: {e}")))
}

// ---------------------------------------------------------------------------
// Main orchestration
// ---------------------------------------------------------------------------

pub async fn execute(
    account: &AccountRecord,
    chain: &ChainDeployment,
    calls: &[Call],
    implementation: &dyn AccountImplementation,
    signer: &dyn P256Signer,
) -> Result<ExecuteResult> {
    execute_with_context(account, chain, calls, implementation, signer, None).await
}

pub async fn execute_with_context(
    account: &AccountRecord,
    chain: &ChainDeployment,
    calls: &[Call],
    implementation: &dyn AccountImplementation,
    signer: &dyn P256Signer,
    paymaster_context: Option<serde_json::Value>,
) -> Result<ExecuteResult> {
    // 0. Extract chain metadata
    let entry_point = chain.entry_point;
    let chain_id = chain.chain_id;
    let sender = account.address;

    // 1. Resolve bundler URL
    let bundler_url = chain.bundler_url.as_ref().ok_or_else(|| {
        Error::Bundler("no bundler URL configured; use --bundler to specify".into())
    })?;

    // 2. Build standard RPC provider (for nonce + gas price queries)
    let rpc_url = parse_rpc_url(&chain.rpc_url)?;
    let provider = ProviderBuilder::new().connect_http(rpc_url);

    // 3. Build bundler client
    let bundler = BundlerClient::new(bundler_url.clone(), entry_point);

    // 4. Encode calldata
    let call_data = implementation.encode_execute(calls);

    // 5. Query nonce
    let nonce = query_nonce(&provider, sender, entry_point).await?;
    tracing::info!("Sender nonce: {nonce}");

    // 6. Build UserOp skeleton with dummy signature
    let dummy_sig = implementation.dummy_signature();
    let mut user_op = build_user_op_skeleton(sender, nonce, call_data, dummy_sig);

    // 7. If paymaster URL set, get stub data
    let has_paymaster = chain.paymaster_url.is_some();
    let pm_context = paymaster_context.unwrap_or(serde_json::Value::Null);
    if let Some(ref pm_url) = chain.paymaster_url {
        let pm_client = PaymasterClient::with_context(pm_url.clone(), pm_context.clone());
        let stub = pm_client
            .get_paymaster_stub_data(&user_op, entry_point, chain_id)
            .await?;
        apply_paymaster_stub(&mut user_op, &stub);
        tracing::info!("Paymaster stub applied");
    }

    // 8. Get gas prices (standard RPC, NOT bundler)
    let (max_fee, max_priority_fee) = get_gas_prices(&provider).await?;
    apply_gas_prices(&mut user_op, max_fee, max_priority_fee);
    tracing::info!("Gas prices: maxFee={max_fee}, maxPriorityFee={max_priority_fee}");

    // 9. Estimate gas
    let estimate = bundler.estimate_user_operation_gas(&user_op).await?;
    apply_gas_estimate(&mut user_op, &estimate);
    tracing::info!(
        "Gas estimate: pvg={}, vgl={}, cgl={}",
        user_op.pre_verification_gas,
        user_op.verification_gas_limit,
        user_op.call_gas_limit
    );

    // 10. If paymaster, get real data (replacing stub)
    if has_paymaster {
        if let Some(ref pm_url) = chain.paymaster_url {
            let pm_client = PaymasterClient::with_context(pm_url.clone(), pm_context);
            let pm_data = pm_client
                .get_paymaster_data(&user_op, entry_point, chain_id)
                .await?;
            apply_paymaster_data(&mut user_op, &pm_data);
            tracing::info!("Paymaster data applied");
        }
    }

    // 11. Compute UserOp hash
    let user_op_hash = compute_user_op_hash(&user_op, entry_point, chain_id)?;
    tracing::info!("UserOp hash: {user_op_hash}");

    // 12. Sign hash with P-256
    let hash_bytes: [u8; 32] = user_op_hash.into();
    let sig = signer.sign(&hash_bytes, &account.key_label)?;

    // 13. Encode signature
    let encoded_sig = implementation.encode_signature(sig.r, sig.s);

    // 14. Set signature on UserOp
    user_op.signature = format!("0x{}", hex::encode(&encoded_sig));

    // 15. Submit to bundler
    let bundler_hash = bundler.send_user_operation(&user_op).await?;

    // 15b. Verify hash
    if bundler_hash != user_op_hash {
        return Err(Error::Other(format!(
            "hash mismatch: computed {user_op_hash}, bundler returned {bundler_hash}"
        )));
    }
    tracing::info!("UserOp submitted: {bundler_hash}");

    // 16. Wait for receipt
    let receipt = bundler
        .wait_for_receipt(bundler_hash, DEFAULT_RECEIPT_TIMEOUT)
        .await?;
    tracing::info!(
        "Receipt: success={}, tx={}",
        receipt.success,
        receipt.receipt.transaction_hash
    );

    // 17. Return result
    Ok(ExecuteResult {
        user_op_hash,
        tx_hash: receipt
            .receipt
            .transaction_hash
            .parse()
            .map_err(|e| Error::Other(format!("invalid tx hash in receipt: {e}")))?,
        success: receipt.success,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::address;

    #[test]
    fn parse_hex_u128_with_prefix() {
        assert_eq!(parse_hex_u128("0x5208").unwrap(), 0x5208u128);
    }

    #[test]
    fn parse_hex_u128_without_prefix() {
        assert_eq!(parse_hex_u128("5208").unwrap(), 0x5208u128);
    }

    #[test]
    fn parse_hex_u128_zero() {
        assert_eq!(parse_hex_u128("0x0").unwrap(), 0u128);
    }

    #[test]
    fn parse_hex_u256_large() {
        let val =
            parse_hex_u256("0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff")
                .unwrap();
        assert_eq!(val, U256::MAX);
    }

    #[test]
    fn parse_hex_bytes_empty() {
        assert_eq!(parse_hex_bytes("0x").unwrap(), Vec::<u8>::new());
    }

    #[test]
    fn parse_hex_bytes_data() {
        assert_eq!(parse_hex_bytes("0x1234").unwrap(), vec![0x12, 0x34]);
    }

    #[test]
    fn pack_u128_pair_known_values() {
        let packed = pack_u128_pair(1, 2);
        let bytes = packed.as_slice();
        // high = 1 in [0..16]
        assert_eq!(&bytes[..16], &1u128.to_be_bytes());
        // low = 2 in [16..32]
        assert_eq!(&bytes[16..], &2u128.to_be_bytes());
    }

    #[test]
    fn pack_u128_pair_max_values() {
        let packed = pack_u128_pair(u128::MAX, u128::MAX);
        assert_eq!(packed, B256::repeat_byte(0xff));
    }

    #[test]
    fn skeleton_fields() {
        let sender = address!("0x1111111111111111111111111111111111111111");
        let nonce = U256::from(42);
        let call_data = Bytes::from(vec![0xab, 0xcd]);
        let dummy_sig = Bytes::from(vec![0x01; 64]);

        let op = build_user_op_skeleton(sender, nonce, call_data, dummy_sig);
        // Checksummed address
        assert_eq!(op.sender, format!("{:?}", sender));
        assert_eq!(op.nonce, "0x2a");
        assert_eq!(op.call_data, "0xabcd");
        assert_eq!(op.call_gas_limit, "0x0");
        assert_eq!(op.verification_gas_limit, "0x0");
        assert_eq!(op.pre_verification_gas, "0x0");
        assert_eq!(op.max_fee_per_gas, "0x0");
        assert_eq!(op.max_priority_fee_per_gas, "0x0");
        assert!(op.factory.is_none());
        assert!(op.paymaster.is_none());
        assert_eq!(op.signature.len(), 2 + 128); // "0x" + 64 bytes hex
    }

    #[test]
    fn apply_gas_estimate_with_buffer() {
        let mut op = build_user_op_skeleton(
            Address::ZERO,
            U256::ZERO,
            Bytes::new(),
            Bytes::from(vec![0x01; 64]),
        );
        let estimate = GasEstimate {
            pre_verification_gas: "0x2710".into(), // 10000
            verification_gas_limit: "0x5208".into(),
            call_gas_limit: "0x7530".into(),
            paymaster_verification_gas_limit: None,
            paymaster_post_op_gas_limit: None,
        };
        apply_gas_estimate(&mut op, &estimate);

        // pvg should have 10% buffer: 10000 * 11 / 10 = 11000 = 0x2af8
        assert_eq!(op.pre_verification_gas, "0x2af8");
        assert_eq!(op.verification_gas_limit, "0x5208");
        assert_eq!(op.call_gas_limit, "0x7530");
    }

    #[test]
    fn apply_gas_prices_formatting() {
        let mut op = build_user_op_skeleton(
            Address::ZERO,
            U256::ZERO,
            Bytes::new(),
            Bytes::from(vec![0x01; 64]),
        );
        apply_gas_prices(&mut op, 1_000_000_000, 100_000_000);
        assert_eq!(op.max_fee_per_gas, "0x3b9aca00");
        assert_eq!(op.max_priority_fee_per_gas, "0x5f5e100");
    }

    #[test]
    fn paymaster_stub_application() {
        let mut op = build_user_op_skeleton(
            Address::ZERO,
            U256::ZERO,
            Bytes::new(),
            Bytes::from(vec![0x01; 64]),
        );
        let stub = PaymasterStubResponse {
            paymaster: Some("0x2222222222222222222222222222222222222222".into()),
            paymaster_data: Some("0xabcd".into()),
            paymaster_verification_gas_limit: Some("0x5208".into()),
            paymaster_post_op_gas_limit: Some("0x0".into()),
        };
        apply_paymaster_stub(&mut op, &stub);
        assert_eq!(
            op.paymaster,
            Some("0x2222222222222222222222222222222222222222".into())
        );
        assert_eq!(op.paymaster_data, Some("0xabcd".into()));
    }

    #[test]
    fn build_init_code_empty() {
        let op = build_user_op_skeleton(
            Address::ZERO,
            U256::ZERO,
            Bytes::new(),
            Bytes::from(vec![0x01; 64]),
        );
        let init_code = build_init_code(&op).unwrap();
        assert!(init_code.is_empty());
    }

    #[test]
    fn build_init_code_populated() {
        let mut op = build_user_op_skeleton(
            Address::ZERO,
            U256::ZERO,
            Bytes::new(),
            Bytes::from(vec![0x01; 64]),
        );
        op.factory = Some("0x3333333333333333333333333333333333333333".into());
        op.factory_data = Some("0xdeadbeef".into());

        let init_code = build_init_code(&op).unwrap();
        // 20 bytes address + 4 bytes data
        assert_eq!(init_code.len(), 24);
        assert_eq!(
            &init_code[..20],
            &hex::decode("3333333333333333333333333333333333333333").unwrap()[..]
        );
        assert_eq!(&init_code[20..], &[0xde, 0xad, 0xbe, 0xef]);
    }

    #[test]
    fn build_paymaster_and_data_empty() {
        let op = build_user_op_skeleton(
            Address::ZERO,
            U256::ZERO,
            Bytes::new(),
            Bytes::from(vec![0x01; 64]),
        );
        let pm_data = build_paymaster_and_data(&op).unwrap();
        assert!(pm_data.is_empty());
    }

    #[test]
    fn build_paymaster_and_data_populated() {
        let mut op = build_user_op_skeleton(
            Address::ZERO,
            U256::ZERO,
            Bytes::new(),
            Bytes::from(vec![0x01; 64]),
        );
        op.paymaster = Some("0x4444444444444444444444444444444444444444".into());
        op.paymaster_verification_gas_limit = Some("0x5208".into()); // 21000
        op.paymaster_post_op_gas_limit = Some("0x2710".into()); // 10000
        op.paymaster_data = Some("0xabcd".into());

        let pm_data = build_paymaster_and_data(&op).unwrap();
        // 20 bytes address + 16 bytes verifGas + 16 bytes postOpGas + 2 bytes data
        assert_eq!(pm_data.len(), 54);

        // Verify address
        assert_eq!(
            &pm_data[..20],
            &hex::decode("4444444444444444444444444444444444444444").unwrap()[..]
        );

        // Verify gas limits are u128::to_be_bytes (16 bytes each), NOT raw hex-decode
        let verif_gas_bytes = &pm_data[20..36];
        assert_eq!(verif_gas_bytes, &21000u128.to_be_bytes());

        let post_op_gas_bytes = &pm_data[36..52];
        assert_eq!(post_op_gas_bytes, &10000u128.to_be_bytes());

        // Verify paymaster data
        assert_eq!(&pm_data[52..], &[0xab, 0xcd]);
    }

    // ── Hash computation tests: vectors from on-chain EntryPoint v0.7 ──
    // Generated by keypo-account/script/GenHashVector.s.sol on Base Sepolia fork.
    // Chain ID: 84532

    const BASE_SEPOLIA_CHAIN_ID: u64 = 84532;
    const ENTRY_POINT: &str = "0x0000000071727De22E5E9d8BAf0edAc6f37da032";

    fn entry_point_addr() -> Address {
        ENTRY_POINT.parse().unwrap()
    }

    #[test]
    fn hash_vector_a_minimal() {
        // Vector A: sender=0x1111..., nonce=0, callData=0xabcdef,
        // accountGasLimits=pack(100000, 50000), pvg=21000,
        // gasFees=pack(1000000000, 2000000000), no factory, no paymaster
        let op = UserOp {
            sender: "0x1111111111111111111111111111111111111111".into(),
            nonce: "0x0".into(),
            factory: None,
            factory_data: None,
            call_data: "0xabcdef".into(),
            call_gas_limit: format!("0x{:x}", 50000u128),
            verification_gas_limit: format!("0x{:x}", 100000u128),
            pre_verification_gas: format!("0x{:x}", 21000u64),
            max_fee_per_gas: format!("0x{:x}", 2000000000u128),
            max_priority_fee_per_gas: format!("0x{:x}", 1000000000u128),
            paymaster: None,
            paymaster_verification_gas_limit: None,
            paymaster_post_op_gas_limit: None,
            paymaster_data: None,
            signature: format!("0x{}", "01".repeat(64)),
        };

        let hash = compute_user_op_hash(&op, entry_point_addr(), BASE_SEPOLIA_CHAIN_ID).unwrap();
        let expected: B256 = "0x6d3d11898608926d924477db9ff6c3406541a2559b638d0aaddc21eb73a43e4d"
            .parse()
            .unwrap();
        assert_eq!(hash, expected, "Vector A hash mismatch");
    }

    #[test]
    fn hash_vector_b_with_paymaster() {
        // Vector B: sender=0x3333..., nonce=5, callData=0x12345678,
        // accountGasLimits=pack(200000, 100000), pvg=50000,
        // gasFees=pack(500000000, 3000000000),
        // paymaster=0x2222..., pmVerifGas=50000, pmPostOpGas=10000, pmData=0xaabbccdd
        let op = UserOp {
            sender: "0x3333333333333333333333333333333333333333".into(),
            nonce: "0x5".into(),
            factory: None,
            factory_data: None,
            call_data: "0x12345678".into(),
            call_gas_limit: format!("0x{:x}", 100000u128),
            verification_gas_limit: format!("0x{:x}", 200000u128),
            pre_verification_gas: format!("0x{:x}", 50000u64),
            max_fee_per_gas: format!("0x{:x}", 3000000000u128),
            max_priority_fee_per_gas: format!("0x{:x}", 500000000u128),
            paymaster: Some("0x2222222222222222222222222222222222222222".into()),
            paymaster_verification_gas_limit: Some(format!("0x{:x}", 50000u128)),
            paymaster_post_op_gas_limit: Some(format!("0x{:x}", 10000u128)),
            paymaster_data: Some("0xaabbccdd".into()),
            signature: format!("0x{}", "01".repeat(64)),
        };

        let hash = compute_user_op_hash(&op, entry_point_addr(), BASE_SEPOLIA_CHAIN_ID).unwrap();
        let expected: B256 = "0x26ed5e3d60edb28e26eabde5165c632869013ed87424add8aa80fec7fc48ac87"
            .parse()
            .unwrap();
        assert_eq!(hash, expected, "Vector B hash mismatch");
    }

    #[test]
    fn hash_vector_c_with_factory() {
        // Vector C: sender=0x5555..., nonce=1, callData=0xcafe,
        // accountGasLimits=pack(300000, 150000), pvg=30000,
        // gasFees=pack(2000000000, 4000000000),
        // factory=0x4444..., factoryData=0xdeadbeef, no paymaster
        let op = UserOp {
            sender: "0x5555555555555555555555555555555555555555".into(),
            nonce: "0x1".into(),
            factory: Some("0x4444444444444444444444444444444444444444".into()),
            factory_data: Some("0xdeadbeef".into()),
            call_data: "0xcafe".into(),
            call_gas_limit: format!("0x{:x}", 150000u128),
            verification_gas_limit: format!("0x{:x}", 300000u128),
            pre_verification_gas: format!("0x{:x}", 30000u64),
            max_fee_per_gas: format!("0x{:x}", 4000000000u128),
            max_priority_fee_per_gas: format!("0x{:x}", 2000000000u128),
            paymaster: None,
            paymaster_verification_gas_limit: None,
            paymaster_post_op_gas_limit: None,
            paymaster_data: None,
            signature: format!("0x{}", "01".repeat(64)),
        };

        let hash = compute_user_op_hash(&op, entry_point_addr(), BASE_SEPOLIA_CHAIN_ID).unwrap();
        let expected: B256 = "0xf425f5f15c96f731bc8d53057aa31709c92daba6cba4a3d2199615730a21efc9"
            .parse()
            .unwrap();
        assert_eq!(hash, expected, "Vector C hash mismatch");
    }
}
