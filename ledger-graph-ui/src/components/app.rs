use crate::components::graph_canvas::GraphCanvas;
use crate::components::query_editor::QueryEditor;
use crate::components::sidebar::Sidebar;
use crate::components::toolbar::Toolbar;
use crate::models::graph::{GraphData, NodeLabel, RelType};
use crate::state::graph_state::{Selection, Viewport};
use dioxus::prelude::*;
use std::collections::{HashMap, HashSet, VecDeque};

const MAIN_CSS: Asset = asset!("/assets/main.css");

/// Given full graph data, compute which node indices belong to each transaction's
/// subtree (BFS via ACTION/CONSEQUENCE edges from transaction root).
fn compute_tx_subtrees(data: &GraphData) -> Vec<(usize, Vec<usize>)> {
    let id_to_idx: HashMap<&str, usize> = data
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.as_str(), i))
        .collect();

    // Build tree adjacency: source -> children (ACTION/CONSEQUENCE only)
    let mut children: HashMap<usize, Vec<usize>> = HashMap::new();
    let mut has_parent: HashSet<usize> = HashSet::new();
    for edge in &data.edges {
        if !matches!(edge.rel_type, RelType::Action | RelType::Consequence) {
            continue;
        }
        if let (Some(&src), Some(&tgt)) =
            (id_to_idx.get(edge.source.as_str()), id_to_idx.get(edge.target.as_str()))
        {
            children.entry(src).or_default().push(tgt);
            has_parent.insert(tgt);
        }
    }

    // Find transaction roots (no incoming ACTION/CONSEQUENCE), sorted by offset
    let mut tx_roots: Vec<usize> = data
        .nodes
        .iter()
        .enumerate()
        .filter(|(i, n)| n.label == NodeLabel::Transaction && !has_parent.contains(i))
        .map(|(i, _)| i)
        .collect();

    tx_roots.sort_by(|&a, &b| {
        let oa = data.nodes[a]
            .properties
            .get("offset")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let ob = data.nodes[b]
            .properties
            .get("offset")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        oa.cmp(&ob)
    });

    // BFS each root to collect its subtree
    let mut result = Vec::new();
    for root in tx_roots {
        let mut subtree = Vec::new();
        let mut queue = VecDeque::new();
        let mut visited = HashSet::new();
        queue.push_back(root);
        visited.insert(root);
        while let Some(idx) = queue.pop_front() {
            subtree.push(idx);
            if let Some(kids) = children.get(&idx) {
                for &kid in kids {
                    if visited.insert(kid) {
                        queue.push_back(kid);
                    }
                }
            }
        }
        result.push((root, subtree));
    }
    result
}

/// Compute the visible subgraph at a given step of the replay.
/// Step N means transactions 0..=N are visible, plus all Party nodes.
/// Edges are included if both endpoints are visible.
fn subgraph_at_step(data: &GraphData, tx_subtrees: &[(usize, Vec<usize>)], step: usize) -> GraphData {
    let mut visible_indices: HashSet<usize> = HashSet::new();

    // Always include Party nodes
    for (i, n) in data.nodes.iter().enumerate() {
        if n.label == NodeLabel::Party {
            visible_indices.insert(i);
        }
    }

    // Include subtrees for transactions 0..=step
    for (_, subtree) in tx_subtrees.iter().take(step + 1) {
        for &idx in subtree {
            visible_indices.insert(idx);
        }
    }

    let visible_ids: HashSet<&str> = visible_indices
        .iter()
        .map(|&i| data.nodes[i].id.as_str())
        .collect();

    let nodes: Vec<_> = visible_indices
        .iter()
        .map(|&i| data.nodes[i].clone())
        .collect();

    let edges: Vec<_> = data
        .edges
        .iter()
        .filter(|e| visible_ids.contains(e.source.as_str()) && visible_ids.contains(e.target.as_str()))
        .cloned()
        .collect();

    GraphData { nodes, edges }
}

