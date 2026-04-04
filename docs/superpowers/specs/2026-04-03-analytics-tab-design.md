# Design: Analytics Tab for Ledger Graph UI

**Date**: 2026-04-03 (updated 2026-04-04)
**Branch**: `feat/add-analytics-to-event-graph-ui`
**Status**: Approved

## Goal

Add an analytics tab to `ledger-graph-ui` that displays line charts based on user-defined Cypher queries against the Neo4j event graph. The chart plots numeric values against ledger offsets, with day boundaries marked on the x-axis. Users can overlay multiple data series, zoom into subsets of the data with a two-handle range slider, and download results as CSV.

## Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Chart rendering | Hand-drawn SVG in RSX | Consistent with graph canvas, no external deps, simple requirements |
| Tab switching | `Signal<ActiveTab>` enum | No router needed, pure component state |
| Layout | 3-panel (mirrors Graph tab) | Left: query manager, Center: chart, Right: controls |
| Query storage | Two TOML files on disk | Shared (committed) + local (gitignored) |
| Time range | Per-query `min_time`/`max_time` + client-side zoom slider | Server fetches data within query's time range; slider narrows the view client-side |
| Day markers | Alternating bands + dashed vertical lines | Maximum clarity |
| Query validation | On save, with LIMIT 10 via subquery | Validates shape without running expensive full query |
| CSV export | Client-side from cached data | No server round-trip needed |

## Architecture

### Data Flow

```
User toggles query ON
  → server: run_analytics_query(cypher, min_time, max_time)
    → if min_time/max_time set: queries offset bounds first, runs user query, filters in Rust
    → if neither set: runs user query unfiltered
    → returns Vec<(i64, f64)>
  → cached in Signal<HashMap<String, Vec<(i64, f64)>>>

First query activation also triggers:
  → server: get_offset_dates() → Vec<(i64, String)>
  → cached in Signal

User drags zoom slider handles
  → client filters cached data to the selected offset sub-range
  → chart re-renders (no server call)

User clicks Refresh
  → re-fetches all active queries (with their min/max times) + offset date mapping

User clicks Download CSV
  → client builds CSV from cached data filtered to current zoom range
  → triggers browser download
```

### State (Signals in Analytics component)

```rust
// Cached query results: label → sorted (offset, value) pairs
query_results: Signal<HashMap<String, Vec<(i64, f64)>>>

// Offset → effective_at date string mapping
offset_dates: Signal<Vec<(i64, String)>>

// Which query labels are currently toggled on
active_queries: Signal<HashSet<String>>

// Zoom range: (min_offset, max_offset) within the fetched data
// Defaults to full extent of fetched data. Adjusted by two-handle slider.
// Preserved when toggling queries on/off; only reset on explicit "Reset Zoom".
zoom_range: Signal<Option<(i64, i64)>>

// Per-query loading state: label → true while fetching
loading_queries: Signal<HashSet<String>>

// Per-query error state: label → error message
query_errors: Signal<HashMap<String, String>>

// All saved queries (merged from shared + local files)
saved_queries: Signal<Vec<AnalyticsQuery>>
```

### Server Functions

```rust
/// Run a Cypher query and return (offset, value) tuples.
/// If min_time/max_time are provided, queries offset bounds first,
/// runs the user query unmodified, then filters results in Rust.
/// Extracts `offset` and `value` columns from the result; extra columns ignored.
#[server]
async fn run_analytics_query(
    cypher: String,
    min_time: Option<String>,  // ISO 8601
    max_time: Option<String>,  // ISO 8601
) -> Result<Vec<(i64, f64)>, ServerFnError>

/// Fetch offset → effective_at mapping for all transactions.
/// Used for x-axis day boundary computation and zoom slider date labels.
#[server]
async fn get_offset_dates() -> Result<Vec<(i64, String)>, ServerFnError>
// Cypher: MATCH (t:Transaction) RETURN t.offset AS offset, t.effective_at AS date ORDER BY t.offset

/// Load all saved analytics queries (shared + local, merged).
#[server]
async fn load_analytics_queries() -> Result<Vec<AnalyticsQuery>, ServerFnError>

/// Save a new query to the local TOML file.
/// Validates shape by running the query with LIMIT 10 via subquery wrapper.
#[server]
async fn save_analytics_query(
    label: String,
    cypher: String,
    min_time: Option<String>,
    max_time: Option<String>,
) -> Result<(), ServerFnError>

/// Delete a query from the local TOML file. Shared queries cannot be deleted.
#[server]
async fn delete_analytics_query(label: String) -> Result<(), ServerFnError>
```

