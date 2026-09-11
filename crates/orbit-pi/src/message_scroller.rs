//! Conversation scroller — an original, plain-GPUI implementation built on
//! [`gpui::ListState`] (no component library).
//!
//! Provides tail following, append/prepend/splice, item remeasure,
//! scroll-to-row, and a jump-to-latest control when the reader leaves the
//! live edge.
//!
//! GPUI 0.2.2 has no `FollowMode::Tail`; [`ListAlignment::Bottom`] plus a
//! past-the-end `scroll_to` is the equivalent: the last row's bottom stays
//! against the viewport while it grows.

use std::{cell::Cell, ops::Range, rc::Rc};

use gpui::{
    div, linear_color_stop, linear_gradient, prelude::*, px, rems, svg, ElementId, ListAlignment,
    ListOffset, ListScrollEvent, ListState,
};

use crate::theme::Theme;

const LIST_OVERDRAW: f32 = 400.0;

/// Entity-owned scrolling state for the transcript list.
#[derive(Clone)]
pub struct MessageScrollerState {
    list: ListState,
    following_tail: Rc<Cell<bool>>,
    /// First row currently visible in the viewport — the "reader is here"
    /// hint that drives the navigation rail's active tick.
    visible_start: Rc<Cell<usize>>,
}

impl MessageScrollerState {
    /// Create state for `item_count` rows and enable tail following.
    pub fn new(item_count: usize) -> Self {
        let list = ListState::new(item_count, ListAlignment::Bottom, px(LIST_OVERDRAW));
        let following_tail = Rc::new(Cell::new(true));
        let visible_start = Rc::new(Cell::new(0));
        {
            let following_tail = following_tail.clone();
            let visible_start = visible_start.clone();
            list.set_scroll_handler(move |event: &ListScrollEvent, _, cx| {
                // Bottom alignment: `is_scrolled` means the reader left the tail.
                following_tail.set(!event.is_scrolled);
                visible_start.set(event.visible_range.start);
                cx.refresh_windows();
            });
        }
        Self {
            list,
            following_tail,
            visible_start,
        }
    }

    /// First row currently in view (viewport-top hint for the rail).
    pub fn first_visible_index(&self) -> usize {
        self.visible_start.get()
    }

    pub fn list_state(&self) -> ListState {
        self.list.clone()
    }

    pub fn item_count(&self) -> usize {
        self.list.item_count()
    }

    /// True when the reader has left the live edge and there is room to scroll.
    pub fn is_scrolled_up(&self) -> bool {
        self.list.max_offset_for_scrollbar().height > px(0.) && !self.is_following_tail()
    }

    /// True when the transcript holds more content than the viewport shows
    /// (drives the Waku navigation-rail visibility).
    pub fn is_scrollable(&self) -> bool {
        self.list.max_offset_for_scrollbar().height > px(0.)
    }

    pub fn is_following_tail(&self) -> bool {
        self.following_tail.get()
    }

    /// Reset to `item_count` rows and resume tail following.
    pub fn reset(&self, item_count: usize) {
        self.list.reset(item_count);
        self.scroll_to_end();
    }

    /// Replace `old_range` with `count` new rows.
    ///
    /// Returns `false` when the range is outside the current list.
    pub fn splice(&self, old_range: Range<usize>, count: usize) -> bool {
        if !self.valid_range(&old_range) {
            return false;
        }
        self.list.splice(old_range, count);
        self.stick_if_following();
        true
    }

    /// Append `count` rows to the end of the list.
    pub fn append(&self, count: usize) -> bool {
        let item_count = self.item_count();
        self.splice(item_count..item_count, count)
    }

    /// Prepend `count` rows while preserving the current scroll anchor.
    #[allow(dead_code)]
    pub fn prepend(&self, count: usize) -> bool {
        self.splice(0..0, count)
    }

    /// Mark rows in `range` for remeasurement (streaming growth).
    pub fn remeasure_items(&self, range: Range<usize>) -> bool {
        if !self.valid_range(&range) || range.start == range.end {
            return false;
        }
        let count = range.end - range.start;
        self.list.splice(range, count);
        self.stick_if_following();
        true
    }

