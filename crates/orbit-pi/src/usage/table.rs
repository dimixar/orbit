//! The usage tables, built on GPUI Kit's `Table` (`gpui_component::table`).
//!
//! The framework owns the mechanics this page would otherwise have to invent:
//! virtual scrolling over thousands of rows, resizable and reorderable column
//! headers, keyboard row/column navigation, and hit-tested hover/selection
//! states. Orbit owns the content: `render_td` maps one [`SessionRow`] or
//! [`BucketRow`] onto a cell, and every sort request is forwarded to the page,
//! which stays the single source of truth for ordering (§49/§80).
//!
//! Only the presentation lives here — no metric is computed in a delegate.

use gpui::{
    div, prelude::*, px, AnyElement, App, Context, Div, FontWeight, Hsla, IntoElement,
    MouseButton, Pixels, SharedString, Stateful, WeakEntity, Window,
};
use gpui_component::table::{Column, ColumnSort, TableDelegate, TableState};
use gpui_component::PixelsExt as _;

use super::aggregate::{BucketRow, SessionRow};
use super::format;
use super::model::Granularity;
use super::page::{BucketSort, SessionSort, UsagePage};
use crate::theme::{self, Theme};
use crate::app::icon;

/// Width the "open in chat" affordance gets.
const OPEN_W: f32 = 38.;

// ── sessions ───────────────────────────────────────────────────────────────

/// The sessions table's delegate.
pub struct SessionTable {
    page: WeakEntity<UsagePage>,
    columns: Vec<Column>,
    /// The sort key behind each column; `None` for un-sortable columns.
    keys: Vec<Option<SessionSort>>,
    rows: Vec<SessionRow>,
}

impl SessionTable {
    pub fn new(page: WeakEntity<UsagePage>, width: f32, sort: SessionSort, desc: bool) -> Self {
        let (columns, keys) = columns_for(width, sort, desc);
        Self {
            page,
            columns,
            keys,
            rows: Vec::new(),
        }
    }

    /// Replace the rows. Returns true when they changed.
    pub fn set_rows(&mut self, rows: Vec<SessionRow>) -> bool {
        if self.rows == rows {
            return false;
        }
        self.rows = rows;
        true
    }

    /// Re-derive the column set for a new width or sort. Returns true when the
    /// columns changed and the table needs a refresh.
    pub fn set_layout(&mut self, width: f32, sort: SessionSort, desc: bool) -> bool {
        let (columns, keys) = columns_for(width, sort, desc);
        if self.keys == keys && self.columns.len() == columns.len() {
            return false;
        }
        self.columns = columns;
        self.keys = keys;
        true
    }
}

impl TableDelegate for SessionTable {
    fn columns_count(&self, _: &App) -> usize {
        self.columns.len()
    }

    fn rows_count(&self, _: &App) -> usize {
        self.rows.len()
    }

    fn column(&self, col_ix: usize, _: &App) -> &Column {
        &self.columns[col_ix]
    }

    /// A header click: forward the ordering to the page, which re-sorts and
    /// pushes the new rows back on the next frame.
    fn perform_sort(
        &mut self,
        col_ix: usize,
        sort: ColumnSort,
        _: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) {
        let page = self.page.clone();
        let key = self.keys.get(col_ix).copied().flatten();
        page.update(cx, |page, cx| match (key, sort) {
            // The third click lands on `Default`: back to the page's default
            // ordering rather than an arbitrary one.
            (Some(key), ColumnSort::Ascending) => page.set_session_sort_from_table(key, false, cx),
            (Some(key), ColumnSort::Descending) => page.set_session_sort_from_table(key, true, cx),
            _ => page.set_session_sort_from_table(SessionSort::Tokens, true, cx),
        })
        .ok();
    }

    fn render_td(
        &mut self,
        row_ix: usize,
        col_ix: usize,
        _: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        let theme = *theme::get(cx);
        let Some(row) = self.rows.get(row_ix) else {
            return div().into_any_element();
        };
        let width = self.columns[col_ix].width;
        match self.keys.get(col_ix).copied().flatten() {
            Some(key) => session_cell(row, key, theme, width),
            // The trailing affordance column opens the session in the chat
            // surface without changing the page's scope.
            None => open_session_cell(row.session, self.page.clone(), theme),
        }
    }

