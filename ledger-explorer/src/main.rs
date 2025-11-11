use clap::{Parser, Subcommand};
use tokio_stream::StreamExt; // for flat_map // Ensure StreamExt trait is in scope for flat_map
use ledger_explorer::graph::apply_cypher_vec_stream_to_neo4j;
use ledger_explorer::cypher;
use client::stream_updates::stream_updates;
use ledger_api::v2::admin::user_management_service_client::UserManagementServiceClient;

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
    /// Update Neo4j graph from the event node stream
    SyncOld {
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
        #[arg(long, default_value = "neo4j://localhost:7687")]
        neo4j_uri: String,
        #[arg(long, default_value = "neo4j")]
        neo4j_user: String,
        #[arg(long, default_value = "password")]
        neo4j_pass: String,
    },
    Sync
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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
        Commands::SyncOld { access_token, url, begin_exclusive, end_inclusive, party, neo4j_uri, neo4j_user, neo4j_pass } => {
            let parties = vec![party];
            let update_stream = stream_updates(Some(&access_token), begin_exclusive, end_inclusive, parties, url).await?;
            let cypher_stream = update_stream.map(|update| {
                cypher::get_updates_response_to_cypher(&update.unwrap())
            });
            let (before, after, update_time) = apply_cypher_vec_stream_to_neo4j(&neo4j_uri, &neo4j_user, &neo4j_pass, cypher_stream).await?;
            println!("Neo4j graph updated from event stream. Before max offset: {:?}, After max offset: {:?}, Update time in millis: {:?}", before, after, update_time);
        }
        Commands::Sync => {
            let config = ledger_explorer::config::read_config_from_toml().expect("failed to read config from toml");
            let party = config.ledger.party;
            let ledger_url = config.ledger.url;
            let neo4j_uri = config.neo4j.uri;
            let neo4j_user = config.neo4j.user;
            let neo4j_pass = config.neo4j.password;

            let channel = tonic::transport::Channel::from_shared(ledger_url.clone())?
                .connect()
                .await?;
            let mut user_management_client = UserManagementServiceClient::new(channel);
            let token = client::jwt::fake_jwt(&mut user_management_client, &party).await?;

            let update_stream = stream_updates(Some(&token), 0, None, vec![party], ledger_url).await?;
            let cypher_stream = update_stream.map(|update| {
                cypher::get_updates_response_to_cypher(&update.unwrap())
            }); 

            let (before, after, update_time) = apply_cypher_vec_stream_to_neo4j(&neo4j_uri, &neo4j_user, &neo4j_pass, cypher_stream).await?;
            println!("Neo4j graph updated from event stream. Before max offset: {:?}, After max offset: {:?}, Update time in millis: {:?}", before, after, update_time);      
            
        }
    }

    Ok(())
}
