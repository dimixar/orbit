//! The Usage page's rendering: header, filters, and every panel.
//!
//! The view is deliberately dumb. It reads a [`UsageSnapshot`] that was
//! computed elsewhere and formats it; the only logic allowed here is layout,
//! interaction, and choosing which register a number is shown in.
//!
//! # Direction contract — Usage, card-stack redesign
//!
//! **THESIS.** The page is an instrument cluster, not a scroll of lists: each
//! measure owns a bordered card so a metric's domain is spatial, refusing the
//! undifferentiated hairline stack the category defaults to.
//!
//! **OWN-WORLD.** Orbit's instrument panel, inherited whole: canvas ground,
//! `bg_raised` cards at 12px radius, one ember accent on the active series,
//! hairlines only *inside* a card, tabular mono figures for every number.
//!
//! **STORY.** The operator scans the summary, reads the trend, ranks one
//! dimension at a time, checks token health, then drills the record tables.
//! Every figure is pi's own session measurement; nothing is invented.
//!
//! **FIRST VIEWPORT.** 44px header + filter bar, then the Summary card: the
//! metric grid first, the quieter secondary figures one click away. The Trend
//! area chart follows as the first chart, so data starts above the fold.
//!
//! **FORM.** Established world, brief-pinned structure — no concept tournament
//! was run. Composition is a card stack on the canvas; the trend is an area
//! chart, and both the Breakdown dimensions and the Details records are the
//! same shared data table (search, columns, sorting, totals, pagination).
//!
//! Sections: Summary (metric grid) · Signals · Trend (area) · Breakdown (four
//! dimensions, each a data table) · Token composition · Cache · Details.

use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    anchored, deferred, div, prelude::*, px, relative, AnyElement, App, Context, Corner, Entity,
    Font, FontFeatures, FontWeight, Hsla, IntoElement, MouseButton, Render, SharedString, Window,
};

use super::aggregate::{
    Breakdown, ChartMetric, Direction, GroupRow, Insight, LatencyMetric, LatencyStats, SessionRow,
    TimeSeries, Tone, ToolRow, Totals, UsageSnapshot,
};
use super::chart;
use super::filters::{self, FilterOption};
use super::format;
use super::model::{
    next_bucket, Granularity, RangePreset, TimeFocus, TokenCounts, UsageFilter, UsageIndex,
};
use super::page::{
    BreakdownQueryResult, BreakdownSort, BreakdownTab, BucketSort, DetailTab, MenuKind,
    SessionQueryResult, SessionSort, ToolQueryResult, UsagePage,
};
use super::table::{
    cell_shell, data_table, empty_cell, text_cell, Column, FailureSort, SortState, TableHandlers,
    TableKind, ROW_H,
};
use super::tooltip::Tooltip;
use crate::app::icon;
use crate::composer::ComposerInput;
use crate::theme::{self, Theme};

/// Inner column width for a data surface (§58): wide enough for a full table,
/// narrow enough that the eye does not have to travel a metre.
pub(super) const CONTENT_MAX_W: f32 = 1180.;
/// The page column's horizontal padding (both sides), which the tables sit inside.
pub(super) const PAGE_PAD: f32 = 40.;
/// Below this column width the paired panels stack into one column.
const TWO_COLUMN_MIN: f32 = 820.;
const FOUR_KPI_MIN: f32 = 880.;

