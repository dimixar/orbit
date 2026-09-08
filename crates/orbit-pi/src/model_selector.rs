//! Model selector popup — a small, Zed-style picker popover anchored above
//! the composer.
//!
//! Anatomy follows Zed's popover conventions (`popup_menu` + `picker` from
//! Zed's `crates/ui`, rebuilt here on bare gpui 0.2.2 primitives):
//!
//! - small fixed width, opaque raised surface, 1px strong border, layered
//!   drop shadow (paints above the composer via `deferred` + `anchored`)
//! - a search field on top that filters the section live
//! - a plain scrollable list (`max_h` + `overflow_y_scroll`, the same shape
//!   Zed's `ContextMenu` uses — not `uniform_list`, whose measured layout
//!   can collapse inside a deferred popover)
//! - a leading check on the current selection, provider as dimmed trailing
//!   text on model rows
//! - keyboard navigation: `up`/`down`/`enter`/`escape` are bound to the
//!   `Picker` key context, which rides on the popup's filter input. The
//!   bindings are registered *after* the composer bindings in `main.rs`, so
//!   at the same dispatch depth they win the tie (gpui breaks depth ties by
//!   registration order) — Enter confirms instead of submitting the prompt.

use gpui::{
    div, point, prelude::*, px, App, Context, ElementId, Entity, FocusHandle, Focusable,
    IntoElement, MouseDownEvent, ParentElement, Render, ScrollHandle, SharedString, Styled, Window,
};

use crate::app::{icon, icon_dyn, ModelEntry};
use crate::composer::ComposerInput;
use crate::theme::{self, Theme};

