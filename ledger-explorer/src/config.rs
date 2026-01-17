use anyhow::{Context, Result};
use serde::Deserialize;
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub logging: LoggingConfig,
    pub neo4j: Neo4jConfig,
    pub ledger: LedgerConfig,
    /// Optional Keycloak configuration for obtaining real JWT tokens
    pub keycloak: Option<KeycloakConfig>,
}

/// Keycloak OAuth2 configuration for client credentials flow
#[derive(Debug, Deserialize, Clone)]
pub struct KeycloakConfig {
    pub client_id: String,
    pub client_secret: String,
    pub token_endpoint: String,
}

#[derive(Debug, Deserialize)]
pub struct LoggingConfig {
    #[serde(default = "default_log_level")]
    pub level: String,
}

fn default_log_level() -> String {
    "info".to_string()
}

#[derive(Debug, Deserialize)]
pub struct Neo4jConfig {
    pub uri: String,
    pub user: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct LedgerConfig {
    pub fake_jwt_user: String,
    pub parties: Option<Vec<String>>,
    pub url: String,
    /// Starting offset for streaming updates (default: 0)
    #[serde(default)]
    pub begin_offset: i64,
}

pub fn read_config<P: AsRef<Path>>(path: P) -> Result<Config> {
    let s = fs::read_to_string(&path)
        .with_context(|| format!("failed to read config file '{}'", path.as_ref().display()))?;
    let cfg: Config = toml::from_str(&s).context("failed to parse TOML config")?;
    Ok(cfg)
}

pub fn read_config_from_toml() -> Result<Config> {
    // Try multiple locations for config.toml:
    // 1. ./config/config.toml (relative to current working directory)
    // 2. CARGO_MANIFEST_DIR/config/config.toml (for cargo run)

    let cwd_config = std::path::PathBuf::from("config").join("config.toml");
    if cwd_config.exists() {
        return read_config(&cwd_config);
    }

    if let Ok(crate_root) = std::env::var("CARGO_MANIFEST_DIR") {
        let cargo_config = std::path::PathBuf::from(&crate_root)
            .join("config")
            .join("config.toml");
        if cargo_config.exists() {
            return read_config(&cargo_config);
        }
    }

    anyhow::bail!("Could not find config.toml in ./config/config.toml or CARGO_MANIFEST_DIR/config/config.toml. Use --config-file to specify a path.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_config_from_toml_and_print()  -> Result<()>{
        let cfg = read_config_from_toml().expect("failed to read config from toml");
        println!("Parsed config: {:#?}", cfg);
        assert!(!cfg.neo4j.uri.is_empty());
        assert!(!cfg.neo4j.user.is_empty());
        assert!(!cfg.neo4j.password.is_empty());
        assert!(!cfg.ledger.fake_jwt_user.is_empty());
        assert!(!cfg.ledger.url.is_empty());    
        Ok(())
    }
}



