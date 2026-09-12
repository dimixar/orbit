//! The Usage page's state: one scanner, one cached snapshot, one filter.
//!
//! The page owns everything mutable — the filter, the view preferences, the
//! popover state, and the last computed [`UsageSnapshot`]. Recompute happens
//! on data arrival or on a state change, never on a paint (§51): the render
//! path reads `snapshot()` and formats.

use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::{
    point, px, App, AppContext, Context, Entity, Focusable, ScrollHandle, Subscription, Window,
};
use gpui_component::table::{TableEvent, TableState};
use serde_json::Value;

use crate::composer::ComposerInput;

use super::aggregate::{BucketRow, ChartMetric, SessionRow, UsageSnapshot};
use super::collect::{now_ms, UsageScanner};
use super::model::*;
use super::table::{BucketTable, FailureSort, FailureTable, FailureRow, SessionTable};

/// How long the page may serve a stale index before it rescans on open.
const STALE_AFTER: Duration = Duration::from_secs(60);
/// Minimum gap between rescans triggered by store writes while the page is
/// open, so a streaming agent doesn't spin the scanner.
const RESCAN_INTERVAL: Duration = Duration::from_secs(3);

/// Which row the session table sorts on.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SessionSort {
    Started,
    Title,
    Workspace,
    Model,
    Requests,
    Input,
    Output,
    Cache,
    Tokens,
    Duration,
    Errors,
    Tools,
}

impl SessionSort {
    pub fn label(self) -> &'static str {
        match self {
            Self::Started => "Started",
            Self::Title => "Session",
            Self::Workspace => "Workspace",
            Self::Model => "Model",
            Self::Requests => "Requests",
            Self::Input => "Input",
            Self::Output => "Output",
            Self::Cache => "Cache",
            Self::Tokens => "Tokens",
            Self::Duration => "Duration",
            Self::Errors => "Errors",
            Self::Tools => "Tools",
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Started => "started",
            Self::Title => "title",
            Self::Workspace => "workspace",
            Self::Model => "model",
            Self::Requests => "requests",
            Self::Input => "input",
            Self::Output => "output",
            Self::Cache => "cache",
            Self::Tokens => "tokens",
            Self::Duration => "duration",
            Self::Errors => "errors",
            Self::Tools => "tools",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        [
            Self::Started,
            Self::Title,
            Self::Workspace,
            Self::Model,
            Self::Requests,
            Self::Input,
            Self::Output,
            Self::Cache,
            Self::Tokens,
            Self::Duration,
            Self::Errors,
            Self::Tools,
        ]
        .into_iter()
        .find(|s| s.as_str() == raw.trim())
    }

    /// Numeric value for sorting; text columns sort on their label.
    fn key(self, row: &SessionRow) -> f64 {
        match self {
            Self::Started => row.ended_ms as f64,
            Self::Requests => row.totals.requests as f64,
            Self::Input => row.totals.tokens.input as f64,
            Self::Output => row.totals.tokens.output as f64,
            Self::Cache => row.totals.tokens.cache_read as f64,
            Self::Tokens => row.totals.tokens.total as f64,
            Self::Duration => row.duration_ms() as f64,
            Self::Errors => (row.totals.errors + row.tool_errors) as f64,
            Self::Tools => row.tool_runs as f64,
            Self::Title | Self::Workspace | Self::Model => 0.0,
        }
    }

    fn text(self, row: &SessionRow) -> &str {
        match self {
            Self::Title => &row.title,
            Self::Workspace => &row.workspace,
            Self::Model => &row.top_model,
            _ => "",
        }
    }

    fn is_text(self) -> bool {
        matches!(self, Self::Title | Self::Workspace | Self::Model)
    }
}

/// Which row the day/week/month table sorts on.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BucketSort {
    Date,
    Requests,
    Tokens,
    Cache,
    Errors,
}

impl BucketSort {
    pub fn label(self) -> &'static str {
        match self {
            Self::Date => "Date",
            Self::Requests => "Requests",
            Self::Tokens => "Tokens",
            Self::Cache => "Cache hit",
            Self::Errors => "Errors",
        }
    }

    fn key(self, row: &BucketRow) -> f64 {
        match self {
            Self::Date => row.start_ms as f64,
            Self::Requests => row.totals.requests as f64,
            Self::Tokens => row.totals.tokens.total as f64,
            Self::Cache => row.totals.cache_hit_rate().unwrap_or(-1.0),
            Self::Errors => row.totals.errors as f64,
        }
    }
}

/// The open filter popover, if any. One at a time, like the app's other
/// anchored menus.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MenuKind {
    Range,
    Workspace,
    Provider,
    Model,
    /// The header's export popover.
    Export,
}

/// Export flavors: the raw filtered records, or the aggregates on screen.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ExportFormat {
    Csv,
    Json,
}

