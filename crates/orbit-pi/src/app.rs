//! The real Orbit shell — Waku-style layout:
//! dark sidebar (New Task / Search nav, sessions grouped by age with
//! project + relative-time meta), top bar with real diff stats and window
//! controls, centered transcript with a spark empty state, floating bottom
//! composer (model / thinking / access pills + round send), and a status
//! bar (workspace, Local, git branch).

use std::{
    path::{Path, PathBuf},
    rc::Rc,
    time::{Duration, Instant},
};

use gpui::{
    anchored, deferred, div, hsla, list, point, prelude::*, px, rgb, AnchoredPositionMode,
    AnyElement, App, Context, Corner, Entity, FocusHandle, Focusable, FontWeight, IntoElement,
    ListAlignment, ListState, MouseButton, MouseUpEvent, Render, SharedString, Window,
    WindowControlArea,
};
use orbit_rpc::{CommandBody, Event, PiClient};

use crate::composer::ComposerInput;
use crate::model_selector::{
    provider_icon, thinking_display, thinking_icon, ModelSelector, PickerKind,
};
use crate::sessions::{self, SessionInfo};
use crate::transcript::Transcript;

// ── palette (waku dark) ────────────────────────────────────────────────────
// Reference UI: warm dark zinc, slightly lifted blacks for depth separation.
const BG_MAIN: u32 = 0x18181A;
const BG_SIDEBAR: u32 = 0x121214;
const BG_COMPOSER: u32 = 0x1E1E20;
const BG_RAISED: u32 = 0x252528;
const BG_HOVER: u32 = 0x2C2C30;
pub(crate) const BORDER: u32 = 0x2C2C2F;
pub(crate) const TEXT: u32 = 0xE4E4E7;
pub(crate) const TEXT_2: u32 = 0xA1A1AA;
pub(crate) const TEXT_3: u32 = 0x7D7D7D;
const OK_GREEN: u32 = 0x6FDC7F;
const STOP_RED: u32 = 0xC05050;
const ADD_GREEN: u32 = 0x7CC97F;
const DEL_RED: u32 = 0xE06C5F;
pub(crate) const SPARK_ORANGE: u32 = 0xE75F3B;

// Menu surface (Waku `raised` + translucent overlays).
pub(crate) const MENU_BG: u32 = 0x232323;
/// Hover wash inside menus (Waku `overlay`).
pub(crate) fn overlay() -> gpui::Hsla {
    hsla(220.0 / 360.0, 0.10, 0.90, 0.05)
}
/// Stronger wash for the open-menu trigger (Waku `overlay_strong`).
fn overlay_strong() -> gpui::Hsla {
    hsla(220.0 / 360.0, 0.10, 0.90, 0.09)
}
/// Menu border (Waku `border_strong`).
pub(crate) fn border_strong() -> gpui::Hsla {
    hsla(220.0 / 360.0, 0.10, 0.90, 0.14)
}

const SIDEBAR_W: f32 = 248.;
const CONTENT_MAX_W: f32 = 760.;

pub struct OrbitApp {
    client: Option<PiClient>,
    transcript: Transcript,
    sessions: Vec<SessionInfo>,
    sidebar_list: ListState,
    sidebar_visible: bool,
    pub(crate) input: Entity<ComposerInput>,
    model_label: String,
    /// Provider id of the active model (drives the brand glyph on the chip).
    model_provider: String,
    thinking_label: String,
    busy: bool,
    status: String,
    current_title: Option<String>,
    current_workspace: Option<PathBuf>,
    /// Real line counts from edit/write tool calls this session.
    added: u64,
    removed: u64,
    focus: FocusHandle,
    /// Catalog of models reported by `get_available_models`.
    available_models: Vec<ModelEntry>,
    /// Thinking levels reported by `get_available_thinking_levels`.
    available_thinking_levels: Vec<String>,
    /// The open picker popup (model or thinking dropdown), if any. The
    /// kind travels with the entity; creating/dropping this *is* the
    /// open/closed state. Each popup is anchored above its own chip.
    model_selector: Option<(PickerKind, Entity<ModelSelector>)>,
    /// Whether the settings surface replaces the main content area.
    settings_open: bool,
    /// Active section within the settings surface.
    settings_section: SettingsSection,
    /// Visited sessions, oldest first — drives the top-bar back/forward
    /// navigation. `history_index` points at the active entry.
    session_history: Vec<SessionInfo>,
    history_index: usize,
    /// Workspaces collapsed in the sessions sidebar (labels).
    collapsed_workspaces: std::collections::HashSet<String>,
    /// Path of the active session file, for the sidebar highlight.
    current_session_path: Option<PathBuf>,
    /// When the popup was dismissed by an outside mouse-down; guards against
    /// the same click's mouse-up immediately re-opening it via the chip.
    menu_dismissed_at: Option<Instant>,
}

/// A model choice from the pi runtime catalog.
#[derive(Debug, Clone)]
pub(crate) struct ModelEntry {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) provider: String,
}

