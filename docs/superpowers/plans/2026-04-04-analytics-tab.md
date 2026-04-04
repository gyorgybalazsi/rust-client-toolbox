# Analytics Tab Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an analytics tab to ledger-graph-ui that displays SVG line charts from user-defined Cypher queries, with per-query time ranges, a two-handle zoom slider, day boundary markers, and CSV export.

**Architecture:** Tab bar switches between existing Graph view and new Analytics view. Analytics has 3-panel layout (query manager | SVG chart + zoom slider | controls + legend). Server functions run Cypher queries against Neo4j and manage TOML-based query storage. All chart rendering and zoom filtering is client-side over cached data.

**Tech Stack:** Dioxus 0.7.3 fullstack, neo4rs, SVG in RSX, web-sys (for CSV blob download), TOML for query persistence.

---

## File Structure

### New Files

| File | Responsibility |
|------|---------------|
| `ledger-graph-ui/src/models/analytics.rs` | `AnalyticsQuery` struct, `AnalyticsQueryFile` wrapper for TOML serde |
| `ledger-graph-ui/src/server/analytics.rs` | 5 server functions: run query, get offset dates, load/save/delete queries |
| `ledger-graph-ui/src/components/analytics.rs` | Main analytics tab component: 3-panel layout, all state signals, data fetching, CSV export |
| `ledger-graph-ui/src/components/analytics_chart.rs` | SVG line chart: axes, gridlines, polylines, day bands, zoom slider, tooltips, legend |
| `ledger-graph-ui/src/components/analytics_queries.rs` | Left panel: query toggle buttons, add/delete form |
| `ledger-graph-ui/analytics-queries.toml` | Default shared queries (3 defaults) |

### Modified Files

| File | Change |
|------|--------|
| `ledger-graph-ui/src/models/mod.rs` | Add `pub mod analytics;` |
| `ledger-graph-ui/src/server/mod.rs` | Add `pub mod analytics;` |
| `ledger-graph-ui/src/components/mod.rs` | Add 3 new module registrations |
| `ledger-graph-ui/src/components/app.rs` | Add `ActiveTab` enum, tab bar, conditional content rendering |
| `ledger-graph-ui/Cargo.toml` | Add `web-sys` dependency with features |
| `ledger-graph-ui/assets/main.css` | Add tab bar, analytics panel, chart, zoom slider styles |
| `.gitignore` | Add `analytics-queries.local.toml` |

---

### Task 1: Add AnalyticsQuery model and default queries TOML

**Files:**
- Create: `ledger-graph-ui/src/models/analytics.rs`
- Create: `ledger-graph-ui/analytics-queries.toml`
- Modify: `ledger-graph-ui/src/models/mod.rs`

- [ ] **Step 1: Create the analytics model**

```rust
// ledger-graph-ui/src/models/analytics.rs
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
```

- [ ] **Step 2: Create default analytics queries TOML**

```toml
# ledger-graph-ui/analytics-queries.toml
[[query]]
label = "Transactions per offset"
cypher = "MATCH (t:Transaction) RETURN t.offset AS offset, 1 AS value ORDER BY offset"

[[query]]
label = "Created per offset"
cypher = "MATCH (t:Transaction)-[:ACTION]->(c:Created) RETURN t.offset AS offset, count(c) AS value ORDER BY offset"

[[query]]
label = "Exercised per offset"
cypher = "MATCH (t:Transaction)-[:ACTION]->(e:Exercised) RETURN t.offset AS offset, count(e) AS value ORDER BY offset"
```

- [ ] **Step 3: Register the module**

In `ledger-graph-ui/src/models/mod.rs`, add:

```rust
pub mod analytics;
```

So the file becomes:

```rust
pub mod analytics;
pub mod graph;
pub mod query;
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo check -p ledger-graph-ui`
Expected: compiles with no errors

- [ ] **Step 5: Commit**

```bash
git add ledger-graph-ui/src/models/analytics.rs ledger-graph-ui/src/models/mod.rs ledger-graph-ui/analytics-queries.toml
git commit -m "feat: add AnalyticsQuery model and default queries TOML"
```

---

### Task 2: Server functions — query storage (load/save/delete)

**Files:**
- Create: `ledger-graph-ui/src/server/analytics.rs`
- Modify: `ledger-graph-ui/src/server/mod.rs`

- [ ] **Step 1: Create server/analytics.rs with TOML load/save/delete functions**

```rust
// ledger-graph-ui/src/server/analytics.rs
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

    // Merge: local overrides shared by label
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
/// Validates shape by running the query with LIMIT 10 via subquery wrapper.
#[server]
pub async fn save_analytics_query(
    label: String,
    cypher: String,
    min_time: Option<String>,
    max_time: Option<String>,
) -> Result<(), ServerFnError> {
    // Validate query shape
    let pool = super::neo4j_pool::pool();
    let validation_cypher = format!(
        "CALL {{ {cypher} }} WITH offset, value LIMIT 10"
    );
    let mut result = pool
        .execute(neo4rs::query(&validation_cypher))
        .await
        .map_err(|e| ServerFnError::new(format!("Query validation failed: {e}")))?;

    // Check that we can extract offset (integer) and value (numeric)
    if let Some(row) = result.next().await.map_err(|e| {
        ServerFnError::new(format!("Failed to read validation result: {e}"))
    })? {
        // Try extracting offset as i64
        let _offset: i64 = row.get("offset").map_err(|e| {
            ServerFnError::new(format!(
                "Column 'offset' must be integer. Got error: {e}"
            ))
        })?;
        // Try extracting value as f64
        let _value: f64 = row.get::<f64>("value").or_else(|_| {
            row.get::<i64>("value").map(|v| v as f64)
        }).map_err(|e| {
            ServerFnError::new(format!(
                "Column 'value' must be numeric. Got error: {e}"
            ))
        })?;
    }
    // Zero rows is a warning but we still save

    // Load existing local queries and append
    let mut local = load_queries_from_file(LOCAL_QUERIES_PATH, false);
    // Remove existing query with same label if present
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
```

- [ ] **Step 2: Register the module**

In `ledger-graph-ui/src/server/mod.rs`, change to:

```rust
#[cfg(feature = "server")]
pub mod neo4j_pool;
pub mod analytics;
pub mod queries;
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check -p ledger-graph-ui`
Expected: compiles (server functions compile on both targets; neo4rs calls are inside `#[server]` which only runs server-side)

- [ ] **Step 4: Commit**

```bash
git add ledger-graph-ui/src/server/analytics.rs ledger-graph-ui/src/server/mod.rs
git commit -m "feat: add server functions for analytics query CRUD"
```

