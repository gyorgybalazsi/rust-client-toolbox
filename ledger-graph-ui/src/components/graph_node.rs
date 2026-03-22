use crate::models::graph::{GraphNode, NodeLabel};
use dioxus::prelude::*;

const NODE_RADIUS: f64 = 20.0;

#[component]
pub fn GraphNodeView(
    node: GraphNode,
    is_selected: bool,
    on_click: EventHandler<String>,
) -> Element {
    let color = node.label.color();
    let stroke = if is_selected { "#FFD700" } else { "#fff" };
    let stroke_width = if is_selected { 3.0 } else { 1.5 };
    let id = node.id.clone();

    match node.label {
        NodeLabel::Transaction => {
            // Rounded rectangle
            let half = NODE_RADIUS;
            rsx! {
                g {
                    onclick: move |_| on_click.call(id.clone()),
                    cursor: "pointer",
                    rect {
                        x: node.x - half,
                        y: node.y - half,
                        width: half * 2.0,
                        height: half * 2.0,
                        rx: 6.0,
                        fill: color,
                        stroke: stroke,
                        stroke_width: stroke_width,
                    }
                    text {
                        x: node.x,
                        y: node.y + 35.0,
                        text_anchor: "middle",
                        font_size: "11px",
                        fill: "#333",
                        {node.display_name.clone()}
                    }
                }
            }
        }
        NodeLabel::Created => {
            // Circle
            rsx! {
                g {
                    onclick: move |_| on_click.call(id.clone()),
                    cursor: "pointer",
                    circle {
                        cx: node.x,
                        cy: node.y,
                        r: NODE_RADIUS,
                        fill: color,
                        stroke: stroke,
                        stroke_width: stroke_width,
                    }
                    text {
                        x: node.x,
                        y: node.y + 35.0,
                        text_anchor: "middle",
                        font_size: "11px",
                        fill: "#333",
                        {node.display_name.clone()}
                    }
                }
            }
        }
        NodeLabel::Exercised => {
            // Diamond (rotated square)
            let r = NODE_RADIUS;
            let points = format!(
                "{},{} {},{} {},{} {},{}",
                node.x, node.y - r,
                node.x + r, node.y,
                node.x, node.y + r,
                node.x - r, node.y
            );
            rsx! {
                g {
                    onclick: move |_| on_click.call(id.clone()),
                    cursor: "pointer",
                    polygon {
                        points: points,
                        fill: color,
                        stroke: stroke,
                        stroke_width: stroke_width,
                    }
                    text {
                        x: node.x,
                        y: node.y + 35.0,
                        text_anchor: "middle",
                        font_size: "11px",
                        fill: "#333",
                        {node.display_name.clone()}
                    }
                }
            }
        }
        NodeLabel::Party => {
            // Hexagon
            let r = NODE_RADIUS;
            let mut pts = String::new();
            for i in 0..6 {
                let angle = std::f64::consts::PI / 3.0 * (i as f64) - std::f64::consts::PI / 6.0;
                let px = node.x + r * angle.cos();
                let py = node.y + r * angle.sin();
                if i > 0 {
                    pts.push(' ');
                }
                pts.push_str(&format!("{px},{py}"));
            }
            rsx! {
                g {
                    onclick: move |_| on_click.call(id.clone()),
                    cursor: "pointer",
                    polygon {
                        points: pts,
                        fill: color,
                        stroke: stroke,
                        stroke_width: stroke_width,
                    }
                    text {
                        x: node.x,
                        y: node.y + 35.0,
                        text_anchor: "middle",
                        font_size: "11px",
                        fill: "#333",
                        {node.display_name.clone()}
                    }
                }
            }
        }
    }
}
