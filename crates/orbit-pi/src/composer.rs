//! Multi-line composer input (Waku-style): text wraps, the editor grows to
//! `MAX_LINES` and then scrolls internally, `Enter` submits, `Shift+Enter`
//! inserts a newline, and ↑/↓ move the caret between visual rows.
//!
//! Built on gpui's `shape_text`/`WrappedLine` (the 0.2.2 text system caches
//! shaped layouts, so re-shaping on keystrokes is cheap). IME is still
//! approximated as plain replaces — flag for P2 polish.

use std::ops::Range;
use unicode_segmentation::UnicodeSegmentation;

use gpui::{
    div, fill, point, prelude::*, px, relative, size, App, Bounds, ClipboardEntry, ClipboardItem,
    ContentMask, Context, CursorStyle, Element, ElementInputHandler, Entity, EntityInputHandler,
    FocusHandle, Focusable, GlobalElementId, Image, InspectorElementId, LayoutId, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad, Pixels, ScrollWheelEvent,
    SharedString, Style, TextAlign, TextRun, UTF16Selection, Window, WrappedLine,
};

use crate::{
    mentions::{detect_trigger, SharedAutocomplete, Trigger},
    theme, Backspace, Copy, Cut, Delete, Down, End, Home, Left, Newline, Paste, Right, SelectAll,
    SelectLeft, SelectRight, Up,
};

/// Visual rows the editor grows to before it scrolls internally.
const MAX_LINES: usize = 8;

pub struct ComposerInput {
    focus_handle: FocusHandle,
    content: String,
    placeholder: SharedString,
    /// Element id used in `render`. Defaults to `composer-input`; form
    /// fields override it so several inputs can coexist as siblings.
    element_id: SharedString,
    /// Key context flags for this input (space separated). Defaults to
    /// `Composer`; the model picker's filter input adds a `Picker` flag so
    /// the picker's enter/escape/arrow bindings can take precedence at the
    /// same dispatch depth.
    key_context: SharedString,
    selected_range: Range<usize>,
    selection_reversed: bool,
    is_selecting: bool,
    /// Layout snapshot from the last prepaint, for hit-testing and IME
    /// bounds outside the paint pass.
    last_lines: Vec<WrappedLine>,
    /// Byte offset of each logical line's first byte (`WrappedLine::len`
    /// excludes the newline that `shape_text` split off, hence +1 steps).
    last_line_starts: Vec<usize>,
    last_line_height: Pixels,
    last_bounds: Option<Bounds<Pixels>>,
    last_wrap_width: Option<Pixels>,
    /// Visual rows across all logical lines at the last wrap width.
    last_total_rows: usize,
    /// Vertical scroll in content pixels (0 until the editor exceeds
    /// `max_lines` rows).
    scroll_offset: Pixels,
    max_lines: usize,
    /// Shared `/`-command and `@`-mention menu state. When the menu is
    /// open, ↑/↓ move the highlight instead of the caret (Enter/Escape are
    /// intercepted by the app, which owns those actions).
    autocomplete: Option<SharedAutocomplete>,
    /// Images pasted (or attached) since the app last drained them — the
    /// app turns these into message attachments.
    pub pasted_images: Vec<Image>,
}

