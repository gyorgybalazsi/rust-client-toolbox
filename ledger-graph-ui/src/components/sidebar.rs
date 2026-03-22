use crate::models::graph::GraphData;
use crate::state::graph_state::Selection;
use dioxus::prelude::*;

#[component]
pub fn Sidebar(graph: GraphData, selection: Signal<Selection>) -> Element {
    let sel = selection.read();
    let selected_node = sel.selected_node(&graph);

    rsx! {
        div { class: "sidebar",
            h3 { "Node Details" }
            match selected_node {
                Some(node) => rsx! {
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
                            for (key, value) in node.properties.iter() {
                                div { class: "property-row",
                                    span { class: "prop-key", "{key}:" }
                                    span { class: "prop-value", "{value}" }
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
