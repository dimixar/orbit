//! Right side pane — the workbench's second column (Waku parity): a
//! tabbed panel with **Review** (git diff of the workspace), **Terminal**
//! (a simple shell runner) and **Browser** (reader-mode page fetch, see
//! `reader.rs`). Closing the pane keeps the last tab; reopening resumes
//! it. With no tab chosen the pane shows the "Open tab" card grid.
//!
//! The pane is a GPUI [`Entity`] owned by [`crate::app::OrbitApp`], so it
//! can hold its own composer inputs, scroll handles and background-job
//! state. `Submit` (Enter) dispatched from a pane input is caught here —
//! the action bubbles from the focused input and this pane's root handles
//! it before the app root's prompt-submit does.

use std::path::{Path, PathBuf};

use gpui::{
    div, point, prelude::*, px, radians, Animation, AnimationExt, AnyElement, ClickEvent, Context,
    CursorStyle, Entity, Focusable, FontWeight, Hsla, Pixels, Render, ScrollHandle, SharedString,
    TextAlign, Transformation, Window,
};

use crate::app::icon;
use crate::composer::ComposerInput;
use crate::git;
use crate::reader::{self, ReaderPage};
use crate::theme::{self, Theme};
use crate::Submit;

/// Pane width defaults / drag clamps.
const PANE_DEFAULT_W: f32 = 400.;
const PANE_MIN_W: f32 = 300.;
/// Terminal buffer cap (lines) — older lines are dropped.
const TERMINAL_MAX_LINES: usize = 800;

/// Drag marker for the side-pane resize handle (gpui typed drag state).
pub struct SidePaneResize;

/// The three tabs the pane can host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidePaneTab {
    Review,
    Terminal,
    Browser,
}

impl SidePaneTab {
    fn label(self) -> &'static str {
        match self {
            SidePaneTab::Review => "Review",
            SidePaneTab::Terminal => "Terminal",
            SidePaneTab::Browser => "Browser",
        }
    }

    fn icon(self) -> &'static str {
        match self {
            SidePaneTab::Review => "icons/file-diff.svg",
            SidePaneTab::Terminal => "icons/terminal.svg",
            SidePaneTab::Browser => "icons/globe.svg",
        }
    }

    const ALL: [SidePaneTab; 3] = [
        SidePaneTab::Review,
        SidePaneTab::Terminal,
        SidePaneTab::Browser,
    ];
}

/// One line of terminal output.
#[derive(Debug, Clone)]
pub struct TermLine {
    kind: TermKind,
    text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TermKind {
    /// The echoed command (accent prompt glyph).
    Command,
    /// stdout.
    Output,
    /// stderr (red).
    Error,
    /// Meta notes (exit codes, cwd).
    Note,
}

/// Classification of one unified-diff line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DiffKind {
    /// `diff --git`, `index`, `---`, `+++` … (never painted — the file
    /// header row already carries the path).
    Meta,
    /// `@@ … @@` hunk headers (painted as Waku-style separators).
    Hunk,
    Added,
    Removed,
    Context,
    /// `\ No newline at end of file`, untracked-file notes.
    Note,
}

/// One unified-diff line with its old/new line numbers resolved from the
/// hunk headers (Waku parity: numbered diff rows).
#[derive(Debug, Clone)]
pub struct DiffLine {
    kind: DiffKind,
    text: String,
    old_no: Option<u32>,
    new_no: Option<u32>,
}

impl DiffLine {
    fn plain(kind: DiffKind, text: impl Into<String>) -> Self {
        Self {
            kind,
            text: text.into(),
            old_no: None,
            new_no: None,
        }
    }
}

/// One file's diff, pre-parsed for rendering.
#[derive(Debug, Clone)]
pub struct DiffFile {
    path: String,
    added: u64,
    removed: u64,
    /// Binary files carry a note instead of line hunks.
    binary: bool,
    lines: Vec<DiffLine>,
    /// Hunk cursor while parsing: current (old, new) line numbers.
    hunk: HunkCursor,
}

impl DiffFile {
    /// An untracked file: listed, with no diff content.
    fn untracked(path: String) -> Self {
        Self {
            path,
            added: 0,
            removed: 0,
            binary: false,
            lines: vec![DiffLine::plain(DiffKind::Note, "untracked file")],
            hunk: None,
        }
    }
}

/// Parsed `git diff HEAD` + untracked files, cached until stale.
#[derive(Debug, Clone, Default)]
pub struct ReviewData {
    files: Vec<DiffFile>,
    added: u64,
    removed: u64,
    error: Option<String>,
}

pub struct SidePane {
    /// Whether the pane is shown at all (toggled from the top bar).
    open: bool,
    /// The selected tab; `None` shows the "Open tab" card grid.
    tab: Option<SidePaneTab>,
    /// Pane width in pixels — adjusted by dragging its left edge.
    width: Pixels,

    /// Workspace the pane operates on (kept in sync by the app).
    workspace: Option<PathBuf>,

