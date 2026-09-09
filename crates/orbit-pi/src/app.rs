//! The real Orbit shell — Waku-style layout:
//! dark sidebar (New Task / Search nav, sessions grouped by workspace with
//! show-more/show-less per group), top bar with real diff stats and window
//! controls, centered transcript with a spark empty state, floating bottom
//! composer (model / thinking / access pills + round send), and a status
//! bar (workspace, Local, git branch).

use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};

use base64::Engine as _;
use gpui::{
    anchored, deferred, div, img, list, point, prelude::*, px, radians, svg, uniform_list,
    AnchoredPositionMode, Animation, AnimationExt, AnyElement, App, ClipboardItem, Context,
    Corner, DragMoveEvent, ElementId, Entity, ExternalPaths, FocusHandle, Focusable, FontWeight,
    Hsla, ImageSource, IntoElement, ListAlignment, ListState, MouseButton, MouseDownEvent,
    MouseUpEvent, ObjectFit, CursorStyle, Pixels, Render, SharedString, StatefulInteractiveElement,
    Subscription, TextAlign, Transformation, Window, WindowControlArea,
};
use orbit_rpc::{CommandBody, ContextUsage, Event, PiClient};
use serde_json::Value;

use crate::branch_picker::BranchPicker;
use crate::composer::ComposerInput;
use crate::context_meter::{self, ContextPopup};
use crate::mentions::{self, AcEntry, SharedAutocomplete, SlashCommand, Trigger, TriggerKind};
use crate::model_selector::{
    provider_icon, thinking_display, thinking_icon, ModelSelector, PickerKind,
};
use crate::model_selector_match::is_model_selected;
use crate::onboarding::{self, Dependency};
use crate::platform::{self, ExternalApp};
use crate::session_picker::SessionPicker;
use crate::sessions::{self, SessionInfo};
use crate::theme::{self, Theme, ThemeId, ThemeMode};
use crate::transcript::{self, Transcript};

const SIDEBAR_DEFAULT_W: f32 = 248.;
const SIDEBAR_MIN_W: f32 = 200.;
const SIDEBAR_MAX_W: f32 = 440.;
/// Sessions shown under each workspace group before "Show more" appears.
const SIDEBAR_GROUP_SESSIONS_VISIBLE: usize = 10;
/// Height of the sidebar nav action rows (New Task, Search).
const SIDEBAR_ACTION_ROW_H: f32 = 32.;

/// Drag marker for the sidebar resize handle (gpui typed drag state).
struct SidebarResize;

/// An invisible drag ghost — resizing leaves no floating preview.
struct DragGhost;

impl Render for DragGhost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}
const CONTENT_MAX_W: f32 = 960.;

/// Rows shown in the `/`-command and `@`-file autocomplete menu.
const AUTOCOMPLETE_LIMIT: usize = 8;
/// Images that may ride along with one prompt.
const MAX_ATTACHMENTS: usize = 8;

/// Maximum sessions kept alive in the background. Beyond this, settled
/// sessions are evicted (their process torn down); running ones never are.
const MAX_LIVE_SESSIONS: usize = 6;

/// A session running (or recently run) in the background: its own pi
/// process, its own live transcript, and its own agent-run state. Parked
/// when the user switches away mid-run; the run continues and events keep
/// draining every tick, so reopening the session resumes exactly where the
/// stream left off.
struct ParkedSession {
    client: PiClient,
    transcript: Transcript,
    busy: bool,
    added: u64,
    removed: u64,
}

pub struct OrbitApp {
    client: Option<PiClient>,
    /// Sessions with a live pi process, keyed by session-file path. The
    /// active session lives in `client`/`transcript` above; this map holds
    /// the background ones (see `ParkedSession`).
    /// Sidebar width in pixels — adjusted by dragging its right edge.
    sidebar_width: Pixels,
    lives: HashMap<PathBuf, ParkedSession>,
    transcript: Transcript,
    sessions: Vec<SessionInfo>,
    sidebar_list: ListState,
    sidebar_visible: bool,
    pub(crate) input: Entity<ComposerInput>,
    model_label: String,
    /// Pi model id of the active model (stable match key for the picker).
    model_id: String,
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
    /// The session switcher palette (sidebar Search row), if open.
    session_picker: Option<Entity<SessionPicker>>,
    /// Open row-actions menu in the sessions sidebar (which session's path
    /// plus whether the popup is showing the delete confirmation).
    session_menu: Option<SessionMenu>,
    /// Whether the settings surface replaces the main content area.
    settings_open: bool,
    /// Active section within the settings surface.
    settings_section: SettingsSection,
    /// Open dropdown on the settings surface (language / font sizes).
    settings_select: Option<SettingsSelect>,
    /// Filter text for the open settings dropdown.
    settings_filter: Entity<ComposerInput>,
    /// Visited sessions, oldest first — drives the top-bar back/forward
    /// navigation. `history_index` points at the active entry.
    session_history: Vec<SessionInfo>,
    history_index: usize,
    /// Active workspace groups the user has explicitly collapsed.
    collapsed_workspaces: HashSet<String>,
    /// Non-active workspace groups the user has explicitly expanded.
    expanded_workspace_groups: HashSet<String>,
    /// Workspace groups whose session list is expanded past
    /// [`SIDEBAR_GROUP_SESSIONS_VISIBLE`] (Show more).
    expanded_session_groups: HashSet<String>,
    /// Path of the active session file, for the sidebar highlight.
    current_session_path: Option<PathBuf>,
    /// When the popup was dismissed by an outside mouse-down; guards against
    /// the same click's mouse-up immediately re-opening it via the chip.
    menu_dismissed_at: Option<Instant>,
    /// Live context-window usage from `get_session_stats`. `None` when pi
    /// hasn't advertised a window (no model) or the command isn't supported.
    context: Option<ContextUsage>,
    /// Hover compact card vs click-to-open breakdown for the context ring.
    context_popup: ContextPopup,
    /// Shared `/`-command and `@`-file menu state between the composer
    /// (which owns ↑/↓) and this app (which owns Enter/Escape + rendering).
    autocomplete: SharedAutocomplete,
    /// Dismissed via outside mouse-down; cleared when the trigger changes.
    autocomplete_dismissed: bool,
    /// Trigger (kind + query) the autocomplete highlight was synced against.
    last_ac_trigger: Option<(TriggerKind, String)>,
    /// Slash commands reported by pi (`get_commands` — extensions + skills).
    slash_commands: Vec<SlashCommand>,
    /// Workspace files for `@`-mentions (cached per workspace).
    mention_files: Vec<String>,
    mention_files_workspace: Option<PathBuf>,
    /// Images queued to ride along with the next prompt (pasted or picked).
    attachments: Vec<Attachment>,
    /// Files are being dragged over the composer (external OS drag).
    file_drag_hovered: bool,
    /// Installed folder-capable apps for the header's "open in" control.
    open_in_apps: Rc<Vec<ExternalApp>>,
    /// Whether the open-in app picker dropdown is open.
    open_in_menu_open: bool,
    /// Filter text for the open-in menu.
    open_in_filter: Entity<ComposerInput>,
    /// Git branch picker anchored to the status-bar branch chip.
    branch_picker: Option<Entity<BranchPicker>>,
    /// A checkout/create is running on the background executor.
    branch_operation_pending: bool,
    /// Persisted preferred open-in app id (see [`ExternalApp::id`]).
    preferred_open_in_app: Option<String>,
    /// Keeps the theme global observer alive so a settings toggle redraws.
    _theme_sub: Subscription,
    /// Onboarding dependency check results (pi, node, git).
    deps: Vec<Dependency>,
    /// Whether the setup page's Refresh check is in flight (spins the button).
    refreshing: bool,
    /// Whether the top-bar session-details popover is open.
    session_details_open: bool,
    /// The `sessionId` pi reports for the active session (its task id).
    session_id: Option<String>,
}

/// An image queued to ride along with the next prompt.
struct Attachment {
    name: String,
    mime: String,
    /// base64 payload (no `data:` prefix) — pi's prompt image shape.
    data: String,
    /// Decoded image for the chip thumbnail.
    preview: Option<Arc<gpui::Image>>,
}

impl Attachment {
    fn from_image(image: &gpui::Image, index: usize) -> Self {
        let ext = match image.format {
            gpui::ImageFormat::Png => "png",
            gpui::ImageFormat::Jpeg => "jpg",
            gpui::ImageFormat::Webp => "webp",
            gpui::ImageFormat::Gif => "gif",
            gpui::ImageFormat::Bmp => "bmp",
            gpui::ImageFormat::Svg => "svg",
            gpui::ImageFormat::Tiff => "tiff",
        };
        Self {
            name: format!("Pasted image {}.{ext}", index + 1),
            mime: image.format.mime_type().to_string(),
            data: base64::engine::general_purpose::STANDARD.encode(&image.bytes),
            preview: Some(Arc::new(image.clone())),
        }
    }

