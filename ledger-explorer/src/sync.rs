use std::sync::Arc;
use std::time::Duration;
use tokio_stream::StreamExt;
use tracing::{debug, error, info, warn};
use anyhow::Result;
use neo4rs::{Graph, query};
use std::time::Instant;

use client::jwt::{TokenManager, TokenSource};
use client::stream_updates::stream_updates;
use client::active_contracts::stream_active_contracts;
use client::ledger_end::{get_pruning_offset, get_ledger_end};
use crate::cypher;
use crate::graph::{apply_cypher_vec_stream_to_neo4j, get_last_processed_offset};

/// Configuration for the resilient sync process
pub struct SyncConfig {
    pub ledger_url: String,
    pub parties: Vec<String>,
    pub neo4j_uri: String,
    pub neo4j_user: String,
    pub neo4j_pass: String,
    /// Starting offset when Neo4j has no data. If None, falls back to pruning offset.
    pub starting_offset: Option<i64>,
}

/// Exponential backoff configuration
pub struct BackoffConfig {
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub multiplier: f64,
}

impl Default for BackoffConfig {
    fn default() -> Self {
        Self {
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(60),
            multiplier: 2.0,
        }
    }
}

/// Ensures required indexes exist in Neo4j for optimal query performance.
/// Creates indexes if they don't exist (idempotent).
async fn ensure_indexes(neo4j_uri: &str, neo4j_user: &str, neo4j_pass: &str) -> Result<()> {
    info!("Ensuring Neo4j indexes exist...");
    let graph = Graph::new(neo4j_uri, neo4j_user, neo4j_pass)?;

    let indexes = [
        "CREATE INDEX created_contract_id IF NOT EXISTS FOR (c:Created) ON (c.contract_id)",
        "CREATE INDEX created_offset_node IF NOT EXISTS FOR (c:Created) ON (c.offset, c.node_id)",
        "CREATE INDEX created_offset IF NOT EXISTS FOR (c:Created) ON (c.offset)",
        "CREATE INDEX created_template_name IF NOT EXISTS FOR (c:Created) ON (c.template_name)",
        "CREATE INDEX exercised_offset_node IF NOT EXISTS FOR (e:Exercised) ON (e.offset, e.node_id)",
        "CREATE INDEX exercised_choice_name IF NOT EXISTS FOR (e:Exercised) ON (e.choice_name)",
        "CREATE INDEX transaction_offset IF NOT EXISTS FOR (t:Transaction) ON (t.offset)",
        "CREATE INDEX party_id IF NOT EXISTS FOR (p:Party) ON (p.party_id)",
    ];

    for index_query in &indexes {
        match graph.run(query(*index_query)).await {
            Ok(_) => debug!("Index ensured: {}", index_query),
            Err(e) => warn!("Failed to create index (may already exist): {} - {}", index_query, e),
        }
    }

    info!("Neo4j indexes ready");
    Ok(())
}