    // ── Review ──
    review: Option<ReviewData>,
    review_loading: bool,
    /// Set when a run settles (or the workspace changes); the diff reloads
    /// next time the Review tab is visible.
    review_stale: bool,
    review_scroll: ScrollHandle,

    // ── Terminal ──
    terminal_input: Entity<ComposerInput>,
    terminal_lines: Vec<TermLine>,
    terminal_running: bool,
    terminal_scroll: ScrollHandle,

    // ── Browser ──
    url_input: Entity<ComposerInput>,
    page: Option<ReaderPage>,
    browser_loading: bool,
    browser_error: Option<String>,
    browser_scroll: ScrollHandle,
}

impl SidePane {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let terminal_input = cx.new(|cx| {
            ComposerInput::new(cx)
                .with_placeholder("Run a command…")
                .with_key_context("Composer SidePane")
        });
        let url_input = cx.new(|cx| {
            ComposerInput::new(cx)
                .with_placeholder("Search or enter URL")
                .with_key_context("Composer SidePane")
        });
        Self {
            open: false,
            tab: None,
            width: px(PANE_DEFAULT_W),
            workspace: None,
            review: None,
            review_loading: false,
            review_stale: true,
            review_scroll: ScrollHandle::new(),
            terminal_input,
            terminal_lines: Vec::new(),
            terminal_running: false,
            terminal_scroll: ScrollHandle::new(),
            url_input,
            page: None,
            browser_loading: false,
            browser_error: None,
            browser_scroll: ScrollHandle::new(),
        }
    }

    // ── state entry points (called from the app shell) ─────────────────

    /// Show/hide the pane (top-bar toggle). Keeps the selected tab.
    pub fn toggle(&mut self, cx: &mut Context<Self>) {
        self.open = !self.open;
        cx.notify();
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn width(&self) -> Pixels {
        self.width
    }

    /// Drag-resize from the pane's left edge.
    pub fn set_width(&mut self, width: Pixels, cx: &mut Context<Self>) {
        let clamped = width.max(px(PANE_MIN_W));
        if clamped != self.width {
            self.width = clamped;
            cx.notify();
        }
    }

    /// Keep the pane's workspace in sync with the app (called every frame;
    /// cheap no-op when unchanged). A workspace change invalidates Review.
    pub fn set_workspace(&mut self, workspace: Option<PathBuf>, cx: &mut Context<Self>) {
        if workspace != self.workspace {
            self.workspace = workspace;
            self.review_stale = true;
            if self.open && self.tab == Some(SidePaneTab::Review) {
                self.load_review(cx);
            }
            cx.notify();
        }
    }

    /// A run settled — Review is stale; reload immediately when visible.
    pub fn mark_review_stale(&mut self, cx: &mut Context<Self>) {
        self.review_stale = true;
        if self.open && self.tab == Some(SidePaneTab::Review) && !self.review_loading {
            self.load_review(cx);
        }
    }

    // ── tab / interactions ─────────────────────────────────────────────

    fn select_tab(&mut self, tab: SidePaneTab, window: &mut Window, cx: &mut Context<Self>) {
        self.open = true;
        if self.tab == Some(tab) {
            return;
        }
        self.tab = Some(tab);
        match tab {
            SidePaneTab::Review => {
                if self.review_stale && !self.review_loading {
                    self.load_review(cx);
                }
            }
            SidePaneTab::Terminal => {
                self.terminal_input.update(cx, |input, _cx| input.focus(window));
            }
            SidePaneTab::Browser => {
                self.url_input.update(cx, |input, _cx| input.focus(window));
            }
        }
        cx.notify();
    }

    fn close(&mut self, cx: &mut Context<Self>) {
        self.open = false;
        cx.notify();
    }

    /// Open the pane straight onto the Review tab — wired to the
    /// transcript's changed-files cards' "Review" buttons (Waku parity).
    pub fn show_review(&mut self, cx: &mut Context<Self>) {
        self.open = true;
        self.tab = Some(SidePaneTab::Review);
        if self.review_stale && !self.review_loading {
            self.load_review(cx);
        }
        cx.notify();
    }

    // ── Review ─────────────────────────────────────────────────────────

    /// Load the workspace's git diff off-thread.
    fn load_review(&mut self, cx: &mut Context<Self>) {
        let cwd = self
            .workspace
            .clone()
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_default();
        self.review_loading = true;
        self.review_stale = false;
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { git_diff(&cwd) })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.review_loading = false;
                this.review = Some(result);
                this.review_scroll.set_offset(point(px(0.), px(0.)));
                cx.notify();
            });
        })
        .detach();
    }

    fn refresh_review(&mut self, cx: &mut Context<Self>) {
        if !self.review_loading {
            self.load_review(cx);
        }
    }

    // ── Terminal ───────────────────────────────────────────────────────

    fn push_terminal(&mut self, lines: Vec<TermLine>, cx: &mut Context<Self>) {
        if lines.is_empty() {
            return;
        }
        self.terminal_lines.extend(lines);
        let overflow = self.terminal_lines.len().saturating_sub(TERMINAL_MAX_LINES);
        if overflow > 0 {
            self.terminal_lines.drain(0..overflow);
        }
        self.terminal_scroll.scroll_to_bottom();
        cx.notify();
    }

    /// Run the composer's current text as a shell command in the workspace.
    fn run_command(&mut self, cx: &mut Context<Self>) {
        if self.terminal_running {
            return;
        }
        let command = self.terminal_input.read(cx).text().trim().to_string();
        if command.is_empty() {
            return;
        }
        self.terminal_input.update(cx, |input, cx| input.clear(cx));
        self.push_terminal(
            vec![TermLine {
                kind: TermKind::Command,
                text: command.clone(),
            }],
            cx,
        );
        // Built-in: clear the buffer (it would otherwise echo nothing).
        if command == "clear" || command == "cls" {
            self.terminal_lines.clear();
            cx.notify();
            return;
        }
        let cwd = self
            .workspace
            .clone()
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_default();
        self.terminal_running = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let lines = cx
                .background_executor()
                .spawn(async move { run_shell(&cwd, &command) })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.terminal_running = false;
                this.push_terminal(lines, cx);
            });
        })
        .detach();
    }

    // ── Browser ────────────────────────────────────────────────────────

    /// Fetch the URL composer's text in reader mode, off-thread.
    fn open_url(&mut self, cx: &mut Context<Self>) {
        if self.browser_loading {
            return;
        }
        let raw = self.url_input.read(cx).text().trim().to_string();
        if raw.is_empty() {
            return;
        }
        self.url_input.update(cx, |input, cx| input.clear(cx));
        self.browser_loading = true;
        self.browser_error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { reader::read_page(&raw) })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.browser_loading = false;
                match result {
                    Ok(page) => {
                        this.page = Some(page);
                        this.browser_scroll.set_offset(point(px(0.), px(0.)));
                    }
                    Err(err) => this.browser_error = Some(err),
                }
                cx.notify();
            });
        })
        .detach();
    }

    // ── keyboard ───────────────────────────────────────────────────────

    /// Enter from a pane input: terminal runs its command, browser opens
    /// its URL. Which input owns focus decides.
    fn on_pane_submit(
        &mut self,
        _: &Submit,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self
            .terminal_input
            .read(cx)
            .focus_handle(cx)
            .is_focused(window)
        {
            self.run_command(cx);
        } else if self.url_input.read(cx).focus_handle(cx).is_focused(window) {
            self.open_url(cx);
        }
    }

    // ── rendering ──────────────────────────────────────────────────────

    fn tab_button(
        &self,
        tab: SidePaneTab,
        selected: bool,
        theme: Theme,
        cx: &Context<Self>,
    ) -> impl IntoElement + use<> {
        let id = SharedString::from(format!("pane-tab-{:?}", tab).to_lowercase());
        div()
            .id(id)
            .flex()
            .items_center()
            .gap_1p5()
            .px(px(8.))
            .h(px(26.))
            .rounded_md()
            .text_size(theme.ui_px(12.))
            .font_weight(FontWeight::MEDIUM)
            .when(selected, |b| {
                b.bg(theme.active).text_color(theme.active_fg)
            })
            .when(!selected, |b| {
                b.text_color(theme.text_2)
                    .cursor_pointer()
                    .hover(|s| s.bg(theme.bg_hover))
            })
            .child(icon(
                tab.icon(),
                13.,
                if selected { theme.active_fg } else { theme.text_2 },
            ))
            .child(tab.label())
            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                this.select_tab(tab, window, cx);
            }))
    }

    fn header(&self, theme: Theme, cx: &Context<Self>) -> impl IntoElement + use<> {
        let mut header = div()
            .h(px(42.))
            .flex()
            .items_center()
            .gap_1()
            .pl(px(8.))
            .pr(px(6.))
            .border_b_1()
            .border_color(theme.border);
        for tab in SidePaneTab::ALL {
            header = header.child(self.tab_button(tab, Some(tab) == self.tab, theme, cx));
        }
        header = header
            .child(div().flex_1())
            .child(
                div()
                    .id("pane-close")
                    .p_1()
                    .rounded_sm()
                    .cursor_pointer()
                    .hover(|s| s.bg(theme.bg_hover))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close(cx)))
                    .child(icon("icons/x.svg", 14., theme.text_3)),
            );
        header
    }

    /// The "Open tab" card grid — the pane's picker state (screenshot
    /// parity: heading, hint line, three tab cards).
    fn empty_state(&self, theme: Theme, cx: &Context<Self>) -> impl IntoElement + use<> {
        let mut cards = div().flex().gap_2().mt_1();
        for tab in SidePaneTab::ALL {
            cards = cards.child(
                div()
                    .id(SharedString::from(format!("pane-card-{:?}", tab).to_lowercase()))
                    .w(px(116.))
                    .h(px(84.))
                    .rounded_lg()
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.bg_raised)
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap_2()
                    .cursor_pointer()
                    .hover(|s| s.bg(theme.bg_hover))
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        this.select_tab(tab, window, cx);
                    }))
                    .child(icon(tab.icon(), 16., theme.text_2))
                    .child(
                        div()
                            .text_size(theme.ui_px(12.5))
                            .font_weight(FontWeight::MEDIUM)
                            .child(tab.label()),
                    ),
            );
        }
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_2()
            .child(
                div()
                    .text_size(theme.ui_px(17.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.text)
                    .child("Open tab"),
            )
            .child(
                div()
                    .text_size(theme.ui_px(12.5))
                    .text_color(theme.text_3)
                    .child("Choose a tab to open in the side pane."),
            )
            .child(cards)
    }

    fn body(&mut self, theme: Theme, cx: &mut Context<Self>) -> AnyElement {
        match self.tab {
            None => self.empty_state(theme, cx).into_any_element(),
            Some(SidePaneTab::Review) => self.review_body(theme, cx),
            Some(SidePaneTab::Terminal) => self.terminal_body(theme, cx),
            Some(SidePaneTab::Browser) => self.browser_body(theme, cx),
        }
    }

    // ── Review body ──

    fn review_body(&mut self, theme: Theme, cx: &mut Context<Self>) -> AnyElement {
        let (added, removed) = self
            .review
            .as_ref()
            .map(|r| (r.added, r.removed))
            .unwrap_or((0, 0));
        // Header row: title + refresh + live ±stats.
        let head = div()
            .h(px(36.))
            .flex()
            .items_center()
            .gap_2()
            .px(px(12.))
            .border_b_1()
            .border_color(theme.border)
            .child(
                div()
                    .text_size(theme.ui_px(12.5))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.text)
                    .child("Review"),
            )
            .child(div().flex_1())
            .children((added > 0 || removed > 0).then(|| {
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_size(theme.ui_px(11.5))
                    .child(
                        div()
                            .text_color(theme.add_green)
                            .child(format!("+{added}")),
                    )
                    .child(
                        div()
                            .text_color(theme.del_red)
                            .child(format!("-{removed}")),
                    )
            }))
            .child(if self.review_loading {
                spinner("review-spinner", theme).into_any_element()
            } else {
                div()
                    .id("review-refresh")
                    .p_1()
                    .rounded_sm()
                    .cursor_pointer()
                    .hover(|s| s.bg(theme.bg_hover))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.refresh_review(cx);
                    }))
                    .child(icon("icons/refresh.svg", 13., theme.text_3))
                    .into_any_element()
            });

        let list: AnyElement = if self.review_loading && self.review.is_none() {
            centered_note(theme, "Loading diff…").into_any_element()
        } else if let Some(review) = &self.review {
            if let Some(error) = &review.error {
                centered_note(theme, &friendly_git_error(error)).into_any_element()
            } else if review.files.is_empty() {
                centered_note(theme, "No local changes").into_any_element()
            } else {
                let files = review.files.clone();
                div()
                    .id("review-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.review_scroll)
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p(px(10.))
                    .children(files.into_iter().map(|file| render_diff_file(file, theme)))
                    .into_any_element()
            }
        } else {
            centered_note(theme, "No local changes").into_any_element()
        };

        div().flex_1().min_h_0().flex().flex_col().child(head).child(list).into_any_element()
    }

    // ── Terminal body ──

    fn terminal_body(&mut self, theme: Theme, _cx: &mut Context<Self>) -> AnyElement {
        let mono = theme::code_font_family();
        let lines = self.terminal_lines.clone();
        let mut out = div()
            .id("terminal-scroll")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .track_scroll(&self.terminal_scroll)
            .flex()
            .flex_col()
            .px(px(12.))
            .py(px(10.))
            .font_family(mono.clone())
            .text_size(theme.ui_px(11.5))
            .line_height(theme.ui_px(17.));
        for line in lines {
            out = out.child(render_term_line(line, theme));
        }
        let cwd = self
            .workspace
            .as_deref()
            .map(workspace_label)
            .unwrap_or_else(|| "~".into());

        // Input row: prompt glyph + one-line composer + optional spinner.
        let mut input_row = div()
            .flex()
            .items_center()
            .gap_2()
            .px(px(12.))
            .py(px(8.))
            .border_t_1()
            .border_color(theme.border)
            .child(
                div()
                    .font_family(mono)
                    .text_size(theme.ui_px(12.))
                    .text_color(theme.accent)
                    .child("❯"),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .bg(theme.bg_composer)
                    .border_1()
                    .border_color(theme.border)
                    .rounded_md()
                    .px(px(8.))
                    .py(px(5.))
                    .child(self.terminal_input.clone()),
            );
        if self.terminal_running {
            input_row = input_row.child(spinner("terminal-spinner", theme));
        }

        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            // cwd hint under the pane header
            .child(
                div()
                    .px(px(12.))
                    .pt(px(6.))
                    .text_size(theme.ui_px(11.))
                    .text_color(theme.text_3)
                    .font_family(theme::code_font_family())
                    .child(cwd),
            )
            .child(out)
            .child(input_row)
            .into_any_element()
    }

    // ── Browser body ──

    fn browser_body(&mut self, theme: Theme, cx: &mut Context<Self>) -> AnyElement {
        // URL row: composer + go button.
        let url_row = div()
            .flex()
            .items_center()
            .gap_2()
            .px(px(12.))
            .py(px(8.))
            .border_b_1()
            .border_color(theme.border)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .bg(theme.bg_composer)
                    .border_1()
                    .border_color(theme.border)
                    .rounded_md()
                    .px(px(8.))
                    .py(px(5.))
                    .child(self.url_input.clone()),
            )
            .child(if self.browser_loading {
                spinner("browser-spinner", theme).into_any_element()
            } else {
                div()
                    .id("browser-go")
                    .p_1()
                    .rounded_sm()
                    .cursor_pointer()
                    .hover(|s| s.bg(theme.bg_hover))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.open_url(cx)))
                    .child(icon("icons/arrow-right.svg", 14., theme.text_2))
                    .into_any_element()
            });

        let content: AnyElement = if self.browser_loading && self.page.is_none() {
            centered_note(theme, "Loading…").into_any_element()
        } else if let Some(error) = &self.browser_error {
            centered_note(theme, error).into_any_element()
        } else if let Some(page) = self.page.as_ref() {
            let title = page.title.clone();
            let lines = page.lines.clone();
            let mut body = div()
                .id("browser-scroll")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .track_scroll(&self.browser_scroll)
                .px(px(14.))
                .py(px(12.))
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_size(theme.ui_px(15.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.text)
                        .pb_1()
                        .child(title),
                )
                // Source URL, so a reader pass is never anonymous.
                .child(
                    div()
                        .font_family(theme::code_font_family())
                        .text_size(theme.ui_px(10.5))
                        .text_color(theme.text_3)
                        .pb_1()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .child(page.url.clone()),
                );
            for line in lines {
                if line.is_empty() {
                    continue;
                }
                body = body.child(
                    div()
                        .text_size(theme.ui_px(12.5))
                        .line_height(theme.ui_px(19.))
                        .text_color(theme.text_2)
                        .child(line),
                );
            }
            body.into_any_element()
        } else {
            centered_note(theme, "Enter a URL to read it here.").into_any_element()
        };

        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .child(url_row)
            .child(content)
            .into_any_element()
    }
}

