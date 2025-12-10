use clap::{Parser, Subcommand};

use client::jwt::fake_jwt;
use client::ledger_end::get_ledger_end;
use client::stream_updates::stream_updates;

use futures_util::StreamExt;
use ledger_api::v2::admin::user_management_service_client::UserManagementServiceClient;
use tracing::{info, debug};
use tracing_subscriber;
use anyhow::Result;

#[derive(Parser, Debug)]
#[command(name = "app")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Get the ledger end
    GetLedgerEnd {
        #[arg(long)]
        url: String,
        #[arg(long)]
        access_token: String,
    },
    /// Create fake access token for Sandbox
    FakeAccessToken {
        #[arg(long)]
        url: String,
        #[arg(long)]
        party: String,
    },
    /// Stream ledger updates for a party
    StreamUpdates {
        #[arg(long)]
        url: String,
        #[arg(long)]
        access_token: String,
        #[arg(long)]
        party: String,
        #[arg(long)]
        begin_exclusive: i64,
        #[arg(long)]
        end_inclusive: Option<i64>,
    },
    /// Stream transactions for a party
    StreamTransactions {
        #[arg(long)]
        url: String,
        #[arg(long)]
        access_token: String,
        #[arg(long)]
        party: String,
        #[arg(long)]
        begin_exclusive: i64,
        #[arg(long)]
        end_inclusive: Option<i64>,
    },
    /// Get parties, optionally filtered by a substring
    Parties {
        #[arg(long)]
        url: String,
        #[arg(long)]
        access_token: String,
        #[arg(long)]
        filter: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stdout)
        .init();
    let cli = Cli::parse();

    match cli.command {
        Commands::GetLedgerEnd { url, access_token } => {
            let ledger_end = get_ledger_end(&url, Some(&access_token)).await?;
            info!("Ledger end: {}", ledger_end);
            Ok(())
        }
        Commands::FakeAccessToken { url, party } => {
            let channel = tonic::transport::Channel::from_shared(url)?
                .connect()
                .await?;
            let mut user_management_client = UserManagementServiceClient::new(channel);
            let token = fake_jwt(&mut user_management_client, &party).await?;
            info!("Fake access token: {}", token);
            Ok(())
        }
        Commands::StreamUpdates {
            access_token,
            party,
            url,
            begin_exclusive,
            end_inclusive,
        } => {
            info!(
                "StreamUpdates called with begin_exclusive: {}, end_inclusive: {:?}, party: {:?}, url: {}",
                begin_exclusive, end_inclusive, party, url
            );
            let mut stream = stream_updates(
                Some(&access_token),
                begin_exclusive,
                end_inclusive,
                vec![party],
                url,
            )
            .await?;
            while let Some(update) = stream.next().await {
                info!("{:#?}", update);
            }
            Ok(())
        }
        Commands::StreamTransactions {
            access_token,
            party,
            url,
            begin_exclusive,
            end_inclusive,
        } => {
            info!(
                "StreamTransactions called with begin_exclusive: {}, end_inclusive: {:?}, party: {:?}, url: {}",
                begin_exclusive, end_inclusive, party, url
            );
            let mut stream = stream_updates(
                Some(&access_token),
                begin_exclusive,
                end_inclusive,
                vec![party],
                url,
            )
            .await?;
            while let Some(Ok(response)) = stream.next().await {
                if let ledger_api::v2::get_updates_response::Update::Transaction(tx) =
                    &response.update.unwrap()
                {
                    info!("Transaction events: {:#?}", tx.events);
                    debug!(
                        "Structure markers: {:#?}",
                        client::utils::structure_markers_from_transaction(tx)
                    );
                }
            }
            Ok(())
        }
        Commands::Parties { filter, url, access_token } => {
            let parties = client::party_management::get_parties::get_parties(url, Some(&access_token), filter).await?;
            if parties.is_empty() {
                info!("No parties found.");
            } else {
                info!("Known parties: {:?}", parties);
            }
            Ok(())
        }
        
    }
}
