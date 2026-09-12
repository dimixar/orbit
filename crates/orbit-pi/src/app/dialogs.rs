use super::*;

impl OrbitApp {
    /// Route an `extension_ui_request` from pi.
    ///
    /// Dialog methods (`select` / `confirm` / `input` / `editor`) open the
    /// modal [`Dialog`] and block the run until the user answers. The
    /// remaining methods are fire-and-forget chrome; the ones with a desktop
    /// equivalent are surfaced, the rest are ignored (never faked).
    pub(super) fn handle_extension_ui_request(
        &mut self,
        id: String,
        method: &str,
        value: &Value,
        cx: &mut Context<Self>,
    ) {
        match method {
            "select" | "confirm" | "input" | "editor" => {
                if let Some(request) = DialogRequest::from_event(id, method, value) {
                    self.open_dialog(request, cx);
                }
            }
            // `notify` is a user-facing toast; the status bar is Orbit's quiet
            // equivalent. `setStatus` rides the same transient line.
            "notify" => {
                if let Some(message) = value.get("message").and_then(Value::as_str) {
                    self.set_status(message.to_owned());
                }
            }
            "setStatus" => {
                if let Some(text) = value.get("statusText").and_then(Value::as_str) {
                    self.set_status(text.to_owned());
                }
            }
            // Seed the composer with the extension's text (pi's `setEditorText`).
            "set_editor_text" => {
                if let Some(text) = value.get("text").and_then(Value::as_str) {
                    self.input.update(cx, |input, cx| input.set_text(text, cx));
                }
            }
            // `setWidget` / `setTitle` are terminal chrome with no desktop home.
            _ => {}
        }
    }

    /// Open the modal dialog for a dialog request. pi blocks one request at a
    /// time, so if a dialog is already up the newcomer is cancelled rather
    /// than replacing (and stranding) the first.
    pub(super) fn open_dialog(&mut self, request: DialogRequest, cx: &mut Context<Self>) {
        if self.dialog.is_some() {
            self.respond_to_dialog(&request.id, &DialogResponse::Cancelled);
            return;
        }
        let id = request.id.clone();
        // The entity talks back through a weak handle, the same shape the
        // command palette uses; the app owns the client the answer is sent on.
        let this = cx.weak_entity();
        let on_respond = Box::new(
            move |response: DialogResponse, window: &mut Window, cx: &mut App| {
                let id = id.clone();
                this.update(cx, |app, cx| {
                    app.dialog = None;
                    app.dialog_focus_pending = false;
                    app.respond_to_dialog(&id, &response);
                    // The dialog's focus handle dies with it; hand focus back
                    // to the composer so typing continues.
                    app.input.read(cx).focus(window);
                    cx.notify();
                })
                .ok();
            },
        ) as Box<dyn Fn(DialogResponse, &mut Window, &mut App)>;
        let dialog = cx.new(|cx| Dialog::new(request, on_respond, cx));
        self.dialog = Some(dialog);
        self.dialog_focus_pending = true;
        cx.notify();
    }

    /// Send the answer for a dialog id over RPC.
    pub(super) fn respond_to_dialog(&mut self, id: &str, response: &DialogResponse) {
        let payload = match response {
            DialogResponse::Value(value) => serde_json::json!({
                "type": "extension_ui_response",
                "id": id,
                "value": value
            }),
            DialogResponse::Confirmed(confirmed) => serde_json::json!({
                "type": "extension_ui_response",
                "id": id,
                "confirmed": confirmed
            }),
            DialogResponse::Cancelled => serde_json::json!({
                "type": "extension_ui_response",
                "id": id,
                "cancelled": true
            }),
        };
        self.send(CommandBody::Raw(payload), "extension_ui_response");
    }

    /// Cancel and close the open dialog. Called when the active session is
    /// replaced: the departing (now parked) run would otherwise stay blocked
    /// forever on a question no one can see.
    pub(super) fn cancel_open_dialog(&mut self, cx: &mut Context<Self>) {
        let Some(dialog) = self.dialog.take() else {
            return;
        };
        self.dialog_focus_pending = false;
        let id = dialog.read(cx).request_id().to_string();
        self.respond_to_dialog(&id, &DialogResponse::Cancelled);
        cx.notify();
    }
}
