use crate::components::analytics_chart::AnalyticsChart;
use crate::components::analytics_queries::AnalyticsQueriesPanel;
use crate::models::analytics::AnalyticsQuery;
use crate::server::analytics::{
    delete_analytics_query, get_offset_dates, load_analytics_queries, run_analytics_query,
    save_analytics_query,
};
use dioxus::prelude::*;
use std::collections::{HashMap, HashSet};

#[component]
pub fn Analytics() -> Element {
    let mut query_results: Signal<HashMap<String, Vec<(i64, f64)>>> =
        use_signal(HashMap::new);
    let mut offset_dates: Signal<Vec<(i64, String)>> = use_signal(Vec::new);
    let mut active_queries: Signal<HashSet<String>> = use_signal(HashSet::new);
    let mut zoom_range: Signal<Option<(i64, i64)>> = use_signal(|| None);
    let mut loading_queries: Signal<HashSet<String>> = use_signal(HashSet::new);
    let mut query_errors: Signal<HashMap<String, String>> = use_signal(HashMap::new);
    let mut saved_queries: Signal<Vec<AnalyticsQuery>> = use_signal(Vec::new);
    let mut dates_loaded = use_signal(|| false);

    // Load saved queries on mount
    let _load = use_future(move || async move {
        match load_analytics_queries().await {
            Ok(queries) => saved_queries.set(queries),
            Err(e) => tracing::error!("Failed to load analytics queries: {e}"),
        }
    });

    // Helper: activate a query by label (fetch data, update signals).
    // Defined as a plain function-like closure that takes all needed signals explicitly,
    // so it can be called from both on_toggle and on_save without borrowing conflicts.
    let mut activate_query = move |label: String| {
        if !*dates_loaded.read() {
            dates_loaded.set(true);
            spawn(async move {
                match get_offset_dates().await {
                    Ok(dates) => offset_dates.set(dates),
                    Err(e) => tracing::error!("Failed to load offset dates: {e}"),
                }
            });
        }

        let query = saved_queries
            .read()
            .iter()
            .find(|q| q.label == label)
            .cloned();
        if let Some(q) = query {
            loading_queries.write().insert(label.clone());
            query_errors.write().remove(&label);
            let label_done = label.clone();
            spawn(async move {
                match run_analytics_query(q.cypher, q.min_time, q.max_time).await {
                    Ok(data) => {
                        query_results.write().insert(label_done.clone(), data);
                    }
                    Err(e) => {
                        query_errors
                            .write()
                            .insert(label_done.clone(), format!("{e}"));
                    }
                }
                loading_queries.write().remove(&label_done);
            });
        }
    };

    let on_toggle = move |label: String| {
        let mut active = active_queries.write();
        if active.contains(&label) {
            active.remove(&label);
            drop(active);
            query_results.write().remove(&label);
            query_errors.write().remove(&label);
        } else {
            active.insert(label.clone());
            drop(active);
            activate_query(label);
        }
    };

    let on_delete = move |label: String| {
        let label_clone = label.clone();
        spawn(async move {
            match delete_analytics_query(label_clone.clone()).await {
                Ok(()) => {
                    saved_queries.write().retain(|q| q.label != label_clone);
                    active_queries.write().remove(&label_clone);
                    query_results.write().remove(&label_clone);
                    query_errors.write().remove(&label_clone);
                }
                Err(e) => tracing::error!("Failed to delete query: {e}"),
            }
        });
    };

    let on_save = move |(label, cypher, min_time, max_time): (
        String,
        String,
        Option<String>,
        Option<String>,
    )| {
        let label_clone = label.clone();
        spawn(async move {
            match save_analytics_query(
                label_clone.clone(),
                cypher.clone(),
                min_time.clone(),
                max_time.clone(),
            )
            .await
            {
                Ok(()) => {
                    match load_analytics_queries().await {
                        Ok(queries) => saved_queries.set(queries),
                        Err(e) => tracing::error!("Failed to reload queries: {e}"),
                    }
                    active_queries.write().insert(label_clone.clone());
                    if !*dates_loaded.read() {
                        dates_loaded.set(true);
                        if let Ok(dates) = get_offset_dates().await {
                            offset_dates.set(dates);
                        }
                    }
                    let q_cypher = cypher.clone();
                    let q_min = min_time.clone();
                    let q_max = max_time.clone();
                    loading_queries.write().insert(label_clone.clone());
                    match run_analytics_query(q_cypher, q_min, q_max).await {
                        Ok(data) => {
                            query_results.write().insert(label_clone.clone(), data);
                        }
                        Err(e) => {
                            query_errors
                                .write()
                                .insert(label_clone.clone(), format!("{e}"));
                        }
                    }
                    loading_queries.write().remove(&label_clone);
                }
                Err(e) => {
                    query_errors
                        .write()
                        .insert(label_clone, format!("{e}"));
                }
            }
        });
    };

    let on_refresh = move |_| {
        spawn(async move {
            match get_offset_dates().await {
                Ok(dates) => offset_dates.set(dates),
                Err(e) => tracing::error!("Failed to refresh offset dates: {e}"),
            }
        });

        let active = active_queries.read().clone();
        let queries = saved_queries.read().clone();
        for label in active {
            if let Some(q) = queries.iter().find(|q| q.label == label) {
                let q = q.clone();
                let label_done = label.clone();
                loading_queries.write().insert(label.clone());
                query_errors.write().remove(&label);
                spawn(async move {
                    match run_analytics_query(q.cypher, q.min_time, q.max_time).await {
                        Ok(data) => {
                            query_results.write().insert(label_done.clone(), data);
                        }
                        Err(e) => {
                            query_errors
                                .write()
                                .insert(label_done.clone(), format!("{e}"));
                        }
                    }
                    loading_queries.write().remove(&label_done);
                });
            }
        }
    };

    let on_reset_zoom = move |_| {
        zoom_range.set(None);
    };

    let on_zoom_change = move |(min, max): (i64, i64)| {
        zoom_range.set(Some((min, max)));
    };

    let on_download_csv = move |_| {
        let results = query_results.read().clone();
        let dates = offset_dates.read().clone();
        let active = active_queries.read().clone();
        let queries = saved_queries.read().clone();
        let zoom = *zoom_range.read();

        let csv = build_csv(&results, &dates, &active, &queries, zoom);
        download_csv(&csv);
    };

    let zoom = *zoom_range.read();

    rsx! {
        div { class: "analytics-left-panel",
            AnalyticsQueriesPanel {
                saved_queries: saved_queries.read().clone(),
                active_queries: active_queries,
                loading_queries: loading_queries,
                query_errors: query_errors,
                on_toggle: on_toggle,
                on_delete: on_delete,
                on_save: on_save,
            }
        }
        div { class: "analytics-center-panel",
            AnalyticsChart {
                query_results: query_results.read().clone(),
                offset_dates: offset_dates.read().clone(),
                active_queries: active_queries.read().clone(),
                saved_queries: saved_queries.read().clone(),
                zoom_range: zoom,
                on_zoom_change: on_zoom_change,
            }
        }
        div { class: "analytics-right-panel",
            h3 { "Date Range" }
            {
                let dates = offset_dates.read().clone();
                let unique_dates: Vec<String> = {
                    let mut d: Vec<String> = dates.iter()
                        .map(|(_, s)| s[..10.min(s.len())].to_string())
                        .collect();
                    d.dedup();
                    d
                };
                let min_date = unique_dates.first().cloned().unwrap_or_default();
                let max_date = unique_dates.last().cloned().unwrap_or_default();
                let from_date = dates.iter()
                    .find(|(o, _)| zoom.map_or(true, |(min, _)| *o >= min))
                    .map(|(_, d)| d[..10.min(d.len())].to_string())
                    .unwrap_or_default();
                let to_date = dates.iter().rev()
                    .find(|(o, _)| zoom.map_or(true, |(_, max)| *o <= max))
                    .map(|(_, d)| d[..10.min(d.len())].to_string())
                    .unwrap_or_default();
                let min_date2 = min_date.clone();
                let max_date2 = max_date.clone();
                let dates_for_from = dates.clone();
                let dates_for_to = dates.clone();
                let to_date_for_from = to_date.clone();
                let from_date_for_to = from_date.clone();

                let on_from_change = move |evt: Event<FormData>| {
                    let selected = evt.value();
                    if selected.is_empty() { return; }
                    // Find first offset on or after selected date
                    let min_off = dates_for_from.iter()
                        .find(|(_, d)| &d[..10.min(d.len())] >= selected.as_str())
                        .map(|(o, _)| *o);
                    // Keep current "to" date's max offset
                    let max_off = dates_for_from.iter().rev()
                        .find(|(_, d)| &d[..10.min(d.len())] <= to_date_for_from.as_str())
                        .map(|(o, _)| *o);
                    if let (Some(min_o), Some(max_o)) = (min_off, max_off) {
                        zoom_range.set(Some((min_o, max_o)));
                    }
                };

                let on_to_change = move |evt: Event<FormData>| {
                    let selected = evt.value();
                    if selected.is_empty() { return; }
                    // Keep current "from" date's min offset
                    let min_off = dates_for_to.iter()
                        .find(|(_, d)| &d[..10.min(d.len())] >= from_date_for_to.as_str())
                        .map(|(o, _)| *o);
                    // Find last offset on or before selected date
                    let max_off = dates_for_to.iter().rev()
                        .find(|(_, d)| &d[..10.min(d.len())] <= selected.as_str())
                        .map(|(o, _)| *o);
                    if let (Some(min_o), Some(max_o)) = (min_off, max_off) {
                        zoom_range.set(Some((min_o, max_o)));
                    }
                };

                let dates_for_24h = dates.clone();
                let dates_for_168h = dates.clone();

                // Compute cutoff by subtracting hours from the latest timestamp in data.
                // Parse ISO 8601 "YYYY-MM-DDTHH:MM:SSZ" into seconds, subtract, format back.
                let mut on_last_n_hours = move |dates_ref: Vec<(i64, String)>, hours: u64| {
                    if let Some((_, latest)) = dates_ref.last() {
                        if let Some(cutoff) = subtract_hours_from_iso(latest, hours) {
                            let min_off = dates_ref.iter()
                                .find(|(_, d)| d.as_str() >= cutoff.as_str())
                                .map(|(o, _)| *o);
                            let max_off = dates_ref.last().map(|(o, _)| *o);
                            if let (Some(min_o), Some(max_o)) = (min_off, max_off) {
                                zoom_range.set(Some((min_o, max_o)));
                            }
                        }
                    }
                };

                let on_last_24h = move |_| {
                    on_last_n_hours(dates_for_24h.clone(), 24);
                };

                let on_last_168h = move |_| {
                    on_last_n_hours(dates_for_168h.clone(), 168);
                };

                rsx! {
                    div { class: "zoom-presets",
                        button { class: "analytics-btn", onclick: on_last_24h, "Last 24 hours" }
                        button { class: "analytics-btn", onclick: on_last_168h, "Last 168 hours" }
                    }
                    div { class: "zoom-slider",
                        div { class: "zoom-row",
                            span { class: "zoom-label", "From:" }
                            input {
                                r#type: "date",
                                class: "zoom-date-input",
                                min: min_date,
                                max: max_date,
                                value: from_date,
                                oninput: on_from_change,
                            }
                        }
                        div { class: "zoom-row",
                            span { class: "zoom-label", "To:" }
                            input {
                                r#type: "date",
                                class: "zoom-date-input",
                                min: min_date2,
                                max: max_date2,
                                value: to_date,
                                oninput: on_to_change,
                            }
                        }
                    }
                }
            }
            h3 { "Actions" }
            div { class: "analytics-buttons",
                button { class: "analytics-btn", onclick: on_refresh, "Refresh" }
                button { class: "analytics-btn", onclick: on_reset_zoom, "Reset Zoom" }
                button { class: "analytics-btn", onclick: on_download_csv, "Download CSV" }
            }
            h4 { "Legend" }
            div { class: "analytics-legend",
                for (idx, query) in saved_queries.read().iter().enumerate() {
                    {
                        let active = active_queries.read();
                        if active.contains(&query.label) {
                            let color = crate::components::analytics_queries::color_for_index(idx);
                            let count = query_results
                                .read()
                                .get(&query.label)
                                .map(|d| {
                                    d.iter()
                                        .filter(|(o, _)| {
                                            zoom.map_or(true, |(min, max)| *o >= min && *o <= max)
                                        })
                                        .count()
                                })
                                .unwrap_or(0);
                            rsx! {
                                div {
                                    class: "legend-row",
                                    span {
                                        class: "legend-dot",
                                        style: "background: {color};",
                                    }
                                    span { class: "legend-label", "{query.label}" }
                                    span { class: "legend-count", "({count})" }
                                }
                            }
                        } else {
                            rsx! {}
                        }
                    }
                }
            }
        }
    }
}

