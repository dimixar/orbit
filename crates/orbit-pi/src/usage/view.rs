//! The Usage page's rendering: header, filters, and every panel.
//!
//! The view is deliberately dumb. It reads a [`UsageSnapshot`] that was
//! computed elsewhere and formats it; the only logic allowed here is layout,
//! interaction, and choosing which register a number is shown in.
//!
//! Structure follows §99/§100: current usage (metric board), what changed
//! (insights), trend (timeline), breakdowns (models, composition, workspaces,
//! providers), health (cache, tools, errors), then the record tables.

use std::sync::Arc;

use gpui::{
    div, prelude::*, px, relative, AnyElement, App, Context, Corner, Entity, Font, FontFeatures,
    FontWeight, Hsla, IntoElement, MouseButton, Render, SharedString, Window,
};
use gpui_component::table::Table;
use gpui_component::{Size as KitSize, Sizable as _};

use super::aggregate::{
    Breakdown, ChartMetric, Direction, GroupRow, Insight, LatencyStats, Tone, Totals,
    UsageSnapshot,
};
use super::chart;
use super::filters::{self, FilterOption};
use super::format;
use super::model::{Granularity, RangePreset, TokenCounts, ToolClass, UsageFilter, UsageIndex};
use super::page::{MenuKind, UsagePage};
use crate::theme::{self, Theme};
use crate::app::icon;

/// Inner column width for a data surface (§58): wide enough for a full table,
/// narrow enough that the eye does not have to travel a metre.
pub(super) const CONTENT_MAX_W: f32 = 1180.;
/// The page column's horizontal padding (both sides), which the tables sit inside.
pub(super) const PAGE_PAD: f32 = 40.;
/// Below this column width the paired panels stack into one column.
const TWO_COLUMN_MIN: f32 = 820.;
const FOUR_KPI_MIN: f32 = 880.;

/// Heights for the framework tables. Each is a whole number of 26px rows plus
/// the header, so a table never ends by cutting a row in half. All three scroll
/// internally rather than growing without bound.
const ROW_H: f32 = 26.;
const SESSION_TABLE_H: f32 = ROW_H * 13.;
const BUCKET_TABLE_H: f32 = ROW_H * 11.;
const FAILURE_TABLE_H: f32 = ROW_H * 10.;
/// Ranked rows per breakdown panel before the tail is pooled into "more".
const RANKED_ROWS: usize = 6;

impl Render for UsagePage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = *theme::get(cx);
        // Layout is driven by the main area's width (handed over by the shell)
        // — never by the window, which also holds the sidebar and side pane.
        let available = px(self.main_width());
        let column_w = available.min(px(CONTENT_MAX_W));
        let wide = column_w >= px(TWO_COLUMN_MIN);
        let kpi_cols = if column_w >= px(FOUR_KPI_MIN) {
            4
        } else if column_w >= px(560.) {
            2
        } else {
            1
        };

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.bg_main)
            .text_color(theme.text)
            .font_family(theme::ui_font_family())
            .text_size(theme.ui_px(13.))
            .child(self.header(theme, cx))
            .child(self.filter_bar(theme, cx))
            .child(
                div()
                    .id("usage-body")
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .overflow_y_scroll()
                    .track_scroll(self.scroll())
                    .child(
                        div()
                            .flex_none()
                            .w(column_w)
                            .mx_auto()
                            .pb(px(56.))
                            .flex()
                            .flex_col()
                            .children(self.scope_bar(theme, cx))
                            .child(
                                div()
                                    .w_full()
                                    .px(px(20.))
                                    .flex()
                                    .flex_col()
                                    .child(self.body(theme, wide, kpi_cols, window, cx)),
                            ),
                    ),
            )
    }
}

impl UsagePage {
    // ── header ─────────────────────────────────────────────────────────────

    /// 44px page header: back affordance, title, freshness, and the page's own
    /// actions. Compact by design — the data starts on the next row.
    fn header(&self, theme: Theme, cx: &mut Context<Self>) -> AnyElement {
        let status = if self.is_refreshing() {
            Some("Refreshing…".to_string())
        } else {
            self.status().map(str::to_string).or_else(|| {
                self.last_updated_ms()
                    .map(|at| format::age_label(super::collect::now_ms() - at))
            })
        };
        let has_data = self.snapshot().is_some_and(|snapshot| !snapshot.is_empty());

        div()
            .h(px(44.))
            .flex_none()
            .px(px(12.))
            .flex()
            .items_center()
            .gap_2()
            .border_b_1()
            .border_color(theme.border)
            .child(
                div()
                    .id("usage-back")
                    .px(px(8.))
                    .h(px(28.))
                    .rounded(px(7.))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .cursor_pointer()
                    .hover(|style| style.bg(theme.bg_hover))
                    .on_mouse_down(MouseButton::Left, {
                        let entity = cx.entity();
                        move |_, window, cx| {
                            entity.update(cx, |page, cx| page.close_page(window, cx));
                        }
                    })
                    .child(icon("icons/arrow-left.svg", 14., theme.text_2))
                    .child(
                        div()
                            .text_size(theme.ui_px(12.5))
                            .text_color(theme.text_2)
                            .child("Back"),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(7.))
                    .child(icon("icons/usage-total.svg", 15., theme.text_2))
                    .child(
                        div()
                            .text_size(theme.ui_px(15.))
                            .font_weight(FontWeight::MEDIUM)
                            .child("Usage"),
                    ),
            )
            .children(status.map(|status| {
                div()
                    .text_size(theme.ui_px(11.5))
                    .text_color(theme.text_3)
                    .child(status)
            }))
            .child(div().flex_1())
            .child(self.export_control(theme, has_data, cx))
            .child(filters::text_button(
                "usage-refresh",
                if self.is_refreshing() {
                    "Refreshing…"
                } else {
                    "Refresh"
                },
                Some("icons/refresh.svg"),
                true,
                theme,
                {
                    let entity = cx.entity();
                    move |_, _, cx| {
                        entity.update(cx, |page, cx| page.refresh(cx));
                    }
                },
            ))
            .into_any_element()
    }

    fn export_control(&self, theme: Theme, enabled: bool, cx: &mut Context<Self>) -> AnyElement {
        let open = self.menu() == Some(MenuKind::Export);
        let entity = cx.entity();
        let button = filters::text_button(
            "usage-export",
            "Export",
            Some("icons/upload.svg"),
            enabled,
            theme,
            move |_, window, cx| {
                entity.update(cx, |page, cx| page.toggle_menu(MenuKind::Export, window, cx));
            },
        );
        if !open {
            return div().child(button).into_any_element();
        }
        let panel = filters::export_menu(cx, theme);
        filters::chip_with_menu(
            "usage-export-anchor",
            button,
            true,
            Corner::TopRight,
            move || panel,
        )
    }

    // ── filters ────────────────────────────────────────────────────────────

    /// The filter bar: one row of chips under the header. Every chip writes to
    /// the same filter the whole page reads (§10), so no panel can drift.
    fn filter_bar(&self, theme: Theme, cx: &mut Context<Self>) -> AnyElement {
        let filter = self.filter();
        let range = filter.range.clone();
        let index = self.index();

        let mut bar = div()
            .w_full()
            .flex_none()
            .px(px(12.))
            .py(px(8.))
            .border_b_1()
            .border_color(theme.border)
            .flex()
            .items_center()
            .gap(px(6.));

        let entity = cx.entity();
        let range_open = self.menu() == Some(MenuKind::Range);
        let range_chip = filters::chip(
            "usage-range-chip",
            range.label(),
            "icons/clock.svg",
            range.preset != RangePreset::Last7,
            theme,
            move |_, window, cx| {
                entity.update(cx, |page, cx| page.toggle_menu(MenuKind::Range, window, cx));
            },
        );
        let range_panel = range_open.then(|| filters::range_menu(self, cx, theme));
        bar = bar.child(filters::chip_with_menu(
            "usage-range-anchor",
            range_chip,
            range_open,
            Corner::TopLeft,
            move || range_panel.unwrap_or_else(|| div().into_any_element()),
        ));

        if let Some(index) = index {
            let ranked_workspaces = filters::workspace_options(index);
            let ranked_providers = filters::provider_options(index);
            let ranked_models = filters::model_options(index);
            let all_workspaces: Vec<FilterOption> = index
                .workspaces
                .iter()
                .enumerate()
                .map(|(ix, entry)| FilterOption {
                    id: ix as u16,
                    label: entry.label.clone(),
                    sub: None,
                })
                .collect();
            let all_providers: Vec<FilterOption> = index
                .providers
                .iter()
                .enumerate()
                .map(|(ix, entry)| FilterOption {
                    id: ix as u16,
                    label: entry.label.clone(),
                    sub: None,
                })
                .collect();
            let all_models: Vec<FilterOption> = ranked_models
                .iter()
                .map(|option| FilterOption {
                    id: option.id,
                    label: option.label.clone(),
                    sub: None,
                })
                .collect();

            bar = bar
                .child(self.multi_chip(
                    MenuKind::Workspace,
                    "usage-workspace",
                    "All workspaces",
                    &ranked_workspaces,
                    &all_workspaces,
                    &filter.workspaces,
                    theme,
                    cx,
                ))
                .child(self.multi_chip(
                    MenuKind::Provider,
                    "usage-provider",
                    "All providers",
                    &ranked_providers,
                    &all_providers,
                    &filter.providers,
                    theme,
                    cx,
                ))
                .child(self.multi_chip(
                    MenuKind::Model,
                    "usage-model",
                    "All models",
                    &ranked_models,
                    &all_models,
                    &filter.models,
                    theme,
                    cx,
                ));
        }

        if filter.errors_only {
            let entity = cx.entity();
            bar = bar.child(filters::toggle_chip(
                "usage-errors-only",
                "Failed requests",
                theme,
                move |_, _, cx| {
                    entity.update(cx, |page, cx| page.set_errors_only(false, cx));
                },
            ));
        }
        if filter.cached_only {
            let entity = cx.entity();
            bar = bar.child(filters::toggle_chip(
                "usage-cached-only",
                "Cached only",
                theme,
                move |_, _, cx| {
                    entity.update(cx, |page, cx| page.set_cached_only(false, cx));
                },
            ));
        }
        if filter.has_narrowing() {
            let entity = cx.entity();
            bar = bar.child(div().flex_1()).child(filters::text_button(
                "usage-clear-filters",
                "Clear filters",
                None,
                true,
                theme,
                move |_, _, cx| {
                    entity.update(cx, |page, cx| page.clear_filters(cx));
                },
            ));
        }
        bar.into_any_element()
    }