/// Heights for the tables. Each is a whole number of 26px rows plus the
/// header, so a table never ends by cutting a row in half. All three scroll
/// internally rather than growing without bound.
const BUCKET_TABLE_H: f32 = ROW_H * 11.;
const FAILURE_TABLE_H: f32 = ROW_H * 10.;
/// Ranked bars the Breakdown chart shows; the table below carries the rest.
const CHART_ROWS: usize = 8;

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
                            .children(self.active_filter_bar(theme, cx))
                            .child(
                                div()
                                    .w_full()
                                    .px(px(20.))
                                    .pt(px(18.))
                                    .flex()
                                    .flex_col()
                                    .gap(px(16.))
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
                entity.update(cx, |page, cx| {
                    page.toggle_menu(MenuKind::Export, window, cx)
                });
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
            Some("icons/clock.svg"),
            range.preset != RangePreset::Last7,
            false,
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

        // Active narrowings are shown once, as removable chips, under the
        // filter row (§45); the filter row itself stays a set of controls.
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
            Some("icons/filter.svg"),
            !selected.is_empty(),
            false,
            theme,
            move |_, window, cx| {
                entity.update(cx, |page, cx| page.toggle_menu(kind, window, cx));
            },
        );
        if !open {
            return filters::chip_with_menu(id, chip, false, Corner::TopLeft, || {
                div().into_any_element()
            });
        }
        let panel = filters::multi_menu(self, kind, ranked, all, selected, cx, theme);
        filters::chip_with_menu(id, chip, true, Corner::TopLeft, move || panel)
    }

    /// The active-filter bar (§45): every narrowing filter as a removable
    /// chip, so the user always knows why the numbers changed. Only shown when
    /// something beyond the date range is applied.
    fn active_filter_bar(&self, theme: Theme, cx: &mut Context<Self>) -> Option<AnyElement> {
        let filter = self.filter();
        let index = self.index()?;
        if !filter.has_narrowing() {
            return None;
        }
        let mut chips: Vec<AnyElement> = Vec::new();

        if let Some(focus) = filter.focus {
            let entity = cx.entity();
            chips.push(filters::toggle_chip(
                "usage-chip-focus".to_string(),
                focus.label(),
                theme,
                move |_, _, cx| {
                    entity.update(cx, |page, cx| page.set_focus(None, cx));
                },
            ));
        }
        if let Some(session) = filter.session {
            if let Some(entry) = index.try_session(session) {
                let label = format!("Session: {}", session_title(entry));
                let entity = cx.entity();
                chips.push(filters::toggle_chip(
                    "usage-chip-session".to_string(),
                    label,
                    theme,
                    move |_, _, cx| {
                        entity.update(cx, |page, cx| page.set_session_scope(None, cx));
                    },
                ));
            }
        }
        for id in filter.workspaces.iter().copied() {
            let Some(entry) = index.workspaces.get(id as usize) else {
                continue;
            };
            let entity = cx.entity();
            chips.push(filters::toggle_chip(
                format!("usage-chip-workspace-{id}"),
                format!("Workspace: {}", entry.label),
                theme,
                move |_, _, cx| {
                    entity.update(cx, |page, cx| {
                        page.toggle_filter_value(MenuKind::Workspace, id, cx)
                    });
                },
            ));
        }
        for id in filter.providers.iter().copied() {
            let Some(entry) = index.providers.get(id as usize) else {
                continue;
            };
            let entity = cx.entity();
            chips.push(filters::toggle_chip(
                format!("usage-chip-provider-{id}"),
                format!("Provider: {}", entry.label),
                theme,
                move |_, _, cx| {
                    entity.update(cx, |page, cx| {
                        page.toggle_filter_value(MenuKind::Provider, id, cx)
                    });
                },
            ));
        }
        for id in filter.models.iter().copied() {
            let Some(entry) = index.models.get(id as usize) else {
                continue;
            };
            let entity = cx.entity();
            chips.push(filters::toggle_chip(
                format!("usage-chip-model-{id}"),
                format!("Model: {}", entry.label),
                theme,
                move |_, _, cx| {
                    entity.update(cx, |page, cx| {
                        page.toggle_filter_value(MenuKind::Model, id, cx)
                    });
                },
            ));
        }
        if filter.errors_only {
            let entity = cx.entity();
            chips.push(filters::toggle_chip(
                "usage-chip-errors".to_string(),
                "Failed requests",
                theme,
                move |_, _, cx| {
                    entity.update(cx, |page, cx| page.set_errors_only(false, cx));
                },
            ));
        }
        if filter.cached_only {
            let entity = cx.entity();
            chips.push(filters::toggle_chip(
                "usage-chip-cached".to_string(),
                "Cached only",
                theme,
                move |_, _, cx| {
                    entity.update(cx, |page, cx| page.set_cached_only(false, cx));
                },
            ));
        }

        let entity = cx.entity();
        let clear_all = filters::text_button(
            "usage-chips-clear",
            "Clear all",
            None,
            true,
            theme,
            move |_, _, cx| {
                entity.update(cx, |page, cx| page.clear_filters(cx));
            },
        );

        Some(
            div()
                .id("usage-active-filters")
                .w_full()
                .px(px(20.))
                .pt(px(14.))
                .flex()
                .flex_wrap()
                .items_center()
                .gap(px(6.))
                .child(
                    div()
                        .pr(px(2.))
                        .text_size(theme.ui_px(11.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.text_3)
                        .child("FILTERS"),
                )
                .children(chips)
                .child(clear_all)
                .into_any_element(),
        )
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

        let mut sections: Vec<AnyElement> = vec![self.summary_card(snapshot, theme, kpi_cols, cx)];
        if !snapshot.insights.is_empty() {
            sections.push(self.signals_card(&snapshot.insights, theme));
        }
        sections.push(self.trend_card(snapshot, theme, cx));
        sections.push(self.breakdown_card(snapshot, theme, cx));
        sections.push(self.health_section(snapshot, theme, wide, cx));
        sections.push(self.details_section(snapshot, theme, window, cx));

        div()
            .w_full()
            .flex()
            .flex_col()
            .gap(px(16.))
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
        // A placeholder bar, drawn in the trough so it reads against a card.
        let bar = |width: f32, height: f32| {
            div()
                .w(px(width))
                .h(px(height))
                .rounded(px(4.))
                .bg(theme.trough)
                .into_any_element()
        };
        // One card shell, so the load→loaded swap keeps the same shape: a
        // header strip over a body.
        let shell = |body: AnyElement| {
            div()
                .w_full()
                .flex_none()
                .rounded(px(12.))
                .border_1()
                .border_color(theme.border)
                .bg(theme.bg_raised)
                .flex()
                .flex_col()
                .overflow_hidden()
                .child(
                    div()
                        .flex_none()
                        .h(px(42.))
                        .px(px(14.))
                        .flex()
                        .items_center()
                        .border_b_1()
                        .border_color(theme.border)
                        .child(bar(96., 10.)),
                )
                .child(body)
                .into_any_element()
        };

        // Summary: a metric grid divided by hairlines.
        let mut grid = div()
            .w_full()
            .flex()
            .flex_wrap()
            .gap(px(1.))
            .bg(theme.border);
        for _ in 0..8 {
            grid = grid.child(
                div()
                    .flex_1()
                    .min_w(px(150.))
                    .p(px(14.))
                    .bg(theme.bg_raised)
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(bar(56., 10.))
                    .child(bar(88., 18.))
                    .child(bar(112., 10.)),
            );
        }

        // Trend: the switcher row, then the plot block.
        let trend = div()
            .p(px(14.))
            .w_full()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(bar(320., 26.))
            .child(div().h(px(168.)).w_full().rounded(px(8.)).bg(theme.trough));

        // Records: a header rule and a handful of rows.
        let mut records = div().w_full().pt(px(12.)).flex().flex_col();
        for ix in 0..6 {
            records = records.child(
                div()
                    .h(px(26.))
                    .w_full()
                    .px(px(14.))
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .when(ix > 0, |row| row.border_t_1().border_color(theme.border))
                    .child(bar(140., 10.))
                    .child(div().flex_1())
                    .child(bar(72., 10.)),
            );
        }

        div()
            .w_full()
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(shell(grid.into_any_element()))
            .child(shell(trend.into_any_element()))
            .child(shell(records.into_any_element()))
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

    // ── summary card ───────────────────────────────────────────────────────

    /// The Summary card: the metric grid first, as full-bleed cells divided by
    /// hairlines (a measurement board, not a grid of floating cards), then the
    /// quieter secondary figures behind a disclosure.
    ///
    /// The grid is built as explicit rows of `flex_1` cells rather than one
    /// wrapping container of percentage-width cells: percentage widths inside a
    /// wrapping flex row are resolved against the row's own (content-derived)
    /// width, which let the board grow past the window.
    fn summary_card(
        &self,
        snapshot: &UsageSnapshot,
        theme: Theme,
        cols: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let cells = self.kpi_cells(snapshot);
        let mut grid = div().w_full().flex().flex_col();

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
                    .px(px(14.))
                    .py(px(13.))
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .when(row_ix > 0, |cell| {
                        cell.border_t_1().border_color(theme.border)
                    })
                    .when(col_ix + 1 < chunk.len(), |cell| {
                        cell.border_r_1().border_color(theme.border)
                    })
                    .when(cell.click.is_some(), |cell| {
                        cell.cursor_pointer()
                            .hover(|style| style.bg(theme.bg_hover))
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
                            // Drill-down: jump to the failed requests, or point
                            // the main chart at the metric the card names (§79).
                            KpiClick::ErrorsOnly => page.set_errors_only(true, cx),
                            KpiClick::Metric(metric) => page.set_metric(metric, cx),
                        });
                    });
                }
                row = row.child(element);
            }
            // Pad a short final row so its cells keep the grid's column widths.
            for _ in chunk.len()..cols {
                row = row.child(div().flex_1().min_w(px(150.)).when(row_ix > 0, |cell| {
                    cell.border_t_1().border_color(theme.border)
                }));
            }
            grid = grid.child(row);
        }

        let body = div().w_full().flex().flex_col().child(grid).child(
            div()
                .w_full()
                .px(px(14.))
                .py(px(10.))
                .border_t_1()
                .border_color(theme.border)
                .child(self.summary_strip(snapshot, theme, cx)),
        );

        card(
            "usage-summary",
            "Summary",
            Some(summary_meta(snapshot)),
            None,
            body.into_any_element(),
            theme,
            true,
        )
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
            click: Some(KpiClick::Metric(ChartMetric::Requests)),
        });
        cells.push(KpiCell {
            label: "Total tokens".into(),
            value: format::compact(totals.tokens.total),
            sub: match totals.tokens_per_request() {
                Some(avg) => format!("{} avg/request", format::compact(avg as u64)),
                None => "no requests".into(),
            },
            tone: CellTone::Normal,
            click: Some(KpiClick::Metric(ChartMetric::Tokens)),
        });
        cells.push(KpiCell {
            label: "Input tokens".into(),
            value: format::compact(totals.tokens.input),
            sub: share_sub(totals.tokens.input, totals.tokens.total),
            tone: CellTone::Normal,
            click: Some(KpiClick::Metric(ChartMetric::Input)),
        });
        cells.push(KpiCell {
            label: "Output tokens".into(),
            value: format::compact(totals.tokens.output),
            sub: match totals.reasoning_reported {
                0 => share_sub(totals.tokens.output, totals.tokens.total),
                _ => format!("{} reasoning", format::compact(totals.reasoning)),
            },
            tone: CellTone::Normal,
            click: Some(KpiClick::Metric(ChartMetric::Output)),
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
                // Focus the cache analytics: plot cache volume over time.
                click: Some(KpiClick::Metric(ChartMetric::Cache)),
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
                click: Some(KpiClick::Metric(ChartMetric::Latency)),
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
            click: (coverage > 0.0).then_some(KpiClick::Metric(ChartMetric::Cost)),
        });
        cells
    }

    /// Secondary readout under the board: the metrics that inform the headline
    /// numbers but should not compete with them (§88/§101).
    ///
    /// The headline items stay visible; the long tail is behind a disclosure so
    /// the default view stays quiet. Nothing is dropped — it is one click away.
    fn summary_strip(
        &self,
        snapshot: &UsageSnapshot,
        theme: Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let totals = &snapshot.summary.totals;
        let headline: Vec<(String, String)> = vec![
            ("Turns".into(), format::count(snapshot.summary.turns)),
            ("Sessions".into(), format::count(snapshot.summary.sessions)),
            (
                "Tool calls".into(),
                format::count(snapshot.summary.tool_runs),
            ),
            ("Models".into(), format::count(snapshot.summary.models)),
            (
                "Workspaces".into(),
                format::count(snapshot.summary.workspaces),
            ),
        ];
        let mut extra: Vec<(String, String)> = vec![(
            "Bash commands".into(),
            format::count(snapshot.summary.bash_runs),
        )];
        if snapshot.summary.tool_errors > 0 {
            extra.push((
                "Tool failures".into(),
                format::count(snapshot.summary.tool_errors),
            ));
        }
        if totals.duration_samples > 0 {
            extra.push((
                "Generation time".into(),
                format::span_ms(totals.duration_ms as i64),
            ));
        }
        if let Some(avg) = totals.avg_prompt() {
            extra.push(("Avg prompt".into(), format::compact(avg as u64)));
        }
        if let Some(ratio) = totals.output_input_ratio() {
            // Three decimals: output is routinely a fraction of a percent of
            // the prompt, and "0.00×" would read as zero.
            let text = if ratio > 0.0 && ratio < 0.001 {
                "<0.001×".to_string()
            } else {
                format!("{ratio:.3}×")
            };
            extra.push(("Output / input".into(), text));
        }
        if let Some(per_mtok) = totals.cost_per_mtok() {
            extra.push(("Cost / M tokens".into(), format::cost(per_mtok)));
        }
        if totals.peak_prompt > 0 {
            extra.push(("Peak prompt".into(), format::compact(totals.peak_prompt)));
        }
        if totals.reasoning_reported > 0 {
            extra.push((
                "Reasoning".into(),
                format!("{} of output", format::compact(totals.reasoning)),
            ));
        }

        let open = self.is_summary_open();
        let entity = cx.entity();
        let toggle = filters::text_button(
            "usage-summary-toggle",
            if open {
                "Fewer metrics"
            } else {
                "More metrics"
            },
            if open {
                Some("icons/chevron-up.svg")
            } else {
                Some("icons/chevron-down.svg")
            },
            true,
            theme,
            move |_, _, cx| {
                entity.update(cx, |page, cx| page.toggle_summary(cx));
            },
        );

        let mut values: Vec<AnyElement> = headline
            .into_iter()
            .map(|(label, value)| summary_stat(label, value, theme))
            .collect();
        if open {
            values.extend(
                extra
                    .into_iter()
                    .map(|(label, value)| summary_stat(label, value, theme)),
            );
        }

        div()
            .w_full()
            .flex()
            .items_start()
            .gap(px(12.))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_wrap()
                    .gap_x(px(20.))
                    .gap_y(px(6.))
                    .children(values),
            )
            .child(toggle)
            .into_any_element()
    }

    /// Derived findings, one line each. Their own card so a warning cannot be
    /// mistaken for a metric.
    fn signals_card(&self, insights: &[Insight], theme: Theme) -> AnyElement {
        card(
            "usage-signals",
            "Signals",
            Some("derived from this range".into()),
            None,
            div()
                .p(px(14.))
                .flex()
                .flex_col()
                .gap(px(7.))
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
            false,
        )
    }

    // ── trend ──────────────────────────────────────────────────────────────

    /// The Trend card: one area chart across several measures. The metric
    /// switcher lives in the card header; the chart is the card's whole body,
    /// because the trend is this page's primary read.
    fn trend_card(
        &self,
        snapshot: &UsageSnapshot,
        theme: Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let metric = self.metric();
        let latency_metric = self.latency_metric();
        let by = snapshot.series.granularity.label();
        let meta = match metric {
            ChartMetric::Cost => format!(
                "{} total · by {by}",
                format::cost(metric.total(&snapshot.summary.totals))
            ),
            ChartMetric::Latency => format!(
                "{} per bucket · by {by}",
                latency_metric.label().to_lowercase()
            ),
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
            .bg(theme.bg_main);
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
                        tab.bg(theme.bg_raised)
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

        // Latency register selector (§20): only offered when the window has
        // enough observations for percentiles to be meaningful.
        let latency_selector = (metric == ChartMetric::Latency).then(|| {
            let mut segment = div()
                .flex()
                .items_center()
                .gap(px(2.))
                .p(px(2.))
                .rounded(px(8.))
                .border_1()
                .border_color(theme.border)
                .bg(theme.bg_main);
            for choice in LatencyMetric::ALL {
                let available = choice.available(&snapshot.latency);
                let active = choice == latency_metric;
                let entity = cx.entity();
                segment = segment.child(
                    div()
                        .id(SharedString::from(format!(
                            "usage-latency-{}",
                            choice.as_str()
                        )))
                        .h(px(24.))
                        .px(px(9.))
                        .rounded(px(6.))
                        .flex()
                        .items_center()
                        .text_size(theme.ui_px(11.5))
                        .when(active, |tab| {
                            tab.bg(theme.bg_raised)
                                .text_color(theme.text)
                                .font_weight(FontWeight::MEDIUM)
                        })
                        .when(!active, |tab| tab.text_color(theme.text_3))
                        .when(available && !active, |tab| {
                            tab.cursor_pointer()
                                .hover(|style| style.bg(theme.bg_hover).text_color(theme.text_2))
                                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                    entity
                                        .update(cx, |page, cx| page.set_latency_metric(choice, cx));
                                })
                        })
                        .child(choice.label()),
                );
            }
            segment.into_any_element()
        });

        // "View data" (§52): the same series as a precise table, so the chart's
        // values are always available without hover.
        let data_open = self.is_chart_data_open();
        let view_data = filters::text_button(
            "usage-view-data",
            if data_open { "Hide data" } else { "View data" },
            None,
            true,
            theme,
            {
                let entity = cx.entity();
                move |_, _, cx| {
                    entity.update(cx, |page, cx| page.toggle_chart_data(cx));
                }
            },
        );

        // The metric switcher sits in the body, above the plot: a row of
        // selectors under a heading reads as controls, and no eight-way
        // switcher can overflow a 42px header on a narrow column.
        let selectors = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .flex_wrap()
            .child(tabs)
            .children(latency_selector);

        let hover = self.hover_bucket();
        let entity = cx.entity();
        let on_hover = move |bucket: Option<usize>, _: &mut Window, cx: &mut App| {
            entity.update(cx, |page, cx| page.set_hover_bucket(bucket, cx));
        };

        // Pre-compute each bucket's width so the click handler (which must be
        // `'static`) can scope the page without touching the snapshot.
        let granularity = snapshot.series.granularity;
        let focus_points: Vec<TimeFocus> = snapshot
            .series
            .points
            .iter()
            .map(|point| TimeFocus {
                start_ms: point.start_ms,
                end_ms: next_bucket(point.start_ms, granularity),
                granularity,
            })
            .collect();
        let selected = self.filter().focus.and_then(|focus| {
            snapshot
                .series
                .points
                .iter()
                .position(|p| p.start_ms == focus.start_ms)
        });
        let select_entity = cx.entity();
        let on_select = move |ix: usize, _: &mut Window, cx: &mut App| {
            let Some(focus) = focus_points.get(ix).copied() else {
                return;
            };
            select_entity.update(cx, |page, cx| {
                // Clicking the selected bucket again clears the scope (§73).
                let next = if page.filter().focus == Some(focus) {
                    None
                } else {
                    Some(focus)
                };
                page.set_focus(next, cx);
            });
        };

        let mut content = div()
            .p(px(14.))
            .w_full()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(selectors)
            .child(chart::timeline(
                "usage-timeline",
                &snapshot.series,
                metric,
                latency_metric,
                hover,
                selected,
                theme,
                on_hover,
                on_select,
            ));
        if data_open {
            content = content.child(self.chart_data_table(snapshot, metric, latency_metric, theme));
        }
        if snapshot.latency.samples > 0 {
            content = content.child(self.latency_line(snapshot, theme));
        }
        let reason = unavailable_reason(&unavailable);
        if !reason.is_empty() {
            content = content.child(
                div()
                    .text_size(theme.ui_px(11.))
                    .text_color(theme.text_3)
                    .child(reason),
            );
        }

        card(
            "usage-trend",
            "Usage over time",
            Some(meta),
            Some(view_data),
            content.into_any_element(),
            theme,
            false,
        )
    }

    /// The chart's data as a compact, precise table (§52).
    fn chart_data_table(
        &self,
        snapshot: &UsageSnapshot,
        metric: ChartMetric,
        latency_metric: LatencyMetric,
        theme: Theme,
    ) -> AnyElement {
        let mut body = div()
            .id("usage-chart-data-body")
            .flex()
            .flex_col()
            .max_h(px(240.))
            .overflow_y_scroll();
        let header = |label: &str, right: bool| {
            div()
                .when(right, |cell| cell.text_align(gpui::TextAlign::Right))
                .text_size(theme.ui_px(10.5))
                .font_weight(FontWeight::MEDIUM)
                .text_color(theme.text_3)
                .child(label.to_uppercase())
                .into_any_element()
        };
        body = body.child(
            div()
                .w_full()
                .px(px(4.))
                .py(px(4.))
                .border_b_1()
                .border_color(theme.border)
                .flex()
                .items_center()
                .gap(px(12.))
                .child(div().flex_1().min_w_0().child(header("Time", false)))
                .child(
                    div()
                        .w(px(90.))
                        .flex_none()
                        .child(header(metric.label(), true)),
                )
                .child(div().w(px(72.)).flex_none().child(header("Requests", true)))
                .child(div().w(px(80.)).flex_none().child(header("Tokens", true))),
        );
        for point in &snapshot.series.points {
            let row = |value: String, width: f32, color: Hsla, numeric: bool| {
                div()
                    .w(px(width))
                    .flex_none()
                    .when(numeric, |cell| cell.text_align(gpui::TextAlign::Right))
                    .font(num_font())
                    .text_size(theme.ui_px(11.))
                    .text_color(color)
                    .child(value)
                    .into_any_element()
            };
            body = body.child(
                div()
                    .w_full()
                    .px(px(4.))
                    .py(px(3.))
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(theme.ui_px(11.))
                            .text_color(theme.text_2)
                            .child(point.stamp.clone()),
                    )
                    .child(row(
                        chart::format_point(metric, latency_metric, point),
                        90.,
                        theme.text,
                        true,
                    ))
                    .child(row(
                        format::exact(point.totals.requests),
                        72.,
                        theme.text_3,
                        true,
                    ))
                    .child(row(
                        format::compact(point.totals.tokens.total),
                        80.,
                        theme.text_3,
                        true,
                    )),
            );
        }
        // A hairline-separated readout under the plot, not a card inside a
        // card: the chart's values are the card's own detail.
        div()
            .w_full()
            .pt(px(8.))
            .border_t_1()
            .border_color(theme.border)
            .child(body)
            .into_any_element()
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

    /// The Breakdown card: one dimension at a time, each shown as a ranked bar
    /// chart over a sortable data table. The chart reads the aggregate's own
    /// token ranking; the table sorts independently, so an operator can rank by
    /// requests or cache without disturbing the overview.
    fn breakdown_card(
        &self,
        snapshot: &UsageSnapshot,
        theme: Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let tab = self.breakdown_tab();
        let tabs = segmented(
            "usage-breakdown-tab",
            &BreakdownTab::ALL,
            tab,
            |choice| choice.label(),
            |choice| choice.as_str(),
            theme,
            {
                let entity = cx.entity();
                move |choice, _, cx| {
                    entity.update(cx, |page, cx| page.set_breakdown_tab(choice, cx));
                }
            },
        );

        // The shape first (a ranked bar chart), then the same treatment as the
        // Sessions table: search, a column picker, the table, totals, pagination.
        let (meta, chart, table, totals, footer) = match tab {
            BreakdownTab::Models => {
                let chart_rows = self.breakdown_top(&snapshot.models, CHART_ROWS, cx);
                let result = self.breakdown_result(&snapshot.models, cx);
                (
                    model_meta(&snapshot.models),
                    self.breakdown_chart(
                        &chart_rows,
                        result.totals.tokens.total,
                        "models",
                        theme,
                        cx,
                    ),
                    self.breakdown_table(&result, "models", theme, cx),
                    self.breakdown_totals(&result, theme),
                    self.breakdown_footer(result.page, result.page_size, result.total, theme, cx),
                )
            }
            BreakdownTab::Workspaces => {
                let chart_rows = self.breakdown_top(&snapshot.workspaces, CHART_ROWS, cx);
                let result = self.breakdown_result(&snapshot.workspaces, cx);
                (
                    workspace_meta(&snapshot.workspaces),
                    self.breakdown_chart(
                        &chart_rows,
                        result.totals.tokens.total,
                        "workspaces",
                        theme,
                        cx,
                    ),
                    self.breakdown_table(&result, "workspaces", theme, cx),
                    self.breakdown_totals(&result, theme),
                    self.breakdown_footer(result.page, result.page_size, result.total, theme, cx),
                )
            }
            BreakdownTab::Providers => {
                let chart_rows = self.breakdown_top(&snapshot.providers, CHART_ROWS, cx);
                let result = self.breakdown_result(&snapshot.providers, cx);
                (
                    provider_meta(&snapshot.providers),
                    self.breakdown_chart(
                        &chart_rows,
                        result.totals.tokens.total,
                        "providers",
                        theme,
                        cx,
                    ),
                    self.breakdown_table(&result, "providers", theme, cx),
                    self.breakdown_totals(&result, theme),
                    self.breakdown_footer(result.page, result.page_size, result.total, theme, cx),
                )
            }
            BreakdownTab::Tools => {
                let chart_rows = self.tools_top(snapshot, CHART_ROWS, cx);
                let result = self.tools_result(snapshot, cx);
                (
                    tools_meta(snapshot),
                    self.tools_chart(&chart_rows, result.calls, theme),
                    self.tools_table(&result, theme, cx),
                    self.tools_totals(&result, theme),
                    self.breakdown_footer(result.page, result.page_size, result.total, theme, cx),
                )
            }
        };

        let search = search_box(self.breakdown_search(), theme);
        let toolbar = div()
            .w_full()
            .px(px(14.))
            .pb(px(10.))
            .flex()
            .items_center()
            .gap(px(8.))
            .child(search)
            .child(div().flex_1())
            .child(self.breakdown_columns_control(theme, cx));

        let body = div()
            .w_full()
            .flex()
            .flex_col()
            .child(div().px(px(14.)).pt(px(14.)).pb(px(10.)).child(tabs))
            .child(chart)
            .child(toolbar)
            .child(table)
            .child(totals)
            .child(footer);

        card(
            "usage-breakdowns",
            "Breakdown",
            Some(meta),
            None,
            body.into_any_element(),
            theme,
            false,
        )
    }

    /// The Breakdown column-visibility control, mirroring the Sessions one.
    fn breakdown_columns_control(&self, theme: Theme, cx: &mut Context<Self>) -> AnyElement {
        let open = self.menu() == Some(MenuKind::BreakdownColumns);
        let entity = cx.entity();
        let chip = filters::chip(
            "usage-breakdown-columns-chip",
            "Columns".to_string(),
            Some("icons/panel-right.svg"),
            !self.breakdown_hidden_columns().is_empty(),
            true,
            theme,
            move |_, window, cx| {
                entity.update(cx, |page, cx| {
                    page.toggle_menu(MenuKind::BreakdownColumns, window, cx)
                });
            },
        );
        if !open {
            return filters::chip_with_menu(
                "usage-breakdown-columns-anchor",
                chip,
                false,
                Corner::TopRight,
                || div().into_any_element(),
            );
        }
        let panel = filters::breakdown_columns_menu(self, cx, theme);
        filters::chip_with_menu(
            "usage-breakdown-columns-anchor",
            chip,
            true,
            Corner::TopRight,
            move || panel,
        )
    }

    /// The totals line under a token-dimension table.
    fn breakdown_totals(&self, result: &BreakdownQueryResult, theme: Theme) -> AnyElement {
        let page_requests: u64 = result.rows.iter().map(|row| row.totals.requests).sum();
        let page_tokens: u64 = result.rows.iter().map(|row| row.totals.tokens.total).sum();
        let mut filtered = format!(
            "Filtered totals: {} requests · {} tokens",
            format::count(result.totals.requests),
            format::compact(result.totals.tokens.total)
        );
        if result.totals.cost_coverage() > 0.0 {
            filtered.push_str(&format!(" · {}", format::cost(result.totals.cost_usd)));
        }
        let page = format!(
            "this page: {} requests · {} tokens",
            format::count(page_requests),
            format::compact(page_tokens)
        );
        totals_row(&filtered, &page, theme)
    }

    /// The totals line under the Tools table.
    fn tools_totals(&self, result: &ToolQueryResult, theme: Theme) -> AnyElement {
        let page_calls: u64 = result.rows.iter().map(|row| row.calls).sum();
        let page_errors: u64 = result.rows.iter().map(|row| row.errors).sum();
        let filtered = format!(
            "Filtered totals: {} calls · {} failed",
            format::count(result.calls),
            format::count(result.errors)
        );
        let page = format!(
            "this page: {} calls · {} failed",
            format::count(page_calls),
            format::count(page_errors)
        );
        totals_row(&filtered, &page, theme)
    }

    /// The breakdown footer: range, rows-per-page, and page stepping — the same
    /// shape as the Sessions footer, driven by the open dimension's state.
    fn breakdown_footer(
        &self,
        page: usize,
        page_size: usize,
        total: usize,
        theme: Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let pages = total.div_ceil(page_size.max(1)).max(1);
        let current = page.clamp(1, pages);
        let start = (current - 1) * page_size;
        let shown = page_size.min(total.saturating_sub(start));
        let summary = if total == 0 {
            "No rows".to_string()
        } else {
            format!(
                "Showing {}–{} of {}",
                format::count((start + 1) as u64),
                format::count((start + shown) as u64),
                format::count(total as u64)
            )
        };

        let size_open = self.menu() == Some(MenuKind::BreakdownPageSize);
        let entity = cx.entity();
        let size_chip = filters::chip(
            "usage-breakdown-page-size-chip",
            page_size.to_string(),
            None,
            false,
            true,
            theme,
            move |_, window, cx| {
                entity.update(cx, |page, cx| {
                    page.toggle_menu(MenuKind::BreakdownPageSize, window, cx)
                });
            },
        );
        let size_panel = size_open.then(|| filters::breakdown_page_size_menu(self, cx, theme));
        let size_control = filters::chip_with_menu(
            "usage-breakdown-page-size-anchor",
            size_chip,
            size_open,
            Corner::TopRight,
            move || size_panel.unwrap_or_else(|| div().into_any_element()),
        );

        let prev = filters::text_button(
            "usage-breakdown-page-prev",
            "Previous",
            Some("icons/chevron-left.svg"),
            current > 1,
            theme,
            {
                let entity = cx.entity();
                move |_, _, cx| {
                    entity.update(cx, |page, cx| {
                        page.set_breakdown_page(current.saturating_sub(1), cx)
                    });
                }
            },
        );
        let next = filters::text_button(
            "usage-breakdown-page-next",
            "Next",
            Some("icons/chevron-right.svg"),
            current < pages,
            theme,
            {
                let entity = cx.entity();
                move |_, _, cx| {
                    entity.update(cx, |page, cx| page.set_breakdown_page(current + 1, cx));
                }
            },
        );

        div()
            .w_full()
            .px(px(14.))
            .pt(px(10.))
            .pb(px(14.))
            .flex()
            .items_center()
            .gap(px(10.))
            .text_size(theme.ui_px(11.5))
            .child(div().text_color(theme.text_3).child(summary))
            .child(div().flex_1())
            .child(div().text_color(theme.text_3).child("Rows per page"))
            .child(size_control)
            .child(prev)
            .child(
                div()
                    .px(px(4.))
                    .font(num_font())
                    .text_color(theme.text_2)
                    .child(format!("{current} / {pages}")),
            )
            .child(next)
            .into_any_element()
    }

    /// One page of a token dimension as a standard data table — the same
    /// component and column shape as the Sessions table. Rows are clickable to
    /// scope the page to that model / workspace / provider.
    fn breakdown_table(
        &self,
        result: &BreakdownQueryResult,
        kind: &'static str,
        theme: Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let rows = &result.rows;
        let (columns, keys) = self.breakdown_columns();
        let total_w: f32 = columns.iter().map(|column| column.width).sum();
        let total_tokens = result.totals.tokens.total;
        let entity = cx.entity();
        let mut elements = Vec::with_capacity(rows.len());
        for row in rows {
            let target = row.id;
            let mut line = div()
                .id(SharedString::from(format!(
                    "usage-breakdown-{kind}-{}",
                    row.label
                )))
                .h(px(ROW_H))
                .w_full()
                .min_w(px(total_w))
                .flex()
                .items_center()
                .border_b_1()
                .border_color(theme.border)
                .cursor_pointer()
                .hover(|style| style.bg(theme.bg_hover));
            for column in &columns {
                let cell = match column.id {
                    "name" => breakdown_name_cell(column, row, theme),
                    "requests" => text_cell(
                        column,
                        format::count(row.totals.requests),
                        theme.text_2,
                        theme,
                    ),
                    "input" => text_cell(
                        column,
                        format::compact(row.totals.tokens.input),
                        theme.text_3,
                        theme,
                    ),
                    "output" => text_cell(
                        column,
                        format::compact(row.totals.tokens.output),
                        theme.text_3,
                        theme,
                    ),
                    "cache" => text_cell(
                        column,
                        format::compact(row.totals.tokens.cache_read),
                        theme.text_3,
                        theme,
                    ),
                    "share" => {
                        let share = if total_tokens > 0 {
                            row.totals.tokens.total as f64 / total_tokens as f64
                        } else {
                            row.share
                        };
                        text_cell(column, format::share(share), theme.text_3, theme)
                    }
                    _ => text_cell(
                        column,
                        format::compact(row.totals.tokens.total),
                        theme.text,
                        theme,
                    ),
                };
                line = line.child(cell);
            }
            let click_entity = entity.clone();
            line = line.on_mouse_down(MouseButton::Left, move |_, _, cx| {
                click_entity.update(cx, |page, cx| {
                    let mut filter = page.filter().clone();
                    match kind {
                        "workspaces" => toggle(&mut filter.workspaces, target),
                        "providers" => toggle(&mut filter.providers, target),
                        _ => toggle(&mut filter.models, target),
                    }
                    page.set_filter(filter, cx);
                });
            });
            elements.push(line.into_any_element());
        }
        // The table grows with its page: choosing 100 rows shows 100, not a
        // ten-row viewport. The page scrolls; the table never does.
        let visible = if result.rows.is_empty() {
            3
        } else {
            result.rows.len()
        } as f32;
        let height = (ROW_H * visible) + 28.;
        self.table_element(
            TableKind::Breakdown,
            "usage-breakdown-table",
            columns,
            elements,
            height,
            empty_cell("No rows match this search.", theme),
            theme,
            Rc::new(move |page, ix, sort, cx| {
                let Some(Some(key)) = keys.get(ix).copied() else {
                    return;
                };
                match sort {
                    SortState::Ascending => page.set_breakdown_sort(key, false, cx),
                    SortState::Descending => page.set_breakdown_sort(key, true, cx),
                    SortState::Default => page.set_breakdown_sort(BreakdownSort::Tokens, true, cx),
                }
            }),
            cx,
        )
    }

    /// The ranked bar chart above a token dimension's table: the top rows as
    /// magnitude bars, so the shape reads before the figures. Rows are clickable
    /// to scope the page exactly like the table's rows.
    fn breakdown_chart(
        &self,
        rows: &[GroupRow],
        total: u64,
        kind: &'static str,
        theme: Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut chart = div()
            .w_full()
            .px(px(14.))
            .pb(px(12.))
            .flex()
            .flex_col()
            .gap(px(2.));
        if rows.is_empty() {
            chart = chart.child(empty_line("No usage in this range.", theme));
        }
        for row in rows {
            let fraction = if total > 0 {
                row.totals.tokens.total as f64 / total as f64
            } else {
                row.share
            };
            let target = row.id;
            let entity = cx.entity();
            chart = chart.child(
                div()
                    .id(SharedString::from(format!(
                        "usage-breakdown-chart-{kind}-{}",
                        row.label
                    )))
                    .px(px(8.))
                    .py(px(5.))
                    .rounded(px(6.))
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .cursor_pointer()
                    .hover(|style| style.bg(theme.bg_hover))
                    .child(chart_label(&row.label, row.sub.as_deref(), theme))
                    .child(bar_track(fraction, theme))
                    .child(chart_value(
                        &format::compact(row.totals.tokens.total),
                        theme,
                    ))
                    .child(chart_share(&format::share(fraction), theme))
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        entity.update(cx, |page, cx| {
                            let mut filter = page.filter().clone();
                            match kind {
                                "workspaces" => toggle(&mut filter.workspaces, target),
                                "providers" => toggle(&mut filter.providers, target),
                                _ => toggle(&mut filter.models, target),
                            }
                            page.set_filter(filter, cx);
                        });
                    }),
            );
        }
        chart.into_any_element()
    }

    /// The ranked bar chart above the Tools table: the busiest tools as bars.
    fn tools_chart(&self, rows: &[ToolRow], total: u64, theme: Theme) -> AnyElement {
        let mut chart = div()
            .w_full()
            .px(px(14.))
            .pb(px(12.))
            .flex()
            .flex_col()
            .gap(px(2.));
        if rows.is_empty() {
            chart = chart.child(empty_line("No tool calls in this range.", theme));
        }
        for row in rows {
            let fraction = if total > 0 {
                row.calls as f64 / total as f64
            } else {
                0.0
            };
            let tooltip = format!(
                "{} · {} calls · {} errors · avg {}",
                row.label,
                format::count(row.calls),
                format::count(row.errors),
                row.avg_duration_ms()
                    .map(format::duration_ms)
                    .unwrap_or_else(|| "—".into())
            );
            chart = chart.child(
                div()
                    .id(SharedString::from(format!(
                        "usage-tools-chart-{}",
                        row.label
                    )))
                    .tooltip(move |_, cx| cx.new(|_| Tooltip::new(tooltip.clone())).into())
                    .px(px(8.))
                    .py(px(5.))
                    .rounded(px(6.))
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(chart_label(&row.label, Some(row.class.label()), theme))
                    .child(bar_track(fraction, theme))
                    .child(chart_value(&format::count(row.calls), theme))
                    .child(chart_share(&format::share(fraction), theme)),
            );
        }
        chart.into_any_element()
    }

    /// The tools data table: every tool with its family, calls, failures, and
    /// duration. Ordered busiest first, the aggregate's own ranking.
    fn tools_table(
        &self,
        result: &ToolQueryResult,
        theme: Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let columns = self.tool_columns();
        let total_w: f32 = columns.iter().map(|column| column.width).sum();
        let mut elements = Vec::with_capacity(result.rows.len());
        for row in &result.rows {
            let mut line = div()
                .h(px(ROW_H))
                .w_full()
                .min_w(px(total_w))
                .flex()
                .items_center()
                .border_b_1()
                .border_color(theme.border)
                .hover(|style| style.bg(theme.bg_hover));
            for column in &columns {
                let cell = match column.id {
                    "tool" => cell_shell(column)
                        .gap(px(8.))
                        .child(
                            div()
                                .flex_none()
                                .max_w(px(column.width - 96.))
                                .truncate()
                                .text_size(theme.ui_px(12.))
                                .text_color(theme.text)
                                .child(row.label.clone()),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .text_size(theme.ui_px(10.5))
                                .text_color(theme.text_3)
                                .child(row.class.label()),
                        )
                        .into_any_element(),
                    "calls" => text_cell(column, format::count(row.calls), theme.text_2, theme),
                    "errors" => text_cell(
                        column,
                        if row.errors > 0 {
                            format::count(row.errors)
                        } else {
                            "—".into()
                        },
                        if row.errors > 0 {
                            theme.crit
                        } else {
                            theme.text_3
                        },
                        theme,
                    ),
                    "avg" => text_cell(
                        column,
                        row.avg_duration_ms()
                            .map(format::duration_ms)
                            .unwrap_or_else(|| "—".into()),
                        theme.text_3,
                        theme,
                    ),
                    _ => text_cell(
                        column,
                        if row.max_ms > 0 {
                            format::duration_ms(row.max_ms as f64)
                        } else {
                            "—".into()
                        },
                        theme.text_3,
                        theme,
                    ),
                };
                line = line.child(cell);
            }
            elements.push(line.into_any_element());
        }
        let visible = if result.rows.is_empty() {
            3
        } else {
            result.rows.len()
        } as f32;
        let height = (ROW_H * visible) + 28.;
        self.table_element(
            TableKind::Breakdown,
            "usage-tools-table",
            columns,
            elements,
            height,
            empty_cell("No tools match this search.", theme),
            theme,
            Rc::new(|_: &mut UsagePage, _: usize, _: SortState, _: &mut Context<UsagePage>| {}),
            cx,
        )
    }

    /// Token composition: input / output / cache read / cache write, as one
    /// stacked bar plus its four rows. The four categories sum to the total,
    /// so nothing is double-counted (§23).
    fn composition_panel(
        &self,
        snapshot: &UsageSnapshot,
        theme: Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let tokens = snapshot.summary.totals.tokens;
        let total = tokens.total;
        let slices: [(&str, u64, Hsla, ChartMetric); 4] = [
            ("Input", tokens.input, theme.accent, ChartMetric::Input),
            (
                "Output",
                tokens.output,
                theme.accent.opacity(0.62),
                ChartMetric::Output,
            ),
            (
                "Cache read",
                tokens.cache_read,
                theme.accent.opacity(0.40),
                ChartMetric::Cache,
            ),
            (
                "Cache write",
                tokens.cache_write,
                theme.accent.opacity(0.22),
                ChartMetric::Cache,
            ),
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
        for (_, value, color, _) in slices {
            if value == 0 || total == 0 {
                continue;
            }
            stack = stack.child(
                div()
                    .h_full()
                    .w(relative(value as f32 / total as f32))
                    .bg(color),
            );
        }

        let mut content = div().flex().flex_col().gap(px(10.)).child(stack);
        for (label, value, color, metric) in slices {
            // Clicking a component points the main chart at it (§19); hover
            // surfaces the exact figure (§42).
            let entity = cx.entity();
            let share = if total == 0 {
                "—".to_string()
            } else {
                format::share(value as f64 / total as f64)
            };
            let tooltip = format!("{label}: {} tokens · {share}", format::exact(value));
            content = content.child(
                div()
                    .id(SharedString::from(format!("usage-composition-{label}")))
                    .tooltip(move |_, cx| cx.new(|_| Tooltip::new(tooltip.clone())).into())
                    .px(px(8.))
                    .py(px(4.))
                    .rounded(px(6.))
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .cursor_pointer()
                    .hover(|style| style.bg(theme.bg_hover))
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        entity.update(cx, |page, cx| page.set_metric(metric, cx));
                    })
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
                            .child(share),
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

        card(
            "usage-composition",
            "Token composition",
            Some(meta),
            None,
            div().p(px(14.)).child(content).into_any_element(),
            theme,
            false,
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
            return card(
                "usage-cache",
                "Cache performance",
                Some(meta),
                None,
                div().p(px(14.)).child(content).into_any_element(),
                theme,
                false,
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
            div().flex().flex_col().gap(px(2.)).children(
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
        // Each bar carries its own hover readout — the strip is a measurement,
        // not decoration — and a bucket with no cache traffic draws as a stub.
        let readable = snapshot
            .series
            .points
            .iter()
            .filter(|point| point.totals.tokens.input + point.totals.tokens.cache_read > 0)
            .count();
        if readable >= 2 {
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
                    .child(hit_rate_bars(&snapshot.series, theme)),
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

        card(
            "usage-cache",
            "Cache performance",
            Some(meta),
            None,
            div().p(px(14.)).child(content).into_any_element(),
            theme,
            false,
        )
    }

    // ── health ─────────────────────────────────────────────────────────────

    /// Two token-health cards share one row on wide layouts: composition (what
    /// the tokens were) beside cache performance (how much was reused). They
    /// stay separate cards so neither reads as a subordinate of the other.
    fn health_section(
        &self,
        snapshot: &UsageSnapshot,
        theme: Theme,
        wide: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let composition = self.composition_panel(snapshot, theme, cx);
        let cache = self.cache_panel(snapshot, theme, cx);
        if wide {
            div()
                .w_full()
                .flex()
                .gap(px(16.))
                .child(div().flex_1().min_w_0().child(composition))
                .child(div().flex_1().min_w_0().child(cache))
                .into_any_element()
        } else {
            div()
                .w_full()
                .flex()
                .flex_col()
                .gap(px(16.))
                .child(composition)
                .child(cache)
                .into_any_element()
        }
    }

    // ── records ────────────────────────────────────────────────────────────

    /// One records section, three tables behind a selector: the searchable
    /// session list, the day/week/month rollup, and the failure log. Keeps the
    /// page from ending in three stacked, mostly-empty table sections.
    fn details_section(
        &mut self,
        snapshot: &UsageSnapshot,
        theme: Theme,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let tab = self.detail_tab();
        let tabs = segmented(
            "usage-detail-tab",
            &DetailTab::ALL,
            tab,
            |choice| choice.label(),
            |choice| choice.as_str(),
            theme,
            {
                let entity = cx.entity();
                move |choice, _, cx| {
                    entity.update(cx, |page, cx| page.set_detail_tab(choice, cx));
                }
            },
        );
        let tabs_row = div().px(px(14.)).pt(px(14.)).pb(px(10.)).child(tabs);

        let (meta, content) = match tab {
            DetailTab::Sessions => {
                let result = self.session_page(cx);
                let all = snapshot.sessions.len();
                let meta = if result.total == all {
                    format!("{} sessions", format::count(all as u64))
                } else {
                    format!(
                        "{} of {} sessions",
                        format::count(result.total as u64),
                        format::count(all as u64)
                    )
                };
                let table = self.sessions_content(&result, theme, cx);
                let toolbar = div()
                    .w_full()
                    .px(px(14.))
                    .pb(px(10.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(self.session_search_box(theme))
                    .child(div().flex_1())
                    .child(self.columns_control(theme, cx));
                (
                    meta,
                    div()
                        .w_full()
                        .flex()
                        .flex_col()
                        .child(tabs_row)
                        .child(toolbar)
                        .child(table)
                        .into_any_element(),
                )
            }
            DetailTab::Daily => {
                let granularity = snapshot.buckets.granularity;
                let rows = self.bucket_rows();
                let meta = format!(
                    "{} buckets · by {}",
                    format::count(rows.len() as u64),
                    granularity.label()
                );
                (
                    meta,
                    div()
                        .w_full()
                        .flex()
                        .flex_col()
                        .child(tabs_row)
                        .child(self.buckets_content(snapshot, theme, cx))
                        .into_any_element(),
                )
            }
            DetailTab::Failures => {
                let errors = &snapshot.errors;
                // Two different measures, both real: recorded failure events
                // (provider errors pi logged, including ones a retry later
                // recovered from) and requests whose stop reason was an error.
                let meta = format!(
                    "{} provider errors recorded · {} tool failures · {} stopped",
                    format::count(errors.provider),
                    format::count(errors.tool),
                    format::count(errors.aborted)
                );
                (
                    meta,
                    div()
                        .w_full()
                        .flex()
                        .flex_col()
                        .child(tabs_row)
                        .child(self.failures_content(snapshot, theme, cx))
                        .into_any_element(),
                )
            }
        };

        card(
            "usage-details",
            "Details",
            Some(meta),
            None,
            content,
            theme,
            false,
        )
    }

    /// The sessions search field, as it appears in the details header.
    fn session_search_box(&self, theme: Theme) -> AnyElement {
        search_box(self.search(), theme)
    }

    /// The session table: searchable, sortable, pageable (§48/§49/§50).
    ///
    /// The page owns the ordering, so a header click comes back here before
    /// anything re-sorts; the table just draws the page it is handed.
    fn sessions_content(
        &self,
        result: &SessionQueryResult,
        theme: Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        // The viewport shows at most 20 rows; a page of 25+ scrolls within it,
        // exactly like a desktop data grid (§38).
        let visible_rows = result.page_size.min(20) as f32;
        let table_h = ROW_H * visible_rows + 28.;
        let footer = self.pagination_footer(result, theme, cx);

        let (columns, keys) = self.session_columns();
        let rows = self.session_row_elements(&result.rows, &columns, &keys, theme, cx);
        let table = self.table_element(
            TableKind::Sessions,
            "usage-sessions-table",
            columns,
            rows,
            table_h,
            empty_cell("No sessions match this search.", theme),
            theme,
            Rc::new(
                move |page, ix, sort, cx| match (keys.get(ix).copied().flatten(), sort) {
                    (Some(key), SortState::Ascending) => page.set_session_sort(key, false, cx),
                    (Some(key), SortState::Descending) => page.set_session_sort(key, true, cx),
                    _ => page.set_session_sort(SessionSort::Tokens, true, cx),
                },
            ),
            cx,
        );

        div()
            .w_full()
            .flex()
            .flex_col()
            .child(div().h(px(table_h)).w_full().child(table))
            .children(self.totals_line(result, theme))
            .child(footer)
            .into_any_element()
    }

    /// Day/week/month table. The granularity adapts with the range, so the
    /// same table is the weekly and monthly trend view for long windows (§42).
    fn buckets_content(
        &self,
        snapshot: &UsageSnapshot,
        theme: Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let granularity = snapshot.buckets.granularity;
        let (columns, keys) = self.bucket_columns();
        let rows = self.bucket_row_elements(&columns, &keys, granularity, theme);
        let table = self.table_element(
            TableKind::Buckets,
            "usage-buckets-table",
            columns,
            rows,
            BUCKET_TABLE_H,
            empty_cell("No buckets in this range.", theme),
            theme,
            Rc::new(
                move |page, ix, sort, cx| match (keys.get(ix).copied(), sort) {
                    (Some(key), SortState::Ascending) => page.set_bucket_sort(key, false, cx),
                    (Some(key), SortState::Descending) => page.set_bucket_sort(key, true, cx),
                    _ => page.set_bucket_sort(BucketSort::Date, true, cx),
                },
            ),
            cx,
        );
        // Bottom breathing room, matching the card's gutter, so the last row's
        // hover fill stays clear of the rounded corners.
        div().w_full().pb(px(14.)).child(table).into_any_element()
    }

    /// Failures (§34): every provider error and tool failure in the range, in
    /// the same table as the rest of the page.
    fn failures_content(
        &self,
        snapshot: &UsageSnapshot,
        theme: Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let errors = &snapshot.errors;
        let shown = errors.rows.len();
        let total_events = errors.provider + errors.tool;

        let mut content = div().w_full().flex().flex_col().gap(px(8.));
        if errors.rows.is_empty() {
            content = content.child(
                div()
                    .px(px(14.))
                    .py(px(8.))
                    .text_size(theme.ui_px(11.5))
                    .text_color(theme.text_3)
                    .child("No failed requests in this range."),
            );
        } else {
            let table = self.failure_table_element(theme, cx);
            content = content.child(div().h(px(FAILURE_TABLE_H)).w_full().child(table));
        }
        content = content.child(retry_note(theme)).child(
            div()
                .px(px(14.))
                .pb(px(14.))
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
                }),
        );
        content.into_any_element()
    }

    // ── column plans ───────────────────────────────────────────────────────

    /// Session columns plus their sort keys (the trailing affordance is not
    /// sortable). Widths come from the plan, overridden by any divider drag.
    fn session_columns(&self) -> (Vec<Column>, Vec<Option<SessionSort>>) {
        // Room the table steals for its scrollbar, hairlines and cell padding.
        const CHROME: f32 = 48.;
        const GAPS: f32 = 12.;
        const TITLE_MIN: f32 = 180.;
        const OPEN_W: f32 = 38.;
        let budget = (self.table_width() - CHROME).max(360.);
        let hidden = self.hidden_columns();
        let (sort, desc) = self.session_sort_state();

        // `true` marks a column dropped when the window is too narrow (and
        // hidden by the user on request).
        let plan: [(SessionSort, f32, bool); 12] = [
            (SessionSort::Title, 0., false),
            (SessionSort::Workspace, 110., false),
            (SessionSort::Provider, 110., false),
            (SessionSort::Model, 156., false),
            (SessionSort::Started, 88., true),
            (SessionSort::Duration, 92., false),
            (SessionSort::Requests, 92., false),
            (SessionSort::Input, 70., true),
            (SessionSort::Output, 76., true),
            (SessionSort::Cache, 72., true),
            (SessionSort::Tokens, 82., false),
            (SessionSort::Errors, 82., false),
        ];
        let keep: Vec<(SessionSort, f32, bool)> = plan
            .into_iter()
            .filter(|(key, _, _)| *key == SessionSort::Title || !hidden.contains(key))
            .collect();

        // Optional columns are added in priority order while the title still
        // breathes: the per-bucket token columns are the first to go.
        let mut running: f32 = keep
            .iter()
            .filter(|(key, _, optional)| !*optional && *key != SessionSort::Title)
            .map(|(key, default, _)| self.col_width(TableKind::Sessions, key.as_str(), *default))
            .sum();
        let mut keep_optional: Vec<SessionSort> = Vec::new();
        for (key, default, optional) in &keep {
            if !*optional {
                continue;
            }
            let width = self.col_width(TableKind::Sessions, key.as_str(), *default);
            if running + width + OPEN_W + GAPS + TITLE_MIN <= budget {
                running += width;
                keep_optional.push(*key);
            }
        }
        let title_default = (budget - running - OPEN_W - GAPS).clamp(TITLE_MIN, 460.);

        let mut columns = Vec::with_capacity(keep.len() + 1);
        let mut keys = Vec::with_capacity(keep.len() + 1);
        for (key, default, optional) in keep {
            if optional && !keep_optional.contains(&key) {
                continue;
            }
            let default = if key == SessionSort::Title {
                title_default
            } else {
                default
            };
            let width = self.col_width(TableKind::Sessions, key.as_str(), default);
            let numeric = !matches!(
                key,
                SessionSort::Title
                    | SessionSort::Workspace
                    | SessionSort::Provider
                    | SessionSort::Model
            );
            let mut column = Column::new(key.as_str(), key.label())
                .width(width)
                .sortable()
                .sort(sort_state(key == sort, desc));
            if numeric {
                column = column.numeric();
            }
            columns.push(column);
            keys.push(Some(key));
        }
        // The trailing affordance: no header label, no sorting, fixed width.
        columns.push(Column::new("open", "").width(OPEN_W));
        keys.push(None);
        (columns, keys)
    }

    /// The token breakdown table's columns plus their sort keys — the same
    /// shape as the Sessions table: a name column that takes the remainder,
    /// then fixed numeric columns, with Share as the trailing display column.
    fn breakdown_columns(&self) -> (Vec<Column>, Vec<Option<BreakdownSort>>) {
        let hidden = self.breakdown_hidden_columns();
        let is_hidden = |id: &'static str| hidden.contains(&id);
        let measures: [(BreakdownSort, f32); 5] = [
            (BreakdownSort::Requests, 92.),
            (BreakdownSort::Input, 80.),
            (BreakdownSort::Output, 80.),
            (BreakdownSort::Cache, 80.),
            (BreakdownSort::Tokens, 88.),
        ];
        let mut fixed = 0.;
        for (key, default) in measures {
            if !is_hidden(key.as_str()) {
                fixed += default;
            }
        }
        if !is_hidden("share") {
            fixed += 72.;
        }
        let budget = self.table_width();
        let name_w = (budget - fixed).clamp(160., 460.);
        let (sort, desc) = self.breakdown_sort_state();
        let mut columns = Vec::with_capacity(7);
        let mut keys = Vec::with_capacity(7);
        columns.push(
            Column::new(BreakdownSort::Name.as_str(), BreakdownSort::Name.label())
                .width(self.col_width(TableKind::Breakdown, "name", name_w))
                .sortable()
                .sort(sort_state(sort == BreakdownSort::Name, desc)),
        );
        keys.push(Some(BreakdownSort::Name));
        for (key, default) in measures {
            if is_hidden(key.as_str()) {
                continue;
            }
            columns.push(
                Column::new(key.as_str(), key.label())
                    .width(self.col_width(TableKind::Breakdown, key.as_str(), default))
                    .numeric()
                    .sortable()
                    .sort(sort_state(key == sort, desc)),
            );
            keys.push(Some(key));
        }
        if !is_hidden("share") {
            columns.push(
                Column::new("share", "Share")
                    .width(self.col_width(TableKind::Breakdown, "share", 72.))
                    .numeric(),
            );
            keys.push(None);
        }
        (columns, keys)
    }

    /// The tools table's columns, with any hidden ones dropped. The tool name
    /// takes the room left over; every measure is its own narrow column.
    fn tool_columns(&self) -> Vec<Column> {
        let hidden = self.breakdown_hidden_columns();
        let is_hidden = |id: &'static str| hidden.contains(&id);
        let measures: [(&'static str, &'static str, f32); 4] = [
            ("calls", "Calls", 76.),
            ("errors", "Errors", 80.),
            ("avg", "Avg", 76.),
            ("max", "Max", 76.),
        ];
        let mut fixed = 0.;
        for (id, _, default) in measures {
            if !is_hidden(id) {
                fixed += default;
            }
        }
        let budget = self.table_width();
        let name_w = (budget - fixed).clamp(160., 460.);
        let mut columns = vec![Column::new("tool", "Tool").width(name_w)];
        for (id, label, default) in measures {
            if !is_hidden(id) {
                columns.push(Column::new(id, label).width(default).numeric());
            }
        }
        columns
    }

    /// Breakdown columns plus their sort keys.
    fn bucket_columns(&self) -> (Vec<Column>, Vec<BucketSort>) {
        let plan: [(BucketSort, f32, bool); 5] = [
            (BucketSort::Date, 220., false),
            (BucketSort::Requests, 104., true),
            (BucketSort::Tokens, 96., true),
            (BucketSort::Cache, 104., true),
            (BucketSort::Errors, 84., true),
        ];
        let mut columns = Vec::with_capacity(plan.len());
        let mut keys = Vec::with_capacity(plan.len());
        let (sort, desc) = self.bucket_sort_state();
        for (key, default, numeric) in plan {
            let width = self.col_width(TableKind::Buckets, key.label(), default);
            let mut column = Column::new(key.label(), key.label())
                .width(width)
                .sortable()
                .sort(sort_state(key == sort, desc));
            if numeric {
                column = column.numeric();
            }
            columns.push(column);
            keys.push(key);
        }
        (columns, keys)
    }

    /// Failure columns plus their sort keys. The message takes the remainder.
    fn failure_columns(&self) -> (Vec<Column>, Vec<FailureSort>) {
        let fixed = 108. + 96. + 150. + 200. + 60.;
        let message_w = (self.table_width() - fixed).clamp(180., 560.);
        let plan: [(FailureSort, f32); 5] = [
            (FailureSort::When, 108.),
            (FailureSort::Kind, 96.),
            (FailureSort::Model, 150.),
            (FailureSort::Session, 200.),
            (FailureSort::Model, message_w),
        ];
        let mut columns = Vec::with_capacity(plan.len());
        let mut keys = Vec::with_capacity(plan.len());
        let (sort, desc) = self.failure_sort_state();
        for (ix, (key, default)) in plan.into_iter().enumerate() {
            let is_message = ix == 4;
            let id: &'static str = if is_message { "message" } else { key.label() };
            let width = self.col_width(TableKind::Failures, id, default);
            let mut column = Column::new(id, if is_message { "Message" } else { key.label() })
                .width(width)
                .resizable();
            if !is_message {
                column = column.sortable().sort(sort_state(key == sort, desc));
            }
            columns.push(column);
            keys.push(key);
        }
        (columns, keys)
    }

    // ── row plans ──────────────────────────────────────────────────────────

    /// One session per row, cell content in the register its column deserves
    /// (§54). The selected row is the page's scoped session.
    fn session_row_elements(
        &self,
        rows: &[SessionRow],
        columns: &[Column],
        keys: &[Option<SessionSort>],
        theme: Theme,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let selected = self.filter().session;
        let entity = cx.entity();
        let context_row = self.context_row();
        let mut out = Vec::with_capacity(rows.len());
        for (ix, row) in rows.iter().enumerate() {
            let mut line = div()
                .id(("usage-session-row", ix))
                .group("usage-row")
                .h(px(ROW_H))
                .w_full()
                .min_w(px(columns.iter().map(|column| column.width).sum::<f32>()))
                .flex()
                .items_center()
                .border_b_1()
                .border_color(theme.border)
                .hover(|style| style.bg(theme.bg_hover))
                .when(selected == Some(row.session), |line| line.bg(theme.active))
                .on_mouse_down(MouseButton::Left, {
                    let entity = entity.clone();
                    let session = row.session;
                    move |event, window, cx| {
                        if event.click_count >= 2 {
                            entity.update(cx, |page, cx| page.open_session(window, cx, session));
                        } else {
                            entity.update(cx, |page, cx| page.set_session_scope(Some(session), cx));
                        }
                    }
                })
                .on_mouse_down(MouseButton::Right, {
                    let entity = entity.clone();
                    move |_, _, cx| {
                        entity.update(cx, |page, cx| page.open_context_menu(ix, cx));
                    }
                });
            for (col_ix, column) in columns.iter().enumerate() {
                let cell = match keys.get(col_ix).copied().flatten() {
                    Some(key) => session_cell(row, key, theme, column),
                    None => open_session_cell(row.session, entity.clone(), theme, column),
                };
                line = line.child(cell);
            }
            if context_row == Some(ix) {
                line = line.child(session_context_menu(row, theme, entity.clone()));
            }
            out.push(line.into_any_element());
        }
        out
    }

    /// One bucket per row. The date column carries the granularity as a quieter
    /// suffix, so an all-time monthly table still says what each row is.
    fn bucket_row_elements(
        &self,
        columns: &[Column],
        keys: &[BucketSort],
        granularity: Granularity,
        theme: Theme,
    ) -> Vec<AnyElement> {
        let rows = self.bucket_rows();
        let total_w: f32 = columns.iter().map(|column| column.width).sum();
        let mut out = Vec::with_capacity(rows.len());
        for row in &rows {
            let mut line = div()
                .h(px(ROW_H))
                .w_full()
                .min_w(px(total_w))
                .flex()
                .items_center()
                .border_b_1()
                .border_color(theme.border)
                .hover(|style| style.bg(theme.bg_hover));
            for (ix, column) in columns.iter().enumerate() {
                let key = keys.get(ix).copied().unwrap_or(BucketSort::Date);
                if key == BucketSort::Date {
                    line = line.child(
                        cell_shell(column)
                            .gap(px(8.))
                            .child(
                                div()
                                    .text_size(theme.ui_px(12.))
                                    .text_color(theme.text)
                                    .child(row.label.clone()),
                            )
                            .child(
                                div()
                                    .text_size(theme.ui_px(10.5))
                                    .text_color(theme.text_3)
                                    .child(granularity.label()),
                            ),
                    );
                } else {
                    let (text, color) = match key {
                        BucketSort::Requests => (format::count(row.totals.requests), theme.text_2),
                        BucketSort::Tokens => {
                            (format::compact(row.totals.tokens.total), theme.text)
                        }
                        BucketSort::Cache => (
                            match row.totals.cache_hit_rate() {
                                Some(rate) => format::percent(rate),
                                None => "—".to_string(),
                            },
                            theme.text_3,
                        ),
                        BucketSort::Errors => (
                            format::count(row.totals.errors),
                            if row.totals.errors > 0 {
                                theme.crit
                            } else {
                                theme.text_3
                            },
                        ),
                        BucketSort::Date => unreachable!("handled above"),
                    };
                    line = line.child(text_cell(column, text, color, theme));
                }
            }
            out.push(line.into_any_element());
        }
        out
    }

    /// One failure per row, newest first.
    fn failure_table_element(&self, theme: Theme, cx: &mut Context<Self>) -> AnyElement {
        let (columns, keys) = self.failure_columns();
        let rows = self.failure_rows();
        let total_w: f32 = columns.iter().map(|column| column.width).sum();
        let mut elements = Vec::with_capacity(rows.len());
        for row in &rows {
            let mut line = div()
                .h(px(ROW_H))
                .w_full()
                .min_w(px(total_w))
                .flex()
                .items_center()
                .border_b_1()
                .border_color(theme.border)
                .hover(|style| style.bg(theme.bg_hover));
            for (ix, column) in columns.iter().enumerate() {
                let is_message = column.id == "message";
                let (text, color) = if is_message {
                    (row.message.clone(), theme.text_3)
                } else {
                    match keys.get(ix).copied().unwrap_or(FailureSort::When) {
                        FailureSort::When => (super::table::failure_when(row.ts_ms), theme.text_3),
                        FailureSort::Kind => {
                            let (label, color) =
                                super::table::failure_kind_register(row.kind, theme);
                            (label.to_string(), color)
                        }
                        FailureSort::Model => (row.model.clone(), theme.text_2),
                        FailureSort::Session => (row.session_title.clone(), theme.text_2),
                    }
                };
                line = line.child(text_cell(column, text, color, theme));
            }
            elements.push(line.into_any_element());
        }
        self.table_element(
            TableKind::Failures,
            "usage-failures-table",
            columns,
            elements,
            FAILURE_TABLE_H,
            empty_cell("No failed requests in this range.", theme),
            theme,
            Rc::new(move |page, ix, sort, cx| {
                let key = keys.get(ix).copied().unwrap_or(FailureSort::When);
                match sort {
                    SortState::Ascending => page.set_failure_sort(key, false, cx),
                    SortState::Descending => page.set_failure_sort(key, true, cx),
                    SortState::Default => page.set_failure_sort(FailureSort::When, true, cx),
                }
            }),
            cx,
        )
    }

    /// A table with its handlers wired to this page. The column ids and widths
    /// are captured so a divider drag knows what it grabbed; `on_sort` maps the
    /// clicked column to a key.
    #[allow(clippy::too_many_arguments)]
    fn table_element(
        &self,
        table: TableKind,
        id: &'static str,
        columns: Vec<Column>,
        rows: Vec<AnyElement>,
        height: f32,
        empty: AnyElement,
        theme: Theme,
        on_sort: Rc<SetSort>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let ids = Rc::new(columns.iter().map(|column| column.id).collect::<Vec<_>>());
        let widths = Rc::new(
            columns
                .iter()
                .map(|column| column.width)
                .collect::<Vec<_>>(),
        );
        let handlers = table_handlers(cx.entity(), table, ids, widths, on_sort);
        data_table(id, &columns, rows, px(height), empty, theme, handlers)
    }

    /// The column-visibility control for the sessions table (§41).
    fn columns_control(&self, theme: Theme, cx: &mut Context<Self>) -> AnyElement {
        let open = self.menu() == Some(MenuKind::Columns);
        let entity = cx.entity();
        let chip = filters::chip(
            "usage-columns-chip",
            "Columns".to_string(),
            Some("icons/panel-right.svg"),
            !self.hidden_columns().is_empty(),
            true,
            theme,
            move |_, window, cx| {
                entity.update(cx, |page, cx| {
                    page.toggle_menu(MenuKind::Columns, window, cx)
                });
            },
        );
        if !open {
            return filters::chip_with_menu(
                "usage-columns-anchor",
                chip,
                false,
                Corner::TopRight,
                || div().into_any_element(),
            );
        }
        let panel = filters::columns_menu(self, cx, theme);
        filters::chip_with_menu(
            "usage-columns-anchor",
            chip,
            true,
            Corner::TopRight,
            move || panel,
        )
    }

    /// The footer summary (§53): totals over the whole filtered set, clearly
    /// distinguished from the totals on the visible page.
    fn totals_line(&self, result: &SessionQueryResult, theme: Theme) -> Option<AnyElement> {
        let snapshot = self.snapshot()?;
        let totals = &snapshot.summary.totals;
        let page_requests: u64 = result.rows.iter().map(|row| row.totals.requests).sum();
        let page_tokens: u64 = result.rows.iter().map(|row| row.totals.tokens.total).sum();

        let mut filtered = format!(
            "Filtered totals: {} requests · {} tokens",
            format::count(totals.requests),
            format::compact(totals.tokens.total)
        );
        if totals.cost_coverage() > 0.0 {
            filtered.push_str(&format!(" · {}", format::cost(totals.cost_usd)));
        }
        let page = format!(
            "this page: {} requests · {} tokens",
            format::count(page_requests),
            format::compact(page_tokens)
        );

        Some(
            div()
                .w_full()
                .px(px(14.))
                .pt(px(10.))
                .flex()
                .items_center()
                .gap(px(12.))
                .text_size(theme.ui_px(10.5))
                .child(div().text_color(theme.text_3).child(filtered))
                .child(div().flex_1())
                .child(div().text_color(theme.text_3).child(page))
                .into_any_element(),
        )
    }

    /// The sessions footer (§27/§75): range, rows-per-page, and page stepping.
    fn pagination_footer(
        &self,
        result: &SessionQueryResult,
        theme: Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let pages = result.page_count();
        let current = result.page;
        let summary = if result.total == 0 {
            "No sessions match".to_string()
        } else {
            format!(
                "Showing {}–{} of {}",
                format::count(result.first_row() as u64),
                format::count(result.last_row() as u64),
                format::count(result.total as u64)
            )
        };

        let size_open = self.menu() == Some(MenuKind::PageSize);
        let entity = cx.entity();
        let size_chip = filters::chip(
            "usage-page-size-chip",
            result.page_size.to_string(),
            None,
            false,
            true,
            theme,
            move |_, window, cx| {
                entity.update(cx, |page, cx| {
                    page.toggle_menu(MenuKind::PageSize, window, cx)
                });
            },
        );
        let size_panel = size_open.then(|| filters::page_size_menu(self, cx, theme));
        let size_control = filters::chip_with_menu(
            "usage-page-size-anchor",
            size_chip,
            size_open,
            Corner::TopRight,
            move || size_panel.unwrap_or_else(|| div().into_any_element()),
        );

        let prev = filters::text_button(
            "usage-page-prev",
            "Previous",
            Some("icons/chevron-left.svg"),
            current > 1,
            theme,
            {
                let entity = cx.entity();
                move |_, _, cx| {
                    entity.update(cx, |page, cx| page.set_page(current.saturating_sub(1), cx));
                }
            },
        );
        let next = filters::text_button(
            "usage-page-next",
            "Next",
            Some("icons/chevron-right.svg"),
            current < pages,
            theme,
            {
                let entity = cx.entity();
                move |_, _, cx| {
                    entity.update(cx, |page, cx| page.set_page(current + 1, cx));
                }
            },
        );

        div()
            .w_full()
            .px(px(14.))
            .pt(px(10.))
            .pb(px(14.))
            .flex()
            .items_center()
            .gap(px(10.))
            .text_size(theme.ui_px(11.5))
            .child(div().text_color(theme.text_3).child(summary))
            .child(div().flex_1())
            .child(div().text_color(theme.text_3).child("Rows per page"))
            .child(size_control)
            .child(prev)
            .child(
                div()
                    .px(px(4.))
                    .font(num_font())
                    .text_color(theme.text_2)
                    .child(format!("{current} / {pages}")),
            )
            .child(next)
            .into_any_element()
    }
}

/// The sort state for a column: only the active key shows a direction.
fn sort_state(active: bool, desc: bool) -> SortState {
    if !active {
        SortState::Default
    } else if desc {
        SortState::Descending
    } else {
        SortState::Ascending
    }
}

/// A header click routed to the page: which column and the state to move to.
type SetSort = dyn Fn(&mut UsagePage, usize, SortState, &mut Context<UsagePage>);

/// Wire a table's handlers to the page: header clicks sort, divider drags
/// resize the column they grabbed.
fn table_handlers(
    entity: Entity<UsagePage>,
    table: TableKind,
    ids: Rc<Vec<&'static str>>,
    widths: Rc<Vec<f32>>,
    on_sort: Rc<SetSort>,
) -> TableHandlers {
    let sort_entity = entity.clone();
    let start_entity = entity.clone();
    let move_entity = entity.clone();
    let end_entity = entity;
    TableHandlers::new(
        move |ix, sort, _window, cx| {
            sort_entity.update(cx, |page, cx| on_sort(page, ix, sort, cx));
        },
        move |ix, x, _window, cx| {
            if let (Some(id), Some(width)) = (ids.get(ix).copied(), widths.get(ix).copied()) {
                start_entity.update(cx, |page, _| page.begin_resize(table, id, x, width));
            }
        },
        move |_ix, x, _window, cx| {
            move_entity.update(cx, |page, cx| page.drag_resize(x, cx));
        },
        move |_window, cx| {
            end_entity.update(cx, |page, _| page.end_resize());
        },
    )
}

/// One session cell, in the register its column deserves (§54).
fn session_cell(row: &SessionRow, key: SessionSort, theme: Theme, column: &Column) -> AnyElement {
    let totals = row.totals;
    let (text, color) = match key {
        SessionSort::Title => (row.title.clone(), theme.text),
        SessionSort::Workspace => (row.workspace.clone(), theme.text_2),
        SessionSort::Provider => (row.provider.clone(), theme.text_2),
        SessionSort::Model => (row.top_model.clone(), theme.text_2),
        SessionSort::Started => (
            super::model::bucket_label(row.ended_ms, Granularity::Day),
            theme.text_3,
        ),
        SessionSort::Duration => (format::span_ms(row.duration_ms()), theme.text_3),
        SessionSort::Requests => (format::count(totals.requests), theme.text_2),
        SessionSort::Input => (format::compact(totals.tokens.input), theme.text_3),
        SessionSort::Output => (format::compact(totals.tokens.output), theme.text_3),
        SessionSort::Cache => (
            // "0" would claim the provider reported no cache reuse; "—" says
            // nothing was reported at all.
            if totals.tokens.cache_read == 0 && !row.cache_capable {
                "—".to_string()
            } else {
                format::compact(totals.tokens.cache_read)
            },
            theme.text_3,
        ),
        SessionSort::Tokens => (format::compact(totals.tokens.total), theme.text),
        SessionSort::Errors => {
            let errors = totals.errors + row.tool_errors;
            (
                format::count(errors),
                if errors > 0 { theme.crit } else { theme.text_3 },
            )
        }
        // Tool calls are shown in the tool activity panel, which ranks them
        // properly; the table's column plan does not include one.
        SessionSort::Tools => (format::count(row.tool_runs), theme.text_3),
    };
    text_cell(column, text, color, theme)
}

/// The trailing affordance: open this session in the chat surface. Hidden
/// until its row is hovered, so thirteen rows do not ship thirteen arrows.
fn open_session_cell(
    session: u16,
    page: Entity<UsagePage>,
    theme: Theme,
    column: &Column,
) -> AnyElement {
    div()
        .w(px(column.width))
        .h_full()
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .id(SharedString::from(format!("usage-open-{session}")))
                .size(px(22.))
                .rounded(px(5.))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .opacity(0.)
                .group_hover("usage-row", |style| style.opacity(1.))
                .hover(|style| style.bg(theme.overlay_strong))
                .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                    // Do not let the row's own click handler also fire: this
                    // is a jump to another surface, not a scope change.
                    cx.stop_propagation();
                    page.update(cx, |page, cx| page.open_session(window, cx, session));
                })
                .child(icon("icons/arrow-up-right.svg", 12., theme.text_3)),
        )
        .into_any_element()
}

/// The session row's right-click menu (§43): only actions that are wired.
fn session_context_menu(row: &SessionRow, theme: Theme, page: Entity<UsagePage>) -> AnyElement {
    let session = row.session;
    let session_id = row.id.clone();
    let provider_id = row.provider_id;
    let model_id = row.top_model_id;
    let provider_label = row.provider.clone();
    let model_label = row.top_model.clone();

    let mut items: Vec<AnyElement> = Vec::new();
    items.push(context_item("Open session".into(), theme, {
        let page = page.clone();
        move |window, cx| {
            page.update(cx, |page, cx| page.open_session(window, cx, session));
        }
    }));
    items.push(context_item("Scope to this session".into(), theme, {
        let page = page.clone();
        move |_, cx| {
            page.update(cx, |page, cx| page.set_session_scope(Some(session), cx));
        }
    }));
    items.push(context_separator(theme));
    items.push(context_item(
        "Copy session ID".into(),
        theme,
        move |_, cx| {
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(session_id.clone()));
        },
    ));
    items.push(context_separator(theme));
    items.push(context_item(
        format!("Filter by provider: {provider_label}"),
        theme,
        {
            let page = page.clone();
            move |_, cx| {
                page.update(cx, |page, cx| {
                    page.toggle_filter_value(MenuKind::Provider, provider_id, cx)
                });
            }
        },
    ));
    if let Some(model_id) = model_id {
        items.push(context_item(
            format!("Filter by model: {model_label}"),
            theme,
            {
                let page = page.clone();
                move |_, cx| {
                    page.update(cx, |page, cx| {
                        page.toggle_filter_value(MenuKind::Model, model_id, cx)
                    });
                }
            },
        ));
    }
    items.push(context_item("Filter by workspace".into(), theme, {
        let page = page.clone();
        move |_, cx| {
            page.update(cx, |page, cx| {
                let workspace = page
                    .index()
                    .and_then(|index| index.try_session(session).map(|entry| entry.workspace));
                if let Some(workspace) = workspace {
                    page.toggle_filter_value(MenuKind::Workspace, workspace, cx);
                }
            });
        }
    }));

    anchored()
        .position_mode(gpui::AnchoredPositionMode::Local)
        .anchor(Corner::TopLeft)
        .snap_to_window()
        .child(deferred(context_menu_shell(theme, page, items)))
        .into_any_element()
}

