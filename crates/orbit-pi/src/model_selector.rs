//! Model selector popup — a searchable picker popover anchored above the
//! composer chips.
//!
//! Follows the same GPUI conventions as the settings selects and the
//! autocomplete menu:
//!
//! - compact raised surface (`menu_bg`, strong border, layered shadow)
//! - inset search field with icon, then a scrollable list with even row gaps
//! - two-line model rows (name + provider) with a provider chip; two-line
//!   thinking rows (label + hint) with a level icon chip
//! - keyboard highlight uses `active`; hover uses `overlay`; the
//!   current choice gets an accent check
//! - keyboard navigation: `up`/`down`/`enter`/`escape` are bound to the
//!   `Picker` key context on the filter input

use gpui::{
    div, point, prelude::*, px, App, Context, ElementId, Entity, FocusHandle, Focusable,
    FontWeight, IntoElement, MouseDownEvent, ParentElement, Render, ScrollHandle, SharedString,
    Styled, Window,
};
use std::time::{Duration, Instant};

use crate::app::{icon, icon_dyn, ModelEntry};
use crate::composer::ComposerInput;
use crate::model_selector_match::is_model_selected;
use crate::theme::{self, Theme};

/// Popup width — room for a provider chip, two-line model label, and check.
const POPOVER_W: f32 = 360.;
/// Uniform row height (two-line model rows; thinking levels center in it).
const ROW_H: f32 = 44.;
/// Air between option rows (not folded into ROW_H so hit targets stay even).
const ROW_GAP: f32 = 2.;
/// Tight gap between the primary label and secondary hint inside a row.
const LABEL_GAP: f32 = 1.;
/// Vertical stride used for keyboard scroll math.
const ROW_STRIDE: f32 = ROW_H + ROW_GAP;
/// Largest list height before it scrolls (≈ 6 visible rows).
const LIST_MAX_H: f32 = 6. * ROW_STRIDE;

/// Which single-section dropdown a picker popup shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickerKind {
    /// Model catalog from `get_available_models`.
    Model,
    /// Thinking levels from `get_available_thinking_levels`.
    Thinking,
}

/// One row of the picker list.
#[derive(Clone)]
enum Row {
    /// A thinking level (`level` is the raw pi value; display only).
    Level { level: String, selected: bool },
    /// `model_ix` indexes into the full model catalog, so selection survives
    /// while the filtered set is recomputed every frame.
    Model { model_ix: usize, selected: bool },
}

/// A single-section model/thinking dropdown. Created by `OrbitApp` when one
/// of the composer chips is clicked; it talks back exclusively through the
/// callbacks it was built with, so it never borrows app state.
pub struct ModelSelector {
    kind: PickerKind,
    models: Vec<ModelEntry>,
    levels: Vec<String>,
    current_model: String,
    current_model_id: String,
    current_model_provider: String,
    current_level: String,
    filter: Entity<ComposerInput>,
    /// Scroll position of the popup's list (keyboard navigation keeps the
    /// highlighted row in view via this handle).
    list_scroll: ScrollHandle,
    highlighted: usize,
    /// Catalog index of the active model (stable match key).
    selected_catalog_ix: Option<usize>,
    /// Ignore hover-driven highlight briefly so opening under the cursor
    /// doesn't snap back to row 0.
    suppress_hover_until: Option<Instant>,
    /// Deferred popovers need a follow-up scroll after layout settles.
    needs_scroll: bool,
    last_filter: String,
    on_select_model: Box<dyn Fn(&str, &str, &mut Window, &mut App)>,
    on_select_level: Box<dyn Fn(&str, &mut Window, &mut App)>,
    /// `bool` = dismissed by an outside mouse-down (vs. escape), so the
    /// owner can suppress the trigger chip's click-through.
    on_dismiss: Box<dyn Fn(bool, &mut Window, &mut App)>,
}