    /// One multi-select chip with its popover. `ranked` is what the menu shows
    /// (busiest first); `all` is the full set "Select all" applies.
    #[allow(clippy::too_many_arguments)]
    fn multi_chip(
        &self,
        kind: MenuKind,
        id: &'static str,
        all_label: &str,
        ranked: &[FilterOption],
        all: &[FilterOption],
        selected: &[u16],
        theme: Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let open = self.menu() == Some(kind);
        let label = filters::selection_label(all_label, selected, |value| {
            ranked
                .iter()
                .chain(all.iter())
                .find(|option| option.id == value)
                .map(|option| option.label.clone())
        });
        let entity = cx.entity();
        let chip = filters::chip(
            id,
            label,
            "icons/filter.svg",
            !selected.is_empty(),
            theme,
            move |_, window, cx| {
                entity.update(cx, |page, cx| page.toggle_menu(kind, window, cx));
            },
        );
        if !open {
            return filters::chip_with_menu(
                id,
                chip,
                false,
                Corner::TopLeft,
                || div().into_any_element(),
            );
        }
        let entity = cx.entity();
        let panel = filters::multi_menu(self, kind, ranked, all, selected, cx, theme);
        let _ = entity;
        filters::chip_with_menu(id, chip, true, Corner::TopLeft, move || panel)
    }