/// The menu shell: the app's popover register, dismissed by an outside click.
fn context_menu_shell(theme: Theme, page: Entity<UsagePage>, items: Vec<AnyElement>) -> AnyElement {
    div()
        .id("usage-row-menu")
        .min_w(px(220.))
        .rounded(px(9.))
        .border_1()
        .border_color(theme.border_strong)
        .bg(theme.menu_bg)
        .shadow(theme.popover_shadow())
        .flex()
        .flex_col()
        .overflow_hidden()
        .occlude()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_mouse_down_out(move |_, _, cx| {
            page.update(cx, |page, cx| page.close_context_menu(cx));
        })
        .children(items)
        .into_any_element()
}

/// One row of a context menu.
fn context_item(
    label: String,
    theme: Theme,
    on_click: impl Fn(&mut Window, &mut App) + 'static,
) -> AnyElement {
    div()
        .px(px(10.))
        .py(px(6.))
        .text_size(theme.ui_px(12.))
        .text_color(theme.text)
        .cursor_pointer()
        .hover(|style| style.bg(theme.bg_hover))
        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
            cx.stop_propagation();
            on_click(window, cx);
        })
        .child(label)
        .into_any_element()
}

fn context_separator(theme: Theme) -> AnyElement {
    div().h(px(1.)).w_full().bg(theme.border).into_any_element()
}

