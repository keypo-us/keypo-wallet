use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

// ---------------------------------------------------------------------------
// Public config type
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct Config {
    pub rpc_url: Option<String>,
    pub bundler_url: Option<String>,
    pub paymaster_url: Option<String>,
    pub paymaster_policy_id: Option<String>,
}

// ---------------------------------------------------------------------------
// TOML schema
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ConfigFile {
    network: Option<NetworkConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct NetworkConfig {
    rpc_url: Option<String>,
    bundler_url: Option<String>,
    paymaster_url: Option<String>,
    paymaster_policy_id: Option<String>,
}

// Known top-level keys and their sub-keys for unknown-key detection.
const KNOWN_KEYS: &[&str] = &[
    "network",
    "network.rpc_url",
    "network.bundler_url",
    "network.paymaster_url",
    "network.paymaster_policy_id",
];

// ---------------------------------------------------------------------------
// Path helpers
// ---------------------------------------------------------------------------

pub fn config_dir() -> Result<PathBuf> {
    let home = dirs::home_dir()
        .ok_or_else(|| Error::Other("could not determine home directory".into()))?;
    Ok(home.join(".keypo"))
}

pub fn config_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("config.toml"))
}

// ---------------------------------------------------------------------------
// Load + validate
// ---------------------------------------------------------------------------

/// Loads and validates `~/.keypo/config.toml`. Returns `None` if the file
/// does not exist, `Err` on malformed TOML or invalid URLs.
pub fn load_config() -> Result<Option<Config>> {
    load_config_at(&config_path()?)
}

/// Loads and validates a config file at the given path.
pub fn load_config_at(path: &Path) -> Result<Option<Config>> {
    if !path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(path)
        .map_err(|e| Error::ConfigParse(format!("failed to read config: {e}")))?;
    validate_config(&raw).map(Some)
}

/// Validates raw TOML, checks for unknown keys, validates URLs.
pub fn validate_config(raw: &str) -> Result<Config> {
    // Parse as generic Value first for unknown-key detection
    let value: toml::Value = raw
        .parse()
        .map_err(|e: toml::de::Error| Error::ConfigParse(format!("invalid TOML: {e}")))?;

    check_unknown_keys(&value, "");

    // Parse into typed struct
    let file: ConfigFile =
        toml::from_str(raw).map_err(|e| Error::ConfigParse(format!("invalid config: {e}")))?;

    let config = match file.network {
        Some(net) => {
            // Validate URL fields
            if let Some(ref u) = net.rpc_url {
                validate_url(u, "network.rpc_url")?;
            }
            if let Some(ref u) = net.bundler_url {
                validate_url(u, "network.bundler_url")?;
            }
            if let Some(ref u) = net.paymaster_url {
                validate_url(u, "network.paymaster_url")?;
            }
            Config {
                rpc_url: net.rpc_url,
                bundler_url: net.bundler_url,
                paymaster_url: net.paymaster_url,
                paymaster_policy_id: net.paymaster_policy_id,
            }
        }
        None => Config::default(),
    };

    Ok(config)
}

fn validate_url(u: &str, field: &str) -> Result<()> {
    let parsed: url::Url = u
        .parse()
        .map_err(|e: url::ParseError| Error::ConfigParse(format!("{field}: invalid URL: {e}")))?;
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return Err(Error::ConfigParse(format!(
            "{field}: URL scheme must be http or https, got '{}'",
            parsed.scheme()
        )));
    }
    Ok(())
}