    fn render_th(
        &mut self,
        col_ix: usize,
        _: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        let theme = *theme::get(cx);
        let numeric = self
            .keys
            .get(col_ix)
            .copied()
            .flatten()
            .is_some_and(|key| {
                !matches!(
                    key,
                    SessionSort::Title | SessionSort::Workspace | SessionSort::Model
                )
            });
        let width = self.columns[col_ix].width;
        header_cell(&self.columns[col_ix].name, numeric, theme, width)
    }

    /// The row carries the group name its trailing affordance reveals on.
    fn render_tr(
        &mut self,
        row_ix: usize,
        _: &mut Window,
        _: &mut Context<TableState<Self>>,
    ) -> Stateful<Div> {
        div().id(("usage-session-row", row_ix)).group("usage-row")
    }

    fn render_empty(
        &mut self,
        _: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        empty_cell("No sessions match this search.", *theme::get(cx))
    }
}

/// Build the session table's columns and their sort keys.
///
/// The title column takes whatever is left after the fixed columns, so the
/// table fills the width it is given instead of scrolling sideways — and the
/// optional per-bucket token columns are only added when the title still has
/// room to be readable (§58).
fn columns_for(
    width: f32,
    sort: SessionSort,
    desc: bool,
) -> (Vec<Column>, Vec<Option<SessionSort>>) {
    // Room the table steals for its scrollbar, hairlines and cell padding.
    const CHROME: f32 = 48.;
    const GAPS: f32 = 12.;
    const TITLE_MIN: f32 = 180.;
    let budget = (width - CHROME).max(360.);

    let mut plan: Vec<(SessionSort, f32)> = vec![(SessionSort::Title, 0.)];
    let mut fixed = 0.;
    let push = |plan: &mut Vec<(SessionSort, f32)>, fixed: &mut f32, key, w| {
        plan.push((key, w));
        *fixed += w;
    };
    push(&mut plan, &mut fixed, SessionSort::Workspace, 110.);
    push(&mut plan, &mut fixed, SessionSort::Model, 156.);
    if budget - fixed - TITLE_MIN >= 88. + GAPS {
        push(&mut plan, &mut fixed, SessionSort::Started, 88.);
    }
    push(&mut plan, &mut fixed, SessionSort::Duration, 92.);
    push(&mut plan, &mut fixed, SessionSort::Requests, 92.);
    push(&mut plan, &mut fixed, SessionSort::Tokens, 82.);
    push(&mut plan, &mut fixed, SessionSort::Errors, 82.);

    // The three per-bucket token columns are the first thing to go when the
    // window is narrow: the totals still say everything essential.
    let extras = (70. + 76. + 72.) + 3. * GAPS;
    let tail = 2. * GAPS; // the extras sit before tokens/errors
    if budget - fixed - OPEN_W - GAPS - extras - tail - TITLE_MIN >= 0. {
        let at = plan
            .iter()
            .position(|(key, _)| *key == SessionSort::Tokens)
            .unwrap_or(plan.len());
        plan.splice(
            at..at,
            [
                (SessionSort::Input, 70.),
                (SessionSort::Output, 76.),
                (SessionSort::Cache, 72.),
            ],
        );
        fixed += extras - tail;
    }

    let title_w = (budget - fixed - OPEN_W - GAPS).clamp(TITLE_MIN, 460.);

    let sort_state = |key: SessionSort| {
        if key == sort {
            if desc {
                ColumnSort::Descending
            } else {
                ColumnSort::Ascending
            }
        } else {
            ColumnSort::Default
        }
    };

    let mut columns = Vec::with_capacity(plan.len() + 1);
    let mut keys = Vec::with_capacity(plan.len() + 1);
    for (key, column_width) in plan {
        let numeric = !matches!(
            key,
            SessionSort::Title | SessionSort::Workspace | SessionSort::Model
        );
        let width = if column_width > 0.0 {
            column_width
        } else {
            title_w
        };
        let mut column = Column::new(key.as_str(), key.label())
            .width(px(width))
            .sortable()
            .sort(sort_state(key))
            .resizable(true)
            .movable(false);
        if numeric {
            column = column.text_right();
        }
        columns.push(column);
        keys.push(Some(key));
    }
    // The trailing affordance: no header label, no sorting, fixed width.
    columns.push(
        Column::new("open", "")
            .width(px(OPEN_W))
            .resizable(false)
            .movable(false),
    );
    keys.push(None);
    (columns, keys)
}