impl Render for SidePane {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.open {
            return div().into_any_element();
        }
        let theme = *theme::get(cx);
        div()
            .id("side-pane")
            .relative()
            .w(self.width)
            .h_full()
            .bg(theme.bg_main)
            .border_l_1()
            .border_color(theme.border)
            .flex()
            .flex_col()
            .min_h_0()
            // Enter from a pane input is handled here, before the app
            // root's prompt-submit sees it.
            .on_action(cx.listener(Self::on_pane_submit))
            // Resize handle: drag the pane's left edge. The drag-move
            // listener lives on the app root so the drag tracks beyond it.
            .child(
                div()
                    .id("side-pane-resize-handle")
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left(px(-3.))
                    .w(px(6.))
                    .cursor(CursorStyle::ResizeLeftRight)
                    .hover(|style| style.bg(theme.accent.opacity(0.4)))
                    .on_drag(SidePaneResize, |_, _, _, cx| cx.new(|_| DragGhost)),
            )
            .child(self.header(theme, cx))
            .child(self.body(theme, cx))
            .into_any_element()
    }
}

/// An invisible drag ghost — resizing leaves no floating preview.
struct DragGhost;

impl Render for DragGhost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

// ── shared render helpers ──────────────────────────────────────────────────

fn spinner(id: &'static str, theme: Theme) -> impl IntoElement + use<> {
    gpui::svg()
        .path("icons/loader.svg")
        .size(px(13.))
        .text_color(theme.text_3)
        .with_animation(
            id,
            Animation::new(std::time::Duration::from_millis(900)).repeat(),
            |svg, delta| {
                svg.with_transformation(Transformation::rotate(radians(
                    delta * std::f32::consts::TAU,
                )))
            },
        )
}