#[component]
pub fn App() -> Element {
    let mut graph = use_signal(GraphData::default);
    let viewport = use_signal(Viewport::default);
    let mut selection = use_signal(Selection::default);

    // Replay state
    let mut full_data = use_signal(GraphData::default);
    let mut replay_step = use_signal(|| Option::<usize>::None); // None = not replaying
    let mut replay_total = use_signal(|| 0usize);
    let mut auto_replay = use_signal(|| false); // true = auto-advance, false = manual step

    // Replay timer: when auto_replay and replay_step is Some, advance every 3s
    let _replay_timer = use_future(move || async move {
        loop {
            gloo_timers::future::TimeoutFuture::new(3000).await;
            if !*auto_replay.read() {
                continue;
            }
            let step = *replay_step.read();
            let total = *replay_total.read();
            if let Some(s) = step {
                if s < total.saturating_sub(1) {
                    let next = s + 1;
                    replay_step.set(Some(next));
                    let data = full_data.read().clone();
                    let subtrees = compute_tx_subtrees(&data);
                    let sub = subgraph_at_step(&data, &subtrees, next);
                    graph.set(sub);
                } else {
                    // Replay finished
                    replay_step.set(None);
                    auto_replay.set(false);
                }
            }
        }
    });

    // Advance one step manually
    let advance_step = move || {
        let step = *replay_step.read();
        let total = *replay_total.read();
        if let Some(s) = step {
            if s < total.saturating_sub(1) {
                let next = s + 1;
                replay_step.set(Some(next));
                let data = full_data.read().clone();
                let subtrees = compute_tx_subtrees(&data);
                let sub = subgraph_at_step(&data, &subtrees, next);
                graph.set(sub);
            } else {
                replay_step.set(None);
                auto_replay.set(false);
            }
        }
    };

    let on_result = move |data: GraphData| {
        replay_step.set(None);
        auto_replay.set(false);
        graph.set(data.clone());
        full_data.set(data);
        selection.set(Selection::default());
    };

    let mut start_replay = move |data: GraphData, auto: bool| {
        let subtrees = compute_tx_subtrees(&data);
        let total = subtrees.len();
        if total == 0 {
            graph.set(data.clone());
            full_data.set(data);
            return;
        }
        full_data.set(data.clone());
        replay_total.set(total);
        auto_replay.set(auto);
        let sub = subgraph_at_step(&data, &subtrees, 0);
        graph.set(sub);
        selection.set(Selection::default());
        replay_step.set(Some(0));
    };

    let on_replay = move |data: GraphData| {
        start_replay(data, true);
    };

    let on_step_start = move |data: GraphData| {
        start_replay(data, false);
    };

    let mut advance_step = advance_step;
    let on_step_next = move |_: ()| {
        advance_step();
    };

    let is_replaying = replay_step.read().is_some();
    let step_display = (*replay_step.read()).map(|s| s + 1).unwrap_or(0);
    let total_display = *replay_total.read();

    rsx! {
        document::Stylesheet { href: MAIN_CSS }
        div { class: "app-container",
            div { class: "top-bar",
                h1 { "Ledger Graph UI" }
                div { class: "top-bar-right",
                    if is_replaying {
                        span { class: "replay-status", "Replaying {step_display}/{total_display}" }
                    }
                    Toolbar { viewport }
                }
            }
            div { class: "main-content",
                div { class: "left-panel",
                    QueryEditor {
                        on_result: on_result,
                        on_replay: on_replay,
                        on_step_start: on_step_start,
                        on_step_next: on_step_next,
                        is_stepping: is_replaying && !*auto_replay.read(),
                    }
                }
                div { class: "center-panel",
                    GraphCanvas {
                        graph: graph.read().clone(),
                        viewport,
                        selection,
                    }
                }
                div { class: "right-panel",
                    Sidebar {
                        graph: graph.read().clone(),
                        selection,
                    }
                }
            }
        }
    }
}