    /// Images only — anything else returns `None` so a drop can fall back to
    /// referencing the file by path instead.
    fn from_path(path: &Path) -> Option<Self> {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_lowercase)?;
        let (mime, format) = match ext.as_str() {
            "png" => ("image/png", gpui::ImageFormat::Png),
            "jpg" | "jpeg" => ("image/jpeg", gpui::ImageFormat::Jpeg),
            "webp" => ("image/webp", gpui::ImageFormat::Webp),
            "gif" => ("image/gif", gpui::ImageFormat::Gif),
            "bmp" => ("image/bmp", gpui::ImageFormat::Bmp),
            _ => return None,
        };
        let bytes = std::fs::read(path).ok()?;
        let preview = Arc::new(gpui::Image::from_bytes(format, bytes.clone()));
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "image".into());
        Some(Self {
            name,
            mime: mime.into(),
            data: base64::engine::general_purpose::STANDARD.encode(bytes),
            preview: Some(preview),
        })
    }

    /// The wire shape pi's `prompt.images` expects.
    fn to_prompt_image(&self) -> Value {
        serde_json::json!({ "type": "image", "data": self.data, "mimeType": self.mime })
    }
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
        let autocomplete: SharedAutocomplete = Rc::new(std::cell::RefCell::new(
            mentions::AutocompleteState::default(),
        ));
        let input = cx.new(|cx| ComposerInput::new(cx).with_autocomplete(autocomplete.clone()));
        // Filter fields for the settings dropdowns and the open-in menu.
        // `Composer Picker` keeps backspace/delete working while Enter/arrows
        // dispatch to the (unhandled) Picker actions rather than submitting.
        let settings_filter = cx.new(|cx| {
            ComposerInput::new(cx)
                .with_placeholder("Filter…")
                .with_key_context("Composer Picker")
        });
        let open_in_filter = cx.new(|cx| {
            ComposerInput::new(cx)
                .with_placeholder("Filter…")
                .with_key_context("Composer Picker")
        });

        // Spawn pi rooted at the repo; sessions live in the real
        // ~/.pi/agent/sessions so they are shared with the CLI.
        let workspace = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let (client, connect_error) = match PiClient::spawn(&workspace, None) {
            Ok(client) => (Some(client), String::new()),
            Err(err) => (None, format!("pi spawn failed: {err}")),
        };

        let theme_sub = cx.observe_global::<Theme>(|this, cx| {
            this.input.update(cx, |_, cx| cx.notify());
            if let Some((_, selector)) = &this.model_selector {
                selector.update(cx, |_, cx| cx.notify());
            }
            cx.notify();
        });

        // Probe the runtime pieces we need (pi, node, git) so the setup page
        // can show install commands when something is missing.
        let deps = onboarding::check_dependencies();

        let mut app = Self {
            client,
            sidebar_width: px(SIDEBAR_DEFAULT_W),
            lives: HashMap::new(),
            transcript: Transcript::new(),
            sessions: sessions::load_sessions(),
            sidebar_list: ListState::new(0, ListAlignment::Top, px(80.)),
            sidebar_visible: true,
            input,
            model_label: "…".into(),
            model_id: String::new(),
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
            session_picker: None,
            session_menu: None,
            settings_open: false,
            settings_section: SettingsSection::General,
            settings_select: None,
            settings_filter: settings_filter,
            session_history: Vec::new(),
            history_index: 0,
            collapsed_workspaces: HashSet::new(),
            expanded_workspace_groups: HashSet::new(),
            expanded_session_groups: HashSet::new(),
            current_session_path: None,
            menu_dismissed_at: None,
            context: None,
            context_popup: ContextPopup::None,
            autocomplete,
            autocomplete_dismissed: false,
            last_ac_trigger: None,
            slash_commands: Vec::new(),
            mention_files: Vec::new(),
            mention_files_workspace: None,
            attachments: Vec::new(),
            file_drag_hovered: false,
            open_in_apps: Rc::new(Vec::new()),
            open_in_menu_open: false,
            open_in_filter: open_in_filter,
            branch_picker: None,
            branch_operation_pending: false,
            preferred_open_in_app: platform::load_preferred_open_in_app(),
            _theme_sub: theme_sub,
            deps,
            refreshing: false,
            session_details_open: false,
            session_id: None,
        };

        if app.client.is_some() {
            app.send(CommandBody::GetState, "get_state");
            app.send(CommandBody::GetAvailableModels, "get_available_models");
            app.send(
                CommandBody::GetAvailableThinkingLevels,
                "get_available_thinking_levels",
            );
            app.send(CommandBody::GetCommands, "get_commands");
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
        self.send(CommandBody::GetCommands, "get_commands");
    }

    /// Re-fetch the current context-window estimate. Cheap; call after
    /// settle, session switch, compaction, and model changes — never per tick.
    fn refresh_context_stats(&mut self) {
        self.send(CommandBody::GetSessionStats, "get_session_stats");
    }

    /// Register the active client under the session file pi reports. The
    /// startup process has no known path until pi's first state/stats
    /// response; a `new_session` re-keys it the same way (the handler clears
    /// `current_session_path` and the next response adopts the new file).
    /// An explicit switch never re-keys — its path is already claimed.
    fn adopt_session_file(&mut self, file: PathBuf) {
        if self.current_session_path.is_none() {
            self.current_session_path = Some(file);
        }
    }

    /// Park a background session, evicting a settled one if over the cap.
    /// Running sessions are never evicted.
    fn park(&mut self, path: PathBuf, parked: ParkedSession) {
        if self.lives.len() >= MAX_LIVE_SESSIONS {
            let victim = self
                .lives
                .iter()
                .find(|(_, p)| !p.busy)
                .map(|(k, _)| k.clone());
            if let Some(victim) = victim {
                self.lives.remove(&victim);
            }
        }
        self.lives.insert(path, parked);
    }

    /// Heartbeat (~90ms): drain protocol events into the UI.
    pub fn tick(&mut self, cx: &mut Context<Self>) {
        self.tick_background(cx);
        // Images pasted in the composer become message attachments.
        self.drain_pasted_images(cx);
        // A drag that left the window clears gpui's active drag without any
        // element event seeing it — the heartbeat drops a stale highlight.
        if self.file_drag_hovered && !cx.has_active_drag() {
            self.file_drag_hovered = false;
            cx.notify();
        }
        let events = {
            let Some(client) = self.client.as_ref() else {
                return;
            };
            client.drain_events()
        };
        let copy_pending = self.transcript.prune_copy_feedback();
        if events.is_empty() {
            // Keep the live "Working for…" clock moving while a turn is open.
            if self.busy || self.transcript.is_streaming() || copy_pending {
                cx.notify();
            }
            return;
        }

        let mut refresh_sessions = false;
        for event in &events {
            match event {
                Event::AgentStart => self.busy = true,
                // pi blocks interactive extension dialogs on a client
                // response; Orbit has no dialog surface yet — cancel the
                // request so the run can settle (Waku parity).
                Event::ExtensionUiRequest { id, .. } => {
                    self.send(
                        CommandBody::Raw(serde_json::json!({
                            "type": "extension_ui_response",
                            "id": id,
                            "cancelled": true
                        })),
                        "extension_ui_response",
                    );
                }
                Event::SessionInfoChanged { name } => {
                    // pi names the session after the first user message;
                    // forward it live the way Waku does.
                    if let Some(name) = name {
                        self.current_title = Some(name.clone());
                    }
                    refresh_sessions = true;
                }
                Event::AutoRetryEnd { value } => {
                    let success = value.get("success").and_then(|v| v.as_bool());
                    if success == Some(false) {
                        let error = value
                            .get("finalError")
                            .and_then(|v| v.as_str())
                            .unwrap_or("pi exhausted its automatic retries");
                        self.status = format!("retry failed: {error}");
                    }
                }
                Event::AgentSettled => {
                    self.busy = false;
                    refresh_sessions = true;
                    self.refresh_context_stats();
                }
                Event::CompactionEnd { .. } => {
                    // Post-compaction usage is unknown until the next turn;
                    // refresh so the meter can show an empty/unknown state.
                    self.refresh_context_stats();
                }
                Event::ProcessExited => {
                    self.busy = false;
                    self.status = "pi process exited — restart the app".into();
                }
                Event::MessageEnd { value } => {
                    // Real edit stats from finalized tool calls.
                    let (a, r) = transcript::diff_from_message(value);
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
            self.sync_session_menu(cx);
            cx.notify();
        }
        // Responses can change app state without touching the transcript
        // (model/thinking labels, catalogs, status) — always redraw a frame
        // in which events were processed so those changes become visible.
        cx.notify();
    }

    /// Drain background (parked) sessions. Their runs continue in their own
    /// pi processes; events keep their transcripts current, so reopening a
    /// parked session resumes the live stream exactly where it left off.
    fn tick_background(&mut self, cx: &mut Context<Self>) {
        if self.lives.is_empty() {
            return;
        }
        let mut changed = false;
        let mut any_busy = false;
        let mut dead: Vec<PathBuf> = Vec::new();
        for (path, parked) in self.lives.iter_mut() {
            for event in parked.client.drain_events() {
                match &event {
                    Event::AgentStart => parked.busy = true,
                    // `agent_settled` is the real settle (queued steering /
                    // follow-up / retry can continue past `agent_end`).
                    Event::AgentSettled | Event::ProcessExited => parked.busy = false,
                    // pi blocks extension dialogs on a client response;
                    // cancel so a background run can settle (same as the
                    // active-session handling in `tick`).
                    Event::ExtensionUiRequest { id, .. } => {
                        let _ = parked.client.respond_dialog(
                            id,
                            serde_json::json!({
                                "type": "extension_ui_response",
                                "id": id,
                                "cancelled": true
                            }),
                        );
                        continue;
                    }
                    Event::MessageEnd { value } => {
                        // Real edit stats from finalized tool calls.
                        let (a, r) = transcript::diff_from_message(value);
                        parked.added += a;
                        parked.removed += r;
                    }
                    _ => {}
                }
                changed |= parked.transcript.apply_event(&event);
            }
            if !parked.client.is_alive() {
                dead.push(path.clone());
            }
            any_busy |= parked.busy;
        }
        for path in dead {
            self.lives.remove(&path);
        }
        if changed || any_busy {
            // Repaint while a background run is live (sidebar loader phase,
            // park-state changes) even though the visible transcript's
            // active client produced no events this tick.
            cx.notify();
        }
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
                if let Some(model) = data.get("model") {
                    if let Some(name) = model.get("name").and_then(serde_json::Value::as_str) {
                        self.model_label = name.to_string();
                    }
                    if let Some(id) = model.get("id").and_then(serde_json::Value::as_str) {
                        self.model_id = id.to_string();
                    }
                    if let Some(provider) = model.get("provider").and_then(serde_json::Value::as_str)
                    {
                        self.model_provider = provider.to_string();
                    }
                }
                if let Some(level) = data
                    .get("thinkingLevel")
                    .and_then(serde_json::Value::as_str)
                {
                    self.thinking_label = level.to_string();
                }
                if let Some(file) = data.get("sessionFile").and_then(Value::as_str) {
                    self.adopt_session_file(PathBuf::from(file));
                }
                if let Some(id) = data.get("sessionId").and_then(Value::as_str) {
                    self.session_id = Some(id.to_string());
                }
                self.sync_model_selector(cx);
                self.refresh_context_stats();
            }
            "get_messages" => {
                self.transcript.load_from(data);
                // Rebuild diff stats from the loaded history.
                self.added = 0;
                self.removed = 0;
                if let Some(messages) = data.get("messages").and_then(serde_json::Value::as_array) {
                    for message in messages {
                        let (a, r) = transcript::diff_from_message(message);
                        self.added += a;
                        self.removed += r;
                    }
                }
                self.refresh_context_stats();
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
            "get_commands" => {
                self.slash_commands = data
                    .get("commands")
                    .and_then(serde_json::Value::as_array)
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|c| {
                                let name = c.get("name").and_then(Value::as_str)?;
                                let description =
                                    c.get("description").and_then(Value::as_str).unwrap_or("");
                                Some(SlashCommand {
                                    name: name.to_string(),
                                    description: description.to_string(),
                                })
                            })
                            .collect()
                    })
                    .unwrap_or_default();
            }
            "get_session_stats" => {
                self.context = ContextUsage::from_stats(data);
                if let Some(file) = data.get("sessionFile").and_then(Value::as_str) {
                    self.adopt_session_file(PathBuf::from(file));
                }
            }
            "switch_session" => {
                if success {
                    self.send(CommandBody::GetMessages, "get_messages");
                    self.send(CommandBody::GetState, "get_state");
                    self.refresh_catalogs();
                }
            }
            "new_session" => {
                self.transcript.clear();
                self.current_title = None;
                self.current_session_path = None;
                self.added = 0;
                self.removed = 0;
                self.context = None;
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

    /// Move images the composer collected (clipboard paste) into the
    /// attachment queue. Called on tick and again at submit so a paste
    /// immediately followed by Enter still attaches.
    fn drain_pasted_images(&mut self, cx: &mut Context<Self>) {
        if !self.input.read(cx).has_pasted_images() {
            return;
        }
        let pasted = self
            .input
            .update(cx, |input, _| std::mem::take(&mut input.pasted_images));
        for image in &pasted {
            if self.attachments.len() >= MAX_ATTACHMENTS {
                self.status = format!("at most {MAX_ATTACHMENTS} images per message");
                break;
            }
            let index = self.attachments.len();
            self.attachments.push(Attachment::from_image(image, index));
        }
        if !pasted.is_empty() {
            cx.notify();
        }
    }

    fn submit(&mut self, text: String, cx: &mut Context<Self>) {
        let text = text.trim().to_string();
        if text.is_empty() {
            return;
        }
        // Collect any paste that raced the submit tick.
        self.drain_pasted_images(cx);
        // Attached images ride prompts; the steer body carries no image
        // field, so they are dropped when the message steers a live run.
        let attachments = std::mem::take(&mut self.attachments);
        let images = if attachments.is_empty() {
            None
        } else {
            Some(
                attachments
                    .iter()
                    .map(|a| a.to_prompt_image())
                    .collect::<Vec<_>>(),
            )
        };
        // Waku behavior: while the agent is mid-turn a follow-up message is
        // a *steer* (injected into the running turn), not a new prompt.
        let steer = self.busy || self.transcript.is_streaming();
        if steer {
            if !attachments.is_empty() {
                self.status =
                    "attachments can't ride a steer — send them with the next prompt".into();
            }
            self.send(
                CommandBody::Steer {
                    message: text.clone(),
                },
                "steer",
            );
        } else {
            self.send(
                CommandBody::Prompt {
                    message: text.clone(),
                    images,
                    streaming_behavior: None,
                },
                "prompt",
            );
        }
        // Show the prompt immediately — pi does not echo it back in RPC mode.
        // The decoded previews ride along so the chat window shows what was
        // attached (pi echoes/snapshots carry image blocks for reloads).
        self.transcript.append_user_message(
            &text,
            if steer {
                Vec::new()
            } else {
                attachments
                    .iter()
                    .filter_map(|a| a.preview.clone())
                    .collect()
            },
        );
        self.input.update(cx, |input, cx| input.clear(cx));
        cx.notify();
    }

    fn on_submit(&mut self, _: &crate::Submit, _: &mut Window, cx: &mut Context<Self>) {
        // Enter commits the highlighted autocomplete entry while the menu
        // is open; a second Enter submits.
        if self.commit_autocomplete_if_open(cx) {
            return;
        }
        let text = self.input.read(cx).text();
        self.submit(text, cx);
    }

    fn on_send_click(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.commit_autocomplete_if_open(cx) {
            return;
        }
        let text = self.input.read(cx).text();
        self.submit(text, cx);
    }

    fn on_abort(&mut self, _: &crate::AbortRun, window: &mut Window, cx: &mut Context<Self>) {
        // Escape backs out of the topmost surface: the autocomplete menu
        // first, then settings, popovers, then a running agent.
        if self.autocomplete.borrow().open {
            self.autocomplete_dismissed = true;
            cx.notify();
            return;
        }
        if self.settings_open {
            // Escape closes an open dropdown first, then leaves settings.
            if self.settings_select.take().is_some() {
                cx.notify();
                return;
            }
            self.settings_open = false;
            cx.notify();
            return;
        }
        if self.model_selector.is_some() {
            self.close_model_selector(window, cx);
            return;
        }
        if self.open_in_menu_open {
            self.open_in_menu_open = false;
            cx.notify();
            return;
        }
        if self.context_popup != ContextPopup::None {
            self.context_popup = ContextPopup::None;
            cx.notify();
            return;
        }
        self.send(CommandBody::Abort, "abort");
        cx.notify();
    }

    fn on_abort_mouse(&mut self, _: &MouseUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.on_abort(&crate::AbortRun, window, cx);
    }

    fn on_new_session(&mut self, _: &crate::NewSession, _window: &mut Window, cx: &mut Context<Self>) {
        self.send(CommandBody::NewSession, "new_session");
        cx.notify();
    }

    /// Plus on a workspace group: start a fresh session rooted at that cwd.
    fn on_new_session_in_workspace(
        &mut self,
        cwd: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.current_workspace.as_ref() == Some(&cwd) && self.client.is_some() {
            self.send(CommandBody::NewSession, "new_session");
            self.input.read(cx).focus(window);
            cx.notify();
            return;
        }

        if let Some(old_path) = self.current_session_path.take() {
            let old_busy = self.busy || self.transcript.is_streaming();
            if let Some(client) = self.client.take() {
                if old_busy {
                    let transcript = std::mem::replace(&mut self.transcript, Transcript::new());
                    self.park(
                        old_path,
                        ParkedSession {
                            client,
                            transcript,
                            busy: true,
                            added: self.added,
                            removed: self.removed,
                        },
                    );
                }
            }
        }

        self.busy = false;
        self.transcript.clear();
        self.current_title = None;
        self.current_session_path = None;
        self.added = 0;
        self.removed = 0;
        self.context = None;
        self.current_workspace = Some(cwd.clone());

        match PiClient::spawn(&cwd, None) {
            Ok(client) => {
                self.client = Some(client);
                self.send(CommandBody::NewSession, "new_session");
                self.refresh_catalogs();
                self.status = "New session".into();
            }
            Err(err) => {
                self.client = None;
                self.status = format!("pi spawn failed: {err}");
            }
        }
        self.input.read(cx).focus(window);
        cx.notify();
    }

    /// Open the OS folder picker and start (or restart) the task in the
    /// selected directory. Used from the new-task page and the status bar.
    fn on_pick_folder_click(
        &mut self,
        _: &MouseUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let picked = rfd::FileDialog::new()
            .set_title("Choose a folder for this task")
            .pick_folder();
        if let Some(folder) = picked {
            self.start_task_in_folder(folder, window, cx);
        }
    }

    /// Start a new task rooted at `folder`: spawn a fresh pi process with
    /// that working directory (the agent reads, edits, and runs commands
    /// there), reset the transcript/state, and focus the composer. The
    /// previous client is idle at this point — the picker only shows on the
    /// empty (new-task) page — so dropping it tears down its process.
    fn start_task_in_folder(
        &mut self,
        folder: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.client.take();
        self.transcript.clear();
        self.current_title = None;
        self.current_workspace = Some(folder);
        self.current_session_path = None;
        self.added = 0;
        self.removed = 0;
        self.context = None;
        match PiClient::spawn(self.current_workspace.as_ref().unwrap(), None) {
            Ok(client) => {
                self.client = Some(client);
                self.send(CommandBody::GetState, "get_state");
                self.refresh_catalogs();
                self.status = "New task started".into();
            }
            Err(err) => {
                self.status = format!("pi spawn failed: {err}");
            }
        }
        // Ready to type: the empty state is gone, so put the caret in the
        // composer.
        self.input.read(cx).focus(window);
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
        self.send(CommandBody::GetState, "get_state");

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

        let initial_highlight = match kind {
            PickerKind::Model => self
                .available_models
                .iter()
                .position(|model| {
                    is_model_selected(
                        model,
                        &self.model_label,
                        &self.model_id,
                        &self.model_provider,
                    )
                })
                .unwrap_or(0),
            PickerKind::Thinking => self
                .available_thinking_levels
                .iter()
                .position(|level| level.eq_ignore_ascii_case(&self.thinking_label))
                .unwrap_or(0),
        };

        let selector = cx.new(|cx| {
            ModelSelector::new(
                kind,
                self.available_models.clone(),
                self.available_thinking_levels.clone(),
                self.model_label.clone(),
                self.model_id.clone(),
                self.model_provider.clone(),
                self.thinking_label.clone(),
                on_select_model,
                on_select_level,
                on_dismiss,
                initial_highlight,
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

    // ── session switcher palette (sidebar Search row) ─────────────────

    /// Toggle the session switcher palette from the sidebar's Search row.
    /// Mutually exclusive with the composer pickers and the row menu.
    fn toggle_session_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // A dismissal from the same click's mouse-down must not immediately
        // re-open — same guard the composer chips use.
        const GESTURE: Duration = Duration::from_millis(200);
        if let Some(dismissed) = self.menu_dismissed_at.take() {
            if dismissed.elapsed() < GESTURE {
                return;
            }
        }
        if self.session_picker.take().is_some() {
            self.input.read(cx).focus(window);
            cx.notify();
            return;
        }
        if self.model_selector.is_some() {
            self.close_model_selector(window, cx);
        }
        self.session_menu = None;

        let this = cx.weak_entity();
        let on_open = Box::new(
            move |session: SessionInfo, _window: &mut Window, cx: &mut App| {
                this.update(cx, |app, cx| {
                    app.session_picker = None;
                    app.on_open_session(session, cx);
                })
                .ok();
            },
        ) as Box<dyn Fn(SessionInfo, &mut Window, &mut App)>;
        let this = cx.weak_entity();
        let on_dismiss = Box::new(move |by_mouse: bool, _window: &mut Window, cx: &mut App| {
            this.update(cx, |app, cx| {
                if by_mouse {
                    app.menu_dismissed_at = Some(Instant::now());
                }
                app.session_picker = None;
                cx.notify();
            })
            .ok();
        }) as Box<dyn Fn(bool, &mut Window, &mut App)>;

        let picker = cx.new(|cx| {
            SessionPicker::new(
                self.sessions.clone(),
                self.current_session_path.clone(),
                on_open,
                on_dismiss,
                cx,
            )
        });
        // Focus the palette's filter input so typing filters immediately.
        window.focus(&picker.read(cx).focus_handle(cx));
        self.session_picker = Some(picker);
        cx.notify();
    }

    /// The anchored palette, rendered inside the sidebar's nav column so it
    /// drops below the Search row.
    ///
    /// Taffy places absolutely-positioned children at the container's start
    /// corner (respecting its alignment) rather than at their in-flow
    /// position — a raw `anchored()` child of the nav column would anchor to
    /// the column's top-left and overlap the rows above. The palette is
    /// therefore wrapped in a zero-height relative anchor that participates
    /// in the column flow right after the Search row, and the popup hangs
    /// from that point.
    fn session_palette(&self) -> Option<AnyElement> {
        self.session_picker.clone().map(|picker| {
            div()
                .relative()
                .w_full()
                .h(px(0.))
                .child(
                    anchored()
                        .position_mode(AnchoredPositionMode::Local)
                        .anchor(Corner::TopLeft)
                        .offset(point(px(0.), px(4.)))
                        .snap_to_window()
                        .child(deferred(picker)),
                )
                .into_any_element()
        })
    }

    /// Toggle the Git branch picker from the status-bar branch chip.
    fn toggle_branch_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.branch_operation_pending {
            return;
        }
        const GESTURE: Duration = Duration::from_millis(200);
        if let Some(dismissed) = self.menu_dismissed_at.take() {
            if dismissed.elapsed() < GESTURE {
                return;
            }
        }
        if self.branch_picker.take().is_some() {
            self.input.read(cx).focus(window);
            cx.notify();
            return;
        }
        if self.model_selector.is_some() {
            self.close_model_selector(window, cx);
        }
        self.session_picker = None;
        self.session_menu = None;

        let cwd = match self
            .current_workspace
            .clone()
            .or_else(|| std::env::current_dir().ok())
        {
            Some(cwd) => cwd,
            None => return,
        };
        if !crate::git::is_repo(&cwd) {
            self.status = "Not a Git repository".into();
            cx.notify();
            return;
        }
        let current = crate::git::current_branch(&cwd).unwrap_or_else(|| "HEAD".into());
        let branches = crate::git::list_branches(&cwd).unwrap_or_else(|err| {
            self.status = format!("branch list failed: {err}");
            vec![current.clone()]
        });
        let workspace_label = sessions::workspace_label(&cwd);

        let this = cx.weak_entity();
        let on_checkout = Box::new(
            move |branch: String, _window: &mut Window, cx: &mut App| {
                this.update(cx, |app, cx| {
                    app.checkout_branch(branch, cx);
                })
                .ok();
            },
        ) as Box<dyn Fn(String, &mut Window, &mut App)>;
        let this = cx.weak_entity();
        let on_create = Box::new(
            move |name: String, _window: &mut Window, cx: &mut App| {
                this.update(cx, |app, cx| {
                    app.create_branch(name, cx);
                })
                .ok();
            },
        ) as Box<dyn Fn(String, &mut Window, &mut App)>;
        let this = cx.weak_entity();
        let on_dismiss = Box::new(move |by_mouse: bool, _window: &mut Window, cx: &mut App| {
            this.update(cx, |app, cx| {
                if by_mouse {
                    app.menu_dismissed_at = Some(Instant::now());
                }
                app.branch_picker = None;
                cx.notify();
            })
            .ok();
        }) as Box<dyn Fn(bool, &mut Window, &mut App)>;

        let picker = cx.new(|cx| {
            BranchPicker::new(
                workspace_label,
                branches,
                current,
                on_checkout,
                on_create,
                on_dismiss,
                cx,
            )
        });
        window.focus(&picker.read(cx).focus_handle(cx));
        self.branch_picker = Some(picker);
        cx.notify();
    }

    fn checkout_branch(&mut self, branch: String, cx: &mut Context<Self>) {
        let Some(cwd) = self
            .current_workspace
            .clone()
            .or_else(|| std::env::current_dir().ok())
        else {
            return;
        };
        self.branch_operation_pending = true;
        self.branch_picker = None;
        cx.notify();

        let label = branch.clone();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { crate::git::checkout_branch(&cwd, &branch) })
                .await;
            let _ = this.update(cx, |app, cx| {
                app.branch_operation_pending = false;
                match result {
                    Ok(()) => app.status = format!("Switched to {label}"),
                    Err(err) => app.status = format!("Branch switch failed: {err}"),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn create_branch(&mut self, name: String, cx: &mut Context<Self>) {
        let Some(cwd) = self
            .current_workspace
            .clone()
            .or_else(|| std::env::current_dir().ok())
        else {
            return;
        };
        self.branch_operation_pending = true;
        self.branch_picker = None;
        cx.notify();

        let label = name.clone();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { crate::git::create_and_checkout_branch(&cwd, &name) })
                .await;
            let _ = this.update(cx, |app, cx| {
                app.branch_operation_pending = false;
                match result {
                    Ok(()) => app.status = format!("Created and switched to {label}"),
                    Err(err) => app.status = format!("Branch create failed: {err}"),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Popover anchored above the status-bar branch chip.
    fn branch_picker_popup(&self) -> Option<AnyElement> {
        self.branch_picker.clone().map(|picker| {
            div()
                .absolute()
                .bottom_0()
                .left_0()
                .size(px(0.))
                .child(
                    anchored()
                        .position_mode(AnchoredPositionMode::Local)
                        .anchor(Corner::BottomLeft)
                        .offset(point(px(0.), px(-6.)))
                        .snap_to_window()
                        .child(deferred(picker)),
                )
                .into_any_element()
        })
    }

    // ── sidebar row-actions menu ───────────────────────────────────────

    /// Open (or toggle closed) the row-actions popup for a session row.
    fn toggle_session_menu(
        &mut self,
        menu: SessionMenu,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Same-click dismissal must not re-open (see toggle_session_picker).
        const GESTURE: Duration = Duration::from_millis(200);
        if let Some(dismissed) = self.menu_dismissed_at.take() {
            if dismissed.elapsed() < GESTURE {
                return;
            }
        }
        if self.session_menu.as_ref() == Some(&menu) {
            self.session_menu = None;
            cx.notify();
            return;
        }
        if self.session_picker.take().is_some() {
            self.input.read(cx).focus(window);
        }
        if self.branch_picker.take().is_some() {
            self.input.read(cx).focus(window);
        }
        if self.model_selector.is_some() {
            self.close_model_selector(window, cx);
        }
        self.session_menu = Some(menu);
        cx.notify();
    }

    fn on_menu_copy_path(&mut self, cx: &mut Context<Self>) {
        if let Some(menu) = &self.session_menu {
            cx.write_to_clipboard(ClipboardItem::new_string(
                menu.path.to_string_lossy().into_owned(),
            ));
        }
        self.session_menu = None;
        cx.notify();
    }

    fn on_menu_reveal(&mut self, cx: &mut Context<Self>) {
        if let Some(menu) = &self.session_menu {
            reveal_in_file_manager(menu.path.clone());
        }
        self.session_menu = None;
        cx.notify();
    }

    /// First Delete click: swap the popup to the confirmation state.
    fn on_menu_delete_request(&mut self, cx: &mut Context<Self>) {
        if let Some(menu) = self.session_menu.as_mut() {
            menu.confirm_delete = true;
            cx.notify();
        }
    }

    /// Confirmed: remove the session file from disk and refresh the list.
    fn on_menu_delete_confirm(&mut self, cx: &mut Context<Self>) {
        if let Some(menu) = self.session_menu.take() {
            if let Err(err) = fs::remove_file(&menu.path) {
                self.status = format!("delete failed: {err}");
            } else {
                self.status = "Session deleted".into();
            }
            self.sessions = sessions::load_sessions();
            cx.notify();
        }
    }

    fn on_menu_cancel(&mut self, cx: &mut Context<Self>) {
        self.session_menu = None;
        cx.notify();
    }

    /// Keep the row menu honest: close it when its session vanishes from
    /// the store (deleted externally, session ended, …).
    fn sync_session_menu(&mut self, cx: &mut Context<Self>) {
        if let Some(menu) = &self.session_menu {
            if !menu.path.exists() {
                self.session_menu = None;
                cx.notify();
            }
        }
    }

    /// Push the latest catalog/current-selection snapshot into the open
    /// popup (no-op while it is closed).
    fn sync_model_selector(&mut self, cx: &mut Context<Self>) {
        if let Some((_, selector)) = &self.model_selector {
            let models = self.available_models.clone();
            let levels = self.available_thinking_levels.clone();
            let model = self.model_label.clone();
            let model_id = self.model_id.clone();
            let provider = self.model_provider.clone();
            let level = self.thinking_label.clone();
            selector.update(cx, |selector, cx| {
                selector.set_catalog(models, levels, model, model_id, provider, level, cx)
            });
        }
    }

    // ── `/` commands + `@` file mentions ───────────────────────────────────

    /// The filtered menu entries for the active trigger (empty when none).
    fn autocomplete_entries(&mut self, trigger: &Trigger) -> Vec<AcEntry> {
        match trigger.kind {
            TriggerKind::Slash => mentions::filter_entries(
                &trigger.query,
                &[],
                &self.slash_commands,
                AUTOCOMPLETE_LIMIT,
            ),
            TriggerKind::At => {
                self.ensure_mention_files();
                mentions::filter_entries(
                    &trigger.query,
                    &self.mention_files,
                    &[],
                    AUTOCOMPLETE_LIMIT,
                )
            }
        }
    }

    /// (Re)build the workspace file cache when the workspace changed.
    fn ensure_mention_files(&mut self) {
        if self.mention_files_workspace == self.current_workspace {
            return;
        }
        self.mention_files_workspace = self.current_workspace.clone();
        self.mention_files = self
            .current_workspace
            .as_deref()
            .map(mentions::list_workspace_files)
            .unwrap_or_default();
    }

    /// Derive the open/closed/highlight state from the composer text.
    /// The trigger is pure text state, so this runs every frame and the
    /// menu opens/closes by itself as the user types.
    fn sync_autocomplete(&mut self, cx: &Context<Self>) {
        let trigger = self.input.read(cx).active_trigger();
        let key = trigger.as_ref().map(|t| (t.kind, t.query.clone()));
        if key != self.last_ac_trigger {
            // A new (or changed) token re-arms a mouse dismissal and resets
            // the highlight.
            self.autocomplete_dismissed = false;
        }
        self.last_ac_trigger = key;
        let mut state = self.autocomplete.borrow_mut();
        if !self.autocomplete_dismissed {
            if let Some(trigger) = &trigger {
                let count = match trigger.kind {
                    TriggerKind::Slash => mentions::filter_entries(
                        &trigger.query,
                        &[],
                        &self.slash_commands,
                        AUTOCOMPLETE_LIMIT,
                    )
                    .len(),
                    TriggerKind::At => {
                        drop(state);
                        self.ensure_mention_files();
                        state = self.autocomplete.borrow_mut();
                        mentions::filter_entries(
                            &trigger.query,
                            &self.mention_files,
                            &[],
                            AUTOCOMPLETE_LIMIT,
                        )
                        .len()
                    }
                };
                state.count = count;
                state.open = count > 0;
                state.highlighted = state.highlighted.min(count.saturating_sub(1));
            } else {
                state.open = false;
                state.count = 0;
                state.highlighted = 0;
            }
        } else {
            state.open = false;
        }
    }

    /// Commit the highlighted entry (Enter/click): replace the trigger
    /// token with the completed `/name ` or `@path ` text.
    fn commit_autocomplete_if_open(&mut self, cx: &mut Context<Self>) -> bool {
        let (open, highlighted) = {
            let state = self.autocomplete.borrow();
            (state.open, state.highlighted)
        };
        if !open {
            return false;
        }
        let Some(trigger) = self.input.read(cx).active_trigger() else {
            return false;
        };
        let entries = self.autocomplete_entries(&trigger);
        let Some(entry) = entries.into_iter().nth(highlighted) else {
            return false;
        };
        self.commit_entry(entry, cx);
        true
    }

    /// Insert `entry`'s completion over the active trigger token.
    fn commit_entry(&mut self, entry: AcEntry, cx: &mut Context<Self>) {
        let Some(trigger) = self.input.read(cx).active_trigger() else {
            return;
        };
        let text = match &entry {
            AcEntry::Command { name, .. } => format!("/{name} "),
            AcEntry::File { path } => format!("@{path} "),
        };
        self.input.update(cx, |input, cx| {
            input.replace_range(trigger.start..trigger.end, &text, cx)
        });
        // Keep the menu closed until the trigger changes (the replaced text
        // still ends in a space, but stay explicit).
        self.autocomplete_dismissed = true;
        self.last_ac_trigger = None;
        self.autocomplete.borrow_mut().open = false;
        cx.notify();
    }

    /// Outside mouse-down dismisses the menu until the trigger changes.
    fn on_autocomplete_outside_down(
        &mut self,
        _: &MouseDownEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.autocomplete.borrow().open {
            self.autocomplete_dismissed = true;
            cx.notify();
        }
    }

    /// The autocomplete popup, anchored above the composer box (mirrors
    /// the model/thinking chip popovers). `None` while closed.
    fn autocomplete_popup(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let highlighted = {
            let state = self.autocomplete.borrow();
            if !state.open || state.count == 0 {
                return None;
            }
            state.highlighted
        };
        let trigger = self.input.read(cx).active_trigger()?;
        let entries = self.autocomplete_entries(&trigger);
        if entries.is_empty() {
            return None;
        }
        let highlighted = highlighted.min(entries.len() - 1);
        let theme = *theme::get(cx);
        let this = cx.weak_entity();
        let mut list = div()
            .id("ac-list")
            .w_full()
            .max_h(px(8. * 34.))
            .overflow_y_scroll()
            .px(px(4.))
            .pt(px(4.))
            .pb(px(4.))
            .flex()
            .flex_col()
            .gap(px(2.));
        for (ix, entry) in entries.iter().enumerate() {
            let selected = ix == highlighted;
            let this = this.clone();
            let entry = entry.clone();
            // Leading glyph: brand glyph for commands; for files, the
            // devicons Nerd Font glyph (falls back to the extension text
            // badge when no Nerd Font is installed).
            let nerd = nerd_font_family(cx);
            let leading: AnyElement = match &entry {
                AcEntry::Command { .. } => {
                    icon("icons/extensions.svg", 13., theme.text_3).into_any_element()
                }
                AcEntry::File { path } => file_glyph(
                    path.as_str(),
                    theme.mode == ThemeMode::Dark,
                    nerd.as_ref(),
                    13.,
                    file_badge(path.as_str(), theme),
                ),
            };
            let title = match &entry {
                AcEntry::Command { name, .. } => format!("/{name}"),
                AcEntry::File { path } => path.rsplit('/').next().unwrap_or(path).to_string(),
            };
            let subtitle = match &entry {
                AcEntry::Command { description, .. } => description.clone(),
                // The directory part, shown dimmed after the basename.
                AcEntry::File { path } => match path.rsplit_once('/') {
                    Some((dir, _)) if !dir.is_empty() => format!("{dir}/"),
                    _ => String::new(),
                },
            };
            list = list.child(
                div()
                    .id(ElementId::NamedInteger("ac-row".into(), ix as u64))
                    .h(px(30.))
                    .px(px(8.))
                    .rounded(px(6.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .cursor_pointer()
                    .when(selected, |row| row.bg(theme.active))
                    .hover(|style| style.bg(theme.overlay))
                    .on_click(move |_, _, cx| {
                        this.update(cx, |app, cx| app.commit_entry(entry.clone(), cx))
                            .ok();
                    })
                    .child(leading)
                    .child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            // Fuzzy match first: the basename (or command
                            // name) leads, then — after a breath — the rest
                            // of the path / description in dimmed text.
                            .child(
                                div()
                                    .flex_none()
                                    .max_w(px(CONTENT_MAX_W / 2.))
                                    .truncate()
                                    .text_size(theme.ui_px(12.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(if selected { theme.active_fg } else { theme.text_2 })
                                    .child(title),
                            )
                            .when(!subtitle.is_empty(), |row| {
                                row.child(
                                    div()
                                        .min_w_0()
                                        .flex_1()
                                        .truncate()
                                        .text_size(theme.ui_px(11.))
                                        .text_color(theme.text_3)
                                        .child(subtitle),
                                )
                            }),
                    ),
            );
        }
        // Full width of the chat box, so long paths are never cut.
        let popup = div()
            .w(px(CONTENT_MAX_W))
            .font_family(theme::ui_font_family())
            .rounded(px(10.))
            .border_1()
            .border_color(theme.border_strong)
            .bg(theme.menu_bg)
            .shadow(theme.popover_shadow())
            .flex()
            .flex_col()
            .overflow_hidden()
            .occlude()
            .on_mouse_down_out(cx.listener(Self::on_autocomplete_outside_down))
            .child(list);

        Some(
            anchored()
                .position_mode(AnchoredPositionMode::Local)
                .anchor(Corner::BottomLeft)
                .offset(point(px(0.), px(-4.)))
                .snap_to_window()
                .child(deferred(popup))
                .into_any_element(),
        )
    }

    /// Attachment chips row shown above the input (pasted/picked images).
    fn attachments_row(&self, cx: &Context<Self>) -> Option<AnyElement> {
        if self.attachments.is_empty() {
            return None;
        }
        let theme = *theme::get(cx);
        let row = div().w_full().flex().flex_wrap().gap(px(6.));
        Some(
            row.children(self.attachments.iter().enumerate().map(|(ix, a)| {
                // Image attachments show the decoded image; the glyph is the
                // fallback if a preview never decoded.
                let visual: AnyElement = match &a.preview {
                    Some(image) => div()
                        .size(px(18.))
                        .flex_none()
                        .rounded(px(4.))
                        .overflow_hidden()
                        .child(
                            img(ImageSource::Image(image.clone()))
                                .size_full()
                                .object_fit(ObjectFit::Cover),
                        )
                        .into_any_element(),
                    None => file_glyph(
                        &a.name,
                        theme.mode == ThemeMode::Dark,
                        nerd_font_family(cx).as_ref(),
                        12.,
                        icon("icons/task.svg", 12., theme.text_3).into_any_element(),
                    )
                    .into_any_element(),
                };
                div()
                    .id(ElementId::NamedInteger("attachment".into(), ix as u64))
                    .h(px(26.))
                    .pl(px(7.))
                    .pr(px(6.))
                    .rounded(px(6.))
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.bg_raised)
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .cursor_pointer()
                    .hover(|s| s.bg(theme.overlay))
                    .child(visual)
                    .child(
                        div()
                            .max_w(px(160.))
                            .truncate()
                            .text_size(theme.ui_px(11.5))
                            .text_color(theme.text_2)
                            .child(a.name.clone()),
                    )
                    .child(
                        div()
                            .text_size(theme.ui_px(11.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.text_3)
                            .child("×".to_string()),
                    )
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(move |this, _: &MouseUpEvent, _, cx| {
                            if ix < this.attachments.len() {
                                this.attachments.remove(ix);
                                cx.notify();
                            }
                        }),
                    )
            }))
            .into_any_element(),
        )
    }

    /// Files dragged over the window (external OS drag — gpui mirrors them
    /// as an `ExternalPaths` drag). `on_drag_move` fires for *every* move
    /// during the drag with this element's bounds, so one handler tracks
    /// both entering and leaving the composer.
    fn on_file_drag_move(
        &mut self,
        event: &DragMoveEvent<ExternalPaths>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.settings_open {
            return;
        }
        let hovered = event.bounds.contains(&event.event.position);
        if hovered != self.file_drag_hovered {
            self.file_drag_hovered = hovered;
            cx.notify();
        }
    }

    /// Files dropped anywhere on the window: images become attachments
    /// (thumbnail chips above the composer); anything else is referenced by
    /// path at the caret so the agent can read it with its tools.
    fn on_file_drop(&mut self, paths: &ExternalPaths, window: &mut Window, cx: &mut Context<Self>) {
        self.file_drag_hovered = false;
        if self.settings_open {
            cx.notify();
            return;
        }
        for path in paths.paths() {
            if self.attachments.len() >= MAX_ATTACHMENTS {
                self.status = format!("at most {MAX_ATTACHMENTS} images per message");
                break;
            }
            match Attachment::from_path(path) {
                Some(attachment) => self.attachments.push(attachment),
                None => {
                    self.input.update(cx, |input, cx| {
                        input.insert_at_caret(&format!("{} ", path.display()), cx);
                    });
                    self.input.read(cx).focus(window);
                }
            }
        }
        cx.notify();
    }

    /// Click on the attach chip: pick an image file and queue it.
    fn on_attach_click(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.attachments.len() >= MAX_ATTACHMENTS {
            self.status = format!("at most {MAX_ATTACHMENTS} images per message");
            cx.notify();
            return;
        }
        let Some(path) = rfd::FileDialog::new()
            .set_title("Attach an image")
            .add_filter("Image", &["png", "jpg", "jpeg", "webp", "gif", "bmp"])
            .pick_file()
        else {
            return;
        };
        match Attachment::from_path(&path) {
            Some(attachment) => {
                self.attachments.push(attachment);
            }
            None => {
                self.status = "unsupported image format".into();
            }
        }
        cx.notify();
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
        self.sync_session_menu(cx);
        cx.notify();
    }

    fn on_open_session(&mut self, session: SessionInfo, cx: &mut Context<Self>) {
        self.switch_to_session(session, true, cx);
    }

    /// Switch the live session. With `push`, the visit is recorded in the
    /// top-bar history (forward entries are dropped, like browser history).
    ///
    /// Each session gets its own pi process, so switching never interrupts a
    /// run: the outgoing session is *parked* mid-run (its process and live
    /// transcript keep going in the background — events drain every tick),
    /// and a parked target resumes exactly where it left off. Idle sessions
    /// are torn down and reload from disk when reopened.
    fn switch_to_session(&mut self, session: SessionInfo, push: bool, cx: &mut Context<Self>) {
        if self.current_session_path.as_ref() == Some(&session.path) {
            return;
        }
        // ── park the outgoing session ──
        if let Some(old_path) = self.current_session_path.take() {
            let old_busy = self.busy || self.transcript.is_streaming();
            if let Some(client) = self.client.take() {
                if old_busy {
                    // Swap the transcript out first so the park doesn't
                    // borrow `self.transcript` while `self` is borrowed.
                    let transcript = std::mem::replace(&mut self.transcript, Transcript::new());
                    self.park(
                        old_path,
                        ParkedSession {
                            client,
                            transcript,
                            busy: true,
                            added: self.added,
                            removed: self.removed,
                        },
                    );
                }
                // Idle: drop the client — the process is torn down and the
                // session reloads from pi's session file when reopened.
            }
        }
        self.busy = false;
        self.added = 0;
        self.removed = 0;
        self.transcript = Transcript::new();

        // ── activate the target ──
        if let Some(parked) = self.lives.remove(&session.path) {
            // Resume a background run. The parked transcript is already up
            // to date (its events drain every tick); anything buffered in
            // the process channel streams in from the next tick on.
            self.client = Some(parked.client);
            self.transcript = parked.transcript;
            self.busy = parked.busy;
            self.added = parked.added;
            self.removed = parked.removed;
            self.send(CommandBody::GetState, "get_state");
            self.refresh_context_stats();
        } else {
            // Fresh open: spawn a dedicated pi process rooted at the
            // session's workspace and point it at the session file. The
            // `switch_session` response triggers the get_messages snapshot.
            let spawned = PiClient::spawn(&session.cwd, None).or_else(|_| {
                PiClient::spawn(
                    &std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
                    None,
                )
            });
            match spawned {
                Ok(client) => {
                    self.client = Some(client);
                    self.send(
                        CommandBody::SwitchSession {
                            session_path: session.path.to_string_lossy().into_owned(),
                        },
                        "switch_session",
                    );
                    // `get_state` is sent from the switch_session success
                    // handler: querying it eagerly here races the switch,
                    // and pi answers with the default model instead of the
                    // session's own.
                }
                Err(err) => {
                    self.client = None;
                    self.status = format!("pi spawn failed: {err}");
                }
            }
        }
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
        // The top-bar info affordance shows the active session's details.
        self.session_details_open = !self.session_details_open;
        cx.notify();
    }

    /// Stage, commit, and push the workspace's changes (the details popover's
    /// "Commit or push" row). Runs off the main thread.
    fn commit_and_push(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        let Some(cwd) = self
            .current_workspace
            .clone()
            .or_else(|| std::env::current_dir().ok())
        else {
            self.status = "No workspace".into();
            cx.notify();
            return;
        };
        if self.branch_operation_pending {
            return;
        }
        self.branch_operation_pending = true;
        self.session_details_open = false;
        cx.notify();

        let message = format!("orbit: {}", chrono::Local::now().format("%Y-%m-%d %H:%M"));
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { crate::git::commit_and_push(&cwd, &message) })
                .await;
            let _ = this.update(cx, |app, cx| {
                app.branch_operation_pending = false;
                match result {
                    Ok(msg) => app.status = msg,
                    Err(err) => app.status = format!("Commit or push failed: {err}"),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// The top-bar info popover: active session's environment + identifiers.
    fn render_session_details_popup(&self, cx: &Context<Self>) -> Option<AnyElement> {
        if !self.session_details_open {
            return None;
        }
        let theme = *theme::get(cx);
        let this = cx.entity();
        let title = self.current_title.clone().unwrap_or_else(|| "New task".into());
        let session_id = self.session_id.clone().unwrap_or_default();
        let session_file = self
            .current_session_path
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        let workspace = self
            .current_workspace
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| {
                std::env::current_dir()
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_default()
            });
        let model = self.model_label.clone();
        let thinking = self.thinking_label.clone();

        let popup = div()
            .w(px(300.))
            .font_family(theme::ui_font_family())
            .rounded(px(8.))
            .border_1()
            .border_color(theme.border_strong)
            .bg(theme.menu_bg)
            .shadow(theme.popover_shadow())
            .flex()
            .flex_col()
            .overflow_hidden()
            .occlude()
            .on_mouse_down_out({
                let this = this.clone();
                move |_: &MouseDownEvent, _, cx: &mut App| {
                    this.update(cx, |app, cx| {
                        if app.session_details_open {
                            app.session_details_open = false;
                            cx.notify();
                        }
                    });
                }
            })
            .child(
                div()
                    .px(px(12.))
                    .py(px(10.))
                    .border_b_1()
                    .border_color(theme.border)
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(
                        div()
                            .text_size(theme.ui_px(12.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.text)
                            .child(title),
                    )
                    .child(
                        div()
                            .text_size(theme.ui_px(11.))
                            .text_color(theme.text_3)
                            .child(if session_id.is_empty() {
                                "No active session".to_string()
                            } else {
                                "Session details".to_string()
                            }),
                    ),
            )
            .child(
                div()
                    .px(px(12.))
                    .py(px(6.))
                    .text_size(theme.ui_px(11.))
                    .text_color(theme.text_3)
                    .child("Environment"),
            )
            .child(
                div()
                    .id(ElementId::Name("sess-commit-push".into()))
                    .px(px(12.))
                    .py(px(8.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .cursor_pointer()
                    .hover(|s| s.bg(theme.bg_hover))
                    .on_click({
                        let this = this.clone();
                        move |_, window, cx| {
                            this.update(cx, |app, cx| {
                                app.commit_and_push(window, cx);
                            });
                        }
                    })
                    .child(icon("icons/branch.svg", 13., theme.text_2))
                    .child(
                        div()
                            .flex_1()
                            .text_size(theme.ui_px(12.))
                            .text_color(theme.text)
                            .child("Commit or push"),
                    )
                    .child(icon("icons/chevron-right.svg", 11., theme.text_3)),
            )
            .child(
                div()
                    .id(ElementId::Name("sess-compare-branch".into()))
                    .px(px(12.))
                    .py(px(8.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .cursor_pointer()
                    .hover(|s| s.bg(theme.bg_hover))
                    .on_click({
                        let this = this.clone();
                        move |_, window, cx| {
                            this.update(cx, |app, cx| {
                                app.toggle_branch_picker(window, cx);
                            });
                        }
                    })
                    .child(icon("icons/file-diff.svg", 13., theme.text_2))
                    .child(
                        div()
                            .flex_1()
                            .text_size(theme.ui_px(12.))
                            .text_color(theme.text)
                            .child("Compare branch"),
                    )
                    .child(icon("icons/chevron-right.svg", 11., theme.text_3)),
            )
            .child(self.session_detail_row(0, "Session ID", &session_id, theme))
            .child(self.session_detail_row(1, "Session file", &session_file, theme))
            .child(self.session_detail_row(2, "Workspace", &workspace, theme))
            .child(self.session_detail_row(3, "Model", &model, theme))
            .child(self.session_detail_row(4, "Thinking", &thinking, theme));

        Some(
            div()
                .absolute()
                .bottom_0()
                .right_0()
                .size(px(0.))
                .child(
                    anchored()
                        .position_mode(AnchoredPositionMode::Local)
                        .anchor(Corner::TopRight)
                        .offset(point(px(0.), px(4.)))
                        .snap_to_window()
                        .child(deferred(popup)),
                )
                .into_any_element(),
        )
    }

    /// A read-only identifier row in the session-details popover, with a copy
    /// affordance that copies `value` to the clipboard.
    fn session_detail_row(
        &self,
        ix: usize,
        label: &str,
        value: &str,
        theme: Theme,
    ) -> impl IntoElement + use<> {
        let label = label.to_string();
        let value = value.to_string();
        let v = value.clone();
        div()
            .px(px(12.))
            .py(px(8.))
            .flex()
            .items_center()
            .gap(px(8.))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(
                        div()
                            .text_size(theme.ui_px(11.))
                            .text_color(theme.text_3)
                            .child(label),
                    )
                    .child(
                        div()
                            .text_size(theme.ui_px(12.))
                            .text_color(theme.text)
                            .truncate()
                            .child(if value.is_empty() { "—".to_string() } else { value }),
                    ),
            )
            .child(
                div()
                    .id(("sess-copy", ix))
                    .p_1()
                    .rounded_sm()
                    .cursor_pointer()
                    .hover(|s| s.bg(theme.bg_hover))
                    .on_click(move |_, _window, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(v.clone()));
                    })
                    .child(icon("icons/copy.svg", 12., theme.text_3)),
            )
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

    // ── open in (external editor / terminal) ───────────────────────────

    /// Resolve installed folder-capable apps once, off-thread.
    pub(crate) fn detect_open_in_apps(&self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let apps = cx
                .background_executor()
                .spawn(async move { platform::detect_open_in_apps() })
                .await;
            if apps.is_empty() {
                return;
            }
            let _ = this.update(cx, |this, cx| {
                this.open_in_apps = Rc::new(apps);
                cx.notify();
            });
        })
        .detach();
    }

    fn preferred_open_in_app<'a>(&'a self, _: &App) -> Option<&'a ExternalApp> {
        self.preferred_open_in_app
            .as_deref()
            .and_then(|id| self.open_in_apps.iter().find(|app| app.id == id))
            .or_else(|| self.open_in_apps.iter().find(|app| app.id == "finder"))
            .or_else(|| self.open_in_apps.first())
    }

    fn open_workspace_in_app(&mut self, path: &Path, app_id: &str, cx: &mut Context<Self>) {
        let Some(bundle_id) = self
            .open_in_apps
            .iter()
            .find(|app| app.id == app_id)
            .map(|app| app.bundle_id)
        else {
            return;
        };
        platform::open_path_in_app(path, bundle_id);
        if self.preferred_open_in_app.as_deref() != Some(app_id) {
            self.preferred_open_in_app = Some(app_id.to_owned());
            platform::persist_preferred_open_in_app(app_id);
        }
        self.open_in_menu_open = false;
        cx.notify();
    }

    fn on_open_in_primary(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(path) = self.current_workspace.clone() else {
            return;
        };
        let Some(app) = self.preferred_open_in_app(cx) else {
            return;
        };
        let app_id = app.id;
        self.open_workspace_in_app(&path, app_id, cx);
    }

    fn on_open_in_caret(&mut self, _: &MouseUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.open_in_apps.is_empty() || self.current_workspace.is_none() {
            return;
        }
        self.open_in_menu_open = !self.open_in_menu_open;
        if self.open_in_menu_open {
            self.open_in_filter.update(cx, |filter, cx| filter.clear(cx));
            let handle = self.open_in_filter.read(cx).focus_handle(cx);
            window.focus(&handle);
        }
        cx.notify();
    }

    /// Split "open in" control: preferred app icon on the left, chevron menu
    /// on the right listing every installed editor/terminal.
    fn render_open_in_control(&self, cx: &Context<Self>) -> Option<AnyElement> {
        let path = self.current_workspace.as_ref()?;
        let preferred = self.preferred_open_in_app(cx)?;
        if self.open_in_apps.is_empty() {
            return None;
        }

        let theme = *theme::get(cx);
        let preferred_id = preferred.id;
        let preferred_icon = preferred.icon.clone();
        let this = cx.entity().clone();
        let path = Rc::from(path.as_path());

        let primary = div()
            .id("header-open-in")
            .h_full()
            .px(px(6.))
            .rounded_tl(px(6.))
            .rounded_bl(px(6.))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .hover(|s| s.bg(theme.overlay))
            .active(|s| s.bg(theme.active).text_color(theme.active_fg))
            .child(
                img(ImageSource::Image(preferred_icon))
                    .size(px(16.))
                    .flex_none(),
            )
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_open_in_primary));

        let caret = div()
            .id("header-open-in-caret")
            .relative()
            .h_full()
            .w(px(18.))
            .rounded_tr(px(6.))
            .rounded_br(px(6.))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .hover(|s| s.bg(theme.overlay))
            .when(self.open_in_menu_open, |s| s.bg(theme.active).text_color(theme.active_fg))
            .child(icon("icons/chevron-down.svg", 11., theme.text_3))
            // Pin the dropdown to the caret's bottom-right — same zero-size
            // anchor trick as the session row menu, so flex centering doesn't
            // pull the popup toward the button's middle.
            .children(self.open_in_menu_popup(&this, path, preferred_id, theme, cx))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_open_in_caret));

        Some(
            div()
                .h(px(28.))
                .rounded(px(7.))
                .border_1()
                .border_color(theme.border_strong)
                .flex_none()
                .flex()
                .items_center()
                .child(primary)
                .child(div().w(px(1.)).h_full().flex_none().bg(theme.border))
                .child(caret)
                .into_any_element(),
        )
    }

    fn open_in_menu_popup(
        &self,
        this: &Entity<OrbitApp>,
        path: Rc<Path>,
        preferred_id: &str,
        theme: Theme,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        if !self.open_in_menu_open {
            return None;
        }

        let preferred_id = preferred_id.to_string();
        let needle = self.open_in_filter.read(cx).text().to_lowercase();
        let apps: Vec<ExternalApp> = self
            .open_in_apps
            .iter()
            .filter(|app| needle.is_empty() || app.label.to_lowercase().contains(&needle))
            .cloned()
            .collect();
        let mut list = div()
            .w_full()
            .px(px(4.))
            .py(px(4.))
            .flex()
            .flex_col()
            .gap(px(2.));
        for app in apps.iter() {
            let selected = app.id == preferred_id;
            let this = this.clone();
            let path = path.clone();
            let app_id = app.id;
            let app_icon = app.icon.clone();
            let label = app.label;
            list = list.child(
                div()
                    .id(ElementId::Name(format!("open-in-{}", app.id).into()))
                    .h(px(28.))
                    .px(px(8.))
                    .rounded(px(6.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .cursor_pointer()
                    .when(selected, |row| row.bg(theme.active))
                    .hover(|style| style.bg(theme.overlay))
                    .child(img(ImageSource::Image(app_icon)).size(px(16.)).flex_none())
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(theme.ui_px(12.))
                            .text_color(if selected {
                                theme.active_fg
                            } else {
                                theme.text_2
                            })
                            .child(label),
                    )
                    .when(selected, |row| {
                        row.child(icon("icons/check.svg", 11., theme.accent))
                    })
                    .on_mouse_up(MouseButton::Left, move |_, _, cx| {
                        this.update(cx, |app, cx| {
                            app.open_workspace_in_app(&path, app_id, cx);
                        });
                    }),
            );
        }

        if apps.is_empty() {
            list = list.child(
                div()
                    .h(px(28.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(theme.ui_px(12.))
                    .text_color(theme.text_3)
                    .child("No matches"),
            );
        }

        let popup = div()
            .w(px(200.))
            .font_family(theme::ui_font_family())
            .rounded(px(8.))
            .border_1()
            .border_color(theme.border_strong)
            .bg(theme.menu_bg)
            .shadow(theme.popover_shadow())
            .flex()
            .flex_col()
            .overflow_hidden()
            .occlude()
            .on_mouse_down_out({
                let this = this.clone();
                move |_: &MouseDownEvent, _, cx: &mut App| {
                    this.update(cx, |app, cx| {
                        if app.open_in_menu_open {
                            app.open_in_menu_open = false;
                            cx.notify();
                        }
                    });
                }
            })
            // Search field — filters the apps below.
            .child(
                div()
                    .h(px(30.))
                    .px(px(8.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .border_b_1()
                    .border_color(theme.border)
                    .text_size(theme.ui_px(12.))
                    .child(icon("icons/search.svg", 13., theme.text_3))
                    .child(self.open_in_filter.clone()),
            )
            .child(list);

        Some(
            div()
                .absolute()
                .bottom_0()
                .right_0()
                .size(px(0.))
                .child(
                    anchored()
                        .position_mode(AnchoredPositionMode::Local)
                        .anchor(Corner::TopRight)
                        .offset(point(px(0.), px(4.)))
                        .snap_to_window()
                        .child(deferred(popup)),
                )
                .into_any_element(),
        )
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

// ── sidebar model ──────────────────────────────────────────────────────────

enum SideRow {
    /// Workspace group header: label, session count, collapsed state.
    Workspace {
        label: String,
        count: usize,
        collapsed: bool,
        /// Workspace path — used to spawn a new session in this project.
        cwd: PathBuf,
    },
    /// Session row — index into the (newest-first) sessions list.
    Session(usize),
    /// Expand a workspace group to reveal hidden sessions (`count` = how many).
    ShowMore { label: String, count: usize },
    /// Collapse a workspace group back to the truncated list.
    ShowLess { label: String },
}

/// State of the row-actions popup in the sessions sidebar: which session
/// it belongs to (by path), what the row can offer, and whether the popup
/// is currently showing the delete confirmation.
#[derive(Clone, PartialEq)]
struct SessionMenu {
    path: PathBuf,
    /// Copy of the row title for the confirm copy.
    title: String,
    /// Sessions with a live pi process (active or parked) must not be
    /// deleted — the process would recreate the file mid-run.
    deletable: bool,
    /// The popup is showing the delete confirmation instead of the menu.
    confirm_delete: bool,
}

/// Sections of the settings surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SettingsSection {
    General,
    Appearance,
    Providers,
    About,
}

/// Which dropdown is open on the settings surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SettingsSelect {
    Theme,
    Language,
    UiFont,
    CodeFont,
    UiFontFamily,
    CodeFontFamily,
}

impl Render for OrbitApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = *theme::get(cx);
        // `/`-command and `@`-file menu state derives from the composer text
        // every frame, so typing opens/closes/filters it without extra sync.
        self.sync_autocomplete(cx);
        let working_label = self.workspace_label();
        // Workspace groups (ordered by each group's most recently active
        // session). Only the active workspace is expanded by default; each
        // open group shows up to SIDEBAR_GROUP_SESSIONS_VISIBLE sessions
        // with per-group Show more / Show less toggles.
        let side_rows = Rc::new(build_sidebar_rows(
            &self.sessions,
            &working_label,
            &self.collapsed_workspaces,
            &self.expanded_workspace_groups,
            &self.expanded_session_groups,
            &self.current_session_path,
        ));
        let old = self.sidebar_list.item_count();
        if old != side_rows.len() {
            self.sidebar_list.splice(0..old, side_rows.len());
        }
        let sessions_data = Rc::new(self.sessions.clone());
        let active_path = Rc::new(self.current_session_path.clone());
        let session_menu = Rc::new(self.session_menu.clone());
        let this = cx.entity();
        // The open session's agent activity, plus which parked (background)
        // sessions are mid-run — both drive the sidebar's running loader.
        let agent_running = self.busy || self.transcript.is_streaming();
        let running_paths: Rc<HashSet<PathBuf>> = Rc::new(
            self.lives
                .iter()
                .filter(|(_, parked)| parked.busy)
                .map(|(path, _)| path.clone())
                .collect(),
        );

        let workspace_label = self.workspace_label();
        let review_workspace = self
            .current_workspace
            .clone()
            .or_else(|| std::env::current_dir().ok());
        // The rail gates on the main area's width (Waku: 872px transcript
        // container), which excludes the sessions sidebar when visible.
        let viewport = window.viewport_size();
        let main_width = viewport.width
            - if self.sidebar_visible && !self.settings_open {
                self.sidebar_width
            } else {
                px(0.)
            };

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
                    .hover(|s| s.bg(theme.bg_hover))
                    .on_mouse_up(MouseButton::Left, cx.listener(Self::on_toggle_sidebar))
                    .child(icon("icons/panel-left.svg", 16., theme.text_2)),
            )
            .child(
                div()
                    .id("history-back")
                    .p_1()
                    .rounded_sm()
                    .when(back_enabled, |b| {
                        b.cursor_pointer()
                            .hover(|s| s.bg(theme.bg_hover))
                            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_history_back))
                    })
                    .child(icon(
                        "icons/arrow-left.svg",
                        14.,
                        if back_enabled {
                            theme.text_2
                        } else {
                            theme.text_3
                        },
                    )),
            )
            .child(
                div()
                    .id("history-forward")
                    .p_1()
                    .rounded_sm()
                    .when(forward_enabled, |b| {
                        b.cursor_pointer()
                            .hover(|s| s.bg(theme.bg_hover))
                            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_history_forward))
                    })
                    .child(icon(
                        "icons/arrow-right.svg",
                        14.,
                        if forward_enabled {
                            theme.text_2
                        } else {
                            theme.text_3
                        },
                    )),
            );

        // ── top-bar right controls ──
        let mut top_controls = div().flex().items_center().gap_2();
        top_controls = top_controls.children(self.render_open_in_control(cx));
        top_controls = top_controls
            .child(
                div()
                    .text_size(theme.ui_px(12.))
                    .text_color(theme.add_green)
                    .child(format!("+{}", self.added)),
            )
            .child(
                div()
                    .text_size(theme.ui_px(12.))
                    .text_color(theme.del_red)
                    .child(format!("-{}", self.removed)),
            )
            .child(
                div()
                    .id("info")
                    .relative()
                    .p_1()
                    .rounded_sm()
                    .cursor_pointer()
                    .hover(|s| s.bg(theme.bg_hover))
                    .on_mouse_up(MouseButton::Left, cx.listener(Self::on_info_click))
                    .children(self.render_session_details_popup(cx))
                    .child(icon("icons/info.svg", 16., theme.text_2)),
            );

        div()
            .size_full()
            .flex()
            .bg(theme.bg_main)
            .text_color(theme.text)
            .font_family(theme::ui_font_family())
            // Dropping files anywhere in the window attaches them (the
            // composer highlights when the drag passes over it).
            .on_drop(cx.listener(Self::on_file_drop))
            // ── sidebar ── (hidden while the settings surface is open —
            // settings is a full-window surface with its own nav, like the
            // reference UI)
            .children((self.sidebar_visible && !self.settings_open).then(|| {
                div()
                    .id("sidebar")
                    .relative()
                    .w(self.sidebar_width)
                    .h_full()
                    .bg(theme.bg_sidebar)
                    .border_r_1()
                    .border_color(theme.border)
                    .flex()
                    .flex_col()
                    // traffic-light strip (drag region)
                    .child(
                        div()
                            .h(px(38.))
                            .w_full()
                            .window_control_area(WindowControlArea::Drag),
                    )
                    // Resize handle: drag the sidebar's right edge to adjust
                    // its width. The drag-move listener lives on the root so
                    // the drag keeps tracking beyond the handle.
                    .child(
                        div()
                            .id("sidebar-resize-handle")
                            .absolute()
                            .top_0()
                            .bottom_0()
                            .right(px(-3.))
                            .w(px(6.))
                            .cursor(CursorStyle::ResizeLeftRight)
                            .hover(|style| style.bg(theme.accent.opacity(0.4)))
                            .on_drag(
                                SidebarResize,
                                |_, _, _, cx| cx.new(|_| DragGhost),
                            ),
                    )
                    // nav
                    .child(
                        div()
                            .px_2()
                            .pt_1()
                            .flex()
                            .flex_col()
                            .gap_0p5()
                            .child(self.sidebar_action_row(
                                "sidebar-new-session",
                                "icons/compose.svg",
                                "New Task",
                                Some(Box::new(cx.listener(|this, _: &MouseUpEvent, w, cx| {
                                    this.on_new_session(&crate::NewSession, w, cx)
                                }))),
                                theme,
                            ))
                            .child(self.sidebar_action_row(
                                "sidebar-search",
                                "icons/search.svg",
                                "Search",
                                Some(Box::new(cx.listener(|this, _: &MouseUpEvent, w, cx| {
                                    this.toggle_session_picker(w, cx)
                                }))),
                                theme,
                            ))
                            // session switcher palette, anchored under the
                            // Search row (rendered only while open)
                            .children(self.session_palette()),
                    )
                    // session list (scrolls), grouped by workspace — or the
                    // empty state when pi's store has no sessions yet
                    .child(if self.sessions.is_empty() {
                        empty_sessions_state(theme).into_any_element()
                    } else {
                        div()
                            .id("sidebar-sessions")
                            .flex_1()
                            .min_h_0()
                            .px_2()
                            .relative()
                            .child(
                                list(self.sidebar_list.clone(), move |ix, _window, cx| {
                                    render_side_row(
                                        &side_rows,
                                        &sessions_data,
                                        active_path.as_deref(),
                                        ix,
                                        &this,
                                        agent_running,
                                        &running_paths,
                                        session_menu.as_ref().as_ref(),
                                        *theme::get(cx),
                                    )
                                    .into_any_element()
                                })
                                .w_full()
                                .h_full(),
                            )
                            .into_any_element()
                    })
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
                                    .hover(|s| s.bg(theme.bg_hover))
                                    .on_mouse_up(
                                        MouseButton::Left,
                                        cx.listener(Self::on_settings_gear_click),
                                    )
                                    .child(icon("icons/settings.svg", 16., theme.text_3)),
                            )
                            .child(div().flex_1())
                            .child(div().size(px(7.)).rounded_full().bg(
                                if self.client.is_some() {
                                    theme.ok_green
                                } else {
                                    theme.stop_red
                                },
                            )),
                    )
            }))
            // ── main ──
            .child(if self.settings_open {
                self.render_settings(cx).into_any_element()
            } else if !self.dependencies_ready() {
                // Missing runtime pieces (pi / node): show the setup page
                // with install commands instead of the empty composer.
                self.render_onboarding(cx).into_any_element()
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
                                        div()
                                            .text_size(theme.ui_px(13.))
                                            .text_color(theme.text_2)
                                            .child(
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
                        self.render_empty_state(cx).into_any_element()
                    } else {
                        div()
                            .flex_1()
                            .min_h_0()
                            .w_full()
                            .relative()
                            .child(self.transcript.render(
                                review_workspace.as_deref(),
                                window.viewport_size().height,
                                main_width,
                                cx,
                            ))
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
                                    // `/`-command and `@`-file menu — anchored
                                    // above the composer box (same deferred
                                    // + anchored pattern as the chip pickers)
                                    .children(self.autocomplete_popup(cx))
                                    // composer box — the picker popups are
                                    // anchored above their own chips
                                    .child(
                                        div()
                                            .w_full()
                                            .relative()
                                            .bg(theme.bg_composer)
                                            .border_1()
                                            .border_color(if self.file_drag_hovered {
                                                theme.accent
                                            } else {
                                                theme.border
                                            })
                                            .rounded_lg()
                                            .px_3()
                                            .pt_2()
                                            .pb_2()
                                            // Base interface font for the input
                                            // (scales with the UI font-size
                                            // setting); the editor inherits it.
                                            .text_size(theme.ui_px(14.))
                                            .flex()
                                            .flex_col()
                                            .gap_2()
                                            .on_mouse_up(
                                                MouseButton::Left,
                                                cx.listener(Self::on_composer_click),
                                            )
                                            .on_drag_move(cx.listener(Self::on_file_drag_move))
                                            .children(self.attachments_row(cx))
                                            .child(self.input.clone())
                                            .child(self.composer_row(cx))
                                            // Drop-target overlay (Waku): fades
                                            // in over the box while files are
                                            // dragged across it. Absolute, so
                                            // highlighting never shifts layout.
                                            .children(self.file_drag_hovered.then(|| {
                                                div()
                                                    .absolute()
                                                    .inset_0()
                                                    .rounded_lg()
                                                    .bg(theme.bg_composer.opacity(0.92))
                                                    .border_1()
                                                    .border_color(theme.accent)
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .gap_2()
                                                    .child(icon(
                                                        "icons/plus.svg",
                                                        14.,
                                                        theme.accent,
                                                    ))
                                                    .child(
                                                        div()
                                                            .text_size(theme.ui_px(12.5))
                                                            .font_weight(FontWeight::MEDIUM)
                                                            .text_color(theme.accent)
                                                            .child("Drop to attach"),
                                                    )
                                                    .with_animation(
                                                        "drop-overlay",
                                                        Animation::new(Duration::from_millis(120)),
                                                        |overlay, delta| overlay.opacity(delta),
                                                    )
                                                    .into_any_element()
                                            })),
                                    )
                                    .child(self.status_bar(&workspace_label, cx)),
                            ),
                    )
                    .into_any_element()
            })
            .track_focus(&self.focus_handle(cx))
            // Sidebar resize: fires for every mouse move while the handle
            // drag is active, wherever the pointer travels.
            .on_drag_move(cx.listener(
                |app: &mut Self,
                 event: &DragMoveEvent<SidebarResize>,
                 _: &mut Window,
                 cx: &mut Context<Self>| {
                    let max = event.bounds.size.width - px(400.);
                    let width = event
                        .event
                        .position
                        .x
                        .clamp(px(SIDEBAR_MIN_W), max.max(px(SIDEBAR_MIN_W)));
                    if width != app.sidebar_width {
                        app.sidebar_width = width;
                        cx.notify();
                    }
                }),
            )
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
        let theme = *theme::get(cx);
        div()
            .flex()
            .items_center()
            .gap_2()
            // attach image (paperclip-style: file picker; paste also works)
            .child(
                div()
                    .id("attach-chip")
                    .h(px(24.))
                    .w(px(26.))
                    .rounded_md()
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .hover(|s| s.bg(theme.overlay))
                    .child(icon("icons/plus.svg", 13., theme.text_3))
                    .on_mouse_up(MouseButton::Left, cx.listener(Self::on_attach_click)),
            )
            .child(self.model_chip(cx))
            .child(self.thinking_chip(cx))
            // access mode (pi runs with full tool access)
            .child(pill_static(
                "icons/lock.svg",
                "Full access",
                *theme::get(cx),
            ))
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
        let theme = *theme::get(cx);
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
                    .text_size(theme.ui_px(12.))
                    .cursor_pointer()
                    .border_color(theme.border)
                    .bg(theme.bg_raised)
                    .hover(|s| s.bg(theme.overlay))
                    .when(self.picker_is_open(PickerKind::Model), |chip| {
                        chip.bg(theme.active)
                            .text_color(theme.active_fg)
                            .border_color(theme.border_strong)
                    })
                    .on_mouse_up(MouseButton::Left, cx.listener(Self::on_model_trigger_click))
                    .child(icon_dyn(
                        provider_icon(&self.model_provider),
                        12.,
                        theme.text_2,
                    ))
                    .child(div().text_color(theme.text).child(self.model_label.clone()))
                    .child(icon("icons/chevron-down.svg", 11., theme.text_3)),
            )
    }

    /// The thinking-level chip: level icon + reasoning level + caret.
    fn thinking_chip(&self, cx: &Context<Self>) -> impl IntoElement + use<> {
        let theme = *theme::get(cx);
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
                    .text_size(theme.ui_px(12.))
                    .cursor_pointer()
                    .border_color(theme.border)
                    .bg(theme.bg_raised)
                    .hover(|s| s.bg(theme.overlay))
                    .when(self.picker_is_open(PickerKind::Thinking), |chip| {
                        chip.bg(theme.active)
                            .text_color(theme.active_fg)
                            .border_color(theme.border_strong)
                    })
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(Self::on_thinking_trigger_click),
                    )
                    .child({
                        let (path, color) = thinking_icon(&self.thinking_label, &theme);
                        icon(path, 15., color)
                    })
                    .child(
                        div()
                            .text_color(theme.text)
                            .child(thinking_display(&self.thinking_label)),
                    )
                    .child(icon("icons/chevron-down.svg", 11., theme.text_3)),
            )
    }

    /// Fading dot-grid backdrop for the new-task page. GPUI tints the SVG
    /// alpha mask with a theme color, so the art must be explicit circles
    /// (patterns/masks do not survive the renderer).
    fn new_task_background(theme: Theme) -> impl IntoElement + use<> {
        let dot_color = match theme.mode {
            ThemeMode::Light => theme.text_3.opacity(0.38),
            ThemeMode::Dark => theme.text_3.opacity(0.28),
        };
        div()
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .bottom_0()
            .child(
                svg()
                    .path("backgrounds/new-task-dots.svg")
                    .size_full()
                    .text_color(dot_color),
            )
    }

    /// New-task empty state — minimal onboarding over the dot grid: one
    /// headline, a ghost workspace row, and the composer below for input.
    fn render_empty_state(&self, cx: &Context<Self>) -> impl IntoElement + use<> {
        let theme = *theme::get(cx);
        let cwd = self
            .current_workspace
            .clone()
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_default();
        let folder_name = sessions::workspace_label(&cwd);
        let path_label = cwd.to_string_lossy().into_owned();

        div()
            .flex_1()
            .min_h_0()
            .w_full()
            .relative()
            .overflow_hidden()
            .child(Self::new_task_background(theme))
            .child(
                div()
                    .relative()
                    .size_full()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .px(px(24.))
                    .pb(px(88.))
                    .child(
                        div()
                            .w_full()
                            .max_w(px(400.))
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap(px(20.))
                            // Spark mark — soft accent wash, no heavy card chrome.
                            .child(
                                div()
                                    .size(px(44.))
                                    .rounded_full()
                                    .bg(theme.accent.opacity(0.12))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(icon("icons/spark.svg", 18., theme.accent)),
                            )
                            // Title block — one idea, one line of guidance.
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .items_center()
                                    .gap(px(6.))
                                    .child(
                                        div()
                                            .text_size(theme.ui_px(22.))
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(theme.text)
                                            .child("What should we build?"),
                                    )
                                    .child(
                                        div()
                                            .text_size(theme.ui_px(13.))
                                            .text_color(theme.text_3)
                                            .text_align(TextAlign::Center)
                                            .child("Pick a workspace, then describe your task below."),
                                    ),
                            )
                            // Workspace — single ghost row; the whole row opens the picker.
                            .child(
                                div()
                                    .id("pick-folder")
                                    .w_full()
                                    .px(px(10.))
                                    .py(px(8.))
                                    .rounded_lg()
                                    .flex()
                                    .items_center()
                                    .gap(px(10.))
                                    .cursor_pointer()
                                    .hover(|s| s.bg(theme.overlay))
                                    .active(|s| s.bg(theme.active).text_color(theme.active_fg))
                                    .on_mouse_up(
                                        MouseButton::Left,
                                        cx.listener(Self::on_pick_folder_click),
                                    )
                                    .child(
                                        div()
                                            .size(px(32.))
                                            .flex_none()
                                            .rounded_md()
                                            .bg(theme.bg_raised)
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .child(icon("icons/folder.svg", 15., theme.text_2)),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .flex()
                                            .flex_col()
                                            .gap(px(2.))
                                            .child(
                                                div()
                                                    .text_size(theme.ui_px(13.))
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(theme.text)
                                                    .truncate()
                                                    .child(folder_name),
                                            )
                                            .child(
                                                div()
                                                    .text_size(theme.ui_px(11.5))
                                                    .text_color(theme.text_3)
                                                    .truncate()
                                                    .child(path_label),
                                            ),
                                    )
                                    .child(icon("icons/chevron-down.svg", 12., theme.text_3)),
                            ),
                    ),
            )
    }

    /// Whether every required runtime dependency is installed.
    fn dependencies_ready(&self) -> bool {
        onboarding::all_required_installed(&self.deps)
    }

    /// Re-run the dependency probe and, if `pi` just became available, spawn
    /// the agent client. Runs the probe off the main thread and spins the
    /// setup page's Refresh button while it's in flight.
    fn refresh_setup(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        if self.refreshing {
            return; // ignore double-clicks while a refresh is running
        }
        self.refreshing = true;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let deps = cx
                .background_executor()
                .spawn(async {
                    // Short pause so the spinner reads as "working" rather than
                    // a flash before the probe returns.
                    std::thread::sleep(Duration::from_millis(250));
                    onboarding::check_dependencies()
                })
                .await;
            let ready = onboarding::all_required_installed(&deps);
            let _ = this.update(cx, |app, cx| {
                app.deps = deps;
                app.refreshing = false;
                if ready && app.client.is_none() {
                    let workspace = app
                        .current_workspace
                        .clone()
                        .or_else(|| std::env::current_dir().ok())
                        .unwrap_or_else(|| PathBuf::from("."));
                    match PiClient::spawn(&workspace, None) {
                        Ok(client) => {
                            app.client = Some(client);
                            app.send(CommandBody::GetState, "get_state");
                            app.refresh_catalogs();
                            app.status = "Connected".into();
                        }
                        Err(err) => app.status = format!("pi spawn failed: {err}"),
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Full-page setup screen shown when a required dependency is missing.
    fn render_onboarding(&self, cx: &Context<Self>) -> impl IntoElement + use<> {
        let theme = *theme::get(cx);
        let missing = onboarding::missing_required_count(&self.deps);

        div()
            .flex_1()
            .min_h_0()
            .w_full()
            .relative()
            .overflow_hidden()
            .child(Self::new_task_background(theme))
            .child(
                div()
                    .relative()
                    .size_full()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .px(px(24.))
                    .pb(px(40.))
                    .child(
                        div()
                            .w_full()
                            .max_w(px(560.))
                            .flex()
                            .flex_col()
                            .gap(px(18.))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .items_center()
                                    .gap(px(10.))
                                    .child(
                                        div()
                                            .size(px(44.))
                                            .rounded_full()
                                            .bg(theme.accent.opacity(0.12))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .child(icon("icons/spark.svg", 18., theme.accent)),
                                    )
                                    .child(
                                        div()
                                            .text_size(theme.ui_px(22.))
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(theme.text)
                                            .child("Set up Orbit"),
                                    )
                                    .child(
                                        div()
                                            .max_w(px(420.))
                                            .text_size(theme.ui_px(13.))
                                            .text_color(theme.text_3)
                                            .text_align(TextAlign::Center)
                                            .child(
                                                "A few pieces are missing before Orbit can run the pi agent. Install them, then refresh.",
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(10.))
                                    .children(self.deps.iter().enumerate().map(|(ix, dep)| {
                                        self.render_dependency_row(dep, ix, cx).into_any_element()
                                    })),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .gap(px(14.))
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(6.))
                                            .text_size(theme.ui_px(12.))
                                            .text_color(theme.text_3)
                                            .child(
                                                div().size(px(8.)).rounded_full().bg(theme.stop_red),
                                            )
                                            .child(if missing > 0 {
                                                format!("{missing} required piece(s) missing")
                                            } else {
                                                "Ready".to_string()
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("refresh-setup")
                                            .px(px(12.))
                                            .py(px(6.))
                                            .rounded_md()
                                            .flex()
                                            .items_center()
                                            .gap(px(6.))
                                            .when(self.refreshing, |b| b.opacity(0.55))
                                            .when(!self.refreshing, |b| {
                                                b.cursor_pointer().hover(|s| s.bg(theme.bg_hover))
                                            })
                                            .on_click({
                                                let this = cx.entity();
                                                move |_, window, cx| {
                                                    this.update(cx, |app, cx| {
                                                        app.refresh_setup(window, cx);
                                                    });
                                                }
                                            })
                                            .child(if self.refreshing {
                                                gpui::svg()
                                                    .path("icons/loader.svg")
                                                    .size(px(13.))
                                                    .text_color(theme.text_2)
                                                    .with_animation(
                                                        "refresh-spin",
                                                        Animation::new(Duration::from_millis(800))
                                                            .repeat(),
                                                        |svg, delta| {
                                                            svg.with_transformation(
                                                                Transformation::rotate(radians(
                                                                    delta * std::f32::consts::TAU,
                                                                )),
                                                            )
                                                        },
                                                    )
                                                    .into_any_element()
                                            } else {
                                                icon("icons/refresh.svg", 13., theme.text_2)
                                                    .into_any_element()
                                            })
                                            .child(
                                                div()
                                                    .text_size(theme.ui_px(12.))
                                                    .text_color(theme.text_2)
                                                    .child(if self.refreshing {
                                                        "Checking…"
                                                    } else {
                                                        "Refresh"
                                                    }),
                                            ),
                                    ),
                            ),
                    ),
            )
    }

    /// One dependency row: status dot, name + detail, and either a version
    /// chip (installed) or the install command with a copy affordance.
    fn render_dependency_row(
        &self,
        dep: &Dependency,
        ix: usize,
        cx: &Context<Self>,
    ) -> impl IntoElement + use<> {
        let theme = *theme::get(cx);
        let status_color = if dep.installed {
            theme.ok_green
        } else {
            theme.stop_red
        };

        div()
            .w_full()
            .px(px(14.))
            .py(px(12.))
            .rounded_lg()
            .bg(theme.bg_raised)
            .border_1()
            .border_color(theme.border)
            .flex()
            .items_center()
            .gap(px(12.))
            .child(div().size(px(9.)).rounded_full().bg(status_color))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(3.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(
                                div()
                                    .text_size(theme.ui_px(13.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme.text)
                                    .child(dep.name),
                            )
                            .child(
                                div()
                                    .text_size(theme.ui_px(11.))
                                    .text_color(theme.text_3)
                                    .child(if dep.required { "required" } else { "optional" }),
                            ),
                    )
                    .child(
                        div()
                            .text_size(theme.ui_px(11.5))
                            .text_color(theme.text_3)
                            .child(dep.detail),
                    ),
            )
            .child(if dep.installed {
                div()
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .text_size(theme.ui_px(11.5))
                    .text_color(theme.ok_green)
                    .child(icon("icons/check.svg", 12., theme.ok_green))
                    .child(dep.version.clone().unwrap_or_else(|| "installed".into()))
                    .into_any_element()
            } else {
                let cmd = dep.install_hint;
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .child(
                        div()
                            .px(px(8.))
                            .py(px(4.))
                            .rounded_md()
                            .bg(theme.code_bg)
                            .border_1()
                            .border_color(theme.border)
                            .text_size(theme.ui_px(11.))
                            .text_color(theme.code_text)
                            .child(cmd),
                    )
                    .child(
                        div()
                            .id(("copy", ix))
                            .p_1()
                            .rounded_sm()
                            .cursor_pointer()
                            .hover(|s| s.bg(theme.bg_hover))
                            .on_click(move |_, _window, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(cmd.to_string()));
                            })
                            .child(icon("icons/copy.svg", 13., theme.text_2)),
                    )
                    .into_any_element()
            })
    }

    /// Status bar under the composer: workspace / transport / branch on the
    /// left, used-context percent + ring on the right.
    fn status_bar(&self, workspace_label: &str, cx: &Context<Self>) -> impl IntoElement + use<> {
        let theme = *theme::get(cx);
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
            .text_size(theme.ui_px(11.5))
            .text_color(theme.text_3)
            .child(
                div()
                    .id("status-workspace")
                    .flex()
                    .items_center()
                    .gap_1p5()
                    .px(px(4.))
                    .py(px(2.))
                    .rounded_md()
                    .cursor_pointer()
                    .hover(|s| {
                        s.bg(theme.overlay).text_color(theme.text_2)
                    })
                    .active(|s| s.bg(theme.active).text_color(theme.active_fg))
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(Self::on_pick_folder_click),
                    )
                    .child(icon("icons/folder.svg", 12., theme.text_3))
                    .child(workspace_label.to_string()),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1p5()
                    .child(icon("icons/monitor.svg", 12., theme.text_3))
                    .child("Local"),
            )
            .children(
                crate::git::current_branch(&cwd).map(|branch| {
                    let open = self.branch_picker.is_some();
                    let pending = self.branch_operation_pending;
                    div()
                        .relative()
                        .child(
                            div()
                                .id("status-branch")
                                .flex()
                                .items_center()
                                .gap_1p5()
                                .px(px(4.))
                                .py(px(2.))
                                .rounded_md()
                                .when(!pending, |chip| chip.cursor_pointer())
                                .when(open, |chip| chip.bg(theme.active).text_color(theme.active_fg))
                                .when(!open && !pending, |chip| {
                                    chip.hover(|s| s.bg(theme.overlay).text_color(theme.text_2))
                                })
                                .when(pending, |chip| chip.opacity(0.6))
                                .on_mouse_up(
                                    MouseButton::Left,
                                    cx.listener(|app, _, window, cx| {
                                        if !app.branch_operation_pending {
                                            app.toggle_branch_picker(window, cx);
                                        }
                                    }),
                                )
                                .child(icon("icons/branch.svg", 12., theme.text_3))
                                .child(branch),
                        )
                        .children(self.branch_picker_popup())
                        .into_any_element()
                }),
            )
            .child(div().flex_1())
            .child(self.context_button(cx))
    }

    fn context_button(&self, cx: &Context<Self>) -> impl IntoElement + use<> {
        let entity = cx.entity();
        let theme = *theme::get(cx);
        context_meter::context_control(
            self.context.as_ref(),
            self.transcript.estimated_tokens(),
            self.context_popup,
            &entity,
            theme,
            |app, hovered, cx| {
                if app.context_popup == ContextPopup::Details {
                    return;
                }
                app.context_popup = if hovered {
                    ContextPopup::Hover
                } else {
                    ContextPopup::None
                };
                cx.notify();
            },
            |app, _, cx| {
                const GESTURE: Duration = Duration::from_millis(200);
                if let Some(dismissed) = app.menu_dismissed_at.take() {
                    if dismissed.elapsed() < GESTURE {
                        return;
                    }
                }
                app.context_popup = if app.context_popup == ContextPopup::Details {
                    ContextPopup::None
                } else {
                    ContextPopup::Details
                };
                if app.context_popup == ContextPopup::Details {
                    app.refresh_context_stats();
                }
                cx.notify();
            },
            |app, _, cx| {
                app.menu_dismissed_at = Some(Instant::now());
                app.context_popup = ContextPopup::None;
                cx.notify();
            },
        )
    }

    /// Sidebar nav row — Waku `render_sidebar_action_row` shape: fixed height,
    /// icon in a 20px slot, secondary label, rounded hover surface.
    fn sidebar_action_row(
        &self,
        id: &'static str,
        icon_path: &'static str,
        label: &str,
        on_click: Option<Box<dyn Fn(&MouseUpEvent, &mut Window, &mut App) + 'static>>,
        theme: Theme,
    ) -> impl IntoElement + use<> {
        let mut row = div()
            .id(ElementId::Name(id.into()))
            .w_full()
            .h(px(SIDEBAR_ACTION_ROW_H))
            .px(px(4.))
            .rounded(px(7.))
            .flex()
            .items_center()
            .gap(px(10.))
            .hover(|s| s.bg(theme.bg_hover))
            .active(|s| s.bg(theme.active).text_color(theme.active_fg))
            .child(
                div()
                    .size(px(20.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(icon(icon_path, 14., theme.text_2)),
            )
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .text_size(theme.ui_px(13.))
                    .text_color(theme.text_2)
                    .child(label.to_string()),
            );

        if let Some(handler) = on_click {
            row = row.cursor_pointer().on_mouse_up(MouseButton::Left, handler);
        }
        row
    }

    fn send_button(&self, cx: &Context<Self>) -> impl IntoElement + use<> {
        let theme = *theme::get(cx);
        if self.busy {
            div()
                .id("stop-btn")
                .size(px(28.))
                .rounded_full()
                .bg(theme.stop_red)
                .hover(|s| s.bg(theme.stop_red_hover))
                .cursor_pointer()
                .flex()
                .items_center()
                .justify_center()
                .text_color(theme.send_fg)
                .on_mouse_up(MouseButton::Left, cx.listener(Self::on_abort_mouse))
                .child(icon("icons/stop.svg", 12., theme.send_fg))
        } else {
            div()
                .id("send-btn")
                .size(px(28.))
                .rounded_full()
                .bg(theme.send_bg)
                .hover(|s| s.bg(theme.send_bg_hover))
                .cursor_pointer()
                .flex()
                .items_center()
                .justify_center()
                .text_color(theme.send_fg)
                .on_mouse_up(MouseButton::Left, cx.listener(Self::on_send_click))
                .child(icon("icons/send.svg", 14., theme.send_fg))
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
        let theme = *theme::get(cx);
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
            .bg(theme.bg_main)
            .font_family(theme::ui_font_family())
            // ── nav column ──
            .child(
                div()
                    .w(px(240.))
                    .h_full()
                    .flex_shrink_0()
                    .bg(theme.bg_sidebar)
                    .border_r_1()
                    .border_color(theme.border)
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
                                .hover(|s| s.bg(theme.bg_hover))
                                .on_mouse_up(MouseButton::Left, cx.listener(Self::on_settings_back))
                                .child(icon("icons/arrow-left.svg", 14., theme.text_2))
                                .child(
                                    div()
                                        .text_size(theme.ui_px(13.))
                                        .text_color(theme.text_2)
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
                                    .text_size(theme.ui_px(13.))
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .cursor_pointer()
                                    .when(selected, |row| row.bg(theme.bg_raised))
                                    .when(!selected, |row| row.hover(|s| s.bg(theme.bg_hover)))
                                    .on_mouse_up(MouseButton::Left, move |_, _, cx| {
                                        this.update(cx, |app, cx| {
                                            app.settings_section = section;
                                            cx.notify();
                                        });
                                    })
                                    .child(icon(
                                        section_icon,
                                        15.,
                                        if selected { theme.text } else { theme.text_3 },
                                    ))
                                    .child(
                                        div()
                                            .text_color(if selected {
                                                theme.text
                                            } else {
                                                theme.text_2
                                            })
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
                            .w_full()
                            .max_w(px(680.))
                            .mx_auto()
                            .px(px(12.))
                            .pt(px(44.))
                            .pb(px(12.))
                            .flex()
                            .flex_col()
                            .gap_3()
                            .child(self.settings_header(theme))
                            .children(self.settings_rows(&this, theme, cx)),
                    ),
            )
    }

    fn settings_header(&self, theme: Theme) -> impl IntoElement + use<> {
        let (title, subtitle) = match self.settings_section {
            SettingsSection::General => (
                "General",
                "How Orbit connects to the pi agent and stores your data.",
            ),
            SettingsSection::Appearance => ("Appearance", "Window, layout, and color preferences."),
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
            .when(self.settings_section == SettingsSection::About, |header| {
                header.child(img(crate::app_icon::ASSET).size(px(72.)).flex_none())
            })
            .child(
                div()
                    .text_size(theme.ui_px(20.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.text)
                    .child(title.to_string()),
            )
            .child(
                div()
                    .text_size(theme.ui_px(12.5))
                    .text_color(theme.text_2)
                    .child(subtitle.to_string()),
            )
    }

    /// The rows for the active section, as card elements. `this` rides
    /// along for closures in interactive controls (rows themselves are
    /// built read-only from app state).
    fn settings_rows(
        &self,
        this: &Entity<OrbitApp>,
        theme: Theme,
        cx: &Context<Self>,
    ) -> Vec<AnyElement> {
        match self.settings_section {
            SettingsSection::General => vec![
                self.card(
                    theme,
                    "pi agent",
                    "Spawned as a child process — newline-delimited JSON over stdio.",
                    Some(self.connection_status(theme)),
                ),
                self.card_with_path(
                    theme,
                    "Local by default",
                    "Sessions live in pi's own store on this computer — no daemon, no cloud.",
                    Some(&sessions::sessions_dir().to_string_lossy()),
                    None,
                ),
                self.card_with_path(
                    theme,
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
                    theme,
                    "Theme",
                    "Pick a Zed-compatible palette for the workbench.",
                    Some(self.theme_select(theme, this.clone(), cx)),
                ),
                self.card(
                    theme,
                    "Language",
                    "Choose the language used throughout Orbit.",
                    Some(self.language_select(theme, this.clone(), cx)),
                ),
                self.card(
                    theme,
                    "UI font size",
                    "Text size across the interface and messages.",
                    Some(self.font_size_select(
                        SettingsSelect::UiFont,
                        theme,
                        this.clone(),
                        cx,
                    )),
                ),
                self.card(
                    theme,
                    "Code font size",
                    "Text size in the file editor, diffs, code blocks, and tool output.",
                    Some(self.font_size_select(
                        SettingsSelect::CodeFont,
                        theme,
                        this.clone(),
                        cx,
                    )),
                ),
                self.card(
                    theme,
                    "UI font",
                    "Typeface for the interface and messages.",
                    Some(self.font_family_select(SettingsSelect::UiFontFamily, theme, this.clone(), cx)),
                ),
                self.card(
                    theme,
                    "Code font",
                    "Typeface for code blocks, diffs, and tool output.",
                    Some(self.font_family_select(SettingsSelect::CodeFontFamily, theme, this.clone(), cx)),
                ),
                self.card(
                    theme,
                    "Show sidebar",
                    "Show the sessions sidebar. Also toggleable from the top bar.",
                    Some(self.sidebar_toggle(theme, this.clone())),
                ),
                self.card(
                    theme,
                    "GPU-rendered streaming",
                    "Stream commits are coalesced (~8 Hz) and highlighting is paint-only, so long tasks never reflow the transcript.",
                    None,
                ),
            ],
            SettingsSection::Providers => self.provider_rows(theme),
            SettingsSection::About => vec![
                self.card(
                    theme,
                    "Orbit Pi",
                    "Native workbench for the pi coding agent.",
                    Some(
                        div()
                            .text_size(theme.ui_px(12.))
                            .text_color(theme.text_2)
                            .child(format!("v{}", env!("CARGO_PKG_VERSION")))
                            .into_any_element(),
                    ),
                ),
                self.card(
                    theme,
                    "GPUI",
                    "GPU-accelerated UI framework (pinned; runtime shaders).",
                    Some(
                        div()
                            .text_size(theme.ui_px(12.))
                            .text_color(theme.text_2)
                            .child("0.2.2")
                            .into_any_element(),
                    ),
                ),
                self.card(
                    theme,
                    "pi CLI",
                    "The only agent runtime — pi speaks its own RPC protocol over stdio.",
                    Some(self.connection_status(theme)),
                ),
            ],
        }
    }

    /// Providers from the live pi catalog, deduplicated in catalog order.
    fn provider_rows(&self, theme: Theme) -> Vec<AnyElement> {
        if self.available_models.is_empty() {
            return vec![self.card(
                theme,
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
                    .bg(theme.bg_composer)
                    .border_1()
                    .border_color(theme.border)
                    .rounded_lg()
                    .px(px(14.))
                    .py(px(10.))
                    .flex()
                    .items_center()
                    .gap_2p5()
                    .child(icon_dyn(provider_icon(&provider), 14., theme.text_2))
                    .child(
                        div()
                            .text_size(theme.ui_px(13.))
                            .text_color(theme.text)
                            .child(provider.clone()),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_size(theme.ui_px(12.))
                            .text_color(theme.text_2)
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
    fn card(
        &self,
        theme: Theme,
        title: &str,
        desc: &str,
        control: Option<AnyElement>,
    ) -> AnyElement {
        self.card_with_path(theme, title, desc, None, control)
    }

    /// Same as [`card`] with an optional dimmed third line (paths).
    fn card_with_path(
        &self,
        theme: Theme,
        title: &str,
        desc: &str,
        path: Option<&str>,
        control: Option<AnyElement>,
    ) -> AnyElement {
        div()
            .bg(theme.bg_composer)
            .border_1()
            .border_color(theme.border)
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
                            .text_size(theme.ui_px(13.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.text)
                            .child(title.to_string()),
                    )
                    .child(
                        div()
                            .text_size(theme.ui_px(12.))
                            .text_color(theme.text_2)
                            .child(desc.to_string()),
                    )
                    .children(path.map(|p| {
                        div()
                            .text_size(theme.ui_px(11.5))
                            .text_color(theme.text_3)
                            .truncate()
                            .child(p.to_string())
                    })),
            )
            .children(control)
            .into_any_element()
    }

    /// Connection state: green dot + "Connected" / red dot + "Not running".
    fn connection_status(&self, theme: Theme) -> AnyElement {
        div()
            .flex()
            .items_center()
            .gap_1p5()
            .child(
                div()
                    .size(px(7.))
                    .rounded_full()
                    .bg(if self.client.is_some() {
                        theme.ok_green
                    } else {
                        theme.stop_red
                    }),
            )
            .child(
                div()
                    .text_size(theme.ui_px(12.))
                    .text_color(theme.text_2)
                    .child(if self.client.is_some() {
                        "Connected"
                    } else {
                        "Not running"
                    }),
            )
            .into_any_element()
    }

    /// The real sidebar toggle, wired to the same state as the top bar.
    fn sidebar_toggle(&self, theme: Theme, this: Entity<OrbitApp>) -> AnyElement {
        let on = self.sidebar_visible;
        div()
            .id("settings-sidebar-toggle")
            .w(px(36.))
            .h(px(20.))
            .rounded_full()
            .p(px(2.))
            .border_1()
            .border_color(theme.border)
            .flex()
            .items_center()
            .cursor_pointer()
            .when(on, |t| t.bg(theme.accent).justify_end())
            .when(!on, |t| t.bg(theme.bg_raised).justify_start())
            .on_mouse_up(MouseButton::Left, move |_, _, cx| {
                this.update(cx, |app, cx| {
                    app.sidebar_visible = !app.sidebar_visible;
                    cx.notify();
                });
            })
            .child(div().size(px(14.)).rounded_full().bg(theme.text))
            .into_any_element()
    }

    /// Theme dropdown on Appearance — lists every selectable palette.
    fn theme_select(
        &self,
        theme: Theme,
        this: Entity<OrbitApp>,
        cx: &Context<Self>,
    ) -> AnyElement {
        let all = ThemeId::ALL;
        let selected = all
            .iter()
            .position(|id| *id == theme.theme_id)
            .unwrap_or(0);
        self.select_control(
            "theme-select",
            SettingsSelect::Theme,
            theme.theme_id.label().to_string(),
            all.iter().map(|id| id.label().to_string()).collect(),
            selected,
            theme,
            this,
            cx,
        )
    }

    // ── Waku General-settings selects (language / font sizes) ──────────

    /// The Language dropdown (System / English).
    fn language_select(&self, theme: Theme, this: Entity<OrbitApp>, cx: &Context<Self>) -> AnyElement {
        let selected = match theme.ui.language {
            crate::theme::Language::System => 0,
            crate::theme::Language::English => 1,
        };
        self.select_control(
            "language-select",
            SettingsSelect::Language,
            theme.ui.language.label().to_string(),
            vec!["System".to_string(), "English".to_string()],
            selected,
            theme,
            this,
            cx,
        )
    }

    /// The UI / code font-size dropdowns (`14 px`, …).
    fn font_size_select(
        &self,
        kind: SettingsSelect,
        theme: Theme,
        this: Entity<OrbitApp>,
        cx: &Context<Self>,
    ) -> AnyElement {
        use crate::theme::{CODE_FONT_SIZES, UI_FONT_SIZES};
        let (sizes, current) = match kind {
            SettingsSelect::UiFont => (UI_FONT_SIZES.to_vec(), theme.ui.ui_font_size),
            SettingsSelect::CodeFont => (CODE_FONT_SIZES.to_vec(), theme.ui.code_font_size),
            SettingsSelect::Language
            | SettingsSelect::Theme
            | SettingsSelect::UiFontFamily
            | SettingsSelect::CodeFontFamily => unreachable!(),
        };
        let selected = sizes.iter().position(|s| *s == current).unwrap_or(0);
        self.select_control(
            match kind {
                SettingsSelect::UiFont => "ui-font-select",
                SettingsSelect::CodeFont => "code-font-select",
                SettingsSelect::Language
                | SettingsSelect::Theme
                | SettingsSelect::UiFontFamily
                | SettingsSelect::CodeFontFamily => "language-select",
            },
            kind,
            format!("{} px", current as u32),
            sizes.iter().map(|s| format!("{} px", *s as u32)).collect(),
            selected,
            theme,
            this,
            cx,
        )
    }

    /// The UI / code font-family dropdowns (installed + bundled faces).
    fn font_family_select(
        &self,
        kind: SettingsSelect,
        theme: Theme,
        this: Entity<OrbitApp>,
        cx: &Context<Self>,
    ) -> AnyElement {
        let fonts = theme::available_fonts(cx);
        let prefs = theme::font_prefs();
        let (id, current) = match kind {
            SettingsSelect::UiFontFamily => ("ui-font-family-select", prefs.ui_font_family.clone()),
            SettingsSelect::CodeFontFamily => {
                ("code-font-family-select", prefs.code_font_family.clone())
            }
            _ => unreachable!(),
        };
        let selected = fonts
            .iter()
            .position(|f| f.as_str() == current.as_ref())
            .unwrap_or(0);
        self.select_control(id, kind, current.to_string(), fonts, selected, theme, this, cx)
    }

    /// A Waku-style select: value chip + caret, dropdown above when open.
    fn select_control(
        &self,
        id: &'static str,
        kind: SettingsSelect,
        label: String,
        options: Vec<String>,
        selected: usize,
        theme: Theme,
        this: Entity<OrbitApp>,
        cx: &Context<Self>,
    ) -> AnyElement {
        let open = self.settings_select == Some(kind);
        let chip_this = this.clone();
        div()
            .flex()
            .flex_col()
            .items_end()
            // dropdown anchored above the chip when open
            .children(self.settings_select_popup(kind, options, selected, theme, &this, cx))
            .child(
                div()
                    .id(ElementId::Name(id.into()))
                    .h(px(26.))
                    .px(px(10.))
                    .rounded(px(7.))
                    .border_1()
                    .border_color(if open {
                        theme.border_strong
                    } else {
                        theme.border
                    })
                    .bg(if open { theme.overlay } else { theme.bg_raised })
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .cursor_pointer()
                    .text_size(theme.ui_px(12.))
                    .text_color(theme.text_2)
                    .hover(|s| s.bg(theme.overlay))
                    .on_mouse_up(MouseButton::Left, move |_, window, cx| {
                        chip_this.update(cx, |app, cx| {
                            if app.settings_select == Some(kind) {
                                app.settings_select = None;
                            } else {
                                app.settings_select = Some(kind);
                                app.settings_filter.update(cx, |filter, cx| filter.clear(cx));
                                let handle = app.settings_filter.read(cx).focus_handle(cx);
                                window.focus(&handle);
                            }
                            cx.notify();
                        });
                    })
                    .child(label)
                    .child(icon("icons/chevron-down.svg", 10., theme.text_3)),
            )
            .into_any_element()
    }

    /// The open dropdown's option list, anchored above its chip.
    fn settings_select_popup(
        &self,
        kind: SettingsSelect,
        options: Vec<String>,
        selected: usize,
        theme: Theme,
        this: &Entity<OrbitApp>,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        if self.settings_select != Some(kind) {
            return None;
        }
        let needle = self.settings_filter.read(cx).text().to_lowercase();
        // Filter options, keeping the original index for click dispatch.
        let rows: Vec<(usize, String)> = options
            .iter()
            .enumerate()
            .filter(|(_, option)| needle.is_empty() || option.to_lowercase().contains(&needle))
            .map(|(ix, option)| (ix, option.clone()))
            .collect();
        let empty = rows.is_empty();
        // Virtualized list — only the visible rows are laid out and painted.
        // The font-family dropdowns can have a thousand+ entries, and the
        // whole app re-renders on every scroll tick, so rendering every row
        // per frame is what made the theme/font selectors lag while scrolling.
        //
        // `uniform_list`'s Infer sizing reads the *available* height, and this
        // popup lives inside a 0×0 anchor div, so the available height is 0 and
        // the list collapses to nothing. Give it an explicit height computed
        // from the row count instead (30 px stride + 8 px vertical padding,
        // capped at the old 220 px max) — same visual, real viewport.
        let list_h = (rows.len() as f32 * 30. + 8.).min(220.);
        let list: AnyElement = if empty {
            div()
                .w_full()
                .px(px(4.))
                .py(px(4.))
                .child(
                    div()
                        .h(px(28.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(theme.ui_px(12.))
                        .text_color(theme.text_3)
                        .child("No matches"),
                )
                .into_any_element()
        } else {
            let this = this.clone();
            uniform_list(
                "settings-select-list",
                rows.len(),
                move |range, _window, _cx| {
                    let mut children = Vec::new();
                    for ix in range {
                        let (orig_ix, option) = &rows[ix];
                        let orig_ix = *orig_ix;
                        let selected_row = orig_ix == selected;
                        let this = this.clone();
                        // 30 px stride = 28 px row + the 2 px gap the old
                        // flex list had between rows (uniform_list has no
                        // gap support, so the gap rides on the row shell).
                        children.push(
                            div()
                                .h(px(30.))
                                .w_full()
                                .child(
                                    div()
                                        .id(ElementId::NamedInteger(
                                            "settings-select-row".into(),
                                            orig_ix as u64,
                                        ))
                                        .h(px(28.))
                                        .w_full()
                                        .px(px(10.))
                                        .rounded(px(6.))
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .gap(px(10.))
                                        .cursor_pointer()
                                        .when(selected_row, |row| row.bg(theme.active))
                                        .hover(|style| style.bg(theme.overlay))
                                        .text_size(theme.ui_px(12.))
                                        .text_color(if selected_row {
                                            theme.active_fg
                                        } else {
                                            theme.text_2
                                        })
                                        .child(option.clone())
                                        .when(selected_row, |row| {
                                            row.child(icon("icons/check.svg", 11., theme.accent))
                                        })
                                        .on_mouse_up(MouseButton::Left, move |_, _, cx| {
                                            this.update(cx, |app, cx| {
                                                app.apply_settings_select(kind, orig_ix, cx);
                                                app.settings_select = None;
                                                cx.notify();
                                            });
                                        }),
                                ),
                        );
                    }
                    children
                },
            )
            .w_full()
            .h(px(list_h))
            .px(px(4.))
            .py(px(4.))
            .into_any_element()
        };
        let popup = div()
            .min_w(px(360.))
            .rounded(px(8.))
            .border_1()
            .border_color(theme.border_strong)
            .bg(theme.menu_bg)
            .shadow(theme.popover_shadow())
            .flex()
            .flex_col()
            .overflow_hidden()
            .occlude()
            .on_mouse_down_out({
                let this = this.clone();
                move |_: &MouseDownEvent, _, cx: &mut App| {
                    this.update(cx, |app, cx| {
                        if app.settings_select.take().is_some() {
                            cx.notify();
                        }
                    });
                }
            })
            // Search field — filters the options below.
            .child(
                div()
                    .h(px(34.))
                    .px(px(10.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .border_b_1()
                    .border_color(theme.border)
                    .text_size(theme.ui_px(12.))
                    .child(icon("icons/search.svg", 13., theme.text_3))
                    .child(self.settings_filter.clone()),
            )
            .child(list);
        // Anchor to the chip's top-right corner (via a zero-size point), so the
        // popup opens above the chip, right-aligned — like the open-in menu.
        Some(
            div()
                .absolute()
                .top_0()
                .right_0()
                .size(px(0.))
                .child(
                    anchored()
                        .position_mode(AnchoredPositionMode::Local)
                        .anchor(Corner::BottomRight)
                        .offset(point(px(0.), px(-4.)))
                        .snap_to_window()
                        .child(deferred(popup)),
                )
                .into_any_element(),
        )
    }

    /// Apply a dropdown choice to the persisted UI customization.
    fn apply_settings_select(&mut self, kind: SettingsSelect, ix: usize, cx: &mut Context<Self>) {
        match kind {
            SettingsSelect::Theme => {
                let id = ThemeId::ALL.get(ix).copied().unwrap_or(ThemeId::OneDark);
                theme::set_theme(cx, id);
                return;
            }
            SettingsSelect::UiFontFamily | SettingsSelect::CodeFontFamily => {
                let fonts = theme::available_fonts(cx);
                let Some(family) = fonts.get(ix) else {
                    return;
                };
                let family = family.clone();
                let mut prefs = theme::font_prefs();
                match kind {
                    SettingsSelect::UiFontFamily => prefs.ui_font_family = family.into(),
                    SettingsSelect::CodeFontFamily => prefs.code_font_family = family.into(),
                    _ => unreachable!(),
                }
                theme::set_font_prefs(prefs);
                return;
            }
            _ => {}
        }
        use crate::theme::{Language, CODE_FONT_SIZES, UI_FONT_SIZES};
        let mut ui = theme::get(cx).ui;
        match kind {
            SettingsSelect::Language => {
                ui.language = if ix == 1 {
                    Language::English
                } else {
                    Language::System
                };
            }
            SettingsSelect::UiFont => {
                ui.ui_font_size = UI_FONT_SIZES.get(ix).copied().unwrap_or(14.);
            }
            SettingsSelect::CodeFont => {
                ui.code_font_size = CODE_FONT_SIZES.get(ix).copied().unwrap_or(13.);
            }
            SettingsSelect::Theme
            | SettingsSelect::UiFontFamily
            | SettingsSelect::CodeFontFamily => unreachable!(),
        }
        theme::set_ui_prefs(cx, ui);
    }
}

/// Render an embedded HugeIcons SVG tinted with the given color.
pub(crate) fn icon(path: &'static str, size: f32, color: Hsla) -> impl IntoElement + use<> {
    gpui::svg().path(path).size(px(size)).text_color(color)
}

/// Same as [`icon`] but for runtime-computed paths (per-provider marks).
pub(crate) fn icon_dyn(path: SharedString, size: f32, color: Hsla) -> impl IntoElement + use<> {
    gpui::svg().path(path).size(px(size)).text_color(color)
}

// ── devicons (Nerd Font file glyphs) ───────────────────────────────

/// The Nerd Font family that covers devicons glyphs, detected once per
/// process (font availability doesn't change mid-run).
static NERD_FONT: std::sync::OnceLock<Option<SharedString>> = std::sync::OnceLock::new();

/// Preferred Nerd Font families, best first — used only to *rank* the
/// installed families; any family whose name contains "Nerd Font" (or the
/// short `NF` style, e.g. `MesloLGS NF`) is a candidate, verified by an
/// actual PUA-glyph probe.
const NERD_FONT_PREFERRED: [&str; 6] = [
    "SymbolsNerdFont",
    "Symbols Nerd Font",
    "JetBrainsMono Nerd Font",
    "Hack Nerd Font",
    "FiraCode Nerd Font",
    "CaskaydiaCove Nerd Font",
];

/// The Nerd Font family to paint devicons glyphs with, or `None` when no
/// Nerd Font is installed (callers fall back to extension text badges).
pub(crate) fn nerd_font_family(cx: &App) -> Option<SharedString> {
    NERD_FONT
        .get_or_init(|| {
            let installed = cx.text_system().all_font_names();
            // Any family named like a Nerd Font, best-known first.
            let mut candidates: Vec<String> = installed
                .iter()
                .filter(|name| {
                    let lower = name.to_lowercase();
                    lower.contains("nerd font")
                        || lower.contains("nerdfont")
                        || lower.ends_with(" nf")
                })
                .cloned()
                .collect();
            // Dedicated symbols fonts paint the widest glyph coverage first;
            // popular patched coding fonts next; the rest in name order.
            candidates.sort_by_key(|name| {
                let lower = name.to_lowercase();
                let pref = NERD_FONT_PREFERRED
                    .iter()
                    .position(|p| lower.eq_ignore_ascii_case(&p.to_lowercase()))
                    .unwrap_or(NERD_FONT_PREFERRED.len());
                (pref, !lower.starts_with("symbols"), name.clone())
            });
            candidates.into_iter().find_map(|family| {
                let font_id = cx.text_system().resolve_font(&gpui::font(family.clone()));
                // `\u{ea60}` sits in Nerd Font's Octicon range — present in any
                // complete Nerd Font, absent from plain coding fonts.
                cx.text_system()
                    .typographic_bounds(font_id, px(12.), '\u{ea60}')
                    .ok()?;
                Some(SharedString::from(family))
            })
        })
        .clone()
}

/// devicons glyph + color for a file path (`README.md` → its Nerd Font
/// markdown glyph). `dark` selects devicons' palette; `None` when the
/// color string is not parseable hex.
pub(crate) fn dev_file_icon(path: &str, dark: bool) -> Option<(char, Hsla)> {
    let theme = if dark {
        devicons::Theme::Dark
    } else {
        devicons::Theme::Light
    };
    let icon = devicons::icon_for_file(path, &Some(theme));
    let value = u32::from_str_radix(icon.color.trim_start_matches('#'), 16).ok()?;
    Some((icon.icon, gpui::rgb(value).into()))
}

/// Paint a devicons glyph (or fall back to `fallback` when no Nerd Font
/// is installed) — shared by the @-mention rows and attachment chips.
pub(crate) fn file_glyph(
    path: &str,
    dark: bool,
    nerd_family: Option<&SharedString>,
    font_size: f32,
    fallback: AnyElement,
) -> AnyElement {
    let Some(family) = nerd_family else {
        return fallback;
    };
    let Some((glyph, color)) = dev_file_icon(path, dark) else {
        return fallback;
    };
    div()
        .w(px(18.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .font_family(family)
        .text_size(px(font_size))
        .text_color(color)
        .child(glyph.to_string())
        .into_any_element()
}

/// Extension text badge (`MD`, `TSX`, `HTML`, …) — the no-Nerd-Font
/// fallback for [`file_glyph`], tinted with the language's brand color.
pub(crate) fn file_badge(path: &str, theme: Theme) -> AnyElement {
    let (label, dark_hex, light_hex) = mentions::file_type_badge(path);
    let color: Hsla = gpui::rgb(if theme.mode == ThemeMode::Dark {
        dark_hex
    } else {
        light_hex
    })
    .into();
    div()
        .size(px(17.))
        .flex_none()
        .rounded(px(4.))
        .bg(color.opacity(0.16))
        .flex()
        .items_center()
        .justify_center()
        .text_size(theme.ui_px(7.5))
        .line_height(theme.ui_px(8.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(color)
        .child(label.to_string())
        .into_any_element()
}

/// A non-interactive pill used for static meta in the composer row.
fn pill_static(icon_path: &'static str, label: &str, theme: Theme) -> impl IntoElement + use<> {
    div()
        .flex()
        .items_center()
        .gap_1p5()
        .px(px(7.))
        .py(px(3.))
        .rounded_md()
        .bg(theme.bg_raised)
        .border_1()
        .border_color(theme.border)
        .text_size(theme.ui_px(12.))
        .text_color(theme.text_2)
        .child(icon(icon_path, 12., theme.text_2))
        .child(label.to_string())
}

/// Whether a workspace group is collapsed in the sidebar. The active
/// workspace is expanded by default; all others are collapsed unless the
/// user has toggled them.
fn is_workspace_group_collapsed(
    label: &str,
    working_label: &str,
    collapsed_workspaces: &HashSet<String>,
    expanded_workspace_groups: &HashSet<String>,
) -> bool {
    if label == working_label {
        collapsed_workspaces.contains(label)
    } else {
        !expanded_workspace_groups.contains(label)
    }
}

/// Toggle a workspace group's open/closed state against the defaults above.
fn toggle_workspace_group(
    label: String,
    working_label: &str,
    collapsed_workspaces: &mut HashSet<String>,
    expanded_workspace_groups: &mut HashSet<String>,
) {
    if label == working_label {
        if !collapsed_workspaces.remove(&label) {
            collapsed_workspaces.insert(label);
        }
    } else if !expanded_workspace_groups.remove(&label) {
        expanded_workspace_groups.insert(label);
    }
}

/// Sessions visible under one workspace group when it is not expanded.
fn visible_sessions_in_group(
    ixs: &[usize],
    sessions: &[SessionInfo],
    expanded: bool,
    active_path: &Option<PathBuf>,
) -> Vec<usize> {
    if expanded || ixs.len() <= SIDEBAR_GROUP_SESSIONS_VISIBLE {
        return ixs.to_vec();
    }

    let limit = SIDEBAR_GROUP_SESSIONS_VISIBLE;
    let mut indices: Vec<usize> = ixs.iter().take(limit).copied().collect();
    if let Some(active) = active_path {
        if let Some(active_ix) = sessions.iter().position(|s| &s.path == active) {
            if ixs.contains(&active_ix) && !indices.contains(&active_ix) {
                indices.pop();
                indices.push(active_ix);
                indices.sort_by_key(|ix| ixs.iter().position(|&i| i == *ix).unwrap_or(usize::MAX));
            }
        }
    }
    indices
}

/// Build the grouped sidebar session list. Each workspace shows at most
/// [`SIDEBAR_GROUP_SESSIONS_VISIBLE`] sessions until expanded.
fn build_sidebar_rows(
    sessions: &[SessionInfo],
    working_label: &str,
    collapsed_workspaces: &HashSet<String>,
    expanded_workspace_groups: &HashSet<String>,
    expanded_session_groups: &HashSet<String>,
    active_path: &Option<PathBuf>,
) -> Vec<SideRow> {
    let mut side_rows: Vec<SideRow> = Vec::new();
    let mut groups: Vec<(String, Vec<usize>)> = Vec::new();
    for (ix, session) in sessions.iter().enumerate() {
        let label = sessions::workspace_label(&session.cwd);
        match groups.iter_mut().find(|(l, _)| *l == label) {
            Some((_, ixs)) => ixs.push(ix),
            None => groups.push((label, vec![ix])),
        }
    }
    for (label, ixs) in groups {
        let collapsed = is_workspace_group_collapsed(
            &label,
            working_label,
            collapsed_workspaces,
            expanded_workspace_groups,
        );
        side_rows.push(SideRow::Workspace {
            label: label.clone(),
            count: ixs.len(),
            collapsed,
            cwd: sessions[ixs[0]].cwd.clone(),
        });
        if collapsed {
            continue;
        }

        let sessions_expanded = expanded_session_groups.contains(&label);
        let visible =
            visible_sessions_in_group(&ixs, sessions, sessions_expanded, active_path);
        for ix in &visible {
            side_rows.push(SideRow::Session(*ix));
        }

        if sessions_expanded {
            if ixs.len() > SIDEBAR_GROUP_SESSIONS_VISIBLE {
                side_rows.push(SideRow::ShowLess { label });
            }
        } else if ixs.len() > SIDEBAR_GROUP_SESSIONS_VISIBLE {
            let hidden = ixs.len().saturating_sub(visible.len());
            if hidden > 0 {
                side_rows.push(SideRow::ShowMore { label, count: hidden });
            }
        }
    }
    side_rows
}

#[allow(clippy::too_many_arguments)]
fn render_side_row(
    rows: &Rc<Vec<SideRow>>,
    sessions_data: &Rc<Vec<SessionInfo>>,
    active_path: Option<&Path>,
    ix: usize,
    this: &Entity<OrbitApp>,
    agent_running: bool,
    running_paths: &Rc<HashSet<PathBuf>>,
    session_menu: Option<&SessionMenu>,
    theme: Theme,
) -> impl IntoElement {
    match &rows[ix] {
        SideRow::Workspace {
            label,
            count,
            collapsed,
            cwd,
        } => {
            let label = label.clone();
            let label_for_click = label.clone();
            let cwd_for_new = cwd.clone();
            let this_toggle = this.clone();
            let this_new = this.clone();
            // Outer shell: inter-group spacing only — hover lives on the inner
            // card so the highlight doesn't bleed into the padding (same split
            // as session rows below).
            div()
                .w_full()
                .px_2()
                .pt(px(10.))
                .pb(px(2.))
                .group("workspace-row")
                .child(
                    div()
                        .w_full()
                        .h(px(28.))
                        .px(px(4.))
                        .rounded_md()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .cursor_pointer()
                        .hover(|s| s.bg(theme.bg_hover))
                        .on_mouse_up(MouseButton::Left, move |_, _, cx| {
                            let label = label_for_click.clone();
                            this_toggle.update(cx, |app, cx| {
                                let working = app.workspace_label();
                                toggle_workspace_group(
                                    label,
                                    &working,
                                    &mut app.collapsed_workspaces,
                                    &mut app.expanded_workspace_groups,
                                );
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
                            theme.text_3,
                        ))
                        .child(icon("icons/folder.svg", 13., theme.text_2))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .text_size(theme.ui_px(12.5))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(theme.text)
                                .child(label.clone()),
                        )
                        .child(
                            div()
                                .px(px(5.))
                                .h(px(16.))
                                .min_w(px(16.))
                                .flex_none()
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded_full()
                                .bg(theme.bg_raised)
                                .text_size(theme.ui_px(10.))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(theme.text_3)
                                .child(format!("{count}")),
                        )
                        .child(
                            div()
                                .id(ElementId::Name(
                                    format!("workspace-new-{label}").into(),
                                ))
                                .flex_none()
                                .size(px(18.))
                                .rounded(px(4.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .hover(|s| s.bg(theme.overlay))
                                .on_mouse_up(
                                    MouseButton::Left,
                                    {
                                        let this = this_new.clone();
                                        move |_, window, cx| {
                                            cx.stop_propagation();
                                            let cwd = cwd_for_new.clone();
                                            this.update(cx, |app, cx| {
                                                app.on_new_session_in_workspace(cwd, window, cx);
                                            });
                                        }
                                    },
                                )
                                .child(icon("icons/plus.svg", 14., theme.text_3)),
                        ),
                )
                .into_any_element()
        }
        SideRow::ShowMore { label, count } => {
            let this = this.clone();
            let label_for_click = label.clone();
            div()
                .w_full()
                .pl(px(20.))
                .pr_2()
                .py(px(4.))
                .flex()
                .items_center()
                .gap_1p5()
                .rounded_md()
                .cursor_pointer()
                .hover(|s| s.bg(theme.bg_hover))
                .on_mouse_up(MouseButton::Left, move |_, _, cx| {
                    let label = label_for_click.clone();
                    this.update(cx, |app, cx| {
                        app.expanded_session_groups.insert(label);
                        cx.notify();
                    });
                })
                .child(icon("icons/chevron-down.svg", 11., theme.text_3))
                .child(
                    div()
                        .text_size(theme.ui_px(12.))
                        .text_color(theme.text_3)
                        .child(format!("Show {count} more")),
                )
                .into_any_element()
        }
        SideRow::ShowLess { label } => {
            let this = this.clone();
            let label_for_click = label.clone();
            div()
                .w_full()
                .pl(px(20.))
                .pr_2()
                .py(px(4.))
                .flex()
                .items_center()
                .gap_1p5()
                .rounded_md()
                .cursor_pointer()
                .hover(|s| s.bg(theme.bg_hover))
                .on_mouse_up(MouseButton::Left, move |_, _, cx| {
                    let label = label_for_click.clone();
                    this.update(cx, |app, cx| {
                        app.expanded_session_groups.remove(&label);
                        cx.notify();
                    });
                })
                .child(icon("icons/chevron-up.svg", 11., theme.text_3))
                .child(
                    div()
                        .text_size(theme.ui_px(12.))
                        .text_color(theme.text_3)
                        .child("Show less"),
                )
                .into_any_element()
        }
        SideRow::Session(ix) => {
            let session = sessions_data[*ix].clone();
            let session_for_click = session.clone();
            let active = active_path == Some(session.path.as_path());
            // The open session runs live; parked (background) sessions run
            // in their own pi processes — both get the loader (Waku).
            let running = (active && agent_running) || running_paths.contains(&session.path);
            let this = this.clone();
            let this_for_row = this.clone();
            let this_for_menu = this.clone();
            let menu = session_menu.filter(|m| m.path == session.path);
            // Sessions with a live pi process must not be deleted — the
            // process would recreate the file mid-run.
            let deletable = !active && !running;
            // Indented under the workspace group so the list reads as a
            // tree. The open row carries an accent-tinted leading icon so it
            // reads at a glance; the icon chip gives every row an anchor to
            // scan by.
            // Outer item carries the inter-row spacing (padding) and the click
            // handler; the inner card holds the background/hover so the gap
            // between cards stays clear. Padding (not margin) is used because
            // the list measures each item's border-box — margins are dropped.
            let mut row = div()
                .id(ElementId::NamedInteger("side-session".into(), *ix as u64))
                .w_full()
                .py(px(2.))
                .cursor_pointer()
                .on_mouse_up(MouseButton::Left, move |_, _, cx| {
                    let session = session_for_click.clone();
                    this_for_row.update(cx, |app, cx| {
                        app.on_open_session(session, cx);
                    });
                });
            let mut card = div()
                .group("srow")
                .relative()
                .w_full()
                .pl(px(20.))
                .pr(px(6.))
                .py(px(6.))
                .rounded_md()
                .flex()
                .items_center()
                .gap(px(10.))
                .when(active, |card| card.bg(theme.bg_raised))
                .when(!active, |card| card.hover(|s| s.bg(theme.bg_hover)));
            // Leading icon chip — the session-kind glyph, tinted on the
            // open row so the active conversation stands out.
            card = card.child(render_session_chip(active, theme));
            // Two-line text column: title, then preview · age.
            card = card.child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .justify_center()
                    .gap(px(2.))
                    // Line 1 — title (the hero line: wrapped in a flex row so
                    // the `flex_1 min_w_0 truncate` pattern takes the full
                    // column width and paints the name — instead of collapsing
                    // to "…" as a bare flex-column child).
                    .child(
                        div()
                            .w_full()
                            .flex()
                            .items_center()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_size(theme.ui_px(13.5))
                                    .line_height(px(18.))
                                    .font_weight(FontWeight::NORMAL)
                                    .text_color(if active { theme.text } else { theme.text_2 })
                                    .child(session.title.clone()),
                            ),
                    )
                    // Line 2 — first message preview + age (kept subtle).
                    .child(
                        div()
                            .w_full()
                            .flex()
                            .items_center()
                            .gap_1p5()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_size(theme.ui_px(11.))
                                    .line_height(px(14.))
                                    .text_color(theme.text_3)
                                    .child(session.first_message.clone()),
                            )
                            .child(
                                div()
                                    .flex_none()
                                    .text_size(theme.ui_px(10.5))
                                    .text_color(theme.text_3)
                                    .child(sessions::relative_time(session.modified)),
                            ),
                    ),
            );
            // Right edge — spinner while running, hover-revealed actions
            // otherwise. Vertically centered against the two-line body.
            card = card.child(
                if running {
                    running_loader(theme, *ix).into_any_element()
                } else {
                    session_menu_button(
                        *ix,
                        menu,
                        session.path.clone(),
                        session.title.clone(),
                        deletable,
                        this_for_menu,
                        theme,
                    )
                    .into_any_element()
                },
            );
            row = row.child(card);
            row.into_any_element()
        }
    }
}

/// The hover-revealed '…' button on a quiet session row. Clicking it opens
/// the row's actions popup; propagation stops so the row's open handler
/// doesn't also fire.
fn session_menu_button(
    ix: usize,
    menu: Option<&SessionMenu>,
    path: PathBuf,
    title: String,
    deletable: bool,
    this: Entity<OrbitApp>,
    theme: Theme,
) -> impl IntoElement + use<> {
    let menu_open = menu.is_some();
    let this_for_popup = this.clone();
    div()
        .id(ElementId::NamedInteger("side-more".into(), ix as u64))
        .relative()
        .flex_none()
        .size(px(18.))
        .rounded(px(4.))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        // Hidden until the row (or the button itself) is hovered, or while
        // this row's menu is open.
        .opacity(if menu_open { 1.0 } else { 0.0 })
        .group_hover("srow", |s| s.opacity(1.))
        .hover(|s| s.bg(theme.bg_hover))
        .on_mouse_up(MouseButton::Left, move |_, window, cx| {
            // Keep the click from also opening the session via the row.
            cx.stop_propagation();
            let (path, title, deletable) = (path.clone(), title.clone(), deletable);
            this.update(cx, |app, cx| {
                app.toggle_session_menu(
                    SessionMenu {
                        path,
                        title,
                        deletable,
                        confirm_delete: false,
                    },
                    window,
                    cx,
                );
            });
        })
        .child(icon("icons/more.svg", 14., theme.text_3))
        // The dropdown hangs off a zero-size anchor pinned to the button's
        // top-left corner. The button centers its icon (`items_center` +
        // `justify_center`), and Taffy lays absolutely-positioned children out
        // with the container's alignment — without the pin, the popup's
        // static position is pulled toward the button's center and it opens
        // up-left of the trigger instead of just below it.
        .children(menu.map(|menu| {
            div()
                .absolute()
                .top_0()
                .left_0()
                .size(px(0.))
                .child(session_menu_popup(menu, this_for_popup.clone(), theme))
        }))
}

/// A session row's leading icon chip: a rounded surface holding the
/// conversation glyph, tinted with the accent when the row is the open
/// session (so the active conversation reads at a glance) and neutral
/// otherwise. No interactivity of its own — the row is the click target.
fn render_session_chip(active: bool, theme: Theme) -> impl IntoElement + use<> {
    div()
        .size(px(28.))
        .flex_none()
        .relative()
        .rounded(px(7.))
        .flex()
        .items_center()
        .justify_center()
        .bg(if active {
            theme.accent.opacity(0.16)
        } else {
            theme.bg_raised
        })
        .child(icon(
            "icons/chat.svg",
            15.,
            if active {
                theme.accent
            } else {
                theme.text_3
            },
        ))
}

/// Waku's sidebar working spinner: a rotating loader arc on the running
/// session's row. GPUI's `with_animation` drives the rotation itself
/// (self-repainting — no help needed from the app tick).
fn running_loader(theme: Theme, id: usize) -> impl IntoElement + use<> {
    gpui::svg()
        .path("icons/loader.svg")
        .size(px(12.))
        .text_color(theme.ok_green)
        .with_animation(
            id,
            Animation::new(Duration::from_millis(900)).repeat(),
            |svg, delta| {
                svg.with_transformation(Transformation::rotate(radians(
                    delta * std::f32::consts::TAU,
                )))
            },
        )
}

// ── sidebar row-actions popup ──────────────────────────────────────────

/// The actions popup anchored to a session row: Copy path / Reveal in
/// Finder / Delete, or the delete confirmation once armed. Painted via
/// `deferred` + `anchored` (same convention as the composer pickers),
/// dismissed by any outside mouse-down.
fn session_menu_popup(menu: &SessionMenu, this: Entity<OrbitApp>, theme: Theme) -> AnyElement {
    let deletable = menu.deletable;
    let confirm = menu.confirm_delete;

    let body: AnyElement = if confirm {
        // Delete confirmation — the destructive step gets a named victim.
        div()
            .w_full()
            .flex()
            .flex_col()
            .gap(px(6.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(
                        div()
                            .text_size(theme.ui_px(12.5))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.text)
                            .child("Delete this session?"),
                    )
                    .child(
                        div()
                            .text_size(theme.ui_px(11.))
                            .text_color(theme.text_2)
                            .child("Removes the session file from disk."),
                    ),
            )
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(6.))
                    .child(
                        div()
                            .id("menu-cancel")
                            .h(px(24.))
                            .px(px(10.))
                            .flex()
                            .items_center()
                            .rounded(px(6.))
                            .bg(theme.bg_raised)
                            .cursor_pointer()
                            .hover(|s| s.bg(theme.bg_hover))
                            .text_size(theme.ui_px(11.5))
                            .text_color(theme.text_2)
                            .on_mouse_down(MouseButton::Left, {
                                let this = this.clone();
                                move |_, _, cx| {
                                    cx.stop_propagation();
                                    this.update(cx, |app, cx| app.on_menu_cancel(cx));
                                }
                            })
                            .child("Cancel"),
                    )
                    .child(
                        div()
                            .id("menu-confirm-delete")
                            .h(px(24.))
                            .px(px(10.))
                            .flex()
                            .items_center()
                            .rounded(px(6.))
                            .bg(theme.stop_red)
                            .cursor_pointer()
                            .hover(|s| s.bg(theme.stop_red_hover))
                            .text_size(theme.ui_px(11.5))
                            .text_color(theme.send_fg)
                            .on_mouse_down(MouseButton::Left, {
                                let this = this.clone();
                                move |_, _, cx| {
                                    cx.stop_propagation();
                                    this.update(cx, |app, cx| app.on_menu_delete_confirm(cx));
                                }
                            })
                            .child("Delete"),
                    ),
            )
            .into_any_element()
    } else {
        div()
            .w_full()
            .flex()
            .flex_col()
            .child(menu_item(
                "menu-copy-path",
                "icons/copy.svg",
                "Copy path",
                theme,
                this.clone(),
                false,
                |app, cx| app.on_menu_copy_path(cx),
            ))
            .child(menu_item(
                "menu-reveal",
                "icons/folder.svg",
                "Reveal in Finder",
                theme,
                this.clone(),
                false,
                |app, cx| app.on_menu_reveal(cx),
            ))
            .when(deletable, |menu| {
                menu.child(div().h(px(1.)).w_full().bg(theme.border).my(px(4.)))
                    .child(menu_item(
                        "menu-delete",
                        "icons/trash.svg",
                        "Delete session…",
                        theme,
                        this.clone(),
                        true,
                        |app, cx| app.on_menu_delete_request(cx),
                    ))
            })
            .into_any_element()
    };

    let popup = div()
        .w(px(190.))
        .p(px(4.))
        .when(confirm, |pop| pop.w(px(210.)).p(px(10.)))
        .rounded(px(10.))
        .border_1()
        .border_color(theme.border_strong)
        .bg(theme.menu_bg)
        .shadow(theme.popover_shadow())
        .flex()
        .flex_col()
        .overflow_hidden()
        .occlude()
        // Any mouse-down outside dismisses; clicks inside are stopped by
        // the item handlers, so they never read as "outside".
        .on_mouse_down_out({
            let this = this.clone();
            move |_, _, cx| {
                this.update(cx, |app, cx| {
                    // Arm the gesture guard so this same click's mouse-up
                    // cannot immediately re-open the menu.
                    app.menu_dismissed_at = Some(Instant::now());
                    app.session_menu = None;
                    cx.notify();
                })
            }
        })
        .child(body);

    // Float the popup: `anchored` takes it out of the layout (no other row
    // moves) and pins its top-left corner just below the '…' button (the
    // button is 18px, so +20px drops the top edge 2px under it, left-aligned
    // to the trigger — the standard app dropdown position). The wrapping
    // zero-size anchor in `session_menu_button` guarantees the static origin
    // is the button's top-left regardless of the button's own centering.
    // `deferred` paints it above the rest of the list — same convention as
    // the composer chip pickers. `snap_to_window` keeps it inside the window
    // near the edges.
    anchored()
        .position_mode(AnchoredPositionMode::Local)
        .anchor(Corner::TopLeft)
        .offset(point(px(0.), px(20.)))
        .snap_to_window()
        .child(deferred(popup))
        .into_any_element()
}

/// One menu row: icon + label, full-width hit target, delegating to an
/// `OrbitApp` handler through the owning entity. When `danger` is set the
/// row gets a destructive red wash (red background, white text/icon) so
/// destructive actions read as dangerous at a glance.
fn menu_item<C: Fn(&mut OrbitApp, &mut Context<OrbitApp>) + 'static>(
    id: &'static str,
    icon_path: &'static str,
    label: &'static str,
    theme: Theme,
    this: Entity<OrbitApp>,
    danger: bool,
    on_click: C,
) -> impl IntoElement + use<C> {
    let on_click = on_click;
    let (hover_bg, text_color, icon_color) = if danger {
        (theme.stop_red_hover, theme.send_fg, theme.send_fg)
    } else {
        (theme.bg_hover, theme.text_2, theme.text_3)
    };
    div()
        .id(id)
        .h(px(30.))
        .px(px(8.))
        .rounded(px(6.))
        .flex()
        .items_center()
        .gap(px(8.))
        .cursor_pointer()
        .when(danger, |s| s.bg(theme.stop_red))
        .hover(move |s| s.bg(hover_bg))
        .text_size(theme.ui_px(12.5))
        .text_color(text_color)
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            cx.stop_propagation();
            this.update(cx, |app, cx| (on_click)(app, cx));
        })
        .child(icon(icon_path, 13., icon_color))
        .child(label.to_string())
}

/// Reveal a session file in the OS file manager (macOS first, matching the
/// product's platform stance; other platforms get the containing folder).
fn reveal_in_file_manager(path: PathBuf) {
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open")
        .arg("-R")
        .arg(path)
        .spawn();
    #[cfg(not(target_os = "macos"))]
    if let Some(dir) = path.parent() {
        #[cfg(target_os = "linux")]
        let _ = std::process::Command::new("xdg-open").arg(dir).spawn();
        #[cfg(target_os = "windows")]
        let _ = std::process::Command::new("explorer").arg(dir).spawn();
        let _ = dir;
    }
}

/// Empty-state panel for a fresh pi store: what this space is for and how
/// to fill it. No cards, no illustration — one calm statement.
fn empty_sessions_state(theme: Theme) -> impl IntoElement + use<> {
    div()
        .flex_1()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(6.))
        .px(px(20.))
        .pb(px(40.))
        .child(
            div()
                .size(px(36.))
                .rounded_full()
                .bg(theme.bg_raised)
                .flex()
                .items_center()
                .justify_center()
                .child(icon("icons/spark.svg", 16., theme.accent)),
        )
        .child(
            div()
                .text_size(theme.ui_px(12.5))
                .font_weight(FontWeight::MEDIUM)
                .text_color(theme.text_2)
                .child("No sessions yet"),
        )
        .child(
            div()
                .text_size(theme.ui_px(11.5))
                .text_color(theme.text_3)
                .text_align(TextAlign::Center)
                .child("Start a task and it will show up here."),
        )
        .child(
            div()
                .mt(px(6.))
                .px(px(6.))
                .h(px(18.))
                .flex()
                .items_center()
                .rounded(px(4.))
                .border_1()
                .border_color(theme.border)
                .text_size(theme.ui_px(10.5))
                .text_color(theme.text_3)
                .child("\u{2318}N"),
        )
}

#[cfg(test)]
mod devicons_tests {
    use super::*;

    #[test]
    fn dev_file_icons_carry_glyph_and_parsed_color() {
        // Markdown: the Nerd Font markdown glyph with a hex color.
        let (md_glyph, md_color) = dev_file_icon("README.md", true).unwrap();
        assert!(md_color != gpui::hsla(0., 0., 0., 1.));
        // TypeScript maps to a different glyph than markdown.
        let (ts_glyph, _) = dev_file_icon("src/lib.ts", true).unwrap();
        assert_ne!(md_glyph, ts_glyph);
        // Unknown extensions still resolve to a fallback glyph.
        assert!(dev_file_icon("data.unknownext123", true).is_some());
        // Dark and light palettes can differ per glyph color.
        let (_, dark) = dev_file_icon("Cargo.toml", true).unwrap();
        let (_, light) = dev_file_icon("Cargo.toml", false).unwrap();
        let _ = (dark, light);
    }
}

#[cfg(test)]
mod popup_layout_tests {
    use super::*;

    /// The settings popup's list: a `uniform_list` with an explicit height
    /// (30 px row stride + 8 px vertical padding, capped at 220 px), inside
    /// the popup's flex column under a 34 px search field.
    struct SettingsPopupListTestView(gpui::UniformListScrollHandle);

    impl Render for SettingsPopupListTestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let rows = 100;
            let list_h = (rows as f32 * 30. + 8.).min(220.);
            div()
                .flex()
                .flex_col()
                .child(div().h(px(34.)))
                .child(
                    uniform_list(
                        "settings-select-list",
                        rows,
                        |range, _, _| {
                            range
                                .map(|ix| {
                                    div()
                                        .h(px(30.))
                                        .w_full()
                                        .child(format!("row {ix}"))
                                        .into_any_element()
                                })
                                .collect()
                        },
                    )
                    .track_scroll(self.0.clone())
                    .w_full()
                    .h(px(list_h))
                    .px(px(4.))
                    .py(px(4.)),
                )
        }
    }

    /// The settings popup lives inside `anchored` inside a 0×0 div, so its
    /// available height is 0. `uniform_list`'s Infer sizing reads the
    /// available height and collapses to nothing there — the explicit height
    /// is what keeps the list visible. Regression test for the empty
    /// font/theme dropdowns.
    #[gpui::test]
    fn settings_popup_list_keeps_a_viewport_in_zero_height_context(
        cx: &mut gpui::TestAppContext,
    ) {
        use gpui::size;

        let cx = cx.add_empty_window();
        let scroll = gpui::UniformListScrollHandle::new();
        let _ = cx.draw(
            point(px(0.), px(0.)),
            size(px(200.), px(0.)),
            |_, cx| cx.new(|_| SettingsPopupListTestView(scroll.clone())),
        );
        let state = scroll.0.borrow();
        let size = state.last_item_size.expect("list was laid out");
        assert!(
            size.item.height > px(0.),
            "settings popup list collapsed to a 0-height viewport"
        );
    }

    /// The same list with `max_h` instead of an explicit height collapses to
    /// 0 — this is the gpui behavior the explicit height works around.
    #[gpui::test]
    fn max_h_collapses_uniform_list_in_zero_height_context(
        cx: &mut gpui::TestAppContext,
    ) {
        use gpui::size;

        let cx = cx.add_empty_window();
        let scroll = gpui::UniformListScrollHandle::new();
        let _ = cx.draw(
            point(px(0.), px(0.)),
            size(px(200.), px(0.)),
            |_, cx| {
                cx.new(|_| MaxHeightListTestView(scroll.clone()))
            },
        );
        let state = scroll.0.borrow();
        let size = state.last_item_size.expect("list was laid out");
        assert_eq!(
            size.item.height,
            px(0.),
            "max_h + Infer sizing collapses to 0 in a 0-height available space"
        );
    }

    /// Same list as [`SettingsPopupListTestView`] but with `max_h` — documents
    /// the gpui sizing behavior the explicit height works around.
    struct MaxHeightListTestView(gpui::UniformListScrollHandle);

    impl Render for MaxHeightListTestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            uniform_list(
                "settings-select-list",
                100,
                |range, _, _| {
                    range
                        .map(|ix| {
                            div()
                                .h(px(30.))
                                .w_full()
                                .child(format!("row {ix}"))
                                .into_any_element()
                        })
                        .collect()
                },
            )
            .track_scroll(self.0.clone())
            .w_full()
            .max_h(px(220.))
            .px(px(4.))
            .py(px(4.))
        }
    }
}