---

### Task 3: Server functions — run_analytics_query and get_offset_dates

**Files:**
- Modify: `ledger-graph-ui/src/server/analytics.rs`

- [ ] **Step 1: Add run_analytics_query and get_offset_dates to server/analytics.rs**

Append to the end of `ledger-graph-ui/src/server/analytics.rs`:

```rust
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

    // Step 1: If time bounds set, determine offset range
    let (min_off, max_off) = if min_time.is_some() || max_time.is_some() {
        let mut where_clauses = Vec::new();
        let mut bound_query = neo4rs::query("MATCH (t:Transaction) RETURN min(t.offset) AS min_off, max(t.offset) AS max_off");

        // Build dynamic WHERE clause
        let mut cypher_str = String::from("MATCH (t:Transaction) WHERE ");
        let mut conditions = Vec::new();
        if let Some(ref min_t) = min_time {
            conditions.push("t.effective_at >= $min_time");
        }
        if let Some(ref max_t) = max_time {
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

    // Step 2: Run user query
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
        let value: f64 = row
            .get::<f64>("value")
            .or_else(|_| row.get::<i64>("value").map(|v| v as f64))
            .unwrap_or(0.0);
        data.push((offset, value));
    }

    // Step 3: Filter by offset bounds if applicable
    if let Some(min_o) = min_off {
        data.retain(|(offset, _)| *offset >= min_o);
    }
    if let Some(max_o) = max_off {
        data.retain(|(offset, _)| *offset <= max_o);
    }

    // Sort by offset
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
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check -p ledger-graph-ui`
Expected: compiles with no errors

- [ ] **Step 3: Commit**

```bash
git add ledger-graph-ui/src/server/analytics.rs
git commit -m "feat: add run_analytics_query and get_offset_dates server functions"
```

---

### Task 4: Tab bar and ActiveTab switching in App

**Files:**
- Modify: `ledger-graph-ui/src/components/app.rs`
- Modify: `ledger-graph-ui/assets/main.css`

- [ ] **Step 1: Add ActiveTab enum and tab bar to app.rs**

At the top of `app.rs`, after the existing imports, add:

```rust
#[derive(Clone, Copy, PartialEq)]
enum ActiveTab {
    Graph,
    Analytics,
}
```

Inside the `App` component, after the existing signal declarations (`selection`, etc.), add:

```rust
let mut active_tab = use_signal(|| ActiveTab::Graph);
```

In the RSX, insert the tab bar between `div { class: "top-bar", ... }` and `div { class: "main-content", ...}`:

```rust
div { class: "tab-bar",
    button {
        class: if *active_tab.read() == ActiveTab::Graph { "tab-btn active" } else { "tab-btn" },
        onclick: move |_| active_tab.set(ActiveTab::Graph),
        "Graph"
    }
    button {
        class: if *active_tab.read() == ActiveTab::Analytics { "tab-btn active" } else { "tab-btn" },
        onclick: move |_| active_tab.set(ActiveTab::Analytics),
        "Analytics"
    }
}
```

Wrap the existing `main-content` div in a conditional on `ActiveTab::Graph`:

```rust
if *active_tab.read() == ActiveTab::Graph {
    div { class: "main-content",
        // ... existing left-panel, center-panel, right-panel ...
    }
}
if *active_tab.read() == ActiveTab::Analytics {
    div { class: "main-content",
        div { class: "center-panel",
            p { "Analytics tab — coming soon" }
        }
    }
}
```

- [ ] **Step 2: Add tab bar CSS**

Append to `ledger-graph-ui/assets/main.css`:

```css
/* Tab Bar */
.tab-bar {
    display: flex;
    gap: 0;
    background: #16213e;
    border-bottom: 2px solid #0f3460;
    padding: 0 16px;
}

.tab-btn {
    padding: 8px 20px;
    background: transparent;
    color: #888;
    border: none;
    border-bottom: 2px solid transparent;
    cursor: pointer;
    font-size: 14px;
    font-weight: 500;
    margin-bottom: -2px;
    transition: color 0.2s, border-color 0.2s;
}

.tab-btn:hover {
    color: #ccc;
}

.tab-btn.active {
    color: #4A90D9;
    border-bottom: 2px solid #4A90D9;
}
```

- [ ] **Step 3: Verify it compiles and renders**

Run: `cargo check -p ledger-graph-ui`
Expected: compiles. Tab bar appears between top bar and content. Clicking "Graph" shows existing graph view; clicking "Analytics" shows placeholder.

- [ ] **Step 4: Commit**

```bash
git add ledger-graph-ui/src/components/app.rs ledger-graph-ui/assets/main.css
git commit -m "feat: add tab bar with Graph/Analytics switching"
```

---

### Task 5: Analytics queries left panel component

**Files:**
- Create: `ledger-graph-ui/src/components/analytics_queries.rs`
- Modify: `ledger-graph-ui/src/components/mod.rs`
- Modify: `ledger-graph-ui/assets/main.css`

- [ ] **Step 1: Create analytics_queries.rs**