impl ModelSelector {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        kind: PickerKind,
        models: Vec<ModelEntry>,
        levels: Vec<String>,
        current_model: String,
        current_model_id: String,
        current_model_provider: String,
        current_level: String,
        on_select_model: Box<dyn Fn(&str, &str, &mut Window, &mut App)>,
        on_select_level: Box<dyn Fn(&str, &mut Window, &mut App)>,
        on_dismiss: Box<dyn Fn(bool, &mut Window, &mut App)>,
        initial_highlight: usize,
        cx: &mut Context<Self>,
    ) -> Self {
        // The filter input carries both the `Composer` context (so backspace,
        // paste, etc. keep working) and the `Picker` flag (so the picker's
        // enter/escape/arrows take precedence at the same dispatch depth).
        let placeholder = match kind {
            PickerKind::Model => "Search models…",
            PickerKind::Thinking => "Search levels…",
        };
        let filter = cx.new(|cx| {
            ComposerInput::new(cx)
                .with_placeholder(placeholder)
                .with_key_context("Composer Picker")
        });
        let selected_catalog_ix = Self::resolve_selected_catalog_ix(
            kind,
            &models,
            &levels,
            &current_model,
            &current_model_id,
            &current_model_provider,
            &current_level,
        );
        let mut list_scroll = ScrollHandle::new();
        if let Some(ix) = selected_catalog_ix {
            let row_count = match kind {
                PickerKind::Model => models.len(),
                PickerKind::Thinking => levels.len(),
            };
            if row_count > 0 {
                Self::apply_scroll_to_row(&mut list_scroll, ix, row_count);
            }
        }
        Self {
            kind,
            models,
            levels,
            current_model,
            current_model_id,
            current_model_provider,
            current_level,
            filter,
            list_scroll,
            highlighted: initial_highlight,
            selected_catalog_ix,
            suppress_hover_until: Some(Instant::now() + Duration::from_millis(400)),
            needs_scroll: true,
            last_filter: String::new(),
            on_select_model,
            on_select_level,
            on_dismiss,
        }
    }

    fn resolve_selected_catalog_ix(
        kind: PickerKind,
        models: &[ModelEntry],
        levels: &[String],
        current_model: &str,
        current_model_id: &str,
        current_model_provider: &str,
        current_level: &str,
    ) -> Option<usize> {
        match kind {
            PickerKind::Model => models.iter().position(|model| {
                is_model_selected(
                    model,
                    current_model,
                    current_model_id,
                    current_model_provider,
                )
            }),
            PickerKind::Thinking => levels
                .iter()
                .position(|level| level.eq_ignore_ascii_case(current_level)),
        }
    }

    fn refresh_selected_catalog_ix(&mut self) {
        self.selected_catalog_ix = Self::resolve_selected_catalog_ix(
            self.kind,
            &self.models,
            &self.levels,
            &self.current_model,
            &self.current_model_id,
            &self.current_model_provider,
            &self.current_level,
        );
    }

    fn apply_scroll_to_row(list_scroll: &mut ScrollHandle, ix: usize, row_count: usize) {
        let n = row_count as f32;
        let content_h = (n * ROW_H + (n - 1.).max(0.) * ROW_GAP).max(0.);
        let viewport_h = content_h.min(LIST_MAX_H);
        let row_top = ix as f32 * ROW_STRIDE;
        let max_offset = (content_h - viewport_h).max(0.);
        let centered = row_top - (viewport_h - ROW_H) / 2.0;
        let offset = centered.clamp(0., max_offset);
        list_scroll.set_offset(point(px(0.), px(-offset)));
    }

    fn hover_highlight_blocked(&self) -> bool {
        self.suppress_hover_until
            .is_some_and(|until| Instant::now() < until)
    }

    fn block_hover_highlight(&mut self) {
        self.suppress_hover_until = Some(Instant::now() + Duration::from_millis(400));
    }

    /// Live catalog refresh while the popup is open (pi re-reports these
    /// after `set_model`, session switches, etc.).
    pub fn set_catalog(
        &mut self,
        models: Vec<ModelEntry>,
        levels: Vec<String>,
        current_model: String,
        current_model_id: String,
        current_model_provider: String,
        current_level: String,
        cx: &mut Context<Self>,
    ) {
        self.models = models;
        self.levels = levels;
        self.current_model = current_model;
        self.current_model_id = current_model_id;
        self.current_model_provider = current_model_provider;
        self.current_level = current_level;
        self.refresh_selected_catalog_ix();
        if let Some(ix) = self.selected_catalog_ix {
            let row_count = match self.kind {
                PickerKind::Model => self.models.len(),
                PickerKind::Thinking => self.levels.len(),
            };
            if row_count > 0 {
                Self::apply_scroll_to_row(&mut self.list_scroll, ix, row_count);
            }
        }
        self.block_hover_highlight();
        self.needs_scroll = true;
        cx.notify();
    }

    // ── actions (Picker key context) ──────────────────────────────────────

    fn on_cancel(&mut self, _: &crate::PickerCancel, window: &mut Window, cx: &mut Context<Self>) {
        (self.on_dismiss)(false, window, cx);
    }

    fn on_confirm(
        &mut self,
        _: &crate::PickerConfirm,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let rows = self.rows(&self.last_filter);
        self.activate(self.highlighted, &rows, window, cx);
    }

    fn on_next(&mut self, _: &crate::PickerSelectNext, _: &mut Window, cx: &mut Context<Self>) {
        self.step(1, cx);
    }

    fn on_prev(&mut self, _: &crate::PickerSelectPrev, _: &mut Window, cx: &mut Context<Self>) {
        self.step(-1, cx);
    }

    /// Outside mouse-down anywhere dismisses the popup.
    fn on_outside_down(&mut self, _: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        (self.on_dismiss)(true, window, cx);
    }

    // ── internals ─────────────────────────────────────────────────────────

    /// The visible rows for the current filter, restricted to this picker's
    /// section, filtered by a case-insensitive substring match on
    /// name/id/provider/level.
    fn rows(&self, needle: &str) -> Vec<Row> {
        let matches = |text: &str| needle.is_empty() || text.to_lowercase().contains(needle);
        let mut rows = Vec::new();

        if self.kind == PickerKind::Thinking {
            for level in self.levels.iter().filter(|level| {
                matches(level) || matches(&thinking_display(level)) || matches(thinking_hint(level))
            }) {
                let selected = level.eq_ignore_ascii_case(&self.current_level);
                rows.push(Row::Level {
                    level: level.clone(),
                    selected,
                });
            }
            return rows;
        }

        for (ix, model) in self.models.iter().enumerate() {
            if matches(&model.name) || matches(&model.id) || matches(&model.provider) {
                let selected = self.selected_catalog_ix == Some(ix);
                rows.push(Row::Model {
                    model_ix: ix,
                    selected,
                });
            }
        }
        rows
    }

    fn selected_row_index(rows: &[Row]) -> Option<usize> {
        rows.iter().position(|row| match row {
            Row::Model { selected, .. } | Row::Level { selected, .. } => *selected,
        })
    }

    fn defer_scroll(&self, ix: usize, row_count: usize, cx: &mut Context<Self>) {
        for delay in [16_u64, 50, 120, 250] {
            let this = cx.weak_entity();
            cx.spawn(async move |_, cx| {
                gpui::Timer::after(Duration::from_millis(delay)).await;
                this.update(cx, |selector, cx| {
                    selector.scroll_to_row(ix, row_count);
                    cx.notify();
                })
                .ok();
            })
            .detach();
        }
    }

    fn scroll_to_row(&mut self, ix: usize, row_count: usize) {
        Self::apply_scroll_to_row(&mut self.list_scroll, ix, row_count);
    }

    fn step(&mut self, dir: isize, cx: &mut Context<Self>) {
        let rows = self.rows(&self.last_filter);
        if rows.is_empty() {
            return;
        }
        let pos = self.highlighted.min(rows.len() - 1);
        let count = rows.len();
        let next = if dir > 0 {
            (pos + 1).min(count - 1)
        } else {
            pos.saturating_sub(1)
        };
        self.highlighted = next;
        self.scroll_to_row(next, count);
        cx.notify();
    }

    fn activate(&mut self, ix: usize, rows: &[Row], window: &mut Window, cx: &mut Context<Self>) {
        match rows.get(ix) {
            Some(Row::Model { model_ix, .. }) => {
                let (id, provider) = {
                    let model = &self.models[*model_ix];
                    (model.id.clone(), model.provider.clone())
                };
                (self.on_select_model)(&id, &provider, window, cx);
            }
            Some(Row::Level { level, .. }) => {
                let level = level.clone();
                (self.on_select_level)(&level, window, cx);
            }
            _ => {}
        }
    }
}