    /// Scroll to the row at `index`, if it exists. Leaves tail following.
    pub fn scroll_to_item(&self, index: usize) -> bool {
        if index >= self.item_count() {
            return false;
        }
        self.following_tail.set(false);
        self.list.scroll_to(ListOffset {
            item_ix: index,
            offset_in_item: px(0.),
        });
        true
    }

    /// Resume tail following and scroll to the latest row.
    pub fn scroll_to_end(&self) {
        self.following_tail.set(true);
        self.list.scroll_to(ListOffset {
            item_ix: self.item_count(),
            offset_in_item: px(0.),
        });
    }

    fn stick_if_following(&self) {
        if self.is_following_tail() {
            self.scroll_to_end();
        }
    }

    fn valid_range(&self, range: &Range<usize>) -> bool {
        range.start <= range.end && range.end <= self.item_count()
    }
}

/// Viewport + jump-to-latest chrome around a virtualized `list()`.
pub fn render_scroller(
    state: MessageScrollerState,
    theme: Theme,
    list: impl IntoElement,
) -> impl IntoElement {
    let scrolled_up = state.is_scrolled_up();
    div()
        .id(ElementId::Name("message-scroller".into()))
        .relative()
        .w_full()
        .min_w_0()
        .h_full()
        .min_h_0()
        .overflow_hidden()
        .child(
            div()
                .id(ElementId::Name("message-scroller-viewport".into()))
                .w_full()
                .min_w_0()
                .h_full()
                .min_h_0()
                .child(list),
        )
        .when(scrolled_up, |shell| {
            shell
                .child(render_bottom_fade(theme))
                .child(render_jump_button(state, theme))
        })
}

fn render_bottom_fade(theme: Theme) -> impl IntoElement {
    div()
        .id(ElementId::Name("message-scroller-fade".into()))
        .absolute()
        .left_0()
        .right_0()
        .bottom_0()
        .h(rems(3.))
        .bg(linear_gradient(
            180.,
            linear_color_stop(theme.bg_main.opacity(0.), 0.),
            linear_color_stop(theme.bg_main, 1.),
        ))
}

fn render_jump_button(state: MessageScrollerState, theme: Theme) -> impl IntoElement {
    div()
        .id(ElementId::Name("message-scroller-jump-layer".into()))
        .absolute()
        .left_0()
        .right_0()
        .bottom(px(8.))
        .w_full()
        .flex()
        .justify_center()
        .child(
            // Waku's floating round scroll-to-bottom affordance.
            div()
                .id(ElementId::Name("message-scroller-jump".into()))
                .size(px(32.))
                .rounded_full()
                .border_1()
                .border_color(theme.border_strong)
                .bg(theme.bg_composer)
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .shadow(theme.card_shadow())
                .hover(|style| style.bg(theme.bg_raised))
                .on_click(move |_, _, cx| {
                    state.scroll_to_end();
                    cx.refresh_windows();
                })
                .child(
                    svg()
                        .path("icons/arrow-down.svg")
                        .flex_none()
                        .size(px(16.))
                        .text_color(theme.text),
                ),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_starts_following_tail() {
        let state = MessageScrollerState::new(3);
        assert_eq!(state.item_count(), 3);
        assert!(state.is_following_tail());
        assert!(!state.is_scrolled_up());
    }

    #[test]
    fn append_prepend_splice_and_reset() {
        let state = MessageScrollerState::new(3);
        assert!(state.append(2));
        assert_eq!(state.item_count(), 5);
        assert!(state.prepend(1));
        assert_eq!(state.item_count(), 6);
        assert!(!state.splice(5..7, 0));
        assert!(state.remeasure_items(0..6));
        assert!(!state.remeasure_items(6..7));
        assert!(state.scroll_to_item(2));
        assert!(!state.is_following_tail());
        assert!(!state.scroll_to_item(6));
        state.scroll_to_end();
        assert!(state.is_following_tail());
        state.reset(2);
        assert_eq!(state.item_count(), 2);
        assert!(state.is_following_tail());
    }
}
