use super::*;
use crate::ask::{self, AskMode};

impl OrbitApp {
    /// Record a running `ask_user_question` call and parse its questionnaire.
    /// Called from the event pump before the transcript applies the same
    /// event, so the panel is ready before the extension's first `select`.
    pub(super) fn on_ask_tool_start(&mut self, value: &Value, cx: &mut Context<Self>) {
        let tool = value.get("toolName").and_then(Value::as_str).unwrap_or("");
        if !ask::is_ask_tool(tool) {
            return;
        }
        self.ask_tool_id = value
            .get("toolCallId")
            .and_then(Value::as_str)
            .map(str::to_string);
        self.ask_questions = ask::questions_from_args(value.get("args"));
        cx.notify();
    }

    /// The running ask tool finished — its panel (if any) can never be
    /// answered, so drop it. Matching by tool call id keeps a concurrent
    /// tool's end from clearing the wrong questionnaire.
    pub(super) fn on_ask_tool_end(&mut self, value: &Value, cx: &mut Context<Self>) {
        let Some(id) = value.get("toolCallId").and_then(Value::as_str) else {
            return;
        };
        if self.ask_tool_id.as_deref() != Some(id) {
            return;
        }
        self.ask_tool_id = None;
        self.ask_questions.clear();
        self.ask = None;
        self.ask_focus_pending = false;
        cx.notify();
    }

    /// True when a questionnaire is live and may claim a `select` / `input`.
    pub(super) fn ask_is_live(&self) -> bool {
        self.ask_tool_id.is_some() && !self.ask_questions.is_empty()
    }

    /// Open (or replace) the inline panel for a `select` request belonging to
    /// the running questionnaire.
    pub(super) fn open_ask_select(&mut self, id: String, value: &Value, cx: &mut Context<Self>) {
        // pi blocks one request at a time; a modal that is somehow already up
        // owns the answer. Cancel rather than clobber it.
        if self.dialog.is_some() || self.approval.is_some() {
            self.respond_to_dialog(&id, &DialogResponse::Cancelled);
            return;
        }
        let title = value.get("title").and_then(Value::as_str).unwrap_or("");
        let ix = ask::question_index_for_title(&self.ask_questions, title)
            .or_else(|| self.ask.as_ref().map(|prompt| prompt.question_ix))
            .unwrap_or(0);
        let Some(question) = self.ask_questions.get(ix) else {
            self.respond_to_dialog(&id, &DialogResponse::Cancelled);
            return;
        };
        let total = self.ask_questions.len();
        let header = question.header.clone();
        let question_text = question.question.clone();
        let options = question.options.clone();
        let custom_row = !question.multi_select;
        self.ask = Some(AskPrompt {
            id,
            mode: AskMode::Select,
            question_ix: ix,
            total,
            header,
            question: question_text,
            options,
            custom_row,
            checked: Vec::new(),
            highlighted: 0,
            submitted: false,
            input: None,
        });
        self.ask_focus_pending = true;
        cx.notify();
    }

    /// Open the inline panel for an `input` request belonging to the running
    /// questionnaire: a multi-select question, or the free-text follow-up to a
    /// single-select "Type something." row.
    pub(super) fn open_ask_input(&mut self, id: String, value: &Value, cx: &mut Context<Self>) {
        if !self.ask_is_live() {
            return;
        }
        let title = value.get("title").and_then(Value::as_str).unwrap_or("");
        let ix = ask::question_index_for_title(&self.ask_questions, title)
            .or_else(|| self.ask.as_ref().map(|prompt| prompt.question_ix))
            .unwrap_or(0);
        let Some(question) = self.ask_questions.get(ix) else {
            self.respond_to_dialog(&id, &DialogResponse::Cancelled);
            return;
        };
        let multi = question.multi_select;
        let total = self.ask_questions.len();
        let header = question.header.clone();
        let question_text = question.question.clone();
        let options = question.options.clone();
        let checked = vec![false; options.len()];
        let placeholder = value
            .get("placeholder")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let input = cx.new(|cx| {
            ComposerInput::new(cx)
                .with_element_id("ask-input")
                .with_placeholder(placeholder)
                .with_max_lines(1)
                .with_key_context("Composer AskInput")
        });
        self.ask = Some(AskPrompt {
            id,
            mode: if multi {
                AskMode::Multi
            } else {
                AskMode::Custom
            },
            question_ix: ix,
            total,
            header,
            question: question_text,
            options,
            custom_row: false,
            checked,
            highlighted: 0,
            submitted: false,
            input: Some(input),
        });
        self.ask_focus_pending = true;
        cx.notify();
    }

