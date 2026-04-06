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
    lasso_active: Signal<bool>,
) -> Element {
    let vp = viewport.read();
    let transform = format!(
        "translate({},{}) scale({})",
        vp.offset_x, vp.offset_y, vp.zoom
    );

    let default_vb = compute_view_box(&graph);
    // Reset custom viewBox when viewport zoom is reset to 1.0
    // (triggered by Reset button in toolbar)
    let mut custom_view_box = use_signal(|| Option::<String>::None);
    if (vp.zoom - 1.0).abs() < 0.001 && vp.offset_x.abs() < 0.001 && vp.offset_y.abs() < 0.001 {
        if custom_view_box.read().is_some() {
            custom_view_box.set(None);
        }
    }
    let view_box = custom_view_box.read().clone().unwrap_or(default_vb.clone());

    // Parse the current viewBox for lasso calculations
    let vb_parts: Vec<f64> = view_box.split_whitespace()
        .filter_map(|s| s.parse().ok())
        .collect();
    let (vb_x, vb_y, vb_w, vb_h) = if vb_parts.len() == 4 {
        (vb_parts[0], vb_parts[1], vb_parts[2], vb_parts[3])
    } else {
        (0.0, 0.0, 800.0, 600.0)
    };

    // Pan: track mouse drag on background
    // Lasso zoom: when lasso_active, drag draws rectangle, zoom to fit on release
    let mut dragging = use_signal(|| false);
    let mut lasso_mode = use_signal(|| false);
    let mut last_mouse = use_signal(|| (0.0f64, 0.0f64));
    let mut lasso_start = use_signal(|| (0.0f64, 0.0f64));
    let mut lasso_end = use_signal(|| (0.0f64, 0.0f64));
    // Store the SVG element's bounding rect at drag start
    let mut svg_rect = use_signal(|| (0.0f64, 0.0f64, 800.0f64, 600.0f64)); // x, y, w, h

    let on_mouse_down = move |evt: MouseEvent| {
        let coords = evt.client_coordinates();
        dragging.set(true);
        lasso_mode.set(*lasso_active.read());
        last_mouse.set((coords.x, coords.y));
        lasso_start.set((coords.x, coords.y));
        lasso_end.set((coords.x, coords.y));

        // Get the SVG element bounds via web_sys (WASM only)
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
                // Try to find the SVG element
                let el = doc.query_selector("svg.graph-canvas").ok().flatten()
                    .or_else(|| doc.query_selector(".graph-canvas").ok().flatten());
                if let Some(el) = el {
                    let rect = el.get_bounding_client_rect();
                    svg_rect.set((rect.x(), rect.y(), rect.width(), rect.height()));
                }
            }
        }
    };

    let on_mouse_move = {
        let mut viewport = viewport;
        move |evt: MouseEvent| {
            if *dragging.read() {
                let coords = evt.client_coordinates();
                if *lasso_mode.read() {
                    // Lasso: update rectangle end
                    lasso_end.set((coords.x, coords.y));
                } else {
                    // Pan
                    let (lx, ly) = *last_mouse.read();
                    let dx = coords.x - lx;
                    let dy = coords.y - ly;
                    viewport.write().offset_x += dx;
                    viewport.write().offset_y += dy;
                    last_mouse.set((coords.x, coords.y));
                }
            }
        }
    };

    let on_mouse_up = move |_: MouseEvent| {
        if *lasso_mode.read() && *dragging.read() {
            let (sx, sy) = *lasso_start.read();
            let (ex, ey) = *lasso_end.read();
            let screen_dx = (ex - sx).abs();
            let screen_dy = (ey - sy).abs();

            if screen_dx > 20.0 && screen_dy > 20.0 {
                let (rect_x, rect_y, rect_w, rect_h) = *svg_rect.read();

                if rect_w > 0.0 && rect_h > 0.0 {
                    // Map screen coords to fractions of the SVG element
                    let frac_x1 = ((sx.min(ex) - rect_x) / rect_w).clamp(0.0, 1.0);
                    let frac_y1 = ((sy.min(ey) - rect_y) / rect_h).clamp(0.0, 1.0);
                    let frac_x2 = ((sx.max(ex) - rect_x) / rect_w).clamp(0.0, 1.0);
                    let frac_y2 = ((sy.max(ey) - rect_y) / rect_h).clamp(0.0, 1.0);

                    // Map fractions to viewBox coordinates
                    let new_vb_x = vb_x + frac_x1 * vb_w;
                    let new_vb_y = vb_y + frac_y1 * vb_h;
                    let new_vb_w = (frac_x2 - frac_x1) * vb_w;
                    let new_vb_h = (frac_y2 - frac_y1) * vb_h;

                    if new_vb_w > 1.0 && new_vb_h > 1.0 {
                        custom_view_box.set(Some(format!(
                            "{new_vb_x} {new_vb_y} {new_vb_w} {new_vb_h}"
                        )));
                    }
                }
                lasso_active.set(false);
            }
        }
        dragging.set(false);
        lasso_mode.set(false);
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

    // Lasso rectangle dimensions (screen coords)
    let is_lasso = *lasso_mode.read() && *dragging.read();
    let (sx, sy) = *lasso_start.read();
    let (ex, ey) = *lasso_end.read();
    let lasso_x = sx.min(ex);
    let lasso_y = sy.min(ey);
    let lasso_w = (ex - sx).abs();
    let lasso_h = (ey - sy).abs();

    rsx! {
        div { class: "graph-canvas-container",
            svg {
                class: if *lasso_active.read() { "graph-canvas lasso-cursor" } else { "graph-canvas" },
                view_box: view_box,
                preserve_aspect_ratio: "none",
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
            if is_lasso {
                div {
                    class: "lasso-rect",
                    style: "left: {lasso_x}px; top: {lasso_y}px; width: {lasso_w}px; height: {lasso_h}px;",
                }
            }
        }
    }
}