impl Focusable for ModelSelector {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.filter.read(cx).focus_handle(cx)
    }
}

impl Render for ModelSelector {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let needle = self.filter.read(cx).text().to_lowercase();
        let rows = self.rows(&needle);
        if needle != self.last_filter {
            self.last_filter = needle.clone();
            if needle.is_empty() {
                self.block_hover_highlight();
                self.needs_scroll = true;
            } else {
                self.highlighted = 0;
            }
            self.list_scroll.set_offset(point(px(0.), px(0.)));
        }

        // Pin focus to the active model on open and after async catalog refresh.
        if self.hover_highlight_blocked() || self.needs_scroll {
            if let Some(ix) = Self::selected_row_index(&rows) {
                self.highlighted = ix;
                self.scroll_to_row(ix, rows.len());
                if self.needs_scroll {
                    self.defer_scroll(ix, rows.len(), cx);
                    self.needs_scroll = false;
                }
            } else if self.needs_scroll {
                self.needs_scroll = false;
            }
        } else if !rows.is_empty() {
            self.highlighted = self.highlighted.min(rows.len() - 1);
        }

        let this = cx.entity();
        let theme = *theme::get(cx);

        // Plain scrollable list — the same shape Zed's `ContextMenu` uses for
        // menu bodies. Rows are ordinary children, so nothing depends on
        // measured-layout sizing inside the deferred popover.
        let mut list = div()
            .id("picker-list")
            .w_full()
            .max_h(px(LIST_MAX_H))
            .overflow_y_scroll()
            .track_scroll(&self.list_scroll)
            .px(px(4.))
            .pt(px(8.))
            .pb(px(2.))
            .flex()
            .flex_col()
            .gap(px(ROW_GAP));
        for ix in 0..rows.len() {
            list = list.child(render_row(
                &rows,
                &self.models,
                ix,
                ix == self.highlighted,
                &this,
                theme,
            ));
        }