// ── shared pieces ─────────────────────────────────────────────────────────

/// A page section as its own card: a header strip (title, meta, controls) over
/// a body, on a raised surface with a single hairline border. `clip` rounds the
/// corners for cards whose content runs edge to edge; it stays off for a card
/// that hosts a popover, which must escape the card to be usable.
fn card(
    id: &'static str,
    title: &str,
    meta: Option<String>,
    right: Option<AnyElement>,
    content: AnyElement,
    theme: Theme,
    clip: bool,
) -> AnyElement {
    let mut root = div()
        .id(SharedString::from(id))
        .flex_none()
        .w_full()
        .rounded(px(12.))
        .border_1()
        .border_color(theme.border)
        .bg(theme.bg_raised)
        .flex()
        .flex_col()
        .child(
            div()
                .flex_none()
                .h(px(42.))
                .px(px(14.))
                .flex()
                .items_center()
                .gap(px(10.))
                .border_b_1()
                .border_color(theme.border)
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
        .child(content);
    if clip {
        root = root.overflow_hidden();
    }
    root.into_any_element()
}

/// A segmented control's selection callback, boxed so one segment's handler
/// can be cloned into every option.
type SegmentPick<T> = Rc<dyn Fn(T, &mut Window, &mut App)>;

/// A compact segmented control: one active segment, the rest quiet. Reuses the
/// metric switcher's register so every tab row on the page reads the same.
fn segmented<T>(
    prefix: &'static str,
    options: &[T],
    active: T,
    label: impl Fn(T) -> &'static str,
    key: impl Fn(T) -> &'static str,
    theme: Theme,
    on_pick: impl Fn(T, &mut Window, &mut App) + 'static,
) -> AnyElement
where
    T: Copy + PartialEq + 'static,
{
    let on_pick: SegmentPick<T> = Rc::new(on_pick);
    let mut row = div()
        .flex()
        .items_center()
        .gap(px(2.))
        .p(px(2.))
        .rounded(px(8.))
        .border_1()
        .border_color(theme.border)
        .bg(theme.bg_main);
    for option in options.iter().copied() {
        let is_active = option == active;
        let handler = on_pick.clone();
        row = row.child(
            div()
                .id(SharedString::from(format!("{prefix}-{}", key(option))))
                .h(px(24.))
                .px(px(9.))
                .rounded(px(6.))
                .flex()
                .items_center()
                .text_size(theme.ui_px(11.5))
                .when(is_active, |tab| {
                    tab.bg(theme.bg_raised)
                        .text_color(theme.text)
                        .font_weight(FontWeight::MEDIUM)
                })
                .when(!is_active, |tab| {
                    tab.text_color(theme.text_3)
                        .cursor_pointer()
                        .hover(|style| style.bg(theme.bg_hover).text_color(theme.text_2))
                        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                            handler(option, window, cx)
                        })
                })
                .child(label(option)),
        );
    }
    row.into_any_element()
}