impl OrbitApp {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| ComposerInput::new(cx));

        // Spawn pi rooted at the repo; sessions live in the real
        // ~/.pi/agent/sessions so they are shared with the CLI.
        let workspace = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let (client, connect_error) = match PiClient::spawn(&workspace, None) {
            Ok(client) => (Some(client), String::new()),
            Err(err) => (None, format!("pi spawn failed: {err}")),
        };

        let mut app = Self {
            client,
            transcript: Transcript::new(),
            sessions: sessions::load_sessions(),
            sidebar_list: ListState::new(0, ListAlignment::Top, px(80.)),
            sidebar_visible: true,
            input,
            model_label: "…".into(),
            model_provider: String::new(),
            thinking_label: "…".into(),
            busy: false,
            status: connect_error,
            current_title: None,
            current_workspace: None,
            added: 0,
            removed: 0,
            focus: cx.focus_handle(),
            available_models: Vec::new(),
            available_thinking_levels: Vec::new(),
            model_selector: None,
            settings_open: false,
            settings_section: SettingsSection::General,
            session_history: Vec::new(),
            history_index: 0,
            collapsed_workspaces: std::collections::HashSet::new(),
            current_session_path: None,
            menu_dismissed_at: None,
        };

        if app.client.is_some() {
            app.send(CommandBody::GetState, "get_state");
            app.send(CommandBody::GetAvailableModels, "get_available_models");
            app.send(
                CommandBody::GetAvailableThinkingLevels,
                "get_available_thinking_levels",
            );
        }
        app
    }

    fn send(&mut self, body: CommandBody, label: &str) {
        let Some(client) = self.client.as_ref() else {
            self.status = "pi is not running".into();
            return;
        };
        match client.send(body) {
            Ok(_) => self.status = format!("→ {label}"),
            Err(err) => self.status = format!("send failed: {err}"),
        }
    }

    /// Re-fetch the model catalog and thinking levels for the current session.
    fn refresh_catalogs(&mut self) {
        self.send(CommandBody::GetAvailableModels, "get_available_models");
        self.send(
            CommandBody::GetAvailableThinkingLevels,
            "get_available_thinking_levels",
        );
    }

    /// Heartbeat (~90ms): drain protocol events into the UI.
    pub fn tick(&mut self, cx: &mut Context<Self>) {
        let events = {
            let Some(client) = self.client.as_ref() else {
                return;
            };
            client.drain_events()
        };
        if events.is_empty() {
            return;
        }

        let mut refresh_sessions = false;
        for event in &events {
            match event {
                Event::AgentStart => self.busy = true,
                Event::AgentSettled => {
                    self.busy = false;
                    refresh_sessions = true;
                }
                Event::ProcessExited => {
                    self.busy = false;
                    self.status = "pi process exited — restart the app".into();
                }
                Event::MessageEnd { value } => {
                    // Real edit stats from finalized tool calls.
                    let (a, r) = diff_from_message(value);
                    self.added += a;
                    self.removed += r;
                }
                Event::Response {
                    command,
                    success,
                    data,
                    ..
                } => {
                    self.on_response(command, *success, data.as_ref(), &mut refresh_sessions, cx);
                }
                _ => {}
            }
            if self.transcript.apply_event(event) {
                cx.notify();
            }
        }
        if refresh_sessions {
            self.sessions = sessions::load_sessions();
            cx.notify();
        }
        // Responses can change app state without touching the transcript
        // (model/thinking labels, catalogs, status) — always redraw a frame
        // in which events were processed so those changes become visible.
        cx.notify();
    }

    fn on_response(
        &mut self,
        command: &str,
        success: bool,
        data: Option<&serde_json::Value>,
        refresh_sessions: &mut bool,
        cx: &mut Context<Self>,
    ) {
        // Commands whose replies carry no payload (e.g. `set_thinking_level`
        // answers `{"success":true}` with no `data`). They still need their
        // follow-up state refresh, so they are matched BEFORE the
        // data-required branch below.
        match command {
            "set_model" => {
                self.send(CommandBody::GetState, "get_state");
                self.send(
                    CommandBody::GetAvailableThinkingLevels,
                    "get_available_thinking_levels",
                );
                return;
            }
            "cycle_model" | "set_thinking_level" | "cycle_thinking_level" => {
                self.send(CommandBody::GetState, "get_state");
                return;
            }
            _ => {}
        }

        let Some(data) = data else { return };
        match command {
            "get_state" => {
                if let Some(name) = data
                    .get("model")
                    .and_then(|m| m.get("name"))
                    .and_then(serde_json::Value::as_str)
                {
                    self.model_label = name.to_string();
                }
                if let Some(provider) = data
                    .get("model")
                    .and_then(|m| m.get("provider"))
                    .and_then(serde_json::Value::as_str)
                {
                    self.model_provider = provider.to_string();
                }
                if let Some(level) = data
                    .get("thinkingLevel")
                    .and_then(serde_json::Value::as_str)
                {
                    self.thinking_label = level.to_string();
                }
                self.sync_model_selector(cx);
            }
            "get_messages" => {
                self.transcript.load_from(data);
                // Rebuild diff stats from the loaded history.
                self.added = 0;
                self.removed = 0;
                if let Some(messages) = data.get("messages").and_then(serde_json::Value::as_array) {
                    for message in messages {
                        let (a, r) = diff_from_message(message);
                        self.added += a;
                        self.removed += r;
                    }
                }
            }
            "get_available_models" => {
                self.available_models = data
                    .get("models")
                    .and_then(serde_json::Value::as_array)
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|m| {
                                let id = m.get("id").and_then(serde_json::Value::as_str)?;
                                let name = m.get("name").and_then(serde_json::Value::as_str)?;
                                let provider =
                                    m.get("provider").and_then(serde_json::Value::as_str)?;
                                Some(ModelEntry {
                                    id: id.into(),
                                    name: name.into(),
                                    provider: provider.into(),
                                })
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                self.sync_model_selector(cx);
            }
            "get_available_thinking_levels" => {
                self.available_thinking_levels = data
                    .get("levels")
                    .and_then(serde_json::Value::as_array)
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(str::to_string))
                            .collect()
                    })
                    .unwrap_or_default();
                self.sync_model_selector(cx);
            }
            "switch_session" => {
                if success {
                    self.send(CommandBody::GetMessages, "get_messages");
                    self.refresh_catalogs();
                }
            }
            "new_session" => {
                self.transcript.clear();
                self.current_title = None;
                self.current_workspace = None;
                self.current_session_path = None;
                self.added = 0;
                self.removed = 0;
                *refresh_sessions = true;
                self.send(CommandBody::GetState, "get_state");
                self.refresh_catalogs();
            }
            _ => {
                let _ = success;
            }
        }
    }

    // ── actions ────────────────────────────────────────────────────────────

    fn submit(&mut self, text: String, cx: &mut Context<Self>) {
        let text = text.trim().to_string();
        if text.is_empty() {
            return;
        }
        self.send(
            CommandBody::Prompt {
                message: text,
                images: None,
                streaming_behavior: None,
            },
            "prompt",
        );
        self.input.update(cx, |input, cx| input.clear(cx));
        cx.notify();
    }

    fn on_submit(&mut self, _: &crate::Submit, _: &mut Window, cx: &mut Context<Self>) {
        let text = self.input.read(cx).text();
        self.submit(text, cx);
    }

    fn on_send_click(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        let text = self.input.read(cx).text();
        self.submit(text, cx);
    }

    fn on_abort(&mut self, _: &crate::AbortRun, window: &mut Window, cx: &mut Context<Self>) {
        // Escape backs out of the topmost surface: settings first, then
        // popovers, then a running agent.
        if self.settings_open {
            self.settings_open = false;
            cx.notify();
            return;
        }
        if self.model_selector.is_some() {
            self.close_model_selector(window, cx);
            return;
        }
        self.send(CommandBody::Abort, "abort");
        cx.notify();
    }

    fn on_abort_mouse(&mut self, _: &MouseUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.on_abort(&crate::AbortRun, window, cx);
    }

    fn on_new_session(&mut self, _: &crate::NewSession, _: &mut Window, cx: &mut Context<Self>) {
        self.send(CommandBody::NewSession, "new_session");
        cx.notify();
    }

    /// Toggle one of the two composer dropdowns (model / thinking). Opening
    /// one closes the other; clicking the open chip closes it. Re-checks the
    /// catalog on open so the list always reflects the live pi session.
    fn toggle_picker(&mut self, kind: PickerKind, window: &mut Window, cx: &mut Context<Self>) {
        if self.picker_is_open(kind) {
            self.close_model_selector(window, cx);
            return;
        }
        if self.model_selector.is_some() {
            self.close_model_selector(window, cx);
        }
        self.open_picker(kind, window, cx);
    }

    fn picker_is_open(&self, kind: PickerKind) -> bool {
        matches!(&self.model_selector, Some((open_kind, _)) if *open_kind == kind)
    }

    fn open_picker(&mut self, kind: PickerKind, window: &mut Window, cx: &mut Context<Self>) {
        self.refresh_catalogs();

        // The popup talks back exclusively through these callbacks; it never
        // borrows app state.
        let this = cx.weak_entity();
        let on_select_model = Box::new(
            move |id: &str, provider: &str, window: &mut Window, cx: &mut App| {
                this.update(cx, |app, cx| {
                    app.set_model(id.to_string(), provider.to_string(), cx);
                    app.close_model_selector(window, cx);
                })
                .ok();
            },
        ) as Box<dyn Fn(&str, &str, &mut Window, &mut App)>;
        let this = cx.weak_entity();
        let on_select_level = Box::new(move |level: &str, window: &mut Window, cx: &mut App| {
            this.update(cx, |app, cx| {
                app.set_thinking_level(level.to_string(), cx);
                app.close_model_selector(window, cx);
            })
            .ok();
        }) as Box<dyn Fn(&str, &mut Window, &mut App)>;
        let this = cx.weak_entity();
        let on_dismiss = Box::new(move |by_mouse: bool, window: &mut Window, cx: &mut App| {
            this.update(cx, |app, cx| {
                // Only mouse dismissals arm the chip's click-through guard.
                if by_mouse {
                    app.menu_dismissed_at = Some(Instant::now());
                }
                app.close_model_selector(window, cx);
            })
            .ok();
        }) as Box<dyn Fn(bool, &mut Window, &mut App)>;

        let selector = cx.new(|cx| {
            ModelSelector::new(
                kind,
                self.available_models.clone(),
                self.available_thinking_levels.clone(),
                self.model_label.clone(),
                self.thinking_label.clone(),
                on_select_model,
                on_select_level,
                on_dismiss,
                cx,
            )
        });
        // Focus the popup's filter input so typing filters immediately.
        window.focus(&selector.read(cx).focus_handle(cx));
        self.model_selector = Some((kind, selector));
        cx.notify();
    }

    /// Drop the popup (if open) and put focus back on the composer.
    fn close_model_selector(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.model_selector.take().is_some() {
            self.input.read(cx).focus(window);
            cx.notify();
        }
    }

    /// Push the latest catalog/current-selection snapshot into the open
    /// popup (no-op while it is closed).
    fn sync_model_selector(&mut self, cx: &mut Context<Self>) {
        if let Some((_, selector)) = &self.model_selector {
            let models = self.available_models.clone();
            let levels = self.available_thinking_levels.clone();
            let model = self.model_label.clone();
            let level = self.thinking_label.clone();
            selector.update(cx, |selector, cx| {
                selector.set_catalog(models, levels, model, level, cx)
            });
        }
    }

    /// Mouse-up on a chip that opens the given picker. If that picker was
    /// just dismissed by this click's mouse-down (outside-click dismissal),
    /// swallow the toggle so it stays closed.
    fn on_chip_trigger_click(
        &mut self,
        kind: PickerKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        const GESTURE: Duration = Duration::from_millis(200);
        if let Some(dismissed) = self.menu_dismissed_at.take() {
            if dismissed.elapsed() < GESTURE {
                return;
            }
        }
        self.toggle_picker(kind, window, cx);
    }

    fn on_model_trigger_click(
        &mut self,
        _: &MouseUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.on_chip_trigger_click(PickerKind::Model, window, cx);
    }

    fn on_thinking_trigger_click(
        &mut self,
        _: &MouseUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.on_chip_trigger_click(PickerKind::Thinking, window, cx);
    }

    fn set_model(&mut self, id: String, provider: String, cx: &mut Context<Self>) {
        self.send(
            CommandBody::SetModel {
                model_id: id,
                provider,
            },
            "set_model",
        );
        cx.notify();
    }

    fn set_thinking_level(&mut self, level: String, cx: &mut Context<Self>) {
        self.send(
            CommandBody::SetThinkingLevel { level },
            "set_thinking_level",
        );
        cx.notify();
    }

    fn on_refresh(&mut self, _: &crate::RefreshSessions, _: &mut Window, cx: &mut Context<Self>) {
        self.sessions = sessions::load_sessions();
        cx.notify();
    }

    fn on_open_session(&mut self, session: SessionInfo, cx: &mut Context<Self>) {
        self.switch_to_session(session, true, cx);
    }

    /// Switch the live session. With `push`, the visit is recorded in the
    /// top-bar history (forward entries are dropped, like browser history).
    /// Navigation itself calls this with `push: false`.
    fn switch_to_session(&mut self, session: SessionInfo, push: bool, cx: &mut Context<Self>) {
        self.send(
            CommandBody::SwitchSession {
                session_path: session.path.to_string_lossy().into_owned(),
            },
            "switch_session",
        );
        self.transcript.clear();
        self.current_title = Some(session.title.clone());
        self.current_workspace = Some(session.cwd.clone());
        self.current_session_path = Some(session.path.clone());
        if push {
            self.session_history.truncate(self.history_index + 1);
            let new_entry = self
                .session_history
                .last()
                .map(|last| last.path != session.path)
                .unwrap_or(true);
            if new_entry {
                self.session_history.push(session);
            }
            self.history_index = self.session_history.len().saturating_sub(1);
        }
        cx.notify();
    }

    fn on_history_back(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.history_index > 0 {
            self.history_index -= 1;
            if let Some(session) = self.session_history.get(self.history_index).cloned() {
                self.switch_to_session(session, false, cx);
                return;
            }
        }
        cx.notify();
    }

    fn on_history_forward(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.history_index + 1 < self.session_history.len() {
            self.history_index += 1;
            if let Some(session) = self.session_history.get(self.history_index).cloned() {
                self.switch_to_session(session, false, cx);
                return;
            }
        }
        cx.notify();
    }

    fn on_info_click(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        // The top-bar info affordance opens Settings → About.
        self.settings_open = true;
        self.settings_section = SettingsSection::About;
        cx.notify();
    }

    fn on_toggle_sidebar(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.sidebar_visible = !self.sidebar_visible;
        cx.notify();
    }

    fn on_composer_click(&mut self, _: &MouseUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.input.read(cx).focus(window);
    }

    // ── settings ──────────────────────────────────────────────────────

    fn open_settings(&mut self, cx: &mut Context<Self>) {
        self.settings_open = true;
        self.settings_section = SettingsSection::General;
        cx.notify();
    }

    fn on_open_settings(
        &mut self,
        _: &crate::OpenSettings,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_settings(cx);
    }

    fn on_settings_gear_click(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        // Toggle: the gear sits in the sessions sidebar, which stays visible
        // while settings is open, so clicking it again should go back.
        if self.settings_open {
            self.settings_open = false;
        } else {
            self.open_settings(cx);
            return;
        }
        cx.notify();
    }

    fn on_settings_back(&mut self, _: &MouseUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.settings_open = false;
        self.input.read(cx).focus(window);
        cx.notify();
    }

    // ── labels ─────────────────────────────────────────────────────────────

    fn workspace_label(&self) -> String {
        self.current_workspace
            .as_ref()
            .map(|p| sessions::workspace_label(p))
            .unwrap_or_else(|| {
                std::env::current_dir()
                    .ok()
                    .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
                    .unwrap_or_else(|| "workspace".into())
            })
    }
}

