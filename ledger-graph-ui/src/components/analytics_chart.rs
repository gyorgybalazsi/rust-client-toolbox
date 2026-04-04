use crate::components::analytics_queries::color_for_index;
use crate::models::analytics::AnalyticsQuery;
use dioxus::prelude::*;
use std::collections::{HashMap, HashSet};

const CHART_PADDING_LEFT: f64 = 60.0;
const CHART_PADDING_RIGHT: f64 = 20.0;
const CHART_PADDING_TOP: f64 = 20.0;
const CHART_PADDING_BOTTOM: f64 = 60.0;
const CHART_WIDTH: f64 = 1000.0;
const CHART_HEIGHT: f64 = 500.0;

fn plot_x(offset: i64, min_off: i64, max_off: i64) -> f64 {
    let range = (max_off - min_off).max(1) as f64;
    CHART_PADDING_LEFT + (offset - min_off) as f64 / range * (CHART_WIDTH - CHART_PADDING_LEFT - CHART_PADDING_RIGHT)
}

fn plot_y(value: f64, max_val: f64) -> f64 {
    let usable = CHART_HEIGHT - CHART_PADDING_TOP - CHART_PADDING_BOTTOM;
    if max_val <= 0.0 {
        return CHART_HEIGHT - CHART_PADDING_BOTTOM;
    }
    CHART_HEIGHT - CHART_PADDING_BOTTOM - (value / max_val) * usable
}

fn compute_day_groups(
    offset_dates: &[(i64, String)],
    min_off: i64,
    max_off: i64,
) -> Vec<(String, i64, i64)> {
    let mut groups: Vec<(String, i64, i64)> = Vec::new();
    for (offset, date_str) in offset_dates {
        if *offset < min_off || *offset > max_off {
            continue;
        }
        let day = &date_str[..10.min(date_str.len())];
        if let Some(last) = groups.last_mut() {
            if last.0 == day {
                last.2 = *offset;
                continue;
            }
        }
        groups.push((day.to_string(), *offset, *offset));
    }
    groups
}

#[component]
pub fn AnalyticsChart(
    query_results: HashMap<String, Vec<(i64, f64)>>,
    offset_dates: Vec<(i64, String)>,
    active_queries: HashSet<String>,
    saved_queries: Vec<AnalyticsQuery>,
    zoom_range: Option<(i64, i64)>,
    on_zoom_change: EventHandler<(i64, i64)>,
) -> Element {
    let all_offsets: Vec<i64> = query_results
        .iter()
        .filter(|(label, _)| active_queries.contains(label.as_str()))
        .flat_map(|(_, data)| data.iter().map(|(o, _)| *o))
        .collect();

    if all_offsets.is_empty() {
        return rsx! {
            div { class: "chart-empty", "Select a query to display data" }
        };
    }

    let data_min = *all_offsets.iter().min().unwrap();
    let data_max = *all_offsets.iter().max().unwrap();
    let (zoom_min, zoom_max) = zoom_range.unwrap_or((data_min, data_max));

    let mut max_val: f64 = 1.0;
    let mut series_points: Vec<(usize, String, Vec<(i64, f64)>)> = Vec::new();
    for (idx, query) in saved_queries.iter().enumerate() {
        if !active_queries.contains(&query.label) {
            continue;
        }
        if let Some(data) = query_results.get(&query.label) {
            let filtered: Vec<(i64, f64)> = data
                .iter()
                .filter(|(o, _)| *o >= zoom_min && *o <= zoom_max)
                .copied()
                .collect();
            for &(_, v) in &filtered {
                if v > max_val {
                    max_val = v;
                }
            }
            series_points.push((idx, query.label.clone(), filtered));
        }
    }

    let grid_step = nice_step(max_val);
    let y_max = (max_val / grid_step).ceil() * grid_step;
    let day_groups = compute_day_groups(&offset_dates, zoom_min, zoom_max);
    let view_box = format!("0 0 {CHART_WIDTH} {CHART_HEIGHT}");

    rsx! {
        div { class: "chart-container",
            svg {
                class: "analytics-chart",
                view_box: view_box,
                preserve_aspect_ratio: "xMidYMid meet",

                // Day bands
                for (i, group) in day_groups.iter().enumerate() {
                    {
                        let x1 = plot_x(group.1, zoom_min, zoom_max);
                        let x2 = plot_x(group.2, zoom_min, zoom_max);
                        let fill = if i % 2 == 0 { "#1e2240" } else { "#1a1a2e" };
                        let label_x = (x1 + x2) / 2.0;
                        rsx! {
                            rect {
                                x: x1, y: CHART_PADDING_TOP,
                                width: (x2 - x1).max(2.0),
                                height: CHART_HEIGHT - CHART_PADDING_TOP - CHART_PADDING_BOTTOM,
                                fill: fill,
                            }
                            if i > 0 {
                                line {
                                    x1: x1, y1: CHART_PADDING_TOP,
                                    x2: x1, y2: CHART_HEIGHT - CHART_PADDING_BOTTOM,
                                    stroke: "#555", stroke_width: 1.0, stroke_dasharray: "4,4",
                                }
                            }
                            text {
                                x: label_x, y: CHART_HEIGHT - CHART_PADDING_BOTTOM + 20.0,
                                text_anchor: "middle", font_size: "11px", fill: "#888",
                                {group.0.clone()}
                            }
                        }
                    }
                }

                // Y-axis gridlines
                {
                    let num_lines = (y_max / grid_step) as usize;
                    (0..=num_lines).map(|i| {
                        let val = (i as f64) * grid_step;
                        let y = plot_y(val, y_max);
                        rsx! {
                            line {
                                x1: CHART_PADDING_LEFT, y1: y,
                                x2: CHART_WIDTH - CHART_PADDING_RIGHT, y2: y,
                                stroke: "#333", stroke_width: 0.5,
                            }
                            text {
                                x: CHART_PADDING_LEFT - 8.0, y: y + 4.0,
                                text_anchor: "end", font_size: "10px", fill: "#888",
                                {format_value(val)}
                            }
                        }
                    })
                }

                // Axes
                line { x1: CHART_PADDING_LEFT, y1: CHART_PADDING_TOP, x2: CHART_PADDING_LEFT, y2: CHART_HEIGHT - CHART_PADDING_BOTTOM, stroke: "#555", stroke_width: 1.0 }
                line { x1: CHART_PADDING_LEFT, y1: CHART_HEIGHT - CHART_PADDING_BOTTOM, x2: CHART_WIDTH - CHART_PADDING_RIGHT, y2: CHART_HEIGHT - CHART_PADDING_BOTTOM, stroke: "#555", stroke_width: 1.0 }

                // Data series
                for (idx, _label, points) in series_points.iter() {
                    {
                        let color = color_for_index(*idx);
                        let polyline_points: String = points
                            .iter()
                            .map(|(o, v)| {
                                let x = plot_x(*o, zoom_min, zoom_max);
                                let y = plot_y(*v, y_max);
                                format!("{x},{y}")
                            })
                            .collect::<Vec<_>>()
                            .join(" ");
                        rsx! {
                            polyline { points: polyline_points, fill: "none", stroke: color, stroke_width: 2.0 }
                        }
                    }
                }

                // Legend
                {
                    let legend_x = CHART_WIDTH - CHART_PADDING_RIGHT - 160.0;
                    let legend_y = CHART_PADDING_TOP + 10.0;
                    let active_series: Vec<_> = saved_queries.iter().enumerate()
                        .filter(|(_, q)| active_queries.contains(&q.label))
                        .collect();
                    rsx! {
                        for (li, (idx, query)) in active_series.iter().enumerate() {
                            {
                                let y = legend_y + (li as f64) * 18.0;
                                let color = color_for_index(*idx);
                                rsx! {
                                    circle { cx: legend_x, cy: y, r: 4.0, fill: color }
                                    text { x: legend_x + 10.0, y: y + 4.0, font_size: "11px", fill: "#ccc", {query.label.clone()} }
                                }
                            }
                        }
                    }
                }
            }

            // Zoom slider
            ZoomSlider {
                data_min, data_max, zoom_min, zoom_max,
                offset_dates: offset_dates.clone(),
                on_change: on_zoom_change,
            }
        }
    }
}

