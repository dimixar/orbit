//! Orbit Pi — a native desktop workbench for the pi coding agent.
//!
//! Pure Rust on GPUI; the agent runtime is the `pi` CLI spoken to over its
//! JSONL RPC protocol (see `crates/orbit-rpc`). Quit with cmd-q.

mod app;
mod assets;
mod composer;
mod file_icons;
mod model_selector;
mod sessions;
mod transcript;

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
        ToggleModelMenu,
        ToggleThinkingMenu
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
        KeyBinding::new("cmd-n", NewSession, None),
        KeyBinding::new("cmd-r", RefreshSessions, None),
        KeyBinding::new("cmd-,", OpenSettings, None),
        KeyBinding::new("cmd-period", AbortRun, None),
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
    ]);
}

fn main() {
    Application::new()
        .with_assets(assets::Assets)
        .run(|cx: &mut App| {
            // Nerd Font for devicons filetype glyphs (see file_icons.rs).
            cx.text_system()
                .add_fonts(vec![std::borrow::Cow::Borrowed(
                    include_bytes!("../assets/fonts/SymbolsNerdFont-Regular.ttf").as_slice(),
                )])
                .expect("failed to register embedded Symbols Nerd Font");

            bind_keys(cx);

            // Size the window relative to the display so it always fits and
            // centers, even on small/scaled screens (a hardcoded 1240x840 gets
            // clamped and shoved top-left by macOS on ~1366x768 displays).
            let (w, h) = match cx.primary_display().map(|d| d.bounds().size) {
                Some(s) => (
                    (f32::from(s.width) * 0.85).min(1440.),
                    (f32::from(s.height) * 0.9).min(920.),
                ),
                None => (1240., 840.),
            };
            let bounds = Bounds::centered(None, size(px(w), px(h)), cx);
            let _window = cx
                .open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(bounds)),
                        titlebar: Some(TitlebarOptions {
                            title: Some(SharedString::from("Orbit Pi")),
                            // Transparent titlebar: the sidebar extends to the top
                            // and the native traffic lights sit inside it (Waku-style).
                            appears_transparent: true,
                            traffic_light_position: Some(point(px(12.), px(13.))),
                        }),
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

                        app
                    },
                )
                .unwrap();

            cx.activate(true);
            cx.on_action(|_: &Quit, cx| cx.quit());
        });
}