impl Focusable for OrbitApp {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

// ── diff stats (real, from tool calls) ─────────────────────────────────────

/// Count added/removed lines from the `edit` / `write` tool calls inside a
/// message value. Counts are honest — they only reflect what the agent did.
fn diff_from_message(value: &serde_json::Value) -> (u64, u64) {
    let mut added = 0u64;
    let mut removed = 0u64;
    let Some(serde_json::Value::Array(blocks)) = value
        .get("message")
        .and_then(|m| m.get("content"))
        .or_else(|| value.get("content"))
    else {
        return (0, 0);
    };
    for block in blocks {
        let kind = block
            .get("type")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        if kind != "toolCall" && kind != "tool_call" {
            continue;
        }
        let name = block
            .get("name")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        let args = block.get("arguments").unwrap_or(&serde_json::Value::Null);
        match name {
            "edit" => {
                if let Some(edits) = args.get("edits").and_then(serde_json::Value::as_array) {
                    for edit in edits {
                        removed += count_lines(edit.get("oldText"));
                        added += count_lines(edit.get("newText"));
                    }
                }
            }
            "write" => {
                added += count_lines(args.get("content"));
            }
            _ => {}
        }
    }
    (added, removed)
}

fn count_lines(value: Option<&serde_json::Value>) -> u64 {
    value
        .and_then(serde_json::Value::as_str)
        .map(|text| text.split('\n').count() as u64)
        .unwrap_or(0)
}

// ── sidebar model ──────────────────────────────────────────────────────────

enum SideRow {
    /// Workspace group header: label, session count, collapsed state.
    Workspace {
        label: String,
        count: usize,
        collapsed: bool,
    },
    /// Session row — index into the (newest-first) sessions list.
    Session(usize),
}

/// Sections of the settings surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SettingsSection {
    General,
    Appearance,
    Providers,
    About,
}