/// Opens a session in the app's chat surface (installed by `OrbitApp`).
pub type OpenSession = Rc<dyn Fn(&str, &mut Window, &mut App)>;
/// Leaves the usage page (installed by `OrbitApp`).
pub type Close = Rc<dyn Fn(&mut Window, &mut App)>;

pub struct UsagePage {
    scanner: UsageScanner,
    index: Option<Rc<UsageIndex>>,
    snapshot: Option<Rc<UsageSnapshot>>,
    /// The snapshot no longer matches the filter or the index.
    dirty: bool,

    // ── filter + view state ──
    filter: UsageFilter,
    metric: ChartMetric,
    session_sort: SessionSort,
    session_sort_desc: bool,
    bucket_sort: BucketSort,
    bucket_sort_desc: bool,
    failure_sort: FailureSort,
    failure_sort_desc: bool,

    // ── controls ──
    /// The page's own scroll position. Opening the page always starts at the
    /// top; a refresh never moves it (§82).
    scroll: ScrollHandle,
    search: Entity<ComposerInput>,
    menu: Option<MenuKind>,
    menu_query: Entity<ComposerInput>,
    menu_highlight: usize,
    /// Month shown in the custom-range calendar (epoch ms inside the month).
    calendar_month: Option<i64>,
    /// The calendar is expanded inside the range menu.
    calendar_open: bool,
    /// Draft custom-range bounds, applied when both ends are chosen.
    custom_start: Option<i64>,
    custom_end: Option<i64>,
    /// When a popover was dismissed by an outside click; guards the same
    /// gesture's mouse-up from re-opening the menu it just closed.
    dismissed_at: Option<Instant>,

    // ── transient state ──
    hover_bucket: Option<usize>,
    loading: bool,
    refreshing: bool,
    last_request: Option<Instant>,
    last_updated_ms: Option<i64>,
    error: Option<String>,
    status: Option<(String, Instant)>,

    /// The two framework-owned tables. The page keeps their delegates in step
    /// with the current snapshot and sort state.
    session_table: Option<Entity<TableState<SessionTable>>>,
    bucket_table: Option<Entity<TableState<BucketTable>>>,
    failure_table: Option<Entity<TableState<FailureTable>>>,
    /// Row/column changed since the delegates were last refreshed.
    tables_dirty: bool,
    /// Re-render when the session search or the menu filter changes: those
    /// inputs live in their own entities, so the page has to watch them.
    _search_sub: Subscription,
    _menu_query_sub: Subscription,
    /// Row-event subscriptions for the tables.
    _table_subs: Vec<Subscription>,
    /// Width of the main area, refreshed by the shell each render.
    main_width: f32,

    on_open_session: Option<OpenSession>,
    on_close: Option<Close>,
}

