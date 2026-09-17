//! The updater's UI surface: drain its background events into the shell,
//! mirror the persisted preference, and start checks and installs.
//!
//! The updater owns its own threads and status; this module only reads the
//! global and relays [`UpdaterEvent`]s. Checks are silent until an update is
//! ready, so the footer button appears only when there is something to do.

use super::*;
use crate::updater::{UpdateStatus, UpdaterEvent, UpdaterState};

impl OrbitApp {
    /// Whether this build can update itself at all (release, managed install,
    /// and a usable signing key).
    pub(super) fn updater_available(&self, cx: &App) -> bool {
        cx.try_global::<UpdaterState>()
            .is_some_and(|state| state.0.is_some())
    }

    /// Re-read the updater's status and preference. Called when Settings
    /// opens so the toggle reflects what is actually on disk.
    pub(super) fn refresh_updater(&mut self, cx: &App) {
        if let Some(updater) = cx
            .try_global::<UpdaterState>()
            .and_then(|state| state.0.as_ref())
        {
            self.updater_status = updater.status();
            self.automatic_updates_enabled = updater.automatically_checks_for_updates();
        }
    }

    /// Drain the updater channel. The heartbeat calls this each tick; it is
    /// cheap when nothing happened.
    pub(super) fn drain_updater_events(&mut self, cx: &mut Context<Self>) {
        let events: Vec<UpdaterEvent> = cx
            .try_global::<UpdaterState>()
            .and_then(|state| state.0.as_ref())
            .map(|updater| {
                let mut events = Vec::new();
                while let Some(event) = updater.try_recv_event() {
                    events.push(event);
                }
                events
            })
            .unwrap_or_default();
        if events.is_empty() {
            return;
        }

        for event in events {
            match event {
                UpdaterEvent::StatusChanged(status) => {
                    self.updater_status = status;
                }
                UpdaterEvent::UpToDate => {
                    self.updater_status = UpdateStatus::Idle;
                    self.set_status("Orbit is up to date");
                }
                UpdaterEvent::Failed(error) => {
                    self.updater_status = UpdateStatus::Idle;
                    self.set_error(format!("Update failed: {error}"));
                }
                #[cfg(unix)]
                UpdaterEvent::QuitAndInstall => {
                    // The helper has the staged build and is waiting for this
                    // process to finish its normal quit handlers.
                    cx.quit();
                    return;
                }
            }
        }
        cx.notify();
    }

    /// Start staging the available update; the shell quits when the helper
    /// acknowledges the handoff.
    pub(super) fn install_available_update(&mut self, cx: &mut Context<Self>) {
        if self.updater_status != UpdateStatus::Available {
            return;
        }
        let started = cx
            .try_global::<UpdaterState>()
            .and_then(|state| state.0.as_ref())
            .is_some_and(|updater| updater.install_available_update());
        if started {
            self.updater_status = UpdateStatus::Updating;
            self.set_status("Preparing the update…");
            cx.notify();
        }
    }

    /// Run a user-initiated check. `on_check_for_updates` (the ⌘⇧U action) and
    /// the Settings button both land here.
    pub(super) fn begin_update_check(&mut self, cx: &mut Context<Self>) {
        if let Some(updater) = cx
            .try_global::<UpdaterState>()
            .and_then(|state| state.0.as_ref())
        {
            updater.check_for_updates();
            self.set_status("Checking for updates…");
        } else {
            self.set_status("Updates are not available in this build");
        }
        cx.notify();
    }

    pub(super) fn on_check_for_updates(
        &mut self,
        _: &crate::CheckForUpdates,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.begin_update_check(cx);
    }

    /// Persist the automatic-check preference and mirror it for the next frame.
    pub(super) fn set_automatic_updates(&mut self, enabled: bool, cx: &mut Context<Self>) {
        if let Some(updater) = cx
            .try_global::<UpdaterState>()
            .and_then(|state| state.0.as_ref())
        {
            updater.set_automatically_checks_for_updates(enabled);
        }
        self.automatic_updates_enabled = enabled;
        cx.notify();
    }

