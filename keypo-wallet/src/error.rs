use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("signer not found: {0}")]
    SignerNotFound(String),

    #[error("signer command failed: {0}")]
    SignerCommand(String),

    #[error("signer output error: {0}")]
    SignerOutput(String),

    #[error("state I/O error: {0}")]
    StateIO(#[from] std::io::Error),

    #[error("state format error: {0}")]
    StateFormat(#[from] serde_json::Error),

    #[error("ABI encoding error: {0}")]
    AbiEncoding(String),

    #[error("provider error: {0}")]
    Provider(String),

    #[error("bundler error: {0}")]
    Bundler(String),

    #[error("paymaster error: {0}")]
    Paymaster(String),

    #[error("account not found: {0}")]
    AccountNotFound(String),

    #[error("chain not deployed: {0}")]
    ChainNotDeployed(u64),

    #[error("duplicate deployment: key {key_label} already deployed on chain {chain_id}")]
    DuplicateDeployment { key_label: String, chain_id: u64 },

    #[error("{0}")]
    Other(String),
}