    /// Breadcrumb for the active scope (§68/§69), with the two ways out: clear
    /// the scope, or open the session it names.
    fn scope_bar(&self, theme: Theme, cx: &mut Context<Self>) -> Option<AnyElement> {
        let filter = self.filter();
        let index = self.index()?;
        let mut crumbs: Vec<String> = Vec::new();
        if let Some(session) = filter.session {
            // A scope whose session just disappeared is cleared by the page's
            // next prune; until then it simply contributes no crumb.
            if let Some(entry) = index.try_session(session) {
                crumbs.push(format!("Session: {}", session_title(entry)));
            }
        }
        if filter.workspaces.len() == 1 {
            if let Some(entry) = index.workspaces.get(filter.workspaces[0] as usize) {
                crumbs.push(format!("Workspace: {}", entry.label));
            }
        }
        if filter.models.len() == 1 {
            if let Some(entry) = index.models.get(filter.models[0] as usize) {
                crumbs.push(format!("Model: {}", entry.label));
            }
        }
        if crumbs.is_empty() {
            return None;
        }

        let mut bar = div()
            .id("usage-scope")
            .w_full()
            .pt(px(16.))
            .flex()
            .items_center()
            .gap(px(6.))
            .text_size(theme.ui_px(11.5))
            .child(
                div()
                    .text_color(theme.text_2)
                    .font_weight(FontWeight::MEDIUM)
                    .child("Usage"),
            );
        for crumb in crumbs {
            bar = bar
                .child(div().text_color(theme.text_3).child("/"))
                .child(div().text_color(theme.text).child(crumb));
        }
        if let Some(session) = filter.session {
            let entity = cx.entity();
            bar = bar.child(
                div()
                    .id("usage-scope-open")
                    .ml(px(4.))
                    .px(px(6.))
                    .h(px(20.))
                    .rounded(px(5.))
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .cursor_pointer()
                    .text_color(theme.text_2)
                    .hover(|style| style.bg(theme.bg_hover).text_color(theme.text))
                    .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                        entity.update(cx, |page, cx| page.open_session(window, cx, session));
                    })
                    .child("Open session")
                    .child(icon("icons/arrow-up-right.svg", 10., theme.text_3)),
            );
        }
        let entity = cx.entity();
        bar = bar.child(
            div()
                .id("usage-scope-clear")
                .px(px(6.))
                .h(px(20.))
                .rounded(px(5.))
                .flex()
                .items_center()
                .cursor_pointer()
                .text_color(theme.text_3)
                .hover(|style| style.bg(theme.bg_hover).text_color(theme.text_2))
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    entity.update(cx, |page, cx| page.clear_filters(cx));
                })
                .child("Clear scope"),
        );
        Some(bar.into_any_element())
    }

    // ── page states ────────────────────────────────────────────────────────

    fn body(
        &mut self,
        theme: Theme,
        wide: bool,
        kpi_cols: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if self.is_loading() {
            return self.skeleton(theme);
        }
        if self.index().is_none() {
            // The store could not be read at all: say so, and offer the retry.
            if let Some(error) = self.error().map(str::to_string) {
                let entity = cx.entity();
                let action = div()
                    .id("usage-retry")
                    .mt(px(2.))
                    .h(px(28.))
                    .px(px(10.))
                    .rounded(px(7.))
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.bg_raised)
                    .flex()
                    .items_center()
                    .cursor_pointer()
                    .text_size(theme.ui_px(12.))
                    .hover(|style| style.bg(theme.bg_hover))
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        entity.update(cx, |page, cx| page.refresh(cx));
                    })
                    .child("Try again")
                    .into_any_element();
                return self.message_state(
                    theme,
                    "icons/info.svg",
                    "Unable to load usage data",
                    &error,
                    Some(action),
                );
            }
            return self.skeleton(theme);
        }
        let Some(index) = self.index() else {
            return self.skeleton(theme);
        };
        if index.is_empty() && index.unreadable_files > 0 {
            return self.message_state(
                theme,
                "icons/info.svg",
                "Unable to read usage data",
                &format!(
                    "{} session files in {} could not be parsed.",
                    format::count(index.unreadable_files as u64),
                    self.store_hint()
                ),
                None,
            );
        }
        if index.is_empty() {
            return self.empty_store_state(theme, index.unreadable_files);
        }
        let Some(snapshot) = self.snapshot().cloned() else {
            return self.skeleton(theme);
        };
        let snapshot = snapshot.as_ref();
        if snapshot.filtered_out() {
            let action = self.clear_filters_button(theme, cx);
            return self.message_state(
                theme,
                "icons/filter.svg",
                "No usage matches these filters",
                "Try widening the date range or clearing a filter.",
                action,
            );
        }
        if snapshot.is_empty() {
            return self.empty_store_state(theme, index.unreadable_files);
        }

        let mut sections: Vec<AnyElement> =
            vec![self.kpi_board(snapshot, theme, kpi_cols, cx), self.secondary_strip(snapshot, theme)];
        if !snapshot.insights.is_empty() {
            sections.push(self.insights_panel(&snapshot.insights, theme));
        }
        sections.push(self.timeline_section(snapshot, theme, cx));

        let model_panel = self.breakdown_panel(
            "model-usage",
            "Model usage",
            model_meta(&snapshot.models),
            &snapshot.models,
            "models",
            theme,
            cx,
        );
        let composition = self.composition_panel(snapshot, theme);
        let workspace_panel = self.breakdown_panel(
            "workspace-usage",
            "Workspace usage",
            workspace_meta(&snapshot.workspaces),
            &snapshot.workspaces,
            "workspaces",
            theme,
            cx,
        );
        let provider_panel = self.breakdown_panel(
            "provider-usage",
            "Provider usage",
            provider_meta(&snapshot.providers),
            &snapshot.providers,
            "providers",
            theme,
            cx,
        );
        let cache_panel = self.cache_panel(snapshot, theme, cx);
        let tools_panel = self.tools_panel(snapshot, theme);

        if wide {
            sections.push(paired(
                divide_left(model_panel, theme),
                divide_right(composition),
            ));
            sections.push(paired(
                divide_left(workspace_panel, theme),
                divide_right(provider_panel),
            ));
            sections.push(paired(
                divide_left(cache_panel, theme),
                divide_right(tools_panel),
            ));
        } else {
            sections.push(model_panel);
            sections.push(composition);
            sections.push(workspace_panel);
            sections.push(provider_panel);
            sections.push(cache_panel);
            sections.push(tools_panel);
        }

        sections.push(self.errors_panel(snapshot, theme, window, cx));
        sections.push(self.buckets_panel(theme, window, cx));
        sections.push(self.sessions_panel(theme, window, cx));

        div()
            .w_full()
            .flex()
            .flex_col()
            .children(sections)
            .into_any_element()
    }

    fn clear_filters_button(&self, theme: Theme, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.filter().has_narrowing() {
            return None;
        }
        let entity = cx.entity();
        Some(
            div()
                .id("usage-state-clear")
                .mt(px(2.))
                .h(px(28.))
                .px(px(10.))
                .rounded(px(7.))
                .border_1()
                .border_color(theme.border)
                .bg(theme.bg_raised)
                .flex()
                .items_center()
                .cursor_pointer()
                .text_size(theme.ui_px(12.))
                .hover(|style| style.bg(theme.bg_hover))
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    entity.update(cx, |page, cx| page.clear_filters(cx));
                })
                .child("Clear filters")
                .into_any_element(),
        )
    }

    /// Lightweight skeleton: the page's shape, without a spinner (§45).
    fn skeleton(&self, theme: Theme) -> AnyElement {
        let bar = |width: f32, height: f32| {
            div()
                .w(px(width))
                .h(px(height))
                .rounded(px(4.))
                .bg(theme.bg_raised)
                .into_any_element()
        };
        let mut board = div()
            .flex()
            .flex_wrap()
            .gap(px(1.))
            .bg(theme.border)
            .rounded(px(12.))
            .overflow_hidden();
        for _ in 0..8 {
            board = board.child(
                div()
                    .flex_1()
                    .min_w(px(180.))
                    .h(px(84.))
                    .bg(theme.bg_raised)
                    .p(px(14.))
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .child(bar(64., 10.))
                    .child(bar(96., 20.))
                    .child(bar(120., 10.)),
            );
        }
        div()
            .w_full()
            .flex()
            .flex_col()
            .pt(px(18.))
            .child(board)
            .child(
                div()
                    .mt(px(28.))
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .child(bar(140., 12.))
                    .child(div().h(px(168.)).w_full().rounded(px(8.)).bg(theme.bg_raised)),
            )
            .into_any_element()
    }

    /// The store has no usage at all (§43).
    fn empty_store_state(&self, theme: Theme, unreadable: usize) -> AnyElement {
        let note = (unreadable > 0).then(|| {
            format!(
                "{} session files could not be read.",
                format::count(unreadable as u64)
            )
        });
        let mut state = self.message_state(
            theme,
            "icons/usage-total.svg",
            "No usage data yet",
            &format!(
                "Once you run the agent, its model, token, session and workspace activity appears here — {} is empty.",
                self.store_hint()
            ),
            None,
        );
        if let Some(note) = note {
            state = div()
                .flex()
                .flex_col()
                .child(state)
                .child(
                    div()
                        .w_full()
                        .flex()
                        .justify_center()
                        .text_size(theme.ui_px(11.))
                        .text_color(theme.text_3)
                        .child(note),
                )
                .into_any_element();
        }
        state
    }

    /// The store the page reads, for the states that have to name it.
    fn store_hint(&self) -> String {
        self.store_path()
            .map(|path| format::short_path(&path))
            .unwrap_or_else(|| "the session store".to_string())
    }

    /// A centered message with an optional action.
    fn message_state(
        &self,
        theme: Theme,
        icon_path: &'static str,
        title: &str,
        body: &str,
        action: Option<AnyElement>,
    ) -> AnyElement {
        div()
            .w_full()
            .h(px(400.))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(10.))
            .child(
                div()
                    .size(px(44.))
                    .rounded_full()
                    .bg(theme.bg_raised)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(icon(icon_path, 20., theme.text_3)),
            )
            .child(
                div()
                    .text_size(theme.ui_px(14.))
                    .font_weight(FontWeight::MEDIUM)
                    .child(title.to_string()),
            )
            .child(
                div()
                    .max_w(px(440.))
                    .text_align(gpui::TextAlign::Center)
                    .text_size(theme.ui_px(12.))
                    .text_color(theme.text_3)
                    .child(body.to_string()),
            )
            .children(action)
            .into_any_element()
    }

    // ── metric board ───────────────────────────────────────────────────────

    /// The KPI board: one bordered readout divided by hairlines — a
    /// measurement panel, not a grid of cards (§13).
    ///
    /// Built as explicit rows of `flex_1` cells rather than one wrapping
    /// container of percentage-width cells: percentage widths inside a
    /// wrapping flex row are resolved against the row's own (content-derived)
    /// width, which let the board grow past the window.
    fn kpi_board(
        &self,
        snapshot: &UsageSnapshot,
        theme: Theme,
        cols: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let cells = self.kpi_cells(snapshot);
        let mut board = div()
            .flex_none()
            .w_full()
            .pt(px(18.))
            .flex()
            .flex_col()
            .rounded(px(12.))
            .border_1()
            .border_color(theme.border)
            .bg(theme.bg_raised)
            .overflow_hidden();

        for (row_ix, chunk) in cells.chunks(cols).enumerate() {
            let mut row = div().w_full().flex();
            for (col_ix, cell) in chunk.iter().enumerate() {
                let entity = cx.entity();
                let tint = match cell.tone {
                    CellTone::Normal => theme.text,
                    CellTone::Alert => theme.crit,
                    CellTone::Muted => theme.text_3,
                };
                let mut element = div()
                    .id(SharedString::from(format!("usage-kpi-{row_ix}-{col_ix}")))
                    .flex_1()
                    .min_w(px(150.))
                    .p(px(14.))
                    .flex()
                    .flex_col()
                    .gap(px(7.))
                    .when(row_ix > 0, |cell| cell.border_t_1().border_color(theme.border))
                    .when(col_ix + 1 < chunk.len(), |cell| {
                        cell.border_r_1().border_color(theme.border)
                    })
                    .when(cell.click.is_some(), |cell| {
                        cell.cursor_pointer().hover(|style| style.bg(theme.bg_hover))
                    })
                    .child(
                        div()
                            .text_size(theme.ui_px(11.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.text_3)
                            .child(cell.label.clone()),
                    )
                    .child(
                        div()
                            .font(num_font())
                            .text_size(theme.ui_px(20.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(tint)
                            .child(cell.value.clone()),
                    )
                    .child(
                        div()
                            .text_size(theme.ui_px(11.5))
                            .text_color(theme.text_3)
                            .child(cell.sub.clone()),
                    );
                if let Some(click) = cell.click {
                    element = element.on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        entity.update(cx, |page, cx| match click {
                            KpiClick::ErrorsOnly => page.set_errors_only(true, cx),
                            KpiClick::CachedOnly => page.set_cached_only(true, cx),
                        });
                    });
                }
                row = row.child(element);
            }
            // Pad a short final row so its cells keep the grid's column widths.
            for _ in chunk.len()..cols {
                row = row.child(
                    div()
                        .flex_1()
                        .min_w(px(150.))
                        .when(row_ix > 0, |cell| cell.border_t_1().border_color(theme.border)),
                );
            }
            board = board.child(row);
        }
        board.into_any_element()
    }

    /// One cell per headline metric. A metric whose data does not exist says
    /// "Unavailable" and why — never a fabricated zero (§6/§44).
    fn kpi_cells(&self, snapshot: &UsageSnapshot) -> Vec<KpiCell> {
        let totals = &snapshot.summary.totals;
        let mut cells = Vec::new();
        cells.push(KpiCell {
            label: "Requests".into(),
            value: format::count(totals.requests),
            sub: delta_sub(snapshot, ChartMetric::Requests, "model requests in range"),
            tone: CellTone::Normal,
            click: None,
        });
        cells.push(KpiCell {
            label: "Total tokens".into(),
            value: format::compact(totals.tokens.total),
            sub: match totals.tokens_per_request() {
                Some(avg) => format!("{} avg/request", format::compact(avg as u64)),
                None => "no requests".into(),
            },
            tone: CellTone::Normal,
            click: None,
        });
        cells.push(KpiCell {
            label: "Input tokens".into(),
            value: format::compact(totals.tokens.input),
            sub: share_sub(totals.tokens.input, totals.tokens.total),
            tone: CellTone::Normal,
            click: None,
        });
        cells.push(KpiCell {
            label: "Output tokens".into(),
            value: format::compact(totals.tokens.output),
            sub: match totals.reasoning_reported {
                0 => share_sub(totals.tokens.output, totals.tokens.total),
                _ => format!("{} reasoning", format::compact(totals.reasoning)),
            },
            tone: CellTone::Normal,
            click: None,
        });
        cells.push(match snapshot.cache.hit_rate {
            Some(rate) => KpiCell {
                label: "Cache hit rate".into(),
                value: format::percent(rate),
                sub: format!(
                    "{} read · {} written",
                    format::compact(snapshot.cache.cache_read),
                    format::compact(snapshot.cache.cache_write)
                ),
                tone: CellTone::Normal,
                click: (snapshot.cache.cache_read > 0).then_some(KpiClick::CachedOnly),
            },
            None => KpiCell {
                label: "Cache hit rate".into(),
                value: "Unavailable".into(),
                sub: "no cache tokens reported".into(),
                tone: CellTone::Muted,
                click: None,
            },
        });
        cells.push(match totals.avg_duration_ms() {
            Some(avg) => KpiCell {
                label: "Avg response".into(),
                value: format::duration_ms(avg),
                sub: if snapshot.latency.has_percentiles() {
                    format!(
                        "p95 {} · {} measured",
                        format::duration_ms(snapshot.latency.p95_ms as f64),
                        format::count(snapshot.latency.samples)
                    )
                } else {
                    format!("{} measured", format::count(snapshot.latency.samples))
                },
                tone: CellTone::Normal,
                click: None,
            },
            None => KpiCell {
                label: "Avg response".into(),
                value: "Unavailable".into(),
                sub: "no measurable request gaps".into(),
                tone: CellTone::Muted,
                click: None,
            },
        });
        cells.push(KpiCell {
            label: "Failed requests".into(),
            value: format::exact(totals.errors),
            sub: match totals.error_rate() {
                Some(_) if totals.errors == 0 => "none in this period".into(),
                Some(rate) => format!(
                    "{} of requests · {} stopped",
                    format::percent(rate),
                    format::count(totals.aborted)
                ),
                None => "no requests".into(),
            },
            tone: if totals.errors > 0 {
                CellTone::Alert
            } else {
                CellTone::Normal
            },
            click: (totals.errors > 0).then_some(KpiClick::ErrorsOnly),
        });
        let coverage = totals.cost_coverage();
        cells.push(KpiCell {
            label: "Cost".into(),
            value: if coverage == 0.0 {
                "Unavailable".into()
            } else {
                format::cost(totals.cost_usd)
            },
            sub: if coverage == 0.0 {
                "no pricing for these models".into()
            } else if coverage < 0.999 {
                format!("{} of requests priced", format::percent(coverage * 100.0))
            } else {
                match totals.cost_per_request() {
                    Some(per) => format!("{} per request", format::cost(per)),
                    None => "priced requests only".into(),
                }
            },
            tone: if coverage == 0.0 {
                CellTone::Muted
            } else {
                CellTone::Normal
            },
            click: None,
        });
        cells
    }

    /// Secondary readout under the board: the metrics that inform the headline
    /// numbers but should not compete with them (§88/§101).
    fn secondary_strip(&self, snapshot: &UsageSnapshot, theme: Theme) -> AnyElement {
        let totals = &snapshot.summary.totals;
        let mut items: Vec<(String, String)> = vec![
            ("Agent turns".into(), format::count(snapshot.summary.turns)),
            ("Sessions".into(), format::count(snapshot.summary.sessions)),
            ("Tool calls".into(), format::count(snapshot.summary.tool_runs)),
            (
                "Bash commands".into(),
                format::count(snapshot.summary.bash_runs),
            ),
            (
                "Tool failures".into(),
                format::count(snapshot.summary.tool_errors),
            ),
        ];
        if totals.duration_samples > 0 {
            items.push((
                "Generation time".into(),
                format::span_ms(totals.duration_ms as i64),
            ));
        }
        if let Some(avg) = totals.avg_prompt() {
            items.push(("Avg prompt".into(), format::compact(avg as u64)));
        }
        if let Some(ratio) = totals.output_input_ratio() {
            // Three decimals: output is routinely a fraction of a percent of
            // the prompt, and "0.00×" would read as zero.
            let text = if ratio > 0.0 && ratio < 0.001 {
                "<0.001×".to_string()
            } else {
                format!("{ratio:.3}×")
            };
            items.push(("Output / input".into(), text));
        }
        if let Some(per_mtok) = totals.cost_per_mtok() {
            items.push(("Cost / M tokens".into(), format::cost(per_mtok)));
        }
        if totals.peak_prompt > 0 {
            items.push(("Peak prompt".into(), format::compact(totals.peak_prompt)));
        }
        if totals.reasoning_reported > 0 {
            items.push((
                "Reasoning".into(),
                format!("{} of output", format::compact(totals.reasoning)),
            ));
        }
        items.push(("Models".into(), format::count(snapshot.summary.models)));
        items.push((
            "Workspaces".into(),
            format::count(snapshot.summary.workspaces),
        ));

        div()
            .flex_none()
            .w_full()
            .pt(px(10.))
            .flex()
            .flex_wrap()
            .gap_x(px(20.))
            .gap_y(px(6.))
            .children(items.into_iter().map(|(label, value)| {
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .text_size(theme.ui_px(11.5))
                    .child(div().text_color(theme.text_3).child(label))
                    .child(div().font(num_font()).text_color(theme.text_2).child(value))
                    .into_any_element()
            }))
            .into_any_element()
    }

    fn insights_panel(&self, insights: &[Insight], theme: Theme) -> AnyElement {
        section(
            "usage-insights",
            "Usage insights",
            Some("derived from this range".into()),
            None,
            div()
                .flex()
                .flex_col()
                .gap(px(6.))
                .children(insights.iter().map(|insight| {
                    let (path, color) = match insight.tone {
                        Tone::Positive => ("icons/check.svg", theme.ok_green),
                        Tone::Warning => ("icons/info.svg", theme.warn),
                        Tone::Neutral => ("icons/spark.svg", theme.text_3),
                    };
                    div()
                        .flex()
                        .items_start()
                        .gap(px(8.))
                        .text_size(theme.ui_px(12.))
                        .child(div().pt(px(1.)).child(icon(path, 12., color)))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .text_color(theme.text_2)
                                .child(insight.text.clone()),
                        )
                        .into_any_element()
                }))
                .into_any_element(),
            theme,
        )
    }

    // ── timeline ───────────────────────────────────────────────────────────

    fn timeline_section(
        &self,
        snapshot: &UsageSnapshot,
        theme: Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let metric = self.metric();
        let by = snapshot.series.granularity.label();
        let meta = match metric {
            ChartMetric::Cost => format!("{} total · by {by}", format::cost(metric.total(&snapshot.summary.totals))),
            ChartMetric::Latency => format!("average per bucket · by {by}"),
            _ => format!(
                "{} total · by {by}",
                format::compact(metric.total(&snapshot.summary.totals) as u64)
            ),
        };

        // Metric switcher: one visualization, several measures (§19). A metric
        // with no source data stays visible but disabled, and the line under
        // the chart says why.
        let mut tabs = div()
            .flex()
            .items_center()
            .gap(px(2.))
            .p(px(2.))
            .rounded(px(8.))
            .border_1()
            .border_color(theme.border)
            .bg(theme.bg_raised);
        let mut unavailable: Vec<&str> = Vec::new();
        for option in ChartMetric::ALL {
            let available = option.available(&snapshot.summary);
            if !available {
                unavailable.push(option.label());
            }
            let active = option == metric;
            let entity = cx.entity();
            tabs = tabs.child(
                div()
                    .id(SharedString::from(format!(
                        "usage-metric-{}",
                        option.as_str()
                    )))
                    .h(px(24.))
                    .px(px(9.))
                    .rounded(px(6.))
                    .flex()
                    .items_center()
                    .text_size(theme.ui_px(11.5))
                    .when(active, |tab| {
                        tab.bg(theme.bg_main)
                            .text_color(theme.text)
                            .font_weight(FontWeight::MEDIUM)
                    })
                    .when(!active, |tab| {
                        tab.text_color(if available {
                            theme.text_3
                        } else {
                            theme.text_3.opacity(0.55)
                        })
                    })
                    .when(available && !active, |tab| {
                        tab.cursor_pointer()
                            .hover(|style| style.bg(theme.bg_hover).text_color(theme.text_2))
                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                entity.update(cx, |page, cx| page.set_metric(option, cx));
                            })
                    })
                    .child(option.label()),
            );
        }

        let hover = self.hover_bucket();
        let entity = cx.entity();
        let on_hover = move |bucket: Option<usize>, _: &mut Window, cx: &mut App| {
            entity.update(cx, |page, cx| page.set_hover_bucket(bucket, cx));
        };

        let mut content = div()
            .w_full()
            .flex()
            .flex_col()
            .child(chart::timeline(
                "usage-timeline",
                &snapshot.series,
                metric,
                hover,
                theme,
                on_hover,
            ));
        if snapshot.latency.samples > 0 {
            content = content.child(self.latency_line(snapshot, theme));
        }
        if !unavailable.is_empty() {
            content = content.child(
                div()
                    .pt(px(8.))
                    .text_size(theme.ui_px(11.))
                    .text_color(theme.text_3)
                    .child(unavailable_reason(&unavailable)),
            );
        }

        section(
            "usage-timeline-section",
            "Usage over time",
            Some(meta),
            Some(tabs.into_any_element()),
            content.into_any_element(),
            theme,
        )
    }

    /// Response-time readout (§36): average, then percentiles once there are
    /// enough observations for them to mean something.
    fn latency_line(&self, snapshot: &UsageSnapshot, theme: Theme) -> AnyElement {
        let latency = &snapshot.latency;
        let mut items: Vec<(&str, String)> = vec![
            ("avg", format::duration_ms(latency.avg_ms)),
            ("max", format::duration_ms(latency.max_ms as f64)),
        ];
        if latency.has_percentiles() {
            items.insert(1, ("p50", format::duration_ms(latency.p50_ms as f64)));
            items.insert(2, ("p95", format::duration_ms(latency.p95_ms as f64)));
            items.insert(3, ("p99", format::duration_ms(latency.p99_ms as f64)));
        }
        div()
            .pt(px(10.))
            .flex()
            .items_center()
            .gap(px(14.))
            .flex_wrap()
            .text_size(theme.ui_px(11.))
            .child(
                div()
                    .text_color(theme.text_3)
                    .font_weight(FontWeight::MEDIUM)
                    .child("Response time"),
            )
            .children(items.into_iter().map(|(label, value)| {
                div()
                    .flex()
                    .items_center()
                    .gap(px(5.))
                    .child(div().text_color(theme.text_3).child(label))
                    .child(div().font(num_font()).text_color(theme.text_2).child(value))
                    .into_any_element()
            }))
            .child(
                div()
                    .text_size(theme.ui_px(10.5))
                    .text_color(theme.text_3)
                    .child(if latency.has_percentiles() {
                        format!("{} measured", format::count(latency.samples))
                    } else {
                        format!(
                            "{} measured — percentiles need {}",
                            format::count(latency.samples),
                            LatencyStats::MIN_SAMPLES
                        )
                    }),
            )
            .into_any_element()
    }

    // ── breakdown panels ───────────────────────────────────────────────────

    /// A ranked distribution: bars, values, shares, and click-to-scope rows.
    #[allow(clippy::too_many_arguments)]
    fn breakdown_panel(
        &self,
        id: &'static str,
        title: &str,
        meta: String,
        breakdown: &Breakdown,
        kind: &'static str,
        theme: Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let rows = ranked_rows(breakdown, RANKED_ROWS);
        let mut content = div().flex().flex_col().gap(px(2.));
        if rows.is_empty() {
            content = content.child(empty_line("No usage in this range.", theme));
        }
        for row in rows {
            content = content.child(self.breakdown_row(&row, kind, theme, cx));
        }
        section(id, title, Some(meta), None, content.into_any_element(), theme)
    }

    fn breakdown_row(
        &self,
        row: &BreakdownRow,
        kind: &'static str,
        theme: Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let entity = cx.entity();
        let target = row.id;
        let clickable = target.is_some();
        div()
            .id(SharedString::from(format!(
                "usage-breakdown-{kind}-{}",
                row.label
            )))
            .px(px(8.))
            .py(px(5.))
            .rounded(px(6.))
            .flex()
            .items_center()
            .gap(px(10.))
            .when(clickable, |line| {
                line.cursor_pointer().hover(|style| style.bg(theme.bg_hover))
            })
            .child(
                div()
                    .w(px(150.))
                    .flex_none()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .truncate()
                            .text_size(theme.ui_px(12.))
                            .text_color(theme.text)
                            .child(row.label.clone()),
                    )
                    .children(row.sub.clone().map(|sub| {
                        div()
                            .truncate()
                            .text_size(theme.ui_px(10.5))
                            .text_color(theme.text_3)
                            .child(sub)
                    })),
            )
            .child(
                div()
                    .flex_1()
                    .min_w(px(48.))
                    .h(px(6.))
                    .rounded(px(3.))
                    .bg(theme.trough)
                    .overflow_hidden()
                    .child(
                        div()
                            .h_full()
                            .w(relative(row.fraction.clamp(0.0, 1.0) as f32))
                            .rounded(px(3.))
                            .bg(row.color(&theme)),
                    ),
            )
            .child(
                div()
                    .w(px(64.))
                    .flex_none()
                    .font(num_font())
                    .text_size(theme.ui_px(11.5))
                    .text_color(theme.text_2)
                    .text_align(gpui::TextAlign::Right)
                    .child(row.value.clone()),
            )
            .child(
                div()
                    .w(px(52.))
                    .flex_none()
                    .whitespace_nowrap()
                    .font(num_font())
                    .text_size(theme.ui_px(11.5))
                    .text_color(theme.text_3)
                    .text_align(gpui::TextAlign::Right)
                    .child(format::share(row.fraction)),
            )
            .when(clickable, |line| {
                line.on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    let Some(value) = target else {
                        return;
                    };
                    entity.update(cx, |page, cx| {
                        let mut filter = page.filter().clone();
                        match kind {
                            "workspaces" => toggle(&mut filter.workspaces, value),
                            "providers" => toggle(&mut filter.providers, value),
                            _ => toggle(&mut filter.models, value),
                        }
                        page.set_filter(filter, cx);
                    });
                })
            })
            .into_any_element()
    }

    /// Token composition: input / output / cache read / cache write, as one
    /// stacked bar plus its four rows. The four categories sum to the total,
    /// so nothing is double-counted (§23).
    fn composition_panel(&self, snapshot: &UsageSnapshot, theme: Theme) -> AnyElement {
        let tokens = snapshot.summary.totals.tokens;
        let total = tokens.total;
        let slices: [(&str, u64, Hsla); 4] = [
            ("Input", tokens.input, theme.accent),
            ("Output", tokens.output, theme.accent.opacity(0.62)),
            ("Cache read", tokens.cache_read, theme.accent.opacity(0.40)),
            ("Cache write", tokens.cache_write, theme.accent.opacity(0.22)),
        ];
        let meta = if total == 0 {
            "no tokens in range".to_string()
        } else {
            format!("{} tokens", format::compact(total))
        };

        let mut stack = div()
            .w_full()
            .h(px(8.))
            .rounded(px(4.))
            .overflow_hidden()
            .flex()
            .bg(theme.trough);
        for (_, value, color) in slices {
            if value == 0 || total == 0 {
                continue;
            }
            stack = stack.child(div().h_full().w(relative(value as f32 / total as f32)).bg(color));
        }

        let mut content = div().flex().flex_col().gap(px(10.)).child(stack);
        for (label, value, color) in slices {
            content = content.child(
                div()
                    .px(px(8.))
                    .py(px(4.))
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(div().size(px(8.)).rounded(px(2.)).flex_none().bg(color))
                    .child(
                        div()
                            .flex_1()
                            .text_size(theme.ui_px(12.))
                            .text_color(theme.text_2)
                            .child(label),
                    )
                    .child(
                        div()
                            .whitespace_nowrap()
                            .font(num_font())
                            .text_size(theme.ui_px(11.5))
                            .text_color(theme.text)
                            .child(format::compact(value)),
                    )
                    .child(
                        div()
                            .w(px(52.))
                            .flex_none()
                            .whitespace_nowrap()
                            .font(num_font())
                            .text_size(theme.ui_px(11.5))
                            .text_color(theme.text_3)
                            .text_align(gpui::TextAlign::Right)
                            .child(if total == 0 {
                                "—".to_string()
                            } else {
                                format::share(value as f64 / total as f64)
                            }),
                    )
                    .into_any_element(),
            );
        }
        content = content.child(
            div()
                .px(px(8.))
                .text_size(theme.ui_px(10.5))
                .text_color(theme.text_3)
                .child(if total == 0 {
                    "No tokens in this range.".to_string()
                } else if tokens.cache_read == 0 && tokens.cache_write == 0 {
                    "Cache data unavailable — these providers report no cache tokens.".to_string()
                } else {
                    "Total = input + output + cache read + cache write. Reasoning tokens are a subset of output."
                        .to_string()
                }),
        );

        section(
            "usage-composition",
            "Token composition",
            Some(meta),
            None,
            content.into_any_element(),
            theme,
        )
    }

    // ── health panels ──────────────────────────────────────────────────────

    /// Cache performance, with the formula stated in the panel (§24/§25).
    fn cache_panel(
        &self,
        snapshot: &UsageSnapshot,
        theme: Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let cache = &snapshot.cache;
        let available = cache.is_available();
        let hit = cache.hit_rate;
        let meta = match hit {
            Some(rate) if available => format!("{} hit rate", format::percent(rate)),
            _ => "unavailable".into(),
        };
        let mut content = div().flex().flex_col().gap(px(10.));
        if !available {
            content = content.child(empty_line(
                "Cache data unavailable — no cache tokens were reported in this range.",
                theme,
            ));
            return section(
                "usage-cache",
                "Cache performance",
                Some(meta),
                None,
                content.into_any_element(),
                theme,
            );
        }

        if let Some(rate) = hit {
            content = content.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(7.))
                    .child(
                        div()
                            .flex()
                            .items_baseline()
                            .gap(px(8.))
                            .child(
                                div()
                                    .font(num_font())
                                    .text_size(theme.ui_px(20.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(format::percent(rate)),
                            )
                            .child(
                                div()
                                    .text_size(theme.ui_px(11.5))
                                    .text_color(theme.text_3)
                                    .child("of prompt tokens served from cache"),
                            ),
                    )
                    .child(
                        div()
                            .w_full()
                            .h(px(6.))
                            .rounded(px(3.))
                            .bg(theme.trough)
                            .overflow_hidden()
                            .child(
                                div()
                                    .h_full()
                                    .w(relative((rate / 100.0).clamp(0.0, 1.0) as f32))
                                    .rounded(px(3.))
                                    .bg(theme.accent),
                            ),
                    ),
            );
        }

        content = content.child(
            div()
                .flex()
                .flex_col()
                .gap(px(2.))
                .children(
                    [
                        ("Cache reads", format::compact(cache.cache_read)),
                        ("Cache writes", format::compact(cache.cache_write)),
                        ("Uncached input", format::compact(cache.uncached_input)),
                        (
                            "Requests served from cache",
                            format!(
                                "{} of {}",
                                format::count(cache.cached_requests),
                                format::count(snapshot.summary.totals.requests)
                            ),
                        ),
                    ]
                    .into_iter()
                    .map(|(label, value)| stat_row(label, &value, theme)),
                ),
        );

        // Hit rate across the window: one bar per bucket, so a drop is visible.
        let rates: Vec<Option<f64>> = snapshot
            .series
            .points
            .iter()
            .map(|point| {
                Totals {
                    tokens: TokenCounts {
                        input: point.totals.tokens.input,
                        cache_read: point.totals.tokens.cache_read,
                        ..Default::default()
                    },
                    ..Totals::default()
                }
                .cache_hit_rate()
            })
            .collect();
        if rates.iter().filter(|rate| rate.is_some()).count() >= 2 {
            content = content.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(5.))
                    .child(
                        div()
                            .text_size(theme.ui_px(10.5))
                            .text_color(theme.text_3)
                            .child("Hit rate over time"),
                    )
                    .child(spark_bars(&rates, theme)),
            );
        }

        let entity = cx.entity();
        let cached_only = self.filter().cached_only;
        content = content.child(
            div()
                .px(px(8.))
                .flex()
                .items_center()
                .gap(px(6.))
                .text_size(theme.ui_px(10.5))
                .child(
                    div()
                        .text_color(theme.text_3)
                        .child("Hit rate = cache reads / (cache reads + uncached input) ·"),
                )
                .child(
                    div()
                        .id("usage-cache-toggle")
                        .text_color(theme.text_2)
                        .cursor_pointer()
                        .hover(|style| style.text_color(theme.accent))
                        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                            entity.update(cx, |page, cx| page.set_cached_only(!cached_only, cx));
                        })
                        .child(if cached_only {
                            "show all requests"
                        } else {
                            "show cached requests only"
                        }),
                ),
        );

        section(
            "usage-cache",
            "Cache performance",
            Some(meta),
            None,
            content.into_any_element(),
            theme,
        )
    }

    /// Tool activity: families, the busiest tools, durations, failures (§32/§33).
    fn tools_panel(&self, snapshot: &UsageSnapshot, theme: Theme) -> AnyElement {
        let tools = &snapshot.tools;
        let meta = match snapshot.summary.tool_error_rate() {
            Some(rate) if tools.errors > 0 => format!(
                "{} calls · {} failed ({})",
                format::count(tools.calls),
                format::count(tools.errors),
                format::percent(rate)
            ),
            Some(_) => format!("{} calls · none failed", format::count(tools.calls)),
            None => "no tool calls".to_string(),
        };
        let mut content = div().flex().flex_col().gap(px(10.));
        if tools.calls == 0 {
            content = content.child(empty_line("No tool calls in this range.", theme));
            return section(
                "usage-tools",
                "Tool activity",
                Some(meta),
                None,
                content.into_any_element(),
                theme,
            );
        }

        let families: Vec<(String, u64, Hsla)> = tools
            .by_class
            .iter()
            .filter(|(_, count)| *count > 0)
            .map(|(class, count)| (class.label().to_string(), *count, class_color(*class, theme)))
            .collect();
        let total: u64 = families.iter().map(|(_, count, _)| count).sum();
        let mut stack = div()
            .w_full()
            .h(px(8.))
            .rounded(px(4.))
            .overflow_hidden()
            .flex()
            .bg(theme.trough);
        for (_, count, color) in &families {
            stack = stack.child(
                div()
                    .h_full()
                    .w(relative(*count as f32 / total as f32))
                    .bg(*color),
            );
        }
        content = content.child(stack).child(
            div()
                .flex()
                .flex_wrap()
                .gap_x(px(14.))
                .gap_y(px(4.))
                .children(families.into_iter().map(|(label, count, color)| {
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .text_size(theme.ui_px(10.5))
                        .child(div().size(px(7.)).rounded(px(2.)).bg(color))
                        .child(div().text_color(theme.text_2).child(label))
                        .child(
                            div()
                                .font(num_font())
                                .text_color(theme.text_3)
                                .child(format::count(count)),
                        )
                        .into_any_element()
                })),
        );

        content = content.children(tools.rows.iter().take(RANKED_ROWS).map(|row| {
            div()
                .px(px(8.))
                .py(px(4.))
                .rounded(px(6.))
                .flex()
                .items_center()
                .gap(px(10.))
                .child(
                    div()
                        .w(px(112.))
                        .flex_none()
                        .truncate()
                        .text_size(theme.ui_px(12.))
                        .text_color(theme.text)
                        .child(row.label.clone()),
                )
                .child(
                    div()
                        .w(px(64.))
                        .flex_none()
                        .font(num_font())
                        .text_size(theme.ui_px(11.5))
                        .text_color(theme.text_2)
                        .text_align(gpui::TextAlign::Right)
                        .child(format::count(row.calls)),
                )
                .child(
                    div()
                        .w(px(58.))
                        .flex_none()
                        .font(num_font())
                        .text_size(theme.ui_px(11.5))
                        .text_align(gpui::TextAlign::Right)
                        .text_color(if row.errors > 0 {
                            theme.crit
                        } else {
                            theme.text_3
                        })
                        .child(match row.error_rate() {
                            Some(rate) if row.errors > 0 => format::percent(rate),
                            _ => "—".to_string(),
                        }),
                )
                .child(
                    div()
                        .w(px(62.))
                        .flex_none()
                        .font(num_font())
                        .text_size(theme.ui_px(11.5))
                        .text_color(theme.text_3)
                        .text_align(gpui::TextAlign::Right)
                        .child(match row.avg_duration_ms() {
                            Some(avg) => format::duration_ms(avg),
                            None => "—".to_string(),
                        }),
                )
                .into_any_element()
        }));

        if let Some(bash) = tools.rows.iter().find(|row| row.label == "bash") {
            content = content.child(
                div()
                    .px(px(8.))
                    .text_size(theme.ui_px(10.5))
                    .text_color(theme.text_3)
                    .child(format!(
                        "Bash: {} commands, {} failed, {} of command time, slowest {}",
                        format::count(bash.calls),
                        format::count(bash.errors),
                        format::span_ms(bash.duration_ms as i64),
                        format::duration_ms(bash.max_ms as f64)
                    )),
            );
        }
        section(
            "usage-tools",
            "Tool activity",
            Some(meta),
            None,
            content.into_any_element(),
            theme,
        )
    }

    /// Failures (§34): every provider error and tool failure in the range, in
    /// the same table as the rest of the page, each row a jump to the session
    /// it happened in.
    fn errors_panel(
        &mut self,
        snapshot: &UsageSnapshot,
        theme: Theme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let state = self.failure_table(window, cx);
        self.sync_tables(cx);
        let errors = &snapshot.errors;
        // Two different measures, both real: "failed requests" (a request whose
        // stop reason was an error) and recorded failure events (provider
        // errors pi logged, including ones a retry later recovered from).
        let meta = format!(
            "{} provider errors recorded · {} tool failures · {} stopped",
            format::count(errors.provider),
            format::count(errors.tool),
            format::count(errors.aborted)
        );
        let shown = errors.rows.len();
        let total_events = errors.provider + errors.tool;

        let mut content = div().flex().flex_col().gap(px(8.));
        if errors.rows.is_empty() {
            content = content.child(empty_line("No failed requests in this range.", theme));
        } else {
            content = content.child(
                div()
                    .h(px(FAILURE_TABLE_H))
                    .w_full()
                    .child(table_element(&state)),
            );
        }
        content = content.child(retry_note(theme));

        section(
            "usage-errors",
            "Failures",
            Some(meta),
            Some(
                div()
                    .text_size(theme.ui_px(10.5))
                    .text_color(theme.text_3)
                    .child(if total_events as usize > shown {
                        format!(
                            "showing the {} most recent of {} events",
                            format::count(shown as u64),
                            format::count(total_events)
                        )
                    } else {
                        format!("{} events", format::count(total_events))
                    })
                    .into_any_element(),
            ),
            content.into_any_element(),
            theme,
        )
    }

    // ── record tables ──────────────────────────────────────────────────────

    /// Day/week/month table. The granularity adapts with the range, so the
    /// same panel is the weekly and monthly trend view for long windows (§42).
    ///
    /// Rendered by GPUI Kit's `Table`: sorting, column resizing and keyboard
    /// navigation come from the framework, the rows come from the snapshot.
    fn buckets_panel(
        &mut self,
        theme: Theme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (_, bucket_table) = self.tables(window, cx);
        self.sync_tables(cx);
        let granularity = self
            .snapshot()
            .map(|snapshot| snapshot.buckets.granularity)
            .unwrap_or(Granularity::Day);
        let title = match granularity {
            Granularity::Day => "Daily breakdown",
            Granularity::Week => "Weekly breakdown",
            Granularity::Month => "Monthly breakdown",
            Granularity::Hour => "Hourly breakdown",
        };
        let rows = self.bucket_rows();
        let meta = format!(
            "{} buckets · by {}",
            format::count(rows.len() as u64),
            granularity.label()
        );
        let _ = theme;

        section(
            "usage-buckets",
            title,
            Some(meta),
            None,
            div()
                .h(px(BUCKET_TABLE_H))
                .w_full()
                .child(table_element(&bucket_table))
                .into_any_element(),
            theme,
        )
    }

    /// The session table: searchable, sortable, virtualized (§48/§49/§50).
    ///
    /// The framework owns the mechanics (virtual rows over thousands of
    /// sessions, resizable columns, keyboard navigation); the page owns the
    /// ordering, so a header click comes back here before anything re-sorts.
    fn sessions_panel(
        &mut self,
        theme: Theme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (session_table, _) = self.tables(window, cx);
        self.sync_tables(cx);
        let search = self.search().clone();
        let shown = self.session_rows(cx).len();
        let total = self
            .snapshot()
            .map(|snapshot| snapshot.sessions.len())
            .unwrap_or(0);
        let meta = if shown == total {
            format!("{} sessions", format::count(total as u64))
        } else {
            format!(
                "{} of {} sessions",
                format::count(shown as u64),
                format::count(total as u64)
            )
        };

        section(
            "usage-sessions",
            "Sessions",
            Some(meta),
            Some(
                div()
                    .w(px(200.))
                    .h(px(26.))
                    .px(px(8.))
                    .rounded(px(6.))
                    .bg(theme.bg_main)
                    .border_1()
                    .border_color(theme.border)
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .child(icon("icons/search.svg", 12., theme.text_3))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(theme.ui_px(12.))
                            .child(search),
                    )
                    .into_any_element(),
            ),
            div()
                .h(px(SESSION_TABLE_H))
                .w_full()
                .child(table_element(&session_table))
                .into_any_element(),
            theme,
        )
    }
}