    /// The sidebar footer's update control. Hidden while idle; a quiet
    /// "Updating…" pill while installing; a clickable "Update" pill when a
    /// signed release is staged and ready.
    pub(super) fn sidebar_updater_button(
        &self,
        theme: Theme,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let status = self.updater_status;
        if status == UpdateStatus::Idle {
            return None;
        }
        let available = status == UpdateStatus::Available;
        let mut button = div()
            .id("sidebar-update")
            .h(px(26.))
            .px(px(10.))
            .rounded_full()
            .flex()
            .items_center()
            .gap(px(6.))
            .text_size(theme.ui_px(11.5))
            .font_weight(FontWeight::MEDIUM);
        if available {
            button = button
                .bg(theme.accent)
                .text_color(theme.bg_main)
                .cursor_pointer()
                .hover(|s| s.opacity(0.9))
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(|this, _: &MouseUpEvent, _window, cx| {
                        this.install_available_update(cx);
                    }),
                )
                .child("Update");
        } else {
            button = button
                .bg(theme.bg_hover)
                .text_color(theme.text_3)
                .cursor_default()
                .child("Updating\u{2026}");
        }
        Some(button.into_any_element())
    }

    /// Settings → General rows for the updater, empty when this build cannot
    /// update itself (so debug and bare binaries never show a dead control).
    pub(super) fn updater_rows(
        &self,
        theme: Theme,
        this: Entity<OrbitApp>,
        cx: &Context<Self>,
    ) -> Vec<AnyElement> {
        if !self.updater_available(cx) {
            return Vec::new();
        }
        vec![
            self.card(
                theme,
                "Automatic updates",
                "Check for a newer signed release once at launch. Updates install after Orbit quits.",
                Some(self.automatic_updates_toggle(theme, this.clone())),
            ),
            self.card(
                theme,
                "Check for updates",
                "Verify a new release now; a staged update downloads, verifies, and installs after Orbit quits.",
                Some(self.check_for_updates_button(theme, this)),
            ),
        ]
    }

    /// Settings → General toggle for scheduled checks.
    pub(super) fn automatic_updates_toggle(
        &self,
        theme: Theme,
        this: Entity<OrbitApp>,
    ) -> AnyElement {
        let on = self.automatic_updates_enabled;
        div()
            .id("settings-automatic-updates")
            .w(px(36.))
            .h(px(20.))
            .rounded_full()
            .p(px(2.))
            .border_1()
            .border_color(theme.border)
            .flex()
            .items_center()
            .cursor_pointer()
            .when(on, |track| track.bg(theme.accent).justify_end())
            .when(!on, |track| track.bg(theme.bg_raised).justify_start())
            .on_mouse_up(MouseButton::Left, move |_, _, cx| {
                this.update(cx, |app, cx| {
                    let next = !app.automatic_updates_enabled;
                    app.set_automatic_updates(next, cx);
                });
            })
            .child(div().size(px(14.)).rounded_full().bg(theme.text))
            .into_any_element()
    }

    /// Settings → General "Check for Updates" button.
    pub(super) fn check_for_updates_button(
        &self,
        theme: Theme,
        this: Entity<OrbitApp>,
    ) -> AnyElement {
        div()
            .id("settings-check-updates")
            .h(px(26.))
            .px(px(12.))
            .rounded_md()
            .border_1()
            .border_color(theme.border)
            .bg(theme.bg_raised)
            .flex()
            .items_center()
            .cursor_pointer()
            .hover(|s| s.bg(theme.bg_hover))
            .text_size(theme.ui_px(12.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(theme.text_2)
            .on_mouse_up(MouseButton::Left, move |_, _, cx| {
                this.update(cx, |app, cx| app.begin_update_check(cx));
            })
            .child("Check for Updates\u{2026}")
            .into_any_element()
    }
}
