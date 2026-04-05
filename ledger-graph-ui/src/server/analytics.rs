use crate::models::analytics::{AnalyticsQuery, AnalyticsQueryFile};
use dioxus::prelude::*;

const SHARED_QUERIES_PATH: &str = "analytics-queries.toml";
const LOCAL_QUERIES_PATH: &str = "analytics-queries.local.toml";

fn load_queries_from_file(path: &str, shared: bool) -> Vec<AnalyticsQuery> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    let file: AnalyticsQueryFile = match toml::from_str(&content) {
        Ok(f) => f,
        Err(e) => {
            tracing::warn!("Failed to parse {path}: {e}");
            return Vec::new();
        }
    };
    file.queries
        .into_iter()
        .map(|mut q| {
            q.shared = shared;
            q
        })
        .collect()
}

fn save_local_queries(queries: &[AnalyticsQuery]) -> Result<(), ServerFnError> {
    let file = AnalyticsQueryFile {
        queries: queries.to_vec(),
    };
    let content = toml::to_string_pretty(&file)
        .map_err(|e| ServerFnError::new(format!("Failed to serialize queries: {e}")))?;
    std::fs::write(LOCAL_QUERIES_PATH, content)
        .map_err(|e| ServerFnError::new(format!("Failed to write {LOCAL_QUERIES_PATH}: {e}")))?;
    Ok(())
}

/// Load all saved analytics queries (shared + local, merged).
/// Local queries override shared queries with the same label.
#[server]
pub async fn load_analytics_queries() -> Result<Vec<AnalyticsQuery>, ServerFnError> {
    let shared = load_queries_from_file(SHARED_QUERIES_PATH, true);
    let local = load_queries_from_file(LOCAL_QUERIES_PATH, false);

    let local_labels: std::collections::HashSet<String> =
        local.iter().map(|q| q.label.clone()).collect();
    let mut merged: Vec<AnalyticsQuery> = shared
        .into_iter()
        .filter(|q| !local_labels.contains(&q.label))
        .collect();
    merged.extend(local);
    Ok(merged)
}

/// Save a new query to the local TOML file.
/// Validates shape by running a limited version of the query.
#[server]
pub async fn save_analytics_query(
    label: String,
    cypher: String,
    min_time: Option<String>,
    max_time: Option<String>,
) -> Result<(), ServerFnError> {
    let pool = super::neo4j_pool::pool();

    // Try wrapping in CALL first; if that fails (e.g. nested CALL), run raw with LIMIT
    let validation_cypher = format!(
        "CALL {{ {cypher} }} WITH offset, value LIMIT 10"
    );
    let validation_result = pool
        .execute(neo4rs::query(&validation_cypher))
        .await;

    let mut result = match validation_result {
        Ok(r) => r,
        Err(_) => {
            // Fallback: run the query directly (it may already have ORDER BY, append LIMIT)
            let fallback = format!("{cypher} LIMIT 10");
            pool.execute(neo4rs::query(&fallback))
                .await
                .map_err(|e| ServerFnError::new(format!("Query validation failed: {e}")))?
        }
    };

    if let Some(row) = result.next().await.map_err(|e| {
        ServerFnError::new(format!("Failed to read validation result: {e}"))
    })? {
        let _offset: i64 = row.get("offset").map_err(|e| {
            ServerFnError::new(format!(
                "Column 'offset' must be integer. Got error: {e}"
            ))
        })?;
        let _value: f64 = row
            .get::<i64>("value")
            .map(|v| v as f64)
            .or_else(|_| row.get::<f64>("value"))
            .map_err(|e| {
                ServerFnError::new(format!(
                    "Column 'value' must be numeric. Got error: {e}"
                ))
            })?;
    }

    let mut local = load_queries_from_file(LOCAL_QUERIES_PATH, false);
    local.retain(|q| q.label != label);
    local.push(AnalyticsQuery {
        label,
        cypher,
        min_time,
        max_time,
        shared: false,
    });
    save_local_queries(&local)
}

/// Delete a query from the local TOML file.
#[server]
pub async fn delete_analytics_query(label: String) -> Result<(), ServerFnError> {
    let mut local = load_queries_from_file(LOCAL_QUERIES_PATH, false);
    let before = local.len();
    local.retain(|q| q.label != label);
    if local.len() == before {
        return Err(ServerFnError::new(format!(
            "Query '{label}' not found in local queries (shared queries cannot be deleted)"
        )));
    }
    save_local_queries(&local)
}