/// A search field, shared by the Sessions table and the Breakdown table so both
/// read the same.
fn search_box(input: &Entity<ComposerInput>, theme: Theme) -> AnyElement {
    div()
        .w(px(232.))
        .h(px(28.))
        .px(px(8.))
        .rounded(px(7.))
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
                .child(input.clone()),
        )
        .into_any_element()
}

/// A chart row's label column (name plus its secondary line).
fn chart_label(label: &str, sub: Option<&str>, theme: Theme) -> AnyElement {
    div()
        .w(px(180.))
        .flex_none()
        .flex()
        .flex_col()
        .child(
            div()
                .truncate()
                .text_size(theme.ui_px(12.))
                .text_color(theme.text)
                .child(label.to_string()),
        )
        .children(sub.map(|sub| {
            div()
                .truncate()
                .text_size(theme.ui_px(10.5))
                .text_color(theme.text_3)
                .child(sub.to_string())
        }))
        .into_any_element()
}

/// A chart row's magnitude track and its accent fill.
fn bar_track(fraction: f64, theme: Theme) -> AnyElement {
    div()
        .flex_1()
        .min_w(px(48.))
        .h(px(8.))
        .rounded(px(4.))
        .bg(theme.trough)
        .overflow_hidden()
        .child(
            div()
                .h_full()
                .w(relative(fraction.clamp(0.0, 1.0) as f32))
                .rounded(px(4.))
                .bg(theme.accent),
        )
        .into_any_element()
}

