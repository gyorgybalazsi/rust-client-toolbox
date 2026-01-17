use clap::{Parser, Subcommand};
use tokio_stream::StreamExt;
use ledger_explorer::cypher;
use ledger_explorer::sync::{run_resilient_sync, SyncConfig, BackoffConfig};
use client::jwt::TokenSource;
use client::stream_updates::stream_updates;
use tracing::{info, debug};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "ledger-explorer")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Print Cypher code generated from the event node stream
    PrintCypher {
        #[arg(long)]
        access_token: String,
        #[arg(long)]
        party: String,
        #[arg(long)]
        url: String,
        #[arg(long)]
        begin_exclusive: i64,
        #[arg(long)]
        end_inclusive: Option<i64>,
    },
    Sync {
        /// Path to config.toml file (defaults to ./config/config.toml or CARGO_MANIFEST_DIR/config/config.toml)
        #[arg(long)]
        config_file: Option<String>,
        /// Optional access token (if not provided, will try Keycloak config, then fall back to fake JWT)
        #[arg(long)]
        access_token: Option<String>,
        /// Use Keycloak to obtain a real JWT token (requires [keycloak] section in config)
        #[arg(long)]
        use_keycloak: bool,
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // Determine log level from config file if available (for Sync command), otherwise default to INFO
    let log_level = match &cli.command {
        Commands::Sync { config_file, .. } => {
            let config = match config_file {
                Some(path) => ledger_explorer::config::read_config(path).ok(),
                None => ledger_explorer::config::read_config_from_toml().ok(),
            };
            config.map(|c| c.logging.level).unwrap_or_else(|| "info".to_string())
        }
        _ => "info".to_string(),
    };

    // Initialize tracing subscriber with env filter (RUST_LOG takes precedence over config)
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(&log_level));
    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .init();

    match cli.command {
        Commands::PrintCypher { access_token, url, begin_exclusive, end_inclusive, party } => {
            let parties = vec![party];
            let mut update_stream = stream_updates(Some(&access_token), begin_exclusive, end_inclusive, parties, url).await?;
            while let Some(response) = update_stream.next().await {
                let cypher_queries = cypher::get_updates_response_to_cypher(&response?);
                println!("Start transaction");
                println!("{:?}", cypher_queries);
                println!("End transaction");
            }
        }
        Commands::Sync { config_file, access_token, use_keycloak } => {
            info!("Starting resilient sync command");

            debug!(config_path = ?config_file, "Reading configuration from TOML file");
            let config = match config_file {
                Some(path) => ledger_explorer::config::read_config(&path).expect("failed to read config from specified path"),
                None => ledger_explorer::config::read_config_from_toml().expect("failed to read config from toml"),
            };
            let fake_jwt_user = config.ledger.fake_jwt_user;
            let parties = config.ledger.parties.unwrap_or_default();
            let ledger_url = config.ledger.url;
            let neo4j_uri = config.neo4j.uri.clone();
            let neo4j_user = config.neo4j.user.clone();
            let neo4j_pass = config.neo4j.password.clone();
            let keycloak_config = config.keycloak;

            info!(
                ledger_url = %ledger_url,
                neo4j_uri = %neo4j_uri,
                parties = ?parties,
                "Configuration loaded"
            );

            // Determine token source for automatic renewal
            let token_source = match access_token {
                Some(token) => {
                    info!("Using provided static access token");
                    TokenSource::Static(token)
                }
                None if use_keycloak => {
                    let kc_config = keycloak_config
                        .expect("--use-keycloak requires [keycloak] section in config file");
                    info!("Using Keycloak for JWT token management at {}", kc_config.token_endpoint);
                    TokenSource::Keycloak(client::jwt::KeycloakConfig {
                        client_id: kc_config.client_id,
                        client_secret: kc_config.client_secret,
                        token_endpoint: kc_config.token_endpoint,
                    })
                }
                None => {
                    info!("Using fake JWT token for user: {}", fake_jwt_user);
                    TokenSource::FakeJwt(fake_jwt_user)
                }
            };

            let sync_config = SyncConfig {
                ledger_url,
                parties,
                neo4j_uri,
                neo4j_user,
                neo4j_pass,
            };

            info!("Starting resilient sync loop (will auto-reconnect on failures, resume from Neo4j checkpoint)");
            run_resilient_sync(sync_config, token_source, BackoffConfig::default()).await?;
        }
    }

    Ok(())
}