/// A centered dim note filling the pane body (loading / empty / errors).
fn centered_note(theme: Theme, text: &str) -> impl IntoElement + use<> {
    div()
        .flex_1()
        .min_h_0()
        .flex()
        .items_center()
        .justify_center()
        .px(px(16.))
        .child(
            div()
                .text_size(theme.ui_px(12.5))
                .text_color(theme.text_3)
                .text_align(TextAlign::Center)
                .child(text.to_string()),
        )
}

fn render_term_line(line: TermLine, theme: Theme) -> impl IntoElement + use<> {
    let (color, glyph) = match line.kind {
        TermKind::Command => (theme.text, Some("❯ ".to_string())),
        TermKind::Output => (theme.text_2, None),
        TermKind::Error => (theme.del_red, None),
        TermKind::Note => (theme.text_3, None),
    };
    div()
        .flex()
        .when(line.kind == TermKind::Command, |row| {
            row.child(
                div()
                    .text_color(theme.accent)
                    .child(glyph.unwrap_or_default()),
            )
        })
        .child(
            div()
                .text_color(color)
                .child(line.text),
        )
}

fn render_diff_file(file: DiffFile, theme: Theme) -> impl IntoElement + use<> {
    let mut lines = div().w_full().min_w_0();
    for line in &file.lines {
        let Some(row) = render_diff_line(line, theme) else {
            continue;
        };
        lines = lines.child(row);
    }
    div()
        .w_full()
        .rounded_md()
        .border_1()
        .border_color(theme.border)
        .overflow_hidden()
        .child(
            // File header: mono path + ±delta.
            div()
                .w_full()
                .flex()
                .items_center()
                .gap_2()
                .px(px(8.))
                .py(px(5.))
                .bg(theme.bg_raised)
                .border_b_1()
                .border_color(theme.border)
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .font_family(theme::code_font_family())
                        .text_size(theme.ui_px(11.5))
                        .text_color(theme.text)
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .child(file.path.clone()),
                )
                .children((file.added > 0).then(|| {
                    div()
                        .text_size(theme.ui_px(11.))
                        .text_color(theme.add_green)
                        .child(format!("+{}", file.added))
                }))
                .children((file.removed > 0).then(|| {
                    div()
                        .text_size(theme.ui_px(11.))
                        .text_color(theme.del_red)
                        .child(format!("-{}", file.removed))
                })),
        )
        .child(lines)
}