fn check_unknown_keys(value: &toml::Value, prefix: &str) {
    if let toml::Value::Table(table) = value {
        for (key, val) in table {
            let full_key = if prefix.is_empty() {
                key.clone()
            } else {
                format!("{prefix}.{key}")
            };
            if !KNOWN_KEYS.contains(&full_key.as_str()) {
                eprintln!("Warning: unknown config key '{full_key}'");
            }
            if val.is_table() {
                check_unknown_keys(val, &full_key);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Value resolution (4-tier: CLI > env > config > error)
// ---------------------------------------------------------------------------

pub fn resolve_value(cli: Option<&str>, env_var: &str, config_val: Option<&str>) -> Option<String> {
    if let Some(v) = cli {
        tracing::debug!("{env_var} resolved from CLI flag");
        return Some(v.to_string());
    }
    if let Ok(v) = std::env::var(env_var) {
        if !v.is_empty() {
            tracing::debug!("{env_var} resolved from env var");
            return Some(v);
        }
    }
    if let Some(v) = config_val {
        tracing::debug!("{env_var} resolved from config file");
        return Some(v.to_string());
    }
    None
}

pub fn resolve_rpc(cli: Option<&str>, config: &Option<Config>) -> Result<String> {
    let config_val = config.as_ref().and_then(|c| c.rpc_url.as_deref());
    resolve_value(cli, "KEYPO_RPC_URL", config_val)
        .ok_or_else(|| Error::ConfigMissing("rpc_url — pass --rpc, set KEYPO_RPC_URL, or add network.rpc_url to ~/.keypo/config.toml".into()))
}

pub fn resolve_bundler(cli: Option<&str>, config: &Option<Config>) -> Result<String> {
    let config_val = config.as_ref().and_then(|c| c.bundler_url.as_deref());
    resolve_value(cli, "KEYPO_BUNDLER_URL", config_val)
        .ok_or_else(|| Error::ConfigMissing("bundler_url — pass --bundler, set KEYPO_BUNDLER_URL, or add network.bundler_url to ~/.keypo/config.toml".into()))
}

pub fn resolve_paymaster(
    cli: Option<&str>,
    no_paymaster: bool,
    config: &Option<Config>,
) -> Option<String> {
    if no_paymaster {
        return None;
    }
    let config_val = config.as_ref().and_then(|c| c.paymaster_url.as_deref());
    resolve_value(cli, "KEYPO_PAYMASTER_URL", config_val)
}

pub fn resolve_paymaster_policy(cli: Option<&str>, config: &Option<Config>) -> Option<String> {
    let config_val = config
        .as_ref()
        .and_then(|c| c.paymaster_policy_id.as_deref());
    resolve_value(cli, "KEYPO_PAYMASTER_POLICY_ID", config_val)
}

// ---------------------------------------------------------------------------
// Config persistence
// ---------------------------------------------------------------------------

pub fn save_config(config: &Config) -> Result<()> {
    save_config_at(config, &config_path()?)
}

pub fn save_config_at(config: &Config, path: &Path) -> Result<()> {
    let file = ConfigFile {
        network: Some(NetworkConfig {
            rpc_url: config.rpc_url.clone(),
            bundler_url: config.bundler_url.clone(),
            paymaster_url: config.paymaster_url.clone(),
            paymaster_policy_id: config.paymaster_policy_id.clone(),
        }),
    };

    let toml_str =
        toml::to_string_pretty(&file).map_err(|e| Error::Other(format!("TOML serialize: {e}")))?;

    // Ensure directory exists
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

    // Atomic write: tmp + rename
    let tmp = path.with_extension("toml.tmp");
    std::fs::write(&tmp, &toml_str)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// config set
// ---------------------------------------------------------------------------

const SETTABLE_KEYS: &[&str] = &[
    "network.rpc_url",
    "network.bundler_url",
    "network.paymaster_url",
    "network.paymaster_policy_id",
];

const URL_KEYS: &[&str] = &[
    "network.rpc_url",
    "network.bundler_url",
    "network.paymaster_url",
];

pub fn set_config_value(key: &str, value: &str) -> Result<()> {
    set_config_value_at(key, value, &config_path()?)
}

pub fn set_config_value_at(key: &str, value: &str, path: &Path) -> Result<()> {
    if !SETTABLE_KEYS.contains(&key) {
        return Err(Error::ConfigParse(format!(
            "unknown config key: '{key}'. Valid keys: {}",
            SETTABLE_KEYS.join(", ")
        )));
    }

    if URL_KEYS.contains(&key) {
        validate_url(value, key)?;
    }

    let mut config = load_config_at(path)?.unwrap_or_default();

    match key {
        "network.rpc_url" => config.rpc_url = Some(value.to_string()),
        "network.bundler_url" => config.bundler_url = Some(value.to_string()),
        "network.paymaster_url" => config.paymaster_url = Some(value.to_string()),
        "network.paymaster_policy_id" => config.paymaster_policy_id = Some(value.to_string()),
        _ => unreachable!(),
    }

    save_config_at(&config, path)
}

// ---------------------------------------------------------------------------
// Redaction + display
// ---------------------------------------------------------------------------

pub fn redact_url(url: &str) -> String {
    if let Ok(mut parsed) = url::Url::parse(url) {
        let has_sensitive = parsed.query_pairs().any(|(k, _)| {
            let kl = k.to_lowercase();
            kl.contains("key") || kl.contains("secret") || kl.contains("token")
        });
        if has_sensitive {
            let redacted_pairs: Vec<(String, String)> = parsed
                .query_pairs()
                .map(|(k, v)| {
                    let kl = k.to_lowercase();
                    if kl.contains("key") || kl.contains("secret") || kl.contains("token") {
                        (k.into_owned(), "***".to_string())
                    } else {
                        (k.into_owned(), v.into_owned())
                    }
                })
                .collect();
            parsed.query_pairs_mut().clear();
            for (k, v) in &redacted_pairs {
                parsed.query_pairs_mut().append_pair(k, v);
            }
            return parsed.to_string();
        }
    }
    url.to_string()
}

fn format_value_with_source(
    label: &str,
    config_val: Option<&str>,
    env_var: &str,
    reveal: bool,
) -> String {
    let env_val = std::env::var(env_var).ok().filter(|v| !v.is_empty());

    let (display_val, source) = if let Some(ref ev) = env_val {
        (ev.as_str(), format!(" (env: {env_var})"))
    } else if let Some(cv) = config_val {
        (cv, String::new())
    } else {
        return format!("  {label}: (not set)\n");
    };

    let shown = if reveal {
        display_val.to_string()
    } else {
        redact_url(display_val)
    };

    format!("  {label}: {shown}{source}\n")
}

pub fn format_config_show(config: &Option<Config>, reveal: bool) -> String {
    let mut out = String::new();

    match config {
        Some(cfg) => {
            out.push_str("[network]\n");
            out.push_str(&format_value_with_source(
                "rpc_url",
                cfg.rpc_url.as_deref(),
                "KEYPO_RPC_URL",
                reveal,
            ));
            out.push_str(&format_value_with_source(
                "bundler_url",
                cfg.bundler_url.as_deref(),
                "KEYPO_BUNDLER_URL",
                reveal,
            ));
            out.push_str(&format_value_with_source(
                "paymaster_url",
                cfg.paymaster_url.as_deref(),
                "KEYPO_PAYMASTER_URL",
                reveal,
            ));
            out.push_str(&format_value_with_source(
                "paymaster_policy_id",
                cfg.paymaster_policy_id.as_deref(),
                "KEYPO_PAYMASTER_POLICY_ID",
                reveal,
            ));
        }
        None => {
            out.push_str("No config file found at ~/.keypo/config.toml\n");
            out.push_str("Run 'keypo-wallet init' to create one.\n");
        }
    }

    out
}

// ---------------------------------------------------------------------------
// Interactive init
// ---------------------------------------------------------------------------

pub fn run_init_interactive(
    reader: &mut impl BufRead,
    writer: &mut impl Write,
    config_path: &Path,
    overwrite: bool,
) -> Result<Config> {
    if config_path.exists() && !overwrite {
        write!(
            writer,
            "Config already exists at {}. Overwrite? [y/N] ",
            config_path.display()
        )
        .map_err(|e| Error::Other(format!("write error: {e}")))?;
        writer
            .flush()
            .map_err(|e| Error::Other(format!("flush error: {e}")))?;

        let mut answer = String::new();
        reader
            .read_line(&mut answer)
            .map_err(|e| Error::Other(format!("read error: {e}")))?;
        if !answer.trim().eq_ignore_ascii_case("y") {
            return Err(Error::Other("aborted".into()));
        }
    }

    // RPC URL
    write!(writer, "RPC URL [https://sepolia.base.org]: ")
        .map_err(|e| Error::Other(format!("write error: {e}")))?;
    writer
        .flush()
        .map_err(|e| Error::Other(format!("flush error: {e}")))?;
    let mut rpc = String::new();
    reader
        .read_line(&mut rpc)
        .map_err(|e| Error::Other(format!("read error: {e}")))?;
    let rpc = rpc.trim();
    let rpc_url = if rpc.is_empty() {
        "https://sepolia.base.org".to_string()
    } else {
        rpc.to_string()
    };

    // Bundler URL
    write!(writer, "Bundler URL (required): ")
        .map_err(|e| Error::Other(format!("write error: {e}")))?;
    writer
        .flush()
        .map_err(|e| Error::Other(format!("flush error: {e}")))?;
    let mut bundler = String::new();
    reader
        .read_line(&mut bundler)
        .map_err(|e| Error::Other(format!("read error: {e}")))?;
    let bundler = bundler.trim();
    if bundler.is_empty() {
        return Err(Error::ConfigMissing("bundler URL is required".into()));
    }
    let bundler_url = bundler.to_string();

    // Paymaster URL (optional)
    write!(writer, "Paymaster URL (optional, press Enter to skip): ")
        .map_err(|e| Error::Other(format!("write error: {e}")))?;
    writer
        .flush()
        .map_err(|e| Error::Other(format!("flush error: {e}")))?;
    let mut pm = String::new();
    reader
        .read_line(&mut pm)
        .map_err(|e| Error::Other(format!("read error: {e}")))?;
    let pm = pm.trim();
    let paymaster_url = if pm.is_empty() {
        None
    } else {
        Some(pm.to_string())
    };

    let config = Config {
        rpc_url: Some(rpc_url),
        bundler_url: Some(bundler_url),
        paymaster_url,
        paymaster_policy_id: None,
    };

    save_config_at(&config, config_path)?;

    writeln!(writer, "\nConfig saved to {}", config_path.display())
        .map_err(|e| Error::Other(format!("write error: {e}")))?;
    writeln!(writer, "Next: keypo-wallet setup --key <label>")
        .map_err(|e| Error::Other(format!("write error: {e}")))?;

    Ok(config)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn tmp_config(dir: &TempDir) -> PathBuf {
        dir.path().join("config.toml")
    }

    #[test]
    fn load_config_missing_file_returns_none() {
        let dir = TempDir::new().unwrap();
        let path = tmp_config(&dir);
        let result = load_config_at(&path).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn load_config_empty_file_returns_defaults() {
        let dir = TempDir::new().unwrap();
        let path = tmp_config(&dir);
        std::fs::write(&path, "").unwrap();
        let config = load_config_at(&path).unwrap().unwrap();
        assert!(config.rpc_url.is_none());
        assert!(config.bundler_url.is_none());
    }

    #[test]
    fn load_config_full_file_parses() {
        let dir = TempDir::new().unwrap();
        let path = tmp_config(&dir);
        std::fs::write(
            &path,
            r#"
[network]
rpc_url = "https://sepolia.base.org"
bundler_url = "https://bundler.example.com"
paymaster_url = "https://paymaster.example.com"
paymaster_policy_id = "sp_test"
"#,
        )
        .unwrap();
        let config = load_config_at(&path).unwrap().unwrap();
        assert_eq!(config.rpc_url.as_deref(), Some("https://sepolia.base.org"));
        assert_eq!(
            config.bundler_url.as_deref(),
            Some("https://bundler.example.com")
        );
        assert_eq!(
            config.paymaster_url.as_deref(),
            Some("https://paymaster.example.com")
        );
        assert_eq!(config.paymaster_policy_id.as_deref(), Some("sp_test"));
    }

    #[test]
    fn load_config_malformed_toml_errors() {
        let dir = TempDir::new().unwrap();
        let path = tmp_config(&dir);
        std::fs::write(&path, "not valid toml [[[").unwrap();
        let result = load_config_at(&path);
        assert!(result.is_err());
        let err = format!("{}", result.unwrap_err());
        assert!(err.contains("invalid TOML"));
    }

    #[test]
    fn load_config_invalid_url_errors() {
        let dir = TempDir::new().unwrap();
        let path = tmp_config(&dir);
        std::fs::write(
            &path,
            r#"
[network]
rpc_url = "not-a-url"
"#,
        )
        .unwrap();
        let result = load_config_at(&path);
        assert!(result.is_err());
        let err = format!("{}", result.unwrap_err());
        assert!(err.contains("invalid URL"));
    }

    #[test]
    fn load_config_non_http_url_errors() {
        let dir = TempDir::new().unwrap();
        let path = tmp_config(&dir);
        std::fs::write(
            &path,
            r#"
[network]
rpc_url = "ftp://example.com"
"#,
        )
        .unwrap();
        let result = load_config_at(&path);
        assert!(result.is_err());
        let err = format!("{}", result.unwrap_err());
        assert!(err.contains("http or https"));
    }

    #[test]
    fn resolve_value_cli_wins() {
        let result = resolve_value(Some("from-cli"), "KEYPO_TEST_UNUSED", Some("from-config"));
        assert_eq!(result, Some("from-cli".into()));
    }

    #[test]
    fn resolve_value_env_wins_over_config() {
        std::env::set_var("KEYPO_TEST_RESOLVE_ENV", "from-env");
        let result = resolve_value(None, "KEYPO_TEST_RESOLVE_ENV", Some("from-config"));
        assert_eq!(result, Some("from-env".into()));
        std::env::remove_var("KEYPO_TEST_RESOLVE_ENV");
    }

    #[test]
    fn resolve_value_config_fallback() {
        std::env::remove_var("KEYPO_TEST_RESOLVE_NONE");
        let result = resolve_value(None, "KEYPO_TEST_RESOLVE_NONE", Some("from-config"));
        assert_eq!(result, Some("from-config".into()));
    }

    #[test]
    fn resolve_value_all_none() {
        std::env::remove_var("KEYPO_TEST_RESOLVE_ALL_NONE");
        let result = resolve_value(None, "KEYPO_TEST_RESOLVE_ALL_NONE", None);
        assert!(result.is_none());
    }

    #[test]
    fn resolve_rpc_required_errors_when_missing() {
        std::env::remove_var("KEYPO_RPC_URL");
        let result = resolve_rpc(None, &None);
        assert!(result.is_err());
        let err = format!("{}", result.unwrap_err());
        assert!(err.contains("rpc_url"));
    }

    #[test]
    fn resolve_bundler_required_errors_when_missing() {
        std::env::remove_var("KEYPO_BUNDLER_URL");
        let result = resolve_bundler(None, &None);
        assert!(result.is_err());
        let err = format!("{}", result.unwrap_err());
        assert!(err.contains("bundler_url"));
    }

    #[test]
    fn resolve_paymaster_no_paymaster_flag() {
        let config = Some(Config {
            paymaster_url: Some("https://pm.example.com".into()),
            ..Default::default()
        });
        let result = resolve_paymaster(None, true, &config);
        assert!(result.is_none());
    }

    #[test]
    fn resolve_paymaster_from_config() {
        std::env::remove_var("KEYPO_PAYMASTER_URL");
        let config = Some(Config {
            paymaster_url: Some("https://pm.example.com".into()),
            ..Default::default()
        });
        let result = resolve_paymaster(None, false, &config);
        assert_eq!(result, Some("https://pm.example.com".into()));
    }

    #[test]
    fn set_config_value_creates_file() {
        let dir = TempDir::new().unwrap();
        let path = tmp_config(&dir);
        set_config_value_at("network.rpc_url", "https://rpc.example.com", &path).unwrap();
        let config = load_config_at(&path).unwrap().unwrap();
        assert_eq!(config.rpc_url.as_deref(), Some("https://rpc.example.com"));
    }

    #[test]
    fn set_config_value_updates_existing() {
        let dir = TempDir::new().unwrap();
        let path = tmp_config(&dir);
        set_config_value_at("network.rpc_url", "https://old.example.com", &path).unwrap();
        set_config_value_at("network.rpc_url", "https://new.example.com", &path).unwrap();
        let config = load_config_at(&path).unwrap().unwrap();
        assert_eq!(config.rpc_url.as_deref(), Some("https://new.example.com"));
    }

    #[test]
    fn set_config_value_rejects_unknown_key() {
        let dir = TempDir::new().unwrap();
        let path = tmp_config(&dir);
        let result = set_config_value_at("network.foo", "bar", &path);
        assert!(result.is_err());
        let err = format!("{}", result.unwrap_err());
        assert!(err.contains("unknown config key"));
    }

    #[test]
    fn set_config_value_rejects_invalid_url() {
        let dir = TempDir::new().unwrap();
        let path = tmp_config(&dir);
        let result = set_config_value_at("network.rpc_url", "not-a-url", &path);
        assert!(result.is_err());
    }

    #[test]
    fn redact_url_apikey() {
        let url = "https://api.pimlico.io/v2/84532/rpc?apikey=secret123";
        let redacted = redact_url(url);
        assert!(redacted.contains("apikey=***"));
        assert!(!redacted.contains("secret123"));
    }

    #[test]
    fn redact_url_no_key_unchanged() {
        let url = "https://sepolia.base.org";
        assert_eq!(redact_url(url), url);
    }

    #[test]
    fn format_config_show_redacted() {
        let config = Some(Config {
            rpc_url: Some("https://api.example.com?apikey=secret".into()),
            bundler_url: Some("https://bundler.example.com".into()),
            paymaster_url: None,
            paymaster_policy_id: None,
        });
        let output = format_config_show(&config, false);
        assert!(output.contains("apikey=***"));
        assert!(!output.contains("secret"));
        assert!(output.contains("bundler.example.com"));
    }

    #[test]
    fn format_config_show_reveal() {
        let config = Some(Config {
            rpc_url: Some("https://api.example.com?apikey=secret".into()),
            ..Default::default()
        });
        let output = format_config_show(&config, true);
        assert!(output.contains("apikey=secret"));
    }

    #[test]
    fn format_config_show_no_config() {
        let output = format_config_show(&None, false);
        assert!(output.contains("No config file found"));
    }

    #[test]
    fn init_interactive_prompts() {
        let dir = TempDir::new().unwrap();
        let path = tmp_config(&dir);

        let input = b"https://rpc.example.com\nhttps://bundler.example.com\n\n";
        let mut reader = &input[..];
        let mut output = Vec::new();

        let config = run_init_interactive(&mut reader, &mut output, &path, false).unwrap();
        assert_eq!(config.rpc_url.as_deref(), Some("https://rpc.example.com"));
        assert_eq!(
            config.bundler_url.as_deref(),
            Some("https://bundler.example.com")
        );
        assert!(config.paymaster_url.is_none());

        // File should exist
        assert!(path.exists());
    }

    #[test]
    fn init_interactive_overwrite_declined() {
        let dir = TempDir::new().unwrap();
        let path = tmp_config(&dir);
        std::fs::write(&path, "existing").unwrap();

        let input = b"n\n";
        let mut reader = &input[..];
        let mut output = Vec::new();

        let result = run_init_interactive(&mut reader, &mut output, &path, false);
        assert!(result.is_err());
        // File should be unchanged
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "existing");
    }

    #[test]
    fn init_interactive_default_rpc() {
        let dir = TempDir::new().unwrap();
        let path = tmp_config(&dir);

        let input = b"\nhttps://bundler.example.com\n\n";
        let mut reader = &input[..];
        let mut output = Vec::new();

        let config = run_init_interactive(&mut reader, &mut output, &path, false).unwrap();
        assert_eq!(config.rpc_url.as_deref(), Some("https://sepolia.base.org"));
    }
}