### Time Filtering on Server

When `min_time` or `max_time` are set on a query, the server uses a **two-step approach** to filter results:

**Step 1**: Query the offset bounds for the time range:
```cypher
MATCH (t:Transaction)
WHERE t.effective_at >= $min_time AND t.effective_at <= $max_time
RETURN min(t.offset) AS min_off, max(t.offset) AS max_off
```

**Step 2**: Run the user's query unmodified, then filter the returned `Vec<(i64, f64)>` in Rust:
```rust
results.retain(|(offset, _)| *offset >= min_off && *offset <= max_off);
```

This avoids Cypher subquery scoping issues and keeps the user's query untouched.

If only `min_time` or only `max_time` is set, the corresponding bound is omitted from Step 1.

If neither is set, the user's query runs unfiltered and no Step 1 is needed.

**Note on string comparison**: `effective_at` is stored as an ISO 8601 string (`YYYY-MM-DDTHH:MM:SSZ`), not a Neo4j native datetime. String comparison (`>=`, `<=`) works correctly for time ordering because this format sorts lexicographically the same as chronologically, as long as all values use the same format — which they do, since `ledger-explorer` writes them consistently.

## Query Storage

### Two files

- **`analytics-queries.toml`** — shipped defaults + team-shared queries, committed to git
- **`analytics-queries.local.toml`** — user's personal queries, gitignored

Both live in the `ledger-graph-ui/` working directory (where `dx serve` runs).

### Format

```toml
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

Each query can optionally include `min_time` and `max_time`:

```toml
[[query]]
label = "Recent transactions"
cypher = "MATCH (t:Transaction) RETURN t.offset AS offset, 1 AS value ORDER BY offset"
min_time = "2024-06-01T00:00:00Z"
max_time = "2024-06-30T23:59:59Z"
```

Default shipped queries have no `min_time`/`max_time` (meaning "all data").

### AnalyticsQuery struct

```rust
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AnalyticsQuery {
    pub label: String,
    pub cypher: String,
    #[serde(default)]
    pub min_time: Option<String>,
    #[serde(default)]
    pub max_time: Option<String>,
    #[serde(skip)]
    pub shared: bool,  // set at load time, not persisted
}
```

### Behavior

- Server loads both files and merges them. Shared queries are marked as `shared: true`.
- If `analytics-queries.local.toml` doesn't exist, the server treats it as empty (no error). It is created on first save.
- Adding a query via UI saves to `.local` file only.
- Deleting only works on local queries. Shared queries show no delete button.
- UI distinguishes shared vs local with a subtle indicator (e.g., "shared" badge).
- If labels collide between shared and local, local takes precedence (allows overriding defaults).

## Query Contract

Queries must return rows with two named columns:
1. `offset` (integer) — plotted on x-axis
2. `value` (numeric: integer or float) — plotted on y-axis

Column names **must** include `offset` and `value` (use `AS offset`, `AS value` aliases). This is required because the validation subquery references these names explicitly. Extra columns are allowed and ignored.

### Validation

On save, the server wraps the query in a subquery with a limit to validate its shape without running the full query:

```cypher
CALL { <user_cypher> } WITH offset, value LIMIT 10
```

This avoids issues with queries that already contain a `LIMIT` clause. The server checks:
- At least one row returned (warning if zero, but still saves)
- First column (`offset`) is integer-compatible
- Second column (`value`) is numeric-compatible

If validation fails, returns an error message and does not save.

### Duplicate offsets

The query is responsible for aggregation. If duplicate offsets are returned, the chart plots the last value for each offset and shows a warning icon. This is a user error in query design, not a chart bug.

## UI Components

### Tab Bar

Rendered between `.top-bar` and `.main-content` in `App`. Two buttons: "Graph" and "Analytics". Active tab highlighted. Selecting a tab swaps the entire content area.

```
.app-container
├── .top-bar (existing)
├── .tab-bar [NEW]
│   ├── button "Graph" (active by default)
│   └── button "Analytics"
└── .main-content (conditional on active tab)
    ├── [Graph tab]: existing 3-panel layout
    └── [Analytics tab]: analytics 3-panel layout
