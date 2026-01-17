use anyhow::{Context, Result};
use async_stream::stream;
use futures::Stream;
use ledger_api::v2::{
    state_service_client::StateServiceClient, CreatedEvent, EventFormat,
    GetActiveContractsRequest,
};
use std::collections::HashMap;
use std::pin::Pin;
use tonic::metadata::MetadataValue;
use tracing::{debug, info};

use crate::utils::build_filters_by_party;

/// Represents an active contract from the ACS snapshot.
#[derive(Debug, Clone)]
pub struct ActiveContract {
    pub created_event: CreatedEvent,
    pub synchronizer_id: String,
}

/// Streams all active contracts for the given parties at a specific offset.
///
/// # Arguments
/// * `access_token` - Optional bearer token for authentication
/// * `active_at_offset` - The offset at which to query the ACS
/// * `parties` - The parties whose visibility to use for querying
/// * `url` - The gRPC endpoint URL of the ledger API
///
/// Returns a stream of ActiveContract items.
pub async fn stream_active_contracts(
    access_token: Option<&str>,
    active_at_offset: i64,
    parties: Vec<String>,
    url: String,
) -> Result<Pin<Box<dyn Stream<Item = Result<ActiveContract>> + Send>>> {
    info!(
        "Starting stream_active_contracts: url={}, parties={:?}, active_at_offset={}",
        url, parties, active_at_offset
    );

    debug!("Connecting to state service at {}", url);
    let mut client = StateServiceClient::connect(url.clone())
        .await
        .with_context(|| format!("Failed to connect to state service at {}", url))?;

    let filters_by_party: HashMap<String, ledger_api::v2::Filters> = build_filters_by_party(&parties);
    debug!("Built filters_by_party: {:?}", filters_by_party);

    let event_format = EventFormat {
        filters_by_party,
        filters_for_any_party: None,
        verbose: true,
    };

    let request = GetActiveContractsRequest {
        filter: None,
        verbose: false,
        active_at_offset,
        event_format: Some(event_format),
    };
    debug!("Created GetActiveContractsRequest: {:?}", request);

    let mut req = tonic::Request::new(request);
    if let Some(token) = access_token {
        debug!("Adding authorization token to request");
        let meta = MetadataValue::try_from(format!("Bearer {}", token))
            .with_context(|| "Failed to parse access token for metadata")?;
        req.metadata_mut().insert("authorization", meta);
    }

    debug!("Sending get_active_contracts request");
    let response = client
        .get_active_contracts(req)
        .await
        .with_context(|| "Failed to get active contracts from ledger")?;

    let mut grpc_stream = response.into_inner();

    let output_stream = stream! {
        while let Some(resp) = grpc_stream
            .message()
            .await
            .transpose()
        {
            match resp {
                Ok(resp) => {
                    if let Some(contract_entry) = resp.contract_entry {
                        match contract_entry {
                            ledger_api::v2::get_active_contracts_response::ContractEntry::ActiveContract(
                                active_contract,
                            ) => {
                                if let Some(created_event) = active_contract.created_event {
                                    debug!("Found active contract: {}", created_event.contract_id);
                                    yield Ok(ActiveContract {
                                        created_event,
                                        synchronizer_id: active_contract.synchronizer_id,
                                    });
                                }
                            }
                            ledger_api::v2::get_active_contracts_response::ContractEntry::IncompleteUnassigned(
                                incomplete,
                            ) => {
                                if let Some(created_event) = incomplete.created_event {
                                    debug!(
                                        "Found contract in incomplete unassigned: {}",
                                        created_event.contract_id
                                    );
                                    let synchronizer_id = incomplete
                                        .unassigned_event
                                        .map(|e| e.source)
                                        .unwrap_or_default();
                                    yield Ok(ActiveContract {
                                        created_event,
                                        synchronizer_id,
                                    });
                                }
                            }
                            ledger_api::v2::get_active_contracts_response::ContractEntry::IncompleteAssigned(
                                incomplete,
                            ) => {
                                if let Some(assigned_event) = incomplete.assigned_event {
                                    if let Some(created_event) = assigned_event.created_event {
                                        debug!(
                                            "Found contract in incomplete assigned: {}",
                                            created_event.contract_id
                                        );
                                        yield Ok(ActiveContract {
                                            created_event,
                                            synchronizer_id: assigned_event.target,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    yield Err(anyhow::anyhow!("Error reading from active contracts stream: {}", e));
                }
            }
        }
    };

    Ok(Box::pin(output_stream))
}
