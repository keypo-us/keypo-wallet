use alloy::primitives::{Address, U256};
use clap::{Parser, Subcommand};
use keypo_pay::address::derive_tempo_address;
use keypo_pay::config;
use keypo_pay::rlp::TempoCall;
use keypo_pay::signer::{KeypoSigner, P256Signer};

#[derive(Parser)]
#[command(
    name = "keypo-pay",
    about = "Keypo Tempo wallet CLI",
    long_about = "Keypo Tempo wallet CLI — manage Tempo accounts with P-256 (Secure Enclave) signing.\n\n\
        Uses root-key-plus-access-keys architecture for agent-initiated payments.\n\
        Requires keypo-signer for key management: brew install keypo-us/tap/keypo-signer",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// RPC URL override (takes precedence over config and env vars)
    #[arg(long, global = true)]
    rpc: Option<String>,

    /// Enable verbose debug output
    #[arg(long, global = true)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Manage wallet (create, info)
    #[command(subcommand)]
    Wallet(WalletAction),

    /// Send a Tempo transaction (low-level, defaults to root key)
    #[command(subcommand)]
    Tx(TxAction),
}

#[derive(Subcommand, Clone)]
enum TxAction {
    /// Send a transaction
    #[command(
        long_about = "Build, sign, and submit a Tempo transaction.\n\n\
            Defaults to root key signing. Use --key to sign with a named access key.",
        after_long_help = "\
EXAMPLES:
  keypo-pay tx send --to 0xRecipient --token pathusd --amount 0.01
  keypo-pay tx send --to 0xRecipient --token pathusd --amount 0.01 --key agent-1"
    )]
    Send {
        /// Recipient address
        #[arg(long)]
        to: String,

        /// Token name or address
        #[arg(long)]
        token: String,

        /// Amount in human-readable units (e.g., 0.01)
        #[arg(long)]
        amount: String,

        /// Named access key to sign with (Keychain signature)
        #[arg(long)]
        key: Option<String>,
    },
}

#[derive(Subcommand, Clone)]
enum WalletAction {
    /// Create a new wallet (root key only)
    #[command(
        long_about = "Create a new keypo-pay wallet.\n\n\
            Generates a root P-256 key in the Secure Enclave and derives the Tempo account address.\n\
            Use --test to create with open policy (no biometric prompt) for automated testing.",
        after_long_help = "\
EXAMPLES:
  keypo-pay wallet create
  keypo-pay wallet create --test

OUTPUT:
  Wallet created!
  Address: 0x1234...abcd
  Root key: com.keypo.signer.tempo-root"
    )]
    Create {
        /// Use open policy for root key (automated testing)
        #[arg(long)]
        test: bool,
    },

    /// Show wallet info
    #[command(
        long_about = "Display wallet address, root key ID, chain ID, and access key status.",
        after_long_help = "\
EXAMPLES:
  keypo-pay wallet info

OUTPUT:
  Address:   0x1234...abcd
  Root key:  com.keypo.signer.tempo-root
  Chain ID:  12345
  RPC:       https://rpc.moderato.tempo.xyz"
    )]
    Info,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let filter = tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        if cli.verbose {
            tracing_subscriber::EnvFilter::new("keypo_pay=debug")
        } else {
            tracing_subscriber::EnvFilter::new("keypo_pay=warn")
        }
    });
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .try_init()
        .ok();

    let result = match cli.command {
        Commands::Wallet(action) => match action {
            WalletAction::Create { test } => run_wallet_create(test).await,
            WalletAction::Info => run_wallet_info(cli.rpc.as_deref()).await,
        },
        Commands::Tx(action) => match action {
            TxAction::Send {
                to,
                token,
                amount,
                key,
            } => run_tx_send(cli.rpc.as_deref(), &to, &token, &amount, key.as_deref()).await,
        },
    };

    if let Err(e) = result {
        eprintln!("Error: {e}");
        if let Some(ke) = e.downcast_ref::<keypo_pay::Error>() {
            if let Some(hint) = ke.suggestion() {
                eprintln!("  Hint: {hint}");
            }
        }
        std::process::exit(1);
    }
}

