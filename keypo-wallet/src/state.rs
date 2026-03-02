use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::types::{AccountRecord, ChainDeployment, P256PublicKey};
use alloy::primitives::Address;

#[derive(Debug, Serialize, Deserialize)]
struct StateFile {
    accounts: Vec<AccountRecord>,
}

/// Persistent store for account records, backed by a JSON file.
#[derive(Debug)]
pub struct StateStore {
    path: PathBuf,
    state: StateFile,
}

impl StateStore {
    /// Opens the default state file at `~/.keypo/accounts.json`.
    ///
    /// Creates the directory (mode 0o700) and file if they don't exist.
    /// Returns an error if the file exists but contains invalid JSON.
    pub fn open() -> Result<Self> {
        let home = dirs::home_dir()
            .ok_or_else(|| Error::Other("could not determine home directory".into()))?;
        let path = home.join(".keypo").join("accounts.json");
        Self::open_at(path)
    }

    /// Opens a state file at a custom path.
    ///
    /// Creates parent directories and the file if they don't exist.
    /// Returns an error if the file exists but contains invalid JSON.
    pub fn open_at(path: PathBuf) -> Result<Self> {
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent)?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))?;
                }
            }
        }

        let state = if path.exists() {
            let contents = std::fs::read_to_string(&path)?;
            serde_json::from_str::<StateFile>(&contents)?
        } else {
            let initial = StateFile {
                accounts: Vec::new(),
            };
            let json = serde_json::to_string_pretty(&initial)?;
            std::fs::write(&path, &json)?;
            initial
        };

        Ok(Self { path, state })
    }

    /// Finds an account by key label and chain ID.
    pub fn find_account(
        &self,
        key_label: &str,
        chain_id: u64,
    ) -> Option<(&AccountRecord, &ChainDeployment)> {
        self.state.accounts.iter().find_map(|acct| {
            if acct.key_label == key_label {
                acct.chains
                    .iter()
                    .find(|c| c.chain_id == chain_id)
                    .map(|chain| (acct, chain))
            } else {
                None
            }
        })
    }

    /// Finds an account record by key label (regardless of chain).
    pub fn find_accounts_for_key(&self, key_label: &str) -> Option<&AccountRecord> {
        self.state
            .accounts
            .iter()
            .find(|acct| acct.key_label == key_label)
    }

    /// Adds a chain deployment to an account, creating the account record if needed.
    ///
    /// Returns `Error::DuplicateDeployment` if the key already has a deployment on this chain.
    pub fn add_chain_deployment(
        &mut self,
        key_label: &str,
        key_policy: &str,
        address: Address,
        public_key: P256PublicKey,
        deployment: ChainDeployment,
    ) -> Result<()> {
        let chain_id = deployment.chain_id;

        if let Some(acct) = self
            .state
            .accounts
            .iter_mut()
            .find(|a| a.key_label == key_label)
        {
            if acct.chains.iter().any(|c| c.chain_id == chain_id) {
                return Err(Error::DuplicateDeployment {
                    key_label: key_label.to_string(),
                    chain_id,
                });
            }
            acct.chains.push(deployment);
        } else {
            let now = chrono::Utc::now().to_rfc3339();
            self.state.accounts.push(AccountRecord {
                address,
                key_label: key_label.to_string(),
                key_policy: key_policy.to_string(),
                public_key,
                chains: vec![deployment],
                created_at: now,
            });
        }

        Ok(())
    }

    /// Returns all account records.
    pub fn list_accounts(&self) -> &[AccountRecord] {
        &self.state.accounts
    }

    /// Atomically saves the state to disk (write to temp, then rename).
    pub fn save(&self) -> Result<()> {
        let tmp_path = self.path.with_extension("tmp");
        let json = serde_json::to_string_pretty(&self.state)?;
        std::fs::write(&tmp_path, &json)?;
        std::fs::rename(&tmp_path, &self.path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::{address, B256};
    use tempfile::TempDir;

    fn test_path(dir: &TempDir) -> PathBuf {
        dir.path().join("keypo").join("accounts.json")
    }

    fn sample_deployment(chain_id: u64) -> ChainDeployment {
        ChainDeployment {
            chain_id,
            implementation: address!("0x6d1566f9aAcf9c06969D7BF846FA090703A38E43"),
            implementation_name: "KeypoAccount".into(),
            entry_point: address!("0x0000000071727De22E5E9d8BAf0edAc6f37da032"),
            bundler_url: "https://bundler.example.com".into(),
            paymaster_url: None,
            rpc_url: "https://sepolia.base.org".into(),
            deployed_at: "2026-03-01T00:00:00Z".into(),
        }
    }

    fn sample_pubkey() -> P256PublicKey {
        P256PublicKey {
            qx: B256::repeat_byte(0x11),
            qy: B256::repeat_byte(0x22),
        }
    }

    #[test]
    fn creates_dir_and_file_from_scratch() {
        let dir = TempDir::new().unwrap();
        let path = test_path(&dir);
        assert!(!path.exists());

        let store = StateStore::open_at(path.clone()).unwrap();
        assert!(path.exists());
        assert!(store.list_accounts().is_empty());
    }

    #[test]
    fn opens_existing_valid_file() {
        let dir = TempDir::new().unwrap();
        let path = test_path(&dir);

        // Create initial
        let _store = StateStore::open_at(path.clone()).unwrap();

        // Re-open
        let store = StateStore::open_at(path).unwrap();
        assert!(store.list_accounts().is_empty());
    }

    #[test]
    fn open_corrupt_file_returns_error() {
        let dir = TempDir::new().unwrap();
        let path = test_path(&dir);

        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "not valid json!!!").unwrap();

        let result = StateStore::open_at(path);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, Error::StateFormat(_)),
            "expected StateFormat error, got: {:?}",
            err
        );
    }

    #[test]
    fn add_first_deployment() {
        let dir = TempDir::new().unwrap();
        let path = test_path(&dir);
        let mut store = StateStore::open_at(path).unwrap();

        let addr = address!("0x1111111111111111111111111111111111111111");
        store
            .add_chain_deployment("my-key", "biometric", addr, sample_pubkey(), sample_deployment(84532))
            .unwrap();

        assert_eq!(store.list_accounts().len(), 1);
        assert_eq!(store.list_accounts()[0].chains.len(), 1);
        assert_eq!(store.list_accounts()[0].chains[0].chain_id, 84532);
    }

    #[test]
    fn add_second_chain_to_same_key() {
        let dir = TempDir::new().unwrap();
        let path = test_path(&dir);
        let mut store = StateStore::open_at(path).unwrap();

        let addr = address!("0x1111111111111111111111111111111111111111");
        store
            .add_chain_deployment("my-key", "biometric", addr, sample_pubkey(), sample_deployment(84532))
            .unwrap();
        store
            .add_chain_deployment("my-key", "biometric", addr, sample_pubkey(), sample_deployment(1))
            .unwrap();

        assert_eq!(store.list_accounts().len(), 1);
        assert_eq!(store.list_accounts()[0].chains.len(), 2);
    }

    #[test]
    fn duplicate_chain_id_rejected() {
        let dir = TempDir::new().unwrap();
        let path = test_path(&dir);
        let mut store = StateStore::open_at(path).unwrap();

        let addr = address!("0x1111111111111111111111111111111111111111");
        store
            .add_chain_deployment("my-key", "biometric", addr, sample_pubkey(), sample_deployment(84532))
            .unwrap();

        let result =
            store.add_chain_deployment("my-key", "biometric", addr, sample_pubkey(), sample_deployment(84532));
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, Error::DuplicateDeployment { .. }),
            "expected DuplicateDeployment, got: {:?}",
            err
        );
    }

    #[test]
    fn find_account_by_key_and_chain() {
        let dir = TempDir::new().unwrap();
        let path = test_path(&dir);
        let mut store = StateStore::open_at(path).unwrap();

        let addr = address!("0x1111111111111111111111111111111111111111");
        store
            .add_chain_deployment("my-key", "biometric", addr, sample_pubkey(), sample_deployment(84532))
            .unwrap();

        let (acct, chain) = store.find_account("my-key", 84532).unwrap();
        assert_eq!(acct.key_label, "my-key");
        assert_eq!(chain.chain_id, 84532);
    }

    #[test]
    fn find_missing_key_returns_none() {
        let dir = TempDir::new().unwrap();
        let path = test_path(&dir);
        let store = StateStore::open_at(path).unwrap();

        assert!(store.find_account("nonexistent", 1).is_none());
        assert!(store.find_accounts_for_key("nonexistent").is_none());
    }

    #[test]
    fn save_and_reload_roundtrip() {
        let dir = TempDir::new().unwrap();
        let path = test_path(&dir);

        {
            let mut store = StateStore::open_at(path.clone()).unwrap();
            let addr = address!("0x1111111111111111111111111111111111111111");
            store
                .add_chain_deployment("my-key", "open", addr, sample_pubkey(), sample_deployment(84532))
                .unwrap();
            store.save().unwrap();
        }

        // Reload
        let store = StateStore::open_at(path).unwrap();
        assert_eq!(store.list_accounts().len(), 1);
        assert_eq!(store.list_accounts()[0].key_label, "my-key");
        assert_eq!(store.list_accounts()[0].key_policy, "open");
        assert_eq!(store.list_accounts()[0].chains[0].chain_id, 84532);
    }
}
