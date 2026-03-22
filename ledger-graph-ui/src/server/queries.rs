use crate::models::graph::{GraphData, GraphEdge, GraphNode, NodeLabel, RelType};
use crate::models::query::SchemaStats;
use dioxus::prelude::*;
use std::collections::HashMap;

fn parse_node_label(labels: &[String]) -> NodeLabel {
    for label in labels {
        match label.as_str() {
            "Transaction" => return NodeLabel::Transaction,
            "Created" => return NodeLabel::Created,
            "Exercised" => return NodeLabel::Exercised,
            "Party" => return NodeLabel::Party,
            _ => {}
        }
    }
    NodeLabel::Transaction
}

fn parse_rel_type(rel_type: &str) -> RelType {
    match rel_type {
        "ACTION" => RelType::Action,
        "CONSEQUENCE" => RelType::Consequence,
        "TARGET" => RelType::Target,
        "CONSUMES" => RelType::Consumes,
        "REQUESTED" => RelType::Requested,
        _ => RelType::Action,
    }
}

fn make_display_name(label: &NodeLabel, props: &HashMap<String, serde_json::Value>) -> String {
    match label {
        NodeLabel::Transaction => {
            if let Some(serde_json::Value::String(uid)) = props.get("update_id") {
                let short = if uid.len() > 12 {
                    &uid[..12]
                } else {
                    uid
                };
                format!("T:{short}")
            } else {
                "Transaction".to_string()
            }
        }
        NodeLabel::Created => {
            if let Some(serde_json::Value::String(tmpl)) = props.get("template_name") {
                tmpl.rsplit('.').next().unwrap_or(tmpl).to_string()
            } else {
                "Created".to_string()
            }
        }
        NodeLabel::Exercised => {
            if let Some(serde_json::Value::String(choice)) = props.get("choice_name") {
                choice.clone()
            } else {
                "Exercised".to_string()
            }
        }
        NodeLabel::Party => {
            if let Some(serde_json::Value::String(pid)) = props.get("party_id") {
                let short = if pid.len() > 20 {
                    &pid[..20]
                } else {
                    pid
                };
                short.to_string()
            } else {
                "Party".to_string()
            }
        }
    }
}

#[cfg(feature = "server")]
fn extract_node(node: &neo4rs::Node) -> GraphNode {
    let labels: Vec<String> = node.labels().iter().map(|l| l.to_string()).collect();
    let label = parse_node_label(&labels);
    let properties: HashMap<String, serde_json::Value> = node
        .keys()
        .iter()
        .filter_map(|k| {
            node.get::<serde_json::Value>(&**k)
                .ok()
                .map(|v| (k.to_string(), v))
        })
        .collect();
    let display_name = make_display_name(&label, &properties);
    GraphNode {
        id: node.id().to_string(),
        label,
        display_name,
        properties,
        x: 0.0,
        y: 0.0,
    }
}

#[cfg(feature = "server")]
fn extract_relation(rel: &neo4rs::Relation) -> GraphEdge {
    let rel_type = parse_rel_type(rel.typ());
    let properties: HashMap<String, serde_json::Value> = rel
        .keys()
        .iter()
        .filter_map(|k| {
            rel.get::<serde_json::Value>(&**k)
                .ok()
                .map(|v| (k.to_string(), v))
        })
        .collect();
    GraphEdge {
        id: rel.id().to_string(),
        source: rel.start_node_id().to_string(),
        target: rel.end_node_id().to_string(),
        rel_type,
        properties,
    }
}