```

### Left Panel — Query Manager

- **Query toggle buttons**: one per saved query. Click to toggle on/off. Active queries show their series color. Inactive ones are grayed out.
  - Shared queries: "shared" badge, no delete button
  - Local queries: delete button (×) on hover
  - Each button shows the label, and if `min_time`/`max_time` are set, a smaller subtitle with the time range
  - While loading: button shows a spinner icon
  - On error: button shows a red indicator; hovering reveals the error message
- **"+ Add Query" button**: expands a form below:
  - Label text input
  - Cypher textarea (4 rows)
  - Min time: `<input type="datetime-local">` (optional)
  - Max time: `<input type="datetime-local">` (optional)
  - "Save" button → validates + saves to `.local` file, auto-activates

### Center Panel — SVG Line Chart

**Structure:**
- SVG with computed `viewBox` based on the zoom range (not full data extent)
- Background: alternating day bands (`#1e2240` / `#1a1a2e`)
- Dashed vertical lines at day boundaries (`#555`, dasharray `4,4`)
- Y-axis: auto-scaled to max visible value within zoom range, 5-6 horizontal gridlines with value labels
- X-axis: offset tick marks, day labels centered in bands
- One `<polyline>` per active series (only points within zoom range)
- Legend: bottom-right of chart, colored dot + label per series (clickable to toggle)

**Day bands computation:**
1. From `offset_dates`, extract date portion from `effective_at` (first 10 chars: `YYYY-MM-DD`)
2. Group consecutive offsets by date (within zoom range)
3. Map date groups to x-coordinate ranges
4. Alternate band fill colors
5. Offsets without date mapping: no band, show numeric offset only

**Hover tooltip:**
- On mouseover near a data point: show offset, date, value, query label
- Implemented as an SVG `<g>` that follows the cursor position

**Empty state:**
- When no queries are active: centered text "Select a query to display data"
- When queries are active but no data in zoom range: "No data in selected range"

