//! The filter bar: the one place the page's scope is set.
//!
//! Four chips (date / workspace / provider / model) open anchored popovers —
//! the same `anchored` + `deferred` pattern as the model picker, so they float
//! above the page and stay inside the window. Multi-selects get a search field
//! and select-all/clear; the range chip carries a compact month calendar for
//! custom windows.
//!
//! Every control writes through `UsagePage`'s filter setters, so one state
//! object drives every panel on the page (§10).

use chrono::Datelike;
use gpui::{
    anchored, deferred, div, point, prelude::*, px, AnyElement, App, Corner, ElementId, Entity,
    FontWeight, IntoElement, MouseButton, MouseDownEvent, SharedString, Window,
};

use super::format;
use super::model::{
    local_day_start, local_month_start, next_bucket, Granularity, RangePreset, UsageIndex,
};
use super::page::{ExportFormat, MenuKind, SessionSort, UsagePage, PAGE_SIZES};
use crate::theme::Theme;
use crate::{app::icon, composer::ComposerInput};

/// One selectable value in a multi-select menu.
pub struct FilterOption {
    pub id: u16,
    pub label: String,
    pub sub: Option<String>,
}

/// A filter chip plus its popover. The panel hangs below (or below-right of)
/// the chip, floating above the page like every other menu in the app.
pub fn chip_with_menu(
    id: &'static str,
    chip: impl IntoElement,
    open: bool,
    corner: Corner,
    panel: impl FnOnce() -> AnyElement,
) -> AnyElement {
    div()
        .id(ElementId::Name(SharedString::from(id)))
        .relative()
        .flex()
        .flex_col()
        .items_start()
        .child(chip)
        .children(open.then(|| {
            anchored()
                .position_mode(gpui::AnchoredPositionMode::Local)
                .anchor(corner)
                .offset(point(px(-1.), px(5.)))
                .snap_to_window()
                .child(deferred(panel()))
        }))
        .into_any_element()
}

/// A filter chip: 28px, hairline, and unmistakably "on" when it narrows the
/// view (active fill, `active_fg` text — never accent alone).
pub fn chip(
    id: &'static str,
    label: String,
    icon_path: &'static str,
    active: bool,
    theme: Theme,
    on_click: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let foreground = if active {
        theme.active_fg
    } else {
        theme.text_2
    };
    div()
        .id(ElementId::Name(SharedString::from(id)))
        .h(px(28.))
        .px(px(9.))
        .rounded(px(7.))
        .border_1()
        .border_color(if active {
            theme.border_strong
        } else {
            theme.border
        })
        .bg(if active {
            theme.active
        } else {
            theme.bg_raised
        })
        .flex()
        .items_center()
        .gap(px(6.))
        .cursor_pointer()
        .hover(|style| style.bg(if active { theme.active } else { theme.bg_hover }))
        .on_mouse_down(MouseButton::Left, on_click)
        .child(icon(
            icon_path,
            12.,
            if active {
                theme.active_fg
            } else {
                theme.text_3
            },
        ))
        .child(
            div()
                .max_w(px(180.))
                .truncate()
                .text_size(theme.ui_px(12.))
                .text_color(foreground)
                .child(label),
        )
        .child(icon(
            "icons/chevron-down.svg",
            10.,
            if active {
                theme.active_fg
            } else {
                theme.text_3
            },
        ))
}

