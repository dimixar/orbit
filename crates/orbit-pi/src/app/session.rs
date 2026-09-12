use super::*;
use super::helpers::*;

impl OrbitApp {
    pub(super) fn submit(&mut self, text: String, cx: &mut Context<Self>) {
        let text = text.trim().to_string();
        if text.is_empty() {
            return;
        }
        // Collect any paste that raced the submit tick.
        self.drain_pasted_images(cx);
        // pi's `follow_up` and `prompt` carry images, so attachments ride
        // whatever we send.
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
        // While the agent is running, the message is queued as a follow-up:
        // it is delivered only once the current task finishes. The queued bar
        // above the composer shows it until then; pi emits the user message
        // into the transcript when it is actually delivered.
        let running = self.busy || self.transcript.is_streaming();
        if running {
            let body = CommandBody::FollowUp {
                message: text.clone(),
                images,
            };
            if !self.send(body, "follow_up") {
                // Keep the prompt and attachments so nothing is lost.
                self.attachments = attachments;
                cx.notify();
                return;
            }
            // Show it immediately; the next `queue_update` reconciles the list.
            self.queue.follow_up.push(text.clone());
            self.pending_follow_up = Some(text);
            self.input.update(cx, |input, cx| input.clear(cx));
            cx.notify();
            return;
        }
        // Not running: a normal prompt starts a new turn.
        let body = CommandBody::Prompt {
            message: text.clone(),
            images,
            streaming_behavior: None,
        };
        if !self.send(body, "prompt") {
            // Keep the prompt and the queued attachments so the user can
            // retry (pi offline, broken pipe, …) instead of losing work.
            self.attachments = attachments;
            cx.notify();
            return;
        }
        self.begin_turn(cx);
        // Show the prompt immediately — pi does not echo it back in RPC mode.
        // The decoded previews ride along so the chat window shows what was
        // attached (pi echoes/snapshots carry image blocks for reloads).
        self.transcript.append_user_message(
            &text,
            attachments
                .iter()
                .filter_map(|a| a.preview.clone())
                .collect(),
        );
        self.input.update(cx, |input, cx| input.clear(cx));
        cx.notify();
    }

    /// Start a new user turn: snapshot the workspace so Review's **Last Turn**
    /// can diff exactly what the agent changes, then let the prompt run.
    pub(super) fn begin_turn(&mut self, cx: &mut Context<Self>) {
        if self.turn_open {
            return;
        }
        let Some(session) = self.session_id.clone() else {
            return;
        };
        self.turn_count += 1;
        self.turn_open = true;
        let turn = self.turn_count;
        let cwd = self
            .current_workspace
            .clone()
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_default();
        cx.spawn(async move |_this, cx| {
            let _ = cx
                .background_executor()
                .spawn(async move { checkpoint::capture_turn_start(&cwd, &session, turn) })
                .await;
        })
        .detach();
    }

