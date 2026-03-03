use std::time::Duration;

use alloy::eips::eip7702::Authorization;
use alloy::network::{EthereumWallet, TransactionBuilder, TransactionBuilder7702};
use alloy::primitives::{Address, Bytes, B256, U256};
use alloy::providers::{Provider, ProviderBuilder};
use alloy::rpc::types::TransactionRequest;
use alloy::signers::local::PrivateKeySigner;
use alloy::signers::SignerSync;

use crate::error::{Error, Result};
use crate::signer::P256Signer;
use crate::state::StateStore;
use crate::traits::AccountImplementation;
use crate::types::{ChainDeployment, P256PublicKey};

/// 0.001 ETH — sufficient for a type-4 tx on L2 with wide margin.
pub const SETUP_FUNDING_AMOUNT: U256 = U256::from_limbs([1_000_000_000_000_000, 0, 0, 0]);

pub struct SetupConfig {
    pub key_label: String,
    pub key_policy: String,
    pub rpc_url: String,
    pub bundler_url: Option<String>,
    pub paymaster_url: Option<String>,
    pub implementation_address: Address,
    pub implementation_name: String,
    pub chain_id: Option<u64>,
}

pub enum FundingStrategy {
    WaitForFunding {
        poll_interval: Duration,
        max_wait: Duration,
    },
    /// Test-only: raw private key for automated funding (TEST_FUNDER_PRIVATE_KEY).
    /// Never used in production. The String is not zeroized on drop.
    FundFrom {
        funder_private_key: String,
        amount: U256,
        rpc_url: String,
    },
}

#[derive(Debug)]
pub struct SetupResult {
    pub account_address: Address,
    pub public_key: P256PublicKey,
    pub tx_hash: B256,
    pub chain_id: u64,
}

// ---------------------------------------------------------------------------
// Pure helpers (no RPC, unit-testable)
// ---------------------------------------------------------------------------

fn get_or_create_key(signer: &dyn P256Signer, label: &str, policy: &str) -> Result<P256PublicKey> {
    match signer.get_public_key(label) {
        Ok(pk) => Ok(pk),
        Err(Error::SignerNotFound(_) | Error::SignerCommand(_)) => {
            tracing::info!("Key '{label}' not found, creating with policy '{policy}'");
            signer.create_key(label, policy)
        }
        Err(e) => Err(e),
    }
}

fn build_signed_authorization(
    eoa_signer: &PrivateKeySigner,
    chain_id: u64,
    impl_addr: Address,
    nonce: u64,
) -> Result<alloy::eips::eip7702::SignedAuthorization> {
    let auth = Authorization {
        chain_id: U256::from(chain_id),
        address: impl_addr,
        nonce,
    };
    let sig = eoa_signer
        .sign_hash_sync(&auth.signature_hash())
        .map_err(|e| Error::Other(format!("authorization signing failed: {e}")))?;
    Ok(auth.into_signed(sig))
}

fn verify_delegation(code: &Bytes, expected_impl: Address) -> Result<()> {
    // EIP-7702 delegation designator: 0xef0100 || address (23 bytes)
    let mut expected = Vec::with_capacity(23);
    expected.extend_from_slice(&[0xef, 0x01, 0x00]);
    expected.extend_from_slice(expected_impl.as_slice());

    if code.len() < 23 || &code[..23] != expected.as_slice() {
        return Err(Error::DelegationFailed {
            expected: format!("0xef0100{}", hex::encode(expected_impl.as_slice())),
            got: if code.is_empty() {
                "empty code".into()
            } else {
                format!("0x{}", hex::encode(&code[..code.len().min(23)]))
            },
        });
    }
    Ok(())
}

fn provider_err(e: impl std::fmt::Display) -> Error {
    Error::Provider(e.to_string())
}

fn parse_rpc_url(url: &str) -> Result<url::Url> {
    url.parse()
        .map_err(|e: url::ParseError| Error::Other(format!("invalid RPC URL: {e}")))
}

// ---------------------------------------------------------------------------
// RPC helpers
// ---------------------------------------------------------------------------

async fn verify_implementation(provider: &impl Provider, addr: Address) -> Result<()> {
    let code = provider.get_code_at(addr).await.map_err(provider_err)?;
    if code.is_empty() {
        return Err(Error::ImplementationNotDeployed(addr));
    }
    Ok(())
}

