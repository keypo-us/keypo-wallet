use std::path::Path;
use std::time::Duration;

use alloy::primitives::Address;
use clap::{Parser, Subcommand};
use keypo_wallet::account::{self, FundingStrategy, SetupConfig, SETUP_FUNDING_AMOUNT};
use keypo_wallet::AccountImplementation;
use keypo_wallet::impls::KeypoAccountImpl;
use keypo_wallet::signer::KeypoSigner;
use keypo_wallet::state::StateStore;

#[derive(Parser)]
#[command(name = "keypo-wallet", about = "Keypo smart wallet CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Set up a smart account on a chain
    Setup {
        /// Key label for the signing key
        #[arg(long)]
        key: String,

        /// Key access policy
        #[arg(long, default_value = "biometric")]
        key_policy: String,

        /// RPC URL for the chain
        #[arg(long)]
        rpc: Option<String>,

        /// Bundler URL
        #[arg(long)]
        bundler: Option<String>,

        /// Chain ID
        #[arg(long)]
        chain_id: Option<u64>,

        /// Paymaster URL
        #[arg(long)]
        paymaster: Option<String>,

        /// Implementation contract address
        #[arg(long)]
        implementation: Option<String>,

        /// Implementation name
        #[arg(long, default_value = "KeypoAccount")]
        impl_name: String,
    },

    /// Send a transaction
    Send {
        /// Key label for the signing key
        #[arg(long)]
        key: String,

        /// Recipient address
        #[arg(long)]
        to: String,

        /// Value in wei (default "0")
        #[arg(long, default_value = "0")]
        value: String,

        /// Calldata (hex-encoded)
        #[arg(long)]
        data: Option<String>,

        /// Chain ID
        #[arg(long)]
        chain_id: Option<u64>,
    },

    /// Send a batch of calls
    Batch {
        /// Key label for the signing key
        #[arg(long)]
        key: String,

        /// Path to JSON file with calls
        #[arg(long)]
        calls: String,

        /// Chain ID
        #[arg(long)]
        chain_id: Option<u64>,
    },

    /// Show account info
    Info {
        /// Key label for the signing key
        #[arg(long)]
        key: String,

        /// Chain ID
        #[arg(long)]
        chain_id: Option<u64>,
    },

    /// Check account balance
    Balance {
        /// Key label for the signing key
        #[arg(long)]
        key: String,

        /// Chain ID
        #[arg(long)]
        chain_id: Option<u64>,

        /// Token contract address (omit for native balance)
        #[arg(long)]
        token: Option<String>,

        /// Path to query JSON file
        #[arg(long)]
        query: Option<String>,
    },
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().try_init().ok();

    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Setup {
            key,
            key_policy,
            rpc,
            bundler,
            chain_id,
            paymaster,
            implementation,
            impl_name,
        } => {
            run_setup(
                key,
                key_policy,
                rpc,
                bundler,
                chain_id,
                paymaster,
                implementation,
                impl_name,
            )
            .await
        }
        Commands::Send { .. } => {
            println!("send: not implemented");
            Ok(())
        }
        Commands::Batch { .. } => {
            println!("batch: not implemented");
            Ok(())
        }
        Commands::Info { .. } => {
            println!("info: not implemented");
            Ok(())
        }
        Commands::Balance { .. } => {
            println!("balance: not implemented");
            Ok(())
        }
    };

    if let Err(e) = result {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

async fn run_setup(
    key: String,
    key_policy: String,
    rpc: Option<String>,
    bundler: Option<String>,
    chain_id: Option<u64>,
    paymaster: Option<String>,
    implementation: Option<String>,
    impl_name: String,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let rpc_url = rpc.ok_or("--rpc is required for setup")?;

    // Resolve implementation address and chain_id
    let (impl_address, resolved_chain_id, imp) = if let Some(ref addr_str) = implementation {
        // Explicit implementation address provided
        let addr: Address = addr_str.parse().map_err(|e| format!("invalid --implementation address: {e}"))?;
        let imp = KeypoAccountImpl::new();
        (addr, chain_id, imp)
    } else {
        // Need chain_id to look up deployment
        let cid = if let Some(id) = chain_id {
            id
        } else {
            // Auto-detect chain_id from RPC
            use alloy::providers::{Provider, ProviderBuilder};
            let url: url::Url = rpc_url.parse().map_err(|e: url::ParseError| format!("invalid RPC URL: {e}"))?;
            let provider = ProviderBuilder::new().connect_http(url);
            provider.get_chain_id().await.map_err(|e| format!("failed to get chain ID: {e}"))?
        };

        // Load deployments from deployments/ directory
        let imp = load_deployments_impl();
        let addr = imp.implementation_address(cid).ok_or_else(|| {
            format!(
                "No deployment found for chain {cid}. Use --implementation to specify the contract address."
            )
        })?;
        (addr, Some(cid), imp)
    };

    let signer = KeypoSigner::new();
    let mut state = StateStore::open()?;

    let config = SetupConfig {
        key_label: key,
        key_policy,
        rpc_url: rpc_url.clone(),
        bundler_url: bundler,
        paymaster_url: paymaster,
        implementation_address: impl_address,
        implementation_name: impl_name,
        chain_id: resolved_chain_id,
    };

    // Determine funding strategy
    let funding = if let Ok(funder_key) = std::env::var("TEST_FUNDER_PRIVATE_KEY") {
        FundingStrategy::FundFrom {
            funder_private_key: funder_key,
            amount: SETUP_FUNDING_AMOUNT,
            rpc_url,
        }
    } else {
        FundingStrategy::WaitForFunding {
            poll_interval: Duration::from_secs(5),
            max_wait: Duration::from_secs(300),
        }
    };

    let result = account::setup(&config, &imp, &signer, &mut state, funding).await?;

    println!("Account setup complete!");
    println!("  Address:  {}", result.account_address);
    println!("  Tx hash:  {}", result.tx_hash);
    println!("  Chain ID: {}", result.chain_id);

    Ok(())
}

fn load_deployments_impl() -> KeypoAccountImpl {
    // Try CARGO_MANIFEST_DIR (works for cargo run), fall back to relative path
    let deployments_dir = option_env!("CARGO_MANIFEST_DIR")
        .map(|dir| Path::new(dir).join("..").join("deployments"))
        .unwrap_or_else(|| Path::new("deployments").to_path_buf());

    match KeypoAccountImpl::from_deployments_dir(&deployments_dir) {
        Ok(imp) => imp,
        Err(_) => KeypoAccountImpl::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn setup_all_args() {
        let cli = Cli::try_parse_from([
            "keypo-wallet",
            "setup",
            "--key",
            "my-key",
            "--key-policy",
            "open",
            "--rpc",
            "https://rpc.example.com",
            "--bundler",
            "https://bundler.example.com",
            "--chain-id",
            "84532",
            "--paymaster",
            "https://paymaster.example.com",
            "--implementation",
            "0x1234",
            "--impl-name",
            "KeypoAccount",
        ])
        .unwrap();

        match cli.command {
            Commands::Setup {
                key,
                key_policy,
                chain_id,
                ..
            } => {
                assert_eq!(key, "my-key");
                assert_eq!(key_policy, "open");
                assert_eq!(chain_id, Some(84532));
            }
            _ => panic!("expected Setup"),
        }
    }

    #[test]
    fn setup_defaults() {
        let cli = Cli::try_parse_from(["keypo-wallet", "setup", "--key", "test"]).unwrap();

        match cli.command {
            Commands::Setup {
                key_policy,
                impl_name,
                ..
            } => {
                assert_eq!(key_policy, "biometric");
                assert_eq!(impl_name, "KeypoAccount");
            }
            _ => panic!("expected Setup"),
        }
    }

    #[test]
    fn send_all_args() {
        let cli = Cli::try_parse_from([
            "keypo-wallet",
            "send",
            "--key",
            "my-key",
            "--to",
            "0xdead",
            "--value",
            "1000000000000000000",
            "--data",
            "0x1234",
            "--chain-id",
            "1",
        ])
        .unwrap();

        match cli.command {
            Commands::Send {
                key, to, value, ..
            } => {
                assert_eq!(key, "my-key");
                assert_eq!(to, "0xdead");
                assert_eq!(value, "1000000000000000000");
            }
            _ => panic!("expected Send"),
        }
    }

    #[test]
    fn batch_args() {
        let cli = Cli::try_parse_from([
            "keypo-wallet",
            "batch",
            "--key",
            "my-key",
            "--calls",
            "calls.json",
            "--chain-id",
            "84532",
        ])
        .unwrap();

        match cli.command {
            Commands::Batch {
                key,
                calls,
                chain_id,
            } => {
                assert_eq!(key, "my-key");
                assert_eq!(calls, "calls.json");
                assert_eq!(chain_id, Some(84532));
            }
            _ => panic!("expected Batch"),
        }
    }

    #[test]
    fn info_args() {
        let cli =
            Cli::try_parse_from(["keypo-wallet", "info", "--key", "my-key", "--chain-id", "1"])
                .unwrap();

        match cli.command {
            Commands::Info { key, chain_id } => {
                assert_eq!(key, "my-key");
                assert_eq!(chain_id, Some(1));
            }
            _ => panic!("expected Info"),
        }
    }

    #[test]
    fn balance_args() {
        let cli = Cli::try_parse_from([
            "keypo-wallet",
            "balance",
            "--key",
            "my-key",
            "--chain-id",
            "84532",
            "--token",
            "0xUSDC",
            "--query",
            "query.json",
        ])
        .unwrap();

        match cli.command {
            Commands::Balance {
                key,
                token,
                query,
                ..
            } => {
                assert_eq!(key, "my-key");
                assert_eq!(token, Some("0xUSDC".into()));
                assert_eq!(query, Some("query.json".into()));
            }
            _ => panic!("expected Balance"),
        }
    }

    #[test]
    fn missing_required_key_fails() {
        let result = Cli::try_parse_from(["keypo-wallet", "setup"]);
        assert!(result.is_err());
    }
}