impl Render for OrbitApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Flatten sidebar rows: workspace groups (ordered by each group's
        // most recent session — the list arrives newest-first) + their
        // sessions, time-sorted, skipping collapsed groups.
        let mut side_rows: Vec<SideRow> = Vec::new();
        let mut groups: Vec<(String, Vec<usize>)> = Vec::new();
        for (ix, session) in self.sessions.iter().enumerate() {
            let label = sessions::workspace_label(&session.cwd);
            match groups.iter_mut().find(|(l, _)| *l == label) {
                Some((_, ixs)) => ixs.push(ix),
                None => groups.push((label, vec![ix])),
            }
        }
        for (label, ixs) in groups {
            let collapsed = self.collapsed_workspaces.contains(&label);
            side_rows.push(SideRow::Workspace {
                label,
                count: ixs.len(),
                collapsed,
            });
            if !collapsed {
                for ix in ixs {
                    side_rows.push(SideRow::Session(ix));
                }
            }
        }
        let old = self.sidebar_list.item_count();
        if old != side_rows.len() {
            self.sidebar_list.splice(0..old, side_rows.len());
        }
        let side_rows = Rc::new(side_rows);
        let sessions_data = Rc::new(self.sessions.clone());
        let active_path = Rc::new(self.current_session_path.clone());
        let this = cx.entity();

        let workspace_label = self.workspace_label();

        // ── top-bar left controls: sidebar toggle + session history ──
        let back_enabled = self.history_index > 0;
        let forward_enabled = self.history_index + 1 < self.session_history.len();
        let left_controls = div()
            .flex()
            .items_center()
            .gap_1()
            .pr(px(6.))
            .child(
                div()
                    .id("toggle-sidebar")
                    .p_1()
                    .rounded_sm()
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(BG_HOVER)))
                    .on_mouse_up(MouseButton::Left, cx.listener(Self::on_toggle_sidebar))
                    .child(icon("icons/panel-left.svg", 16., TEXT_2)),
            )
            .child(
                div()
                    .id("history-back")
                    .p_1()
                    .rounded_sm()
                    .when(back_enabled, |b| {
                        b.cursor_pointer()
                            .hover(|s| s.bg(rgb(BG_HOVER)))
                            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_history_back))
                    })
                    .child(icon(
                        "icons/arrow-left.svg",
                        14.,
                        if back_enabled { TEXT_2 } else { TEXT_3 },
                    )),
            )
            .child(
                div()
                    .id("history-forward")
                    .p_1()
                    .rounded_sm()
                    .when(forward_enabled, |b| {
                        b.cursor_pointer()
                            .hover(|s| s.bg(rgb(BG_HOVER)))
                            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_history_forward))
                    })
                    .child(icon(
                        "icons/arrow-right.svg",
                        14.,
                        if forward_enabled { TEXT_2 } else { TEXT_3 },
                    )),
            );

        // ── top-bar right controls ──
        let mut top_controls = div().flex().items_center().gap_2();
        top_controls = top_controls
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(rgb(ADD_GREEN))
                    .child(format!("+{}", self.added)),
            )
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(rgb(DEL_RED))
                    .child(format!("-{}", self.removed)),
            );
        top_controls = top_controls
            // model selector pill (chat icon | chevron)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .rounded_md()
                    .bg(rgb(BG_RAISED))
                    .border_1()
                    .border_color(rgb(BORDER))
                    .px(px(6.))
                    .py(px(4.))
                    .text_size(px(12.))
                    .text_color(rgb(TEXT_2))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(BG_HOVER)))
                    .on_mouse_up(MouseButton::Left, cx.listener(Self::on_model_trigger_click))
                    .child(icon_dyn(provider_icon(&self.model_provider), 13., TEXT_2))
                    .child(div().w(px(1.)).h(px(12.)).bg(rgb(BORDER)))
                    .child(icon("icons/chevron-down.svg", 12., TEXT_3)),
            )
            .child(
                div()
                    .id("info")
                    .p_1()
                    .rounded_sm()
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(BG_HOVER)))
                    .on_mouse_up(MouseButton::Left, cx.listener(Self::on_info_click))
                    .child(icon("icons/info.svg", 16., TEXT_2)),
            );

        div()
            .size_full()
            .flex()
            .bg(rgb(BG_MAIN))
            .text_color(rgb(TEXT))
            // ── sidebar ── (hidden while the settings surface is open —
            // settings is a full-window surface with its own nav, like the
            // reference UI)
            .children((self.sidebar_visible && !self.settings_open).then(|| {
                div()
                    .w(px(SIDEBAR_W))
                    .h_full()
                    .bg(rgb(BG_SIDEBAR))
                    .border_r_1()
                    .border_color(rgb(BORDER))
                    .flex()
                    .flex_col()
                    // traffic-light strip (drag region)
                    .child(
                        div()
                            .h(px(38.))
                            .w_full()
                            .window_control_area(WindowControlArea::Drag),
                    )
                    // nav
                    .child(
                        div()
                            .px_2()
                            .pt_1()
                            .flex()
                            .flex_col()
                            .gap_0p5()
                            .child(self.nav_row(
                                "icons/task.svg",
                                "New Task",
                                false,
                                Some(Box::new(cx.listener(|this, _: &MouseUpEvent, w, cx| {
                                    this.on_new_session(&crate::NewSession, w, cx)
                                }))),
                            ))
                            .child(self.nav_row("icons/search.svg", "Search", true, None)),
                    )
                    // session list (scrolls), grouped by age
                    .child(
                        div()
                            .id("sidebar-sessions")
                            .flex_1()
                            .min_h_0()
                            .px_2()
                            .relative()
                            .child(
                                list(self.sidebar_list.clone(), move |ix, _window, _cx| {
                                    render_side_row(
                                        &side_rows,
                                        &sessions_data,
                                        active_path.as_deref(),
                                        ix,
                                        &this,
                                    )
                                    .into_any_element()
                                })
                                .w_full()
                                .h_full(),
                            ),
                    )
                    // footer — settings gear + connection dot
                    .child(
                        div()
                            .h(px(40.))
                            .px_3()
                            .flex()
                            .items_center()
                            .child(
                                div()
                                    .id("settings")
                                    .p_1()
                                    .rounded_sm()
                                    .cursor_pointer()
                                    .hover(|s| s.bg(rgb(BG_HOVER)))
                                    .on_mouse_up(
                                        MouseButton::Left,
                                        cx.listener(Self::on_settings_gear_click),
                                    )
                                    .child(icon("icons/settings.svg", 16., TEXT_3)),
                            )
                            .child(div().flex_1())
                            .child(div().size(px(7.)).rounded_full().bg(rgb(
                                if self.client.is_some() {
                                    OK_GREEN
                                } else {
                                    STOP_RED
                                },
                            ))),
                    )
            }))
            // ── main ──
            .child(if self.settings_open {
                self.render_settings(cx).into_any_element()
            } else {
                div()
                    .flex_1()
                    .h_full()
                    .flex()
                    .flex_col()
                    .min_h_0()
                    // top bar — left controls clear the traffic lights when
                    // the sessions sidebar is hidden; the drag spacer between
                    // left controls and the right cluster drags the window
                    .child(
                        div()
                            .h(px(44.))
                            .w_full()
                            .flex()
                            .items_center()
                            .pl(px(if self.sidebar_visible { 20. } else { 76. }))
                            .pr(px(12.))
                            .child(left_controls)
                            .child(
                                div()
                                    .flex_1()
                                    .h_full()
                                    .window_control_area(WindowControlArea::Drag)
                                    .flex()
                                    .items_center()
                                    .child(
                                        div().text_size(px(13.)).text_color(rgb(TEXT_2)).child(
                                            self.current_title
                                                .clone()
                                                .unwrap_or_else(|| "New task".into()),
                                        ),
                                    ),
                            )
                            .child(top_controls),
                    )
                    // transcript (centered column) or empty state
                    .child(if self.transcript.is_empty() {
                        empty_state(&workspace_label).into_any_element()
                    } else {
                        div()
                            .flex_1()
                            .min_h_0()
                            .w_full()
                            .flex()
                            .justify_center()
                            .child(self.transcript.render())
                            .into_any_element()
                    })
                    // floating composer + status bar — one centered column
                    .child(
                        div()
                            .w_full()
                            .flex()
                            .flex_col()
                            .items_center()
                            .px_4()
                            .pb_4()
                            // One centered column: composer + status bar share
                            // the same max width so the folder/meta row always
                            // aligns to the composer's edges.
                            .child(
                                div()
                                    .max_w(px(CONTENT_MAX_W))
                                    .w_full()
                                    .flex()
                                    .flex_col()
                                    // composer box — the picker popups are
                                    // anchored above their own chips
                                    .child(
                                        div()
                                            .w_full()
                                            .bg(rgb(BG_COMPOSER))
                                            .border_1()
                                            .border_color(rgb(BORDER))
                                            .rounded_lg()
                                            .px_3()
                                            .pt_2()
                                            .pb_2()
                                            .flex()
                                            .flex_col()
                                            .gap_2()
                                            .on_mouse_up(
                                                MouseButton::Left,
                                                cx.listener(Self::on_composer_click),
                                            )
                                            .child(self.input.clone())
                                            .child(self.composer_row(cx)),
                                    )
                                    .child(self.status_bar(&workspace_label)),
                            ),
                    )
                    .into_any_element()
            })
            .track_focus(&self.focus_handle(cx))
            .on_action(cx.listener(Self::on_submit))
            .on_action(cx.listener(Self::on_abort))
            .on_action(cx.listener(Self::on_refresh))
            .on_action(cx.listener(Self::on_open_settings))
    }
}

