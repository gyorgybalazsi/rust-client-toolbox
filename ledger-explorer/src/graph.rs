use futures_util::Stream;
use neo4rs::{Graph, Query, query};
use std::time::Instant;
use tokio_stream::StreamExt;


pub async fn apply_cypher_vec_stream_to_neo4j<S>(
    uri: &str,
    user: &str,
    pass: &str,
    mut query_stream: S,
) -> Result<(Option<i64>, Option<i64>, u128), Box<dyn std::error::Error>>
where
    S: Stream<Item = Vec<Query>> + Unpin,
{

    
    let graph = Graph::new(uri, user, pass)?;

    // Query max offset before update
    let before_offset = {
        let mut result = graph.execute(query("MATCH (n) RETURN max(n.offset) as max_offset")).await?;
        match result.next().await {
            Ok(Some(row)) => row.get::<Option<i64>>("max_offset")?,
            Ok(None) => None,
            Err(e) => return Err(Box::new(e)),
        }
    };

    // Measure update time
    let start_time = Instant::now();

    while let Some(cypher_vec) = query_stream.next().await {
        let mut txn = graph.start_txn().await?;
        txn.run_queries(cypher_vec).await?;
        txn.commit().await?;
    }

    let update_time_ms = start_time.elapsed().as_millis();

    // Query max offset after update
    let after_offset = {
        let mut result = graph.execute(query("MATCH (n) RETURN max(n.offset) as max_offset")).await?;
        match result.next().await {
            Ok(Some(row)) => row.get::<Option<i64>>("max_offset")?,
            Ok(None) => None,
            Err(e) => return Err(Box::new(e)),
        }
    };

    Ok((before_offset, after_offset, update_time_ms))
}
