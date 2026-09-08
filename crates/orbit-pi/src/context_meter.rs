//! Context-window indicator: a circular used-ring, compact hover copy, and
//! a click-to-open breakdown. Numbers come from pi `get_session_stats`;
//! conversation vs "other" is a chars/4 estimate over the loaded transcript
//! (the same heuristic pi's own /context view uses). Categories the protocol
//! does not expose (rules, MCP, skills, …) are not invented.

use std::rc::Rc;

use gpui::{
    canvas, deferred, div, point, prelude::*, px, relative, AnyElement, App, Background, Bounds,
    ClickEvent, Context, Entity, Hsla, IntoElement, MouseDownEvent, PathBuilder, Pixels,
    SharedString, Window,
};
use orbit_rpc::ContextUsage;

use crate::theme::Theme;

const RING: f32 = 16.;
const RING_BOX: f32 = 22.;
const STROKE: f32 = 2.;
/// Gap between the ring and the bottom edge of the hover/details card.
const POPUP_GAP: f32 = 8.;
/// Hover card width. The overlay lives in the 22px ring box; without an
/// explicit width Taffy clamps it to that hit target and the copy wraps
/// one character per line.
const HOVER_W: f32 = 200.;
const PANEL_W: f32 = 280.;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ContextPopup {
    None,
    Hover,
    Details,
}

pub struct ContextSlice {
    pub label: SharedString,
    pub tokens: u64,
    pub color: Hsla,
}

/// Fill color for the ring / "In use" slice, keyed off the used fraction
/// (0..=1) so the color always matches the drawn ring even when pi reports
/// `tokens` without an explicit `percent`.
pub fn fill_color(fraction: Option<f32>, theme: &Theme) -> Hsla {
    match fraction {
        Some(f) if f >= 0.90 => theme.crit,
        Some(f) if f >= 0.70 => theme.warn,
        _ => theme.ring_fill,
    }
}

pub fn format_percent(usage: &ContextUsage) -> Option<String> {
    let percent = usage.percent?;
    let rounded = percent.round().clamp(0.0, 100.0);
    if usage.tokens.unwrap_or(0) > 0 && rounded < 1.0 {
        Some("<1%".into())
    } else {
        Some(format!("{rounded:.0}%"))
    }
}

/// Compact token label (`151K`, `1.0K`, `688`) matching the reference UI.
pub fn format_tokens(n: u64) -> String {
    if n >= 1_000_000 {
        let whole = n / 1_000_000;
        let tenths = ((n % 1_000_000) + 50_000) / 100_000;
        if tenths == 0 {
            format!("{whole}M")
        } else if tenths == 10 {
            format!("{}M", whole + 1)
        } else {
            format!("{whole}.{tenths}M")
        }
    } else if n >= 1_000 {
        let whole = n / 1_000;
        let tenths = ((n % 1_000) + 50) / 100;
        if tenths == 0 {
            format!("{whole}K")
        } else if tenths == 10 {
            format!("{}K", whole + 1)
        } else {
            format!("{whole}.{tenths}K")
        }
    } else {
        n.to_string()
    }
}

/// Split current window usage into conversation vs everything else.
///
/// `conversation_est` is the chars/4 heuristic over loaded messages. It is
/// clamped to the pi-reported used total so the slices never exceed 100%.
pub fn usage_slices(
    usage: &ContextUsage,
    conversation_est: u64,
    theme: &Theme,
) -> Vec<ContextSlice> {
    let Some(used) = usage.tokens else {
        return Vec::new();
    };
    if used == 0 {
        return Vec::new();
    }
    let conversation = conversation_est.min(used);
    let other = used - conversation;
    let mut out = Vec::new();
    if other > 0 {
        let label = if conversation == 0 {
            "In use"
        } else {
            "Other context"
        };
        out.push(ContextSlice {
            label: label.into(),
            tokens: other,
            color: if conversation == 0 {
                fill_color(usage.fraction(), theme)
            } else {
                theme.slice_other
            },
        });
    }
    if conversation > 0 {
        out.push(ContextSlice {
            label: "Conversation".into(),
            tokens: conversation,
            color: theme.slice_convo,
        });
    }
    out
}

/// Circular used-ring. `fraction` is 0..=1 of the context window.
pub fn context_ring(fraction: f32, fill: Hsla, track: Hsla) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            paint_ring(window, bounds, fraction, fill.to_rgb(), track.to_rgb());
        },
    )
    .size(px(RING))
}