/// The popover shell shared by every filter menu. A click outside closes it,
/// and so does Escape — while a filter menu is open it owns that key, so it can
/// never reach the global abort binding.
fn panel(
    id: &'static str,
    width: f32,
    theme: Theme,
    children: Vec<AnyElement>,
    page: Entity<UsagePage>,
) -> AnyElement {
    let dismiss_click = page.clone();
    div()
        .id(ElementId::Name(SharedString::from(id)))
        .w(px(width))
        .rounded(px(9.))
        .border_1()
        .border_color(theme.border_strong)
        .bg(theme.menu_bg)
        .shadow(theme.popover_shadow())
        .flex()
        .flex_col()
        .overflow_hidden()
        .occlude()
        .key_context("Picker")
        .on_mouse_down_out(move |_, _, cx| {
            dismiss_click.update(cx, |page, cx| page.dismiss_menus(cx));
        })
        .on_action({
            let cancel_page = page.clone();
            move |_: &crate::PickerCancel, _, cx| {
                cancel_page.update(cx, |page, cx| page.dismiss_menus(cx));
            }
        })
        // Enter belongs to the menu while it is open: without this it would
        // reach the app's Submit and send the composer's pending text.
        .on_action(move |_: &crate::PickerConfirm, _, cx| {
            page.update(cx, |page, cx| page.dismiss_menus(cx));
        })
        .children(children)
        .into_any_element()
}

/// The search field row at the top of a multi-select menu.
fn search_row(query: &Entity<ComposerInput>, theme: Theme) -> AnyElement {
    div()
        .p(px(6.))
        .pb(px(4.))
        .border_b_1()
        .border_color(theme.border)
        .child(
            div()
                .h(px(26.))
                .px(px(8.))
                .rounded(px(6.))
                .bg(theme.bg_main)
                .border_1()
                .border_color(theme.border)
                .flex()
                .items_center()
                .gap(px(6.))
                .text_size(theme.ui_px(12.))
                .child(icon("icons/search.svg", 12., theme.text_3))
                .child(div().flex_1().min_w_0().child(query.clone())),
        )
        .into_any_element()
}

/// One row in a menu.
#[allow(clippy::too_many_arguments)]
fn row(
    id: ElementId,
    label: &str,
    sub: Option<&str>,
    selected: bool,
    highlighted: bool,
    theme: Theme,
    on_click: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    on_hover: impl Fn(&bool, &mut Window, &mut App) + 'static,
) -> AnyElement {
    div()
        .id(id)
        .h(px(28.))
        .mx(px(4.))
        .px(px(8.))
        .rounded(px(6.))
        .flex()
        .items_center()
        .gap(px(8.))
        .cursor_pointer()
        .when(highlighted, |row| row.bg(theme.active))
        .hover(|style| style.bg(theme.overlay))
        .on_hover(on_hover)
        .on_mouse_down(MouseButton::Left, on_click)
        .child(
            // A check mark column keeps labels aligned whether or not a row
            // is selected; selection is also carried by the row's text weight.
            div()
                .w(px(12.))
                .flex_none()
                .flex()
                .items_center()
                .child(if selected {
                    icon("icons/check.svg", 11., theme.accent).into_any_element()
                } else {
                    div().into_any_element()
                }),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(theme.ui_px(12.))
                .when(selected, |text| text.font_weight(FontWeight::MEDIUM))
                .text_color(if highlighted {
                    theme.active_fg
                } else {
                    theme.text_2
                })
                .child(label.to_string()),
        )
        .children(sub.map(|sub| {
            div()
                .flex_none()
                .max_w(px(120.))
                .truncate()
                .text_size(theme.ui_px(10.5))
                .text_color(theme.text_3)
                .child(sub.to_string())
        }))
        .into_any_element()
}

fn footer_row(
    id: &'static str,
    label: &str,
    theme: Theme,
    on_click: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
) -> AnyElement {
    div()
        .id(id)
        .h(px(26.))
        .flex_1()
        .rounded(px(6.))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .text_size(theme.ui_px(11.5))
        .text_color(theme.text_3)
        .hover(|style| style.bg(theme.bg_hover).text_color(theme.text_2))
        .on_mouse_down(MouseButton::Left, on_click)
        .child(label.to_string())
        .into_any_element()
}

fn menu_footer(theme: Theme, left: AnyElement, right: AnyElement) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap(px(4.))
        .px(px(4.))
        .py(px(4.))
        .border_t_1()
        .border_color(theme.border)
        .child(left)
        .child(right)
        .into_any_element()
}