```rust
// ledger-graph-ui/src/components/analytics_queries.rs
use crate::models::analytics::AnalyticsQuery;
use dioxus::prelude::*;
use std::collections::{HashMap, HashSet};

const COLORS: &[&str] = &[
    "#4A90D9", "#50C878", "#F5A623", "#9B59B6",
    "#E74C3C", "#1ABC9C", "#E67E22", "#3498DB",
];

pub fn color_for_index(idx: usize) -> &'static str {
    COLORS[idx % COLORS.len()]
}

#[component]
pub fn AnalyticsQueriesPanel(
    saved_queries: Vec<AnalyticsQuery>,
    active_queries: Signal<HashSet<String>>,
    loading_queries: Signal<HashSet<String>>,
    query_errors: Signal<HashMap<String, String>>,
    on_toggle: EventHandler<String>,
    on_delete: EventHandler<String>,
    on_save: EventHandler<(String, String, Option<String>, Option<String>)>,
) -> Element {
    let mut show_form = use_signal(|| false);
    let mut new_label = use_signal(String::new);
    let mut new_cypher = use_signal(String::new);
    let mut new_min_time = use_signal(String::new);
    let mut new_max_time = use_signal(String::new);

    let active = active_queries.read();
    let loading = loading_queries.read();
    let errors = query_errors.read();

    rsx! {
        div { class: "analytics-queries",
            h3 { "Queries" }
            for (idx, query) in saved_queries.iter().enumerate() {
                {
                    let label = query.label.clone();
                    let is_active = active.contains(&label);
                    let is_loading = loading.contains(&label);
                    let error = errors.get(&label).cloned();
                    let color = if is_active { color_for_index(idx) } else { "#555" };
                    let label_toggle = label.clone();
                    let label_delete = label.clone();

                    rsx! {
                        div {
                            class: if is_active { "query-toggle active" } else { "query-toggle" },
                            style: "border-left: 3px solid {color};",
                            onclick: move |_| on_toggle.call(label_toggle.clone()),
                            div { class: "query-toggle-content",
                                span { class: "query-label", "{label}" }
                                if query.shared {
                                    span { class: "shared-badge", "shared" }
                                }
                                if is_loading {
                                    span { class: "query-spinner", "..." }
                                }
                                if let Some(ref err) = error {
                                    span { class: "query-error-indicator", title: "{err}", "!" }
                                }
                            }
                            if let (Some(ref min_t), Some(ref max_t)) = (&query.min_time, &query.max_time) {
                                div { class: "query-time-range", "{min_t} - {max_t}" }
                            }
                            if !query.shared {
                                button {
                                    class: "query-delete-btn",
                                    onclick: move |evt| {
                                        evt.stop_propagation();
                                        on_delete.call(label_delete.clone());
                                    },
                                    "x"
                                }
                            }
                        }
                    }
                }
            }

            if *show_form.read() {
                div { class: "add-query-form",
                    input {
                        class: "query-form-input",
                        r#type: "text",
                        placeholder: "Query label",
                        value: "{new_label}",
                        oninput: move |evt| new_label.set(evt.value()),
                    }
                    textarea {
                        class: "query-form-textarea",
                        rows: 4,
                        placeholder: "Cypher query (must return offset, value columns)",
                        value: "{new_cypher}",
                        oninput: move |evt| new_cypher.set(evt.value()),
                    }
                    div { class: "query-form-time",
                        label { "Min time:" }
                        input {
                            r#type: "datetime-local",
                            class: "query-form-input",
                            value: "{new_min_time}",
                            oninput: move |evt| new_min_time.set(evt.value()),
                        }
                    }
                    div { class: "query-form-time",
                        label { "Max time:" }
                        input {
                            r#type: "datetime-local",
                            class: "query-form-input",
                            value: "{new_max_time}",
                            oninput: move |evt| new_max_time.set(evt.value()),
                        }
                    }
                    button {
                        class: "query-form-save",
                        onclick: move |_| {
                            let label = new_label.read().clone();
                            let cypher = new_cypher.read().clone();
                            let min_t = {
                                let v = new_min_time.read().clone();
                                if v.is_empty() { None } else { Some(v) }
                            };
                            let max_t = {
                                let v = new_max_time.read().clone();
                                if v.is_empty() { None } else { Some(v) }
                            };
                            if !label.is_empty() && !cypher.is_empty() {
                                on_save.call((label, cypher, min_t, max_t));
                                new_label.set(String::new());
                                new_cypher.set(String::new());
                                new_min_time.set(String::new());
                                new_max_time.set(String::new());
                                show_form.set(false);
                            }
                        },
                        "Save"
                    }
                    button {
                        class: "query-form-cancel",
                        onclick: move |_| show_form.set(false),
                        "Cancel"
                    }
                }
            } else {
                button {
                    class: "add-query-btn",
                    onclick: move |_| show_form.set(true),
                    "+ Add Query"
                }
            }
        }
    }
}
```

- [ ] **Step 2: Register the module**

In `ledger-graph-ui/src/components/mod.rs`, add:

```rust
pub mod analytics_queries;
```

- [ ] **Step 3: Add query panel CSS**

Append to `ledger-graph-ui/assets/main.css`:

```css
/* Analytics Queries Panel */
.analytics-queries {
    padding: 12px;
}

.analytics-queries h3 {
    font-size: 17px;
    margin-bottom: 10px;
    color: #4A90D9;
}

.query-toggle {
    display: flex;
    flex-direction: column;
    padding: 6px 8px;
    margin-bottom: 4px;
    background: #0f3460;
    border-radius: 4px;
    cursor: pointer;
    position: relative;
    transition: background 0.15s;
}

.query-toggle:hover {
    background: #1a4a8e;
}

.query-toggle.active {
    background: #1a3a6e;
}

.query-toggle-content {
    display: flex;
    align-items: center;
    gap: 6px;
}

.query-label {
    font-size: 13px;
    color: #e0e0e0;
    flex: 1;
}

.shared-badge {
    font-size: 9px;
    color: #888;
    background: #0a1a3e;
    padding: 1px 4px;
    border-radius: 3px;
}

.query-spinner {
    color: #4A90D9;
    font-size: 12px;
    animation: pulse 1s infinite;
}

@keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.3; }
}

.query-error-indicator {
    color: #E74C3C;
    font-weight: bold;
    font-size: 14px;
    cursor: help;
}

.query-time-range {
    font-size: 10px;
    color: #666;
    margin-top: 2px;
}

.query-delete-btn {
    position: absolute;
    top: 4px;
    right: 4px;
    background: none;
    border: none;
    color: #666;
    cursor: pointer;
    font-size: 12px;
    opacity: 0;
    transition: opacity 0.15s;
}

.query-toggle:hover .query-delete-btn {
    opacity: 1;
}

.query-delete-btn:hover {
    color: #E74C3C;
}

.add-query-btn {
    width: 100%;
    padding: 6px;
    margin-top: 8px;
    background: #0f3460;
    color: #888;
    border: 1px dashed #333;
    border-radius: 4px;
    cursor: pointer;
    font-size: 12px;
}

.add-query-btn:hover {
    color: #ccc;
    border-color: #4A90D9;
}

.add-query-form {
    margin-top: 8px;
    display: flex;
    flex-direction: column;
    gap: 6px;
}

.query-form-input {
    background: #0a1a3e;
    color: #e0e0e0;
    border: 1px solid #0f3460;
    border-radius: 4px;
    padding: 4px 6px;
    font-size: 12px;
}

.query-form-textarea {
    background: #0a1a3e;
    color: #e0e0e0;
    border: 1px solid #0f3460;
    border-radius: 4px;
    padding: 4px 6px;
    font-size: 12px;
    font-family: monospace;
    resize: vertical;
}

.query-form-time {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 11px;
    color: #888;
}

.query-form-time input {
    flex: 1;
    color-scheme: dark;
}

.query-form-save, .query-form-cancel {
    padding: 4px 10px;
    border: none;
    border-radius: 4px;
    cursor: pointer;
    font-size: 12px;
}

.query-form-save {
    background: #4A90D9;
    color: white;
}

.query-form-cancel {
    background: #333;
    color: #aaa;
}
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo check -p ledger-graph-ui`
Expected: compiles with no errors