#[component]
fn ZoomSlider(
    data_min: i64, data_max: i64, zoom_min: i64, zoom_max: i64,
    offset_dates: Vec<(i64, String)>,
    on_change: EventHandler<(i64, i64)>,
) -> Element {
    let left_date = offset_dates.iter()
        .find(|(o, _)| *o >= zoom_min)
        .map(|(_, d)| d[..10.min(d.len())].to_string())
        .unwrap_or_default();
    let right_date = offset_dates.iter().rev()
        .find(|(o, _)| *o <= zoom_max)
        .map(|(_, d)| d[..10.min(d.len())].to_string())
        .unwrap_or_default();

    let on_left_change = move |evt: Event<FormData>| {
        if let Ok(val) = evt.value().parse::<i64>() {
            let clamped = val.min(zoom_max - 1);
            on_change.call((clamped, zoom_max));
        }
    };

    let on_right_change = move |evt: Event<FormData>| {
        if let Ok(val) = evt.value().parse::<i64>() {
            let clamped = val.max(zoom_min + 1);
            on_change.call((zoom_min, clamped));
        }
    };

    rsx! {
        div { class: "zoom-slider",
            div { class: "zoom-labels",
                span { class: "zoom-date", "{left_date}" }
                span { class: "zoom-date", "{right_date}" }
            }
            div { class: "zoom-inputs",
                input {
                    r#type: "range",
                    class: "zoom-range zoom-range-left",
                    min: data_min as f64, max: data_max as f64,
                    value: zoom_min as f64,
                    oninput: on_left_change,
                }
                input {
                    r#type: "range",
                    class: "zoom-range zoom-range-right",
                    min: data_min as f64, max: data_max as f64,
                    value: zoom_max as f64,
                    oninput: on_right_change,
                }
            }
        }
    }
}

fn nice_step(max_val: f64) -> f64 {
    if max_val <= 0.0 { return 1.0; }
    let rough = max_val / 5.0;
    let magnitude = 10.0f64.powf(rough.log10().floor());
    let residual = rough / magnitude;
    let nice = if residual <= 1.5 { 1.0 } else if residual <= 3.5 { 2.0 } else if residual <= 7.5 { 5.0 } else { 10.0 };
    nice * magnitude
}

fn format_value(val: f64) -> String {
    if val == val.floor() { format!("{}", val as i64) } else { format!("{val:.1}") }
}