        div()
            .w(px(POPOVER_W))
            .font_family(theme::ui_font_family())
            .pt(px(6.))
            .pb(px(6.))
            .rounded(px(10.))
            .border_1()
            .border_color(theme.border_strong)
            .bg(theme.menu_bg)
            .shadow(theme.popover_shadow())
            .flex()
            .flex_col()
            .overflow_hidden()
            .occlude()
            // any mouse-down outside the popup dismisses it
            .on_mouse_down_out(cx.listener(Self::on_outside_down))
            .on_action(cx.listener(Self::on_cancel))
            .on_action(cx.listener(Self::on_confirm))
            .on_action(cx.listener(Self::on_next))
            .on_action(cx.listener(Self::on_prev))
            // search field — grouped above the list with a divider
            .child(
                div()
                    .h(px(34.))
                    .px(px(12.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .border_b_1()
                    .border_color(theme.border)
                    .text_size(theme.ui_px(12.5))
                    .child(icon("icons/search.svg", 13., theme.text_3))
                    .child(self.filter.clone()),
            )
            .child(if rows.is_empty() {
                empty_row(self.kind, theme).into_any_element()
            } else {
                list.into_any_element()
            })
    }
}

/// Icon + tint for a pi thinking level (HugeIcons stroke set in
/// `assets/icons/`). Mapping: idea-01 (off/none), idea (low/minimal), brain
/// (medium), brain-02 (high), ai-brain-02 (xhigh/ultra), ai-brain-03 (max),
/// sparkles (auto). Unknown levels fall back to the plain spark.
pub(crate) fn thinking_icon(level: &str, theme: &Theme) -> (&'static str, gpui::Hsla) {
    match level.to_ascii_lowercase().as_str() {
        "off" | "none" => ("icons/thinking-none.svg", theme.text_3),
        "minimal" | "low" => ("icons/thinking-low.svg", theme.accent),
        "medium" => ("icons/thinking-medium.svg", theme.accent),
        "high" => ("icons/thinking-high.svg", theme.accent),
        "xhigh" | "ultra" => ("icons/thinking-xhigh.svg", theme.accent),
        "max" => ("icons/thinking-max.svg", theme.accent),
        "auto" => ("icons/thinking-auto.svg", theme.accent),
        _ => ("icons/spark.svg", theme.accent),
    }
}

/// Brand glyph for a pi provider — mono SVGs from [theSVG.org]
/// (https://thesvg.org), embedded under `assets/icons/providers/` and named
/// by provider id. Every pi built-in provider has a mark; unknown or
/// user-defined providers (custom `models.json` entries) fall back to a
/// neutral cloud glyph.
pub(crate) fn provider_icon(provider: &str) -> SharedString {
    const KNOWN: &[&str] = &[
        "amazon-bedrock",
        "ant-ling",
        "anthropic",
        "azure-openai-responses",
        "baseten",
        "cerebras",
        "cloudflare-ai-gateway",
        "cloudflare-workers-ai",
        "deepseek",
        "fireworks",
        "github-copilot",
        "google",
        "google-vertex",
        "groq",
        "huggingface",
        "kimi-coding",
        "minimax",
        "minimax-cn",
        "mistral",
        "moonshotai",
        "moonshotai-cn",
        "nvidia",
        "ollama",
        "openai",
        "openai-codex",
        "opencode",
        "opencode-go",
        "openrouter",
        "qwen-token-plan",
        "qwen-token-plan-cn",
        "qwen-token-plan-individual",
        "together",
        "vercel-ai-gateway",
        "xai",
        "xiaomi",
        "xiaomi-token-plan-ams",
        "xiaomi-token-plan-cn",
        "xiaomi-token-plan-sgp",
        "zai",
        "zai-coding-cn",
    ];
    if KNOWN.contains(&provider) {
        format!("icons/providers/{provider}.svg").into()
    } else {
        "icons/cloud.svg".into()
    }
}

/// Display form of a pi thinking level: `high` → `High` (capitalization is
/// presentational; the value stays pi's).
pub(crate) fn thinking_display(level: &str) -> String {
    let mut chars = level.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// Short hint shown under the thinking level label in the picker.
fn thinking_hint(level: &str) -> &'static str {
    match level.to_ascii_lowercase().as_str() {
        "off" | "none" => "No reasoning",
        "minimal" | "low" => "Fast reasoning",
        "medium" => "Balanced reasoning",
        "high" => "Deeper reasoning",
        "xhigh" | "ultra" => "Extensive reasoning",
        "max" => "Maximum reasoning",
        "auto" => "Model chooses depth",
        _ => "Custom reasoning level",
    }
}

/// Icon chip in the thinking picker rows — sized like sidebar session chips
/// so HugeIcons glyphs read clearly (some levels use small artwork in the
/// 24×24 viewBox, e.g. minimal’s dot).
const THINKING_CHIP: f32 = 28.;
const THINKING_ICON: f32 = 15.;

fn trailing_check(selected: bool, theme: Theme) -> impl IntoElement + use<> {
    div()
        .w(px(14.))
        .flex()
        .items_center()
        .justify_center()
        .child(if selected {
            icon("icons/check.svg", 11., theme.accent).into_any_element()
        } else {
            div().into_any_element()
        })
}

fn provider_chip(provider: &str, theme: Theme) -> impl IntoElement + use<> {
    div()
        .size(px(22.))
        .flex_none()
        .rounded(px(6.))
        .bg(theme.bg_raised)
        .border_1()
        .border_color(theme.border)
        .flex()
        .items_center()
        .justify_center()
        .child(icon_dyn(provider_icon(provider), 12., theme.text_2))
}

fn thinking_chip(level: &str, selected: bool, theme: Theme) -> impl IntoElement + use<> {
    let (path, color) = thinking_icon(level, &theme);
    let is_off = level.eq_ignore_ascii_case("off");
    div()
        .size(px(THINKING_CHIP))
        .flex_none()
        .rounded(px(7.))
        .bg(if selected && !is_off {
            theme.accent.opacity(0.14)
        } else {
            theme.bg_raised
        })
        .border_1()
        .border_color(if selected && !is_off {
            theme.accent.opacity(0.35)
        } else {
            theme.border
        })
        .flex()
        .items_center()
        .justify_center()
        .child(icon(path, THINKING_ICON, color))
}

fn empty_row(kind: PickerKind, theme: Theme) -> impl IntoElement + use<> {
    div()
        .h(px(64.))
        .flex()
        .items_center()
        .justify_center()
        .text_size(theme.ui_px(12.))
        .text_color(theme.text_3)
        .child(match kind {
            PickerKind::Model => "No matching models",
            PickerKind::Thinking => "No matching levels",
        })
}

fn label_column<P: IntoElement, S: IntoElement>(
    primary: P,
    secondary: S,
    selected: bool,
    theme: Theme,
) -> impl IntoElement + use<P, S> {
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .justify_center()
        .gap(px(LABEL_GAP))
        .child(
            div()
                .w_full()
                .truncate()
                .text_size(theme.ui_px(12.5))
                .font_weight(if selected {
                    FontWeight::MEDIUM
                } else {
                    FontWeight::NORMAL
                })
                .text_color(if selected {
                    theme.active_fg
                } else {
                    theme.text_2
                })
                .child(primary),
        )
        .child(
            div()
                .w_full()
                .truncate()
                .text_size(theme.ui_px(11.))
                .text_color(theme.text_3)
                .child(secondary),
        )
}

fn render_row(
    rows: &[Row],
    models: &[ModelEntry],
    ix: usize,
    highlighted: bool,
    this: &Entity<ModelSelector>,
    theme: Theme,
) -> impl IntoElement + use<> {
    let row = &rows[ix];
    let selected = match row {
        Row::Level { selected, .. } | Row::Model { selected, .. } => *selected,
    };
    let focused = selected || highlighted;
    let this = this.clone();

    let row_shell = |content: gpui::Div| {
        content
            .id(ElementId::NamedInteger("picker-row".into(), ix as u64))
            .h(px(ROW_H))
            .px(px(10.))
            .rounded(px(6.))
            .cursor_pointer()
            .flex()
            .items_center()
            .gap(px(8.))
            .on_hover({
                let this = this.clone();
                move |hovering, _, cx| {
                    if *hovering {
                        this.update(cx, |selector, cx| {
                            if selector.hover_highlight_blocked() {
                                return;
                            }
                            if selector.highlighted != ix {
                                selector.highlighted = ix;
                                cx.notify();
                            }
                        });
                    }
                }
            })
            .on_click({
                let this = this.clone();
                move |_, window, cx| {
                    this.update(cx, |selector, cx| {
                        let rows = selector.rows(&selector.last_filter);
                        selector.activate(ix, &rows, window, cx);
                    });
                }
            })
            .when(focused, |row| row.bg(theme.active))
            .when(!focused, |row| row.hover(|style| style.bg(theme.overlay)))
    };

    match row {
        Row::Level { level, .. } => row_shell(div())
            .child(thinking_chip(level, selected, theme))
            .child(label_column(
                thinking_display(level),
                thinking_hint(level),
                selected,
                theme,
            ))
            .child(trailing_check(selected, theme)),
        Row::Model { model_ix, .. } => {
            let model = &models[*model_ix];
            row_shell(div())
                .child(provider_chip(&model.provider, theme))
                .child(label_column(
                    model.name.clone(),
                    model.provider.clone(),
                    selected,
                    theme,
                ))
                .child(trailing_check(selected, theme))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn offset_for(ix: usize, row_count: usize) -> f32 {
        let mut scroll = ScrollHandle::new();
        ModelSelector::apply_scroll_to_row(&mut scroll, ix, row_count);
        scroll.offset().y.into()
    }

    #[test]
    fn scroll_offset_is_negative_when_scrolled_down() {
        // gpui's `set_offset` is a negative value for a scrolled-down list
        // (clamped to [-max, 0]); a positive value would be clamped to 0.
        assert!(offset_for(20, 30) < 0.);
    }

    #[test]
    fn scroll_offset_zero_at_top() {
        assert_eq!(offset_for(0, 30), 0.);
    }

    #[test]
    fn scroll_offset_centers_selected_row() {
        let offset = offset_for(20, 30);
        // Row 20's top sits at 20 * ROW_STRIDE; the list is 6 rows tall and
        // centered, so the scrolled offset places the row within the viewport.
        let row_top = 20. * ROW_STRIDE;
        let viewport_h = LIST_MAX_H;
        assert!(row_top + offset >= 0.);
        assert!(row_top + offset <= viewport_h - ROW_H + 0.5);
    }
}