impl OrbitApp {
    /// Bottom row inside the composer: separate model and thinking-level
    /// chips (each opens its own dropdown anchored above it), full-access
    /// pill, and the round send button.
    fn composer_row(&self, cx: &Context<Self>) -> impl IntoElement + use<> {
        div()
            .flex()
            .items_center()
            .gap_2()
            .child(self.model_chip(cx))
            .child(self.thinking_chip(cx))
            // access mode (pi runs with full tool access)
            .child(pill_static("icons/lock.svg", "Full access"))
            .child(div().flex_1())
            .child(self.send_button(cx))
    }

    /// The anchored popup for `kind`, when that picker is open. `deferred`
    /// paints it on top of everything; `anchored` takes it out of the layout
    /// and pins its bottom-left corner just above the chip, flipping at the
    /// window edges via `snap_to_window`.
    fn chip_popup(&self, kind: PickerKind) -> Option<impl IntoElement + use<>> {
        self.model_selector
            .clone()
            .and_then(|(open_kind, selector)| {
                (open_kind == kind).then(move || {
                    anchored()
                        .position_mode(AnchoredPositionMode::Local)
                        .anchor(Corner::BottomLeft)
                        .offset(point(px(0.), px(-4.)))
                        .snap_to_window()
                        .child(deferred(selector))
                })
            })
    }

    /// The model chip: chat glyph + model name + caret. Highlighted while
    /// its dropdown is open.
    fn model_chip(&self, cx: &Context<Self>) -> impl IntoElement + use<> {
        div()
            .flex()
            .flex_col()
            .items_start()
            .children(self.chip_popup(PickerKind::Model))
            .child(
                div()
                    .id("model-chip")
                    .flex()
                    .items_center()
                    .gap_1p5()
                    .px(px(7.))
                    .h(px(24.))
                    .rounded_md()
                    .border_1()
                    .text_size(px(12.))
                    .cursor_pointer()
                    .border_color(rgb(BORDER))
                    .bg(rgb(BG_RAISED))
                    .hover(|s| s.bg(overlay()))
                    .when(self.picker_is_open(PickerKind::Model), |chip| {
                        chip.bg(overlay_strong()).border_color(border_strong())
                    })
                    .on_mouse_up(MouseButton::Left, cx.listener(Self::on_model_trigger_click))
                    .child(icon_dyn(provider_icon(&self.model_provider), 12., TEXT_2))
                    .child(div().text_color(rgb(TEXT)).child(self.model_label.clone()))
                    .child(icon("icons/chevron-down.svg", 11., TEXT_3)),
            )
    }