/// The search-filtered view of a menu's options.
fn visible_options<'a>(
    options: &'a [FilterOption],
    query: &str,
    selected: &[u16],
) -> Vec<&'a FilterOption> {
    let needle = query.trim().to_lowercase();
    let mut rows: Vec<&FilterOption> = options
        .iter()
        .filter(|option| {
            needle.is_empty()
                || option.label.to_lowercase().contains(&needle)
                || option
                    .sub
                    .as_deref()
                    .is_some_and(|sub| sub.to_lowercase().contains(&needle))
        })
        .collect();
    // Selected values first, then the index's own order (already ranked by
    // the caller).
    rows.sort_by_key(|option| !selected.contains(&option.id));
    rows
}

/// A multi-select menu: search, the ranked list, and select-all/clear.
pub fn multi_menu(
    page: &UsagePage,
    kind: MenuKind,
    options: &[FilterOption],
    all: &[FilterOption],
    selected: &[u16],
    cx: &mut gpui::Context<UsagePage>,
    theme: Theme,
) -> AnyElement {
    let query = page.menu_query().read(cx).text();
    let rows = visible_options(options, &query, selected);
    let highlight = page.menu_highlight();

    let mut children: Vec<AnyElement> = vec![search_row(page.menu_query(), theme)];
    let list: Vec<AnyElement> =
        std::iter::once((None, "All".to_string(), None::<String>, selected.is_empty()))
            .chain(rows.iter().map(|option| {
                (
                    Some(option.id),
                    option.label.clone(),
                    option.sub.clone(),
                    selected.contains(&option.id),
                )
            }))
            .enumerate()
            .map(|(ix, (id, label, sub, is_selected))| {
                let entity = cx.entity();
                let target = id;
                row(
                    ElementId::NamedInteger("usage-menu-row".into(), ix as u64),
                    &label,
                    sub.as_deref(),
                    is_selected,
                    ix == highlight,
                    theme,
                    move |_, _, cx| {
                        entity.update(cx, |page, cx| match target {
                            Some(id) => page.toggle_filter_value(kind, id, cx),
                            None => page.clear_dimension(kind, cx),
                        });
                    },
                    {
                        let entity = cx.entity();
                        move |entered, _, cx| {
                            if *entered {
                                entity.update(cx, |page, cx| page.set_menu_highlight(ix, cx));
                            }
                        }
                    },
                )
            })
            .collect();
    children.push(
        div()
            .id("usage-menu-list")
            .max_h(px(260.))
            .overflow_y_scroll()
            .py(px(4.))
            .flex()
            .flex_col()
            .children(list)
            .into_any_element(),
    );
    if rows.is_empty() && !query.trim().is_empty() {
        children.push(
            div()
                .px(px(12.))
                .pb(px(8.))
                .text_size(theme.ui_px(11.5))
                .text_color(theme.text_3)
                .child(format!("No match for “{}”", query.trim()))
                .into_any_element(),
        );
    }
    let ids: Vec<u16> = all.iter().map(|option| option.id).collect();
    let entity = cx.entity();
    let select_all = footer_row("usage-menu-all", "Select all", theme, {
        let ids = ids.clone();
        move |_, _, cx| {
            entity.update(cx, |page, cx| page.select_all(kind, &ids, cx));
        }
    });
    let entity = cx.entity();
    let clear = footer_row("usage-menu-clear", "Clear", theme, move |_, _, cx| {
        entity.update(cx, |page, cx| page.clear_dimension(kind, cx));
    });
    children.push(menu_footer(theme, select_all, clear));
    let page = cx.entity();
    panel("usage-filter-menu", 268., theme, children, page)
}

