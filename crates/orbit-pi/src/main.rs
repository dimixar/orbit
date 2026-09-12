//! Orbit Pi — a native desktop workbench for the pi coding agent.
//!
//! Pure Rust on GPUI; the agent runtime is the `pi` CLI spoken to over its
//! JSONL RPC protocol (see `crates/orbit-rpc`). Quit with cmd-q.

mod app;
mod app_icon;
mod assets;
mod auth;
mod branch_picker;
mod checkpoint;
mod command_palette;
mod commit_message;
mod composer;
mod context_meter;
mod dither;
mod git;
mod git_panel;
mod highlight;
mod http;
mod mentions;
mod message_scroller;
mod model_selector;
mod model_selector_match;
mod onboarding;
mod platform;
mod providers;
mod review;
mod sessions;
mod sidepane;
mod theme;
mod transcript;
mod transcript_view;
mod usage;
mod watch;
mod workspace_picker;

use std::time::Duration;

use app::OrbitApp;
use gpui::{
    actions, point, prelude::*, px, size, App, Application, AsyncWindowContext, Bounds, Entity,
    Focusable, KeyBinding, SharedString, Timer, TitlebarOptions, WindowBounds, WindowOptions,
};

// Composer-scoped actions (bound in the `Composer` key context).
actions!(
    composer_keys,
    [
        Backspace,
        Delete,
        Left,
        Right,
        Up,
        Down,
        Newline,
        SelectLeft,
        SelectRight,
        SelectAll,
        Home,
        End,
        Paste,
        Cut,
        Copy,
        Submit,
    ]
);

// App-level actions.
actions!(
    orbit_keys,
    [
        Quit,
        AbortRun,
        NewSession,
        RefreshSessions,
        OpenSettings,
        ToggleUsage,
        ToggleCommandPalette,
        ToggleModelMenu,
        ToggleThinkingMenu,
        CopyLastResponse,
        PrevTurn,
        NextTurn
    ]
);

// Model-selector popup actions (bound to the `Picker` context, which rides
// on the popup's filter input).
actions!(
    picker_keys,
    [
        PickerCancel,
        PickerConfirm,
        PickerSelectNext,
        PickerSelectPrev
    ]
);

// Composer "+" add-menu actions (bound to the `AddMenu` context, which
// rides on the open menu's focus handle).
actions!(
    add_menu_keys,
    [AddMenuNext, AddMenuPrev, AddMenuConfirm, AddMenuClose]
);

fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("cmd-q", Quit, None),
        KeyBinding::new("escape", AbortRun, None),
        // Composer keys only apply while the input is focused.
        KeyBinding::new("backspace", Backspace, Some("Composer")),
        KeyBinding::new("delete", Delete, Some("Composer")),
        KeyBinding::new("left", Left, Some("Composer")),
        KeyBinding::new("right", Right, Some("Composer")),
        KeyBinding::new("shift-left", SelectLeft, Some("Composer")),
        KeyBinding::new("shift-right", SelectRight, Some("Composer")),
        KeyBinding::new("cmd-a", SelectAll, Some("Composer")),
        KeyBinding::new("home", Home, Some("Composer")),
        KeyBinding::new("end", End, Some("Composer")),
        KeyBinding::new("cmd-v", Paste, Some("Composer")),
        KeyBinding::new("cmd-c", Copy, Some("Composer")),
        KeyBinding::new("cmd-x", Cut, Some("Composer")),
        KeyBinding::new("enter", Submit, Some("Composer")),
        KeyBinding::new("cmd-enter", Submit, Some("Composer")),
        KeyBinding::new("shift-enter", Newline, Some("Composer")),
        KeyBinding::new("up", Up, Some("Composer")),
        KeyBinding::new("down", Down, Some("Composer")),
        KeyBinding::new("cmd-n", NewSession, None),
        KeyBinding::new("cmd-r", RefreshSessions, None),
        KeyBinding::new("cmd-,", OpenSettings, None),
        // The Usage page is a destination: cmd-u matches the sidebar row.
        KeyBinding::new("cmd-u", ToggleUsage, None),
        KeyBinding::new("cmd-p", ToggleCommandPalette, None),
        KeyBinding::new("cmd-period", AbortRun, None),
        // Transcript accelerators (work regardless of focus):
        // copy the newest response; jump between user turns like the rail.
        KeyBinding::new("cmd-shift-c", CopyLastResponse, None),
        KeyBinding::new("cmd-up", PrevTurn, None),
        KeyBinding::new("cmd-down", NextTurn, None),
        // Model picker keys — the `Picker` context rides on the popup's
        // filter input, i.e. the *same* dispatch node as `Composer`, so these
        // bindings sit at the same depth as the composer ones. gpui breaks
        // depth ties by registration order, and these are registered later,
        // so enter/escape win over `Submit`/`AbortRun` while the popup is
        // open (arrows don't conflict — the input binds none).
        KeyBinding::new("escape", PickerCancel, Some("Picker")),
        KeyBinding::new("enter", PickerConfirm, Some("Picker")),
        KeyBinding::new("up", PickerSelectPrev, Some("Picker")),
        KeyBinding::new("down", PickerSelectNext, Some("Picker")),
        // Add-menu keys — the `AddMenu` context rides on the menu's own
        // focus handle (deeper than the global escape/enter bindings, so
        // these win while the menu is open).
        KeyBinding::new("escape", AddMenuClose, Some("AddMenu")),
        KeyBinding::new("enter", AddMenuConfirm, Some("AddMenu")),
        KeyBinding::new("up", AddMenuPrev, Some("AddMenu")),
        KeyBinding::new("down", AddMenuNext, Some("AddMenu")),
    ]);
}

