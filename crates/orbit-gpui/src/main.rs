//! Orbit · GPUI Phase 0 spike
//!
//! Proves the performance thesis before any real UI work:
//!   1. A virtualized list of 10,000 chat-style messages, rendered on GPU.
//!   2. Variable-height items (prose + code blocks) to exercise real layout,
//!      not a uniform grid.
//!   3. A fake "streaming agent" appending messages every 80ms with
//!      stick-to-latest autoscroll — the worst case for a chat app.
//!
//! Run with:   cargo run -p orbit-gpui  (from crates/orbit-gpui: cargo run)
//! Quit with:  cmd-q

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};

use gpui::{
    actions, div, list, prelude::*, px, rgb, size, App, Application, AsyncWindowContext, Bounds,
    Context, Div, Entity, KeyBinding, ListAlignment, ListOffset, ListScrollEvent, ListState,
    MouseButton, MouseUpEvent, Render, SharedString, Timer, TitlebarOptions, Window, WindowBounds,
    WindowOptions,
};

actions!(orbit_spike, [Quit]);

// ---------------------------------------------------------------------------
// Mock data — deterministic, with plausible chat variety and big
// height variance between items (2 lines of prose up to a ~7-line code block).
// ---------------------------------------------------------------------------

const WORDS: &[&str] = &[
    "buffer", "composer", "diff", "frame", "session", "stream", "tool", "token", "render",
    "pixel", "async", "syntax", "theme", "undo", "model", "agent", "orbit", "cursor", "layout",
    "glyph",
];

fn words(n: usize, salt: usize) -> String {
    (0..n)
        .map(|i| WORDS[(i * 7 + salt * 13) % WORDS.len()])
        .collect::<Vec<_>>()
        .join(" ")
}

struct MockMessage {
    user: bool,
    text: String,
    code: Option<String>,
}

