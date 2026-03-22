use crate::components::graph_canvas::GraphCanvas;
use crate::components::query_editor::QueryEditor;
use crate::components::sidebar::Sidebar;
use crate::components::toolbar::Toolbar;
use crate::models::graph::GraphData;
use crate::state::graph_state::{Selection, Viewport};
use dioxus::prelude::*;

const MAIN_CSS: Asset = asset!("/assets/main.css");

#[component]
pub fn App() -> Element {
    let mut graph = use_signal(GraphData::default);
    let viewport = use_signal(Viewport::default);
    let mut selection = use_signal(Selection::default);

    rsx! {
        document::Stylesheet { href: MAIN_CSS }
        div { class: "app-container",
            div { class: "top-bar",
                h1 { "Ledger Graph UI" }
                Toolbar { viewport }
            }
            div { class: "main-content",
                div { class: "left-panel",
                    QueryEditor {
                        on_result: move |data: GraphData| {
                            graph.set(data);
                            selection.set(Selection::default());
                        },
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