    /// The thinking-level chip: spark glyph + reasoning level + caret.
    fn thinking_chip(&self, cx: &Context<Self>) -> impl IntoElement + use<> {
        div()
            .flex()
            .flex_col()
            .items_start()
            .children(self.chip_popup(PickerKind::Thinking))
            .child(
                div()
                    .id("thinking-chip")
                    .flex()
                    .items_center()
                    .gap_1p5()
                    .px(px(7.))
                    .h(px(24.))
                    .rounded_md()
                    .border_1()
                    .text_size(px(12.))
                    .cursor_pointer()
                    .border_color(rgb(BORDER))
                    .bg(rgb(BG_RAISED))
                    .hover(|s| s.bg(overlay()))
                    .when(self.picker_is_open(PickerKind::Thinking), |chip| {
                        chip.bg(overlay_strong()).border_color(border_strong())
                    })
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(Self::on_thinking_trigger_click),
                    )
                    .child({
                        let (path, color) = thinking_icon(&self.thinking_label);
                        icon(path, 12., color)
                    })
                    .child(
                        div()
                            .text_color(rgb(TEXT))
                            .child(thinking_display(&self.thinking_label)),
                    )
                    .child(icon("icons/chevron-down.svg", 11., TEXT_3)),
            )
    }

    /// Status bar under the composer: workspace / transport / branch on the
    /// left, run-state indicator on the right. Aligned to the composer edges.
    /// Uses a folder icon for the workspace/project to match the reference UI.
    fn status_bar(&self, workspace_label: &str) -> impl IntoElement + use<> {
        let cwd = self
            .current_workspace
            .clone()
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_default();
        div()
            .pt_1p5()
            .w_full()
            .flex()
            .items_center()
            .gap_4()
            .text_size(px(11.5))
            .text_color(rgb(TEXT_3))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1p5()
                    .child(icon("icons/folder.svg", 12., TEXT_3))
                    .child(workspace_label.to_string()),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1p5()
                    .child(icon("icons/monitor.svg", 12., TEXT_3))
                    .child("Local"),
            )
            .children(git_branch(&cwd).map(|branch| {
                div()
                    .flex()
                    .items_center()
                    .gap_1p5()
                    .child(icon("icons/branch.svg", 12., TEXT_3))
                    .child(branch)
            }))
            .child(div().flex_1())
            .child(self.run_indicator())
    }

    /// Run-state indicator: hollow circle while idle, orange spark while the
    /// agent runs, red dot if the pi process is gone.
    fn run_indicator(&self) -> impl IntoElement + use<> {
        if !self.client.is_some() {
            return div()
                .size(px(9.))
                .rounded_full()
                .bg(rgb(STOP_RED))
                .into_any_element();
        }
        if self.busy {
            return icon("icons/spark.svg", 12., SPARK_ORANGE).into_any_element();
        }
        div()
            .size(px(9.))
            .rounded_full()
            .border_1()
            .border_color(rgb(TEXT_3))
            .into_any_element()
    }

    fn nav_row(
        &self,
        icon_path: &'static str,
        label: &str,
        dimmed: bool,
        on_click: Option<Box<dyn Fn(&MouseUpEvent, &mut Window, &mut App) + 'static>>,
    ) -> impl IntoElement + use<> {
        let (color, icon_color) = if dimmed {
            (TEXT_3, TEXT_3)
        } else {
            (TEXT, TEXT_2)
        };
        let mut row = div()
            .w_full()
            .px_2()
            .py(px(5.))
            .rounded_md()
            .text_size(px(13.))
            .flex()
            .items_center()
            .gap_2()
            .hover(|s| s.bg(rgb(BG_HOVER)))
            .child(icon(icon_path, 16., icon_color))
            .child(div().text_color(rgb(color)).child(label.to_string()));

        if let Some(handler) = on_click {
            row = row.cursor_pointer().on_mouse_up(MouseButton::Left, handler);
        }
        row
    }

    fn send_button(&self, cx: &Context<Self>) -> impl IntoElement + use<> {
        if self.busy {
            div()
                .id("stop-btn")
                .size(px(28.))
                .rounded_full()
                .bg(rgb(STOP_RED))
                .hover(|s| s.bg(rgb(0xD06060)))
                .cursor_pointer()
                .flex()
                .items_center()
                .justify_center()
                .text_color(rgb(TEXT))
                .on_mouse_up(MouseButton::Left, cx.listener(Self::on_abort_mouse))
                .child(icon("icons/stop.svg", 12., TEXT))
        } else {
            div()
                .id("send-btn")
                .size(px(28.))
                .rounded_full()
                .bg(rgb(0x2A2A2C))
                .hover(|s| s.bg(rgb(0x343438)))
                .cursor_pointer()
                .flex()
                .items_center()
                .justify_center()
                .text_color(rgb(TEXT_2))
                .on_mouse_up(MouseButton::Left, cx.listener(Self::on_send_click))
                .child(icon("icons/send.svg", 14., TEXT))
        }
    }
}

impl OrbitApp {
    // ── settings surface ───────────────────────────────────────────
    // Waku-style: left nav (Back + sections), right column of setting
    // rows. Every control maps to real app state; read-only rows show
    // real pi/runtime facts (PRODUCT.md: nothing decorative that
    // pretends to be functional).