/// Loads the Active Contract Set (ACS) into Neo4j at a specific offset.
/// This ensures we have all contracts that were active at that offset before starting to stream updates.
///
/// The offset should be the pruning offset (or start offset for streaming) so that all contracts
/// that will be archived in the stream already exist as Created nodes.
async fn load_acs_to_neo4j(
    ledger_url: &str,
    neo4j_uri: &str,
    neo4j_user: &str,
    neo4j_pass: &str,
    parties: &[String],
    token: &str,
    acs_offset: i64,
) -> Result<()> {
    info!("Loading Active Contract Set (ACS) into Neo4j at offset {}...", acs_offset);
    let start_time = Instant::now();

    // Connect to Neo4j
    let graph = Graph::new(neo4j_uri, neo4j_user, neo4j_pass)?;

    // Stream active contracts at the specified offset
    let mut acs_stream = stream_active_contracts(
        Some(token),
        acs_offset,
        parties.to_vec(),
        ledger_url.to_string(),
    ).await?;

    let mut contract_count = 0u64;
    let mut batch_queries = Vec::new();
    const BATCH_SIZE: usize = 500;

    while let Some(contract_result) = acs_stream.next().await {
        match contract_result {
            Ok(contract) => {
                let queries = cypher::created_event_to_cypher(&contract.created_event);
                batch_queries.extend(queries.into_iter().map(|cq| cq.query));
                contract_count += 1;

                // Commit in batches
                if batch_queries.len() >= BATCH_SIZE {
                    let mut txn = graph.start_txn().await?;
                    let queries_to_run: Vec<neo4rs::Query> = batch_queries.drain(..).collect();
                    txn.run_queries(queries_to_run).await?;
                    txn.commit().await?;
                    debug!("Committed batch of ACS contracts, total so far: {}", contract_count);
                }
            }
            Err(e) => {
                error!("Error streaming ACS contract: {}", e);
                return Err(e);
            }
        }
    }

    // Commit any remaining queries
    if !batch_queries.is_empty() {
        let mut txn = graph.start_txn().await?;
        txn.run_queries(batch_queries).await?;
        txn.commit().await?;
    }

    let elapsed = start_time.elapsed();
    info!(
        "ACS loading complete: {} contracts loaded in {:.2}s at offset {}",
        contract_count,
        elapsed.as_secs_f64(),
        acs_offset
    );

    Ok(())
}

/// Checks if the ACS has already been loaded into Neo4j.
/// We use the presence of any from_acs=true nodes as an indicator.
async fn is_acs_loaded(neo4j_uri: &str, neo4j_user: &str, neo4j_pass: &str) -> Result<bool> {
    let graph = Graph::new(neo4j_uri, neo4j_user, neo4j_pass)?;
    let mut result = graph.execute(query("MATCH (c:Created {from_acs: true}) RETURN count(c) as count LIMIT 1")).await?;

    match result.next().await {
        Ok(Some(row)) => {
            let count: i64 = row.get("count")?;
            Ok(count > 0)
        }
        Ok(None) => Ok(false),
        Err(e) => Err(e.into()),
    }
}

/// Clears all data from Neo4j database.
async fn clear_neo4j_database(neo4j_uri: &str, neo4j_user: &str, neo4j_pass: &str) -> Result<()> {
    info!("Clearing Neo4j database...");
    let graph = Graph::new(neo4j_uri, neo4j_user, neo4j_pass)?;

    // Use APOC for efficient deletion if available, otherwise fall back to batched delete
    let delete_result = graph.run(query("CALL apoc.periodic.iterate('MATCH (n) RETURN n', 'DETACH DELETE n', {batchSize: 10000})")).await;

    match delete_result {
        Ok(_) => {
            info!("Database cleared using APOC");
        }
        Err(_) => {
            // Fall back to regular delete (may be slow for large datasets)
            warn!("APOC not available, using standard delete (may be slow)");
            graph.run(query("MATCH (n) DETACH DELETE n")).await?;
            info!("Database cleared using standard delete");
        }
    }

    Ok(())
}