/// One framework table, configured to look like the rest of the page: no
/// outer border, no stripes (hairline rows instead), compact density, and a
/// vertical scrollbar only — the column plan always fits the width it is given.
fn table_element<D>(state: &Entity<gpui_component::table::TableState<D>>) -> impl IntoElement
where
    D: gpui_component::table::TableDelegate,
{
    Table::new(state)
        .bordered(false)
        .stripe(false)
        .with_size(KitSize::XSmall)
        // Both axes: columns are resizable, so a widened column has to stay
        // reachable.
        .scrollbar_visible(true, true)
        .into_any_element()
}

// ── shared pieces ─────────────────────────────────────────────────────────

/// A page section: an uppercase label with its meta line, optional controls on
/// the right, then content. Sections are separated by hairlines, never cards.
fn section(
    id: &'static str,
    title: &str,
    meta: Option<String>,
    right: Option<AnyElement>,
    content: AnyElement,
    theme: Theme,
) -> AnyElement {
    div()
        .id(SharedString::from(id))
        .flex_none()
        .w_full()
        .pt(px(22.))
        .flex()
        .flex_col()
        .child(
            div()
                .pb(px(12.))
                .flex()
                .items_center()
                .gap(px(10.))
                .child(
                    div()
                        .text_size(theme.ui_px(11.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.text_3)
                        .child(title.to_uppercase()),
                )
                .children(meta.map(|meta| {
                    div()
                        .text_size(theme.ui_px(11.))
                        .text_color(theme.text_3)
                        .child(meta)
                }))
                .child(div().flex_1())
                .children(right),
        )
        .child(content)
        .into_any_element()
}

/// A paired row: left panel with a hairline down its right edge, right panel
/// with matching padding — one surface with a rule, not two cards.
fn paired(left: AnyElement, right: AnyElement) -> AnyElement {
    div()
        .flex_none()
        .w_full()
        .flex()
        .child(left)
        .child(right)
        .into_any_element()
}

fn divide_left(content: AnyElement, theme: Theme) -> AnyElement {
    div()
        .flex_1()
        .min_w_0()
        .pr(px(28.))
        .border_r_1()
        .border_color(theme.border)
        .child(content)
        .into_any_element()
}

fn divide_right(content: AnyElement) -> AnyElement {
    div()
        .flex_1()
        .min_w_0()
        .pl(px(28.))
        .child(content)
        .into_any_element()
}

fn empty_line(text: &str, theme: Theme) -> AnyElement {
    div()
        .px(px(8.))
        .py(px(6.))
        .text_size(theme.ui_px(11.5))
        .text_color(theme.text_3)
        .child(text.to_string())
        .into_any_element()
}

fn retry_note(theme: Theme) -> AnyElement {
    div()
        .px(px(8.))
        .text_size(theme.ui_px(10.5))
        .text_color(theme.text_3)
        .child("Retries are unavailable: pi records automatic retries as live events, not in session files.")
        .into_any_element()
}

fn stat_row(label: &str, value: &str, theme: Theme) -> AnyElement {
    div()
        .px(px(8.))
        .py(px(3.))
        .flex()
        .items_center()
        .gap(px(12.))
        .text_size(theme.ui_px(11.5))
        .child(div().flex_1().text_color(theme.text_3).child(label.to_string()))
        .child(
            div()
                .font(num_font())
                .text_color(theme.text_2)
                .child(value.to_string()),
        )
        .into_any_element()
}

/// A compact bar strip: one bar per bucket, drawn bottom-up (§24).
fn spark_bars(rates: &[Option<f64>], theme: Theme) -> AnyElement {
    div()
        .w_full()
        .h(px(34.))
        .flex()
        .items_end()
        .gap(px(2.))
        .children(rates.iter().map(|rate| {
            div()
                .flex_1()
                .min_w(px(1.))
                .h_full()
                .flex()
                .items_end()
                .child(
                    div()
                        .w_full()
                        .rounded_t(px(2.))
                        .bg(match rate {
                            Some(_) => theme.accent.opacity(0.5),
                            None => theme.trough,
                        })
                        .h(relative(match rate {
                            Some(rate) => (rate / 100.0).clamp(0.02, 1.0) as f32,
                            None => 0.02,
                        })),
                )
                .into_any_element()
        }))
        .into_any_element()
}

/// Tabular figures for numeric columns (§54). Both bundled faces carry `tnum`,
/// and requesting it is what stops columns from jittering as digits change.
pub(super) fn num_font() -> Font {
    let mut font = gpui::font(theme::ui_font_family());
    font.features = FontFeatures(Arc::new(vec![
        ("tnum".to_string(), 1),
        ("lnum".to_string(), 1),
    ]));
    font
}

/// A ranked breakdown row, already reduced to what the view draws.
struct BreakdownRow {
    id: Option<u16>,
    label: String,
    sub: Option<String>,
    value: String,
    fraction: f64,
    color: fn(&Theme) -> Hsla,
}

impl BreakdownRow {
    fn color(&self, theme: &Theme) -> Hsla {
        (self.color)(theme)
    }
}

/// Rank the top rows and pool the tail into one labelled remainder — the shape
/// the distribution panels describe, without hiding what is left.
fn ranked_rows(breakdown: &Breakdown, limit: usize) -> Vec<BreakdownRow> {
    let total_tokens = breakdown.totals.tokens.total;
    let total_requests = breakdown.totals.requests;
    let mut rows: Vec<BreakdownRow> = Vec::new();
    for (ix, row) in breakdown.rows.iter().take(limit).enumerate() {
        rows.push(BreakdownRow {
            id: Some(row.id),
            label: row.label.clone(),
            sub: row.sub.clone(),
            value: format::compact(row.totals.tokens.total),
            fraction: if total_tokens > 0 {
                row.totals.tokens.total as f64 / total_tokens as f64
            } else {
                row.share
            },
            color: series_color(ix),
        });
    }
    let tail: Vec<&GroupRow> = breakdown.rows.iter().skip(limit).collect();
    if !tail.is_empty() {
        let tokens: u64 = tail.iter().map(|row| row.totals.tokens.total).sum();
        let requests: u64 = tail.iter().map(|row| row.totals.requests).sum();
        rows.push(BreakdownRow {
            id: None,
            label: format!("{} more", tail.len()),
            // Say what the remainder covers, so the row is not just a label.
            sub: Some(format!(
                "{} requests",
                format::count(requests.max(total_requests.min(requests)))
            )),
            value: format::compact(tokens),
            fraction: if total_tokens > 0 {
                tokens as f64 / total_tokens as f64
            } else {
                0.0
            },
            color: |theme: &Theme| theme.text_3.opacity(0.5),
        });
    }
    rows
}

/// The accent ramp for ranked rows: one hue at four weights, then neutrals.
/// No rainbow (§17/§63).
fn series_color(ix: usize) -> fn(&Theme) -> Hsla {
    match ix {
        0 => |theme: &Theme| theme.accent,
        1 => |theme: &Theme| theme.accent.opacity(0.7),
        2 => |theme: &Theme| theme.accent.opacity(0.5),
        3 => |theme: &Theme| theme.accent.opacity(0.34),
        4 => |theme: &Theme| theme.text_2,
        _ => |theme: &Theme| theme.text_3,
    }
}

fn class_color(class: ToolClass, theme: Theme) -> Hsla {
    match class {
        ToolClass::Terminal => theme.accent,
        ToolClass::Read => theme.accent.opacity(0.7),
        ToolClass::Edit => theme.accent.opacity(0.5),
        ToolClass::Search => theme.accent.opacity(0.34),
        ToolClass::Web => theme.text_2,
        ToolClass::Plan => theme.text_2.opacity(0.6),
        ToolClass::Ask => theme.text_3,
        ToolClass::Other => theme.text_3.opacity(0.6),
    }
}

fn toggle(list: &mut Vec<u16>, value: u16) {
    match list.iter().position(|entry| *entry == value) {
        Some(ix) => {
            list.remove(ix);
        }
        None => list.push(value),
    }
}

fn session_title(entry: &super::model::SessionEntry) -> String {
    if entry.title.is_empty() {
        format!("Session {}", &entry.id.chars().take(8).collect::<String>())
    } else {
        entry.title.clone()
    }
}

fn model_meta(breakdown: &Breakdown) -> String {
    format!(
        "{} models · {} requests",
        format::count(breakdown.rows.len() as u64),
        format::count(breakdown.totals.requests)
    )
}

fn workspace_meta(breakdown: &Breakdown) -> String {
    format!(
        "{} workspaces · {} tokens",
        format::count(breakdown.rows.len() as u64),
        format::compact(breakdown.totals.tokens.total)
    )
}

fn provider_meta(breakdown: &Breakdown) -> String {
    format!(
        "{} providers · {} requests",
        format::count(breakdown.rows.len() as u64),
        format::count(breakdown.totals.requests)
    )
}

/// Secondary line for a KPI cell: the comparison when one exists, otherwise a
/// plain fact about the range. Direction is carried by an arrow as well as the
/// sign, so it never depends on color (§62).
fn delta_sub(snapshot: &UsageSnapshot, metric: ChartMetric, fallback: &str) -> String {
    let delta = snapshot.delta(metric);
    if delta.unavailable {
        return fallback.to_string();
    }
    // The sign already carries direction; the arrow only earns its place when
    // there is no percentage to read.
    match delta.pct {
        Some(_) => format!("{} vs previous period", format::delta(delta.pct)),
        None => match delta.direction {
            Direction::Up => "up from nothing last period".to_string(),
            Direction::Down => "down from last period".to_string(),
            Direction::Flat => "no change vs previous period".to_string(),
        },
    }
}

fn share_sub(part: u64, total: u64) -> String {
    if total == 0 {
        return "no tokens".into();
    }
    format!("{} of tokens", format::share(part as f64 / total as f64))
}

/// Why a metric is not offered, phrased for the data that is missing.
fn unavailable_reason(unavailable: &[&str]) -> String {
    let mut reasons: Vec<String> = Vec::new();
    if unavailable.contains(&"Cost") {
        reasons.push("cost needs a price table for these models".to_string());
    }
    if unavailable.contains(&"Latency") {
        reasons.push("response time needs measurable request gaps".to_string());
    }
    if reasons.is_empty() {
        String::new()
    } else {
        format!("{} not shown: {}", unavailable.join(", "), reasons.join("; "))
    }
}

/// The metric cell model: label, value, secondary line, and interaction.
struct KpiCell {
    label: String,
    value: String,
    sub: String,
    tone: CellTone,
    click: Option<KpiClick>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CellTone {
    Normal,
    Alert,
    Muted,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum KpiClick {
    ErrorsOnly,
    CachedOnly,
}

// ── export ─────────────────────────────────────────────────────────────────

/// Every request matching the filter, as CSV. The columns are the normalized
/// record; nothing derived is exported as if it were measured.
pub fn export_csv(index: &UsageIndex, filter: &UsageFilter) -> String {
    let mut out = String::from(
        "timestamp,session_id,session,workspace,provider,model,input,output,cache_read,cache_write,total,reasoning,cost_usd,duration_ms,outcome\n",
    );
    for record in &index.requests {
        if !filter.matches_request(index, record) {
            continue;
        }
        let session = index.session(record.session);
        let (provider, model) = index.model_pair(record.model);
        let stamp = super::model::local_datetime(record.ts_ms)
            .map(|dt| dt.to_rfc3339_opts(chrono::SecondsFormat::Secs, false))
            .unwrap_or_default();
        out.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
            csv_field(&stamp),
            csv_field(&session.id),
            csv_field(&session.title),
            csv_field(&index.workspace_of_session(record.session).path),
            csv_field(&provider),
            csv_field(&model),
            record.tokens.input,
            record.tokens.output,
            record.tokens.cache_read,
            record.tokens.cache_write,
            record.tokens.total,
            record.reasoning.unwrap_or(0),
            record
                .cost_usd
                .map(|cost| format!("{cost:.6}"))
                .unwrap_or_default(),
            record.duration_ms.unwrap_or(0),
            match record.outcome {
                super::model::Outcome::Stop => "stop",
                super::model::Outcome::ToolUse => "tool_use",
                super::model::Outcome::Length => "length",
                super::model::Outcome::Error => "error",
                super::model::Outcome::Aborted => "aborted",
            },
        ));
    }
    out
}

fn csv_field(value: &str) -> String {
    if value.contains([',', '"', '\n']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

/// The current view as JSON: the aggregates on screen, respecting the filter
/// and the session search.
pub fn export_json(index: &UsageIndex, snapshot: &UsageSnapshot, search: &str) -> String {
    let summary = &snapshot.summary;
    let totals = &summary.totals;
    // Rows carry the interned id, not the raw one: resolve it so an export
    // can be joined against pi's own session and model identifiers.
    let breakdown = |rows: &[GroupRow], kind: &str| -> Vec<serde_json::Value> {
        rows.iter()
            .map(|row| {
                let raw_id = match kind {
                    "models" => index.model(row.id).id.clone(),
                    "providers" => index.providers[row.id as usize].id.clone(),
                    _ => index.workspaces[row.id as usize].path.clone(),
                };
                serde_json::json!({
                    "id": raw_id,
                    "label": row.label,
                    "sub": row.sub,
                    "requests": row.totals.requests,
                    "tokens": row.totals.tokens.total,
                    "input": row.totals.tokens.input,
                    "output": row.totals.tokens.output,
                    "cache_read": row.totals.tokens.cache_read,
                    "cache_write": row.totals.tokens.cache_write,
                    "cost_usd": row.totals.cost_usd,
                    "share": row.share,
                })
            })
            .collect()
    };
    let needle = search.trim().to_lowercase();
    let sessions: Vec<serde_json::Value> = snapshot
        .sessions
        .iter()
        .filter(|row| {
            needle.is_empty()
                || row.title.to_lowercase().contains(&needle)
                || row.workspace.to_lowercase().contains(&needle)
                || row.top_model.to_lowercase().contains(&needle)
        })
        .map(|row| {
            serde_json::json!({
                "session_id": index.session(row.session).id,
                "title": row.title,
                "workspace": row.workspace,
                "top_model": row.top_model,
                "models": row.models,
                "started_ms": row.started_ms,
                "ended_ms": row.ended_ms,
                "requests": row.totals.requests,
                "input": row.totals.tokens.input,
                "output": row.totals.tokens.output,
                "cache_read": row.totals.tokens.cache_read,
                "cache_write": row.totals.tokens.cache_write,
                "total": row.totals.tokens.total,
                "cost_usd": row.totals.cost_usd,
                "errors": row.totals.errors,
                "tool_calls": row.tool_runs,
                "tool_errors": row.tool_errors,
            })
        })
        .collect();
    let buckets: Vec<serde_json::Value> = snapshot
        .buckets
        .rows
        .iter()
        .map(|row| {
            serde_json::json!({
                "start_ms": row.start_ms,
                "label": row.label,
                "requests": row.totals.requests,
                "tokens": row.totals.tokens.total,
                "input": row.totals.tokens.input,
                "output": row.totals.tokens.output,
                "cache_hit_rate": row.totals.cache_hit_rate(),
                "errors": row.totals.errors,
            })
        })
        .collect();
    let errors: Vec<serde_json::Value> = snapshot
        .errors
        .rows
        .iter()
        .map(|row| {
            serde_json::json!({
                "timestamp_ms": row.ts_ms,
                "session_id": index.session(row.session).id,
                "kind": row.kind.label(),
                "model": index.model(row.model).label,
                "message": row.message,
            })
        })
        .collect();
    let tools: Vec<serde_json::Value> = snapshot
        .tools
        .rows
        .iter()
        .map(|row| {
            serde_json::json!({
                "tool": row.label,
                "class": row.class.label(),
                "calls": row.calls,
                "errors": row.errors,
                "avg_duration_ms": row.avg_duration_ms(),
                "max_duration_ms": row.max_ms,
            })
        })
        .collect();

    let payload = serde_json::json!({
        "generated_at_ms": super::collect::now_ms(),
        "range": {
            "preset": snapshot.filter.range.preset.as_str(),
            "label": snapshot.filter.range.label(),
            "start_ms": snapshot.filter.range.start_ms,
            "end_ms": snapshot.filter.range.end_ms,
            "granularity": snapshot.series.granularity.label(),
        },
        "summary": {
            "requests": totals.requests,
            "turns": summary.turns,
            "sessions": summary.sessions,
            "models": summary.models,
            "providers": summary.providers,
            "workspaces": summary.workspaces,
            "input": totals.tokens.input,
            "output": totals.tokens.output,
            "cache_read": totals.tokens.cache_read,
            "cache_write": totals.tokens.cache_write,
            "total": totals.tokens.total,
            "reasoning": totals.reasoning,
            "cache_hit_rate": totals.cache_hit_rate(),
            "errors": totals.errors,
            "aborted": totals.aborted,
            "avg_duration_ms": totals.avg_duration_ms(),
            "duration_samples": totals.duration_samples,
            "tool_calls": summary.tool_runs,
            "bash_calls": summary.bash_runs,
            "tool_errors": summary.tool_errors,
            "cost_usd": totals.cost_usd,
            "priced_requests": totals.priced_requests,
        },
        "previous": snapshot.previous.as_ref().map(|previous| {
            serde_json::json!({
                "requests": previous.totals.requests,
                "total": previous.totals.tokens.total,
                "input": previous.totals.tokens.input,
                "output": previous.totals.tokens.output,
                "cache_read": previous.totals.tokens.cache_read,
                "cache_write": previous.totals.tokens.cache_write,
                "cost_usd": previous.totals.cost_usd,
                "errors": previous.totals.errors,
                "avg_duration_ms": previous.totals.avg_duration_ms(),
            })
        }),
        "series": snapshot.series.points.iter().map(|point| {
            serde_json::json!({
                "start_ms": point.start_ms,
                "label": point.label,
                "requests": point.totals.requests,
                "input": point.totals.tokens.input,
                "output": point.totals.tokens.output,
                "cache_read": point.totals.tokens.cache_read,
                "cache_write": point.totals.tokens.cache_write,
                "total": point.totals.tokens.total,
                "errors": point.totals.errors,
                "cost_usd": point.totals.cost_usd,
                "avg_duration_ms": point.totals.avg_duration_ms(),
            })
        }).collect::<Vec<_>>(),
        "models": breakdown(&snapshot.models.rows, "models"),
        "providers": breakdown(&snapshot.providers.rows, "providers"),
        "workspaces": breakdown(&snapshot.workspaces.rows, "workspaces"),
        "sessions": sessions,
        "buckets": buckets,
        "tools": tools,
        "errors_detail": errors,
        "insights": snapshot.insights.iter().map(|insight| insight.text.clone()).collect::<Vec<_>>(),
    });
    serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "{}".to_string())
}