impl ComposerInput {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx_focus_handle(_cx),
            content: String::new(),
            placeholder: "Do anything…".into(),
            element_id: "composer-input".into(),
            key_context: "Composer".into(),
            selected_range: 0..0,
            selection_reversed: false,
            is_selecting: false,
            last_lines: Vec::new(),
            last_line_starts: Vec::new(),
            last_line_height: px(18.),
            last_bounds: None,
            last_wrap_width: None,
            last_total_rows: 1,
            scroll_offset: px(0.),
            max_lines: MAX_LINES,
            autocomplete: None,
            pasted_images: Vec::new(),
        }
    }

    /// Override the placeholder text.
    pub fn with_placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// Replace the key context flags for this input (space separated).
    pub fn with_key_context(mut self, context: impl Into<SharedString>) -> Self {
        self.key_context = context.into();
        self
    }

    /// Seed the content (used by the provider editor's form fields).
    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        self.content = text.into();
        self.selected_range = self.content.len()..self.content.len();
        self
    }

    /// Override the element id so sibling inputs don't collide.
    pub fn with_element_id(mut self, id: impl Into<SharedString>) -> Self {
        self.element_id = id.into();
        self
    }

    /// Cap how many visual rows the editor grows to before scrolling.
    /// Form fields pass `1` so they stay a single line.
    pub fn with_max_lines(mut self, max_lines: usize) -> Self {
        self.max_lines = max_lines.max(1);
        self
    }

    /// Share the `/`+`@` autocomplete state (see `mentions::AutocompleteState`).
    pub fn with_autocomplete(mut self, state: SharedAutocomplete) -> Self {
        self.autocomplete = Some(state);
        self
    }

    pub fn text(&self) -> String {
        self.content.clone()
    }

    /// The active `/`-command or `@`-file trigger at the caret, if any.
    pub fn active_trigger(&self) -> Option<Trigger> {
        detect_trigger(&self.content, self.cursor_offset())
    }

    /// Whether any pasted images are waiting to be drained by the app.
    pub fn has_pasted_images(&self) -> bool {
        !self.pasted_images.is_empty()
    }

    /// Replace `range` with `text` (used by autocomplete commits).
    pub fn replace_range(&mut self, range: Range<usize>, text: &str, cx: &mut Context<Self>) {
        let start = range.start.min(self.content.len());
        let end = range.end.min(self.content.len()).max(start);
        self.content = self.content[0..start].to_owned() + text + &self.content[end..];
        self.selected_range = start + text.len()..start + text.len();
        cx.notify();
    }
    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.content.clear();
        self.selected_range = 0..0;
        self.scroll_offset = px(0.);
        cx.notify();
    }

    /// Replace the whole content and place the caret at the end. Used to seed
    /// form fields from app state (session rename, restored queue text).
    pub fn set_text(&mut self, text: impl Into<String>, cx: &mut Context<Self>) {
        self.content = text.into();
        self.selected_range = self.content.len()..self.content.len();
        self.selection_reversed = false;
        self.scroll_offset = px(0.);
        cx.notify();
    }

    /// Insert `text` at the caret (files dropped on the composer reference
    /// non-image attachments by path here).
    pub fn insert_at_caret(&mut self, text: &str, cx: &mut Context<Self>) {
        let at = self.cursor_offset();
        self.replace_range(at..at, text, cx);
    }

    pub fn focus(&self, window: &mut Window) {
        window.focus(&self.focus_handle);
    }

    // ── movement ───────────────────────────────────────────────────────

    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.previous_boundary(self.cursor_offset()), cx);
        } else {
            self.move_to(self.selected_range.start, cx)
        }
    }

    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.next_boundary(self.cursor_offset()), cx);
        } else {
            self.move_to(self.selected_range.end, cx)
        }
    }

    fn up(&mut self, _: &Up, _: &mut Window, cx: &mut Context<Self>) {
        // While the autocomplete menu is open the arrows navigate it, not
        // the caret (Waku parity).
        if self.autocomplete_navigate(-1, cx) {
            return;
        }
        self.move_vertically(-1., cx);
    }

    fn down(&mut self, _: &Down, _: &mut Window, cx: &mut Context<Self>) {
        if self.autocomplete_navigate(1, cx) {
            return;
        }
        self.move_vertically(1., cx);
    }

    /// Move the autocomplete highlight when the menu is open. Returns true
    /// when the keystroke was consumed by the menu.
    fn autocomplete_navigate(&mut self, delta: i32, cx: &mut Context<Self>) -> bool {
        let Some(state) = &self.autocomplete else {
            return false;
        };
        let mut state = state.borrow_mut();
        if !state.open || state.count == 0 {
            return false;
        }
        state.move_highlight(delta);
        drop(state);
        cx.notify();
        true
    }

    /// Move the caret one visual row up/down, preserving the horizontal
    /// position when possible.
    fn move_vertically(&mut self, rows: f32, cx: &mut Context<Self>) {
        if self.last_lines.is_empty() {
            return;
        }
        let line_height = self.last_line_height;
        let caret = self.position_for_offset(self.cursor_offset());
        let target_y = caret.y + line_height * rows;
        if target_y < px(0.) {
            self.move_to(0, cx);
            return;
        }
        self.move_to(self.index_at_content_position(point(caret.x, target_y)), cx);
    }

    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous_boundary(self.cursor_offset()), cx)
    }

    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.next_boundary(self.selected_range.end), cx)
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, cx);
        self.select_to(self.content.len(), cx)
    }

    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, cx);
    }

    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.content.len(), cx);
    }

    fn newline(&mut self, _: &Newline, window: &mut Window, cx: &mut Context<Self>) {
        self.replace_text_in_range(None, "\n", window, cx);
    }

    // ── editing ────────────────────────────────────────────────────────

    fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.select_to(self.previous_boundary(self.cursor_offset()), cx)
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.select_to(self.next_boundary(self.cursor_offset()), cx)
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.is_selecting = true;
        if event.modifiers.shift {
            self.select_to(self.index_for_mouse_position(event.position), cx);
        } else {
            self.move_to(self.index_for_mouse_position(event.position), cx)
        }
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _window: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.is_selecting {
            self.select_to(self.index_for_mouse_position(event.position), cx)
        }
    }

    fn on_scroll_wheel(
        &mut self,
        event: &ScrollWheelEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let rows = self.last_total_rows.max(1);
        let visible = rows.min(self.max_lines);
        let max_scroll = ((rows - visible) as f32 * self.last_line_height).max(px(0.));
        let dy = event.delta.pixel_delta(self.last_line_height).y;
        self.scroll_offset = (self.scroll_offset - dy).clamp(px(0.), max_scroll);
        cx.notify();
    }

    fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        let Some(item) = cx.read_from_clipboard() else {
            return;
        };
        // Pasted images become message attachments (Waku: "Show pasted
        // images as attachments") — the app drains `pasted_images` and
        // renders chips above the composer.
        let images: Vec<Image> = item
            .entries()
            .iter()
            .filter_map(|entry| match entry {
                ClipboardEntry::Image(image) => Some(image.clone()),
                ClipboardEntry::String(_) => None,
            })
            .collect();
        if !images.is_empty() {
            self.pasted_images.extend(images);
            cx.notify();
            return;
        }
        if let Some(text) = item.text() {
            self.replace_text_in_range(None, &text, window, cx)
        }
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
        }
    }

    fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
            self.replace_text_in_range(None, "", window, cx)
        }
    }

    // ── geometry ───────────────────────────────────────────────────────

    fn move_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.selected_range = offset..offset;
        cx.notify()
    }

    fn cursor_offset(&self) -> usize {
        if self.selection_reversed {
            self.selected_range.start
        } else {
            self.selected_range.end
        }
    }

    /// Window position → position in content coordinates (scroll applied).
    fn content_position(&self, position: gpui::Point<Pixels>) -> gpui::Point<Pixels> {
        let Some(bounds) = self.last_bounds else {
            return point(px(0.), px(0.));
        };
        point(
            position.x - bounds.origin.x,
            position.y - bounds.origin.y + self.scroll_offset,
        )
    }

    /// The byte index under a point in *content* coordinates.
    fn index_at_content_position(&self, pos: gpui::Point<Pixels>) -> usize {
        if self.content.is_empty() || self.last_lines.is_empty() {
            return 0;
        }
        let line_height = self.last_line_height;
        let mut y_acc = px(0.);
        for (i, line) in self.last_lines.iter().enumerate() {
            let height = line.size(line_height).height;
            let is_last = i == self.last_lines.len() - 1;
            if pos.y <= y_acc + height || is_last {
                let local_x = pos.x.clamp(px(0.), line.width());
                let local_y = pos.y.clamp(y_acc, y_acc + height - px(0.5)) - y_acc;
                return line
                    .closest_index_for_position(point(local_x, local_y), line_height)
                    .unwrap_or_else(|ix| ix);
            }
            y_acc += height;
        }
        self.content.len()
    }

    fn index_for_mouse_position(&self, position: gpui::Point<Pixels>) -> usize {
        self.index_at_content_position(self.content_position(position))
    }

    /// The (x, y) of a byte offset in content coordinates — y is the top of
    /// the visual row the offset sits on.
    fn position_for_offset(&self, offset: usize) -> gpui::Point<Pixels> {
        let line_height = self.last_line_height;
        let line_lens: Vec<usize> = self.last_lines.iter().map(|line| line.len()).collect();
        if let Some((i, local)) = line_at_offset(&self.last_line_starts, &line_lens, offset) {
            let line = &self.last_lines[i];
            return line
                .position_for_index(local, line_height)
                .unwrap_or_else(|| {
                    point(
                        line.width(),
                        line.wrap_boundaries().len() as f32 * line_height,
                    )
                });
        }
        point(px(0.), px(0.))
    }

    fn select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        if self.selection_reversed {
            self.selected_range.start = offset
        } else {
            self.selected_range.end = offset
        };
        if self.selected_range.end < self.selected_range.start {
            self.selection_reversed = !self.selection_reversed;
            self.selected_range = self.selected_range.end..self.selected_range.start;
        }
        cx.notify()
    }

    fn offset_from_utf16(&self, offset: usize) -> usize {
        let mut utf8_offset = 0;
        let mut utf16_count = 0;
        for ch in self.content.chars() {
            if utf16_count >= offset {
                break;
            }
            utf16_count += ch.len_utf16();
            utf8_offset += ch.len_utf8();
        }
        utf8_offset
    }

    fn offset_to_utf16(&self, offset: usize) -> usize {
        let mut utf16_offset = 0;
        let mut utf8_count = 0;
        for ch in self.content.chars() {
            if utf8_count >= offset {
                break;
            }
            utf8_count += ch.len_utf8();
            utf16_offset += ch.len_utf16();
        }
        utf16_offset
    }

    fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }

    fn range_from_utf16(&self, range_utf16: &Range<usize>) -> Range<usize> {
        self.offset_from_utf16(range_utf16.start)..self.offset_from_utf16(range_utf16.end)
    }

    fn previous_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .rev()
            .find_map(|(idx, _)| (idx < offset).then_some(idx))
            .unwrap_or(0)
    }

    fn next_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .find_map(|(idx, _)| (idx > offset).then_some(idx))
            .unwrap_or(self.content.len())
    }
}