/// Subtract N hours from an ISO 8601 timestamp "YYYY-MM-DDTHH:MM:SSZ".
/// Returns None if parsing fails. Uses simple day/hour arithmetic (no leap seconds).
fn subtract_hours_from_iso(iso: &str, hours: u64) -> Option<String> {
    // Parse "2026-03-23T15:29:14Z"
    if iso.len() < 19 { return None; }
    let year: i64 = iso[0..4].parse().ok()?;
    let month: i64 = iso[5..7].parse().ok()?;
    let day: i64 = iso[8..10].parse().ok()?;
    let hour: i64 = iso[11..13].parse().ok()?;
    let min: i64 = iso[14..16].parse().ok()?;
    let sec: i64 = iso[17..19].parse().ok()?;

    // Convert to a simple epoch-like total hours, subtract, convert back
    // Use a rough days-since-epoch approach
    let days_in_month = [0, 31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let is_leap = |y: i64| y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);

    let mut total_days: i64 = 0;
    for y in 2000..year {
        total_days += if is_leap(y) { 366 } else { 365 };
    }
    for m in 1..month {
        total_days += days_in_month[m as usize] as i64;
        if m == 2 && is_leap(year) { total_days += 1; }
    }
    total_days += day - 1;

    let total_secs = total_days * 86400 + hour * 3600 + min * 60 + sec;
    let new_secs = total_secs - (hours as i64) * 3600;

    // Convert back
    let mut remaining = new_secs;
    let new_sec = remaining % 60; remaining /= 60;
    let new_min = remaining % 60; remaining /= 60;
    let new_hour = remaining % 24; remaining /= 24;

    // remaining = days since 2000-01-01
    let mut y = 2000i64;
    loop {
        let dy = if is_leap(y) { 366 } else { 365 };
        if remaining < dy { break; }
        remaining -= dy;
        y += 1;
    }
    let mut m = 1i64;
    loop {
        let mut dm = days_in_month[m as usize] as i64;
        if m == 2 && is_leap(y) { dm += 1; }
        if remaining < dm { break; }
        remaining -= dm;
        m += 1;
    }
    let d = remaining + 1;

    Some(format!("{y:04}-{m:02}-{d:02}T{new_hour:02}:{new_min:02}:{new_sec:02}Z"))
}