/// Run a Cypher query and return (offset, value) tuples.
/// If min_time/max_time are provided, queries offset bounds first,
/// runs the user query, then filters results in Rust.
#[server]
pub async fn run_analytics_query(
    cypher: String,
    min_time: Option<String>,
    max_time: Option<String>,
) -> Result<Vec<(i64, f64)>, ServerFnError> {
    let pool = super::neo4j_pool::pool();

    let (min_off, max_off) = if min_time.is_some() || max_time.is_some() {
        let mut cypher_str = String::from("MATCH (t:Transaction) WHERE ");
        let mut conditions = Vec::new();
        if min_time.is_some() {
            conditions.push("t.effective_at >= $min_time");
        }
        if max_time.is_some() {
            conditions.push("t.effective_at <= $max_time");
        }
        cypher_str.push_str(&conditions.join(" AND "));
        cypher_str.push_str(" RETURN min(t.offset) AS min_off, max(t.offset) AS max_off");

        let mut q = neo4rs::query(&cypher_str);
        if let Some(ref min_t) = min_time {
            q = q.param("min_time", min_t.as_str());
        }
        if let Some(ref max_t) = max_time {
            q = q.param("max_time", max_t.as_str());
        }

        let mut result = pool.execute(q).await.map_err(|e| {
            ServerFnError::new(format!("Offset bounds query failed: {e}"))
        })?;

        if let Some(row) = result.next().await.map_err(|e| {
            ServerFnError::new(format!("Failed to read offset bounds: {e}"))
        })? {
            let min_o: Option<i64> = row.get("min_off").ok();
            let max_o: Option<i64> = row.get("max_off").ok();
            (min_o, max_o)
        } else {
            (None, None)
        }
    } else {
        (None, None)
    };

    let mut result = pool
        .execute(neo4rs::query(&cypher))
        .await
        .map_err(|e| ServerFnError::new(format!("Analytics query failed: {e}")))?;

    let mut data: Vec<(i64, f64)> = Vec::new();
    while let Some(row) = result.next().await.map_err(|e| {
        ServerFnError::new(format!("Failed to read analytics row: {e}"))
    })? {
        let offset: i64 = match row.get("offset") {
            Ok(v) => v,
            Err(_) => continue,
        };
        // Neo4j count() returns i64. Try i64 first, then f64.
        let value: f64 = row
            .get::<i64>("value")
            .map(|v| v as f64)
            .or_else(|_| row.get::<f64>("value"))
            .unwrap_or(0.0);
        data.push((offset, value));
    }

    if let Some(min_o) = min_off {
        data.retain(|(offset, _)| *offset >= min_o);
    }
    if let Some(max_o) = max_off {
        data.retain(|(offset, _)| *offset <= max_o);
    }

    data.sort_by_key(|(offset, _)| *offset);

    Ok(data)
}

/// Fetch offset → effective_at mapping for all transactions.
#[server]
pub async fn get_offset_dates() -> Result<Vec<(i64, String)>, ServerFnError> {
    let pool = super::neo4j_pool::pool();
    let query = neo4rs::query(
        "MATCH (t:Transaction) RETURN t.offset AS offset, t.effective_at AS date ORDER BY t.offset"
    );
    let mut result = pool.execute(query).await.map_err(|e| {
        ServerFnError::new(format!("Offset dates query failed: {e}"))
    })?;

    let mut dates: Vec<(i64, String)> = Vec::new();
    while let Some(row) = result.next().await.map_err(|e| {
        ServerFnError::new(format!("Failed to read offset date row: {e}"))
    })? {
        let offset: i64 = match row.get("offset") {
            Ok(v) => v,
            Err(_) => continue,
        };
        let date: String = row.get("date").unwrap_or_default();
        if !date.is_empty() {
            dates.push((offset, date));
        }
    }

    Ok(dates)
}

/// Fetch all distinct template names from Created nodes.
#[server]
pub async fn get_template_names() -> Result<Vec<String>, ServerFnError> {
    let pool = super::neo4j_pool::pool();
    let query = neo4rs::query(
        "MATCH (c:Created) RETURN DISTINCT c.template_name AS template ORDER BY template"
    );
    let mut result = pool.execute(query).await.map_err(|e| {
        ServerFnError::new(format!("Template names query failed: {e}"))
    })?;

    let mut names: Vec<String> = Vec::new();
    while let Some(row) = result.next().await.map_err(|e| {
        ServerFnError::new(format!("Failed to read template name row: {e}"))
    })? {
        let name: String = row.get("template").unwrap_or_default();
        if !name.is_empty() {
            names.push(name);
        }
    }

    Ok(names)
}