/// The export popover: the two things the page can write, and what each
/// contains. Both respect the current filters.
pub fn export_menu(cx: &mut gpui::Context<UsagePage>, theme: Theme) -> AnyElement {
    let mut children: Vec<AnyElement> = Vec::new();
    for (format, label, hint) in [
        (
            ExportFormat::Csv,
            "Filtered requests (CSV)",
            "one row per model request",
        ),
        (
            ExportFormat::Json,
            "Current view (JSON)",
            "summary, series, breakdowns, tables",
        ),
    ] {
        let entity = cx.entity();
        children.push(row(
            ElementId::Name(SharedString::from(format!("usage-export-{label}"))),
            label,
            Some(hint),
            false,
            false,
            theme,
            move |_, _, cx| {
                entity.update(cx, |page, cx| {
                    page.export(format, cx);
                    page.close_menu(cx);
                });
            },
            |_, _, _| {},
        ));
    }
    let page = cx.entity();
    panel("usage-export-menu", 268., theme, children, page)
}

/// The rows-per-page picker (§26): one choice, applied immediately.
pub fn page_size_menu(
    page: &UsagePage,
    cx: &mut gpui::Context<UsagePage>,
    theme: Theme,
) -> AnyElement {
    let current = page.page_size();
    let mut children: Vec<AnyElement> = vec![menu_title("Rows per page", theme)];
    for size in PAGE_SIZES {
        let selected = size == current;
        let entity = cx.entity();
        children.push(row(
            ElementId::NamedInteger("usage-page-size".into(), size as u64),
            &size.to_string(),
            None,
            selected,
            false,
            theme,
            move |_, _, cx| {
                entity.update(cx, |page, cx| page.set_page_size(size, cx));
            },
            |_, _, _| {},
        ));
    }
    let page = cx.entity();
    panel("usage-page-size-menu", 180., theme, children, page)
}

/// The column-visibility picker (§41). The title column is fixed; every other
/// column can be turned off, and the choice is persisted.
pub fn columns_menu(
    page: &UsagePage,
    cx: &mut gpui::Context<UsagePage>,
    theme: Theme,
) -> AnyElement {
    let mut children: Vec<AnyElement> = vec![menu_title("Columns", theme)];
    // The display order of the table's own plan, minus the fixed title/open.
    let columns: [SessionSort; 11] = [
        SessionSort::Workspace,
        SessionSort::Provider,
        SessionSort::Model,
        SessionSort::Started,
        SessionSort::Duration,
        SessionSort::Requests,
        SessionSort::Input,
        SessionSort::Output,
        SessionSort::Cache,
        SessionSort::Tokens,
        SessionSort::Errors,
    ];
    for column in columns {
        let selected = page.column_visible(column);
        let entity = cx.entity();
        children.push(row(
            ElementId::Name(SharedString::from(format!(
                "usage-column-{}",
                column.as_str()
            ))),
            column.label(),
            None,
            selected,
            false,
            theme,
            move |_, _, cx| {
                entity.update(cx, |page, cx| page.toggle_column(column, cx));
            },
            |_, _, _| {},
        ));
    }
    let entity = cx.entity();
    let show_all = footer_row("usage-columns-all", "Show all", theme, move |_, _, cx| {
        entity.update(cx, |page, cx| page.show_all_columns(cx));
    });
    children.push(menu_footer(theme, show_all, div().into_any_element()));
    let page = cx.entity();
    panel("usage-columns-menu", 220., theme, children, page)
}

/// A small heading at the top of a menu.
fn menu_title(label: &str, theme: Theme) -> AnyElement {
    div()
        .px(px(12.))
        .pt(px(8.))
        .pb(px(4.))
        .text_size(theme.ui_px(10.5))
        .font_weight(FontWeight::MEDIUM)
        .text_color(theme.text_3)
        .child(label.to_uppercase())
        .into_any_element()
}