impl UsagePage {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let prefs = Prefs::load();
        let now = now_ms();
        let mut range = match RangePreset::parse(&prefs.preset) {
            Some(RangePreset::Custom) => match (prefs.custom_start, prefs.custom_end) {
                // A saved window with nonsense bounds (an epoch stamp, an
                // inverted pair) falls back to a range that shows something.
                (Some(start), Some(end)) if end > start && start > 0 => {
                    DateRange::custom(start, end)
                }
                _ => DateRange::for_preset(RangePreset::Last7, now),
            },
            Some(preset) => DateRange::for_preset(preset, now),
            None => DateRange::for_preset(RangePreset::Last7, now),
        };
        // A persisted relative preset is re-resolved against "now" (the window
        // moves), which is the point of storing the preset and not the range.
        if range.preset != RangePreset::Custom {
            range = DateRange::for_preset(range.preset, now);
        }
        let search = cx.new(|cx| {
            ComposerInput::new(cx)
                .with_element_id("usage-session-search")
                .with_placeholder("Search sessions…")
                .with_key_context("Composer Picker")
                .with_max_lines(1)
        });
        let menu_query = cx.new(|cx| {
            ComposerInput::new(cx)
                .with_element_id("usage-menu-query")
                .with_placeholder("Filter…")
                .with_key_context("Composer Picker")
                .with_max_lines(1)
        });
        let search_sub = cx.observe(&search, |_, _, cx| {
            // The search narrows the sessions table, so its rows are stale.
            cx.notify();
        });
        let menu_query_sub = cx.observe(&menu_query, |_, _, cx| cx.notify());
        Self {
            scanner: UsageScanner::start(),
            index: None,
            snapshot: None,
            dirty: true,
            filter: UsageFilter::new(range),
            metric: ChartMetric::parse(&prefs.metric).unwrap_or(ChartMetric::Tokens),
            session_sort: SessionSort::parse(&prefs.session_sort).unwrap_or(SessionSort::Tokens),
            session_sort_desc: prefs.session_sort_desc,
            bucket_sort: BucketSort::Date,
            bucket_sort_desc: true,
            failure_sort: FailureSort::When,
            failure_sort_desc: true,
            scroll: ScrollHandle::new(),
            search,
            menu: None,
            menu_query,
            menu_highlight: 0,
            calendar_month: None,
            calendar_open: false,
            custom_start: prefs.custom_start,
            custom_end: prefs.custom_end,
            dismissed_at: None,
            hover_bucket: None,
            loading: true,
            refreshing: false,
            last_request: None,
            last_updated_ms: None,
            error: None,
            status: None,
            session_table: None,
            bucket_table: None,
            failure_table: None,
            tables_dirty: true,
            _search_sub: search_sub,
            _menu_query_sub: menu_query_sub,
            _table_subs: Vec::new(),
            main_width: 1000.,
            on_open_session: None,
            on_close: None,
        }
    }

    pub fn set_open_session(&mut self, handler: OpenSession) {
        self.on_open_session = Some(handler);
    }

    pub fn set_on_close(&mut self, handler: Close) {
        self.on_close = Some(handler);
    }

    /// Enter the page: load on first open, and quietly re-check the store when
    /// the last scan is old enough to matter.
    pub fn open(&mut self, cx: &mut Context<Self>) {
        self.scroll.set_offset(point(px(0.), px(0.)));
        let have_none = self.index.is_none();
        let stale = self
            .last_updated_ms
            .is_none_or(|at| now_ms() - at >= STALE_AFTER.as_millis() as i64);
        self.maybe_scan(have_none || stale, cx);
    }

    pub fn close(&mut self, _cx: &mut Context<Self>) {
        self.menu = None;
        self.hover_bucket = None;
    }

    /// The user asked for fresh numbers.
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        self.maybe_scan(true, cx);
    }

    /// The session store changed under us (pi wrote a session while the page
    /// is open). Rate-limited so a streaming agent doesn't spin the scanner.
    pub fn mark_stale(&mut self, cx: &mut Context<Self>) {
        let due = self.last_request.is_none_or(|last| {
            Instant::now().duration_since(last) >= RESCAN_INTERVAL
        });
        self.maybe_scan(due, cx);
    }

    /// Called from the app heartbeat: drain finished scans and recompute.
    pub fn sync(&mut self, cx: &mut Context<Self>) {
        let mut changed = false;
        if let Some(index) = self.scanner.take_index() {
            self.loading = false;
            self.refreshing = false;
            self.last_updated_ms = Some(index.scanned_at_ms);
            self.error = if index.is_empty() && index.unreadable_files > 0 {
                Some(format!(
                    "{} session files could not be read",
                    index.unreadable_files
                ))
            } else {
                None
            };
            let index = Rc::new(index);
            self.prune_filter(&index);
            self.index = Some(index);
            self.dirty = true;
            changed = true;
        }
        if self.dirty {
            self.recompute();
            changed = true;
        }
        if let Some((_, at)) = &self.status {
            if at.elapsed() >= Duration::from_secs(6) {
                self.status = None;
                changed = true;
            }
        }
        if changed {
            cx.notify();
        }
    }

    fn maybe_scan(&mut self, condition: bool, cx: &mut Context<Self>) {
        if !condition {
            return;
        }
        self.scanner.request_scan();
        self.last_request = Some(Instant::now());
        if self.index.is_some() {
            self.refreshing = true;
        } else {
            self.loading = true;
        }
        cx.notify();
    }

    /// Drop filter ids that the new index does not contain. A session file
    /// removed while it was scoped is the realistic case: the scope has to
    /// clear itself (and say so) rather than leave the page empty forever.
    fn prune_filter(&mut self, index: &UsageIndex) {
        let mut filter = self.filter.clone();
        let before = filter.clone();
        if filter.session.is_some_and(|id| index.try_session(id).is_none()) {
            filter.session = None;
            self.status = Some((
                "That session is no longer in the store — scope cleared".to_string(),
                Instant::now(),
            ));
        }
        filter.workspaces.retain(|id| index.workspaces.len() > *id as usize);
        filter.providers.retain(|id| index.providers.len() > *id as usize);
        filter.models.retain(|id| index.models.len() > *id as usize);
        if filter != before {
            self.filter = filter;
        }
    }

    /// Recompute the snapshot. Only ever called when `dirty` (§51).
    fn recompute(&mut self) {
        self.dirty = false;
        self.tables_dirty = true;
        let Some(index) = self.index.clone() else {
            self.snapshot = None;
            return;
        };
        self.snapshot = Some(Rc::new(UsageSnapshot::compute(&index, &self.filter)));

    }

    // ── reads used by the view ─────────────────────────────────────────────

    /// The scroll handle the body tracks, so opening the page can start at the
    /// top without disturbing a refresh.
    pub fn scroll(&self) -> &ScrollHandle {
        &self.scroll
    }

    pub fn snapshot(&self) -> Option<&Rc<UsageSnapshot>> {
        self.snapshot.as_ref()
    }

    /// The width of the main area the shell gave us. The page lays itself out
    /// against this instead of guessing from the window size (§58).
    pub fn main_width(&self) -> f32 {
        self.main_width
    }

    /// The width a table actually gets: the page column is capped at
    /// [`super::view::CONTENT_MAX_W`] and padded, so budgeting against the raw
    /// main-area width would over-commit by the difference and squeeze the last
    /// column.
    pub(super) fn table_width(&self) -> f32 {
        (self.main_width.min(super::view::CONTENT_MAX_W) - super::view::PAGE_PAD).max(360.)
    }

    /// Set by `OrbitApp` on every render: the main area's width in points.
    pub fn set_main_width(&mut self, width: f32, cx: &mut Context<Self>) {
        if (self.main_width - width).abs() > 0.5 {
            self.main_width = width;
            // Column widths follow the available width.
            self.tables_dirty = true;
            cx.notify();
        }
    }

    // ── tables (gpui-component) ────────────────────────────────────────────

    /// Create the tables on first render (they need a window) and hand back
    /// their states.
    pub(super) fn tables(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> (
        Entity<TableState<SessionTable>>,
        Entity<TableState<BucketTable>>,
    ) {
        if self.session_table.is_none() {
            let page = cx.entity().downgrade();
            let (width, sort, desc) = (self.main_width, self.session_sort, self.session_sort_desc);
            let state = cx.new(|cx| {
                TableState::new(SessionTable::new(page, width, sort, desc), window, cx)
                    .row_selectable(true)
            });
            let sub = cx.subscribe_in(
                &state,
                window,
                |page: &mut Self, _state, event: &TableEvent, window, cx| {
                    page.on_table_event(event, window, cx);
                },
            );
            self._table_subs.push(sub);
            self.session_table = Some(state);
        }
        if self.bucket_table.is_none() {
            let page = cx.entity().downgrade();
            let (sort, desc) = (self.bucket_sort, self.bucket_sort_desc);
            let state = cx.new(|cx| {
                TableState::new(BucketTable::new(page, sort, desc, Granularity::Day), window, cx)
            });
            let sub = cx.subscribe_in(
                &state,
                window,
                |page: &mut Self, _state, event: &TableEvent, window, cx| {
                    page.on_table_event(event, window, cx);
                },
            );
            self._table_subs.push(sub);
            self.bucket_table = Some(state);
        }
        if self.failure_table.is_none() {
            let page = cx.entity().downgrade();
            let state = cx.new(|cx| TableState::new(FailureTable::new(page), window, cx));
            let sub = cx.subscribe_in(
                &state,
                window,
                |page: &mut Self, _state, event: &TableEvent, window, cx| {
                    page.on_table_event(event, window, cx);
                },
            );
            self._table_subs.push(sub);
            self.failure_table = Some(state);
        }
        (
            self.session_table.clone().expect("created above"),
            self.bucket_table.clone().expect("created above"),
        )
    }

    /// Create (if needed) and hand back the failures table.
    pub(super) fn failure_table(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<TableState<FailureTable>> {
        self.tables(window, cx);
        self.failure_table.clone().expect("created above")
    }

    /// Failures in the range, resolved against the index and ordered by the
    /// table's current sort.
    pub fn failure_rows(&self) -> Vec<FailureRow> {
        let (Some(index), Some(snapshot)) = (self.index.as_ref(), self.snapshot.as_ref()) else {
            return Vec::new();
        };
        let mut rows: Vec<FailureRow> = snapshot
            .errors
            .rows
            .iter()
            .map(|row| FailureRow {
                ts_ms: row.ts_ms,
                session: row.session,
                kind: row.kind,
                model: index.model(row.model).label.clone(),
                session_title: {
                    let entry = index.session(row.session);
                    if entry.title.is_empty() {
                        format!("Session {}", &entry.id.chars().take(8).collect::<String>())
                    } else {
                        entry.title.clone()
                    }
                },
                message: row.message.clone(),
            })
            .collect();
        let desc = self.failure_sort_desc;
        rows.sort_by(|a, b| {
            let ordering = match self.failure_sort {
                FailureSort::When => a.ts_ms.cmp(&b.ts_ms),
                FailureSort::Kind => a.kind.label().cmp(b.kind.label()),
                FailureSort::Model => a.model.to_lowercase().cmp(&b.model.to_lowercase()),
                FailureSort::Session => a
                    .session_title
                    .to_lowercase()
                    .cmp(&b.session_title.to_lowercase()),
            };
            let ordering = if desc { ordering.reverse() } else { ordering };
            ordering.then_with(|| a.ts_ms.cmp(&b.ts_ms))
        });
        rows
    }

    /// A header click on the failures table.
    pub fn set_failure_sort(
        &mut self,
        sort: FailureSort,
        desc: bool,
        cx: &mut Context<Self>,
    ) {
        if self.failure_sort == sort && self.failure_sort_desc == desc {
            return;
        }
        self.failure_sort = sort;
        self.failure_sort_desc = desc;
        self.tables_dirty = true;
        cx.notify();
    }

    /// Push the current rows and column plan into the delegates. Runs before
    /// the tables paint, and only when something they show actually changed.
    pub(super) fn sync_tables(&mut self, cx: &mut Context<Self>) {
        if !self.tables_dirty {
            return;
        }
        self.tables_dirty = false;

        let width = self.table_width();
        let (sort, desc) = (self.session_sort, self.session_sort_desc);
        let rows = self.session_rows(cx);
        if let Some(state) = self.session_table.clone() {
            state.update(cx, |state, cx| {
                let delegate = state.delegate_mut();
                let mut changed = delegate.set_rows(rows);
                changed |= delegate.set_layout(width, sort, desc);
                if changed {
                    state.refresh(cx);
                }
            });
        }

        let granularity = self
            .snapshot
            .as_ref()
            .map(|snapshot| snapshot.buckets.granularity)
            .unwrap_or(Granularity::Day);
        let rows = self.bucket_rows();
        let (sort, desc) = (self.bucket_sort, self.bucket_sort_desc);
        if let Some(state) = self.failure_table.clone() {
            let failures = self.failure_rows();
            let (sort, desc) = (self.failure_sort, self.failure_sort_desc);
            state.update(cx, |state, cx| {
                let delegate = state.delegate_mut();
                let mut changed = delegate.set_rows(failures);
                changed |= delegate.set_layout(sort, desc, width);
                if changed {
                    state.refresh(cx);
                }
            });
        }
        if let Some(state) = self.bucket_table.clone() {
            state.update(cx, |state, cx| {
                let delegate = state.delegate_mut();
                let mut changed = delegate.set_rows(rows, granularity);
                changed |= delegate.set_sort(sort, desc);
                if changed {
                    state.refresh(cx);
                }
            });
        }
    }

    /// Row events from either table: the row order is the page's own, so a
    /// row index maps back onto the same [`SessionRow`] list the delegate draws.
    fn on_table_event(&mut self, event: &TableEvent, window: &mut Window, cx: &mut Context<Self>) {
        let ix = match event {
            TableEvent::SelectRow(ix) | TableEvent::DoubleClickedRow(ix) => *ix,
            _ => return,
        };
        let rows = self.session_rows(cx);
        let Some(row) = rows.get(ix) else {
            return;
        };
        let session = row.session;
        match event {
            // A single click scopes the whole page to that session (§68).
            TableEvent::SelectRow(_) => self.set_session_scope(Some(session), cx),
            // A double click goes further and opens it in the chat (§29).
            TableEvent::DoubleClickedRow(_) => self.open_session(window, cx, session),
            _ => {}
        }
    }

    /// The directory this page reads (pi's own session store).
    pub fn store_path(&self) -> Option<String> {
        Some(self.scanner.store().to_string_lossy().to_string())
    }

    pub fn index(&self) -> Option<&Rc<UsageIndex>> {
        self.index.as_ref()
    }

    pub fn filter(&self) -> &UsageFilter {
        &self.filter
    }

    pub fn metric(&self) -> ChartMetric {
        self.metric
    }

    pub fn is_loading(&self) -> bool {
        self.loading && self.snapshot.is_none()
    }

    pub fn is_refreshing(&self) -> bool {
        self.refreshing
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub fn status(&self) -> Option<&str> {
        self.status.as_ref().map(|(text, _)| text.as_str())
    }

    pub fn last_updated_ms(&self) -> Option<i64> {
        self.last_updated_ms
    }

    pub fn hover_bucket(&self) -> Option<usize> {
        self.hover_bucket
    }

    pub fn menu(&self) -> Option<MenuKind> {
        self.menu
    }

    pub fn menu_highlight(&self) -> usize {
        self.menu_highlight
    }

    pub fn search(&self) -> &Entity<ComposerInput> {
        &self.search
    }

    pub fn menu_query(&self) -> &Entity<ComposerInput> {
        &self.menu_query
    }

    pub fn calendar_month(&self) -> Option<i64> {
        self.calendar_month
    }

    pub fn calendar_open(&self) -> bool {
        self.calendar_open
    }

    pub fn custom_bounds(&self) -> (Option<i64>, Option<i64>) {
        (self.custom_start, self.custom_end)
    }

    /// Session rows, filtered by the search box and ordered by the active sort.
    pub fn session_rows(&self, cx: &App) -> Vec<SessionRow> {
        let Some(snapshot) = &self.snapshot else {
            return Vec::new();
        };
        let query = self.search.read(cx).text().to_lowercase();
        let mut rows: Vec<SessionRow> = snapshot
            .sessions
            .iter()
            .filter(|row| {
                if query.is_empty() {
                    return true;
                }
                row.title.to_lowercase().contains(&query)
                    || row.workspace.to_lowercase().contains(&query)
                    || row.top_model.to_lowercase().contains(&query)
            })
            .cloned()
            .collect();
        let sort = self.session_sort;
        let desc = self.session_sort_desc;
        rows.sort_by(|a, b| {
            let ordering = if sort.is_text() {
                sort.text(a).to_lowercase().cmp(&sort.text(b).to_lowercase())
            } else {
                sort.key(a)
                    .partial_cmp(&sort.key(b))
                    .unwrap_or(std::cmp::Ordering::Equal)
            };
            let ordering = if desc { ordering.reverse() } else { ordering };
            ordering.then_with(|| a.title.cmp(&b.title))
        });
        rows
    }

    pub fn bucket_rows(&self) -> Vec<BucketRow> {
        let Some(snapshot) = &self.snapshot else {
            return Vec::new();
        };
        let mut rows = snapshot.buckets.rows.clone();
        let sort = self.bucket_sort;
        let desc = self.bucket_sort_desc;
        rows.sort_by(|a, b| {
            let ordering = sort
                .key(a)
                .partial_cmp(&sort.key(b))
                .unwrap_or(std::cmp::Ordering::Equal);
            if desc {
                ordering.reverse()
            } else {
                ordering
            }
        });
        rows
    }

    // ── mutations ──────────────────────────────────────────────────────────

    pub fn set_filter(&mut self, filter: UsageFilter, cx: &mut Context<Self>) {
        self.filter = filter;
        self.dirty = true;
        self.recompute();
        self.persist();
        cx.notify();
    }

    pub fn set_preset(&mut self, preset: RangePreset, cx: &mut Context<Self>) {
        let mut filter = self.filter.clone();
        filter.range = DateRange::for_preset(preset, now_ms());
        self.custom_start = None;
        self.custom_end = None;
        self.set_filter(filter, cx);
    }

    pub fn set_metric(&mut self, metric: ChartMetric, cx: &mut Context<Self>) {
        if self.metric == metric {
            return;
        }
        self.metric = metric;
        self.persist();
        cx.notify();
    }

    pub fn set_hover_bucket(&mut self, bucket: Option<usize>, cx: &mut Context<Self>) {
        if self.hover_bucket != bucket {
            self.hover_bucket = bucket;
            cx.notify();
        }
    }

    pub fn open_menu(&mut self, menu: Option<MenuKind>, window: &mut Window, cx: &mut Context<Self>) {
        self.menu = menu;
        self.menu_highlight = 0;
        self.calendar_open = false;
        if menu == Some(MenuKind::Range) {
            // Start the calendar on the current range's month.
            self.calendar_month = Some(self.filter.range.start_ms);
        }
        self.menu_query.update(cx, |input, cx| input.clear(cx));
        // Only the multi-select menus own a text field; the range menu keeps
        // focus where it was so Escape/arrows still reach the page.
        if matches!(
            menu,
            Some(MenuKind::Workspace) | Some(MenuKind::Provider) | Some(MenuKind::Model)
        ) {
            let handle = self.menu_query.read(cx).focus_handle(cx);
            window.focus(&handle);
        }
        cx.notify();
    }

    /// Toggle a chip's popover, ignoring the mouse-up half of a click that
    /// just dismissed a popover outside its bounds.
    pub fn toggle_menu(&mut self, kind: MenuKind, window: &mut Window, cx: &mut Context<Self>) {
        let just_dismissed = self
            .dismissed_at
            .is_some_and(|at| at.elapsed() < Duration::from_millis(250));
        if self.menu == Some(kind) || just_dismissed {
            self.menu = None;
            self.calendar_open = false;
            cx.notify();
            return;
        }
        self.open_menu(Some(kind), window, cx);
    }

    /// Close whatever popover is open (no window needed — used after an
    /// action taken from inside a menu).
    pub fn close_menu(&mut self, cx: &mut Context<Self>) {
        if self.menu.is_none() {
            return;
        }
        self.menu = None;
        self.calendar_open = false;
        cx.notify();
    }

    /// Any click outside a popover closes it.
    pub fn dismiss_menus(&mut self, cx: &mut Context<Self>) {
        if self.menu.is_none() {
            return;
        }
        self.menu = None;
        self.dismissed_at = Some(Instant::now());
        self.calendar_open = false;
        cx.notify();
    }

    /// Expand the custom-range calendar inside the range menu.
    ///
    /// The draft window is seeded from what the dashboard is currently
    /// showing, and the calendar opens where the data is — an all-time range
    /// starts at the epoch, which would open the picker in 1970.
    pub fn open_calendar(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.menu = Some(MenuKind::Range);
        self.calendar_open = true;
        let range = self.filter.range.clone();
        let (mut start, mut end) = (range.start_ms, (range.end_ms - 1).max(range.start_ms));
        if range.preset == RangePreset::All {
            if let Some((oldest, newest)) = self.index.as_ref().and_then(|index| index.span()) {
                start = oldest;
                end = newest;
            }
        }
        self.custom_start = Some(local_day_start(start));
        self.custom_end = Some(local_day_start(end));
        self.calendar_month = Some(end);
        cx.notify();
    }

    pub fn set_calendar_month(&mut self, month_ms: i64, cx: &mut Context<Self>) {
        self.calendar_month = Some(month_ms);
        cx.notify();
    }

    pub fn pick_day(&mut self, day_ms: i64, cx: &mut Context<Self>) {
        // First click sets the start, second completes the range; a third
        // starts over.
        match (self.custom_start, self.custom_end) {
            (_, Some(_)) => {
                self.custom_start = Some(day_ms);
                self.custom_end = None;
                cx.notify();
            }
            (Some(start), None) if day_ms < start => {
                self.custom_start = Some(day_ms);
                self.custom_end = Some(start);
                self.apply_custom(cx);
            }
            (Some(_), None) => {
                self.custom_end = Some(day_ms);
                self.apply_custom(cx);
            }
            (None, None) => {
                self.custom_start = Some(day_ms);
                cx.notify();
            }
        }
    }

    fn apply_custom(&mut self, cx: &mut Context<Self>) {
        if let (Some(start), Some(end)) = (self.custom_start, self.custom_end) {
            let mut filter = self.filter.clone();
            filter.range = DateRange::custom(start, end);
            self.set_filter(filter, cx);
            self.menu = None;
            self.calendar_open = false;
        }
    }

    pub fn set_menu_highlight(&mut self, ix: usize, cx: &mut Context<Self>) {
        if self.menu_highlight != ix {
            self.menu_highlight = ix;
            cx.notify();
        }
    }

    /// Toggle one value in a multi-select dimension.
    pub fn toggle_filter_value(&mut self, kind: MenuKind, id: u16, cx: &mut Context<Self>) {
        let mut filter = self.filter.clone();
        let list = match kind {
            MenuKind::Workspace => &mut filter.workspaces,
            MenuKind::Provider => &mut filter.providers,
            MenuKind::Model => &mut filter.models,
            MenuKind::Range | MenuKind::Export => return,
        };
        if let Some(ix) = list.iter().position(|value| *value == id) {
            list.remove(ix);
        } else {
            list.push(id);
        }
        self.set_filter(filter, cx);
    }

    pub fn select_all(&mut self, kind: MenuKind, ids: &[u16], cx: &mut Context<Self>) {
        let mut filter = self.filter.clone();
        match kind {
            MenuKind::Workspace => filter.workspaces = ids.to_vec(),
            MenuKind::Provider => filter.providers = ids.to_vec(),
            MenuKind::Model => filter.models = ids.to_vec(),
            MenuKind::Range | MenuKind::Export => return,
        }
        self.set_filter(filter, cx);
    }

    pub fn clear_dimension(&mut self, kind: MenuKind, cx: &mut Context<Self>) {
        let mut filter = self.filter.clone();
        match kind {
            MenuKind::Workspace => filter.workspaces.clear(),
            MenuKind::Provider => filter.providers.clear(),
            MenuKind::Model => filter.models.clear(),
            MenuKind::Range | MenuKind::Export => return,
        }
        self.set_filter(filter, cx);
    }

    pub fn clear_filters(&mut self, cx: &mut Context<Self>) {
        self.set_filter(self.filter.cleared(), cx);
    }

    pub fn set_session_scope(&mut self, session: Option<u16>, cx: &mut Context<Self>) {
        let mut filter = self.filter.clone();
        filter.session = session;
        self.set_filter(filter, cx);
    }

    pub fn set_errors_only(&mut self, only: bool, cx: &mut Context<Self>) {
        let mut filter = self.filter.clone();
        filter.errors_only = only;
        if only {
            filter.cached_only = false;
        }
        self.set_filter(filter, cx);
    }

    pub fn set_cached_only(&mut self, only: bool, cx: &mut Context<Self>) {
        let mut filter = self.filter.clone();
        filter.cached_only = only;
        if only {
            filter.errors_only = false;
        }
        self.set_filter(filter, cx);
    }

    /// The table delegate knows the exact ordering it was asked for (its header
    /// cycles through three states), so it sets both key and direction instead
    /// of toggling.
    pub fn set_session_sort_from_table(
        &mut self,
        sort: SessionSort,
        desc: bool,
        cx: &mut Context<Self>,
    ) {
        if self.session_sort == sort && self.session_sort_desc == desc {
            return;
        }
        self.session_sort = sort;
        self.session_sort_desc = desc;
        self.tables_dirty = true;
        self.persist();
        cx.notify();
    }

    /// Same for the breakdown table.
    pub fn set_bucket_sort_from_table(
        &mut self,
        sort: BucketSort,
        desc: bool,
        cx: &mut Context<Self>,
    ) {
        if self.bucket_sort == sort && self.bucket_sort_desc == desc {
            return;
        }
        self.bucket_sort = sort;
        self.bucket_sort_desc = desc;
        self.tables_dirty = true;
        cx.notify();
    }

     /// Open a session in the chat surface.
    pub fn open_session(&mut self, window: &mut Window, cx: &mut Context<Self>, session: u16) {
        let Some(index) = &self.index else {
            return;
        };
        let id = index.session(session).id.clone();
        if let Some(handler) = self.on_open_session.clone() {
            handler(&id, window, cx);
        }
    }

    pub fn close_page(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(handler) = self.on_close.clone() {
            handler(window, cx);
        }
    }

    /// Write the filtered data to disk. Returns the chosen path, if any.
    pub fn export(&mut self, format: ExportFormat, cx: &mut Context<Self>) -> Option<PathBuf> {
        let (index, snapshot) = (self.index.clone()?, self.snapshot.clone()?);
        let default_name = match format {
            ExportFormat::Csv => "orbit-usage.csv",
            ExportFormat::Json => "orbit-usage.json",
        };
        let path = rfd::FileDialog::new()
            .set_file_name(default_name)
            .save_file()?;
        let body = match format {
            ExportFormat::Csv => super::view::export_csv(&index, &self.filter),
            ExportFormat::Json => {
                super::view::export_json(&index, &snapshot, &self.search.read(cx).text())
            }
        };
        let result = std::fs::write(&path, body);
        self.status = Some(match result {
            Ok(()) => (
                format!(
                    "Exported {} to {}",
                    match format {
                        ExportFormat::Csv => "filtered requests",
                        ExportFormat::Json => "the current view",
                    },
                    path.file_name()
                        .map(|name| name.to_string_lossy().to_string())
                        .unwrap_or_else(|| path.to_string_lossy().to_string())
                ),
                Instant::now(),
            ),
            Err(err) => (format!("Export failed: {err}"), Instant::now()),
        });
        cx.notify();
        Some(path)
    }

    fn persist(&self) {
        Prefs {
            preset: self.filter.range.preset.as_str().to_string(),
            custom_start: self.custom_start,
            custom_end: self.custom_end,
            metric: self.metric.as_str().to_string(),
            session_sort: self.session_sort.as_str().to_string(),
            session_sort_desc: self.session_sort_desc,
        }
        .persist();
    }
}

/// Persisted view preferences (§61). Data-derived state is never persisted;
/// neither is anything transient.
#[derive(Default)]
struct Prefs {
    preset: String,
    custom_start: Option<i64>,
    custom_end: Option<i64>,
    metric: String,
    session_sort: String,
    session_sort_desc: bool,
}

impl Prefs {
    fn path() -> PathBuf {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".orbit-pi")
            .join("usage.json")
    }

    fn load() -> Self {
        let Ok(raw) = std::fs::read_to_string(Self::path()) else {
            return Self {
                session_sort: SessionSort::Tokens.as_str().to_string(),
                session_sort_desc: true,
                metric: ChartMetric::Tokens.as_str().to_string(),
                ..Default::default()
            };
        };
        let Ok(value) = serde_json::from_str::<Value>(&raw) else {
            return Self::default();
        };
        let text = |key: &str| value.get(key).and_then(Value::as_str).map(str::to_string);
        Self {
            preset: text("preset").unwrap_or_default(),
            custom_start: value.get("custom_start").and_then(Value::as_i64),
            custom_end: value.get("custom_end").and_then(Value::as_i64),
            metric: text("metric").unwrap_or_else(|| ChartMetric::Tokens.as_str().to_string()),
            session_sort: text("session_sort")
                .unwrap_or_else(|| SessionSort::Tokens.as_str().to_string()),
            session_sort_desc: value
                .get("session_sort_desc")
                .and_then(Value::as_bool)
                .unwrap_or(true),
        }
    }

    fn persist(self) {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let payload = serde_json::json!({
            "preset": self.preset,
            "custom_start": self.custom_start,
            "custom_end": self.custom_end,
            "metric": self.metric,
            "session_sort": self.session_sort,
            "session_sort_desc": self.session_sort_desc,
        });
        let _ = std::fs::write(path, payload.to_string());
    }
}
