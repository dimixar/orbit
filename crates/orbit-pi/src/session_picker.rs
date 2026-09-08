//! Session switcher palette — a Zed-style picker for jumping between
//! sessions, opened from the sidebar's Search row.
//!
//! Same anatomy and conventions as the model selector (`model_selector.rs`,
//! which itself follows Zed's `popup_menu` + `picker`):
//!
//! - fixed-width opaque raised surface, 1px strong border, layered shadow,
//!   painted via `deferred` + `anchored` so it floats above the sidebar
//! - a filter field on top (`ComposerInput` with the `Composer Picker` key
//!   context, so backspace/paste work and the picker's enter/escape/arrows
//!   win at the same dispatch depth)
//! - two-line rows: session title, then `workspace · age`
//! - keyboard navigation; Enter opens the highlighted session
//! - the active session's icon and title are emphasized

use std::path::PathBuf;

use gpui::{
    div, point, prelude::*, px, App, Context, ElementId, Entity, FocusHandle, Focusable,
    IntoElement, MouseDownEvent, ParentElement, Render, ScrollHandle, SharedString, Styled, Window,
};

use crate::app::icon;
use crate::composer::ComposerInput;
use crate::sessions::SessionInfo;
use crate::theme::{self, Theme};

/// Popup width — room for a truncated title plus a trailing age label.
const POPOVER_W: f32 = 280.;
/// Uniform row height for the two-line session rows.
const ROW_H: f32 = 40.;
/// Air between rows (kept out of ROW_H so hit targets stay even).
const ROW_GAP: f32 = 2.;
/// Vertical stride used for keyboard scroll math.
const ROW_STRIDE: f32 = ROW_H + ROW_GAP;
/// Largest list height before it scrolls (≈ 6 visible rows).
const LIST_MAX_H: f32 = 6. * ROW_STRIDE;

/// How many sessions the palette shows at most — the newest activity wins
/// (`load_sessions` already sorts), so truncation drops the oldest.
const MAX_ROWS: usize = 50;

/// A filterable, keyboard-navigable list of sessions. Created by `OrbitApp`
/// when the Search row is clicked; it talks back exclusively through the
/// callbacks it was built with.
pub struct SessionPicker {
    sessions: Vec<SessionInfo>,
    /// The open session's path, so its row can be emphasized.
    active_path: Option<PathBuf>,
    filter: Entity<ComposerInput>,
    list_scroll: ScrollHandle,
    highlighted: usize,
    last_filter: String,
    on_open: Box<dyn Fn(SessionInfo, &mut Window, &mut App)>,
    /// `bool` = dismissed by an outside mouse-down (vs. escape).
    on_dismiss: Box<dyn Fn(bool, &mut Window, &mut App)>,
}

impl SessionPicker {
    pub fn new(
        sessions: Vec<SessionInfo>,
        active_path: Option<PathBuf>,
        on_open: Box<dyn Fn(SessionInfo, &mut Window, &mut App)>,
        on_dismiss: Box<dyn Fn(bool, &mut Window, &mut App)>,
        cx: &mut Context<Self>,
    ) -> Self {
        let filter = cx.new(|cx| {
            ComposerInput::new(cx)
                .with_placeholder("Search sessions…")
                .with_key_context("Composer Picker")
        });
        Self {
            sessions,
            active_path,
            filter,
            list_scroll: ScrollHandle::new(),
            highlighted: 0,
            last_filter: String::new(),
            on_open,
            on_dismiss,
        }
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

    fn on_outside_down(&mut self, _: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        (self.on_dismiss)(true, window, cx);
    }

    // ── internals ─────────────────────────────────────────────────────────

    /// Sessions matching the filter (title or workspace), newest first —
    /// `load_sessions` already ordered by last activity.
    fn rows(&self, needle: &str) -> Vec<SessionInfo> {
        let matches = |text: &str| needle.is_empty() || text.to_lowercase().contains(needle);
        self.sessions
            .iter()
            .filter(|s| {
                matches(&s.title)
                    || matches(&crate::sessions::workspace_label(&s.cwd))
                    || matches(&s.cwd.to_string_lossy())
            })
            .take(MAX_ROWS)
            .cloned()
            .collect()
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

    fn activate(
        &mut self,
        ix: usize,
        rows: &[SessionInfo],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(session) = rows.get(ix).cloned() {
            (self.on_open)(session, window, cx);
        }
    }
}

impl Focusable for SessionPicker {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.filter.read(cx).focus_handle(cx)
    }
}

impl Render for SessionPicker {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let needle = self.filter.read(cx).text().to_lowercase();
        if needle != self.last_filter {
            self.last_filter = needle.clone();
            self.highlighted = 0;
            self.list_scroll.set_offset(point(px(0.), px(0.)));
        }
        let rows = self.rows(&needle);
        if !rows.is_empty() {
            self.highlighted = self.highlighted.min(rows.len() - 1);
        }

