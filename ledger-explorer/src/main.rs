use clap::{Parser, Subcommand};
use tokio_stream::StreamExt; // for flat_map // Ensure StreamExt trait is in scope for flat_map
use ledger_explorer::graph::apply_cypher_vec_stream_to_neo4j;
use ledger_explorer::cypher;
use client::stream_updates::stream_updates;
use tracing::{info, debug, error};

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
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing subscriber with env filter (defaults to INFO, configurable via RUST_LOG)
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env()
            .add_directive(tracing::Level::INFO.into()))
        .init();

    let cli = Cli::parse();

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
        Commands::Sync { config_file } => {
            info!("Starting sync command");

            debug!("Reading configuration from TOML file");
            let config = match config_file {
                Some(path) => ledger_explorer::config::read_config(&path).expect("failed to read config from specified path"),
                None => ledger_explorer::config::read_config_from_toml().expect("failed to read config from toml"),
            };
            let reader_user = config.ledger.reader_user;
            let parties = config.ledger.parties.unwrap_or_default();
            let ledger_url = config.ledger.url;
            let neo4j_uri = config.neo4j.uri;
            let neo4j_user = config.neo4j.user;
            let neo4j_pass = config.neo4j.password;

            info!(
                ledger_url = %ledger_url,
                neo4j_uri = %neo4j_uri,
                parties = ?parties,
                "Configuration loaded"
            );
            info!("Obtaining JWT token for reader user: {}", reader_user);

            let token = client::jwt::fake_jwt_for_user(&reader_user);
            info!("JWT token obtained successfully");

            info!("Starting update stream from offset 0");
            let update_stream = stream_updates(Some(&token), 0, None, parties.clone(), ledger_url).await?;
            let cypher_stream = update_stream.map(|update| {
                match &update {
                    Ok(_) => debug!("Processing update from stream"),
                    Err(e) => error!(error = %e, "Error in update stream"),
                }
                cypher::get_updates_response_to_cypher(&update.unwrap())
            });

            info!("Applying cypher queries to Neo4j");
            let (before, after, update_time) = apply_cypher_vec_stream_to_neo4j(&neo4j_uri, &neo4j_user, &neo4j_pass, cypher_stream).await?;

            info!(
                before_offset = ?before,
                after_offset = ?after,
                update_time_ms = ?update_time,
                "Neo4j graph sync completed"
            );
        }
    }

    Ok(())
}