fn paint_ring(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    fraction: f32,
    fill: gpui::Rgba,
    track: gpui::Rgba,
) {
    let cx = bounds.origin.x + bounds.size.width / 2.;
    let cy = bounds.origin.y + bounds.size.height / 2.;
    let stroke = px(STROKE);
    let radius = (bounds.size.width.min(bounds.size.height) / 2.) - stroke / 2.;
    if radius <= px(0.) {
        return;
    }

    if let Ok(path) = circle_stroke(cx, cy, radius, stroke) {
        window.paint_path(path, Background::from(track));
    }

    let fraction = fraction.clamp(0.0, 1.0);
    if fraction < 0.008 {
        return;
    }
    let path = if fraction >= 0.997 {
        circle_stroke(cx, cy, radius, stroke)
    } else {
        arc_stroke(cx, cy, radius, fraction, stroke)
    };
    if let Ok(path) = path {
        window.paint_path(path, Background::from(fill));
    }
}

fn circle_stroke(
    cx: Pixels,
    cy: Pixels,
    radius: Pixels,
    stroke: Pixels,
) -> anyhow::Result<gpui::Path<Pixels>> {
    // Two semicircles: a single 360° SVG arc is degenerate (start == end).
    let mut builder = PathBuilder::stroke(stroke);
    builder.move_to(point(cx, cy - radius));
    builder.arc_to(
        point(radius, radius),
        px(0.),
        false,
        true,
        point(cx, cy + radius),
    );
    builder.arc_to(
        point(radius, radius),
        px(0.),
        false,
        true,
        point(cx, cy - radius),
    );
    builder.build()
}

fn arc_stroke(
    cx: Pixels,
    cy: Pixels,
    radius: Pixels,
    fraction: f32,
    stroke: Pixels,
) -> anyhow::Result<gpui::Path<Pixels>> {
    // 12 o'clock, clockwise. `sweep = true` is SVG clockwise.
    let start = point(cx, cy - radius);
    let angle = fraction * std::f32::consts::TAU;
    let end = point(cx + radius * angle.sin(), cy - radius * angle.cos());
    let mut builder = PathBuilder::stroke(stroke);
    builder.move_to(start);
    builder.arc_to(point(radius, radius), px(0.), fraction > 0.5, true, end);
    builder.build()
}

/// Compact hover card: "59% context used" / "151K / 256K tokens".
pub fn compact_card(usage: Option<&ContextUsage>, theme: Theme) -> impl IntoElement {
    let (title, subtitle) = match usage {
        Some(usage) => {
            let title = format_percent(usage)
                .map(|p| format!("{p} context used"))
                .unwrap_or_else(|| "Context usage".into());
            let subtitle = match usage.tokens {
                Some(used) => format!(
                    "{} / {} tokens",
                    format_tokens(used),
                    format_tokens(usage.context_window)
                ),
                None => format!("of {} tokens", format_tokens(usage.context_window)),
            };
            (title, subtitle)
        }
        None => ("Context usage".into(), "Waiting for the pi agent".into()),
    };
    div()
        .w_full()
        .px(px(12.))
        .py(px(8.))
        .rounded(px(10.))
        .border_1()
        .border_color(theme.border_strong)
        .bg(theme.menu_bg)
        .shadow(theme.card_shadow())
        .flex()
        .flex_col()
        .gap(px(2.))
        .child(
            div()
                .text_size(px(13.))
                .text_color(theme.text)
                .whitespace_nowrap()
                .child(title),
        )
        .child(
            div()
                .text_size(px(12.))
                .text_color(theme.text_3)
                .whitespace_nowrap()
                .child(subtitle),
        )
}