**Color palette** (assigned by query's position in `saved_queries` list, not among active queries — this keeps colors stable when toggling series on/off):
`#4A90D9`, `#50C878`, `#F5A623`, `#9B59B6`, `#E74C3C`, `#1ABC9C`, `#E67E22`, `#3498DB`

### Zoom Slider

A **two-handle range slider** rendered below the chart (or as an overlay at the bottom of the chart area).

- **Full extent**: the min and max offset across all active series' cached data
- **Handles**: left handle = zoom start, right handle = zoom end
- **Default**: both handles at full extent (showing all data)
- **Behavior**: dragging a handle narrows the visible offset range. The chart re-renders instantly (client-side filtering of cached data). Y-axis auto-rescales to the visible data within the zoom window.
- **On query toggle**: the zoom range is preserved at its current absolute offset values. The slider's full extent may grow if the new query brings offsets outside the previous range, but the zoom window stays put. Only "Reset Zoom" expands to full extent.
- **Date labels**: the slider shows dates at the handle positions (looked up from `offset_dates`)
- **Implementation**: a horizontal SVG bar with two draggable circle handles. A filled region between the handles indicates the visible range.

### Right Panel — Controls

- **Buttons**:
  - **Refresh**: re-fetches all active queries (with their per-query min/max times) + offset date mapping from Neo4j
  - **Download CSV**: exports data within current zoom range for all active series
  - **Reset Zoom**: resets slider to full extent

- **Legend** (below buttons):
  - One row per active series: colored dot + label + point count (within zoom range)
  - Click to toggle series off (mirrors left panel toggle)

## CSV Export

**Trigger**: "Download CSV" button in right panel.

**Format**:
```csv
offset,effective_at,Transactions per offset,Created per offset
1,2024-01-15T10:30:00Z,3,5
2,2024-01-15T11:00:00Z,1,
3,2024-01-15T11:30:00Z,,2
```

- Columns: `offset`, `effective_at` (from date mapping), then one column per active query label
- Rows: union of all offsets across active series within the current zoom range, sorted ascending
- Empty cells left blank (not 0) when a series has no data at that offset
- Generated entirely client-side from cached data
- Download triggered via `web-sys` (`Blob`, `Url::create_object_url_with_blob`, `HtmlAnchorElement`) to create a temporary download link and programmatically click it. Requires `web-sys` with features `Blob`, `Url`, `HtmlAnchorElement`, `HtmlElement`, `Document` (or use `gloo-utils` as a convenience wrapper).

## File Changes

### New Files

| File | Purpose |
|------|---------|
| `src/components/analytics.rs` | Main analytics tab: 3-panel layout, state management, zoom filtering, CSV export |
| `src/components/analytics_chart.rs` | SVG line chart: axes, gridlines, polylines, day bands, tooltips, legend, zoom slider |
| `src/components/analytics_queries.rs` | Left panel: query toggle buttons, add/delete form with time range inputs |
| `src/server/analytics.rs` | Server functions: run query with time filter, CRUD queries, fetch offset dates |
| `analytics-queries.toml` | Default shared queries (committed to git) |

### Modified Files

| File | Change |
|------|--------|
| `src/components/app.rs` | Add `ActiveTab` signal, tab bar rendering, conditional content |
| `src/components/mod.rs` | Register `analytics`, `analytics_chart`, `analytics_queries` |
| `src/server/mod.rs` | Register `analytics` |
| `Cargo.toml` | Add `web-sys` with features `Blob`, `Url`, `HtmlAnchorElement`, `HtmlElement`, `Document` (for CSV download) |
| `assets/main.css` | Tab bar, chart, analytics panel, zoom slider styles |
| `.gitignore` | Add `analytics-queries.local.toml` |

## Edge Cases

- **Empty `effective_at`**: Offsets with missing `effective_at` are excluded from date mapping. They appear on the chart without day band coverage, showing numeric offset only.
- **No data returned**: Query returns zero rows → show "No data" message, series line not rendered.
- **Zoom slider with no data**: Slider disabled until at least one query is active and has data. Shows "No data to zoom" placeholder.
- **Large datasets**: No pagination — the chart renders all points as a polyline. SVG handles thousands of points fine. If performance becomes an issue (>10k points), we can downsample later.
- **Label collision** (shared vs local): Local query takes precedence, effectively overriding the shared default.
- **Concurrent fetches**: Each query result updates only its own key in the HashMap. No race condition.
- **Query returns non-numeric data**: Server validation rejects on save. If a previously valid query starts returning bad data (schema change), show error on refresh.
- **Per-query time ranges with zoom**: The server fetches data within `[min_time, max_time]`. The zoom slider operates within that fetched set. Zooming cannot exceed the fetched data — it only narrows the view.
- **Multiple queries with different time ranges**: Each query fetches its own range independently. The zoom slider extent is the union of all fetched offsets. Series with no data in the zoomed region simply show no line.
- **Very large ledgers (>100k transactions)**: `get_offset_dates()` fetches all transactions' offset+date pairs. This is lightweight (two columns) but may be slow on very large ledgers. Consider adding optional time bounds or caching in a follow-up if this becomes an issue.
- **Missing `analytics-queries.local.toml`**: Server returns empty list; file is created on first save.
- **Query fetch failure**: Per-query error state shown on the toggle button. Other active queries are unaffected.