    /// Move the panel highlight (arrow keys). `Custom` has one focusable field
    /// and nothing to move between.
    pub(super) fn ask_move(&mut self, forward: bool, cx: &mut Context<Self>) {
        let Some(prompt) = self.ask.as_mut() else {
            return;
        };
        if prompt.mode == AskMode::Custom || prompt.submitted {
            return;
        }
        let count = prompt.row_count();
        if count == 0 {
            return;
        }
        prompt.highlighted = if forward {
            (prompt.highlighted + 1) % count
        } else {
            (prompt.highlighted + count - 1) % count
        };
        cx.notify();
    }

    /// Answer the panel: the highlighted row (select), the toggles (multi), or
    /// the typed text (custom). Sends the `extension_ui_response` pi is
    /// blocked on and hands focus back to the composer so typing resumes; the
    /// next request re-focuses the panel.
    pub(super) fn ask_confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(prompt) = self.ask.as_ref() else {
            return;
        };
        if prompt.submitted {
            return;
        }
        let response = match prompt.mode {
            AskMode::Select => {
                let ix = prompt.highlighted;
                if ix < prompt.options.len() {
                    // The extension parses the leading number out of the
                    // chosen string (`parseIndex`); the label keeps the value
                    // legible in the transcript envelope.
                    DialogResponse::Value(format!("{}. {}", ix + 1, prompt.options[ix].label))
                } else if prompt.custom_row {
                    DialogResponse::Value(format!("{}. Type something.", prompt.options.len() + 1))
                } else {
                    DialogResponse::Cancelled
                }
            }
            AskMode::Multi => {
                let typed = prompt
                    .input
                    .as_ref()
                    .map(|input| input.read(cx).text())
                    .unwrap_or_default();
                let value = if typed.trim().is_empty() {
                    prompt
                        .checked
                        .iter()
                        .enumerate()
                        .filter(|(_, on)| **on)
                        .map(|(ix, _)| (ix + 1).to_string())
                        .collect::<Vec<_>>()
                        .join(",")
                } else {
                    typed.trim().to_string()
                };
                DialogResponse::Value(value)
            }
            AskMode::Custom => {
                let text = prompt
                    .input
                    .as_ref()
                    .map(|input| input.read(cx).text())
                    .unwrap_or_default();
                DialogResponse::Value(text)
            }
        };
        let id = prompt.id.clone();
        self.respond_to_dialog(&id, &response);
        if let Some(prompt) = self.ask.as_mut() {
            prompt.submitted = true;
        }
        self.input.read(cx).focus(window);
        cx.notify();
    }

    /// Click / keyboard action for one panel row.
    pub(super) fn ask_choose(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(prompt) = self.ask.as_ref() else {
            return;
        };
        if prompt.submitted {
            return;
        }
        match prompt.mode {
            // A single-select row is the answer — no separate confirm step.
            AskMode::Select => {
                if let Some(prompt) = self.ask.as_mut() {
                    prompt.highlighted = ix;
                }
                self.ask_confirm(window, cx);
            }
            AskMode::Multi => {
                if ix < prompt.options.len() {
                    if let Some(prompt) = self.ask.as_mut() {
                        if let Some(slot) = prompt.checked.get_mut(ix) {
                            *slot = !*slot;
                        }
                        prompt.highlighted = ix;
                    }
                    cx.notify();
                } else {
                    // The trailing Continue row commits the toggles.
                    self.ask_confirm(window, cx);
                }
            }
            AskMode::Custom => {}
        }
    }

    /// Cancel the questionnaire (Esc / session replaced): pi sees a decline
    /// and the run continues without an answer.
    pub(super) fn ask_cancel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(prompt) = self.ask.take() else {
            return;
        };
        self.ask_focus_pending = false;
        self.respond_to_dialog(&prompt.id, &DialogResponse::Cancelled);
        self.input.read(cx).focus(window);
        cx.notify();
    }

    // ── key handlers (the `AskPanel` / `AskInput` contexts) ──────────────

    pub(super) fn on_ask_next(
        &mut self,
        _: &crate::AskNext,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.ask_move(true, cx);
    }

    pub(super) fn on_ask_prev(
        &mut self,
        _: &crate::AskPrev,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.ask_move(false, cx);
    }

    pub(super) fn on_ask_confirm(
        &mut self,
        _: &crate::AskConfirm,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // In multi-select, Enter/Space toggles the highlighted row; the
        // trailing Continue row submits. Every other mode answers outright.
        if self
            .ask
            .as_ref()
            .is_some_and(|prompt| prompt.mode == AskMode::Multi)
        {
            let ix = self
                .ask
                .as_ref()
                .map(|prompt| prompt.highlighted)
                .unwrap_or(0);
            self.ask_choose(ix, window, cx);
        } else {
            self.ask_confirm(window, cx);
        }
    }

    /// Enter inside the panel's text field always submits the typed value
    /// (custom answer, or a multi custom answer).
    pub(super) fn on_ask_submit(
        &mut self,
        _: &crate::AskSubmit,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.ask_confirm(window, cx);
    }

    pub(super) fn on_ask_close(
        &mut self,
        _: &crate::AskClose,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.ask_cancel(window, cx);
    }
}