/// Execute arbitrary Cypher and return graph data.
/// Wraps the query to extract nodes and relationships via COLLECT.
#[server]
pub async fn run_cypher(
    cypher: String,
    params: HashMap<String, String>,
) -> Result<GraphData, ServerFnError> {
    use super::neo4j_pool;

    let graph = neo4j_pool::pool();

    // Wrap user's Cypher to collect all nodes and relationships from paths.
    // We use CALL to run the user's query, then collect graph elements.
    let wrapped = format!(
        r#"CALL {{
  {cypher}
}}
WITH *
UNWIND keys({{_placeholder_: null}}) AS _dummy_
RETURN null"#,
    );

    // First, try the direct approach: run the user's query and extract
    // nodes/rels from each row by trying known types.
    let mut query = neo4rs::query(&cypher);
    for (k, v) in &params {
        query = query.param(k.as_str(), v.as_str());
    }

    let mut result = graph.execute(query).await.map_err(|e| {
        ServerFnError::new(format!("Neo4j query failed: {e}"))
    })?;

    let mut nodes: Vec<GraphNode> = Vec::new();
    let mut edges: Vec<GraphEdge> = Vec::new();
    let mut seen_nodes = std::collections::HashSet::new();
    let mut seen_edges = std::collections::HashSet::new();

    // We don't have access to column names directly, so try common column names
    // that the user might use. This covers the typical MATCH (a)-[r]->(b) RETURN a, r, b pattern.
    let _ = &wrapped; // suppress unused warning

    while let Some(row) = result.next().await.map_err(|e| {
        ServerFnError::new(format!("Neo4j row fetch failed: {e}"))
    })? {
        // Try to extract nodes from common column names
        let node_names = ["n", "m", "a", "b", "t", "e", "c", "p", "node", "start", "end",
                          "source", "target", "tx", "created", "exercised", "party"];
        for name in &node_names {
            if let Ok(node) = row.get::<neo4rs::Node>(name) {
                let id = node.id().to_string();
                if seen_nodes.insert(id) {
                    nodes.push(extract_node(&node));
                }
            }
        }

        // Try to extract relationships
        let rel_names = ["r", "rel", "r1", "r2", "action", "consequence"];
        for name in &rel_names {
            if let Ok(rel) = row.get::<neo4rs::Relation>(name) {
                let id = rel.id().to_string();
                if seen_edges.insert(id) {
                    edges.push(extract_relation(&rel));
                }
            }
        }

        // Try to extract paths
        let path_names = ["path", "p"];
        for name in &path_names {
            if let Ok(path) = row.get::<neo4rs::Path>(name) {
                let path_nodes = path.nodes();
                for node in &path_nodes {
                    let id = node.id().to_string();
                    if seen_nodes.insert(id) {
                        nodes.push(extract_node(node));
                    }
                }
                // For paths, pair nodes[i] -> rels[i] -> nodes[i+1]
                let path_rels = path.rels();
                for (i, rel) in path_rels.iter().enumerate() {
                    let id = rel.id().to_string();
                    if seen_edges.insert(id.clone()) {
                        let rel_type = parse_rel_type(rel.typ());
                        let keys = rel.keys();
                        let properties: HashMap<String, serde_json::Value> = keys
                            .iter()
                            .filter_map(|k| {
                                rel.get::<serde_json::Value>(&**k)
                                    .ok()
                                    .map(|v| (k.to_string(), v))
                            })
                            .collect();
                        let source = path_nodes.get(i).map(|n: &neo4rs::Node| n.id().to_string()).unwrap_or_default();
                        let target = path_nodes.get(i + 1).map(|n: &neo4rs::Node| n.id().to_string()).unwrap_or_default();
                        edges.push(GraphEdge {
                            id,
                            source,
                            target,
                            rel_type,
                            properties,
                        });
                    }
                }
            }
        }
    }

    // Assign initial positions in a circle layout
    let count = nodes.len() as f64;
    for (i, node) in nodes.iter_mut().enumerate() {
        let angle = 2.0 * std::f64::consts::PI * (i as f64) / count.max(1.0);
        let radius = 200.0 + count * 10.0;
        node.x = 400.0 + radius * angle.cos();
        node.y = 300.0 + radius * angle.sin();
    }

    Ok(GraphData { nodes, edges })
}

/// Get schema statistics: node counts by label, relationship counts by type
#[server]
pub async fn get_schema_stats() -> Result<SchemaStats, ServerFnError> {
    use super::neo4j_pool;

    let graph = neo4j_pool::pool();

    let mut node_counts = HashMap::new();
    for label in &["Transaction", "Created", "Exercised", "Party"] {
        let query = neo4rs::query(&format!("MATCH (n:{label}) RETURN count(n) as cnt"));
        let mut result = graph.execute(query).await.map_err(|e| {
            ServerFnError::new(format!("Neo4j query failed: {e}"))
        })?;
        if let Some(row) = result.next().await.map_err(|e| {
            ServerFnError::new(format!("Neo4j row fetch failed: {e}"))
        })? {
            let cnt: i64 = row.get("cnt").unwrap_or(0);
            node_counts.insert(label.to_string(), cnt as u64);
        }
    }

    let mut rel_counts = HashMap::new();
    for rel in &["ACTION", "CONSEQUENCE", "TARGET", "CONSUMES", "REQUESTED"] {
        let query = neo4rs::query(&format!("MATCH ()-[r:{rel}]->() RETURN count(r) as cnt"));
        let mut result = graph.execute(query).await.map_err(|e| {
            ServerFnError::new(format!("Neo4j query failed: {e}"))
        })?;
        if let Some(row) = result.next().await.map_err(|e| {
            ServerFnError::new(format!("Neo4j row fetch failed: {e}"))
        })? {
            let cnt: i64 = row.get("cnt").unwrap_or(0);
            rel_counts.insert(rel.to_string(), cnt as u64);
        }
    }

    Ok(SchemaStats {
        node_counts,
        rel_counts,
    })
}
