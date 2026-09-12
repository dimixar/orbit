//! The usage timeline.
//!
//! The plot itself is GPUI Kit's `AreaChart` (`gpui_component::chart`): real
//! axes, tick labels and a dashed grid, drawn by a maintained implementation
//! and colored through the [`super::kit`] theme bridge.
//!
//! Orbit keeps the parts the framework does not provide: the compact y-axis
//! figures, the hover marker and the readout card — the page's own interaction,
//! in the page's own type (§18/§56).
//!
//! Restraint is the point (§17): one accent, straight segments, no rainbow.
//! The chart is a measurement, not a poster.

use std::rc::Rc;

use gpui::{
    div, prelude::*, px, relative, AnyElement, App, ElementId, FontWeight, IntoElement,
    SharedString, Window,
};
use gpui_component::chart::AreaChart;
use gpui_component::plot::AXIS_GAP;

use super::aggregate::{ChartMetric, TimeSeries};
use super::format;
use crate::theme::Theme;

const PLOT_H: f32 = 168.;
const Y_AXIS_W: f32 = 56.;
const TOOLTIP_W: f32 = 208.;
/// Most x-axis labels before they start colliding.
const MAX_X_LABELS: usize = 8;

/// The page's hover callback: the bucket under the pointer, or nothing.
type HoverFn = Rc<dyn Fn(Option<usize>, &mut Window, &mut App)>;

/// One plotted point. The framework's chart takes an owned data set and two
/// accessor closures, so the row is a plain value type.
#[derive(Clone)]
struct Datum {
    label: SharedString,
    value: f64,
}

/// Round a maximum up to a friendly axis top (1/2/2.5/5 × 10ⁿ).
fn nice_max(max: f64) -> f64 {
    if !max.is_finite() || max <= 0.0 {
        return 1.0;
    }
    let exponent = max.log10().floor();
    let base = 10f64.powf(exponent);
    let normalized = max / base;
    let step = if normalized <= 1.0 {
        1.0
    } else if normalized <= 2.0 {
        2.0
    } else if normalized <= 2.5 {
        2.5
    } else if normalized <= 5.0 {
        5.0
    } else {
        10.0
    };
    step * base
}

/// Axis label for a value in this metric's units.
fn axis_label(value: f64, metric: ChartMetric) -> String {
    match metric {
        ChartMetric::Cost => format::cost(value),
        ChartMetric::Latency => format::duration_ms(value),
        _ => format::compact(value.max(0.0) as u64),
    }
}

