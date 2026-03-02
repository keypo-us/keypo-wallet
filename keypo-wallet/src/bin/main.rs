use clap::{Parser, Subcommand};

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

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Setup { .. } => println!("setup: not implemented"),
        Commands::Send { .. } => println!("send: not implemented"),
        Commands::Batch { .. } => println!("batch: not implemented"),
        Commands::Info { .. } => println!("info: not implemented"),
        Commands::Balance { .. } => println!("balance: not implemented"),
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
