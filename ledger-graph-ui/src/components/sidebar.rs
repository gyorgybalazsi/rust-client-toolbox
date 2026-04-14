use crate::models::graph::GraphData;
use crate::state::graph_state::Selection;
use dioxus::prelude::*;

/// Ordered property keys. Keys matching these appear first in this order.
/// Keys starting with "create_arg." are grouped together after template_name.
/// Everything else goes to the end.
fn sort_key(key: &str) -> (usize, String) {
    match key {
        "offset" => (0, String::new()),
        "template_name" => (1, String::new()),
        k if k.starts_with("create_arg.") => (2, k.to_string()),
        "signatories" => (3, String::new()),
        "created_at" => (4, String::new()),
        "contract_id" => (5, String::new()),
        "node_id" => (6, String::new()),
        other => (7, other.to_string()),
    }
}

#[component]
pub fn Sidebar(graph: GraphData, selection: Signal<Selection>) -> Element {
    let sel = selection.read();
    let selected_node = sel.selected_node(&graph);

    rsx! {
        div { class: "sidebar",
            h3 { "Node Details" }
            match selected_node {
                Some(node) => {
                    let mut props: Vec<_> = node.properties.iter().collect();
                    props.sort_by(|(a, _), (b, _)| {
                        sort_key(a).cmp(&sort_key(b))
                    });
                    rsx! {
                        div { class: "node-detail",
                            div { class: "node-label",
                                span {
                                    class: "label-badge",
                                    style: "background-color: {node.label.color()}",
                                    {node.label.display()}
                                }
                            }
                            div { class: "node-name", "{node.display_name}" }
                            div { class: "node-id", "ID: {node.id}" }
                            h4 { "Properties" }
                            div { class: "properties",
                                for (key, value) in props {
                                    div { class: "property-row",
                                        span { class: "prop-key", "{key}:" }
                                        span { class: "prop-value", "{value}" }
                                    }
                                }
                            }
                        }
                    }
                },
                None => rsx! {
                    p { class: "no-selection", "Click a node to view its details." }
                },
            }
        }
    }
}