- [ ] **Step 5: Commit**

```bash
git add ledger-graph-ui/src/components/analytics_queries.rs ledger-graph-ui/src/components/mod.rs ledger-graph-ui/assets/main.css
git commit -m "feat: add analytics queries left panel component"
```

---

### Task 6: SVG line chart component

**Files:**
- Create: `ledger-graph-ui/src/components/analytics_chart.rs`
- Modify: `ledger-graph-ui/src/components/mod.rs`

- [ ] **Step 1: Create analytics_chart.rs**

```rust
// ledger-graph-ui/src/components/analytics_chart.rs
use crate::components::analytics_queries::color_for_index;
use crate::models::analytics::AnalyticsQuery;
use dioxus::prelude::*;
use std::collections::{HashMap, HashSet};

const CHART_PADDING_LEFT: f64 = 60.0;
const CHART_PADDING_RIGHT: f64 = 20.0;
const CHART_PADDING_TOP: f64 = 20.0;
const CHART_PADDING_BOTTOM: f64 = 60.0;
const CHART_WIDTH: f64 = 1000.0;
const CHART_HEIGHT: f64 = 500.0;

fn plot_x(offset: i64, min_off: i64, max_off: i64) -> f64 {
    let range = (max_off - min_off).max(1) as f64;
    CHART_PADDING_LEFT + (offset - min_off) as f64 / range * (CHART_WIDTH - CHART_PADDING_LEFT - CHART_PADDING_RIGHT)
}

fn plot_y(value: f64, max_val: f64) -> f64 {
    let usable = CHART_HEIGHT - CHART_PADDING_TOP - CHART_PADDING_BOTTOM;
    if max_val <= 0.0 {
        return CHART_HEIGHT - CHART_PADDING_BOTTOM;
    }
    CHART_HEIGHT - CHART_PADDING_BOTTOM - (value / max_val) * usable
}

/// Compute day boundary info: Vec<(date_string, first_offset, last_offset)>
fn compute_day_groups(
    offset_dates: &[(i64, String)],
    min_off: i64,
    max_off: i64,
) -> Vec<(String, i64, i64)> {
    let mut groups: Vec<(String, i64, i64)> = Vec::new();
    for &(offset, ref date_str) in offset_dates {
        if offset < min_off || offset > max_off {
            continue;
        }
        let day = &date_str[..10.min(date_str.len())];
        if let Some(last) = groups.last_mut() {
            if last.0 == day {
                last.2 = offset;
                continue;
            }
        }
        groups.push((day.to_string(), offset, offset));
    }
    groups
}

#[component]
pub fn AnalyticsChart(
    query_results: HashMap<String, Vec<(i64, f64)>>,
    offset_dates: Vec<(i64, String)>,
    active_queries: HashSet<String>,
    saved_queries: Vec<AnalyticsQuery>,
    zoom_range: Option<(i64, i64)>,
    on_zoom_change: EventHandler<(i64, i64)>,
) -> Element {
    // Determine visible data
    let all_offsets: Vec<i64> = query_results
        .iter()
        .filter(|(label, _)| active_queries.contains(label.as_str()))
        .flat_map(|(_, data)| data.iter().map(|(o, _)| *o))
        .collect();

    if all_offsets.is_empty() {
        return rsx! {
            div { class: "chart-empty", "Select a query to display data" }
        };
    }

    let data_min = *all_offsets.iter().min().unwrap();
    let data_max = *all_offsets.iter().max().unwrap();
    let (zoom_min, zoom_max) = zoom_range.unwrap_or((data_min, data_max));

    // Filter data to zoom range and find max value
    let mut max_val: f64 = 1.0;
    let mut series_points: Vec<(usize, String, Vec<(i64, f64)>)> = Vec::new();
    for (idx, query) in saved_queries.iter().enumerate() {
        if !active_queries.contains(&query.label) {
            continue;
        }
        if let Some(data) = query_results.get(&query.label) {
            let filtered: Vec<(i64, f64)> = data
                .iter()
                .filter(|(o, _)| *o >= zoom_min && *o <= zoom_max)
                .copied()
                .collect();
            for &(_, v) in &filtered {
                if v > max_val {
                    max_val = v;
                }
            }
            series_points.push((idx, query.label.clone(), filtered));
        }
    }

    // Round up max_val for nice gridlines
    let grid_step = nice_step(max_val);
    let y_max = (max_val / grid_step).ceil() * grid_step;

    // Day groups for bands
    let day_groups = compute_day_groups(&offset_dates, zoom_min, zoom_max);

    let view_box = format!("0 0 {CHART_WIDTH} {CHART_HEIGHT}");

    rsx! {
        div { class: "chart-container",
            svg {
                class: "analytics-chart",
                view_box: view_box,
                preserve_aspect_ratio: "xMidYMid meet",

                // Day bands (alternating)
                for (i, group) in day_groups.iter().enumerate() {
                    {
                        let x1 = plot_x(group.1, zoom_min, zoom_max);
                        let x2 = plot_x(group.2, zoom_min, zoom_max);
                        let fill = if i % 2 == 0 { "#1e2240" } else { "#1a1a2e" };
                        let label_x = (x1 + x2) / 2.0;
                        rsx! {
                            rect {
                                x: x1,
                                y: CHART_PADDING_TOP,
                                width: (x2 - x1).max(2.0),
                                height: CHART_HEIGHT - CHART_PADDING_TOP - CHART_PADDING_BOTTOM,
                                fill: fill,
                            }
                            // Day boundary dashed line (skip first)
                            if i > 0 {
                                line {
                                    x1: x1,
                                    y1: CHART_PADDING_TOP,
                                    x2: x1,
                                    y2: CHART_HEIGHT - CHART_PADDING_BOTTOM,
                                    stroke: "#555",
                                    stroke_width: 1.0,
                                    stroke_dasharray: "4,4",
                                }
                            }
                            // Day label
                            text {
                                x: label_x,
                                y: CHART_HEIGHT - CHART_PADDING_BOTTOM + 20.0,
                                text_anchor: "middle",
                                font_size: "11px",
                                fill: "#888",
                                {group.0.clone()}
                            }
                        }
                    }
                }

                // Y-axis gridlines
                {
                    let num_lines = (y_max / grid_step) as usize;
                    (0..=num_lines).map(|i| {
                        let val = (i as f64) * grid_step;
                        let y = plot_y(val, y_max);
                        rsx! {
                            line {
                                x1: CHART_PADDING_LEFT,
                                y1: y,
                                x2: CHART_WIDTH - CHART_PADDING_RIGHT,
                                y2: y,
                                stroke: "#333",
                                stroke_width: 0.5,
                            }
                            text {
                                x: CHART_PADDING_LEFT - 8.0,
                                y: y + 4.0,
                                text_anchor: "end",
                                font_size: "10px",
                                fill: "#888",
                                {format_value(val)}
                            }
                        }
                    })
                }

                // Axes
                line {
                    x1: CHART_PADDING_LEFT,
                    y1: CHART_PADDING_TOP,
                    x2: CHART_PADDING_LEFT,
                    y2: CHART_HEIGHT - CHART_PADDING_BOTTOM,
                    stroke: "#555",
                    stroke_width: 1.0,
                }
                line {
                    x1: CHART_PADDING_LEFT,
                    y1: CHART_HEIGHT - CHART_PADDING_BOTTOM,
                    x2: CHART_WIDTH - CHART_PADDING_RIGHT,
                    y2: CHART_HEIGHT - CHART_PADDING_BOTTOM,
                    stroke: "#555",
                    stroke_width: 1.0,
                }

                // Data series
                for (idx, label, points) in series_points.iter() {
                    {
                        let color = color_for_index(*idx);
                        let polyline_points: String = points
                            .iter()
                            .map(|(o, v)| {
                                let x = plot_x(*o, zoom_min, zoom_max);
                                let y = plot_y(*v, y_max);
                                format!("{x},{y}")
                            })
                            .collect::<Vec<_>>()
                            .join(" ");
                        rsx! {
                            polyline {
                                points: polyline_points,
                                fill: "none",
                                stroke: color,
                                stroke_width: 2.0,
                            }
                        }
                    }
                }

                // Legend
                {
                    let legend_x = CHART_WIDTH - CHART_PADDING_RIGHT - 160.0;
                    let legend_y = CHART_PADDING_TOP + 10.0;
                    let active_series: Vec<_> = saved_queries.iter().enumerate()
                        .filter(|(_, q)| active_queries.contains(&q.label))
                        .collect();
                    rsx! {
                        for (li, (idx, query)) in active_series.iter().enumerate() {
                            {
                                let y = legend_y + (li as f64) * 18.0;
                                let color = color_for_index(*idx);
                                rsx! {
                                    circle {
                                        cx: legend_x,
                                        cy: y,
                                        r: 4.0,
                                        fill: color,
                                    }
                                    text {
                                        x: legend_x + 10.0,
                                        y: y + 4.0,
                                        font_size: "11px",
                                        fill: "#ccc",
                                        {query.label.clone()}
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Zoom slider
            ZoomSlider {
                data_min,
                data_max,
                zoom_min,
                zoom_max,
                offset_dates: offset_dates.clone(),
                on_change: on_zoom_change,
            }
        }
    }
}

/// Two-handle range slider rendered as SVG below the chart
#[component]
fn ZoomSlider(
    data_min: i64,
    data_max: i64,
    zoom_min: i64,
    zoom_max: i64,
    offset_dates: Vec<(i64, String)>,
    on_change: EventHandler<(i64, i64)>,
) -> Element {
    let slider_width = CHART_WIDTH - CHART_PADDING_LEFT - CHART_PADDING_RIGHT;
    let range = (data_max - data_min).max(1) as f64;

    let left_x = CHART_PADDING_LEFT + (zoom_min - data_min) as f64 / range * slider_width;
    let right_x = CHART_PADDING_LEFT + (zoom_max - data_min) as f64 / range * slider_width;

    // Find dates for handle labels
    let left_date = offset_dates.iter()
        .find(|(o, _)| *o >= zoom_min)
        .map(|(_, d)| &d[..10.min(d.len())])
        .unwrap_or("");
    let right_date = offset_dates.iter()
        .rfind(|(o, _)| *o <= zoom_max)
        .map(|(_, d)| &d[..10.min(d.len())])
        .unwrap_or("");

    // Mouse drag state
    let mut dragging = use_signal(|| Option::<&'static str>::None); // "left", "right", or None
    let mut drag_start_x = use_signal(|| 0.0f64);
    let mut drag_start_val = use_signal(|| 0i64);

    let on_bg_click = {
        move |evt: MouseEvent| {
            // Click on track: move nearest handle to click position
            let coords = evt.client_coordinates();
            let frac = (coords.x - CHART_PADDING_LEFT as f64) / slider_width;
            let offset = data_min + (frac * range) as i64;
            let offset = offset.clamp(data_min, data_max);

            let dist_left = (offset - zoom_min).abs();
            let dist_right = (offset - zoom_max).abs();
            if dist_left <= dist_right {
                on_change.call((offset, zoom_max));
            } else {
                on_change.call((zoom_min, offset));
            }
        }
    };

    rsx! {
        svg {
            class: "zoom-slider",
            view_box: format!("0 0 {CHART_WIDTH} 50"),
            preserve_aspect_ratio: "xMidYMid meet",
            onclick: on_bg_click,

            // Track background
            rect {
                x: CHART_PADDING_LEFT,
                y: 18.0,
                width: slider_width,
                height: 6.0,
                rx: 3.0,
                fill: "#0f3460",
            }

            // Active range
            rect {
                x: left_x,
                y: 18.0,
                width: (right_x - left_x).max(2.0),
                height: 6.0,
                fill: "#4A90D9",
                rx: 3.0,
            }

            // Left handle
            circle {
                cx: left_x,
                cy: 21.0,
                r: 8.0,
                fill: "#4A90D9",
                stroke: "#fff",
                stroke_width: 1.5,
                cursor: "ew-resize",
            }
            text {
                x: left_x,
                y: 42.0,
                text_anchor: "middle",
                font_size: "9px",
                fill: "#888",
                {left_date.to_string()}
            }

            // Right handle
            circle {
                cx: right_x,
                cy: 21.0,
                r: 8.0,
                fill: "#4A90D9",
                stroke: "#fff",
                stroke_width: 1.5,
                cursor: "ew-resize",
            }
            text {
                x: right_x,
                y: 42.0,
                text_anchor: "middle",
                font_size: "9px",
                fill: "#888",
                {right_date.to_string()}
            }
        }
    }
}

fn nice_step(max_val: f64) -> f64 {
    if max_val <= 0.0 {
        return 1.0;
    }
    let rough = max_val / 5.0;
    let magnitude = 10.0f64.powf(rough.log10().floor());
    let residual = rough / magnitude;
    let nice = if residual <= 1.5 {
        1.0
    } else if residual <= 3.5 {
        2.0
    } else if residual <= 7.5 {
        5.0
    } else {
        10.0
    };
    nice * magnitude
}

fn format_value(val: f64) -> String {
    if val == val.floor() {
        format!("{}", val as i64)
    } else {
        format!("{val:.1}")
    }
}
```

