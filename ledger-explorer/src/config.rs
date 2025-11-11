use anyhow::{Context, Result};
use serde::Deserialize;
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub neo4j: Neo4jConfig,
    pub ledger: LedgerConfig,
}

#[derive(Debug, Deserialize)]
pub struct Neo4jConfig {
    pub uri: String,
    pub user: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct LedgerConfig {
    pub party: String,
    pub url: String,
}

pub fn read_config<P: AsRef<Path>>(path: P) -> Result<Config> {
    let s = fs::read_to_string(&path)
        .with_context(|| format!("failed to read config file '{}'", path.as_ref().display()))?;
    let cfg: Config = toml::from_str(&s).context("failed to parse TOML config")?;
    Ok(cfg)
}

pub fn read_config_from_toml() -> Result<Config> {
    let crate_root = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let cfg_path = std::path::PathBuf::from(&crate_root)
        .join("config")
        .join("config.toml")
        .canonicalize()
        .expect("Failed to canonicalize config path");

    read_config(&cfg_path)
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
        assert!(!cfg.ledger.party.is_empty());
        assert!(!cfg.ledger.url.is_empty());    
        Ok(())
    }
}
// #[tokio::main]
// async fn main() -> Result<()> {
//     let cfg_path = Path::new("demo-ledger-explorer/config/config.toml");
//     let config = read_config(cfg_path)?;

//     let party = config.ledger.party;
//     let url = config.ledger.url;

    
//     // let parties = vec![party];
//     // let update_stream = stream_updates(Some(&access_token), begin_exclusive, end_inclusive, parties, url).await?;
//     // let cypher_stream = update_stream.map(|update| {
//     //     cypher::get_updates_response_to_cypher(&update.unwrap())
//     // });
    

//     Ok(())
// }


