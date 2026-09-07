# AGENT.md — Working on Orbit

Project guidance for agents and humans. Read this before touching the repo.

## What this project is

**Orbit** is a native desktop workbench for the [pi coding agent](https://github.com/earendil-works/pi) —
a chat-style GUI rendered **entirely in Rust** on **GPUI** (Zed's GPU-accelerated UI framework),
speaking the **pi CLI's RPC protocol** directly over stdio.

The reference architecture is [Waku](https://github.com/egoist/waku): native Rust UI on the GPU,
pi as a child process, no web, no webview, no Node daemon.

**Product docs:** `PRODUCT.md` (positioning, capabilities, constraints) · `README.md` (status, setup) ·
`INTENT.md` (decisions and rationale).

## Hard rules

1. **No web anywhere in the UI.** The React/Vite/Tauri stack has been **removed from the repo** — do not reintroduce React, webviews, DOM, Tailwind, or Node tooling. New UI code is GPUI only.
2. **The pi CLI is the only agent runtime.** We spawn `pi --mode rpc` as a child process per open session and speak newline-delimited JSON over stdio (`crates/orbit-rpc`). There is no Node daemon anymore.
3. **GPUI is pre-1.0 and pinned.** `crates/orbit-pi` uses `gpui = { version = "0.2.2", features = ["runtime_shaders"] }`. Upgrade deliberately on a schedule, never track `main`.
4. **Performance is a product requirement.** The transcript is virtualized (`list()`), stream commits are coalesced (≤ ~8.3 Hz), and syntax highlighting is paint-only so streaming never reflows. Never block a frame with I/O.
5. **Trust pi's truth.** Render real RPC events and on-disk state; nothing decorative pretending to be functional. Access modes the protocol can't deliver are shown as unavailable, not faked.

## Current state

The repo is **pure Rust** — the v0.1 web stack (React, Vite, Tauri, Node daemon) was removed. App icons for future packaging live in `assets/icons/`.

| Component | Location | Status |
|---|---|---|
| GPUI app | `crates/orbit-pi/` | **real app** — sessions sidebar (grouped by project, from `~/.pi/agent/sessions`), live transcript (get_messages + streaming deltas), composer with text input, model-selector picker popup (search + keyboard nav) over the pi model/thinking catalogs; no mock data |
| pi RPC client | `crates/orbit-rpc/` | **working** — process spawn, JSONL reader/writer, response correlation; 5 unit + 3 live integration tests |
| Transport | — | **mostly proven** — `get_state` round-trip + live prompt (`agent_start → text → agent_settled`) pass against real pi; per-tool *approval* events remain to probe |

## GPUI 0.2.2 API notes (learned on this repo — save re-deriving them)

These are the shapes that compiled and ran against the pinned version. When in doubt, check
`~/.cargo/registry/src/.../gpui-0.2.2/` (source is the docs) and `examples/` inside it.

- **Bootstrap:**
  ```rust
  Application::new().run(|cx: &mut App| {
      let bounds = Bounds::centered(None, size(px(1200.), px(800.)), cx);
      cx.open_window(WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)),
          titlebar: Some(TitlebarOptions { title: Some(SharedString::from("…")), ..Default::default() }),
          focus: true, ..Default::default() },
          |window, cx| { /* build root Entity */ }).unwrap();
      cx.activate(true);
      cx.on_action(|_: &Quit, cx| cx.quit());
      cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);
  });
  ```
  `open_window`'s callback is `FnOnce(&mut Window, &mut App) -> Entity<V>`; create entities with `cx.new(|cx| …)`.
- **Virtualized chat list** (the transcript backbone):
  - `ListState::new(count, ListAlignment::Bottom, overdraw_px)` — `Bottom` aligns like a chat log.
  - `list(state.clone(), move |ix, window, cx| … .into_any_element())` builds the element; the closure is `'static`, so capture `Rc<RefCell<_>>` shared state, never `&self`.
  - Appends: `state.splice(old_len..old_len, n)`. Stick-to-latest: `state.scroll_to(ListOffset { item_ix: len, offset_in_item: px(0.) })`.
  - Pin tracking: `set_scroll_handler(|ev: &ListScrollEvent, _, _| pinned.set(!ev.is_scrolled))` — for `Bottom` alignment, `is_scrolled` is exactly "not at bottom", and it only fires for real user scrolls, not programmatic `scroll_to`.
- **Async / streaming:** `window.spawn(cx, async move |cx: &mut AsyncWindowContext| { loop { Timer::after(dur).await; view.update(cx, |this, cx| …).ok() } }).detach()` — `AsyncWindowContext` derefs to `AsyncApp` and implements `AppContext`, so `Entity::update` works across awaits. `pub use smol::Timer` at the crate root.
- **Rendering:** `impl Render { fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement }`.
- **Events:** handlers are `cx.listener(Self::method)`; the listener bound is `Fn(&Event, &mut Window, &mut App)` (not `Context<T>`). `.hover(|s| …)` for hover styles.
- **Multi-line text:** there is **no `WhiteSpace::PreWrap`** in 0.2.2. Render code blocks as one `div()` per line (e.g. `div().h(px(19.)).child(line)`).
- **Any-elements:** `.into_any()` is on `Element` (concrete types); on an opaque `impl IntoElement` return use `.into_any_element()`.
- **`list()` needs a definite height:** its default `ListSizingBehavior` is **`Auto`**, which gives the element *no intrinsic height*. As a plain block child (e.g. inside a non-flex sidebar column) it lays out at height 0 → `prepaint_items` clears its item list → **nothing paints** (while request_layout's overdraw probe still renders ~3 items — the telltale symptom). Fix: `.h_full()` on the list, or put it in a flex container where it stretches (why the transcript's list worked: its parent is a flex row). Never wrap a `list()` in an outer `overflow_y_scroll()` div — the list scrolls itself.
- **Key-binding precedence:** bindings are ordered by dispatch-tree depth (deepest/focused node first); ties at the same depth break by registration order (later wins). Bindings with no context count as the deepest depth. So a popup can override the composer's `enter → Submit`: give the focused input *both* context flags (`.key_context("Composer Picker")`) and register the popup's `enter → Confirm` binding **after** the composer's.
- **Popovers:** `anchored().anchor(Corner::…).snap_to_window()` + `deferred(...)` paints a floating popup; `on_mouse_down_out` on the popup root dismisses on outside clicks. Layered `BoxShadow`es (`vec![contact, ambient]`) read as a real modal surface.
- **Picker lists:** `uniform_list(id, count, …).track_scroll(UniformListScrollHandle::default())` requires **uniform item heights** — render headers as same-height rows. `scroll_to_item(ix, ScrollStrategy::…)` is non-strict (only scrolls when out of view). `.on_hover(&bool)` / `.on_click` live on *stateful* elements (`.id(...)` required).
- **Fonts:** `font_family("Menlo")` resolves via font-kit on macOS; embed app fonts (Commit Mono, Iosevka) as assets.

## Pi CLI RPC protocol (official spec)

- Start: `pi --mode rpc [--provider …] [--model …] [--name …]` — see `packages/coding-agent/docs/rpc.md` in the pi repo.
- **Framing is strict JSONL, LF only.** Split on `\n`; strip a trailing `\r`; never use a reader that splits on `U+2028/U+2029`.
- Commands → stdin (JSON lines, optional `id` for correlation): `prompt` (incl. base64 `images`), `steer`, `abort`, `clear_queue`, `new_session`, `switch_session`, `get_state`, `get_messages`, `get_entries`, `get_tree`, `fork`, `clone`, `set_session_name`, `get_available_models`, `set_model`, `get_available_thinking_levels`, `set_thinking_level`, `get_commands`, `bash`…
- Events → stdout JSON lines: `agent_start/end/settled`, `turn_start/end`, `message_start/update/end` (`assistantMessageEvent`: `text_delta`, `thinking_delta`, `toolcall_start/delta/end`), `tool_execution_*`, `bash_execution_update`, `queue_update`, `compaction_*`, `auto_retry_*`, `extension_ui_request`…
- **User interaction (question dialogs, approvals):** extension UI sub-protocol — `extension_ui_request` (`select`/`confirm`/`input`/`editor`; also fire-and-forget `notify`/`setStatus`/`setWidget`/`setTitle`) on stdout, answer with `extension_ui_response` on stdin with matching `id`.
- **Open item (P0 probe):** whether per-tool *permission* requests surface in RPC mode (pi's permission system is TUI/launch-flag territory today; Waku sidesteps with `--approve`). If they don't, launch with `--approve` and implement the permission UI when the protocol supports it.

## Plan (P0–P6) — see `INTENT.md` for decisions, rationale, estimates, and phase gates

### P0 — Transport probe + foundation (done)
1. ✅ Probe: RPC framing + `get_state` verified against real pi; live prompt streaming verified.
2. ✅ Root `Cargo.toml` workspace.
3. ✅ pi process spawner: `crates/orbit-rpc/src/client.rs` (per-session process, kill-on-drop, stderr ring).
4. ✅ Rust RPC client: serde types per the spec + JSONL reader/writer threads.
5. ✅ First live round-trip rendered in the spike window (`live.rs`).
6. ⬜ Residual: trigger a tool permission/approval in RPC mode and record whether it surfaces as an event (or confirm `--approve` fallback).

### P1 — Shell & theme (5–8 days)
- Window, top bar, macOS traffic lights — GPUI native: `TitlebarOptions.traffic_light_position` (no objc2 needed; the Tauri objc2 shim is gone with the web stack).
- Theme: Orbit tokens (zinc palette) → GPUI `Theme`; dark/light, accent, font, density.
- Fonts embedded (Commit Mono, Iosevka).
- Sidebar shell + sessions grouped by project (reads `~/.pi/agent/sessions`).
- Keybindings, focus traversal, native dialogs.

### P2 — Data layer (8–12 days)
- serde models for all RPC commands/events; event normalization port.
- Sessions: list, open, switch, new, fork, rename, running detection.
- Catalog: `get_available_models`, thinking levels.
- Workbench readers: skills, plugins, usage history, settings (on-disk).
- Providers + scoped-models file I/O (replaces daemon endpoints).

### P3 — Core chat (25–40 days) — the bulk
- Transcript: virtualized list, stick-to-latest, stream veil (Waku `md/veil.rs` pattern).
- Streaming pipeline: coalesce deltas ≤ ~8.3 Hz commits.
- Markdown: pulldown-cmark, one `StyledText` per block, highlight-as-paint.
- Composer: multi-line editor, file attach → base64 images, suggestions, mentions.
- Tool renderers (13): bash, edit, todo, plan, search, mcp, thinking, question + approval dialogs, error states.
- Diff viewer + file-change blocks; image lightbox; shimmer.

### P4 — Workbench (12–20 days)
- Usage charts (custom GPU painting), skills, plugins, models, providers CRUD, settings persistence.

### P5 — Mermaid + accessibility (5–18 days)
- Mermaid: code-block fallback first (never fake a diagram); evaluate graphviz DOT and/or native WKWebView-snapshot later (D2).
- Keyboard operability, visible focus, system reduce-motion.

### P6 — Test & ship (8–13 days)
- Port remaining unit tests: event normalization edge cases, streaming state machine.
- Perf harness: 10k-message scroll, ~1MB streaming markdown, memory profile.
- Packaging: `.app` bundle (icons already in `assets/icons/`), notarization script, CI.
- **Gate:** feature parity per the contract in `INTENT.md`.

## Waku reference file map (pattern reuse, not copy — their gpui is a fork)

| Need | Waku file(s) |
|---|---|
| Transcript + streaming + veil | `src/app/transcript_view.rs`, `src/app/streaming.rs`, `src/md/veil.rs` |
| Markdown renderer / highlight | `src/md/render.rs`, `src/md/highlight.rs`, `src/md/parser.rs` |
| pi RPC transport (adapt to raw protocol) | `crates/waku-core/src/driver/pi.rs` |
| Theme/chrome/sidebar/sessions | `src/theme.rs`, `src/app/window_chrome.rs`, `src/app/sidebar.rs`, `src/app/sessions.rs` |
| Usage/charts | `src/app/usage_page.rs`, `src/app/usage_meter.rs` |

## Definition of done (whole migration)

Every feature in `PRODUCT.md` → `Capabilities` runs natively in the GPUI app against the pi CLI with the same behavior as the legacy UI; legacy tracks deleted; `cargo run` is the only way to launch.

## Verification

- `cargo build --workspace` must stay clean (zero warnings) after every change.
- `cargo test --workspace` — unit tests + live pi integration tests must pass (they spawn a real `pi --mode rpc` process).
- Run the app: `cargo run -p orbit-pi` — check: 10k scroll smoothness on the mock tab; Connect → get_state → prompt «OK» round-trip on the live tab.
- No `unsafe` without a comment; no new dependencies without a stated reason.