/// The date-range menu: presets, then a compact calendar for custom windows.
pub fn range_menu(page: &UsagePage, cx: &mut gpui::Context<UsagePage>, theme: Theme) -> AnyElement {
    let current = page.filter().range.clone();
    let mut children: Vec<AnyElement> = Vec::new();
    let mut rows: Vec<AnyElement> = Vec::new();
    for preset in RangePreset::ALL {
        let selected = current.preset == preset;
        let entity = cx.entity();
        // The custom row names the action, not the range it would replace.
        let label = preset.label().to_string();
        rows.push(row(
            ElementId::Name(SharedString::from(format!(
                "usage-range-{}",
                preset.as_str()
            ))),
            &label,
            preset_summary(preset),
            selected,
            false,
            theme,
            move |_, window, cx| {
                entity.update(cx, |page, cx| {
                    if preset == RangePreset::Custom {
                        page.open_calendar(window, cx);
                    } else {
                        page.set_preset(preset, cx);
                        page.open_menu(None, window, cx);
                    }
                });
            },
            |_, _, _| {},
        ));
    }
    // The calendar appears in place once "Custom…" is chosen.
    if page.calendar_open() {
        rows.push(
            div()
                .h(px(1.))
                .mx(px(8.))
                .my(px(4.))
                .bg(theme.border)
                .into_any_element(),
        );
        rows.push(calendar(page, cx, theme));
    }
    children.push(
        div()
            .py(px(4.))
            .flex()
            .flex_col()
            .children(rows)
            .into_any_element(),
    );
    let page = cx.entity();
    panel("usage-range-menu", 244., theme, children, page)
}

/// The hint shown on the right of each preset row.
fn preset_summary(preset: RangePreset) -> Option<&'static str> {
    match preset {
        RangePreset::Today => Some("compare with yesterday"),
        RangePreset::Last7 => Some("vs previous 7 days"),
        RangePreset::Last14 => Some("vs previous 14 days"),
        RangePreset::Last30 => Some("vs previous 30 days"),
        RangePreset::PreviousMonth => Some("complete month"),
        RangePreset::All => Some("no comparison"),
        _ => None,
    }
}