fn cx_focus_handle(cx: &mut Context<ComposerInput>) -> FocusHandle {
    cx.focus_handle()
}

impl Focusable for ComposerInput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl EntityInputHandler for ComposerInput {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.range_from_utf16(&range_utf16);
        actual_range.replace(self.range_to_utf16(&range));
        Some(self.content[range].to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.range_to_utf16(&self.selected_range),
            reversed: self.selection_reversed,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        None
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {}

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|range| self.range_from_utf16(range))
            .unwrap_or(self.selected_range.clone());
        self.content =
            self.content[0..range.start].to_owned() + new_text + &self.content[range.end..];
        self.selected_range = range.start + new_text.len()..range.start + new_text.len();
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // IME composition is stubbed; treat as a plain replace.
        let range = range_utf16
            .as_ref()
            .map(|range| self.range_from_utf16(range))
            .unwrap_or(self.selected_range.clone());
        self.content =
            self.content[0..range.start].to_owned() + new_text + &self.content[range.end..];
        self.selected_range = new_selected_range_utf16
            .as_ref()
            .map(|r| self.range_from_utf16(r))
            .map(|r| r.start + new_text.len()..r.end + new_text.len())
            .unwrap_or_else(|| range.start + new_text.len()..range.start + new_text.len());
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let range = self.range_from_utf16(&range_utf16);
        let start = self.position_for_offset(range.start);
        let end = self.position_for_offset(range.end);
        let origin = point(
            bounds.origin.x + start.x,
            bounds.origin.y + start.y - self.scroll_offset,
        );
        Some(Bounds::from_corners(
            origin,
            point(
                origin.x + (end.x - start.x).abs(),
                origin.y + self.last_line_height,
            ),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: gpui::Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        Some(self.index_for_mouse_position(point))
    }
}

/// Locate the logical line that contains byte `offset`, returning the line
/// index and the local byte index within it. `line_starts[i]` is the byte
/// offset of line `i`; `line_lens[i]` is its length excluding the trailing
/// newline that `shape_text` split off.
///
/// A caret can sit on a newline byte, which belongs to no line's text; we
/// attribute it to the end of the preceding line. `offset` can also be stale
/// relative to freshly shaped lines, so the local index saturates instead of
/// underflowing (which would panic in debug builds).
fn line_at_offset(
    line_starts: &[usize],
    line_lens: &[usize],
    offset: usize,
) -> Option<(usize, usize)> {
    let last = line_lens.len().checked_sub(1)?;
    line_starts
        .iter()
        .zip(line_lens)
        .enumerate()
        .find_map(|(i, (&start, &len))| {
            (offset <= start + len || i == last).then(|| (i, offset.saturating_sub(start).min(len)))
        })
}

/// The painted text element for the input.
struct TextElement {
    input: Entity<ComposerInput>,
}

impl IntoElement for TextElement {
    type Element = Self;
    fn into_element(self) -> Self::Element {
        self
    }
}

struct PrepaintState {
    lines: Vec<WrappedLine>,
    /// y offset of each logical line in content coordinates.
    line_y: Vec<Pixels>,
    scroll_offset: Pixels,
    /// Selection wash quads, painted under the text.
    selection: Vec<PaintQuad>,
    /// Caret quad, painted over the text (focused only).
    caret: Option<PaintQuad>,
}

impl Element for TextElement {
    type RequestLayoutState = ();
    type PrepaintState = PrepaintState;

