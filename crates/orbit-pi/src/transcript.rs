//! The transcript: pi `AgentMessage`s rendered as a virtualized chat list.
//!
//! Data flows in two ways and lands in the same model:
//! - **Snapshot** — `get_messages` response (`data.messages`) rebuilds the
//!   whole transcript when a session is opened.
//! - **Stream** — `message_start` / `message_update` / `message_end` events
//!   append and mutate messages live while the agent runs.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use gpui::{div, list, prelude::*, px, rgb, ListAlignment, ListOffset, ListScrollEvent, ListState};

use crate::file_icons::dev_icon;
use orbit_rpc::{AssistantMessageEvent, Event};
use serde_json::Value;

pub struct ChatMessage {
    pub user: bool,
    pub text: String,
    pub thinking: String,
    /// Tool calls rendered under the assistant text.
    pub tools: Vec<ToolCall>,
}

/// One tool call row: name, args summary, and the file path it operates on
/// (when the tool is file-oriented — drives the devicons glyph).
pub struct ToolCall {
    pub name: String,
    pub summary: String,
    pub path: Option<String>,
}

impl ToolCall {
    fn from_value(name: &str, args: Option<&Value>) -> Self {
        Self {
            name: name.to_string(),
            summary: summarize_value(args.unwrap_or(&Value::Null)),
            path: args
                .and_then(|a| a.get("path"))
                .and_then(Value::as_str)
                .map(str::to_string),
        }
    }
}

impl ChatMessage {
    fn empty_assistant() -> Self {
        Self {
            user: false,
            text: String::new(),
            thinking: String::new(),
            tools: Vec::new(),
        }
    }