fn mock_message(i: usize) -> MockMessage {
    if i % 4 == 0 {
        // user message: a short question
        MockMessage {
            user: true,
            text: format!("Message #{i} — {}", words(4 + i % 5, i)),
            code: None,
        }
    } else {
        // assistant reply, sometimes with a multi-line code block so
        // item heights vary widely (the worst case for layout caches)
        let code = if i % 3 == 1 {
            let lines = 1 + i % 7;
            Some(
                (0..lines)
                    .map(|line| match (i * 31 + line * 7) % 3 {
                        0 => format!("+ fn apply_{i}_{line}() -> Result<()> {{\n+     self.buffer.transact(cx);"),
                        1 => {
                            format!("- let old_len = self.history.len();\n- self.history.truncate(old_len - 1);")
                        }
                        _ => format!(
                            "  self.undo_stack.push(entry);\n  cx.notify(); // {}",
                            words(3, i + line)
                        ),
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
            )
        } else {
            None
        };
        MockMessage {
            user: false,
            text: format!("Reply #{i} — {}", words(3 + i % 8, i + 1)),
            code,
        }
    }
}

// ---------------------------------------------------------------------------
// The view
// ---------------------------------------------------------------------------

struct ChatSpike {
    /// Shared so the list's render closure (`'static`) can index it;
    /// interior mutability so the streaming tick can append.
    messages: Rc<RefCell<Vec<MockMessage>>>,
    list: ListState,
    streaming: bool,
    /// Whether the list is currently pinned to the newest message.
    /// Kept on an `Rc` so the scroll handler can flip it without a full
    /// entity borrow cycle.
    pinned: Rc<Cell<bool>>,
    visible: Rc<RefCell<(usize, usize)>>,
}

impl ChatSpike {
    fn new() -> Self {
        let messages: Vec<MockMessage> = (0..10_000).map(mock_message).collect();
        let messages = Rc::new(RefCell::new(messages));

        // Bottom-aligned: like a chat log, index 0 at the top, newest at bottom.
        let list = ListState::new(10_000, ListAlignment::Bottom, px(160.));

        let pinned = Rc::new(Cell::new(true));
        let visible = Rc::new(RefCell::new((0usize, 0usize)));

        {
            let pinned = pinned.clone();
            let visible = visible.clone();
            list.set_scroll_handler(move |event: &ListScrollEvent, _window, _cx| {
                // For a Bottom-aligned list, `is_scrolled` is exactly
                // "not at the bottom" — scroll_events carry it only for
                // real user scrolls, not programmatic `scroll_to` calls.
                pinned.set(!event.is_scrolled);
                *visible.borrow_mut() = (event.visible_range.start, event.visible_range.end);
            });
        }

        // Start pinned to the latest message.
        list.scroll_to(ListOffset {
            item_ix: 10_000,
            offset_in_item: px(0.),
        });

        ChatSpike {
            messages,
            list,
            streaming: true,
            pinned,
            visible,
        }
    }

    /// Called every ~80ms by the fake agent task.
    fn stream_tick(&mut self, cx: &mut Context<Self>) {
        if !self.streaming {
            return;
        }
        let start = self.messages.borrow().len();
        let msg = mock_message(start);
        {
            let mut msgs = self.messages.borrow_mut();
            msgs.push(msg);
            let end = msgs.len();
            // splice: replaced nothing, inserted 1 item at the end
            self.list.splice(start..start, 1);
            if self.pinned.get() {
                self.list.scroll_to(ListOffset {
                    item_ix: end,
                    offset_in_item: px(0.),
                });
            }
        }
        cx.notify();
    }

    fn on_toggle_streaming(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.streaming = !self.streaming;
        cx.notify();
    }

    fn on_jump_to_latest(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        let end = self.messages.borrow().len();
        self.list.scroll_to(ListOffset {
            item_ix: end,
            offset_in_item: px(0.),
        });
        self.pinned.set(true);
        cx.notify();
    }

    fn header(&self, cx: &mut Context<Self>) -> Div {
        let total = self.messages.borrow().len();
        let (vis_start, vis_end) = *self.visible.borrow();

        div()
            .h(px(48.))
            .w_full()
            .bg(rgb(0x1a1d23))
            .border_b_1()
            .border_color(rgb(0x2a2e36))
            .flex()
            .items_center()
            .gap_3()
            .px_4()
            .child(
                div()
                    .text_color(rgb(0x9aa0b1))
                    .text_size(px(14.))
                    .child("orbit·gpui-spike"),
            )
            .child(
                div()
                    .bg(rgb(0x22252d))
                    .rounded_md()
                    .px_2()
                    .py_1()
                    .text_size(px(12.))
                    .text_color(if self.streaming {
                        rgb(0x6fdc7f)
                    } else {
                        rgb(0x8b90a0)
                    })
                    .child(if self.streaming { "● streaming" } else { "‖ paused" }),
            )
            .child(div().flex_1())
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(rgb(0x7c8294))
                    .child(format!(
                        "visible {vis_start}–{vis_end} · {total} messages · virtualization {:0.1}%",
                        100.0 * (vis_end - vis_start).max(1) as f64 / total as f64
                    )),
            )
            .child(div().w_3())
            .child(self.header_button("Jump to latest", cx.listener(Self::on_jump_to_latest)))
            .child(
                self.header_button(
                    if self.streaming { "Pause stream" } else { "Resume stream" },
                    cx.listener(Self::on_toggle_streaming),
                ),
            )
    }

    fn header_button(
        &self,
        label: &str,
        handler: impl Fn(&MouseUpEvent, &mut Window, &mut App) + 'static,
    ) -> Div {
        div()
            .px_3()
            .py_1()
            .rounded_md()
            .bg(rgb(0x262a33))
            .text_size(px(13.))
            .text_color(rgb(0xd5d9e2))
            .cursor_pointer()
            .hover(|s| s.bg(rgb(0x2f3440)))
            .on_mouse_up(MouseButton::Left, handler)
            .child(label.to_string())
    }

    fn composer(&self) -> Div {
        div()
            .h(px(56.))
            .w_full()
            .bg(rgb(0x1a1d23))
            .border_t_1()
            .border_color(rgb(0x2a2e36))
            .flex()
            .items_center()
            .px_4()
            .child(
                div()
                    .flex_1()
                    .h(px(36.))
                    .rounded_md()
                    .bg(rgb(0x22252d))
                    .px_3()
                    .flex()
                    .items_center()
                    .text_size(px(13.))
                    .text_color(rgb(0x6b7180))
                    .child("Composer placeholder — real editor lands in Phase 3"),
            )
    }
}

impl Render for ChatSpike {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let messages = self.messages.clone();

        div()
            .size_full()
            .bg(rgb(0x14161a))
            .text_color(rgb(0xd6d9e0))
            .flex()
            .flex_col()
            .child(self.header(cx))
            .child(
                list(self.list.clone(), move |ix, _window, _cx| {
                    render_message(&messages, ix).into_any_element()
                })
                .flex_1()
                .w_full(),
            )
            .child(self.composer())
    }
}