        let this = cx.entity();
        let theme = *theme::get(cx);

        let mut list = div()
            .id("session-picker-list")
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
            list = list.child(render_session_row(
                &rows,
                ix,
                ix == self.highlighted,
                &self.active_path,
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
                div()
                    .h(px(64.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(theme.ui_px(12.))
                    .text_color(theme.text_3)
                    .child("No sessions match")
                    .into_any_element()
            } else {
                list.into_any_element()
            })
    }
}

fn render_session_row(
    rows: &[SessionInfo],
    ix: usize,
    highlighted: bool,
    active_path: &Option<PathBuf>,
    this: &Entity<SessionPicker>,
    theme: Theme,
) -> impl IntoElement + use<> {
    let session = rows[ix].clone();
    let active = active_path.as_ref() == Some(&session.path);
    let this = this.clone();
    div()
        .id(ElementId::NamedInteger("session-row".into(), ix as u64))
        .h(px(ROW_H))
        .px(px(8.))
        .rounded(px(6.))
        .cursor_pointer()
        .flex()
        .flex_col()
        .justify_center()
        .gap(px(1.))
        // Hover moves the keyboard highlight; click activates the row.
        .on_hover({
            let this = this.clone();
            move |hovering, _, cx| {
                if *hovering {
                    this.update(cx, |picker, cx| {
                        if picker.highlighted != ix {
                            picker.highlighted = ix;
                            cx.notify();
                        }
                    });
                }
            }
        })
        .on_click({
            let this = this.clone();
            move |_, window, cx| {
                this.update(cx, |picker, cx| {
                    let rows = picker.rows(&picker.last_filter);
                    picker.activate(ix, &rows, window, cx);
                });
            }
        })
        .when(highlighted, |row| row.bg(theme.overlay))
        // Line 1 — leading conversation icon, then title; the icon tints on
        // the open row so the current conversation reads at a glance.
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .child(icon(
                    "icons/chat.svg",
                    13.,
                    if active { theme.accent } else { theme.text_3 },
                ))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_size(theme.ui_px(12.5))
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(if active { theme.text } else { theme.text_2 })
                        .child(session.title.clone()),
                ),
        )
        // Line 2 — workspace · age.
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(4.))
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_size(theme.ui_px(11.))
                        .text_color(theme.text_3)
                        .child(SharedString::from(crate::sessions::workspace_label(
                            &session.cwd,
                        ))),
                )
                .child(
                    div()
                        .flex_none()
                        .text_size(theme.ui_px(11.))
                        .text_color(theme.text_3)
                        .child("·"),
                )
                .child(
                    div()
                        .flex_none()
                        .text_size(theme.ui_px(11.))
                        .text_color(theme.text_3)
                        .child(SharedString::from(crate::sessions::relative_time(
                            session.modified,
                        ))),
                ),
        )
}