    /// Parse one `AgentMessage` JSON value from pi.
    fn from_value(value: &Value) -> Option<ChatMessage> {
        let role = value.get("role")?.as_str()?;
        let user = role == "user";
        let mut message = ChatMessage {
            user,
            text: String::new(),
            thinking: String::new(),
            tools: Vec::new(),
        };

        match value.get("content") {
            Some(Value::String(text)) => message.text = text.clone(),
            Some(Value::Array(blocks)) => {
                for block in blocks {
                    let kind = block.get("type").and_then(Value::as_str).unwrap_or("");
                    match kind {
                        "text" => {
                            if let Some(text) = block.get("text").and_then(Value::as_str) {
                                if !message.text.is_empty() {
                                    message.text.push_str("\n\n");
                                }
                                message.text.push_str(text);
                            }
                        }
                        "thinking" => {
                            if let Some(text) = block.get("thinking").and_then(Value::as_str) {
                                message.thinking.push_str(text);
                            }
                        }
                        "toolCall" | "tool_call" => {
                            let name = block.get("name").and_then(Value::as_str).unwrap_or("tool");
                            message
                                .tools
                                .push(ToolCall::from_value(name, block.get("arguments")));
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
        Some(message)
    }
}

fn summarize_value(value: &Value) -> String {
    let text = value.to_string();
    if text.len() > 160 {
        let mut out: String = text.chars().take(160).collect();
        out.push('…');
        out
    } else {
        text
    }
}

/// Transcript state shared between the view and the RPC event pump.
pub struct Transcript {
    messages: Rc<RefCell<Vec<ChatMessage>>>,
    list: ListState,
    pinned: Rc<Cell<bool>>,
    /// Index of the assistant message currently being streamed, if any.
    streaming: Rc<Cell<Option<usize>>>,
}

impl Transcript {
    pub fn new() -> Self {
        let messages = Rc::new(RefCell::new(Vec::new()));
        let list = ListState::new(0, ListAlignment::Bottom, px(120.));

        let pinned = Rc::new(Cell::new(true));
        {
            let pinned = pinned.clone();
            list.set_scroll_handler(move |event: &ListScrollEvent, _window, _cx| {
                pinned.set(!event.is_scrolled);
            });
        }

        Self {
            messages,
            list,
            pinned,
            streaming: Rc::new(Cell::new(None)),
        }
    }

    /// Rebuild the whole transcript from a `get_messages` response payload.
    pub fn load_from(&mut self, data: &Value) {
        let mut parsed = Vec::new();
        if let Some(messages) = data.get("messages").and_then(Value::as_array) {
            for message in messages {
                if let Some(parsed_message) = ChatMessage::from_value(message) {
                    parsed.push(parsed_message);
                }
            }
        }
        let count = parsed.len();
        *self.messages.borrow_mut() = parsed;
        self.list.reset(count);
        self.scroll_to_end();
        self.streaming.set(None);
    }

    /// Clear for a fresh session.
    pub fn clear(&mut self) {
        *self.messages.borrow_mut() = Vec::new();
        self.list.reset(0);
        self.streaming.set(None);
    }

    /// Apply one protocol event; returns `true` if the transcript changed.
    pub fn apply_event(&mut self, event: &Event) -> bool {
        match event {
            Event::MessageStart { value } => self.on_message_boundary(value),
            Event::MessageUpdate { assistant, .. } => self.on_message_update(assistant),
            Event::MessageEnd { value } => self.on_message_end(value),
            _ => false,
        }
    }

    fn on_message_boundary(&mut self, value: &Value) -> bool {
        let Some(message) = ChatMessage::from_value(value) else {
            return false;
        };
        let mut messages = self.messages.borrow_mut();
        if message.user {
            // pi echoes the user's own message; show it (dedupe if identical).
            if messages.last().map(|m| m.user && m.text == message.text) != Some(true) {
                messages.push(message);
                self.streaming.set(None);
                self.on_appended(&messages);
                return true;
            }
            return false;
        }
        // Assistant message starts (possibly empty) — becomes the stream target.
        messages.push(message);
        let ix = messages.len() - 1;
        self.streaming.set(Some(ix));
        self.on_appended(&messages);
        true
    }

    fn on_message_update(&mut self, assistant: &Option<AssistantMessageEvent>) -> bool {
        use AssistantMessageEvent as Am;
        let Some(assistant) = assistant else {
            return false;
        };
        let changed = match assistant {
            Am::TextDelta { delta } => self.with_streaming(|m| m.text.push_str(delta)),
            Am::ThinkingDelta { delta } => self.with_streaming(|m| m.thinking.push_str(delta)),
            Am::ToolcallStart { value } => {
                let name = value
                    .get("toolName")
                    .and_then(Value::as_str)
                    .unwrap_or("tool");
                self.with_streaming(|m| {
                    m.tools
                        .push(ToolCall::from_value(name, value.get("arguments")))
                })
            }
            Am::ToolcallEnd { value } => {
                let name = value
                    .get("toolName")
                    .and_then(Value::as_str)
                    .unwrap_or("tool");
                let tool = ToolCall::from_value(name, value.get("arguments"));
                self.with_streaming(|m| {
                    if let Some(last) = m.tools.last_mut() {
                        *last = tool;
                    } else {
                        m.tools.push(tool);
                    }
                })
            }
            _ => false,
        };
        if changed {
            let messages = self.messages.borrow();
            self.on_appended(&messages);
        }
        changed
    }

    fn on_message_end(&mut self, value: &Value) -> bool {
        let Some(final_message) = ChatMessage::from_value(value) else {
            return false;
        };
        let mut messages = self.messages.borrow_mut();
        if final_message.user {
            // A finalized user message replaces any optimistic copy.
            if let Some(last) = messages.last_mut() {
                if last.user && last.text == final_message.text {
                    return false;
                }
            }
            messages.push(final_message);
        } else if let Some(ix) = self.streaming.get() {
            if let Some(slot) = messages.get_mut(ix) {
                *slot = final_message;
            }
            self.streaming.set(None);
        } else {
            messages.push(final_message);
        }
        self.on_appended(&messages);
        true
    }

    /// Mutate the assistant message currently being streamed (creating it if
    /// a delta arrives before its `message_start`).
    fn with_streaming(&mut self, mutate: impl FnOnce(&mut ChatMessage)) -> bool {
        let mut messages = self.messages.borrow_mut();
        let ix = match self.streaming.get() {
            Some(ix) => ix,
            None => {
                messages.push(ChatMessage::empty_assistant());
                let ix = messages.len() - 1;
                self.streaming.set(Some(ix));
                ix
            }
        };
        if let Some(message) = messages.get_mut(ix) {
            mutate(message);
            true
        } else {
            false
        }
    }

    fn on_appended(&self, messages: &[ChatMessage]) {
        let count = messages.len();
        self.list
            .splice(count.saturating_sub(1)..count.saturating_sub(1), 1);
        if self.pinned.get() {
            self.scroll_to_end();
        }
    }

    fn scroll_to_end(&self) {
        let count = self.messages.borrow().len();
        self.list.scroll_to(ListOffset {
            item_ix: count,
            offset_in_item: px(0.),
        });
    }

    /// True when no messages are loaded (drives the empty state).
    pub fn is_empty(&self) -> bool {
        self.messages.borrow().is_empty()
    }

    /// Render into the chat panel — a centered, max-width column.
    pub fn render(&self) -> impl IntoElement + use<> {
        let messages = self.messages.clone();

        div()
            .flex_1()
            .w_full()
            .flex()
            .justify_center()
            .min_h_0()
            .child(
                list(self.list.clone(), move |ix, _window, _cx| {
                    render_message(&messages, ix).into_any_element()
                })
                .w_full()
                .max_w(px(760.)),
            )
    }
}

use gpui::AnyElement;

fn render_message(messages: &Rc<RefCell<Vec<ChatMessage>>>, ix: usize) -> AnyElement {
    let messages = messages.borrow();
    let message = &messages[ix];

    if message.user {
        return div()
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
                    .child(message.text.clone()),
            )
            .into_any_element();
    }

    // Assistant: thinking block (dimmed) → text → tool rows.
    let mut column = div().flex_1().flex_col().gap_2();

    if !message.thinking.is_empty() {
        column = column.child(
            div()
                .rounded_md()
                .bg(rgb(0x1c1f27))
                .border_l_2()
                .border_color(rgb(0x6b5c8f))
                .px_3()
                .py_2()
                .text_size(px(12.5))
                .text_color(rgb(0x9a8fb8))
                .children(
                    message
                        .thinking
                        .lines()
                        .map(|line| div().h(px(17.)).child(format!("  {line}"))),
                ),
        );
    }

    if !message.text.is_empty() {
        column = column.child(
            div()
                .text_size(px(14.))
                .text_color(rgb(0xd6d9e0))
                .whitespace_normal()
                .child(message.text.clone()),
        );
    }

    if !message.tools.is_empty() {
        column = column.child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .children(message.tools.iter().map(|tool| {
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .rounded_md()
                        .bg(rgb(0x141820))
                        .border_1()
                        .border_color(rgb(0x29303a))
                        .px_3()
                        .py_1()
                        .font_family("Menlo")
                        .text_size(px(12.))
                        .text_color(rgb(0xd69c4c))
                        // devicons glyph for the target file, gear otherwise
                        .child(match &tool.path {
                            Some(path) => dev_icon(path, 13., 0x8b90a0).into_any_element(),
                            None => div().child("⚙").into_any_element(),
                        })
                        .child(tool.name.clone())
                        .child(
                            div()
                                .flex_1()
                                .text_color(rgb(0x8b90a0))
                                .overflow_x_hidden()
                                .child(tool.summary.clone()),
                        )
                })),
        );
    }

    div()
        .w_full()
        .flex()
        .px_4()
        .py_2()
        .gap_3()
        .child(div().w(px(3.)).rounded_sm().bg(rgb(0x4c8dff)))
        .child(column)
        .into_any_element()
}