    /// A run settled: snapshot the workspace's end state so Review's **Last
    /// Turn** has a complete range, then mark Review stale. Without an open
    /// turn (e.g. a settled retry) it still refreshes Review.
    pub(super) fn finish_turn(&mut self, cx: &mut Context<Self>) {
        let Some(session) = self.session_id.clone() else {
            self.turn_open = false;
            self.sidepane
                .update(cx, |pane, cx| pane.mark_review_stale(cx));
            return;
        };
        if !self.turn_open {
            self.sidepane
                .update(cx, |pane, cx| pane.mark_review_stale(cx));
            return;
        }
        // Close the turn synchronously so a second settle event can't start a
        // duplicate capture while this one is still running.
        self.turn_open = false;
        let turn = self.turn_count;
        let cwd = self
            .current_workspace
            .clone()
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_default();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { checkpoint::capture_turn(&cwd, &session, turn) })
                .await;
            let _ = this.update(cx, |app, cx| {
                app.turn_open = false;
                if result.is_ok() {
                    app.latest_turn = Some(turn);
                }
                app.sidepane
                    .update(cx, |pane, cx| pane.mark_review_stale(cx));
                cx.notify();
            });
        })
        .detach();
    }

    /// Forget turn checkpoints when the session or workspace changes.
    pub(super) fn reset_turns(&mut self) {
        self.turn_count = 0;
        self.turn_open = false;
        self.latest_turn = None;
    }

    /// Forget the queued-message mirror — the queue belongs to the previous
    /// session/process. The next `queue_update`/`get_state` re-establishes it.
    pub(super) fn reset_queue(&mut self) {
        self.queue = PendingQueue::default();
        self.restore_queue_on_clear = false;
    }

    /// Recover the newest completed turn from the persisted checkpoint refs,
    /// so Review's **Last Turn** is available immediately after a restart or
    /// session switch. Waku persists the same fact in its session model; Orbit
    /// keeps it in `refs/orbit/…` and reads it back here.
    pub(super) fn recover_latest_turn(&mut self, cx: &mut Context<Self>) {
        let Some(session) = self.session_id.clone() else {
            return;
        };
        let cwd = self
            .current_workspace
            .clone()
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_default();
        cx.spawn(async move |this, cx| {
            let lookup_session = session.clone();
            let latest = cx
                .background_executor()
                .spawn(async move { checkpoint::latest_turn(&cwd, &lookup_session) })
                .await;
            let _ = this.update(cx, |app, cx| {
                if app.session_id.as_deref() != Some(session.as_str()) {
                    return;
                }
                app.latest_turn = latest;
                if let Some(latest) = latest {
                    // Continue numbering after the recovered turn so new refs
                    // never clobber the persisted ones.
                    app.turn_count = app.turn_count.max(latest);
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn on_submit(&mut self, _: &crate::Submit, _: &mut Window, cx: &mut Context<Self>) {
        // Enter commits the highlighted autocomplete entry while the menu
        // is open; a second Enter submits.
        if self.commit_autocomplete_if_open(cx) {
            return;
        }
        let text = self.input.read(cx).text();
        self.submit(text, cx);
    }

    pub(super) fn on_send_click(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.commit_autocomplete_if_open(cx) {
            return;
        }
        let text = self.input.read(cx).text();
        self.submit(text, cx);
    }

    pub(super) fn on_abort(&mut self, _: &crate::AbortRun, window: &mut Window, cx: &mut Context<Self>) {
        // Escape backs out of the topmost surface: the command palette (when
        // focus somehow sits outside it), the autocomplete menu first, then
        // settings, popovers, then a running agent.
        if self.command_palette.take().is_some() {
            self.input.read(cx).focus(window);
            cx.notify();
            return;
        }
        if self.workspace_picker.take().is_some() {
            self.input.read(cx).focus(window);
            cx.notify();
            return;
        }
        if self.autocomplete.borrow().open {
            self.autocomplete_dismissed = true;
            cx.notify();
            return;
        }
        if self.add_menu_open {
            self.close_add_menu(window, cx);
            return;
        }
        if self.git_open && self.git_panel.read(cx).has_modal() {
            self.git_panel
                .update(cx, |panel, cx| panel.dismiss_modal(cx));
            return;
        }
        if self.git_open {
            self.close_git(cx);
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
        // The Review pane's source menu is closed by Escape before the key
        // falls through to aborting a run.
        if self.sidepane.read(cx).is_source_menu_open() {
            self.sidepane
                .update(cx, |pane, cx| pane.close_source_menu(cx));
            return;
        }
        // Interactive Esc: drop the pending queue first so its text can be
        // restored to the composer when the `clear_queue` response lands
        // (docs), then abort the run.
        if !self.queue.is_empty() {
            self.restore_queue_on_clear = true;
            self.send(CommandBody::ClearQueue, "clear_queue");
        }
        self.send(CommandBody::Abort, "abort");
        cx.notify();
    }

    pub(super) fn on_abort_mouse(&mut self, _: &MouseUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.on_abort(&crate::AbortRun, window, cx);
    }

    pub(super) fn on_new_session(
        &mut self,
        _: &crate::NewSession,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.send(CommandBody::NewSession, "new_session");
        cx.notify();
    }

    /// Plus on a workspace group: start a fresh session rooted at that cwd.
    pub(super) fn on_new_session_in_workspace(
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
        self.reset_turns();
        self.reset_queue();
        self.current_workspace = Some(cwd.clone());

        match PiClient::spawn(&cwd, None) {
            Ok(client) => {
                self.adopt_client(client);
                self.send(CommandBody::NewSession, "new_session");
                self.refresh_catalogs();
                self.set_status("New session");
            }
            Err(err) => {
                let message = format!("pi spawn failed: {err}");
                self.client = None;
                self.runtime.error = Some(message.clone());
                self.set_status(message);
            }
        }
        self.input.read(cx).focus(window);
        cx.notify();
    }

    /// Open the OS folder picker and start (or restart) the task in the
    /// selected directory. Used from the new-task page and the status bar.
    pub(super) fn on_pick_folder_click(
        &mut self,
        _: &MouseUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.browse_for_folder(window, cx);
    }

    /// The native folder dialog — the workspace picker's "Choose folder…" row
    /// and the status-bar chip both land here.
    pub(super) fn browse_for_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let picked = rfd::FileDialog::new()
            .set_title("Choose a folder for this task")
            .pick_folder();
        if let Some(folder) = picked {
            self.start_task_in_folder(folder, window, cx);
        }
    }

    /// Switch the live session. With `push`, the visit is recorded in the
    /// top-bar history (forward entries are dropped, like browser history).
    ///
    /// Each session gets its own pi process, so switching never interrupts a
    /// run: the outgoing session is *parked* mid-run (its process and live
    /// transcript keep going in the background — events drain every tick),
    /// and a parked target resumes exactly where it left off. Idle sessions
    /// are torn down and reload from disk when reopened.
    pub(super) fn switch_to_session(&mut self, session: SessionInfo, push: bool, cx: &mut Context<Self>) {
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
        // The queue belongs to the session we just left; the target's state
        // re-establishes it from `get_state`/`queue_update`.
        self.reset_queue();

        // ── activate the target ──
        if let Some(parked) = self.lives.remove(&session.path) {
            // Resume a background run. The parked transcript is already up
            // to date (its events drain every tick); anything buffered in
            // the process channel streams in from the next tick on.
            self.adopt_client(parked.client);
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
                    self.adopt_client(client);
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
                    let message = format!("pi spawn failed: {err}");
                    self.client = None;
                    self.runtime.error = Some(message.clone());
                    self.set_status(message);
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

    pub(super) fn on_open_session(&mut self, session: SessionInfo, cx: &mut Context<Self>) {
        self.switch_to_session(session, true, cx);
    }

    pub(super) fn on_history_back(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.history_index > 0 {
            self.history_index -= 1;
            if let Some(session) = self.session_history.get(self.history_index).cloned() {
                self.switch_to_session(session, false, cx);
                return;
            }
        }
        cx.notify();
    }

    pub(super) fn on_history_forward(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.history_index + 1 < self.session_history.len() {
            self.history_index += 1;
            if let Some(session) = self.session_history.get(self.history_index).cloned() {
                self.switch_to_session(session, false, cx);
                return;
            }
        }
        cx.notify();
    }

    pub(super) fn on_info_click(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        // The top-bar info affordance shows the active session's details.
        self.session_details_open = !self.session_details_open;
        cx.notify();
    }

    /// Open the full-page Git panel (Changes / History / Graph) and load it.
    pub(super) fn open_git(&mut self, cx: &mut Context<Self>) {
        self.git_open = true;
        // One main-area page at a time.
        self.usage_open = false;
        self.session_details_open = false;
        self.git_panel.update(cx, |panel, cx| panel.show(cx));
        cx.notify();
    }

    /// Top-bar GitHub affordance: same destination as the session-details
    /// **Commit or push** row.
    pub(super) fn on_open_git_click(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.open_git(cx);
    }

    /// Close the Git page and return to the chat.
    pub(super) fn close_git(&mut self, cx: &mut Context<Self>) {
        self.git_open = false;
        self.git_panel.update(cx, |panel, cx| panel.hide(cx));
        cx.notify();
    }

    /// Open the Usage page and let it load (or refresh) the session store.
    pub(super) fn open_usage(&mut self, cx: &mut Context<Self>) {
        self.usage_open = true;
        self.git_open = false;
        self.session_details_open = false;
        self.usage.update(cx, |page, cx| page.open(cx));
        cx.notify();
    }

    /// Leave the Usage page.
    pub(super) fn close_usage(&mut self, cx: &mut Context<Self>) {
        self.usage_open = false;
        self.usage.update(cx, |page, cx| page.close(cx));
        cx.notify();
    }

    /// Sidebar nav row: Usage is a destination, toggled like Settings.
    pub(super) fn on_usage_nav_click(
        &mut self,
        _: &MouseUpEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.usage_open {
            self.close_usage(cx);
        } else {
            self.open_usage(cx);
        }
    }

    /// Open a session the Usage page named, by pi's session id (§29). The id
    /// is resolved against the loaded session list; a session the sidebar has
    /// not picked up yet forces one reload before giving up.
    pub(super) fn open_session_by_id(&mut self, id: &str, _: &mut Window, cx: &mut Context<Self>) {
        let mut target = self.sessions.iter().find(|s| s.id == id).cloned();
        if target.is_none() {
            self.sessions = sessions::load_sessions();
            target = self.sessions.iter().find(|s| s.id == id).cloned();
        }
        if let Some(session) = target {
            self.usage_open = false;
            self.switch_to_session(session, true, cx);
            cx.notify();
        }
    }

    /// The top-bar info popover: active session's environment + identifiers.
    pub(super) fn render_session_details_popup(&self, cx: &Context<Self>) -> Option<AnyElement> {
        if !self.session_details_open {
            return None;
        }
        let theme = *theme::get(cx);
        let this = cx.entity();
        let title = self
            .current_title
            .clone()
            .unwrap_or_else(|| "New task".into());
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
                        move |_, _window, cx| {
                            this.update(cx, |app, cx| {
                                app.open_git(cx);
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
    pub(super) fn session_detail_row(
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
                            .child(if value.is_empty() {
                                "—".to_string()
                            } else {
                                value
                            }),
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

    pub(super) fn on_toggle_sidebar(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.sidebar_visible = !self.sidebar_visible;
        cx.notify();
    }

    pub(super) fn on_toggle_side_pane(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.sidepane.update(cx, |pane, cx| pane.toggle(cx));
        cx.notify();
    }

    /// The top-bar `+N -M` chip opens Review on the working tree's
    /// **Uncommitted** changes.
    pub(super) fn on_open_uncommitted_review(
        &mut self,
        _: &MouseUpEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.sidepane
            .update(cx, |pane, cx| pane.show_uncommitted(cx));
        cx.notify();
    }

    /// Hands the transcript's changed-files cards a way to open the side
    /// pane's Review tab on that run's **Last Turn** git diff (the pane lives
    /// in the app, the cards don't know that). Cheap to build per frame — an
    /// `Rc` closure over the entity and the latest captured turn.
    pub(super) fn review_opener(&self, _: &Context<Self>) -> crate::transcript_view::ReviewOpener {
        let pane = self.sidepane.clone();
        let latest = self.latest_turn;
        Rc::new(move |_window, cx| {
            pane.update(cx, |pane, cx| pane.show_review_turn(latest, cx));
        })
    }

    pub(super) fn on_composer_click(&mut self, _: &MouseUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.input.read(cx).focus(window);
    }

    pub(super) fn on_refresh(&mut self, _: &crate::RefreshSessions, _: &mut Window, cx: &mut Context<Self>) {
        self.sessions = sessions::load_sessions();
        self.sync_session_menu(cx);
        // ⌘R on the Usage page refreshes the analytics too.
        if self.usage_open {
            self.usage.update(cx, |page, cx| page.refresh(cx));
        }
        cx.notify();
    }

    /// `cmd-shift-c`: copy the newest assistant response — the keyboard
    /// mirror of the message footer's copy button. Lights the same green
    /// check in that row's footer.
    pub(super) fn on_copy_last_response(
        &mut self,
        _: &crate::CopyLastResponse,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((ix, text)) = self.transcript.last_response_text() else {
            self.set_status("No response to copy yet");
            cx.notify();
            return;
        };
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.transcript.mark_copied(ix);
        self.set_status("Copied latest response");
        cx.notify();
    }

    /// `cmd-up` / `cmd-down`: jump between user turns — the keyboard mirror
    /// of the navigation rail. Jumping is also using the rail, so it
    /// dismisses the one-time rail hint.
    pub(super) fn on_prev_turn(&mut self, _: &crate::PrevTurn, _: &mut Window, cx: &mut Context<Self>) {
        self.transcript.jump_turn(-1);
        self.transcript.dismiss_rail_hint();
        cx.notify();
    }

    pub(super) fn on_next_turn(&mut self, _: &crate::NextTurn, _: &mut Window, cx: &mut Context<Self>) {
        self.transcript.jump_turn(1);
        self.transcript.dismiss_rail_hint();
        cx.notify();
    }
}