fn render_message(messages: &Rc<RefCell<Vec<MockMessage>>>, ix: usize) -> impl IntoElement {
    // Bind the Ref first so items built from it don't out-live it.
    let messages = messages.borrow();
    let msg = &messages[ix];

    if msg.user {
        // User row: right-aligned bubble.
        div()
            .w_full()
            .flex()
            .justify_end()
            .px_4()
            .py_2()
            .child(
                div()
                    .max_w(px(640.))
                    .rounded_lg()
                    .bg(rgb(0x2c3a55))
                    .text_color(rgb(0xc9d4ea))
                    .px_3()
                    .py_2()
                    .text_size(px(14.))
                    .child(msg.text.clone()),
            )
    } else {
        // Assistant row: left-aligned with an accent bar + code block.
        let mut row = div()
            .w_full()
            .flex()
            .px_4()
            .py_2()
            .gap_3()
            .child(
                div()
                    .w(px(3.))
                    .rounded_sm()
                    .bg(rgb(0x4c8dff)),
            );

        if let Some(code) = msg.code.as_ref() {
            row = row.child(
                div()
                    .flex_1()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_size(px(14.))
                            .child(msg.text.clone()),
                    )
                    .child(
                        div()
                            .rounded_md()
                            .bg(rgb(0x0e1014))
                            .border_1()
                            .border_color(rgb(0x29303a))
                            .px_3()
                            .py_2()
                            .font_family("Menlo")
                            .text_size(px(12.5))
                            .text_color(rgb(0xa8d0a0))
                            .children(code.lines().map(|line| {
                                div().h(px(19.)).child(format!("  {line}"))
                            })),
                    ),
            );
        } else {
            row = row.child(
                div()
                    .flex_1()
                    .text_size(px(14.))
                    .child(msg.text.clone()),
            );
        }

        row
    }
}

// ---------------------------------------------------------------------------
// Bootstrap
// ---------------------------------------------------------------------------

fn main() {
    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1200.), px(800.)), cx);
        let _window = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some(SharedString::from(
                            "Orbit · GPUI spike — 10k virtualized messages",
                        )),
                        ..Default::default()
                    }),
                    focus: true,
                    ..Default::default()
                },
                |window, cx| {
                    let view: Entity<ChatSpike> = cx.new(|_| ChatSpike::new());

                    // Fake agent: append a message every 80ms and let the
                    // view keep the list pinned to the newest item.
                    let streamer = view.clone();
                    window
                        .spawn(cx, {
                            async move |cx: &mut AsyncWindowContext| {
                                loop {
                                    Timer::after(Duration::from_millis(80)).await;
                                    if streamer
                                        .update(cx, |spike, cx| spike.stream_tick(cx))
                                        .is_err()
                                    {
                                        break;
                                    }
                                }
                            }
                        })
                        .detach();

                    view
                },
            )
            .unwrap();

        cx.activate(true);
        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);
    });
}