/// A compact month calendar for the custom range. Two clicks set the window:
/// the first is the start, the second the end (a reversed pair is normalised).
fn calendar(page: &UsagePage, cx: &mut gpui::Context<UsagePage>, theme: Theme) -> AnyElement {
    let month_ms = page
        .calendar_month()
        .unwrap_or_else(|| page.filter().range.start_ms);
    let first = local_month_start(month_ms);
    let (start, end) = page.custom_bounds();
    let today = local_day_start(super::collect::now_ms());

    let Some(dt) = super::model::local_datetime(first) else {
        return div().into_any_element();
    };
    let days_in_month = {
        let (year, month) = (dt.year(), dt.month());
        let next = if month == 12 {
            chrono::NaiveDate::from_ymd_opt(year + 1, 1, 1)
        } else {
            chrono::NaiveDate::from_ymd_opt(year, month + 1, 1)
        };
        match next {
            Some(next) => (next - dt.date_naive()).num_days().max(28),
            None => 30,
        }
    };
    let offset = dt.weekday().num_days_from_monday() as i64;

    // Month header with the two navigation affordances.
    let header = div()
        .h(px(28.))
        .px(px(8.))
        .flex()
        .items_center()
        .child(
            div()
                .id("usage-cal-prev")
                .size(px(22.))
                .rounded(px(6.))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .hover(|style| style.bg(theme.bg_hover))
                .on_mouse_down(MouseButton::Left, {
                    let entity = cx.entity();
                    let prev = local_month_start(first - 86_400_000);
                    move |_, _, cx| {
                        entity.update(cx, |page, cx| page.set_calendar_month(prev, cx));
                    }
                })
                .child(icon("icons/chevron-left.svg", 12., theme.text_3)),
        )
        .child(
            div()
                .flex_1()
                .flex()
                .justify_center()
                .text_size(theme.ui_px(12.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(theme.text)
                .child(dt.format("%B %Y").to_string()),
        )
        .child(
            div()
                .id("usage-cal-next")
                .size(px(22.))
                .rounded(px(6.))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .hover(|style| style.bg(theme.bg_hover))
                .on_mouse_down(MouseButton::Left, {
                    let entity = cx.entity();
                    let next = next_bucket(first, Granularity::Month);
                    move |_, _, cx| {
                        entity.update(cx, |page, cx| page.set_calendar_month(next, cx));
                    }
                })
                .child(icon("icons/chevron-right.svg", 12., theme.text_3)),
        );

    let weekdays =
        div()
            .flex()
            .px(px(8.))
            .pb(px(2.))
            .children(["M", "T", "W", "T", "F", "S", "S"].map(|day| {
                div()
                    .flex_1()
                    .flex()
                    .justify_center()
                    .text_size(theme.ui_px(10.))
                    .text_color(theme.text_3)
                    .child(day)
                    .into_any_element()
            }));

    let mut grid = div().flex().flex_col().px(px(8.)).pb(px(6.));
    let day_ix = 1i64;
    let mut cell_ix = 0i64;
    let total_cells = ((offset + days_in_month + 6) / 7) * 7;
    while cell_ix < total_cells {
        let mut week = div().flex();
        for _ in 0..7 {
            let day_number = day_ix + cell_ix - offset;
            if day_ix + cell_ix <= offset || day_number > days_in_month {
                week = week.child(div().flex_1().h(px(24.)).into_any_element());
            } else {
                let day_ms = local_day_start(first + (day_number - 1) * 86_400_000);
                let selected = Some(day_ms) == start || Some(day_ms) == end;
                let in_range = match (start, end) {
                    (Some(from), Some(to)) => day_ms > from.min(to) && day_ms < from.max(to),
                    _ => false,
                };
                let entity = cx.entity();
                week = week.child(
                    div()
                        .id(ElementId::NamedInteger(
                            "usage-cal-day".into(),
                            day_number as u64,
                        ))
                        .flex_1()
                        .h(px(24.))
                        .rounded(px(5.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(theme.ui_px(11.5))
                        .cursor_pointer()
                        .when(selected, |cell| {
                            cell.bg(theme.active).text_color(theme.active_fg)
                        })
                        .when(in_range, |cell| {
                            cell.bg(theme.overlay).text_color(theme.text)
                        })
                        .when(!selected && !in_range, |cell| {
                            cell.text_color(if day_ms == today {
                                theme.accent
                            } else {
                                theme.text_2
                            })
                            .hover(|style| style.bg(theme.bg_hover))
                        })
                        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                            entity.update(cx, |page, cx| page.pick_day(day_ms, cx));
                        })
                        .child(day_number.to_string())
                        .into_any_element(),
                );
            }
            cell_ix += 1;
        }
        grid = grid.child(week.into_any_element());
    }

    let hint = match (start, end) {
        (Some(from), Some(to)) => super::model::DateRange::custom(from, to).label(),
        (Some(_), None) => "Pick the end date".to_string(),
        _ => "Pick the start date".to_string(),
    };

    div()
        .flex()
        .flex_col()
        .child(header)
        .child(weekdays)
        .child(grid)
        .child(
            div()
                .px(px(12.))
                .pb(px(6.))
                .text_size(theme.ui_px(10.5))
                .text_color(theme.text_3)
                .child(hint),
        )
        .into_any_element()
}

/// The chip label for a multi-select dimension.
pub fn selection_label(
    all: &str,
    selected: &[u16],
    label_of: impl Fn(u16) -> Option<String>,
) -> String {
    match selected.len() {
        0 => all.to_string(),
        1 => label_of(selected[0]).unwrap_or_else(|| all.to_string()),
        count => format!("{count} selected"),
    }
}

/// A small "narrowed" indicator chip used for the boolean filters that are
/// switched on (errors only / cached only), with an inline clear ×.
pub fn toggle_chip(
    id: String,
    label: impl Into<String>,
    theme: Theme,
    on_clear: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
) -> AnyElement {
    let clear_id = format!("{id}-clear");
    div()
        .id(ElementId::Name(SharedString::from(id)))
        .h(px(28.))
        .pl(px(9.))
        .pr(px(6.))
        .rounded(px(7.))
        .border_1()
        .border_color(theme.border_strong)
        .bg(theme.active)
        .flex()
        .items_center()
        .gap(px(6.))
        .text_size(theme.ui_px(12.))
        .text_color(theme.active_fg)
        .child(label.into())
        .child(
            div()
                .id(ElementId::Name(SharedString::from(clear_id)))
                .size(px(16.))
                .rounded(px(4.))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .hover(|style| style.bg(theme.overlay_strong))
                .on_mouse_down(MouseButton::Left, on_clear)
                .child(icon("icons/x.svg", 10., theme.active_fg)),
        )
        .into_any_element()
}

/// A flat text button (Clear filters, Export…).
pub fn text_button(
    id: &'static str,
    label: &str,
    icon_path: Option<&'static str>,
    enabled: bool,
    theme: Theme,
    on_click: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
) -> AnyElement {
    let color = if enabled { theme.text_2 } else { theme.text_3 };
    div()
        .id(ElementId::Name(SharedString::from(id)))
        .h(px(28.))
        .px(px(8.))
        .rounded(px(7.))
        .flex()
        .items_center()
        .gap(px(6.))
        .text_size(theme.ui_px(12.))
        .text_color(color)
        .when(enabled, |button| {
            button
                .cursor_pointer()
                .hover(|style| style.bg(theme.bg_hover).text_color(theme.text))
                .on_mouse_down(MouseButton::Left, on_click)
        })
        .children(icon_path.map(|path| icon(path, 12., color)))
        .child(label.to_string())
        .into_any_element()
}

/// Options for the workspace dimension, ranked by request count.
pub fn workspace_options(index: &UsageIndex) -> Vec<FilterOption> {
    let mut counts = vec![0u64; index.workspaces.len()];
    for record in &index.requests {
        counts[index.session(record.session).workspace as usize] += 1;
    }
    let mut options: Vec<(u64, FilterOption)> = (0..index.workspaces.len())
        .map(|ix| {
            let entry = &index.workspaces[ix];
            (
                counts[ix],
                FilterOption {
                    id: ix as u16,
                    label: entry.label.clone(),
                    sub: Some(entry.path.clone()),
                },
            )
        })
        .collect();
    options.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.label.cmp(&b.1.label)));
    options.into_iter().map(|(_, option)| option).collect()
}