/// One session cell, in the register its column deserves (§54).
fn session_cell(row: &SessionRow, key: SessionSort, theme: Theme, width: Pixels) -> AnyElement {
    let totals = row.totals;
    let (text, color, numeric) = match key {
        SessionSort::Title => (row.title.clone(), theme.text, false),
        SessionSort::Workspace => (row.workspace.clone(), theme.text_2, false),
        SessionSort::Model => (row.top_model.clone(), theme.text_2, false),
        SessionSort::Started => (
            super::model::bucket_label(row.ended_ms, Granularity::Day),
            theme.text_3,
            false,
        ),
        SessionSort::Duration => (format::span_ms(row.duration_ms()), theme.text_3, true),
        SessionSort::Requests => (format::count(totals.requests), theme.text_2, true),
        SessionSort::Input => (
            format::compact(totals.tokens.input),
            theme.text_3,
            true,
        ),
        SessionSort::Output => (
            format::compact(totals.tokens.output),
            theme.text_3,
            true,
        ),
        SessionSort::Cache => (
            // "0" would claim the provider reported no cache reuse; "—" says
            // nothing was reported at all.
            if totals.tokens.cache_read == 0 && !row.cache_capable {
                "—".to_string()
            } else {
                format::compact(totals.tokens.cache_read)
            },
            theme.text_3,
            true,
        ),
        SessionSort::Tokens => (format::compact(totals.tokens.total), theme.text, true),
        SessionSort::Errors => {
            let errors = totals.errors + row.tool_errors;
            (
                format::count(errors),
                if errors > 0 { theme.crit } else { theme.text_3 },
                true,
            )
        }
        // Tool calls are shown in the tool activity panel, which ranks them
        // properly; the table's column plan does not include one.
        SessionSort::Tools => (format::count(row.tool_runs), theme.text_3, true),
    };
    cell(text, color, numeric, theme, width)
}

/// A cell's shell: fills the framework's cell, truncates, and aligns figures
/// right in tabular digits.
fn cell(text: String, color: Hsla, numeric: bool, theme: Theme, width: Pixels) -> AnyElement {
    let text = clip_to(&text, width, CELL_PADDING, theme.ui_px(11.5).as_f32(), 0.52);
    if numeric {
        return div()
            .size_full()
            .flex()
            .items_center()
            .justify_end()
            .child(
                div()
                    .font(super::view::num_font())
                    .whitespace_nowrap()
                    .text_size(theme.ui_px(11.5))
                    .text_color(color)
                    .child(text),
            )
            .into_any_element();
    }
    div()
        .size_full()
        .flex()
        .items_center()
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(theme.ui_px(11.5))
                .text_color(color)
                .child(text),
        )
        .into_any_element()
}

/// Clip a string to what its column can actually show, on a rough
/// average-glyph factor.
///
/// GPUI's own text ellipsis needs a definite width at shaping time; inside a
/// virtualized table cell the width arrives from the layout pass, so a cell
/// that overflows is cut mid-glyph instead of ending in "…". Budgeting the
/// characters ourselves keeps every clipped cell legible, and the element's
/// own `truncate()` remains as the backstop.
fn clip_to(text: &str, width: Pixels, chrome: f32, font_size: f32, factor: f32) -> String {
    let usable = (width.as_f32() - chrome).max(24.);
    let budget = (usable / (font_size * factor)).floor().max(3.) as usize;
    if text.chars().count() <= budget {
        return text.to_string();
    }
    let mut out: String = text.chars().take(budget.saturating_sub(1)).collect();
    out.push('…');
    out
}

/// The framework's cell padding at `Size::XSmall` (6px each side) plus slack.
const CELL_PADDING: f32 = 14.;
/// A header cell also has to leave room for the framework's sort caret:
/// 6px cell padding either side plus the caret's 16px hit area.
const HEADER_CHROME: f32 = 28.;

