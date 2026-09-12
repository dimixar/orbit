use super::*;

impl OrbitApp {
    /// Heartbeat (~90ms): drain protocol events into the UI.
    pub(crate) fn tick(&mut self, cx: &mut Context<Self>) {
        // Let a lapsed status message disappear from the status bar.
        if self
            .status_at
            .is_some_and(|at| at.elapsed() >= STATUS_MESSAGE_TTL)
        {
            self.status_at = None;
            cx.notify();
        }
        self.tick_background(cx);
        // Sessions can be written by the CLI or another Orbit window; the
        // watcher has already scanned off-thread, so this only swaps the list.
        if let Some(reloaded) = self
            .session_watcher
            .as_ref()
            .and_then(sessions::SessionWatcher::take_reload)
        {
            self.sessions = reloaded;
            self.sync_session_menu(cx);
            // The usage index is built from the same files; a write means the
            // analytics are stale (rate-limited inside the page).
            if self.usage_open {
                self.usage.update(cx, |page, cx| page.mark_stale(cx));
            }
            cx.notify();
        }
        // Usage: drain any finished scan and recompute if the filter or the
        // index moved. Cheap when nothing changed.
        self.usage.update(cx, |page, cx| page.sync(cx));
        // Workspace edits (pi, the user, or git) have no RPC event either;
        // refresh Review and the Git page when the tree changes.
        self.sync_workspace_watcher();
        if self
            .workspace_watcher
            .as_ref()
            .is_some_and(watch::WorkspaceWatcher::take_dirty)
        {
            self.sidepane
                .update(cx, |pane, cx| pane.mark_review_stale(cx));
            self.git_panel.update(cx, |panel, cx| panel.refresh(cx));
            cx.notify();
        }
        // Expire a stalled login and auto-cancel it with pi.
        let had_login = self.auth.login().is_some();
        let auth_effects = self.auth.poll(Instant::now());
        self.handle_auth_effects(auth_effects, cx);
        if had_login || self.auth.login().is_some() {
            cx.notify();
        }
        // Keep the Runtime panel's liveness fresh (try_wait is cheap).
        if let Some(client) = self.client.as_mut() {
            self.runtime.alive = client.is_alive();
        }
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
        // The one-time rail hint dismisses itself once its TTL lapses.
        if self.transcript.rail_hint_timed_out() {
            cx.notify();
        }
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
                Event::Auth(event) => {
                    let effects = self.auth.on_event(event.clone());
                    self.handle_auth_effects(effects, cx);
                }
                Event::AutoRetryStart { value } => {
                    // A transient provider error is not a user-facing failure:
                    // it rides the quiet run-status strip (with the attempt
                    // counter) until the retry resolves — never the banner.
                    self.retrying = true;
                    self.retry_detail = Some(RetryDetail {
                        attempt: value.get("attempt").and_then(Value::as_u64).unwrap_or(1),
                        max: value.get("maxAttempts").and_then(Value::as_u64),
                        error: value
                            .get("errorMessage")
                            .and_then(Value::as_str)
                            .unwrap_or("transient error")
                            .to_string(),
                    });
                }
                Event::AutoRetryEnd { value } => {
                    self.retrying = false;
                    self.retry_detail = None;
                    let success = value.get("success").and_then(|v| v.as_bool());
                    if success == Some(false) {
                        let error = value
                            .get("finalError")
                            .and_then(|v| v.as_str())
                            .unwrap_or("pi exhausted its automatic retries");
                        self.set_error(format!("Automatic retry failed: {error}"));
                    }
                }
                Event::QueueUpdate { value } => {
                    self.queue = PendingQueue::from_value(value);
                    // pi confirmed the optimistic follow-up; stop tracking it.
                    let confirmed = self
                        .pending_follow_up
                        .as_ref()
                        .is_some_and(|pending| self.queue.follow_up.iter().any(|t| t == pending));
                    if confirmed {
                        self.pending_follow_up = None;
                    }
                }
                Event::ExtensionError { value } => {
                    let path = value
                        .get("extensionPath")
                        .and_then(Value::as_str)
                        .unwrap_or("");
                    let name = Path::new(path)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .filter(|n| !n.is_empty())
                        .unwrap_or("extension");
                    let hook = value
                        .get("event")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown");
                    let error = value
                        .get("error")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown error");
                    self.set_error(format!("Extension {name} failed on {hook}: {error}"));
                }
                Event::AgentSettled => {
                    self.busy = false;
                    self.retrying = false;
                    self.retry_detail = None;
                    self.pending_follow_up = None;
                    // A settled run has no queued continuation left; clear the
                    // bar even if the final `queue_update` was missed.
                    self.queue = PendingQueue::default();
                    refresh_sessions = true;
                    self.refresh_context_stats();
                    // Capture the turn's end checkpoint, then refresh Review.
                    self.finish_turn(cx);
                }
                Event::CompactionStart { .. } => {
                    // The run-status strip carries the in-progress state;
                    // no transient status line that would lapse mid-compact.
                    self.is_compacting = true;
                }
                Event::CompactionEnd { value } => {
                    self.is_compacting = false;
                    // A failed compaction carries `errorMessage` (docs).
                    if let Some(error) = value.get("errorMessage").and_then(Value::as_str) {
                        self.set_error(format!("Compaction failed: {error}"));
                    } else if value.get("aborted").and_then(Value::as_bool) == Some(true) {
                        self.set_status("Compaction aborted");
                    }
                    // Post-compaction usage is unknown until the next turn;
                    // refresh so the meter can show an empty/unknown state.
                    self.refresh_context_stats();
                }
                Event::ProcessExited => {
                    self.busy = false;
                    self.retrying = false;
                    self.retry_detail = None;
                    self.pending_follow_up = None;
                    self.queue = PendingQueue::default();
                    self.runtime.alive = false;
                    self.runtime.exited = true;
                    self.auth.on_disconnect();
                    self.set_error("pi process exited — restart it from Settings → Runtime");
                }
                Event::MessageEnd { value } => {
                    // A failed LLM call ends the assistant message with
                    // `stopReason: "error"` + `errorMessage` (e.g. an
                    // unsupported model). Surface it in the banner; the
                    // transcript renders the same text inline.
                    if let Some(error) = transcript::message_error(value) {
                        self.set_error(format!("Agent error: {error}"));
                    } else if self
                        .error
                        .as_deref()
                        .is_some_and(|error| error.starts_with("Agent error:"))
                    {
                        // The next attempt produced a message — clear the
                        // stale agent-error banner.
                        self.error = None;
                    }
                    // Real edit stats from finalized tool calls.
                    let (a, r) = transcript::diff_from_message(value);
                    self.added += a;
                    self.removed += r;
                }
                Event::Response {
                    command,
                    success,
                    data,
                    error,
                    id: _,
                } => {
                    self.on_response(
                        command,
                        *success,
                        data.as_ref(),
                        error.as_deref(),
                        &mut refresh_sessions,
                        cx,
                    );
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
    pub(super) fn tick_background(&mut self, cx: &mut Context<Self>) {
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

    pub(super) fn on_response(
        &mut self,
        command: &str,
        success: bool,
        data: Option<&serde_json::Value>,
        error: Option<&str>,
        refresh_sessions: &mut bool,
        cx: &mut Context<Self>,
    ) {
        // Provider-auth commands are handled here so success/failure and the
        // no-payload replies (`auth.cancel`) never fall through the
        // data-required branch below.
        if command.starts_with("auth.") {
            self.on_auth_response(command, success, data, error, cx);
            return;
        }
        // Error handling (docs #error-handling): a failed command carries an
        // `error` string. Surface it, unwind command-specific optimistic
        // state, and stop — success paths below assume `data` is valid.
        if !success {
            self.on_command_failure(command, error, cx);
            match command {
                "compact" => {
                    self.is_compacting = false;
                    self.refresh_context_stats();
                }
                // These changed local UI optimistically; re-read pi's state.
                "set_model"
                | "cycle_model"
                | "set_thinking_level"
                | "cycle_thinking_level"
                | "set_session_name" => {
                    self.send(CommandBody::GetState, "get_state");
                }
                _ => {}
            }
            return;
        }
        // A successful retry clears the banner it raised earlier.
        self.clear_error_for(command);
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
            // Agent-control commands answer with no payload; nothing more to do.
            "set_steering_mode"
            | "set_follow_up_mode"
            | "set_auto_compaction"
            | "set_auto_retry"
            | "abort_retry"
            | "set_session_name"
            | "steer"
            | "follow_up" => {
                return;
            }
            // `compact` also has a result payload on success; clearing the
            // compacting state belongs before the data gate.
            "compact" => {
                self.is_compacting = false;
                self.set_status("Context compacted");
                self.refresh_context_stats();
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
                    if let Some(provider) =
                        model.get("provider").and_then(serde_json::Value::as_str)
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
                    if self.session_id.as_deref() != Some(id) {
                        self.session_id = Some(id.to_string());
                        self.recover_latest_turn(cx);
                    }
                }
                // Agent control surface: streaming/compaction state, queue
                // modes, auto-compaction, and the session display name.
                let state = SessionState::from_value(data);
                self.busy = state.is_streaming || state.is_compacting;
                self.is_compacting = state.is_compacting;
                self.follow_up_mode = state.follow_up_mode;
                self.auto_compaction = state.auto_compaction_enabled;
                match &state.session_name {
                    Some(name) if self.session_name.as_deref() != Some(name.as_str()) => {
                        self.session_name = Some(name.clone());
                        let name = name.clone();
                        self.session_name_input
                            .update(cx, |input, cx| input.set_text(name, cx));
                    }
                    None if self.session_name.is_some() => {
                        self.session_name = None;
                        self.session_name_input
                            .update(cx, |input, cx| input.set_text(String::new(), cx));
                    }
                    _ => {}
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
                                    context_window: m
                                        .get("contextWindow")
                                        .and_then(serde_json::Value::as_u64),
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
                self.session_usage = SessionUsage::from_stats(data);
                if let Some(file) = data.get("sessionFile").and_then(Value::as_str) {
                    self.adopt_session_file(PathBuf::from(file));
                }
            }
            "switch_session" => {
                if success {
                    self.reset_turns();
                    self.reset_queue();
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
                self.session_usage = None;
                self.reset_turns();
                self.reset_queue();
                *refresh_sessions = true;
                self.send(CommandBody::GetState, "get_state");
                self.refresh_catalogs();
            }
            // `clear_queue` echoes the text it dropped so Escape can put it
            // back in the composer (docs' interactive-Esc behavior).
            "clear_queue" => {
                let queue = PendingQueue::from_value(data);
                if self.restore_queue_on_clear {
                    self.restore_queue_on_clear = false;
                    if let Some(text) = queue.restore_text() {
                        self.input.update(cx, |input, cx| input.set_text(text, cx));
                    }
                }
                // `queue_update` mirrors the same fact; this keeps the row
                // correct even if the event is missed.
                self.queue = queue;
            }
            _ => {
                let _ = success;
            }
        }
    }

    /// Apply a provider-auth command response to the [`AuthManager`] and act
    /// on any effects it returns.
    pub(super) fn on_auth_response(
        &mut self,
        command: &str,
        success: bool,
        data: Option<&serde_json::Value>,
        error: Option<&str>,
        cx: &mut Context<Self>,
    ) {
        match command {
            "auth.list" => {
                let before = self.auth.support();
                self.auth.on_list_response(success, data, error);
                if self.auth.support() != before && self.auth.support() == AuthSupport::Unsupported
                {
                    self.set_status(
                        "This pi build has no auth RPC — provider sign-in uses Terminal",
                    );
                }
            }
            "auth.status" => self.auth.on_status_response(success, data),
            "auth.login" => {
                self.auth.on_login_response(success, data, error);
                // Acknowledgement can arrive after a `started` event; make
                // sure the card reflects the manager either way.
            }
            "auth.logout" => {
                if let Some(provider) = data
                    .and_then(|data| data.get("provider"))
                    .and_then(serde_json::Value::as_str)
                {
                    self.auth.on_logout_response(success, provider);
                } else if success {
                    // Some servers answer without echoing the provider; the
                    // UI already refreshed via `auth_credentials_changed`.
                }
            }
            "auth.cancel" => {
                // Nothing to reconcile: the event stream confirms cancellation.
            }
            _ => {}
        }
        cx.notify();
    }

    /// Move images the composer collected (clipboard paste) into the
    /// attachment queue. Called on tick and again at submit so a paste
    /// immediately followed by Enter still attaches.
    pub(super) fn drain_pasted_images(&mut self, cx: &mut Context<Self>) {
        if !self.input.read(cx).has_pasted_images() {
            return;
        }
        let pasted = self
            .input
            .update(cx, |input, _| std::mem::take(&mut input.pasted_images));
        for image in &pasted {
            if self.attachments.len() >= MAX_ATTACHMENTS {
                self.set_status(format!("at most {MAX_ATTACHMENTS} images per message"));
                break;
            }
            let index = self.attachments.len();
            self.attachments.push(Attachment::from_image(image, index));
        }
        if !pasted.is_empty() {
            cx.notify();
        }
    }

    /// Re-point the workspace watcher when the active workspace moves.
    /// Comparing the last-attempted dir (rather than the live watcher) means
    /// a backend that failed to start is retried on the next workspace
    /// change, not every heartbeat.
    pub(super) fn sync_workspace_watcher(&mut self) {
        if self.workspace_watch_dir == self.current_workspace {
            return;
        }
        self.workspace_watch_dir = self.current_workspace.clone();
        self.workspace_watcher = self
            .current_workspace
            .as_deref()
            .and_then(watch::WorkspaceWatcher::start);
    }
}