- [ ] **Step 2: Register the module**

Add to `ledger-graph-ui/src/components/mod.rs`:

```rust
pub mod analytics_chart;
```

- [ ] **Step 3: Add chart CSS**

Append to `ledger-graph-ui/assets/main.css`:

```css
/* Analytics Chart */
.chart-container {
    display: flex;
    flex-direction: column;
    width: 100%;
    height: 100%;
}

.analytics-chart {
    flex: 1;
    width: 100%;
    min-height: 0;
}

.chart-empty {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
    color: #666;
    font-size: 16px;
}

.zoom-slider {
    width: 100%;
    height: 50px;
    flex-shrink: 0;
}
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo check -p ledger-graph-ui`
Expected: compiles

- [ ] **Step 5: Commit**

```bash
git add ledger-graph-ui/src/components/analytics_chart.rs ledger-graph-ui/src/components/mod.rs ledger-graph-ui/assets/main.css
git commit -m "feat: add SVG line chart component with day bands and zoom slider"
```

---

### Task 7: Main analytics tab component (wiring it all together)

**Files:**
- Create: `ledger-graph-ui/src/components/analytics.rs`
- Modify: `ledger-graph-ui/src/components/mod.rs`
- Modify: `ledger-graph-ui/src/components/app.rs`
- Modify: `ledger-graph-ui/assets/main.css`