async fn run_wallet_create(test: bool) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let wallet_path = config::wallet_config_path()?;

    // Idempotency guard
    if wallet_path.exists() {
        return Err(keypo_pay::Error::WalletExists(wallet_path.display().to_string()).into());
    }

    let signer = KeypoSigner::new();
    let policy = if test { "open" } else { "biometric" };
    let label = "tempo-root";

    // Create root key
    let pub_key = signer.create_key(label, policy)?;
    let address = derive_tempo_address(&pub_key);

    // Build and save wallet config
    let wallet = config::WalletConfig {
        chain_id: config::TESTNET_CHAIN_ID,
        rpc_url: config::TESTNET_RPC_URL.to_string(),
        root_key_id: format!("com.keypo.signer.{}", label),
        address: format!("{address}"),
        default_token: Some("pathusd".to_string()),
        block_explorer_url: None,
    };

    config::save_wallet_config(&wallet)?;

    // Save default tokens
    let tokens = config::default_testnet_tokens();
    config::save_tokens(&tokens)?;

    // Create empty access keys file
    let access_keys = config::AccessKeysFile::default();
    config::save_access_keys(&access_keys)?;

    println!("Wallet created!");
    println!("  Address:  {address}");
    println!("  Root key: com.keypo.signer.{label}");
    println!("  Chain ID: {}", wallet.chain_id);
    println!("  RPC:      {}", wallet.rpc_url);

    Ok(())
}

async fn run_wallet_info(
    rpc_override: Option<&str>,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let wallet = config::load_wallet_config()?;
    let access_keys = config::load_access_keys()?;

    let rpc = config::resolve_rpc(rpc_override, &wallet);

    println!("Address:   {}", wallet.address);
    println!("Root key:  {}", wallet.root_key_id);
    println!("Chain ID:  {}", wallet.chain_id);
    println!("RPC:       {rpc}");

    if access_keys.keys.is_empty() {
        println!("Access keys: (none)");
    } else {
        println!("Access keys:");
        for key in &access_keys.keys {
            println!("  {} — {} ({})", key.name, key.address, key.key_id);
        }
    }

    Ok(())
}