/// The header cell: the page's own label register (uppercase 10.5px, tertiary)
/// with the framework's sort caret, so a table header reads exactly like the
/// section label above it.
fn header_cell(label: &str, numeric: bool, theme: Theme, width: Pixels) -> AnyElement {
    // Uppercase runs wider than the values below it, so it is budgeted with a
    // larger factor.
    let text = clip_to(
        &label.to_uppercase(),
        width,
        HEADER_CHROME,
        theme.ui_px(11.).as_f32(),
        0.60,
    );
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .items_center()
        .when(numeric, |cell| cell.justify_end())
        .child(
            div()
                .truncate()
                .text_size(theme.ui_px(11.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(theme.text_3)
                .child(text),
        )
        .into_any_element()
}

/// The trailing affordance: open this session in the chat surface. Hidden
/// until its row is hovered, so thirteen rows do not ship thirteen arrows.
fn open_session_cell(session: u16, page: WeakEntity<UsagePage>, theme: Theme) -> AnyElement {
    div()
        .size_full()
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
                    page.update(cx, |page, cx| page.open_session(window, cx, session))
                        .ok();
                })
                .child(icon("icons/arrow-up-right.svg", 12., theme.text_3)),
        )
        .into_any_element()
}

// ── day/week/month buckets ─────────────────────────────────────────────────

/// The breakdown table's delegate (daily, weekly, or monthly rows).
pub struct BucketTable {
    columns: Vec<Column>,
    keys: Vec<Option<BucketSort>>,
    rows: Vec<BucketRow>,
    granularity: Granularity,
    page: WeakEntity<UsagePage>,
}

impl BucketTable {
    pub fn new(page: WeakEntity<UsagePage>, sort: BucketSort, desc: bool, granularity: Granularity) -> Self {
        let (columns, keys) = bucket_columns(sort, desc);
        Self {
            columns,
            keys,
            rows: Vec::new(),
            granularity,
            page,
        }
    }

    pub fn set_rows(&mut self, rows: Vec<BucketRow>, granularity: Granularity) -> bool {
        let changed = self.rows != rows || self.granularity != granularity;
        self.rows = rows;
        self.granularity = granularity;
        changed
    }

    pub fn set_sort(&mut self, sort: BucketSort, desc: bool) -> bool {
        let (columns, keys) = bucket_columns(sort, desc);
        if self.keys == keys {
            return false;
        }
        self.columns = columns;
        self.keys = keys;
        true
    }
}

impl TableDelegate for BucketTable {
    fn columns_count(&self, _: &App) -> usize {
        self.columns.len()
    }

    fn rows_count(&self, _: &App) -> usize {
        self.rows.len()
    }

    fn column(&self, col_ix: usize, _: &App) -> &Column {
        &self.columns[col_ix]
    }

    fn perform_sort(
        &mut self,
        col_ix: usize,
        sort: ColumnSort,
        _: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) {
        let page = self.page.clone();
        let key = self.keys.get(col_ix).copied().flatten();
        page.update(cx, |page, cx| match (key, sort) {
            (Some(key), ColumnSort::Ascending) => page.set_bucket_sort_from_table(key, false, cx),
            (Some(key), ColumnSort::Descending) => page.set_bucket_sort_from_table(key, true, cx),
            _ => page.set_bucket_sort_from_table(BucketSort::Date, true, cx),
        })
        .ok();
    }

    fn render_td(
        &mut self,
        row_ix: usize,
        col_ix: usize,
        _: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        let theme = *theme::get(cx);
        let Some(row) = self.rows.get(row_ix) else {
            return div().into_any_element();
        };
        let Some(key) = self.keys.get(col_ix).copied().flatten() else {
            return div().into_any_element();
        };
        let granularity = self.granularity;
        let width = self.columns[col_ix].width;
        let (text, color, numeric) = match key {
            BucketSort::Date => (row.label.clone(), theme.text, false),
            BucketSort::Requests => (format::count(row.totals.requests), theme.text_2, true),
            BucketSort::Tokens => (format::compact(row.totals.tokens.total), theme.text, true),
            BucketSort::Cache => (
                match row.totals.cache_hit_rate() {
                    Some(rate) => format::percent(rate),
                    None => "—".to_string(),
                },
                theme.text_3,
                true,
            ),
            BucketSort::Errors => (
                format::count(row.totals.errors),
                if row.totals.errors > 0 {
                    theme.crit
                } else {
                    theme.text_3
                },
                true,
            ),
        };
        // The date column carries the granularity as a quieter suffix, so an
        // all-time monthly table still says what each row is.
        if key == BucketSort::Date {
            return div()
                .size_full()
                .flex()
                .items_center()
                .gap(px(8.))
                .min_w_0()
                .text_size(theme.ui_px(12.))
                .child(div().text_color(theme.text).child(row.label.clone()))
                .child(
                    div()
                        .text_size(theme.ui_px(10.5))
                        .text_color(theme.text_3)
                        .child(granularity.label()),
                )
                .into_any_element();
        }
        cell(text, color, numeric, theme, width)
    }