/// Full click panel: percent, token total, segmented bar, per-slice legend.
pub fn details_card(
    usage: Option<&ContextUsage>,
    slices: &[ContextSlice],
    theme: Theme,
    on_close: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    on_outside: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let percent = usage.and_then(format_percent);
    let totals = usage.map(|u| match u.tokens {
        Some(used) => format!(
            "~{} / {} Tokens",
            format_tokens(used),
            format_tokens(u.context_window)
        ),
        None => format!("of {} Tokens", format_tokens(u.context_window)),
    });
    let window_tokens = usage.map(|u| u.context_window).unwrap_or(0);

    div()
        .w(px(PANEL_W))
        .rounded(px(12.))
        .border_1()
        .border_color(theme.border_strong)
        .bg(theme.menu_bg)
        .shadow(theme.card_shadow())
        .occlude()
        .on_mouse_down_out(on_outside)
        .flex()
        .flex_col()
        .px(px(14.))
        .pt(px(12.))
        .pb(px(12.))
        .gap(px(10.))
        .child(
            div()
                .flex()
                .items_center()
                .child(
                    div()
                        .flex_1()
                        .text_size(px(13.))
                        .text_color(theme.text_2)
                        .child("Context Usage"),
                )
                .child(
                    div()
                        .id("context-close")
                        .size(px(20.))
                        .rounded_sm()
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .hover(|s| s.bg(theme.overlay))
                        .on_click(on_close)
                        .child(div().text_size(px(14.)).text_color(theme.text_3).child("×")),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .child(
                    div()
                        .flex_1()
                        .text_size(px(13.))
                        .text_color(theme.text)
                        .child(
                            percent
                                .map(|p| format!("{p} Full"))
                                .unwrap_or_else(|| "Unknown".into()),
                        ),
                )
                .children(totals.map(|label| {
                    div()
                        .text_size(px(12.))
                        .text_color(theme.text_3)
                        .child(label)
                })),
        )
        .child(segmented_bar(slices, window_tokens, theme))
        .child(legend(slices, theme))
}

fn segmented_bar(slices: &[ContextSlice], window: u64, theme: Theme) -> impl IntoElement + use<> {
    let mut bar = div()
        .w_full()
        .h(px(6.))
        .rounded_full()
        .overflow_hidden()
        .bg(theme.trough)
        .flex();
    if window > 0 {
        for slice in slices {
            let fraction = (slice.tokens as f32 / window as f32).clamp(0.0, 1.0);
            if fraction <= 0.0 {
                continue;
            }
            let color = slice.color;
            bar = bar.child(div().h_full().w(relative(fraction)).bg(color));
        }
    }
    bar
}

fn legend(slices: &[ContextSlice], theme: Theme) -> impl IntoElement + use<> {
    if slices.is_empty() {
        return div()
            .text_size(px(12.))
            .text_color(theme.text_3)
            .child("No usage reported for this session yet.")
            .into_any_element();
    }
    div()
        .flex()
        .flex_col()
        .gap(px(6.))
        .children(slices.iter().map(|slice| {
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(div().size(px(8.)).rounded(px(2.)).bg(slice.color))
                .child(
                    div()
                        .flex_1()
                        .text_size(px(12.5))
                        .text_color(theme.text_2)
                        .child(slice.label.clone()),
                )
                .child(
                    div()
                        .text_size(px(12.5))
                        .text_color(theme.text)
                        .child(format_tokens(slice.tokens)),
                )
        }))
        .into_any_element()
}

/// Sit the card fully above the ring, right-aligned with it.
///
/// `bottom: 100%` pins the card's bottom edge to the ring's top. `right: 0`
/// keeps it on the meter instead of drifting left. Width is required: this
/// overlay's containing block is the 22px ring, so auto-width collapses.
fn popup_above_ring(width: f32, child: impl IntoElement) -> AnyElement {
    div()
        .absolute()
        .bottom_full()
        .right_0()
        .mb(px(POPUP_GAP))
        .w(px(width))
        .child(deferred(child))
        .into_any_element()
}

/// Ring chip + optional hover/details popover, anchored above the control.
pub fn context_control<V: 'static>(
    usage: Option<&ContextUsage>,
    conversation_est: u64,
    popup: ContextPopup,
    entity: &Entity<V>,
    theme: Theme,
    on_hover: impl Fn(&mut V, bool, &mut Context<V>) + 'static,
    on_click: impl Fn(&mut V, &mut Window, &mut Context<V>) + 'static,
    on_close: impl Fn(&mut V, &mut Window, &mut Context<V>) + 'static,
) -> impl IntoElement {
    let fraction = usage.and_then(ContextUsage::fraction).unwrap_or(0.0);
    let fill = fill_color(Some(fraction), &theme);
    let slices = usage
        .map(|u| usage_slices(u, conversation_est, &theme))
        .unwrap_or_default();
    let this = entity.clone();
    let this_click = entity.clone();
    let this_close = entity.clone();
    let this_outside = entity.clone();
    let on_close = Rc::new(on_close);

    let popup_el: Option<AnyElement> = match popup {
        ContextPopup::None => None,
        ContextPopup::Hover => Some(popup_above_ring(HOVER_W, compact_card(usage, theme))),
        ContextPopup::Details => {
            let close = this_close.clone();
            let outside = this_outside;
            let on_close_btn = on_close.clone();
            let on_close_out = on_close.clone();
            Some(popup_above_ring(
                PANEL_W,
                details_card(
                    usage,
                    &slices,
                    theme,
                    move |_, window, cx| {
                        close.update(cx, |app, cx| {
                            (on_close_btn.as_ref())(app, window, cx);
                        });
                    },
                    move |_, window, cx| {
                        outside.update(cx, |app, cx| {
                            (on_close_out.as_ref())(app, window, cx);
                        });
                    },
                ),
            ))
        }
    };

    let percent = usage.and_then(format_percent);

    // Percent + ring share one hit target. The overlay stays on the 22px
    // ring box so it still opens above the meter, not the label.
    div()
        .id("context-meter")
        .flex()
        .items_center()
        .gap(px(6.))
        .cursor_pointer()
        .on_hover(move |hovered, _, cx| {
            this.update(cx, |app, cx| on_hover(app, *hovered, cx));
        })
        .on_mouse_up(gpui::MouseButton::Left, move |_, window, cx| {
            this_click.update(cx, |app, cx| on_click(app, window, cx));
        })
        .children(percent.map(|label| {
            div()
                .text_size(px(12.))
                .text_color(fill)
                .whitespace_nowrap()
                .child(label)
        }))
        .child(
            div()
                .relative()
                .size(px(RING_BOX))
                .flex()
                .items_center()
                .justify_center()
                .child(context_ring(fraction, fill, theme.ring_track))
                .children(popup_el),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn usage(tokens: Option<u64>, window: u64, percent: Option<f64>) -> ContextUsage {
        ContextUsage {
            tokens,
            context_window: window,
            percent,
        }
    }

    #[test]
    fn tokens_match_reference_style() {
        assert_eq!(format_tokens(688), "688");
        assert_eq!(format_tokens(1_000), "1K");
        assert_eq!(format_tokens(1_900), "1.9K");
        assert_eq!(format_tokens(9_600), "9.6K");
        assert_eq!(format_tokens(60_000), "60K");
        assert_eq!(format_tokens(150_700), "150.7K");
        assert_eq!(format_tokens(256_000), "256K");
    }

    #[test]
    fn tokens_round_boundaries_have_no_trailing_zero() {
        assert_eq!(format_tokens(100_050), "100.1K");
        assert_eq!(format_tokens(100_000), "100K");
        assert_eq!(format_tokens(1_000_001), "1M");
        assert_eq!(format_tokens(1_050_000), "1.1M");
        assert_eq!(format_tokens(1_000_000), "1M");
    }

    #[test]
    fn fill_color_tracks_fraction() {
        let theme = Theme::dark();
        assert_eq!(fill_color(Some(0.5), &theme), theme.ring_fill);
        assert_eq!(fill_color(Some(0.75), &theme), theme.warn);
        assert_eq!(fill_color(Some(0.95), &theme), theme.crit);
        assert_eq!(fill_color(None, &theme), theme.ring_fill);
    }

    #[test]
    fn percent_sub_one() {
        let u = usage(Some(400), 200_000, Some(0.2));
        assert_eq!(format_percent(&u), Some("<1%".into()));
    }

    #[test]
    fn slices_split_conversation_and_other() {
        let u = usage(Some(60_000), 200_000, Some(30.0));
        let slices = usage_slices(&u, 50_000, &Theme::dark());
        assert_eq!(slices.len(), 2);
        assert_eq!(slices[0].label.as_ref(), "Other context");
        assert_eq!(slices[0].tokens, 10_000);
        assert_eq!(slices[1].label.as_ref(), "Conversation");
        assert_eq!(slices[1].tokens, 50_000);
    }

    #[test]
    fn slices_clamp_conversation_to_used() {
        let u = usage(Some(1_000), 200_000, Some(1.0));
        let slices = usage_slices(&u, 9_999, &Theme::dark());
        assert_eq!(slices.len(), 1);
        assert_eq!(slices[0].label.as_ref(), "Conversation");
        assert_eq!(slices[0].tokens, 1_000);
    }

    #[test]
    fn slices_single_in_use_without_conversation() {
        let u = usage(Some(8_000), 200_000, Some(4.0));
        let slices = usage_slices(&u, 0, &Theme::dark());
        assert_eq!(slices.len(), 1);
        assert_eq!(slices[0].label.as_ref(), "In use");
        assert_eq!(slices[0].tokens, 8_000);
    }
}