- [ ] **Step 1: Create analytics.rs**

```rust
// ledger-graph-ui/src/components/analytics.rs
use crate::components::analytics_chart::AnalyticsChart;
use crate::components::analytics_queries::AnalyticsQueriesPanel;
use crate::models::analytics::AnalyticsQuery;
use crate::server::analytics::{
    delete_analytics_query, get_offset_dates, load_analytics_queries, run_analytics_query,
    save_analytics_query,
};
use dioxus::prelude::*;
use std::collections::{HashMap, HashSet};

#[component]
pub fn Analytics() -> Element {
    let mut query_results: Signal<HashMap<String, Vec<(i64, f64)>>> =
        use_signal(HashMap::new);
    let mut offset_dates: Signal<Vec<(i64, String)>> = use_signal(Vec::new);
    let mut active_queries: Signal<HashSet<String>> = use_signal(HashSet::new);
    let mut zoom_range: Signal<Option<(i64, i64)>> = use_signal(|| None);
    let mut loading_queries: Signal<HashSet<String>> = use_signal(HashSet::new);
    let mut query_errors: Signal<HashMap<String, String>> = use_signal(HashMap::new);
    let mut saved_queries: Signal<Vec<AnalyticsQuery>> = use_signal(Vec::new);
    let mut dates_loaded = use_signal(|| false);

    // Load saved queries on mount
    let _load = use_future(move || async move {
        match load_analytics_queries().await {
            Ok(queries) => saved_queries.set(queries),
            Err(e) => tracing::error!("Failed to load analytics queries: {e}"),
        }
    });

    let on_toggle = move |label: String| {
        let mut active = active_queries.write();
        if active.contains(&label) {
            active.remove(&label);
            drop(active);
            // Remove cached results
            query_results.write().remove(&label);
            query_errors.write().remove(&label);
        } else {
            active.insert(label.clone());
            drop(active);

            // Fetch offset dates if not yet loaded
            if !*dates_loaded.read() {
                dates_loaded.set(true);
                spawn(async move {
                    match get_offset_dates().await {
                        Ok(dates) => offset_dates.set(dates),
                        Err(e) => tracing::error!("Failed to load offset dates: {e}"),
                    }
                });
            }

            // Fetch query data
            let query = saved_queries
                .read()
                .iter()
                .find(|q| q.label == label)
                .cloned();
            if let Some(q) = query {
                loading_queries.write().insert(label.clone());
                query_errors.write().remove(&label);
                let label_done = label.clone();
                spawn(async move {
                    match run_analytics_query(q.cypher, q.min_time, q.max_time).await {
                        Ok(data) => {
                            query_results.write().insert(label_done.clone(), data);
                        }
                        Err(e) => {
                            query_errors
                                .write()
                                .insert(label_done.clone(), format!("{e}"));
                        }
                    }
                    loading_queries.write().remove(&label_done);
                });
            }
        }
    };

    let on_delete = move |label: String| {
        let label_clone = label.clone();
        spawn(async move {
            match delete_analytics_query(label_clone.clone()).await {
                Ok(()) => {
                    // Remove from saved, active, results
                    saved_queries.write().retain(|q| q.label != label_clone);
                    active_queries.write().remove(&label_clone);
                    query_results.write().remove(&label_clone);
                    query_errors.write().remove(&label_clone);
                }
                Err(e) => tracing::error!("Failed to delete query: {e}"),
            }
        });
    };

    let on_save = move |(label, cypher, min_time, max_time): (
        String,
        String,
        Option<String>,
        Option<String>,
    )| {
        let label_clone = label.clone();
        let cypher_clone = cypher.clone();
        let min_clone = min_time.clone();
        let max_clone = max_time.clone();
        spawn(async move {
            match save_analytics_query(
                label_clone.clone(),
                cypher_clone,
                min_clone.clone(),
                max_clone.clone(),
            )
            .await
            {
                Ok(()) => {
                    // Reload queries
                    match load_analytics_queries().await {
                        Ok(queries) => saved_queries.set(queries),
                        Err(e) => tracing::error!("Failed to reload queries: {e}"),
                    }
                    // Auto-activate
                    on_toggle(label_clone);
                }
                Err(e) => {
                    query_errors
                        .write()
                        .insert(label_clone, format!("{e}"));
                }
            }
        });
    };

    let on_refresh = move |_| {
        // Re-fetch offset dates
        spawn(async move {
            match get_offset_dates().await {
                Ok(dates) => offset_dates.set(dates),
                Err(e) => tracing::error!("Failed to refresh offset dates: {e}"),
            }
        });

        // Re-fetch all active queries
        let active = active_queries.read().clone();
        let queries = saved_queries.read().clone();
        for label in active {
            if let Some(q) = queries.iter().find(|q| q.label == label) {
                let q = q.clone();
                let label_done = label.clone();
                loading_queries.write().insert(label.clone());
                query_errors.write().remove(&label);
                spawn(async move {
                    match run_analytics_query(q.cypher, q.min_time, q.max_time).await {
                        Ok(data) => {
                            query_results.write().insert(label_done.clone(), data);
                        }
                        Err(e) => {
                            query_errors
                                .write()
                                .insert(label_done.clone(), format!("{e}"));
                        }
                    }
                    loading_queries.write().remove(&label_done);
                });
            }
        }
    };

    let on_reset_zoom = move |_| {
        zoom_range.set(None);
    };

    let on_zoom_change = move |(min, max): (i64, i64)| {
        zoom_range.set(Some((min, max)));
    };

    let on_download_csv = move |_| {
        let results = query_results.read().clone();
        let dates = offset_dates.read().clone();
        let active = active_queries.read().clone();
        let queries = saved_queries.read().clone();
        let zoom = *zoom_range.read();

        let csv = build_csv(&results, &dates, &active, &queries, zoom);
        download_csv(&csv);
    };

    // Count visible points per series for legend
    let zoom = *zoom_range.read();

    rsx! {
        div { class: "analytics-left-panel",
            AnalyticsQueriesPanel {
                saved_queries: saved_queries.read().clone(),
                active_queries,
                loading_queries,
                query_errors,
                on_toggle: on_toggle,
                on_delete: on_delete,
                on_save: on_save,
            }
        }
        div { class: "analytics-center-panel",
            AnalyticsChart {
                query_results: query_results.read().clone(),
                offset_dates: offset_dates.read().clone(),
                active_queries: active_queries.read().clone(),
                saved_queries: saved_queries.read().clone(),
                zoom_range: zoom,
                on_zoom_change: on_zoom_change,
            }
        }
        div { class: "analytics-right-panel",
            h3 { "Controls" }
            div { class: "analytics-buttons",
                button { class: "analytics-btn", onclick: on_refresh, "Refresh" }
                button { class: "analytics-btn", onclick: on_reset_zoom, "Reset Zoom" }
                button { class: "analytics-btn", onclick: on_download_csv, "Download CSV" }
            }
            h4 { "Legend" }
            div { class: "analytics-legend",
                for (idx, query) in saved_queries.read().iter().enumerate() {
                    {
                        let active = active_queries.read();
                        if active.contains(&query.label) {
                            let color = crate::components::analytics_queries::color_for_index(idx);
                            let count = query_results
                                .read()
                                .get(&query.label)
                                .map(|d| {
                                    d.iter()
                                        .filter(|(o, _)| {
                                            zoom.map_or(true, |(min, max)| *o >= min && *o <= max)
                                        })
                                        .count()
                                })
                                .unwrap_or(0);
                            rsx! {
                                div {
                                    class: "legend-row",
                                    span {
                                        class: "legend-dot",
                                        style: "background: {color};",
                                    }
                                    span { class: "legend-label", "{query.label}" }
                                    span { class: "legend-count", "({count})" }
                                }
                            }
                        } else {
                            rsx! {}
                        }
                    }
                }
            }
        }
    }
}

fn build_csv(
    results: &HashMap<String, Vec<(i64, f64)>>,
    dates: &[(i64, String)],
    active: &HashSet<String>,
    queries: &[AnalyticsQuery],
    zoom: Option<(i64, i64)>,
) -> String {
    let date_map: HashMap<i64, &str> = dates.iter().map(|(o, d)| (*o, d.as_str())).collect();

    // Active query labels in saved order
    let labels: Vec<&str> = queries
        .iter()
        .filter(|q| active.contains(&q.label))
        .map(|q| q.label.as_str())
        .collect();

    // Union of all offsets
    let mut all_offsets: Vec<i64> = results
        .iter()
        .filter(|(l, _)| active.contains(l.as_str()))
        .flat_map(|(_, data)| data.iter().map(|(o, _)| *o))
        .collect();
    all_offsets.sort();
    all_offsets.dedup();

    // Filter by zoom
    if let Some((min, max)) = zoom {
        all_offsets.retain(|o| *o >= min && *o <= max);
    }

    // Build lookup: (label, offset) → value
    let mut lookup: HashMap<(&str, i64), f64> = HashMap::new();
    for (label, data) in results {
        if active.contains(label.as_str()) {
            for &(o, v) in data {
                lookup.insert((label.as_str(), o), v);
            }
        }
    }

    let mut csv = String::from("offset,effective_at");
    for label in &labels {
        csv.push(',');
        csv.push_str(label);
    }
    csv.push('\n');

    for &offset in &all_offsets {
        csv.push_str(&offset.to_string());
        csv.push(',');
        csv.push_str(date_map.get(&offset).unwrap_or(&""));
        for label in &labels {
            csv.push(',');
            if let Some(val) = lookup.get(&(*label, offset)) {
                if *val == val.floor() {
                    csv.push_str(&(*val as i64).to_string());
                } else {
                    csv.push_str(&format!("{val:.2}"));
                }
            }
        }
        csv.push('\n');
    }

    csv
}

fn download_csv(csv: &str) {
    #[cfg(target_arch = "wasm32")]
    {
        use web_sys::{Blob, BlobPropertyBag, HtmlAnchorElement, Url};
        let window = web_sys::window().unwrap();
        let document = window.document().unwrap();

        let array = js_sys::Array::new();
        array.push(&wasm_bindgen::JsValue::from_str(csv));
        let mut opts = BlobPropertyBag::new();
        opts.type_("text/csv");
        let blob = Blob::new_with_str_sequence_and_options(&array, &opts).unwrap();
        let url = Url::create_object_url_with_blob(&blob).unwrap();

        let a: HtmlAnchorElement = document
            .create_element("a")
            .unwrap()
            .dyn_into()
            .unwrap();
        a.set_href(&url);
        a.set_download("analytics.csv");
        a.click();
        Url::revoke_object_url(&url).unwrap();
    }
}
```