async fn run_tx_send(
    rpc_override: Option<&str>,
    to: &str,
    token: &str,
    amount: &str,
    key_name: Option<&str>,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let wallet = config::load_wallet_config()?;
    let tokens = config::load_tokens()?;
    let rpc_url = config::resolve_rpc(rpc_override, &wallet);

    // Resolve token address
    let token_addr_str = config::resolve_token(token, &tokens.tokens)?;
    let token_addr: Address = token_addr_str
        .parse()
        .map_err(|e| format!("invalid token address: {e}"))?;

    // Query token decimals
    let client = reqwest::Client::new();
    let decimals_data = keypo_pay::transaction::encode_decimals();
    let decimals_result = keypo_pay::rpc::eth_call(&client, &rpc_url, token_addr, &decimals_data).await
        .map_err(|e| format!("failed to query decimals: {e}"))?;
    let decimals = if decimals_result.len() >= 32 {
        decimals_result[31] // last byte of the 32-byte uint256
    } else {
        18 // fallback
    };
    tracing::debug!("token decimals: {decimals}");

    // Parse amount with correct decimals
    let amount_f64: f64 = amount
        .parse()
        .map_err(|e| format!("invalid amount '{amount}': {e}"))?;
    let multiplier = 10f64.powi(decimals as i32);
    let amount_wei = U256::from((amount_f64 * multiplier) as u128);

    // Resolve recipient
    let to_addr: Address = to
        .parse()
        .map_err(|e| format!("invalid --to address: {e}"))?;

    // Build TIP-20 transfer call
    let calldata = keypo_pay::transaction::encode_tip20_transfer(to_addr, amount_wei);
    let call = TempoCall {
        to: token_addr,
        value: U256::ZERO,
        data: calldata,
    };

    let signer = KeypoSigner::new();

    // Determine signing key and signature type
    let (signing_label, root_address) = if let Some(name) = key_name {
        // Access key signing (Keychain signature)
        let access_keys = config::load_access_keys()?;
        let entry = access_keys
            .keys
            .iter()
            .find(|k| k.name == name)
            .ok_or_else(|| keypo_pay::Error::AccessKeyNotFound(name.to_string()))?;
        let wallet_addr: Address = wallet.address.parse()
            .map_err(|e| format!("invalid wallet address: {e}"))?;
        (entry.key_id.split('.').next_back().unwrap_or(&entry.key_id).to_string(), Some(wallet_addr))
    } else {
        // Root key signing (P-256 signature)
        let label = wallet
            .root_key_id
            .strip_prefix("com.keypo.signer.")
            .unwrap_or(&wallet.root_key_id);
        (label.to_string(), None)
    };

    let result = keypo_pay::transaction::send_tempo_tx(
        &rpc_url,
        &wallet,
        vec![call],
        &signer,
        &signing_label,
        root_address,
        None,
    )
    .await?;

    println!("Transaction sent!");
    println!("  Tx hash:  {}", result.tx_hash);
    println!("  Block:    {}", result.block_number);
    println!("  Gas used: {}", result.gas_used);
    println!("  Success:  {}", result.success);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn wallet_create_parses() {
        let cli = Cli::try_parse_from(["keypo-pay", "wallet", "create"]).unwrap();
        match cli.command {
            Commands::Wallet(WalletAction::Create { test }) => {
                assert!(!test);
            }
            _ => panic!("expected Wallet Create"),
        }
    }

    #[test]
    fn wallet_create_test_flag() {
        let cli = Cli::try_parse_from(["keypo-pay", "wallet", "create", "--test"]).unwrap();
        match cli.command {
            Commands::Wallet(WalletAction::Create { test }) => {
                assert!(test);
            }
            _ => panic!("expected Wallet Create"),
        }
    }

    #[test]
    fn wallet_info_parses() {
        let cli = Cli::try_parse_from(["keypo-pay", "wallet", "info"]).unwrap();
        assert!(matches!(cli.command, Commands::Wallet(WalletAction::Info)));
    }

    #[test]
    fn global_rpc_flag() {
        let cli =
            Cli::try_parse_from(["keypo-pay", "--rpc", "https://custom.rpc", "wallet", "info"])
                .unwrap();
        assert_eq!(cli.rpc, Some("https://custom.rpc".into()));
    }

    #[test]
    fn verbose_flag() {
        let cli =
            Cli::try_parse_from(["keypo-pay", "--verbose", "wallet", "info"]).unwrap();
        assert!(cli.verbose);
    }

    #[test]
    fn tx_send_parses() {
        let cli = Cli::try_parse_from([
            "keypo-pay",
            "tx",
            "send",
            "--to",
            "0xdead",
            "--token",
            "pathusd",
            "--amount",
            "0.01",
        ])
        .unwrap();
        match cli.command {
            Commands::Tx(TxAction::Send {
                to,
                token,
                amount,
                key,
            }) => {
                assert_eq!(to, "0xdead");
                assert_eq!(token, "pathusd");
                assert_eq!(amount, "0.01");
                assert!(key.is_none());
            }
            _ => panic!("expected Tx Send"),
        }
    }

    #[test]
    fn tx_send_with_key() {
        let cli = Cli::try_parse_from([
            "keypo-pay",
            "tx",
            "send",
            "--to",
            "0xdead",
            "--token",
            "pathusd",
            "--amount",
            "0.01",
            "--key",
            "agent-1",
        ])
        .unwrap();
        match cli.command {
            Commands::Tx(TxAction::Send { key, .. }) => {
                assert_eq!(key, Some("agent-1".into()));
            }
            _ => panic!("expected Tx Send"),
        }
    }
}
