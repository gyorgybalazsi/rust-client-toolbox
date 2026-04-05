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

                // Day bands — compute boundaries as midpoints between adjacent days
                {
                    let chart_left = CHART_PADDING_LEFT;
                    let chart_right = CHART_WIDTH - CHART_PADDING_RIGHT;
                    let chart_top = CHART_PADDING_TOP;
                    let chart_bottom = CHART_HEIGHT - CHART_PADDING_BOTTOM;

                    // Compute boundary x-positions between consecutive day groups
                    let mut boundaries: Vec<f64> = Vec::new();
                    for i in 0..day_groups.len().saturating_sub(1) {
                        let end_of_current = plot_x(day_groups[i].2, zoom_min, zoom_max);
                        let start_of_next = plot_x(day_groups[i + 1].1, zoom_min, zoom_max);
                        boundaries.push((end_of_current + start_of_next) / 2.0);
                    }

                    rsx! {
                        for (i, group) in day_groups.iter().enumerate() {
                            {
                                let band_left = if i == 0 { chart_left } else { boundaries[i - 1] };
                                let band_right = if i == day_groups.len() - 1 { chart_right } else { boundaries[i] };
                                let fill = if i % 2 == 0 { "#1e2240" } else { "#1a1a2e" };
                                let label_x = (band_left + band_right) / 2.0;
                                rsx! {
                                    rect {
                                        x: band_left, y: chart_top,
                                        width: band_right - band_left,
                                        height: chart_bottom - chart_top,
                                        fill: fill,
                                    }
                                    if i > 0 {
                                        line {
                                            x1: band_left, y1: chart_top,
                                            x2: band_left, y2: chart_bottom,
                                            stroke: "#555", stroke_width: 1.0, stroke_dasharray: "4,4",
                                        }
                                    }
                                    text {
                                        x: label_x, y: chart_bottom + 20.0,
                                        text_anchor: "middle", font_size: "11px", fill: "#888",
                                        {group.0.clone()}
                                    }
                                }
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

                // Offset tick marks on x-axis
                {
                    let mut tick_offsets: Vec<i64> = series_points.iter()
                        .flat_map(|(_, _, pts)| pts.iter().map(|(o, _)| *o))
                        .collect();
                    tick_offsets.sort();
                    tick_offsets.dedup();
                    rsx! {
                        for off in tick_offsets.iter() {
                            {
                                let x = plot_x(*off, zoom_min, zoom_max);
                                let y_base = CHART_HEIGHT - CHART_PADDING_BOTTOM;
                                rsx! {
                                    line {
                                        x1: x, y1: y_base,
                                        x2: x, y2: y_base + 4.0,
                                        stroke: "#666", stroke_width: 1.0,
                                    }
                                    text {
                                        x: x, y: y_base + 14.0,
                                        text_anchor: "middle", font_size: "9px", fill: "#888",
                                        {off.to_string()}
                                    }
                                }
                            }
                        }
                    }
                }

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
                            // Dots at each data point
                            for (o, v) in points.iter() {
                                {
                                    let cx = plot_x(*o, zoom_min, zoom_max);
                                    let cy = plot_y(*v, y_max);
                                    rsx! {
                                        circle { cx: cx, cy: cy, r: 3.5, fill: color, stroke: "#1a1a2e", stroke_width: 1.5 }
                                    }
                                }
                            }
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

        }
    }
}

/// Find the min offset whose date >= given date string (YYYY-MM-DD)
fn first_offset_on_or_after(offset_dates: &[(i64, String)], date: &str) -> Option<i64> {
    offset_dates.iter()
        .find(|(_, d)| &d[..10.min(d.len())] >= date)
        .map(|(o, _)| *o)
}

/// Find the max offset whose date <= given date string (YYYY-MM-DD)
fn last_offset_on_or_before(offset_dates: &[(i64, String)], date: &str) -> Option<i64> {
    offset_dates.iter().rev()
        .find(|(_, d)| &d[..10.min(d.len())] <= date)
        .map(|(o, _)| *o)
}

/// Extract sorted unique dates from offset_dates
fn unique_dates(offset_dates: &[(i64, String)]) -> Vec<String> {
    let mut dates: Vec<String> = offset_dates.iter()
        .map(|(_, d)| d[..10.min(d.len())].to_string())
        .collect();
    dates.dedup();
    dates
}

#[component]
fn ZoomSlider(
    data_min: i64, data_max: i64, zoom_min: i64, zoom_max: i64,
    offset_dates: Vec<(i64, String)>,
    on_change: EventHandler<(i64, i64)>,
) -> Element {
    let dates = unique_dates(&offset_dates);

    // Current date values for the inputs
    let from_date = offset_dates.iter()
        .find(|(o, _)| *o >= zoom_min)
        .map(|(_, d)| d[..10.min(d.len())].to_string())
        .unwrap_or_default();
    let to_date = offset_dates.iter().rev()
        .find(|(o, _)| *o <= zoom_max)
        .map(|(_, d)| d[..10.min(d.len())].to_string())
        .unwrap_or_default();

    let min_date = dates.first().cloned().unwrap_or_default();
    let max_date = dates.last().cloned().unwrap_or_default();

    let offset_dates_left = offset_dates.clone();
    let offset_dates_right = offset_dates.clone();

    let on_from_change = move |evt: Event<FormData>| {
        let selected_date = evt.value();
        if selected_date.is_empty() { return; }
        if let Some(min_off) = first_offset_on_or_after(&offset_dates_left, &selected_date) {
            on_change.call((min_off, zoom_max));
        }
    };

    let on_to_change = move |evt: Event<FormData>| {
        let selected_date = evt.value();
        if selected_date.is_empty() { return; }
        if let Some(max_off) = last_offset_on_or_before(&offset_dates_right, &selected_date) {
            on_change.call((zoom_min, max_off));
        }
    };

    let min_date2 = min_date.clone();
    let max_date2 = max_date.clone();

    rsx! {
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
