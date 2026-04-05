use crate::models::analytics::AnalyticsQuery;
use crate::server::analytics::{get_choice_names, get_template_names};
use dioxus::prelude::*;
use std::collections::{HashMap, HashSet};

const COLORS: &[&str] = &[
    "#4A90D9", "#50C878", "#F5A623", "#9B59B6",
    "#E74C3C", "#1ABC9C", "#E67E22", "#3498DB",
];

pub fn color_for_index(idx: usize) -> &'static str {
    COLORS[idx % COLORS.len()]
}

#[component]
pub fn AnalyticsQueriesPanel(
    saved_queries: Vec<AnalyticsQuery>,
    active_queries: Signal<HashSet<String>>,
    loading_queries: Signal<HashSet<String>>,
    query_errors: Signal<HashMap<String, String>>,
    on_toggle: EventHandler<String>,
    on_delete: EventHandler<String>,
    on_save: EventHandler<(String, String, Option<String>, Option<String>)>,
) -> Element {
    let mut show_form = use_signal(|| false);
    let mut new_label = use_signal(String::new);
    let mut new_cypher = use_signal(String::new);
    let mut new_min_time = use_signal(String::new);
    let mut new_max_time = use_signal(String::new);

    let active = active_queries.read();
    let loading = loading_queries.read();
    let errors = query_errors.read();

    rsx! {
        div { class: "analytics-queries",
            h3 { "Queries" }
            for (idx, query) in saved_queries.iter().enumerate() {
                {
                    let label = query.label.clone();
                    let is_active = active.contains(&label);
                    let is_loading = loading.contains(&label);
                    let error = errors.get(&label).cloned();
                    let color = if is_active { color_for_index(idx) } else { "#555" };
                    let label_toggle = label.clone();
                    let label_delete = label.clone();

                    rsx! {
                        div {
                            class: if is_active { "query-toggle active" } else { "query-toggle" },
                            style: "border-left: 3px solid {color};",
                            onclick: move |_| on_toggle.call(label_toggle.clone()),
                            div { class: "query-toggle-content",
                                span { class: "query-label", "{label}" }
                                if query.shared {
                                    span { class: "shared-badge", "shared" }
                                }
                                if is_loading {
                                    span { class: "query-spinner", "..." }
                                }
                                if let Some(ref err) = error {
                                    span { class: "query-error-indicator", title: "{err}", "!" }
                                }
                            }
                            if let (Some(min_t), Some(max_t)) = (&query.min_time, &query.max_time) {
                                div { class: "query-time-range", "{min_t} - {max_t}" }
                            }
                            if !query.shared {
                                button {
                                    class: "query-delete-btn",
                                    onclick: move |evt| {
                                        evt.stop_propagation();
                                        on_delete.call(label_delete.clone());
                                    },
                                    "x"
                                }
                            }
                        }
                    }
                }
            }

            // Template query builders
            TemplateAcsDropdown { on_save: on_save }
            TemplateExercisesDropdown { on_save: on_save }

            if *show_form.read() {
                div { class: "add-query-form",
                    input {
                        class: "query-form-input",
                        r#type: "text",
                        placeholder: "Query label",
                        value: "{new_label}",
                        oninput: move |evt| new_label.set(evt.value()),
                    }
                    textarea {
                        class: "query-form-textarea",
                        rows: 4,
                        placeholder: "Cypher query (must return offset, value columns)",
                        value: "{new_cypher}",
                        oninput: move |evt| new_cypher.set(evt.value()),
                    }
                    div { class: "query-form-time",
                        label { "Min time:" }
                        input {
                            r#type: "datetime-local",
                            class: "query-form-input",
                            value: "{new_min_time}",
                            oninput: move |evt| new_min_time.set(evt.value()),
                        }
                    }
                    div { class: "query-form-time",
                        label { "Max time:" }
                        input {
                            r#type: "datetime-local",
                            class: "query-form-input",
                            value: "{new_max_time}",
                            oninput: move |evt| new_max_time.set(evt.value()),
                        }
                    }
                    button {
                        class: "query-form-save",
                        onclick: move |_| {
                            let label = new_label.read().clone();
                            let cypher = new_cypher.read().clone();
                            let min_t = {
                                let v = new_min_time.read().clone();
                                if v.is_empty() { None } else { Some(v) }
                            };
                            let max_t = {
                                let v = new_max_time.read().clone();
                                if v.is_empty() { None } else { Some(v) }
                            };
                            if !label.is_empty() && !cypher.is_empty() {
                                on_save.call((label, cypher, min_t, max_t));
                                new_label.set(String::new());
                                new_cypher.set(String::new());
                                new_min_time.set(String::new());
                                new_max_time.set(String::new());
                                show_form.set(false);
                            }
                        },
                        "Save"
                    }
                    button {
                        class: "query-form-cancel",
                        onclick: move |_| show_form.set(false),
                        "Cancel"
                    }
                }
            } else {
                button {
                    class: "add-query-btn",
                    onclick: move |_| show_form.set(true),
                    "+ Add Query"
                }
            }
        }
    }
}