- [ ] **Step 2: Register the module**

Add to `ledger-graph-ui/src/components/mod.rs`:

```rust
pub mod analytics;
```

So the full file becomes:

```rust
pub mod analytics;
pub mod analytics_chart;
pub mod analytics_queries;
pub mod app;
pub mod graph_canvas;
pub mod graph_edge;
pub mod graph_node;
pub mod query_editor;
pub mod sidebar;
pub mod toolbar;
```

- [ ] **Step 3: Wire analytics into app.rs**

Replace the Analytics placeholder in `app.rs`:

```rust
// Add import at top
use crate::components::analytics::Analytics;
```

Replace the analytics tab conditional:

```rust
if *active_tab.read() == ActiveTab::Analytics {
    div { class: "main-content",
        Analytics {}
    }
}
```

- [ ] **Step 4: Add analytics panel CSS**

Append to `ledger-graph-ui/assets/main.css`:

```css
/* Analytics panels */
.analytics-left-panel {
    width: 280px;
    min-width: 240px;
    background: #16213e;
    border-right: 1px solid #0f3460;
    overflow-y: auto;
    flex-shrink: 0;
}

.analytics-center-panel {
    flex: 1;
    overflow: hidden;
    min-width: 0;
    display: flex;
    flex-direction: column;
}

.analytics-right-panel {
    width: 280px;
    min-width: 240px;
    background: #16213e;
    border-left: 1px solid #0f3460;
    overflow-y: auto;
    padding: 12px;
    flex-shrink: 0;
}

.analytics-right-panel h3 {
    font-size: 17px;
    margin-bottom: 10px;
    color: #4A90D9;
}

.analytics-right-panel h4 {
    font-size: 14px;
    margin-top: 12px;
    margin-bottom: 6px;
    color: #888;
}

.analytics-buttons {
    display: flex;
    flex-direction: column;
    gap: 6px;
}

.analytics-btn {
    padding: 6px 12px;
    background: #0f3460;
    color: #e0e0e0;
    border: 1px solid #1a1a5e;
    border-radius: 4px;
    cursor: pointer;
    font-size: 13px;
}

.analytics-btn:hover {
    background: #1a4a8e;
}

.analytics-legend {
    display: flex;
    flex-direction: column;
    gap: 4px;
}

.legend-row {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
}

.legend-dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    flex-shrink: 0;
}

.legend-label {
    color: #ccc;
    flex: 1;
}

.legend-count {
    color: #888;
    font-size: 11px;
}
```