    fn render_th(
        &mut self,
        col_ix: usize,
        _: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        let theme = *theme::get(cx);
        let numeric = self
            .keys
            .get(col_ix)
            .copied()
            .flatten()
            .is_some_and(|key| key != BucketSort::Date);
        let width = self.columns[col_ix].width;
        header_cell(&self.columns[col_ix].name, numeric, theme, width)
    }

    fn render_empty(
        &mut self,
        _: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        empty_cell("No buckets in this range.", *theme::get(cx))
    }
}

fn bucket_columns(sort: BucketSort, desc: bool) -> (Vec<Column>, Vec<Option<BucketSort>>) {
    let sort_state = |key: BucketSort| {
        if key == sort {
            if desc {
                ColumnSort::Descending
            } else {
                ColumnSort::Ascending
            }
        } else {
            ColumnSort::Default
        }
    };
    let plan: [(BucketSort, f32, bool); 5] = [
        (BucketSort::Date, 220., false),
        (BucketSort::Requests, 104., true),
        (BucketSort::Tokens, 96., true),
        (BucketSort::Cache, 104., true),
        (BucketSort::Errors, 84., true),
    ];
    let mut columns = Vec::with_capacity(plan.len());
    let mut keys = Vec::with_capacity(plan.len());
    for (key, width, numeric) in plan {
        let mut column = Column::new(key.label(), key.label())
            .width(px(width))
            .sortable()
            .sort(sort_state(key))
            .resizable(true)
            .movable(false);
        if numeric {
            column = column.text_right();
        }
        columns.push(column);
        keys.push(Some(key));
    }
    (columns, keys)
}

/// The table's own empty state, styled like the rest of the page.
fn empty_cell(text: &str, theme: Theme) -> AnyElement {
    div()
        .w_full()
        .py(px(24.))
        .flex()
        .justify_center()
        .text_size(theme.ui_px(12.))
        .text_color(theme.text_3)
        .child(text.to_string())
        .into_any_element()
}

// ── failures ───────────────────────────────────────────────────────────────

/// Sort key for the failures table.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FailureSort {
    When,
    Kind,
    Model,
    Session,
}

impl FailureSort {
    fn label(self) -> &'static str {
        match self {
            Self::When => "When",
            Self::Kind => "Type",
            Self::Model => "Model",
            Self::Session => "Session",
        }
    }
}

/// One row of the failures table, already resolved against the index so the
/// delegate does no lookups while painting.
#[derive(Clone, PartialEq, Debug)]
pub struct FailureRow {
    pub ts_ms: i64,
    pub session: u16,
    pub kind: super::model::ErrorKind,
    pub model: String,
    pub session_title: String,
    pub message: String,
}

/// The failures table's delegate (§34): every provider error and tool failure
/// in the range, newest first, each row a jump to the session it happened in.
pub struct FailureTable {
    page: WeakEntity<UsagePage>,
    columns: Vec<Column>,
    keys: Vec<FailureSort>,
    rows: Vec<FailureRow>,
    sort: FailureSort,
    desc: bool,
}

impl FailureTable {
    pub fn new(page: WeakEntity<UsagePage>) -> Self {
        let (columns, keys) = failure_columns(FailureSort::When, true, 1200.);
        Self {
            page,
            columns,
            keys,
            rows: Vec::new(),
            sort: FailureSort::When,
            desc: true,
        }
    }

    pub fn set_rows(&mut self, rows: Vec<FailureRow>) -> bool {
        if self.rows == rows {
            return false;
        }
        self.rows = rows;
        true
    }