/// The timeline chart for one metric.
///
/// `on_hover` reports the bucket under the pointer (or nothing when the
/// pointer leaves the plot), so the page keeps the readout in its own state
/// and the marker and card always agree.
pub fn timeline(
    id: &'static str,
    series: &TimeSeries,
    metric: ChartMetric,
    hover: Option<usize>,
    theme: Theme,
    on_hover: impl Fn(Option<usize>, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let values: Vec<f64> = series
        .points
        .iter()
        .map(|bucket| metric.value(&bucket.totals))
        .collect();
    let max = nice_max(series.max(metric));
    let hover = hover.filter(|ix| *ix < values.len());
    let count = values.len();
    let accent = theme.accent;

    let data: Vec<Datum> = series
        .points
        .iter()
        .zip(values.iter())
        .map(|(bucket, value)| Datum {
            label: bucket.label.clone().into(),
            value: *value,
        })
        .collect();

    // X labels: the framework draws every `tick_margin`-th one.
    let tick_margin = if count <= MAX_X_LABELS {
        1
    } else {
        count.div_ceil(MAX_X_LABELS)
    };

    // Y axis: five figures, each centered on the grid line it names. The
    // framework draws those lines at quarters of `height = PLOT_H - AXIS_GAP`,
    // so the labels are placed at the same fractions rather than distributed
    // by eye — and in the same 10px register as the framework's x labels.
    let plot_h = PLOT_H - AXIS_GAP;
    let line_h = theme.ui_px(10.) * 1.4;
    let axis = div()
        .relative()
        .w(px(Y_AXIS_W))
        .h(px(PLOT_H))
        .flex_none()
        .child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .w_full()
                .h(px(plot_h))
                .text_size(theme.ui_px(10.))
                .text_color(theme.text_3)
                .children((0..=4).map(|step| {
                    // step 4 = the axis maximum (top line), 0 = the baseline.
                    // The plot's origin is its top edge, so the position is the
                    // complement of the value's fraction.
                    let fraction = 1.0 - step as f32 / 4.0;
                    div()
                        .absolute()
                        .top(relative(fraction))
                        .right(px(8.))
                        .mt(-(line_h / 2.0))
                        .child(axis_label(
                            max * (step as f64 / 4.0),
                            metric,
                        ))
                        .into_any_element()
                })),
        );

    // Hover targets: one flexible cell per bucket. Exact hit-testing with no
    // pointer-to-data coordinate math, and cheap at ≤ 70 cells.
    let on_hover: HoverFn = Rc::new(on_hover);
    let cells: Vec<AnyElement> = (0..count)
        .map(|ix| {
            let on_hover = on_hover.clone();
            div()
                .id(ElementId::NamedInteger("usage-chart-cell".into(), ix as u64))
                .flex_1()
                .h_full()
                .on_hover(move |entered, window, cx| {
                    if *entered {
                        on_hover(Some(ix), window, cx);
                    }
                })
                .into_any_element()
        })
        .collect();
    let clear_hover = on_hover.clone();

    // The readout sits inside the plot so it can never escape the page, and
    // flips side with the bucket so it never covers the marker.
    let tooltip = hover.map(|ix| readout(series, metric, ix, theme, count));

    let plot = div()
        .relative()
        .flex_1()
        .min_w_0()
        .h(px(PLOT_H))
        .child(
            div()
                .absolute()
                .inset_0()
                .child(
                    AreaChart::new(data)
                        .x(|datum: &Datum| datum.label.clone())
                        .y(|datum: &Datum| datum.value)
                        .tick_margin(tick_margin)
                        .stroke(accent)
                        .fill(accent.opacity(0.14))
                        // Buckets are discrete measurements; the framework's
                        // default smoothing would imply values between them.
                        .linear(),
                ),
        )
        // The framework's plot has no hover state of its own, so the page owns
        // the pointer: one invisible cell per bucket, plus a guide line and a
        // marker drawn by the plot's decoration overlay.
        .children(hover.map(|ix| marker(ix, count, values[ix], max, theme)))
        .child(
            div()
                .id(SharedString::new_static("usage-chart-overlay"))
                .absolute()
                .inset_0()
                .flex()
                .on_hover(move |entered, window, cx| {
                    if !*entered {
                        clear_hover(None, window, cx);
                    }
                })
                .children(cells),
        )
        .children(tooltip);

    div()
        .id(id)
        .w_full()
        .flex()
        .flex_col()
        .child(div().w_full().flex().items_start().child(axis).child(plot))
}

/// The hover guide and marker: pure layout, so no canvas is needed.
///
/// The dot is placed by `bottom`, which resolves against the plot's height — a
/// percentage margin would resolve against the *width* and park the dot on the
/// baseline. Its ring is the page background, so the marker reads as a point on
/// the line rather than a blob over it.
fn marker(ix: usize, count: usize, value: f64, max: f64, theme: Theme) -> AnyElement {
    let fraction = (ix as f32 + 0.5) / count.max(1) as f32;
    let value_fraction = (value / max.max(f64::EPSILON)).clamp(0.0, 1.0) as f32;
    div()
        .absolute()
        .top_0()
        .bottom(px(AXIS_GAP))
        .left(relative(fraction))
        .w(px(1.))
        .bg(theme.accent.opacity(0.35))
        .child(
            div()
                .absolute()
                .bottom(relative(value_fraction))
                .left(px(-3.5))
                .mt(px(3.5))
                .size(px(7.))
                .rounded_full()
                .border_2()
                .border_color(theme.bg_main)
                .bg(theme.accent),
        )
        .into_any_element()
}

