use ledger_api::v2::{
    EventFormat, GetUpdatesRequest, TransactionFormat, TransactionShape, UpdateFormat,
    update_service_client::UpdateServiceClient,
};
use crate::utils::build_filters_by_party;
use tonic::metadata::MetadataValue;
use anyhow::{Context, Result};

/// Streams ledger updates for the given parties, starting after `begin_exclusive` offset.
/// `end_inclusive` is optional; if set, the stream will end at that offset.
pub async fn stream_updates(
    access_token: Option<&str>,
    begin_exclusive: i64,
    end_inclusive: Option<i64>,
    parties: Vec<String>,
    url: String,
) -> Result<tonic::Streaming<ledger_api::v2::GetUpdatesResponse>> {
    let mut client = UpdateServiceClient::connect(url.clone())
        .await
        .with_context(|| format!("Failed to connect to update service at {}", url))?;

    let filters_by_party = build_filters_by_party(&parties);

    let event_format = EventFormat {
        filters_by_party,
        filters_for_any_party: None,
        verbose: true,
    };

    let transaction_format = TransactionFormat {
        event_format: Some(event_format),
        transaction_shape: TransactionShape::LedgerEffects as i32,
    };

    let update_format = UpdateFormat {
        include_transactions: Some(transaction_format),
        include_reassignments: None,
        include_topology_events: None,
    };

    let request = GetUpdatesRequest {
        begin_exclusive,
        end_inclusive,
        update_format: Some(update_format),
        ..Default::default()
    };

    let mut req = tonic::Request::new(request);
    if let Some(token) = access_token {
        let meta = MetadataValue::try_from(format!("Bearer {}", token))
            .with_context(|| "Failed to parse access token for metadata")?;
        req.metadata_mut().insert("authorization", meta);
    }

    let response = client.get_updates(req)
        .await
        .with_context(|| "Failed to get updates from ledger")?;

    Ok(response.into_inner())
}