/// A chart row's value column.
fn chart_value(value: &str, theme: Theme) -> AnyElement {
    div()
        .w(px(72.))
        .flex_none()
        .font(num_font())
        .text_size(theme.ui_px(11.5))
        .text_color(theme.text)
        .text_align(gpui::TextAlign::Right)
        .child(value.to_string())
        .into_any_element()
}

/// A chart row's share column.
fn chart_share(share: &str, theme: Theme) -> AnyElement {
    div()
        .w(px(56.))
        .flex_none()
        .whitespace_nowrap()
        .font(num_font())
        .text_size(theme.ui_px(11.5))
        .text_color(theme.text_3)
        .text_align(gpui::TextAlign::Right)
        .child(share.to_string())
        .into_any_element()
}

/// The totals line under a data table: filtered totals left, this page right.
fn totals_row(filtered: &str, page: &str, theme: Theme) -> AnyElement {
    div()
        .w_full()
        .px(px(14.))
        .pt(px(10.))
        .flex()
        .items_center()
        .gap(px(12.))
        .text_size(theme.ui_px(10.5))
        .child(div().text_color(theme.text_3).child(filtered.to_string()))
        .child(div().flex_1())
        .child(div().text_color(theme.text_3).child(page.to_string()))
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
        .px(px(14.))
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
        .child(
            div()
                .flex_1()
                .text_color(theme.text_3)
                .child(label.to_string()),
        )
        .child(
            div()
                .font(num_font())
                .text_color(theme.text_2)
                .child(value.to_string()),
        )
        .into_any_element()
}