    pub fn set_layout(&mut self, sort: FailureSort, desc: bool, width: f32) -> bool {
        if self.sort == sort && self.desc == desc {
            return false;
        }
        self.sort = sort;
        self.desc = desc;
        let (columns, keys) = failure_columns(sort, desc, width);
        self.columns = columns;
        self.keys = keys;
        true
    }
}

fn failure_columns(
    sort: FailureSort,
    desc: bool,
    width: f32,
) -> (Vec<Column>, Vec<FailureSort>) {
    let sort_state = |key: FailureSort| {
        if key == sort {
            if desc {
                ColumnSort::Descending
            } else {
                ColumnSort::Ascending
            }
        } else {
            ColumnSort::Default
        }
    };
    // The message column takes the remainder; the others are content-sized.
    let fixed = 108. + 96. + 150. + 200. + 60.;
    let message_w = (width - fixed).clamp(180., 560.);
    let plan: [(FailureSort, f32); 5] = [
        (FailureSort::When, 108.),
        (FailureSort::Kind, 96.),
        (FailureSort::Model, 150.),
        (FailureSort::Session, 200.),
        (FailureSort::Model, message_w),
    ];
    let mut columns = Vec::with_capacity(5);
    let mut keys = Vec::with_capacity(5);
    for (ix, (key, w)) in plan.into_iter().enumerate() {
        let is_message = ix == 4;
        let mut column = Column::new(
            if is_message { "message" } else { key.label() },
            if is_message { "Message" } else { key.label() },
        )
        .width(px(w))
        .resizable(true)
        .movable(false);
        if !is_message {
            column = column.sortable().sort(sort_state(key));
        }
        columns.push(column);
        keys.push(key);
    }
    (columns, keys)
}

impl TableDelegate for FailureTable {
    fn columns_count(&self, _: &App) -> usize {
        self.columns.len()
    }

    fn rows_count(&self, _: &App) -> usize {
        self.rows.len()
    }

    fn column(&self, col_ix: usize, _: &App) -> &Column {
        &self.columns[col_ix]
    }

    fn perform_sort(
        &mut self,
        col_ix: usize,
        sort: ColumnSort,
        _: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) {
        let page = self.page.clone();
        let key = self.keys.get(col_ix).copied().unwrap_or(FailureSort::When);
        page.update(cx, |page, cx| match sort {
            ColumnSort::Ascending => page.set_failure_sort(key, false, cx),
            ColumnSort::Descending => page.set_failure_sort(key, true, cx),
            ColumnSort::Default => page.set_failure_sort(FailureSort::When, true, cx),
        })
        .ok();
    }

    fn render_td(
        &mut self,
        row_ix: usize,
        col_ix: usize,
        _: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        let theme = *theme::get(cx);
        let Some(row) = self.rows.get(row_ix) else {
            return div().into_any_element();
        };
        let width = self.columns[col_ix].width;
        let is_message = col_ix == 4;
        let (text, color, numeric) = if is_message {
            (row.message.clone(), theme.text_3, false)
        } else {
            match self.keys.get(col_ix).copied().unwrap_or(FailureSort::When) {
                FailureSort::When => (
                    super::model::local_datetime(row.ts_ms)
                        .map(|dt| dt.format("%b %-d, %H:%M").to_string())
                        .unwrap_or_default(),
                    theme.text_3,
                    true,
                ),
                FailureSort::Kind => (
                    row.kind.label().to_string(),
                    match row.kind {
                        super::model::ErrorKind::Provider => theme.crit,
                        super::model::ErrorKind::Tool => theme.warn,
                    },
                    false,
                ),
                FailureSort::Model => (row.model.clone(), theme.text_2, false),
                FailureSort::Session => (row.session_title.clone(), theme.text_2, false),
            }
        };
        cell(text, color, numeric, theme, width)
    }

    fn render_th(
        &mut self,
        col_ix: usize,
        _: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        let theme = *theme::get(cx);
        let numeric = self.keys.get(col_ix).copied() == Some(FailureSort::When);
        let width = self.columns[col_ix].width;
        header_cell(&self.columns[col_ix].name, numeric, theme, width)
    }

    fn render_empty(
        &mut self,
        _: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        empty_cell("No failed requests in this range.", *theme::get(cx))
    }
}