- [ ] **Step 5: Verify it compiles**

Run: `cargo check -p ledger-graph-ui`
Expected: compiles (the `download_csv` function uses `web_sys` which is only available on `wasm32` — the `#[cfg(target_arch = "wasm32")]` guard handles this)

- [ ] **Step 6: Commit**

```bash
git add ledger-graph-ui/src/components/analytics.rs ledger-graph-ui/src/components/mod.rs ledger-graph-ui/src/components/app.rs ledger-graph-ui/assets/main.css
git commit -m "feat: add analytics tab with chart, query panel, controls, and CSV export"
```

---

### Task 8: Add web-sys dependency and update .gitignore

**Files:**
- Modify: `ledger-graph-ui/Cargo.toml`
- Modify: `.gitignore`

- [ ] **Step 1: Add web-sys and js-sys to Cargo.toml**

Add to `[dependencies]` in `ledger-graph-ui/Cargo.toml`:

```toml
[target.'cfg(target_arch = "wasm32")'.dependencies]
web-sys = { version = "0.3", features = ["Blob", "BlobPropertyBag", "Url", "HtmlAnchorElement", "HtmlElement", "Document", "Window"] }
js-sys = "0.3"
wasm-bindgen = "0.2"
```

- [ ] **Step 2: Add analytics-queries.local.toml to .gitignore**

Add to `.gitignore`:

```
analytics-queries.local.toml
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check -p ledger-graph-ui`
Expected: compiles

- [ ] **Step 4: Commit**

```bash
git add ledger-graph-ui/Cargo.toml .gitignore
git commit -m "feat: add web-sys dependency for CSV export, gitignore local queries"
```

---

### Task 9: Integration test — run dx serve and verify

**Files:** None (manual verification)

- [ ] **Step 1: Start the app**

```bash
cd ledger-graph-ui && dx serve
```

- [ ] **Step 2: Verify tab bar renders**

Open http://localhost:8080. Expect: "Graph" and "Analytics" tabs visible. Graph tab active by default showing existing graph view.

- [ ] **Step 3: Verify analytics tab**

Click "Analytics" tab. Expect: 3-panel layout with query buttons on left (3 default queries), empty chart in center, controls on right.

- [ ] **Step 4: Verify query execution**

Click "Transactions per offset" toggle. Expect: button activates with color, chart shows data line with day bands.

- [ ] **Step 5: Verify zoom slider**

Click on the zoom slider track. Expect: zoom handles move, chart updates to show zoomed range.

- [ ] **Step 6: Verify CSV download**

Click "Download CSV". Expect: browser downloads `analytics.csv` file with offset, effective_at, and value columns.

- [ ] **Step 7: Verify add query**

Click "+ Add Query", fill label "Test", cypher `MATCH (t:Transaction) RETURN t.offset AS offset, 1 AS value ORDER BY offset`, click Save. Expect: new button appears, `analytics-queries.local.toml` created on disk.

- [ ] **Step 8: Verify delete query**

Hover over "Test" query, click ×. Expect: button removed, query removed from local file.

- [ ] **Step 9: Commit final state**

```bash
git add -A
git commit -m "feat: analytics tab integration verified"
```

---

## Self-Review Checklist

**Spec coverage:**
- [x] Tab bar switching (Task 4)
- [x] AnalyticsQuery model with min_time/max_time (Task 1)
- [x] Default queries TOML (Task 1)
- [x] Server functions: load/save/delete queries (Task 2)
- [x] Server functions: run_analytics_query with time filter (Task 3)
- [x] Server functions: get_offset_dates (Task 3)
- [x] Query validation on save via subquery (Task 2)
- [x] Left panel: query toggles, shared badge, delete, add form (Task 5)
- [x] SVG line chart with day bands + dashed lines (Task 6)
- [x] Polylines per series, color palette (Task 6)
- [x] Y-axis auto-scaling with gridlines (Task 6)
- [x] Legend in chart (Task 6)
- [x] Two-handle zoom slider (Task 6)
- [x] Main analytics component: state, fetching, wiring (Task 7)
- [x] Right panel: Refresh, Reset Zoom, Download CSV, Legend (Task 7)
- [x] CSV export via web-sys blob (Task 7, Task 8)
- [x] .gitignore for local queries (Task 8)
- [x] Color stability by saved_queries position (Task 5 `color_for_index`, used in Task 6 + Task 7)
- [x] Per-query loading/error state (Task 5 + Task 7)
- [x] Zoom preserved on toggle (Task 7 — zoom_range is independent signal)
- [x] Empty states (Task 6)
- [x] Duplicate offset handling: last value wins (implicitly via HashMap in CSV, polyline draws all points)

**Placeholder scan:** No TBDs, TODOs, or "similar to" references found. All steps have complete code.

**Type consistency:** `AnalyticsQuery` struct consistent across Task 1 (definition), Task 2 (server usage), Task 5 (component props), Task 7 (analytics state). `color_for_index` defined in Task 5, used in Task 6 and Task 7. Server function names consistent across Task 2/3 (definition) and Task 7 (imports).