    fn id(&self) -> Option<gpui::ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let line_height = window.line_height();
        let input = self.input.read(cx);
        // Auto-grow: one line up to `max_lines`, then the height pins and
        // the content scrolls internally.
        let visible_rows = input.last_total_rows.max(1).min(input.max_lines);
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = (visible_rows as f32 * line_height).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let line_height = window.line_height();
        let (content, selected_range, cursor, max_lines) = {
            let input = self.input.read(cx);
            (
                input.content.clone(),
                input.selected_range.clone(),
                input.cursor_offset(),
                input.max_lines,
            )
        };
        let style = window.text_style();
        let theme = theme::get(cx);

        let (display_text, text_color) = if content.is_empty() {
            (self.input.read(cx).placeholder.clone(), theme.text_3)
        } else {
            (SharedString::from(content.clone()), style.color)
        };

        let run = TextRun {
            len: display_text.len(),
            font: style.font(),
            color: text_color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let font_size = style.font_size.to_pixels(window.rem_size());
        let wrap_width = (bounds.size.width > px(0.)).then_some(bounds.size.width);
        let lines: Vec<WrappedLine> = window
            .text_system()
            .shape_text(display_text, font_size, &[run], wrap_width, None)
            .map(|shaped| shaped.to_vec())
            .unwrap_or_default();

        // Byte offset and y of each logical line (`WrappedLine::len`
        // excludes the newline that `shape_text` split off).
        let mut line_starts = Vec::with_capacity(lines.len());
        let mut line_lens = Vec::with_capacity(lines.len());
        let mut line_y = Vec::with_capacity(lines.len());
        let mut byte_acc = 0usize;
        let mut y_acc = px(0.);
        for line in &lines {
            line_starts.push(byte_acc);
            line_lens.push(line.len());
            line_y.push(y_acc);
            byte_acc += line.len() + 1;
            y_acc += line.size(line_height).height;
        }
        let total_rows: usize = lines
            .iter()
            .map(|line| line.wrap_boundaries().len() + 1)
            .sum::<usize>()
            .max(1);
        let visible_rows = total_rows.min(max_lines);
        let content_height = total_rows as f32 * line_height;
        let visible_height = visible_rows as f32 * line_height;

        // Clamp the scroll so the caret stays on screen after edits.
        let mut scroll_offset = self.input.read(cx).scroll_offset;
        let max_scroll = (content_height - visible_height).max(px(0.));
        scroll_offset = scroll_offset.min(max_scroll).max(px(0.));
        if !lines.is_empty() {
            if let Some((i, local)) = line_at_offset(&line_starts, &line_lens, cursor) {
                if let Some(pos) = lines[i].position_for_index(local, line_height) {
                    let caret_y = line_y[i] + pos.y;
                    scroll_offset = scroll_offset.max(caret_y + line_height - visible_height);
                    scroll_offset = scroll_offset.min(caret_y).min(max_scroll).max(px(0.));
                }
            }
        }

        // Window-coordinate mapping of a content point.
        let map = |x: Pixels, y: Pixels| -> gpui::Point<Pixels> {
            point(bounds.origin.x + x, bounds.origin.y + y - scroll_offset)
        };
        let caret_fallback = |line: &WrappedLine| -> gpui::Point<Pixels> {
            point(
                line.width(),
                line.wrap_boundaries().len() as f32 * line_height,
            )
        };

        // Selection wash per logical line: one rect per visual row.
        let mut selection: Vec<PaintQuad> = Vec::new();
        let sel = selected_range.start..selected_range.end;
        if !sel.is_empty() {
            for (i, line) in lines.iter().enumerate() {
                let line_start = line_starts[i];
                let line_end = line_start + line.len();
                let overlap_start = sel.start.max(line_start);
                let overlap_end = sel.end.min(line_end);
                if overlap_start >= overlap_end {
                    continue;
                }
                let start_pos = line
                    .position_for_index(overlap_start - line_start, line_height)
                    .unwrap_or_else(|| point(px(0.), px(0.)));
                let end_pos = line
                    .position_for_index(overlap_end - line_start, line_height)
                    .unwrap_or_else(|| caret_fallback(line));
                let row_start = (start_pos.y / line_height).floor() as i32;
                let row_end = (end_pos.y / line_height).floor() as i32;
                for row in row_start..=row_end {
                    let (x0, x1) = if row == row_start && row == row_end {
                        (start_pos.x, end_pos.x)
                    } else if row == row_start {
                        (start_pos.x, line.width())
                    } else if row == row_end {
                        (px(0.), end_pos.x)
                    } else {
                        (px(0.), line.width())
                    };
                    let y = line_y[i] + row as f32 * line_height;
                    selection.push(fill(
                        Bounds::from_corners(
                            map(x0, y),
                            point(map(x1, y).x, map(x1, y).y + line_height),
                        ),
                        theme.accent.opacity(0.25),
                    ));
                }
            }
        }

        // Caret: 2px accent bar spanning the visual row.
        let mut caret_pos: Option<gpui::Point<Pixels>> = None;
        if !lines.is_empty() {
            if content.is_empty() {
                caret_pos = Some(point(px(0.), px(0.)));
            } else if let Some((i, local)) = line_at_offset(&line_starts, &line_lens, cursor) {
                let line = &lines[i];
                let raw = line
                    .position_for_index(local, line_height)
                    .unwrap_or_else(|| caret_fallback(line));
                caret_pos = Some(point(raw.x, line_y[i] + raw.y));
            }
        }
        let caret = caret_pos.map(|caret| {
            let origin = map(caret.x, caret.y);
            fill(
                Bounds::new(
                    point(origin.x, origin.y + px(2.)),
                    size(px(2.), line_height - px(4.)),
                ),
                theme.accent,
            )
        });

        let width_changed = self.input.read(cx).last_wrap_width != wrap_width;
        let rows_changed = self.input.read(cx).last_total_rows != total_rows;
        let snapshot = (lines.clone(), line_starts.clone(), scroll_offset);
        self.input.update(cx, |input, _| {
            input.last_lines = snapshot.0;
            input.last_line_starts = snapshot.1;
            input.scroll_offset = snapshot.2;
            input.last_line_height = line_height;
            input.last_bounds = Some(bounds);
            input.last_wrap_width = wrap_width;
            input.last_total_rows = total_rows;
        });
        // A resize changed the wrap width *and* the row count — the height
        // used at request_layout was one frame stale; re-layout.
        if width_changed && rows_changed {
            window.refresh();
        }

        PrepaintState {
            lines,
            line_y,
            scroll_offset,
            selection,
            caret,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus_handle = self.input.read(cx).focus_handle.clone();
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.input.clone()),
            cx,
        );