fn acs_cypher_for_template(template: &str) -> String {
    format!(
        "MATCH (t:Transaction) WITH t.offset AS offset ORDER BY offset \
         CALL {{ WITH offset \
         MATCH (t2:Transaction)-[:ACTION]->(e)-[:CONSEQUENCE*0..]->(c:Created) \
         WHERE t2.offset <= offset AND c.template_name = '{}' \
         WITH c WHERE NOT EXISTS {{ \
         MATCH (t3:Transaction)-[:ACTION]->(e2)-[:CONSEQUENCE*0..]->(x:Exercised)-[:CONSUMES]->(c) \
         WHERE t3.offset <= offset }} \
         RETURN count(DISTINCT c) AS acs }} \
         RETURN offset, acs AS value ORDER BY offset",
        template.replace('\'', "\\'")
    )
}

#[component]
fn TemplateAcsDropdown(
    on_save: EventHandler<(String, String, Option<String>, Option<String>)>,
) -> Element {
    let mut templates: Signal<Vec<String>> = use_signal(Vec::new);
    let mut selected = use_signal(String::new);

    // Fetch template names on mount
    let _load = use_future(move || async move {
        match get_template_names().await {
            Ok(names) => templates.set(names),
            Err(e) => tracing::error!("Failed to load template names: {e}"),
        }
    });

    let on_select = move |evt: Event<FormData>| {
        let template = evt.value();
        if template.is_empty() {
            return;
        }
        selected.set(template.clone());
        let short_name = template.rsplit('.').next().unwrap_or(&template);
        let label = format!("ACS: {short_name}");
        let cypher = acs_cypher_for_template(&template);
        // Use on_save which reloads queries, saves to local file, and auto-activates
        on_save.call((label, cypher, None, None));
    };

    let tmpl_list = templates.read();

    rsx! {
        div { class: "template-acs-section",
            h4 { "ACS by Template" }
            select {
                class: "template-select",
                value: "{selected}",
                oninput: on_select,
                option { value: "", "Select template..." }
                for tmpl in tmpl_list.iter() {
                    {
                        let short = tmpl.rsplit('.').next().unwrap_or(tmpl);
                        rsx! {
                            option { value: "{tmpl}", "{short}" }
                        }
                    }
                }
            }
        }
    }
}

fn exercises_cypher_for_choice(template: &str, choice: &str) -> String {
    format!(
        "MATCH (t:Transaction)-[:ACTION]->(e)-[:CONSEQUENCE*0..]->(x:Exercised)-[:TARGET]->(c:Created) \
         WHERE c.template_name = '{}' AND x.choice_name = '{}' \
         RETURN t.offset AS offset, count(x) AS value ORDER BY offset",
        template.replace('\'', "\\'"),
        choice.replace('\'', "\\'")
    )
}

#[component]
fn TemplateExercisesDropdown(
    on_save: EventHandler<(String, String, Option<String>, Option<String>)>,
) -> Element {
    let mut templates: Signal<Vec<String>> = use_signal(Vec::new);
    let mut choices: Signal<Vec<String>> = use_signal(Vec::new);
    let mut selected_template = use_signal(String::new);
    let mut selected_choice = use_signal(String::new);

    let _load = use_future(move || async move {
        match get_template_names().await {
            Ok(names) => templates.set(names),
            Err(e) => tracing::error!("Failed to load template names: {e}"),
        }
    });

    let on_template_select = move |evt: Event<FormData>| {
        let template = evt.value();
        selected_template.set(template.clone());
        selected_choice.set(String::new());
        choices.set(Vec::new());
        if template.is_empty() {
            return;
        }
        // Fetch choices for this template
        spawn(async move {
            match get_choice_names(template).await {
                Ok(names) => choices.set(names),
                Err(e) => tracing::error!("Failed to load choice names: {e}"),
            }
        });
    };

    let on_choice_select = move |evt: Event<FormData>| {
        let choice = evt.value();
        if choice.is_empty() {
            return;
        }
        selected_choice.set(choice.clone());
        let template = selected_template.read().clone();
        let short_tmpl = template.rsplit('.').next().unwrap_or(&template).to_string();
        let label = format!("{short_tmpl}:{choice}");
        let cypher = exercises_cypher_for_choice(&template, &choice);
        on_save.call((label, cypher, None, None));
    };

    let tmpl_list = templates.read();
    let choice_list = choices.read();
    let has_template = !selected_template.read().is_empty();

    rsx! {
        div { class: "template-acs-section",
            h4 { "Exercises by Choice" }
            select {
                class: "template-select",
                value: "{selected_template}",
                oninput: on_template_select,
                option { value: "", "Select template..." }
                for tmpl in tmpl_list.iter() {
                    {
                        let short = tmpl.rsplit('.').next().unwrap_or(tmpl);
                        rsx! {
                            option { value: "{tmpl}", "{short}" }
                        }
                    }
                }
            }
            if has_template {
                select {
                    class: "template-select",
                    value: "{selected_choice}",
                    oninput: on_choice_select,
                    option { value: "", "Select choice..." }
                    for choice in choice_list.iter() {
                        {
                            rsx! {
                                option { value: "{choice}", "{choice}" }
                            }
                        }
                    }
                }
            }
        }
    }
}
