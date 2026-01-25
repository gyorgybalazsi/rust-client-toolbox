use futures_util::Stream;
use tracing::{debug, info, warn};
use neo4rs::{Graph, query};
use std::time::Instant;
use tokio_stream::StreamExt;
use anyhow::Result;

pub use crate::cypher::CypherQuery;

/// Queries Neo4j for the maximum offset stored in the graph.
/// This is used to determine where to resume processing after a restart.
pub async fn get_last_processed_offset(uri: &str, user: &str, pass: &str) -> Result<Option<i64>> {
    debug!("Connecting to Neo4j at {} to query last offset", uri);
    let graph = Graph::new(uri, user, pass)?;

    // Exclude ACS contracts (offset = -1) from the max offset calculation
    let mut result = graph.execute(query("MATCH (n) WHERE n.offset IS NOT NULL AND n.offset >= 0 RETURN max(n.offset) as max_offset")).await?;
    match result.next().await {
        Ok(Some(row)) => {
            let offset = row.get::<Option<i64>>("max_offset")?;
            info!("Last processed offset from Neo4j: {:?}", offset);
            Ok(offset)
        }
        Ok(None) => {
            info!("No offset found in Neo4j (empty database)");
            Ok(None)
        }
        Err(e) => {
            warn!("Failed to query last offset from Neo4j: {}", e);
            Err(e.into())
        }
    }
}

pub async fn apply_cypher_vec_stream_to_neo4j<S>(
    uri: &str,
    user: &str,
    pass: &str,
    mut query_stream: S,
) -> Result<(Option<i64>, Option<i64>, u128), Box<dyn std::error::Error>>
where
    S: Stream<Item = Vec<CypherQuery>> + Unpin,
{
    info!("Connecting to Neo4j at {}", uri);
    let graph = Graph::new(uri, user, pass)?;
    debug!(uri = %uri, user = %user, "Successfully connected to Neo4j");

    // Query max offset before update
    debug!("Querying max offset before update");
    let before_offset = {
        let mut result = graph.execute(query("MATCH (n) RETURN max(n.offset) as max_offset")).await?;
        match result.next().await {
            Ok(Some(row)) => row.get::<Option<i64>>("max_offset")?,
            Ok(None) => None,
            Err(e) => return Err(Box::new(e)),
        }
    };
    info!("Max offset before update: {:?}", before_offset);

    // Measure update time
    let start_time = Instant::now();
    info!("Starting to process query stream");

    // Batch multiple updates together for better Neo4j throughput
    const BATCH_SIZE: usize = 20; // Commit every 20 updates (~500 queries)
    let mut batch_count = 0u64;
    let mut pending_queries: Vec<neo4rs::Query> = Vec::new();
    let mut updates_in_batch = 0usize;

    while let Some(cypher_vec) = query_stream.next().await {
        batch_count += 1;
        let query_count = cypher_vec.len();
        debug!(batch = batch_count, query_count = query_count, "Received update");

        // Accumulate queries
        let query_count_this_update = cypher_vec.len();
        pending_queries.extend(cypher_vec.into_iter().map(|cq| cq.query));
        updates_in_batch += 1;

        if updates_in_batch == 1 {
            info!("First update received, {} queries", query_count_this_update);
        }

        // Commit when batch is full
        if updates_in_batch >= BATCH_SIZE {
            let total_queries = pending_queries.len();
            info!("Starting batch commit: {} updates, {} queries", updates_in_batch, total_queries);
            let commit_start = Instant::now();
            let mut txn = graph.start_txn().await?;
            txn.run_queries(pending_queries).await?;
            txn.commit().await?;
            let commit_time = commit_start.elapsed();
            info!("Committed batch of {} updates ({} queries) in {:?} ({} total updates)",
                  BATCH_SIZE, total_queries, commit_time, batch_count);
            pending_queries = Vec::new();
            updates_in_batch = 0;
        }
    }

    // Commit any remaining queries
    if !pending_queries.is_empty() {
        debug!(updates = updates_in_batch, queries = pending_queries.len(), "Committing final batch");
        let mut txn = graph.start_txn().await?;
        txn.run_queries(pending_queries).await?;
        txn.commit().await?;
        info!("Committed final batch of {} updates", updates_in_batch);
    }

    let update_time_ms = start_time.elapsed().as_millis();
    info!("Processed {} batches in {} ms", batch_count, update_time_ms);

    // Query max offset after update
    debug!("Querying max offset after update");
    let after_offset = {
        let mut result = graph.execute(query("MATCH (n) RETURN max(n.offset) as max_offset")).await?;
        match result.next().await {
            Ok(Some(row)) => row.get::<Option<i64>>("max_offset")?,
            Ok(None) => None,
            Err(e) => return Err(Box::new(e)),
        }
    };
    info!("Max offset after update: {:?}", after_offset);

    info!(
        "Neo4j update complete: offset {:?} -> {:?}, {} batches in {} ms",
        before_offset, after_offset, batch_count, update_time_ms
    );

    Ok((before_offset, after_offset, update_time_ms))
}