/// One diff row, Waku-style: dim old/new line-number gutters, tinted row
/// background on added/removed lines. Meta rows never render (the file
/// header carries the path); returns `None` to skip.
fn render_diff_line(line: &DiffLine, theme: Theme) -> Option<AnyElement> {
    let mono = div()
        .w_full()
        .min_w_0()
        .font_family(theme::code_font_family())
        .text_size(theme.ui_px(11.))
        .line_height(theme.ui_px(16.));
    match line.kind {
        DiffKind::Meta => None,
        DiffKind::Note => Some(
            mono.text_color(theme.text_3)
                .italic()
                .px(px(8.))
                .child(line.text.clone())
                .into_any_element(),
        ),
        DiffKind::Hunk => Some(
            mono.text_color(theme.accent.opacity(0.75))
                .bg(theme.bg_raised)
                .px(px(8.))
                .overflow_hidden()
                .whitespace_nowrap()
                .child(line.text.clone())
                .into_any_element(),
        ),
        DiffKind::Added => Some(
            numbered_row(&line, theme.add_green.opacity(0.12), theme.add_green, theme)
                .into_any_element(),
        ),
        DiffKind::Removed => Some(
            numbered_row(&line, theme.del_red.opacity(0.12), theme.del_red, theme)
                .into_any_element(),
        ),
        DiffKind::Context => Some(
            numbered_row(&line, gpui::Hsla::default(), theme.text_2, theme)
                .into_any_element(),
        ),
    }
}