/// Runs the sync process with automatic reconnection and token refresh.
///
/// This function will:
/// 1. Load the Active Contract Set (ACS) if not already loaded
/// 2. Query Neo4j for the last processed offset (resume point)
/// 3. Start streaming from that offset
/// 4. On stream errors, reconnect with exponential backoff
/// 5. Proactively refresh JWT tokens before they expire
///
/// If `fresh` is true, clears the database and starts from current ledger end.
pub async fn run_resilient_sync(
    sync_config: SyncConfig,
    token_source: TokenSource,
    backoff_config: BackoffConfig,
    fresh: bool,
) -> Result<()> {
    // If fresh start, clear the database first
    if fresh {
        clear_neo4j_database(&sync_config.neo4j_uri, &sync_config.neo4j_user, &sync_config.neo4j_pass).await?;
    }

    // Ensure indexes exist before starting sync
    ensure_indexes(&sync_config.neo4j_uri, &sync_config.neo4j_user, &sync_config.neo4j_pass).await?;

    let token_manager = Arc::new(TokenManager::new(token_source));

    // Start background token refresh
    let token_manager_clone = Arc::clone(&token_manager);
    let _refresh_handle = token_manager_clone.start_background_refresh();
    info!("Started background JWT token refresh");

    // Start background offset progress logger with ETA
    let neo4j_uri_clone = sync_config.neo4j_uri.clone();
    let neo4j_user_clone = sync_config.neo4j_user.clone();
    let neo4j_pass_clone = sync_config.neo4j_pass.clone();
    let ledger_url_clone = sync_config.ledger_url.clone();
    let token_manager_for_progress = Arc::clone(&token_manager);
    let _progress_handle = tokio::spawn(async move {
        let mut prev_offset: Option<i64> = None;
        let mut prev_time: Option<Instant> = None;

        loop {
            tokio::time::sleep(Duration::from_secs(300)).await; // 5 minutes

            let current_offset = match get_last_processed_offset(&neo4j_uri_clone, &neo4j_user_clone, &neo4j_pass_clone).await {
                Ok(Some(offset)) => offset,
                Ok(None) => {
                    info!("[Progress] No offset data in Neo4j yet");
                    continue;
                }
                Err(e) => {
                    warn!("[Progress] Failed to query Neo4j offset: {}", e);
                    continue;
                }
            };

            // Get ledger end for ETA calculation
            let ledger_end = match token_manager_for_progress.get_token().await {
                Ok(token) => {
                    match client::ledger_end::get_ledger_end(&ledger_url_clone, Some(&token)).await {
                        Ok(end) => Some(end),
                        Err(e) => {
                            warn!("[Progress] Failed to get ledger end: {}", e);
                            None
                        }
                    }
                }
                Err(_) => None,
            };

            // Calculate rate and ETA
            let rate_info = if let (Some(prev), Some(time)) = (prev_offset, prev_time) {
                let elapsed_secs = time.elapsed().as_secs_f64();
                let offsets_processed = current_offset - prev;
                let rate = offsets_processed as f64 / elapsed_secs;

                if rate > 0.0 {
                    if let Some(end) = ledger_end {
                        let remaining = end - current_offset;
                        let eta_secs = remaining as f64 / rate;
                        let eta_hours = eta_secs / 3600.0;
                        format!(
                            "rate: {:.1} offsets/s, remaining: {}, ETA: {:.1} hours",
                            rate, remaining, eta_hours
                        )
                    } else {
                        format!("rate: {:.1} offsets/s", rate)
                    }
                } else {
                    "rate: stalled".to_string()
                }
            } else {
                if let Some(end) = ledger_end {
                    format!("ledger end: {}, remaining: {}", end, end - current_offset)
                } else {
                    "calculating...".to_string()
                }
            };

            info!("[Progress] Neo4j offset: {}, {}", current_offset, rate_info);

            prev_offset = Some(current_offset);
            prev_time = Some(Instant::now());
        }
    });
    info!("Started background progress logger (every 5 min)");

    let mut current_delay = backoff_config.initial_delay;
    let mut consecutive_failures = 0u32;
    let mut acs_loaded_checked = false;
    let mut fresh_start_offset: Option<i64> = None; // Used only on first iteration when fresh=true

    loop {
        // Get a fresh token
        let token = match token_manager.get_token().await {
            Ok(t) => t,
            Err(e) => {
                error!("Failed to get JWT token: {}. Retrying in {:?}", e, current_delay);
                tokio::time::sleep(current_delay).await;
                current_delay = std::cmp::min(
                    Duration::from_secs_f64(current_delay.as_secs_f64() * backoff_config.multiplier),
                    backoff_config.max_delay,
                );
                continue;
            }
        };

        // First, determine the starting offset
        let begin_offset = if fresh && fresh_start_offset.is_none() {
            // Fresh start: use current ledger end
            match get_ledger_end(&sync_config.ledger_url, Some(&token)).await {
                Ok(ledger_end) => {
                    info!("FRESH START: Using current ledger end as starting point: {}", ledger_end);
                    fresh_start_offset = Some(ledger_end);
                    ledger_end
                }
                Err(e) => {
                    error!("Failed to get ledger end: {}. Retrying in {:?}", e, current_delay);
                    tokio::time::sleep(current_delay).await;
                    current_delay = std::cmp::min(
                        Duration::from_secs_f64(current_delay.as_secs_f64() * backoff_config.multiplier),
                        backoff_config.max_delay,
                    );
                    continue;
                }
            }
        } else if let Some(offset) = fresh_start_offset {
            // Fresh start already determined, use that offset
            offset
        } else {
            // Normal mode: check Neo4j for resume point
            match get_last_processed_offset(
                &sync_config.neo4j_uri,
                &sync_config.neo4j_user,
                &sync_config.neo4j_pass,
            ).await {
                Ok(Some(offset)) => {
                    info!("Resuming from Neo4j offset: {}", offset);
                    offset
                }
                Ok(None) => {
                    // No data in Neo4j, use configured starting_offset or fall back to pruning offset
                    if let Some(configured_offset) = sync_config.starting_offset {
                        info!("No existing data in Neo4j, starting from configured starting_offset: {}", configured_offset);
                        configured_offset
                    } else {
                        match get_pruning_offset(&sync_config.ledger_url, Some(&token)).await {
                            Ok(pruning_offset) => {
                                info!("No existing data in Neo4j, starting from ledger pruning offset: {}", pruning_offset);
                                pruning_offset
                            }
                            Err(e) => {
                                error!("Failed to get pruning offset from ledger: {}. Starting from 0", e);
                                0
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to query Neo4j for last offset: {}. Querying ledger for pruning offset", e);
                    match get_pruning_offset(&sync_config.ledger_url, Some(&token)).await {
                        Ok(pruning_offset) => {
                            info!("Starting from ledger pruning offset: {}", pruning_offset);
                            pruning_offset
                        }
                        Err(e2) => {
                            error!("Failed to get pruning offset from ledger: {}. Starting from 0", e2);
                            0
                        }
                    }
                }
            }
        };

        // Load ACS on first run if not already loaded (at the starting offset)
        if !acs_loaded_checked {
            match is_acs_loaded(
                &sync_config.neo4j_uri,
                &sync_config.neo4j_user,
                &sync_config.neo4j_pass,
            ).await {
                Ok(true) => {
                    info!("ACS already loaded, skipping ACS load");
                    acs_loaded_checked = true;
                }
                Ok(false) => {
                    info!("ACS not yet loaded, loading at offset {}...", begin_offset);
                    match load_acs_to_neo4j(
                        &sync_config.ledger_url,
                        &sync_config.neo4j_uri,
                        &sync_config.neo4j_user,
                        &sync_config.neo4j_pass,
                        &sync_config.parties,
                        &token,
                        begin_offset,
                    ).await {
                        Ok(()) => {
                            info!("ACS loaded successfully");
                            acs_loaded_checked = true;
                        }
                        Err(e) => {
                            error!("Failed to load ACS: {}. Retrying in {:?}", e, current_delay);
                            tokio::time::sleep(current_delay).await;
                            current_delay = std::cmp::min(
                                Duration::from_secs_f64(current_delay.as_secs_f64() * backoff_config.multiplier),
                                backoff_config.max_delay,
                            );
                            continue;
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to check ACS status: {}. Assuming not loaded.", e);
                    match load_acs_to_neo4j(
                        &sync_config.ledger_url,
                        &sync_config.neo4j_uri,
                        &sync_config.neo4j_user,
                        &sync_config.neo4j_pass,
                        &sync_config.parties,
                        &token,
                        begin_offset,
                    ).await {
                        Ok(()) => {
                            info!("ACS loaded successfully");
                            acs_loaded_checked = true;
                        }
                        Err(e) => {
                            error!("Failed to load ACS: {}. Retrying in {:?}", e, current_delay);
                            tokio::time::sleep(current_delay).await;
                            current_delay = std::cmp::min(
                                Duration::from_secs_f64(current_delay.as_secs_f64() * backoff_config.multiplier),
                                backoff_config.max_delay,
                            );
                            continue;
                        }
                    }
                }
            }
        }

        info!("Starting stream from offset {}", begin_offset);

        // Start the update stream
        let update_stream = match stream_updates(
            Some(&token),
            begin_offset,
            None,
            sync_config.parties.clone(),
            sync_config.ledger_url.clone(),
        ).await {
            Ok(stream) => stream,
            Err(e) => {
                consecutive_failures += 1;
                error!(
                    "Failed to connect to ledger (attempt {}): {}. Retrying in {:?}",
                    consecutive_failures, e, current_delay
                );
                tokio::time::sleep(current_delay).await;
                current_delay = std::cmp::min(
                    Duration::from_secs_f64(current_delay.as_secs_f64() * backoff_config.multiplier),
                    backoff_config.max_delay,
                );
                continue;
            }
        };

        // Reset backoff on successful connection
        current_delay = backoff_config.initial_delay;
        consecutive_failures = 0;
        info!("Successfully connected to ledger stream");

        // Process the stream - take items while they're Ok, stop on first error
        // This allows us to gracefully reconnect when token expires
        let cypher_stream = update_stream
            .take_while(|update| {
                match update {
                    Ok(_) => true,
                    Err(e) => {
                        error!(error = %e, "Error in update stream, will reconnect");
                        false // Stop the stream on error
                    }
                }
            })
            .map(|update| {
                // Safe to unwrap here because take_while filters out errors
                let response = update.unwrap();
                let offset = response.update.as_ref().map(|u| match u {
                    ledger_api::v2::get_updates_response::Update::Transaction(tx) => tx.offset,
                    ledger_api::v2::get_updates_response::Update::Reassignment(r) => r.offset,
                    ledger_api::v2::get_updates_response::Update::OffsetCheckpoint(c) => c.offset,
                    ledger_api::v2::get_updates_response::Update::TopologyTransaction(t) => t.offset,
                });
                debug!(offset = ?offset, "Processing update from stream");
                cypher::get_updates_response_to_cypher(&response)
            });

        // Apply to Neo4j - this will return when the stream ends or errors
        match apply_cypher_vec_stream_to_neo4j(
            &sync_config.neo4j_uri,
            &sync_config.neo4j_user,
            &sync_config.neo4j_pass,
            cypher_stream,
        ).await {
            Ok((before, after, time)) => {
                info!(
                    "Stream processing completed. Offset {} -> {}, took {} ms",
                    before.unwrap_or(-1),
                    after.unwrap_or(-1),
                    time
                );
                // Stream ended - could be graceful end, server closed, or error filtered out
                // Either way, reconnect with a fresh token
                info!("Stream ended, reconnecting in {:?}", backoff_config.initial_delay);
                tokio::time::sleep(backoff_config.initial_delay).await;
            }
            Err(e) => {
                consecutive_failures += 1;
                error!(
                    "Stream processing failed (attempt {}): {}. Reconnecting in {:?}",
                    consecutive_failures, e, current_delay
                );
                tokio::time::sleep(current_delay).await;
                current_delay = std::cmp::min(
                    Duration::from_secs_f64(current_delay.as_secs_f64() * backoff_config.multiplier),
                    backoff_config.max_delay,
                );
            }
        }
    }
}