fn main() {
    let mut application = Application::new().with_assets(assets::Assets);
    // Remote author avatars need an HTTP client; without one GPUI renders
    // nothing for `img("https://…")` and the monogram fallback shows instead.
    if let Some(client) = http::avatar_client() {
        application = application.with_http_client(client);
    }
    application.run(|cx: &mut App| {
        bind_keys(cx);
        theme::init(cx);
        // GPUI Kit's component layer (data table + plot), then project Orbit's
        // palette onto it so its widgets look like this app rather than
        // another product.
        usage::kit::init(cx);
        let theme = *theme::get(cx);
        usage::kit::sync(cx, &theme);
        // Bundle Zed's UI/mono faces so `.ZedSans`/`.ZedMono` resolve to
        // real fonts (IBM Plex Sans / Lilex) without OS dependencies.
        assets::register_zed_fonts(cx).expect("failed to register Zed fonts");
        app_icon::set_dock_icon();

        // Open maximized: full width of the screen, filling the visible
        // frame (Waku-style workbench). The computed bounds are the
        // restore size macOS returns to when the window is un-zoomed,
        // sized relative to the display so it always fits even on
        // small/scaled screens.
        let (w, h) = match cx.primary_display().map(|d| d.bounds().size) {
            Some(s) => (
                (f32::from(s.width) * 0.85).min(1440.),
                (f32::from(s.height) * 0.9).min(920.),
            ),
            None => (1240., 840.),
        };
        let restore_bounds = Bounds::centered(None, size(px(w), px(h)), cx);
        let _window = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Maximized(restore_bounds)),
                    // Keep the window usable when shrunk: sidebar (min
                    // 200px) + a readable transcript + the composer.
                    window_min_size: Some(size(px(960.), px(640.))),
                    titlebar: Some(TitlebarOptions {
                        title: Some(SharedString::from("Orbit Pi")),
                        // Transparent titlebar: the sidebar extends to the top
                        // and the native traffic lights sit inside it (Waku-style).
                        appears_transparent: true,
                        traffic_light_position: Some(point(px(12.), px(13.))),
                    }),
                    app_id: Some("dev.orbit.pi".into()),
                    focus: true,
                    ..Default::default()
                },
                |window, cx| {
                    let app: Entity<OrbitApp> = cx.new(|cx| OrbitApp::new(cx));

                    // Focus the composer so typing works immediately.
                    app.update(cx, |app, cx| {
                        let handle = app.input.read(cx).focus_handle(cx);
                        window.focus(&handle);
                    });

                    // Heartbeat: drain pi events into the UI.
                    let heartbeat = app.clone();
                    window
                        .spawn(cx, async move |cx: &mut AsyncWindowContext| loop {
                            Timer::after(Duration::from_millis(90)).await;
                            heartbeat
                                .update(cx, |app: &mut OrbitApp, cx| app.tick(cx))
                                .ok();
                        })
                        .detach();

                    // Detect installed editors/terminals for the header
                    // "open in" control (off-thread; icons load once).
                    app.update(cx, |app, cx| app.detect_open_in_apps(cx));

                    app
                },
            )
            .unwrap();

        cx.activate(true);
        cx.on_action(|_: &Quit, cx| cx.quit());
    });
}