/// Popup width — still compact, with room for icon + name + provider + check
/// after the row padding.
const POPOVER_W: f32 = 256.;
/// Uniform row height for every row in the list.
const ROW_H: f32 = 32.;
/// Air between option rows (not folded into ROW_H so hit targets stay even).
const ROW_GAP: f32 = 2.;
/// Vertical stride used for keyboard scroll math.
const ROW_STRIDE: f32 = ROW_H + ROW_GAP;
/// Largest list height before it scrolls (≈ 8 visible rows).
const LIST_MAX_H: f32 = 8. * ROW_STRIDE;

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
    current_level: String,
    filter: Entity<ComposerInput>,
    /// Scroll position of the popup's list (keyboard navigation keeps the
    /// highlighted row in view via this handle).
    list_scroll: ScrollHandle,
    highlighted: usize,
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
        current_level: String,
        on_select_model: Box<dyn Fn(&str, &str, &mut Window, &mut App)>,
        on_select_level: Box<dyn Fn(&str, &mut Window, &mut App)>,
        on_dismiss: Box<dyn Fn(bool, &mut Window, &mut App)>,
        cx: &mut Context<Self>,
    ) -> Self {
        // The filter input carries both the `Composer` context (so backspace,
        // paste, etc. keep working) and the `Picker` flag (so the picker's
        // enter/escape/arrows take precedence at the same dispatch depth).
        let filter = cx.new(|cx| {
            ComposerInput::new(cx)
                .with_placeholder("Filter models…")
                .with_key_context("Composer Picker")
        });
        Self {
            kind,
            models,
            levels,
            current_model,
            current_level,
            filter,
            list_scroll: ScrollHandle::new(),
            highlighted: 0,
            last_filter: String::new(),
            on_select_model,
            on_select_level,
            on_dismiss,
        }
    }

    /// Live catalog refresh while the popup is open (pi re-reports these
    /// after `set_model`, session switches, etc.).
    pub fn set_catalog(
        &mut self,
        models: Vec<ModelEntry>,
        levels: Vec<String>,
        current_model: String,
        current_level: String,
        cx: &mut Context<Self>,
    ) {
        self.models = models;
        self.levels = levels;
        self.current_model = current_model;
        self.current_level = current_level;
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
            for level in self.levels.iter().filter(|level| matches(level)) {
                let selected = *level == self.current_level;
                rows.push(Row::Level {
                    level: level.clone(),
                    selected,
                });
            }
            return rows;
        }

        for (ix, model) in self.models.iter().enumerate() {
            if matches(&model.name) || matches(&model.id) || matches(&model.provider) {
                let selected = model.name == self.current_model;
                rows.push(Row::Model {
                    model_ix: ix,
                    selected,
                });
            }
        }
        rows
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
        // Keep the highlighted row fully visible (plain top-down scrolling).
        let n = rows.len() as f32;
        let content_h = (n * ROW_H + (n - 1.).max(0.) * ROW_GAP).max(0.);
        let viewport_h = content_h.min(LIST_MAX_H);
        let row_top = next as f32 * ROW_STRIDE;
        let current: f32 = self.list_scroll.offset().y.into();
        let mut offset = current;
        if row_top < current {
            offset = row_top;
        } else if row_top + ROW_H > current + viewport_h {
            offset = row_top + ROW_H - viewport_h;
        }
        let max_offset = (content_h - viewport_h).max(0.);
        let offset = offset.clamp(0., max_offset);
        self.list_scroll.set_offset(point(px(0.), px(offset)));
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
        if needle != self.last_filter {
            self.last_filter = needle.clone();
            self.highlighted = 0;
            // A new filter invalidates the scroll position.
            self.list_scroll.set_offset(point(px(0.), px(0.)));
        }
        let rows = self.rows(&needle);
        if !rows.is_empty() {
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
            // block mouse interaction with the content underneath the popup
            .occlude()
            // any mouse-down outside the popup dismisses it
            .on_mouse_down_out(cx.listener(Self::on_outside_down))
            .on_action(cx.listener(Self::on_cancel))
            .on_action(cx.listener(Self::on_confirm))
            .on_action(cx.listener(Self::on_next))
            .on_action(cx.listener(Self::on_prev))
            // search field — grouped tightly; the list sits after a real gap
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

/// Icon + tint for a pi thinking level (HugeIcons stroke set, embedded in
/// `assets/icons/`). Mapping follows the recommended ladder: zap for
/// fast/lightweight reasoning, bulb for analysis, brain for deeper
/// reasoning, sparkle cluster for max/deep, AI-marked brain for auto, and a
/// slash-circle when thinking is off. Unknown levels fall back to the plain
/// spark.
pub(crate) fn thinking_icon(level: &str, theme: &Theme) -> (&'static str, gpui::Hsla) {
    match level.to_ascii_lowercase().as_str() {
        "off" => ("icons/thinking-off.svg", theme.text_3),
        "minimal" => ("icons/thinking-minimal.svg", theme.spark_orange),
        "low" => ("icons/thinking-low.svg", theme.spark_orange),
        "medium" => ("icons/thinking-medium.svg", theme.spark_orange),
        "high" => ("icons/thinking-high.svg", theme.spark_orange),
        "xhigh" | "max" | "ultra" => ("icons/thinking-xhigh.svg", theme.spark_orange),
        "auto" => ("icons/thinking-auto.svg", theme.spark_orange),
        _ => ("icons/spark.svg", theme.spark_orange),
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

fn trailing_check(selected: bool, theme: Theme) -> impl IntoElement + use<> {
    div()
        .w(px(12.))
        .flex()
        .items_center()
        .justify_center()
        .child(if selected {
            icon("icons/check.svg", 11., theme.text).into_any_element()
        } else {
            div().into_any_element()
        })
}

fn empty_row(kind: PickerKind, theme: Theme) -> impl IntoElement + use<> {
    div()
        .px(px(12.))
        .pt(px(8.))
        .pb(px(6.))
        .flex()
        .items_center()
        .text_size(theme.ui_px(12.5))
        .text_color(theme.text_3)
        .child(match kind {
            PickerKind::Model => "No matching models",
            PickerKind::Thinking => "No matching levels",
        })
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
    let base = div()
        .id(ElementId::NamedInteger("picker-row".into(), ix as u64))
        .h(px(ROW_H))
        .px(px(8.))
        .rounded(px(6.))
        .flex()
        .items_center()
        .gap(px(8.))
        .cursor_pointer()
        // Hover moves the keyboard highlight; click activates the row.
        .on_hover({
            let this = this.clone();
            move |hovering, _, cx| {
                if *hovering {
                    this.update(cx, |selector, cx| {
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
        .when(highlighted, |row| row.bg(theme.overlay));

    match row {
        Row::Level { level, selected } => base
            .text_size(theme.ui_px(12.5))
            .text_color(if *selected { theme.text } else { theme.text_2 })
            .child({
                let (path, color) = thinking_icon(level, &theme);
                icon(path, 12., color)
            })
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .child(thinking_display(level)),
            )
            .child(trailing_check(*selected, theme)),
        Row::Model { model_ix, selected } => {
            let model = &models[*model_ix];
            base.text_size(theme.ui_px(12.5))
                .text_color(if *selected { theme.text } else { theme.text_2 })
                .child(icon_dyn(provider_icon(&model.provider), 12., theme.text_2))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(
                            div()
                                .min_w_0()
                                .flex_1()
                                .truncate()
                                .child(model.name.clone()),
                        )
                        .child(
                            div()
                                .min_w_0()
                                .flex_shrink_0()
                                .truncate()
                                .text_size(theme.ui_px(11.))
                                .text_color(theme.text_3)
                                .child(model.provider.clone()),
                        ),
                )
                .child(trailing_check(*selected, theme))
        }
    }
}