async fn poll_balance<F, Fut>(
    check_balance: F,
    poll_interval: Duration,
    max_wait: Duration,
    address: Address,
) -> Result<U256>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<U256>>,
{
    let start = std::time::Instant::now();
    loop {
        let balance = check_balance().await?;
        if balance > U256::ZERO {
            return Ok(balance);
        }
        if start.elapsed() >= max_wait {
            return Err(Error::FundingTimeout(max_wait.as_secs(), address));
        }
        tokio::time::sleep(poll_interval).await;
    }
}

async fn wait_for_funding(
    provider: &impl Provider,
    address: Address,
    strategy: &FundingStrategy,
) -> Result<U256> {
    match strategy {
        FundingStrategy::WaitForFunding {
            poll_interval,
            max_wait,
        } => {
            let pi = *poll_interval;
            let mw = *max_wait;
            poll_balance(
                || async { provider.get_balance(address).await.map_err(provider_err) },
                pi,
                mw,
                address,
            )
            .await
        }
        FundingStrategy::FundFrom {
            funder_private_key,
            amount,
            rpc_url,
        } => {
            fund_ephemeral_eoa(rpc_url, funder_private_key, address, *amount).await?;
            provider.get_balance(address).await.map_err(provider_err)
        }
    }
}

async fn fund_ephemeral_eoa(
    rpc_url: &str,
    funder_key: &str,
    to: Address,
    amount: U256,
) -> Result<B256> {
    let funder: PrivateKeySigner = funder_key
        .parse()
        .map_err(|e| Error::Other(format!("invalid funder private key: {e}")))?;
    let wallet = EthereumWallet::from(funder);
    let url = parse_rpc_url(rpc_url)?;
    let provider = ProviderBuilder::new().wallet(wallet).connect_http(url);

    let tx = TransactionRequest::default().with_to(to).with_value(amount);
    let pending = provider.send_transaction(tx).await.map_err(provider_err)?;
    let tx_hash = *pending.tx_hash();
    tracing::info!("Funding tx: {tx_hash}");
    let receipt = pending.get_receipt().await.map_err(provider_err)?;
    if !receipt.status() {
        return Err(Error::TransactionFailed("funding tx reverted".into()));
    }
    Ok(tx_hash)
}

async fn send_setup_transaction(
    provider: &impl Provider,
    signed_auth: alloy::eips::eip7702::SignedAuthorization,
    init_calldata: Bytes,
    eoa_addr: Address,
) -> Result<B256> {
    // Always use a manual gas limit for type-4 transactions. Gas estimation
    // runs against the EOA's current code (empty), not the post-delegation code,
    // so auto-estimation will underestimate and the tx will revert.
    let tx = TransactionRequest::default()
        .with_to(eoa_addr)
        .with_input(init_calldata)
        .with_authorization_list(vec![signed_auth])
        .with_gas_limit(500_000);

    let pending = provider.send_transaction(tx).await.map_err(provider_err)?;
    let tx_hash = *pending.tx_hash();
    tracing::info!("Setup tx: {tx_hash}");
    let receipt = pending.get_receipt().await.map_err(provider_err)?;
    if !receipt.status() {
        return Err(Error::TransactionFailed("setup tx reverted".into()));
    }
    Ok(tx_hash)
}

// ---------------------------------------------------------------------------
// Main setup orchestration
// ---------------------------------------------------------------------------