        let focused = focus_handle.is_focused(window);
        let scroll = prepaint.scroll_offset;
        let line_height = window.line_height();

        // Clip everything to the element: wrapping keeps text inside, the
        // mask handles the scrolled state past `max_lines`.
        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            // Selection wash under the text.
            for quad in prepaint.selection.drain(..) {
                window.paint_quad(quad);
            }
            for (i, line) in prepaint.lines.iter().enumerate() {
                line.paint(
                    point(
                        bounds.origin.x,
                        bounds.origin.y + prepaint.line_y[i] - scroll,
                    ),
                    line_height,
                    TextAlign::Left,
                    None,
                    window,
                    cx,
                )
                .unwrap();
            }
            // Caret over the text.
            if focused {
                if let Some(caret) = prepaint.caret.take() {
                    window.paint_quad(caret);
                }
            }
        });
    }
}

impl Render for ComposerInput {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Transparent: the floating composer box in app.rs provides the
        // background/border; this is just the editable (auto-growing) area.
        div()
            .id(self.element_id.clone())
            .flex_1()
            .min_w_0()
            .key_context(self.key_context.as_ref())
            .track_focus(&self.focus_handle(cx))
            .cursor(CursorStyle::IBeam)
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::up))
            .on_action(cx.listener(Self::down))
            .on_action(cx.listener(Self::newline))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::copy))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_scroll_wheel(cx.listener(Self::on_scroll_wheel))
            .child(TextElement { input: cx.entity() })
    }
}