/// `[old №][new №][line text]` with a full-row tint (Waku diff viewer).
fn numbered_row(line: &DiffLine, bg: Hsla, color: Hsla, theme: Theme) -> impl IntoElement + use<> {
    let gutter = |no: Option<u32>| {
        div()
            .w(px(34.))
            .flex_none()
            .px(px(4.))
            .text_align(TextAlign::Right)
            .text_color(theme.text_3.opacity(0.7))
            .child(no.map(|n| n.to_string()).unwrap_or_default())
    };
    div()
        .w_full()
        .min_w_0()
        .flex()
        .font_family(theme::code_font_family())
        .text_size(theme.ui_px(11.))
        .line_height(theme.ui_px(16.))
        .when(bg != Hsla::default(), |row| row.bg(bg))
        .child(gutter(line.old_no))
        .child(gutter(line.new_no))
        .child(
            div()
                .min_w_0()
                .flex_1()
                .pr(px(8.))
                .text_color(color)
                .child(line.text.clone()),
        )
}

/// Map raw git errors to reader-friendly copy.
fn friendly_git_error(error: &str) -> String {
    if error.contains("not a git repository") {
        "Not a git repository".into()
    } else if error.contains("does not have any commits yet") {
        "No commits yet — nothing to diff against".into()
    } else {
        format!("git failed: {error}")
    }
}

fn workspace_label(path: &Path) -> String {
    crate::sessions::workspace_label(path)
}

// ── background jobs (pure functions, unit-tested) ─────────────────────────

