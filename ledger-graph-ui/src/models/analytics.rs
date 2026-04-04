use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AnalyticsQuery {
    pub label: String,
    pub cypher: String,
    #[serde(default)]
    pub min_time: Option<String>,
    #[serde(default)]
    pub max_time: Option<String>,
    #[serde(skip)]
    pub shared: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnalyticsQueryFile {
    #[serde(rename = "query")]
    pub queries: Vec<AnalyticsQuery>,
}