/// The hover readout. Every row is a real measurement from the bucket; the
/// metric's own register decides which rows are worth showing.
fn readout(series: &TimeSeries, metric: ChartMetric, ix: usize, theme: Theme, count: usize) -> AnyElement {
    let Some(bucket) = series.points.get(ix) else {
        return div().into_any_element();
    };
    let totals = &bucket.totals;
    let mut rows: Vec<(&str, String)> = Vec::new();
    match metric {
        ChartMetric::Tokens | ChartMetric::Input | ChartMetric::Output | ChartMetric::Cache => {
            rows.push(("Requests", format::exact(totals.requests)));
            rows.push(("Input", format::compact(totals.tokens.input)));
            rows.push(("Output", format::compact(totals.tokens.output)));
            rows.push(("Cache read", format::compact(totals.tokens.cache_read)));
            rows.push(("Cache write", format::compact(totals.tokens.cache_write)));
        }
        ChartMetric::Requests => {
            rows.push(("Requests", format::exact(totals.requests)));
            rows.push(("Errors", format::exact(totals.errors)));
            rows.push(("Tokens", format::compact(totals.tokens.total)));
        }
        ChartMetric::Cost => {
            rows.push(("Cost", format::cost(totals.cost_usd)));
            rows.push(("Requests", format::exact(totals.requests)));
            rows.push(("Priced", format::exact(totals.priced_requests)));
        }
        ChartMetric::Latency => {
            rows.push((
                "Average",
                totals
                    .avg_duration_ms()
                    .map(format::duration_ms)
                    .unwrap_or_else(|| "—".into()),
            ));
            rows.push(("Measured", format::exact(totals.duration_samples)));
            rows.push(("Requests", format::exact(totals.requests)));
        }
        ChartMetric::Errors => {
            rows.push(("Errors", format::exact(totals.errors)));
            rows.push(("Stopped", format::exact(totals.aborted)));
            rows.push(("Requests", format::exact(totals.requests)));
        }
    }
    let footer = match metric {
        ChartMetric::Tokens | ChartMetric::Input | ChartMetric::Output | ChartMetric::Cache => Some((
            "Total",
            format!(
                "{} ({})",
                format::compact(totals.tokens.total),
                format::exact(totals.tokens.total)
            ),
        )),
        ChartMetric::Errors => totals.error_rate().map(|rate| ("Failure rate", format::percent(rate))),
        _ => None,
    };

    let mut body = div()
        .flex()
        .flex_col()
        .gap(px(2.))
        .line_height(theme.ui_px(11.5) * 1.25)
        .child(
            div()
                .text_size(theme.ui_px(11.5))
                .font_weight(FontWeight::MEDIUM)
                .text_color(theme.text)
                .child(bucket.stamp.clone()),
        );
    for (label, value) in rows {
        body = body.child(readout_row(label, &value, theme));
    }
    if let Some((label, value)) = footer {
        body = body
            .child(div().h(px(1.)).w_full().bg(theme.border))
            .child(readout_row(label, &value, theme));
    }

    let fraction = (ix as f32 + 0.5) / count.max(1) as f32;
    let on_left_half = fraction <= 0.5;
    let card = div()
        .absolute()
        .top(px(4.))
        .w(px(TOOLTIP_W))
        .p(px(8.))
        .rounded(px(8.))
        .border_1()
        .border_color(theme.border_strong)
        .bg(theme.menu_bg)
        .shadow(theme.card_shadow())
        .flex()
        .flex_col()
        .child(body)
        .occlude();
    if on_left_half {
        card.ml(px(12.))
            .left(relative(fraction))
            .into_any_element()
    } else {
        card.mr(px(12.))
            .right(relative(1.0 - fraction))
            .into_any_element()
    }
}

fn readout_row(label: &str, value: &str, theme: Theme) -> AnyElement {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap(px(12.))
        .whitespace_nowrap()
        .text_size(theme.ui_px(11.5))
        .child(div().flex_none().text_color(theme.text_3).child(label.to_string()))
        .child(div().min_w_0().text_color(theme.text).child(value.to_string()))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn axis_tops_are_friendly_numbers() {
        assert_eq!(nice_max(18_400_000.0), 20_000_000.0);
        assert_eq!(nice_max(842.0), 1_000.0);
        assert_eq!(nice_max(1.0), 1.0);
        assert_eq!(nice_max(2_500.0), 2_500.0);
        assert_eq!(nice_max(0.0), 1.0);
        assert!(nice_max(f64::NAN) > 0.0);
    }

    #[test]
    fn axis_labels_use_the_metric_register() {
        assert_eq!(axis_label(18_400_000.0, ChartMetric::Tokens), "18.4M");
        assert_eq!(axis_label(1_800.0, ChartMetric::Latency), "1.8s");
        assert_eq!(axis_label(12.4, ChartMetric::Cost), "$12.40");
        assert_eq!(axis_label(482.0, ChartMetric::Requests), "482");
    }
}