/// Run `command` through `sh -c` in `cwd` and collect its output as
/// terminal lines. Blocking; must run on the background executor.
fn run_shell(cwd: &Path, command: &str) -> Vec<TermLine> {
    let mut lines = Vec::new();
    match std::process::Command::new("sh")
        .arg("-c")
        .arg(command)
        .current_dir(cwd)
        .output()
    {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            for text in stdout.lines() {
                lines.push(TermLine {
                    kind: TermKind::Output,
                    text: text.into(),
                });
            }
            for text in stderr.lines() {
                lines.push(TermLine {
                    kind: TermKind::Error,
                    text: text.into(),
                });
            }
            if let Some(code) = output.status.code() {
                if code != 0 {
                    lines.push(TermLine {
                        kind: TermKind::Note,
                        text: format!("exit {code}"),
                    });
                }
            } else {
                lines.push(TermLine {
                    kind: TermKind::Note,
                    text: "terminated".into(),
                });
            }
        }
        Err(err) => lines.push(TermLine {
            kind: TermKind::Error,
            text: format!("failed to run: {err}"),
        }),
    }
    if lines.len() > TERMINAL_MAX_LINES {
        lines.truncate(TERMINAL_MAX_LINES);
        lines.push(TermLine {
            kind: TermKind::Note,
            text: "… output truncated".into(),
        });
    }
    lines
}

/// `git diff HEAD --no-color` + untracked-file listing for the workspace.
fn git_diff(cwd: &Path) -> ReviewData {
    let diff = match git::run_git(cwd, &["diff", "HEAD", "--no-color"]) {
        Ok(diff) => diff,
        Err(err) => {
            return ReviewData {
                error: Some(err),
                ..Default::default()
            };
        }
    };
    let untracked: Vec<String> = match git::run_git(cwd, &["status", "--porcelain"]) {
        Ok(status) => status
            .lines()
            .filter_map(|line| line.strip_prefix("?? "))
            .map(|path| path.trim().trim_end_matches('/').to_string())
            .filter(|path| !path.is_empty())
            .collect(),
        Err(_) => Vec::new(),
    };
    let files = with_untracked(parse_diff(&diff), untracked);
    let added = files.iter().map(|f| f.added).sum();
    let removed = files.iter().map(|f| f.removed).sum();
    ReviewData {
        files,
        added,
        removed,
        error: None,
    }
}

/// Per-file hunk cursor while parsing: `Some((old, new))` line numbers,
/// cleared at file boundaries.
type HunkCursor = Option<(Option<u32>, Option<u32>)>;

/// Unified-diff meta prefixes that never render as diff content.
const META_PREFIXES: [&str; 10] = [
    "+++ ",
    "--- ",
    "index ",
    "new file",
    "deleted file",
    "old mode",
    "new mode",
    "similarity ",
    "rename ",
    "diff --git",
];

/// Parse unified diff text into per-file records, resolving each line's
/// old/new numbers from the hunk headers (Waku parity).
fn parse_diff(diff: &str) -> Vec<DiffFile> {
    let mut files: Vec<DiffFile> = Vec::new();
    let mut current: Option<DiffFile> = None;
    for line in diff.lines() {
        if let Some(rest) = line.strip_prefix("diff --git ") {
            if let Some(done) = current.take() {
                files.push(done);
            }
            // `a/<path> b/<path>` — prefer the b/ side.
            let path = rest
                .split_once(" b/")
                .map(|(_, b)| b.to_string())
                .unwrap_or_else(|| rest.to_string());
            current = Some(DiffFile {
                path,
                added: 0,
                removed: 0,
                binary: false,
                lines: Vec::new(),
                hunk: None,
            });
            continue;
        }
        let Some(file) = current.as_mut() else {
            continue;
        };
        if line.starts_with("Binary files") {
            file.binary = true;
            file.lines.push(DiffLine::plain(
                DiffKind::Note,
                "binary file — contents not shown",
            ));
            continue;
        }
        if line.starts_with("@@") {
            let (old_start, new_start) = hunk_starts(line);
            file.hunk = Some((old_start, new_start));
            file.lines.push(DiffLine::plain(DiffKind::Hunk, line));
            continue;
        }
        if line.starts_with('\\') {
            // `\ No newline at end of file`
            file.lines.push(DiffLine::plain(DiffKind::Note, line));
            continue;
        }
        if META_PREFIXES.iter().any(|p| line.starts_with(p)) {
            file.lines.push(DiffLine::plain(DiffKind::Meta, line));
            continue;
        }
        if let Some(rest) = line.strip_prefix('+') {
            file.added += 1;
            let new_no = bump(&mut file.hunk, Bump::New);
            file.lines.push(DiffLine {
                kind: DiffKind::Added,
                text: rest.into(),
                old_no: None,
                new_no,
            });
        } else if let Some(rest) = line.strip_prefix('-') {
            file.removed += 1;
            let old_no = bump(&mut file.hunk, Bump::Old);
            file.lines.push(DiffLine {
                kind: DiffKind::Removed,
                text: rest.into(),
                old_no,
                new_no: None,
            });
        } else {
            // Context line — the leading space is trimmed for display.
            let text = line.strip_prefix(' ').unwrap_or(line);
            let (old_no, new_no) = bump_both(&mut file.hunk);
            file.lines.push(DiffLine {
                kind: DiffKind::Context,
                text: text.into(),
                old_no,
                new_no,
            });
        }
    }
    if let Some(done) = current.take() {
        files.push(done);
    }
    files
}

