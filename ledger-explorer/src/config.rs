use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Top-level config file structure with profile support
#[derive(Debug, Deserialize)]
pub struct ConfigFile {
    pub logging: LoggingConfig,
    pub neo4j: Neo4jConfig,
    /// The active profile name (can be overridden via CLI)
    pub active_profile: String,
    /// Named profiles containing ledger and keycloak settings
    pub profiles: HashMap<String, ProfileConfig>,
}

/// A named profile containing environment-specific settings
#[derive(Debug, Deserialize, Clone)]
pub struct ProfileConfig {
    pub ledger: LedgerConfig,
    pub keycloak: Option<KeycloakConfig>,
}

/// Resolved config after selecting a profile
#[derive(Debug)]
pub struct Config {
    pub logging: LoggingConfig,
    pub neo4j: Neo4jConfig,
    pub ledger: LedgerConfig,
    pub keycloak: Option<KeycloakConfig>,
}

/// Authentication method for Keycloak
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "grant_type", rename_all = "snake_case")]
pub enum KeycloakAuthMethod {
    /// OAuth2 Client Credentials flow (service accounts)
    ClientCredentials {
        client_secret: String,
    },
    /// OAuth2 Resource Owner Password Credentials flow (user authentication)
    Password {
        username: String,
        password: String,
        #[serde(default)]
        client_secret: Option<String>,
    },
}

/// Keycloak OAuth2 configuration
#[derive(Debug, Deserialize, Clone)]
pub struct KeycloakConfig {
    pub client_id: String,
    pub token_endpoint: String,
    #[serde(flatten)]
    pub auth_method: KeycloakAuthMethod,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LoggingConfig {
    #[serde(default = "default_log_level")]
    pub level: String,
}

fn default_log_level() -> String {
    "info".to_string()
}

#[derive(Debug, Deserialize, Clone)]
pub struct Neo4jConfig {
    pub uri: String,
    pub user: String,
    pub password: String,
    /// Number of updates to batch before committing to Neo4j
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,
    /// Flush timeout in seconds - commit even if batch isn't full after this duration
    #[serde(default = "default_flush_timeout")]
    pub flush_timeout_secs: u64,
}

fn default_batch_size() -> usize {
    100
}

fn default_flush_timeout() -> u64 {
    1
}

#[derive(Debug, Deserialize, Clone)]
pub struct LedgerConfig {
    pub fake_jwt_user: String,
    pub parties: Option<Vec<String>>,
    pub url: String,
    /// Starting offset for sync when Neo4j has no data.
    /// If not specified, falls back to ledger pruning offset.
    pub starting_offset: Option<i64>,
}

/// Read and parse the config file
pub fn read_config_file<P: AsRef<Path>>(path: P) -> Result<ConfigFile> {
    let s = fs::read_to_string(&path)
        .with_context(|| format!("failed to read config file '{}'", path.as_ref().display()))?;
    let cfg: ConfigFile = toml::from_str(&s).context("failed to parse TOML config")?;
    Ok(cfg)
}

/// Read config file and resolve with the specified profile (or active_profile if None)
pub fn read_config<P: AsRef<Path>>(path: P, profile: Option<&str>) -> Result<Config> {
    let config_file = read_config_file(&path)?;
    resolve_config(config_file, profile)
}

/// Resolve a ConfigFile into a Config using the specified profile
pub fn resolve_config(config_file: ConfigFile, profile_override: Option<&str>) -> Result<Config> {
    let profile_name = profile_override.unwrap_or(&config_file.active_profile);

    let profile = config_file.profiles.get(profile_name)
        .with_context(|| format!(
            "profile '{}' not found. Available profiles: {:?}",
            profile_name,
            config_file.profiles.keys().collect::<Vec<_>>()
        ))?;

    Ok(Config {
        logging: config_file.logging,
        neo4j: config_file.neo4j,
        ledger: profile.ledger.clone(),
        keycloak: profile.keycloak.clone(),
    })
}

pub fn read_config_from_toml(profile: Option<&str>) -> Result<Config> {
    // Try multiple locations for config.toml:
    // 1. ./config/config.toml (relative to current working directory)
    // 2. CARGO_MANIFEST_DIR/config/config.toml (for cargo run)

    let cwd_config = std::path::PathBuf::from("config").join("config.toml");
    if cwd_config.exists() {
        return read_config(&cwd_config, profile);
    }

    if let Ok(crate_root) = std::env::var("CARGO_MANIFEST_DIR") {
        let cargo_config = std::path::PathBuf::from(&crate_root)
            .join("config")
            .join("config.toml");
        if cargo_config.exists() {
            return read_config(&cargo_config, profile);
        }
    }

    anyhow::bail!("Could not find config.toml in ./config/config.toml or CARGO_MANIFEST_DIR/config/config.toml. Use --config-file to specify a path.")
}

/// Helper to get available profile names from a config file
pub fn list_profiles<P: AsRef<Path>>(path: P) -> Result<Vec<String>> {
    let config_file = read_config_file(path)?;
    Ok(config_file.profiles.keys().cloned().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_config_from_toml_and_print() -> Result<()> {
        let cfg = read_config_from_toml(None).expect("failed to read config from toml");
        println!("Parsed config: {:#?}", cfg);
        assert!(!cfg.neo4j.uri.is_empty());
        assert!(!cfg.neo4j.user.is_empty());
        assert!(!cfg.neo4j.password.is_empty());
        assert!(!cfg.ledger.fake_jwt_user.is_empty());
        assert!(!cfg.ledger.url.is_empty());
        Ok(())
    }
}
