use crate::models::graph::GraphData;
use crate::server::queries::run_cypher;
use dioxus::prelude::*;
use std::collections::HashMap;

#[component]
pub fn QueryEditor(on_result: EventHandler<GraphData>) -> Element {
    let mut cypher = use_signal(|| {
        "MATCH (t:Transaction)-[r]->(e) RETURN t, r, e LIMIT 50".to_string()
    });
    let mut error = use_signal(|| Option::<String>::None);
    let mut loading = use_signal(|| false);

    let execute = move |_| {
        let query = cypher.read().clone();
        loading.set(true);
        error.set(None);
        spawn(async move {
            match run_cypher(query, HashMap::new()).await {
                Ok(data) => {
                    on_result.call(data);
                }
                Err(e) => {
                    error.set(Some(format!("{e}")));
                }
            }
            loading.set(false);
        });
    };

    let is_loading = *loading.read();

    rsx! {
        div { class: "query-editor",
            h3 { "Cypher Query" }
            textarea {
                class: "cypher-input",
                rows: 4,
                value: "{cypher}",
                oninput: move |evt| cypher.set(evt.value()),
            }
            div { class: "query-actions",
                button {
                    class: "execute-btn",
                    disabled: is_loading,
                    onclick: execute,
                    if is_loading { "Running..." } else { "Execute" }
                }
            }
            if let Some(err) = error.read().as_ref() {
                div { class: "query-error", "{err}" }
            }
        }
    }
}