    fn render_settings(&self, cx: &Context<Self>) -> impl IntoElement + use<> {
        let this = cx.entity();
        let sections: [(SettingsSection, &'static str, &'static str); 4] = [
            (SettingsSection::General, "icons/settings.svg", "General"),
            (
                SettingsSection::Appearance,
                "icons/contrast.svg",
                "Appearance",
            ),
            (SettingsSection::Providers, "icons/cloud.svg", "Providers"),
            (SettingsSection::About, "icons/info.svg", "About"),
        ];

        div()
            .flex_1()
            .min_h_0()
            .flex()
            .bg(rgb(BG_MAIN))
            // ── nav column ──
            .child(
                div()
                    .w(px(240.))
                    .h_full()
                    .flex_shrink_0()
                    .bg(rgb(BG_SIDEBAR))
                    .border_r_1()
                    .border_color(rgb(BORDER))
                    .flex()
                    .flex_col()
                    // traffic-light strip (drag region)
                    .child(
                        div()
                            .h(px(38.))
                            .w_full()
                            .window_control_area(WindowControlArea::Drag),
                    )
                    // Back — inset like the sessions nav, breathing room below
                    .child(
                        div().px_2().pt_1().pb_3().child(
                            div()
                                .w_full()
                                .px_2()
                                .py(px(5.))
                                .rounded_md()
                                .flex()
                                .items_center()
                                .gap_1p5()
                                .cursor_pointer()
                                .hover(|s| s.bg(rgb(BG_HOVER)))
                                .on_mouse_up(MouseButton::Left, cx.listener(Self::on_settings_back))
                                .child(icon("icons/arrow-left.svg", 14., TEXT_2))
                                .child(
                                    div()
                                        .text_size(px(13.))
                                        .text_color(rgb(TEXT_2))
                                        .child("Back"),
                                ),
                        ),
                    )
                    // section rows — inset wrapper so hover/selected pills
                    // don't bleed to the window edge (matches sessions nav)
                    .child(
                        div()
                            .px_2()
                            .flex()
                            .flex_col()
                            .gap_0p5()
                            .children(sections.map(|(section, section_icon, label)| {
                                let this = this.clone();
                                let selected = self.settings_section == section;
                                div()
                                    .w_full()
                                    .px_2()
                                    .py(px(5.))
                                    .rounded_md()
                                    .text_size(px(13.))
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .cursor_pointer()
                                    .when(selected, |row| row.bg(rgb(BG_RAISED)))
                                    .when(!selected, |row| row.hover(|s| s.bg(rgb(BG_HOVER))))
                                    .on_mouse_up(MouseButton::Left, move |_, _, cx| {
                                        this.update(cx, |app, cx| {
                                            app.settings_section = section;
                                            cx.notify();
                                        });
                                    })
                                    .child(icon(
                                        section_icon,
                                        15.,
                                        if selected { TEXT } else { TEXT_3 },
                                    ))
                                    .child(
                                        div()
                                            .text_color(rgb(if selected { TEXT } else { TEXT_2 }))
                                            .child(label.to_string()),
                                    )
                            })),
                    ),
            )
            // ── content column (scrolls) ──
            .child(
                div()
                    .id("settings-content")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(
                        div()
                            .max_w(px(680.))
                            .mx_auto()
                            .px(px(12.))
                            .pt(px(44.))
                            .pb(px(12.))
                            .flex()
                            .flex_col()
                            .gap_3()
                            .child(self.settings_header())
                            .children(self.settings_rows(&this)),
                    ),
            )
    }

    fn settings_header(&self) -> impl IntoElement + use<> {
        let (title, subtitle) = match self.settings_section {
            SettingsSection::General => (
                "General",
                "How Orbit connects to the pi agent and stores your data.",
            ),
            SettingsSection::Appearance => ("Appearance", "Window and layout preferences."),
            SettingsSection::Providers => (
                "Providers",
                "Models and providers advertised by the running pi agent.",
            ),
            SettingsSection::About => ("About", "Versions and the rendering stack."),
        };
        div()
            .flex()
            .flex_col()
            .gap_1()
            .pb_1()
            .child(
                div()
                    .text_size(px(20.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(TEXT))
                    .child(title.to_string()),
            )
            .child(
                div()
                    .text_size(px(12.5))
                    .text_color(rgb(TEXT_2))
                    .child(subtitle.to_string()),
            )
    }

    /// The rows for the active section, as card elements. `this` rides
    /// along for closures in interactive controls (rows themselves are
    /// built read-only from app state).
    fn settings_rows(&self, this: &Entity<OrbitApp>) -> Vec<AnyElement> {
        match self.settings_section {
            SettingsSection::General => vec![
                self.card(
                    "pi agent",
                    "Spawned as a child process — newline-delimited JSON over stdio.",
                    Some(self.connection_status()),
                ),
                self.card_with_path(
                    "Local by default",
                    "Sessions live in pi's own store on this computer — no daemon, no cloud.",
                    Some(&sessions::sessions_dir().to_string_lossy()),
                    None,
                ),
                self.card_with_path(
                    "Workspace",
                    "New tasks start in this directory.",
                    Some(
                        &self
                            .current_workspace
                            .clone()
                            .or_else(|| std::env::current_dir().ok())
                            .unwrap_or_default()
                            .to_string_lossy(),
                    ),
                    None,
                ),
            ],
            SettingsSection::Appearance => vec![
                self.card(
                    "Show sidebar",
                    "Show the sessions sidebar. Also toggleable from the top bar.",
                    Some(self.sidebar_toggle(this.clone())),
                ),
                self.card(
                    "GPU-rendered streaming",
                    "Stream commits are coalesced (~8 Hz) and highlighting is paint-only, so long tasks never reflow the transcript.",
                    None,
                ),
            ],
            SettingsSection::Providers => self.provider_rows(),
            SettingsSection::About => vec![
                self.card(
                    "Orbit Pi",
                    "Native workbench for the pi coding agent.",
                    Some(
                        div()
                            .text_size(px(12.))
                            .text_color(rgb(TEXT_2))
                            .child(format!("v{}", env!("CARGO_PKG_VERSION")))
                            .into_any_element(),
                    ),
                ),
                self.card(
                    "GPUI",
                    "GPU-accelerated UI framework (pinned; runtime shaders).",
                    Some(
                        div()
                            .text_size(px(12.))
                            .text_color(rgb(TEXT_2))
                            .child("0.2.2")
                            .into_any_element(),
                    ),
                ),
                self.card(
                    "pi CLI",
                    "The only agent runtime — pi speaks its own RPC protocol over stdio.",
                    Some(self.connection_status()),
                ),
            ],
        }
    }

    /// Providers from the live pi catalog, deduplicated in catalog order.
    fn provider_rows(&self) -> Vec<AnyElement> {
        if self.available_models.is_empty() {
            return vec![self.card(
                "No models in the catalog",
                if self.client.is_some() {
                    "The running pi agent hasn't advertised any models yet."
                } else {
                    "The pi agent is not running — restart Orbit to reconnect."
                },
                None,
            )];
        }
        let mut providers: Vec<(String, usize)> = Vec::new();
        for model in &self.available_models {
            match providers.iter_mut().find(|(p, _)| *p == model.provider) {
                Some((_, count)) => *count += 1,
                None => providers.push((model.provider.clone(), 1)),
            }
        }
        providers
            .into_iter()
            .map(|(provider, count)| {
                div()
                    .w_full()
                    .bg(rgb(BG_COMPOSER))
                    .border_1()
                    .border_color(rgb(BORDER))
                    .rounded_lg()
                    .px(px(14.))
                    .py(px(10.))
                    .flex()
                    .items_center()
                    .gap_2p5()
                    .child(icon_dyn(provider_icon(&provider), 14., TEXT_2))
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(rgb(TEXT))
                            .child(provider.clone()),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(rgb(TEXT_2))
                            .child(format!(
                                "{} model{}",
                                count,
                                if count == 1 { "" } else { "s" }
                            )),
                    )
                    .into_any_element()
            })
            .collect()
    }

    /// A setting card: title + description on the left, optional control on
    /// the right.
    fn card(&self, title: &str, desc: &str, control: Option<AnyElement>) -> AnyElement {
        self.card_with_path(title, desc, None, control)
    }