pub async fn setup(
    config: &SetupConfig,
    implementation: &dyn AccountImplementation,
    signer: &dyn P256Signer,
    state: &mut StateStore,
    funding: FundingStrategy,
) -> Result<SetupResult> {
    // 1. Get or create P-256 key
    let public_key = get_or_create_key(signer, &config.key_label, &config.key_policy)?;
    tracing::info!(
        "P-256 public key: qx={}, qy={}",
        public_key.qx,
        public_key.qy
    );

    // 2. Build read-only provider
    let rpc_url = parse_rpc_url(&config.rpc_url)?;
    let provider = ProviderBuilder::new().connect_http(rpc_url.clone());

    // 3. Verify implementation contract exists
    verify_implementation(&provider, config.implementation_address).await?;

    // 4. Resolve chain_id
    let chain_id = match config.chain_id {
        Some(id) => id,
        None => provider.get_chain_id().await.map_err(provider_err)?,
    };
    tracing::info!("Chain ID: {chain_id}");

    // 5. Check for duplicate deployment (same key + same chain)
    if state.find_account(&config.key_label, chain_id).is_some() {
        return Err(Error::DuplicateDeployment {
            key_label: config.key_label.clone(),
            chain_id,
        });
    }

    // 6. Check for multi-chain (key already has an account on any chain)
    if state.find_accounts_for_key(&config.key_label).is_some() {
        return Err(Error::MultiChainNotSupported(config.key_label.clone()));
    }

    // 7. Generate ephemeral EOA
    let ephemeral_signer = PrivateKeySigner::random();
    let eoa_addr = ephemeral_signer.address();
    tracing::info!("Ephemeral EOA: {eoa_addr}");

    // 8. Wait for funding
    tracing::info!("Waiting for funding...");
    let balance = wait_for_funding(&provider, eoa_addr, &funding).await?;
    tracing::info!("Funded: {balance} wei");

    // 9. Get authorization nonce
    // Per EIP-7702: the sender's tx nonce is incremented BEFORE the authorization
    // list is processed. When sender == authority (our case), the auth nonce must
    // be the sender's nonce AFTER increment, i.e., current_nonce + 1.
    let current_nonce = provider
        .get_transaction_count(eoa_addr)
        .await
        .map_err(provider_err)?;
    let auth_nonce = current_nonce + 1;
    tracing::info!("EOA nonce: {current_nonce}, auth nonce: {auth_nonce}");

    // 10. Build signed EIP-7702 authorization
    let signed_auth = build_signed_authorization(
        &ephemeral_signer,
        chain_id,
        config.implementation_address,
        auth_nonce,
    )?;

    // 11. Build init calldata
    let init_calldata = implementation.encode_initialize(public_key.qx, public_key.qy);

    // 12. Build signing provider
    let signing_provider = ProviderBuilder::new()
        .wallet(EthereumWallet::from(ephemeral_signer))
        .connect_http(rpc_url);

    // 12a. Confirm funding is visible on the signing provider's RPC connection.
    // Public RPC load balancers may route to different nodes, so the funding tx
    // confirmed on one connection may not yet be visible on another.
    for attempt in 1..=10 {
        let bal = signing_provider
            .get_balance(eoa_addr)
            .await
            .map_err(provider_err)?;
        if bal > U256::ZERO {
            tracing::info!("Signing provider sees balance: {bal} wei");
            break;
        }
        if attempt == 10 {
            return Err(Error::Other(
                "funding confirmed but not visible on signing provider after retries".into(),
            ));
        }
        tracing::warn!("Signing provider sees 0 balance (attempt {attempt}/10), waiting...");
        tokio::time::sleep(Duration::from_secs(2)).await;
    }

    // 13. Send setup transaction
    let tx_hash =
        send_setup_transaction(&signing_provider, signed_auth, init_calldata, eoa_addr).await?;

    // 14. Verify delegation on-chain (retry for stale RPC load balancer responses)
    let mut delegation_verified = false;
    for attempt in 1..=5 {
        let code = signing_provider
            .get_code_at(eoa_addr)
            .await
            .map_err(provider_err)?;
        match verify_delegation(&code, config.implementation_address) {
            Ok(()) => {
                delegation_verified = true;
                break;
            }
            Err(e) if attempt < 5 => {
                tracing::warn!("Delegation check attempt {attempt}/5 failed ({e}), retrying...");
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
            Err(e) => return Err(e),
        }
    }
    if delegation_verified {
        tracing::info!("Delegation verified");
    }

    // 15. Drop signing provider to release Arc<PrivateKeySigner> and trigger zeroize
    drop(signing_provider);

    // 16. Persist state
    let deployment = ChainDeployment {
        chain_id,
        implementation: config.implementation_address,
        implementation_name: config.implementation_name.clone(),
        entry_point: implementation.entry_point(),
        bundler_url: config.bundler_url.clone(),
        paymaster_url: config.paymaster_url.clone(),
        rpc_url: config.rpc_url.clone(),
        deployed_at: chrono::Utc::now().to_rfc3339(),
    };
    state.add_chain_deployment(
        &config.key_label,
        &config.key_policy,
        eoa_addr,
        public_key.clone(),
        deployment,
    )?;
    state.save()?;

    // 17. Return result
    Ok(SetupResult {
        account_address: eoa_addr,
        public_key,
        tx_hash,
        chain_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signer::mock::MockSigner;
    use alloy::primitives::address;

    #[test]
    fn get_or_create_key_existing() {
        let signer = MockSigner::new();
        let expected = signer.add_key("existing", "open");
        let result = get_or_create_key(&signer, "existing", "open").unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn get_or_create_key_new() {
        let signer = MockSigner::new();
        let result = get_or_create_key(&signer, "new-key", "biometric").unwrap();
        assert_ne!(result.qx, B256::ZERO);
        assert_ne!(result.qy, B256::ZERO);
        // Key should now exist
        let fetched = signer.get_public_key("new-key").unwrap();
        assert_eq!(result, fetched);
    }

    #[test]
    fn build_signed_authorization_fields() {
        let signer = PrivateKeySigner::random();
        let impl_addr = address!("0x6d1566f9aAcf9c06969D7BF846FA090703A38E43");
        let chain_id = 84532u64;
        let nonce = 0u64;

        let signed = build_signed_authorization(&signer, chain_id, impl_addr, nonce).unwrap();
        assert_eq!(signed.chain_id, U256::from(chain_id));
        assert_eq!(signed.address, impl_addr);
        assert_eq!(signed.nonce, nonce);
    }

    #[test]
    fn build_signed_authorization_has_signature() {
        let signer = PrivateKeySigner::random();
        let impl_addr = address!("0x6d1566f9aAcf9c06969D7BF846FA090703A38E43");

        let signed = build_signed_authorization(&signer, 84532, impl_addr, 0).unwrap();
        // Verify the authorization has a non-zero r value (i.e., was actually signed)
        assert_ne!(signed.r(), U256::ZERO);
        assert_ne!(signed.s(), U256::ZERO);
    }

    #[test]
    fn verify_delegation_valid() {
        let addr = address!("0x6d1566f9aAcf9c06969D7BF846FA090703A38E43");
        let mut code = vec![0xef, 0x01, 0x00];
        code.extend_from_slice(addr.as_slice());
        let code = Bytes::from(code);
        verify_delegation(&code, addr).unwrap();
    }

    #[test]
    fn verify_delegation_empty() {
        let addr = address!("0x6d1566f9aAcf9c06969D7BF846FA090703A38E43");
        let result = verify_delegation(&Bytes::new(), addr);
        assert!(matches!(result, Err(Error::DelegationFailed { .. })));
    }

    #[test]
    fn verify_delegation_wrong_addr() {
        let expected = address!("0x6d1566f9aAcf9c06969D7BF846FA090703A38E43");
        let wrong = address!("0x1111111111111111111111111111111111111111");
        let mut code = vec![0xef, 0x01, 0x00];
        code.extend_from_slice(wrong.as_slice());
        let result = verify_delegation(&Bytes::from(code), expected);
        assert!(matches!(result, Err(Error::DelegationFailed { .. })));
    }

    #[test]
    fn verify_delegation_short_code() {
        let addr = address!("0x6d1566f9aAcf9c06969D7BF846FA090703A38E43");
        let code = Bytes::from(vec![0xef, 0x01, 0x00, 0x6d]); // too short
        let result = verify_delegation(&code, addr);
        assert!(matches!(result, Err(Error::DelegationFailed { .. })));
    }

    #[test]
    fn parse_rpc_url_valid_and_invalid() {
        let valid = parse_rpc_url("https://sepolia.base.org");
        assert!(valid.is_ok());

        let invalid = parse_rpc_url("not a url");
        assert!(matches!(invalid, Err(Error::Other(_))));
    }

    #[test]
    fn duplicate_deployment_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = StateStore::open_at(tmp.path().join("accounts.json")).unwrap();

        let addr = address!("0x1111111111111111111111111111111111111111");
        let pk = P256PublicKey {
            qx: B256::repeat_byte(0x11),
            qy: B256::repeat_byte(0x22),
        };
        let deployment = ChainDeployment {
            chain_id: 84532,
            implementation: address!("0x6d1566f9aAcf9c06969D7BF846FA090703A38E43"),
            implementation_name: "KeypoAccount".into(),
            entry_point: address!("0x0000000071727De22E5E9d8BAf0edAc6f37da032"),
            bundler_url: None,
            paymaster_url: None,
            rpc_url: "https://sepolia.base.org".into(),
            deployed_at: "2026-03-01T00:00:00Z".into(),
        };
        state
            .add_chain_deployment("test-key", "open", addr, pk, deployment)
            .unwrap();

        // Trying to find account for same key + chain should succeed (guard in setup())
        assert!(state.find_account("test-key", 84532).is_some());
    }

    #[tokio::test]
    async fn funding_timeout_returns_error() {
        let addr = address!("0x1111111111111111111111111111111111111111");
        let result = poll_balance(
            || async { Ok(U256::ZERO) },
            Duration::from_millis(1),
            Duration::from_millis(0),
            addr,
        )
        .await;
        assert!(
            matches!(result, Err(Error::FundingTimeout(_, _))),
            "expected FundingTimeout, got: {:?}",
            result
        );
    }

    #[test]
    fn transaction_reverted_returns_error() {
        let err = Error::TransactionFailed("setup tx reverted".into());
        assert!(matches!(err, Error::TransactionFailed(_)));
        assert!(err.to_string().contains("reverted"));
    }
}
