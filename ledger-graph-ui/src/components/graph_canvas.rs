use crate::components::graph_edge::GraphEdgeView;
use crate::components::graph_node::GraphNodeView;
use crate::models::graph::GraphData;
use crate::state::graph_state::{Selection, Viewport};
use dioxus::prelude::*;

const PADDING: f64 = 80.0;

fn compute_view_box(graph: &GraphData) -> String {
    if graph.nodes.is_empty() {
        return "0 0 800 600".to_string();
    }

    let min_x = graph.nodes.iter().map(|n| n.x).fold(f64::INFINITY, f64::min);
    let max_x = graph.nodes.iter().map(|n| n.x).fold(f64::NEG_INFINITY, f64::max);
    let min_y = graph.nodes.iter().map(|n| n.y).fold(f64::INFINITY, f64::min);
    let max_y = graph.nodes.iter().map(|n| n.y).fold(f64::NEG_INFINITY, f64::max);

    let x = min_x - PADDING;
    let y = min_y - PADDING;
    let w = (max_x - min_x + PADDING * 2.0).max(200.0);
    let h = (max_y - min_y + PADDING * 2.0).max(200.0);

    format!("{x} {y} {w} {h}")
}

#[component]
pub fn GraphCanvas(
    graph: GraphData,
    viewport: Signal<Viewport>,
    selection: Signal<Selection>,
) -> Element {
    let vp = viewport.read();
    let transform = format!(
        "translate({},{}) scale({})",
        vp.offset_x, vp.offset_y, vp.zoom
    );

    let view_box = compute_view_box(&graph);

    // Pan: track mouse drag on background
    let mut dragging = use_signal(|| false);
    let mut last_mouse = use_signal(|| (0.0f64, 0.0f64));

    let on_mouse_down = move |evt: MouseEvent| {
        dragging.set(true);
        let coords = evt.client_coordinates();
        last_mouse.set((coords.x, coords.y));
    };

    let on_mouse_move = {
        let mut viewport = viewport;
        move |evt: MouseEvent| {
            if *dragging.read() {
                let coords = evt.client_coordinates();
                let (lx, ly) = *last_mouse.read();
                let dx = coords.x - lx;
                let dy = coords.y - ly;
                viewport.write().offset_x += dx;
                viewport.write().offset_y += dy;
                last_mouse.set((coords.x, coords.y));
            }
        }
    };

    let on_mouse_up = move |_: MouseEvent| {
        dragging.set(false);
    };

    // Zoom: mouse wheel
    let on_wheel = {
        let mut viewport = viewport;
        move |evt: WheelEvent| {
            let data = evt.data();
            let dy = data.delta().strip_units().y;
            let factor = if dy < 0.0 { 1.1 } else { 1.0 / 1.1 };
            let new_zoom = (viewport.read().zoom * factor).clamp(0.1, 5.0);
            viewport.write().zoom = new_zoom;
        }
    };

    let sel = selection.read().clone();

    // Compute set of node IDs that have an incoming CONSUMES edge
    let consumed_ids: std::collections::HashSet<&str> = graph
        .edges
        .iter()
        .filter(|e| e.rel_type == crate::models::graph::RelType::Consumes)
        .map(|e| e.target.as_str())
        .collect();

    rsx! {
        svg {
            class: "graph-canvas",
            view_box: view_box,
            preserve_aspect_ratio: "xMidYMid meet",
            onmousedown: on_mouse_down,
            onmousemove: on_mouse_move,
            onmouseup: on_mouse_up,
            onwheel: on_wheel,

            // Definitions: drop shadow, selection glow
            defs {
                dangerous_inner_html: "<filter id=\"shadow\" x=\"-20%\" y=\"-20%\" width=\"140%\" height=\"140%\"><feGaussianBlur in=\"SourceAlpha\" stdDeviation=\"3\" result=\"blur\"/><feOffset dx=\"2\" dy=\"2\" result=\"shifted\"/><feFlood flood-color=\"rgba(0,0,0,0.25)\" result=\"color\"/><feComposite in=\"color\" in2=\"shifted\" operator=\"in\" result=\"shadow\"/><feMerge><feMergeNode in=\"shadow\"/><feMergeNode in=\"SourceGraphic\"/></feMerge></filter><filter id=\"glow\" x=\"-30%\" y=\"-30%\" width=\"160%\" height=\"160%\"><feGaussianBlur in=\"SourceAlpha\" stdDeviation=\"4\" result=\"blur\"/><feFlood flood-color=\"gold\" result=\"color\"/><feComposite in=\"color\" in2=\"blur\" operator=\"in\" result=\"glow\"/><feMerge><feMergeNode in=\"glow\"/><feMergeNode in=\"SourceGraphic\"/></feMerge></filter>",
            }

            // Background
            rect {
                width: "10000",
                height: "10000",
                x: "-5000",
                y: "-5000",
                fill: "#f8f9fa",
            }

            g {
                transform: transform,

                // Render edges first (below nodes)
                for edge in graph.edges.iter() {
                    {
                        let source = graph.nodes.iter().find(|n| n.id == edge.source);
                        let target = graph.nodes.iter().find(|n| n.id == edge.target);
                        if let (Some(s), Some(t)) = (source, target) {
                            rsx! {
                                GraphEdgeView {
                                    key: "{edge.id}",
                                    edge: edge.clone(),
                                    source: s.clone(),
                                    target: t.clone(),
                                }
                            }
                        } else {
                            rsx! {}
                        }
                    }
                }

                // Render nodes
                for node in graph.nodes.iter() {
                    {
                        let is_selected = sel.selected_node_id.as_ref() == Some(&node.id);
                        let is_consumed = consumed_ids.contains(node.id.as_str());
                        let mut sel_signal = selection;
                        rsx! {
                            GraphNodeView {
                                key: "{node.id}",
                                node: node.clone(),
                                is_selected: is_selected,
                                is_consumed: is_consumed,
                                on_click: move |id: String| {
                                    sel_signal.write().selected_node_id = Some(id);
                                },
                            }
                        }
                    }
                }
            }
        }
    }
}