/// The cache hit rate across the window: one bar per bucket, drawn bottom-up,
/// each a real measurement with its own hover readout (§24). A bucket that
/// reported no cache traffic draws as a faint stub rather than a 0% bar —
/// "no data" and "a 0% hit rate" are different facts.
fn hit_rate_bars(series: &TimeSeries, theme: Theme) -> AnyElement {
    div()
        .w_full()
        .h(px(40.))
        .flex()
        .items_end()
        .gap(px(2.))
        .children(series.points.iter().enumerate().map(|(ix, point)| {
            let rate = Totals {
                tokens: TokenCounts {
                    input: point.totals.tokens.input,
                    cache_read: point.totals.tokens.cache_read,
                    ..Default::default()
                },
                ..Totals::default()
            }
            .cache_hit_rate();
            let (height, color, tooltip) = match rate {
                Some(rate) => (
                    (rate / 100.0).clamp(0.03, 1.0) as f32,
                    theme.accent.opacity(0.55),
                    format!("{} · {} hit rate", point.stamp, format::percent(rate)),
                ),
                None => (
                    0.03,
                    theme.trough,
                    format!("{} · no cache traffic", point.stamp),
                ),
            };
            div()
                .id(SharedString::from(format!("usage-cache-bar-{ix}")))
                .group("usage-cache-bar")
                .tooltip(move |_, cx| cx.new(|_| Tooltip::new(tooltip.clone())).into())
                .flex_1()
                .min_w(px(2.))
                .h_full()
                .flex()
                .items_end()
                .child(
                    div()
                        .w_full()
                        .rounded_t(px(2.))
                        .bg(color)
                        .h(relative(height))
                        .group_hover("usage-cache-bar", |style| style.bg(theme.accent)),
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

/// The name cell of a breakdown table: the row's label, with its secondary
/// line (provider, parent folder) trailing in the quiet register.
fn breakdown_name_cell(column: &Column, row: &GroupRow, theme: Theme) -> AnyElement {
    cell_shell(column)
        .gap(px(8.))
        .child(
            div()
                .flex_none()
                .max_w(px(column.width - 24.))
                .truncate()
                .text_size(theme.ui_px(12.))
                .text_color(theme.text)
                .child(row.label.clone()),
        )
        .children(row.sub.clone().map(|sub| {
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(theme.ui_px(10.5))
                .text_color(theme.text_3)
                .child(sub)
        }))
        .into_any_element()
}

/// The Summary card's meta line: the range's headline counts.
fn summary_meta(snapshot: &UsageSnapshot) -> String {
    format!(
        "{} requests · {} tokens · {} sessions",
        format::count(snapshot.summary.totals.requests),
        format::compact(snapshot.summary.totals.tokens.total),
        format::count(snapshot.summary.sessions)
    )
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
        format!("Session {}", entry.id.chars().take(8).collect::<String>())
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

fn tools_meta(snapshot: &UsageSnapshot) -> String {
    let tools = &snapshot.tools;
    match snapshot.summary.tool_error_rate() {
        Some(rate) if tools.errors > 0 => format!(
            "{} calls · {} failed ({})",
            format::count(tools.calls),
            format::count(tools.errors),
            format::percent(rate)
        ),
        Some(_) => format!("{} calls · none failed", format::count(tools.calls)),
        None => "no tool calls".to_string(),
    }
}

/// One label/value pair in the secondary readout under the board.
fn summary_stat(label: String, value: String, theme: Theme) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap(px(6.))
        .text_size(theme.ui_px(11.5))
        .child(div().text_color(theme.text_3).child(label))
        .child(div().font(num_font()).text_color(theme.text_2).child(value))
        .into_any_element()
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
        format!(
            "{} not shown: {}",
            unavailable.join(", "),
            reasons.join("; ")
        )
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
    /// Focus the failures: show only failed requests.
    ErrorsOnly,
    /// Point the main chart at the metric this card measures.
    Metric(ChartMetric),
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