/// What a line advances in the hunk cursor.
enum Bump {
    Old,
    New,
}

fn bump(cursor: &mut HunkCursor, which: Bump) -> Option<u32> {
    let (old, new) = cursor.unwrap_or((None, None));
    let result = match which {
        Bump::Old => old,
        Bump::New => new,
    };
    *cursor = Some(match which {
        Bump::Old => (old.map(|n| n + 1), new),
        Bump::New => (old, new.map(|n| n + 1)),
    });
    result
}

fn bump_both(cursor: &mut HunkCursor) -> (Option<u32>, Option<u32>) {
    let old = bump(cursor, Bump::Old);
    let new = bump(cursor, Bump::New);
    (old, new)
}

/// `@@ -1,4 +1,5 @@ fn main()` → `(Some(1), Some(1))`.
fn hunk_starts(line: &str) -> (Option<u32>, Option<u32>) {
    let mut old = None;
    let mut new = None;
    for token in line.split_whitespace().skip(1) {
        if old.is_none() && token.starts_with('-') {
            old = token[1..].split(',').next().and_then(|n| n.parse().ok());
        } else if new.is_none() && token.starts_with('+') {
            new = token[1..].split(',').next().and_then(|n| n.parse().ok());
        }
        if old.is_some() && new.is_some() {
            break;
        }
    }
    (old, new)
}

/// Attach untracked files (listed by `git status`) to parsed diff files.
fn with_untracked(mut files: Vec<DiffFile>, untracked: Vec<String>) -> Vec<DiffFile> {
    let tracked: Vec<String> = files.iter().map(|f| f.path.clone()).collect();
    for path in untracked {
        if !tracked.contains(&path) {
            files.push(DiffFile::untracked(path));
        }
    }
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
diff --git a/src/lib.rs b/src/lib.rs
index 83db48f..bf269f4 100644
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1,4 +1,5 @@
 fn main() {
-    println!(\"old\");
+    println!(\"new\");
+    println!(\"extra\");
 }
";

    #[test]
    fn parse_diff_splits_files_and_counts() {
        let files = parse_diff(SAMPLE);
        assert_eq!(files.len(), 1);
        let file = &files[0];
        assert_eq!(file.path, "src/lib.rs");
        assert_eq!(file.added, 2);
        assert_eq!(file.removed, 1);
        assert!(matches!(file.lines[0].kind, DiffKind::Meta));
        assert!(matches!(file.lines[3].kind, DiffKind::Hunk));
        // Context: numbers resolved from `@@ -1,4 +1,5 @@`.
        assert_eq!(
            (file.lines[4].old_no, file.lines[4].new_no),
            (Some(1), Some(1))
        );
        assert!(matches!(file.lines[5].kind, DiffKind::Removed));
        assert_eq!(file.lines[5].old_no, Some(2));
        assert_eq!(file.lines[5].new_no, None);
        assert!(matches!(file.lines[6].kind, DiffKind::Added));
        assert_eq!(file.lines[6].old_no, None);
        assert_eq!(file.lines[6].new_no, Some(2));
        // Context advances both counters — past the removal and inserts.
        assert_eq!(
            (file.lines[8].old_no, file.lines[8].new_no),
            (Some(3), Some(4))
        );
    }

    #[test]
    fn hunk_starts_parses_offsets() {
        let (old, new) = hunk_starts("@@ -10,3 +12,4 @@ fn main() {");
        assert_eq!(old, Some(10));
        assert_eq!(new, Some(12));
        let (old, new) = hunk_starts("@@ -0,0 +1 @@");
        assert_eq!(old, Some(0));
        assert_eq!(new, Some(1));
    }

    #[test]
    fn parse_diff_flags_binary() {
        let files = parse_diff("diff --git a/img.png b/img.png\nBinary files a/img.png and b/img.png differ\n");
        assert_eq!(files.len(), 1);
        assert!(files[0].binary);
    }

    #[test]
    fn untracked_files_are_appended() {
        let files = with_untracked(parse_diff(SAMPLE), vec!["notes.md".into(), "src/lib.rs".into()]);
        assert_eq!(files.len(), 2);
        assert_eq!(files[1].path, "notes.md");
    }

    #[test]
    fn run_shell_captures_stdout_stderr_and_exit() {
        let cwd = std::env::temp_dir();
        let lines = run_shell(&cwd, "echo out; echo err 1>&2; exit 3");
        assert_eq!(lines[0].kind, TermKind::Output);
        assert_eq!(lines[0].text, "out");
        assert_eq!(lines[1].kind, TermKind::Error);
        assert_eq!(lines[1].text, "err");
        let last = lines.last().unwrap();
        assert_eq!(last.kind, TermKind::Note);
        assert_eq!(last.text, "exit 3");
    }

    #[test]
    fn review_data_reports_error_off_repo() {
        let data = git_diff(Path::new("/definitely/not/a/repo/orbit"));
        assert!(data.error.is_some());
    }
}