#[cfg(test)]
mod tests {
    use super::line_at_offset;

    /// A caret on the newline byte (`"abc\n"` at offset 3) must resolve to
    /// the end of the preceding line, not underflow into the next line's
    /// start (the panic this guard was added for).
    #[test]
    fn caret_on_newline_belongs_to_preceding_line() {
        let starts = [0, 4];
        let lens = [3, 0];
        assert_eq!(line_at_offset(&starts, &lens, 3), Some((0, 3)));
    }

    #[test]
    fn caret_positions_map_to_their_line() {
        let starts = [0, 4, 8];
        let lens = [3, 3, 2];
        assert_eq!(line_at_offset(&starts, &lens, 0), Some((0, 0)));
        assert_eq!(line_at_offset(&starts, &lens, 3), Some((0, 3)));
        assert_eq!(line_at_offset(&starts, &lens, 4), Some((1, 0)));
        assert_eq!(line_at_offset(&starts, &lens, 7), Some((1, 3)));
        assert_eq!(line_at_offset(&starts, &lens, 8), Some((2, 0)));
        assert_eq!(line_at_offset(&starts, &lens, 10), Some((2, 2)));
    }

    /// A stale offset past the shaped lines clamps to the last line instead
    /// of panicking.
    #[test]
    fn stale_offset_clamps_to_last_line() {
        let starts = [0, 4];
        let lens = [3, 0];
        assert_eq!(line_at_offset(&starts, &lens, 99), Some((1, 0)));
    }

    #[test]
    fn no_lines_yields_none() {
        assert_eq!(line_at_offset(&[], &[], 0), None);
    }
}