fn build_csv(
    results: &HashMap<String, Vec<(i64, f64)>>,
    dates: &[(i64, String)],
    active: &HashSet<String>,
    queries: &[AnalyticsQuery],
    zoom: Option<(i64, i64)>,
) -> String {
    let date_map: HashMap<i64, &str> = dates.iter().map(|(o, d)| (*o, d.as_str())).collect();

    let labels: Vec<&str> = queries
        .iter()
        .filter(|q| active.contains(&q.label))
        .map(|q| q.label.as_str())
        .collect();

    let mut all_offsets: Vec<i64> = results
        .iter()
        .filter(|(l, _)| active.contains(l.as_str()))
        .flat_map(|(_, data)| data.iter().map(|(o, _)| *o))
        .collect();
    all_offsets.sort();
    all_offsets.dedup();

    if let Some((min, max)) = zoom {
        all_offsets.retain(|o| *o >= min && *o <= max);
    }

    let mut lookup: HashMap<(&str, i64), f64> = HashMap::new();
    for (label, data) in results {
        if active.contains(label.as_str()) {
            for &(o, v) in data {
                lookup.insert((label.as_str(), o), v);
            }
        }
    }

    let mut csv = String::from("offset,effective_at");
    for label in &labels {
        csv.push(',');
        csv.push_str(label);
    }
    csv.push('\n');

    for &offset in &all_offsets {
        csv.push_str(&offset.to_string());
        csv.push(',');
        csv.push_str(date_map.get(&offset).unwrap_or(&""));
        for label in &labels {
            csv.push(',');
            if let Some(val) = lookup.get(&(*label, offset)) {
                if *val == val.floor() {
                    csv.push_str(&(*val as i64).to_string());
                } else {
                    csv.push_str(&format!("{val:.2}"));
                }
            }
        }
        csv.push('\n');
    }

    csv
}

fn download_csv(csv: &str) {
    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::JsCast;
        use web_sys::{Blob, BlobPropertyBag, HtmlAnchorElement, Url};
        let window = web_sys::window().unwrap();
        let document = window.document().unwrap();

        let array = js_sys::Array::new();
        array.push(&wasm_bindgen::JsValue::from_str(csv));
        let mut opts = BlobPropertyBag::new();
        opts.type_("text/csv");
        let blob = Blob::new_with_str_sequence_and_options(&array, &opts).unwrap();
        let url = Url::create_object_url_with_blob(&blob).unwrap();

        let a: HtmlAnchorElement = document
            .create_element("a")
            .unwrap()
            .dyn_into()
            .unwrap();
        a.set_href(&url);
        a.set_download("analytics.csv");
        a.click();
        Url::revoke_object_url(&url).unwrap();
    }
}