    /// Same as [`card`] with an optional dimmed third line (paths).
    fn card_with_path(
        &self,
        title: &str,
        desc: &str,
        path: Option<&str>,
        control: Option<AnyElement>,
    ) -> AnyElement {
        div()
            .w_full()
            .bg(rgb(BG_COMPOSER))
            .border_1()
            .border_color(rgb(BORDER))
            .rounded_lg()
            .px(px(14.))
            .py(px(12.))
            .flex()
            .items_center()
            .gap_3()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_size(px(13.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(TEXT))
                            .child(title.to_string()),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(rgb(TEXT_2))
                            .child(desc.to_string()),
                    )
                    .children(path.map(|p| {
                        div()
                            .text_size(px(11.5))
                            .text_color(rgb(TEXT_3))
                            .truncate()
                            .child(p.to_string())
                    })),
            )
            .children(control)
            .into_any_element()
    }

    /// Connection state: green dot + "Connected" / red dot + "Not running".
    fn connection_status(&self) -> AnyElement {
        div()
            .flex()
            .items_center()
            .gap_1p5()
            .child(
                div()
                    .size(px(7.))
                    .rounded_full()
                    .bg(rgb(if self.client.is_some() {
                        OK_GREEN
                    } else {
                        STOP_RED
                    })),
            )
            .child(div().text_size(px(12.)).text_color(rgb(TEXT_2)).child(
                if self.client.is_some() {
                    "Connected"
                } else {
                    "Not running"
                },
            ))
            .into_any_element()
    }

    /// The real sidebar toggle, wired to the same state as the top bar.
    fn sidebar_toggle(&self, this: Entity<OrbitApp>) -> AnyElement {
        let on = self.sidebar_visible;
        div()
            .id("settings-sidebar-toggle")
            .w(px(36.))
            .h(px(20.))
            .rounded_full()
            .p(px(2.))
            .border_1()
            .border_color(rgb(BORDER))
            .flex()
            .items_center()
            .cursor_pointer()
            .when(on, |t| t.bg(rgb(SPARK_ORANGE)).justify_end())
            .when(!on, |t| t.bg(rgb(BG_RAISED)).justify_start())
            .on_mouse_up(MouseButton::Left, move |_, _, cx| {
                this.update(cx, |app, cx| {
                    app.sidebar_visible = !app.sidebar_visible;
                    cx.notify();
                });
            })
            .child(div().size(px(14.)).rounded_full().bg(rgb(TEXT)))
            .into_any_element()
    }
}

/// Render an embedded HugeIcons SVG tinted with the given color.
pub(crate) fn icon(path: &'static str, size: f32, color: u32) -> impl IntoElement + use<> {
    gpui::svg().path(path).size(px(size)).text_color(rgb(color))
}

/// Same as [`icon`] but for runtime-computed paths (per-provider marks).
pub(crate) fn icon_dyn(path: SharedString, size: f32, color: u32) -> impl IntoElement + use<> {
    gpui::svg().path(path).size(px(size)).text_color(rgb(color))
}

/// A non-interactive pill used for static meta in the composer row.
fn pill_static(icon_path: &'static str, label: &str) -> impl IntoElement + use<> {
    div()
        .flex()
        .items_center()
        .gap_1p5()
        .px(px(7.))
        .py(px(3.))
        .rounded_md()
        .bg(rgb(BG_RAISED))
        .border_1()
        .border_color(rgb(BORDER))
        .text_size(px(12.))
        .text_color(rgb(TEXT_2))
        .child(icon(icon_path, 12., TEXT_2))
        .child(label.to_string())
}

/// Centered empty state — spark + "What should we build in <workspace>?".
fn empty_state(workspace_label: &str) -> impl IntoElement + use<> {
    div()
        .flex_1()
        .min_h_0()
        .w_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap_3()
        .child(icon("icons/spark.svg", 26., SPARK_ORANGE))
        .child(
            div()
                .text_size(px(21.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(rgb(TEXT))
                .child(format!("What should we build in {workspace_label}?")),
        )
}

fn git_branch(cwd: &Path) -> Option<String> {
    let head = std::fs::read_to_string(cwd.join(".git/HEAD")).ok()?;
    let head = head.trim();
    head.strip_prefix("ref: refs/heads/")
        .map(str::to_string)
        .or_else(|| head.get(..7).map(str::to_string))
}

fn render_side_row(
    rows: &Rc<Vec<SideRow>>,
    sessions_data: &Rc<Vec<SessionInfo>>,
    active_path: Option<&Path>,
    ix: usize,
    this: &Entity<OrbitApp>,
) -> impl IntoElement {
    match &rows[ix] {
        SideRow::Workspace {
            label,
            count,
            collapsed,
        } => {
            let label = label.clone();
            let label_for_click = label.clone();
            let this = this.clone();
            div()
                .w_full()
                .px_2()
                .pt_3()
                .pb_1()
                .flex()
                .items_center()
                .gap_1p5()
                .rounded_md()
                .cursor_pointer()
                .hover(|s| s.bg(rgb(BG_HOVER)))
                .on_mouse_up(MouseButton::Left, move |_, _, cx| {
                    let label = label_for_click.clone();
                    this.update(cx, |app, cx| {
                        // Toggle the group's collapse; HashSet::remove returns
                        // whether it was present, so insert only when absent.
                        if !app.collapsed_workspaces.remove(&label) {
                            app.collapsed_workspaces.insert(label);
                        }
                        cx.notify();
                    });
                })
                .child(icon(
                    if *collapsed {
                        "icons/chevron-right.svg"
                    } else {
                        "icons/chevron-down.svg"
                    },
                    11.,
                    TEXT_3,
                ))
                .child(icon("icons/folder.svg", 13., TEXT_2))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_size(px(12.5))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(rgb(TEXT))
                        .child(label.clone()),
                )
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(rgb(TEXT_3))
                        .child(format!("{}", count)),
                )
                .into_any_element()
        }
        SideRow::Session(ix) => {
            let session = sessions_data[*ix].clone();
            let session_for_click = session.clone();
            let active = active_path == Some(session.path.as_path());
            let this = this.clone();
            div()
                .w_full()
                .px_2()
                .py(px(5.))
                .rounded_md()
                .cursor_pointer()
                .flex()
                .items_center()
                .gap_2()
                .when(active, |row| row.bg(rgb(BG_RAISED)))
                .when(!active, |row| row.hover(|s| s.bg(rgb(BG_HOVER))))
                .on_mouse_up(MouseButton::Left, move |_, _, cx| {
                    let session = session_for_click.clone();
                    this.update(cx, |app, cx| {
                        app.on_open_session(session, cx);
                    });
                })
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_size(px(12.5))
                        .text_color(rgb(if active { TEXT } else { TEXT_2 }))
                        .child(session.title.clone()),
                )
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(rgb(TEXT_3))
                        .child(sessions::relative_time(session.modified)),
                )
                .into_any_element()
        }
    }
}
