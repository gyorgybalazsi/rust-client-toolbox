use crate::models::graph::{GraphEdge, GraphNode};
use dioxus::prelude::*;

#[component]
pub fn GraphEdgeView(edge: GraphEdge, source: GraphNode, target: GraphNode) -> Element {
    let color = edge.rel_type.stroke_color();
    let dash = edge.rel_type.dash_array();
    let width = edge.rel_type.stroke_width();

    // Calculate midpoint for arrowhead and label
    let mx = (source.x + target.x) / 2.0;
    let my = (source.y + target.y) / 2.0;

    // Direction vector for arrowhead
    let dx = target.x - source.x;
    let dy = target.y - source.y;
    let len = (dx * dx + dy * dy).sqrt().max(1.0);
    let ux = dx / len;
    let uy = dy / len;

    // Arrowhead at midpoint
    let arrow_size = 8.0;
    let ax = mx + ux * arrow_size;
    let ay = my + uy * arrow_size;
    let bx = mx - ux * arrow_size + uy * arrow_size * 0.5;
    let by = my - uy * arrow_size - ux * arrow_size * 0.5;
    let cx = mx - ux * arrow_size - uy * arrow_size * 0.5;
    let cy = my - uy * arrow_size + ux * arrow_size * 0.5;
    let arrow_points = format!("{ax},{ay} {bx},{by} {cx},{cy}");

    rsx! {
        g {
            line {
                x1: source.x,
                y1: source.y,
                x2: target.x,
                y2: target.y,
                stroke: color,
                stroke_width: width,
                stroke_dasharray: dash,
            }
            polygon {
                points: arrow_points,
                fill: color,
            }
            text {
                x: mx,
                y: my - 6.0,
                text_anchor: "middle",
                font_size: "9px",
                fill: "#666",
                {edge.rel_type.display().to_string()}
            }
        }
    }
}
