use anyhow::{Context, Result};
use serde::Deserialize;
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    pub logging: LoggingConfig,
    pub neo4j: Neo4jConfig,
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
}

pub fn read_config<P: AsRef<Path>>(path: P) -> Result<AppConfig> {
    let s = fs::read_to_string(&path)
        .with_context(|| format!("failed to read config file '{}'", path.as_ref().display()))?;
    let cfg: AppConfig = toml::from_str(&s).context("failed to parse TOML config")?;
    Ok(cfg)
}

pub fn find_and_read_config() -> Result<AppConfig> {
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

    // Try parent directory (workspace root) config
    let parent_config = std::path::PathBuf::from("../ledger-explorer/config/config.toml");
    if parent_config.exists() {
        return read_config(&parent_config);
    }

    anyhow::bail!(
        "Could not find config.toml. Place it at ./config/config.toml or set CARGO_MANIFEST_DIR."
    )
}