/// Options for the provider dimension, ranked by request count.
pub fn provider_options(index: &UsageIndex) -> Vec<FilterOption> {
    let mut counts = vec![0u64; index.providers.len()];
    for record in &index.requests {
        counts[index.model(record.model).provider as usize] += 1;
    }
    let mut options: Vec<(u64, FilterOption)> = (0..index.providers.len())
        .map(|ix| {
            let entry = &index.providers[ix];
            (
                counts[ix],
                FilterOption {
                    id: ix as u16,
                    label: entry.label.clone(),
                    sub: Some(format::count(counts[ix])),
                },
            )
        })
        .collect();
    options.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.label.cmp(&b.1.label)));
    options.into_iter().map(|(_, option)| option).collect()
}

/// Options for the model dimension, ranked by request count.
pub fn model_options(index: &UsageIndex) -> Vec<FilterOption> {
    let mut counts = vec![0u64; index.models.len()];
    for record in &index.requests {
        counts[record.model as usize] += 1;
    }
    let mut options: Vec<(u64, FilterOption)> = (0..index.models.len())
        .map(|ix| {
            let entry = &index.models[ix];
            (
                counts[ix],
                FilterOption {
                    id: ix as u16,
                    label: entry.label.clone(),
                    sub: Some(index.provider_of(ix as u16).label.clone()),
                },
            )
        })
        .collect();
    options.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.label.cmp(&b.1.label)));
    options.into_iter().map(|(_, option)| option).collect()
}
