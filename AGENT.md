# AGENT.md — Working on Orbit

Project guidance for agents and humans. Read this before touching the repo.

When this file conflicts with `INTENT.md` on architecture choices, **`INTENT.md` wins**.
Update `INTENT.md` if a recorded decision (D1–D6) changes.

## What this project is

**Orbit** is a native desktop workbench for the [pi coding agent](https://github.com/earendil-works/pi) —
a chat-style GUI rendered **entirely in Rust** on **GPUI** (Zed's GPU-accelerated UI framework),
speaking the **pi CLI's RPC protocol** directly over stdio.

The reference architecture is [Waku](https://github.com/egoist/waku): native Rust UI on the GPU,
pi as a child process, no web, no webview, no Node daemon.

**Product docs:** `PRODUCT.md` (positioning, capabilities, constraints) · `README.md` (status, setup) ·
`INTENT.md` (decisions and rationale).

## Hard rules

1. **No web anywhere in the UI.** Do not reintroduce React, Vite, Tauri, webviews, DOM, Tailwind, or Node tooling. New UI is GPUI only. The legacy `src-tauri/` tree has been deleted; never rebuild it.
2. **The pi CLI is the only agent runtime.** Spawn `pi --mode rpc` as a child process per open session and speak newline-delimited JSON over stdio (`crates/orbit-rpc`). There is no Node daemon.
3. **GPUI is pre-1.0 and pinned.** `crates/orbit-pi` uses `gpui = { version = "0.2.2", features = ["runtime_shaders"] }`. Upgrade deliberately on a schedule, never track `main`.
4. **Performance is a product requirement.** The transcript and sidebar are virtualized (`list()`). The UI drains RPC events on a ~90 ms heartbeat — never block a frame with I/O. Syntax highlighting, when it lands, must be paint-only so streaming never reflows.
5. **Trust pi's truth.** Render real RPC events and on-disk state; nothing decorative pretending to be functional. Access modes the protocol can't deliver are shown as unavailable (or as a static fact), not faked. The sidebar **Search** row opens the command palette (sessions/commands/settings) — it is not a transcript-content search; do not wire a fake one.

## Repo layout

```
Cargo.toml              workspace: orbit-pi, orbit-rpc
crates/orbit-pi/        GPUI app — window, shell, chat, settings
  src/main.rs           bootstrap, assets, keybindings, heartbeat
  src/app.rs            OrbitApp state model + shared types + controller wiring (module map in the file header)
  src/app/runtime.rs    pi process lifecycle, status/error, provider auth
  src/app/events.rs     heartbeat event drain, RPC response routing, session/workspace watchers
  src/app/session.rs    prompt/queue/turn lifecycle, abort, session switching + navigation
  src/app/pickers.rs    model, command-palette, branch, and new-task workspace pickers
  src/app/composer_ops.rs autocomplete, attachments, "+" add-menu, model/thinking chips
  src/app/sidebar.rs    sidebar rows + session/workspace row menus; hidden-workspace prefs
  src/app/settings.rs   Settings surface: General/Runtime/Agent/Appearance/Providers (+ provider CRUD)
  src/app/view.rs       top-level chrome: sidebar, transcript, composer, status bar, onboarding
  src/app/open_in.rs    installed-editor/terminal detection and the "open in" menu
  src/app/helpers.rs    icon/file-glyph UI kit + small formatting helpers
  src/auth.rs           non-sensitive provider-auth state machine for the auth.* RPC namespace (login/cancel/timeout/restart recovery)
  src/transcript.rs     virtualized messages (snapshot + stream)
  src/transcript_view.rs Waku-style transcript paint (rail, activity cards, copy); plain-GPUI message rows (no component library)
  src/message_scroller.rs tail-following list + jump-to-latest (original, plain gpui ListState)
  src/composer.rs       multi-line EntityInputHandler (wraps, auto-grows, scrolls)
  src/model_selector.rs model / thinking picker popover
  src/command_palette.rs  ⌘P command palette (sections, fuzzy match, modal scrim layer)
  src/workspace_picker.rs new-task folder selector (recent folders + native browse)
  src/sessions.rs       reads ~/.pi/agent/sessions (+ debounced session-store watcher)
  src/watch.rs          shared debounced fs watching (workspace tree for Review/Git)
  src/assets.rs         include_dir AssetSource (SVGs + app icon)
  src/app_icon.rs       dock icon (macOS) + Settings → About mark
  src/review.rs         git diff model: sources, parsing, context gaps, changed-files tree
  src/checkpoint.rs     per-turn git snapshot refs (refs/orbit/…) backing Review's Last Turn
  src/highlight.rs      paint-only syntax lexer for diff lines (Waku port, 16 languages)
  src/sidepane.rs       Review pane: virtualized diff, sticky file headers, tree, source menu
  src/git.rs            git plumbing: branch discovery, review diffs, status/staging/history/graph
  src/git_panel.rs      full-page Git surface: Changes / History / Graph + commit bar
  src/providers.rs      provider catalog + models.json / auth.json read/write for Settings → Providers (provider metadata introspected from pi-ai; curated table is the fallback)
  src/commit_message.rs one-shot, tool-free `pi -p` conventional-commit generation
  assets/icons/         HugeIcons SVGs (MIT) + provider brand marks
  assets/fonts/         SymbolsNerdFont-Regular.ttf
  assets/app-icon.png   512px app mark (from the 1024 macOS source)
crates/orbit-rpc/       pi CLI process + JSONL protocol
  src/client.rs         spawn, writer/reader/stderr threads, kill-on-drop
  src/types.rs          CommandBody / Event (permissive serde) + auth.* wire types
  docs/auth-rpc.md      provider-auth RPC server contract (secrets never cross it)
  tests/live_pi.rs      live get_state / prompt / process-alive (skip if no pi)
  tests/live_catalog.rs live get_available_models + thinking levels
  tests/live_auth.rs    live auth.* capability probe (skips on stock pi)
  tests/auth_fake_pi.rs scripted-server coverage of the auth.* round trip
contrib/pi-auth-rpc/    apply.mjs: inject auth.* into an installed pi build
                        (browser OAuth until pi ships the server upstream)
assets/icons/           app icon source (1024 PNG) + icon.icns / icon.png
PRODUCT.md  INTENT.md  README.md  AGENT.md
```

Workspace edition is **2021**. Prefer **Rust 1.94+**. Root `Cargo.lock` is the lockfile; ignore a nested `crates/orbit-pi/Cargo.lock` if present.

Override the pi binary with `PI_BIN` (default: `pi` on `PATH`).

## Current state

The product is the GPUI app (`cargo run -p orbit-pi`). The v0.1 React/Vite/Tauri stack is gone from git history as of the native rewrite; do not resurrect it.

| Area | Location | What is real today |
|---|---|---|
| Shell | `app.rs` | Transparent titlebar, traffic lights inside the sidebar, sessions sidebar grouped by workspace (collapsible), top-bar back/forward + title + edit +/− counts, floating composer, status bar (workspace / Local / git branch from `.git/HEAD`) |
| Sessions | `sessions.rs` | Reads `~/.pi/agent/sessions/<slug>/*.jsonl`; title = first user message; header-only files (fresh `new_session`, nothing sent yet) are drafts and **not listed** until the first user message lands (Waku drafts parity); `cmd-n` new, click to `switch_session` + `get_messages` |
| Transcript | `transcript.rs` + `transcript_view.rs` + `message_scroller.rs` | Virtualized `list()` (content column max 960px) with MessageScroller tail-follow: append/remeasure without blank rows, **Jump to latest** when you scroll up (a **New activity** capsule when content landed while away). Rows use Message Start/End slots (avatar + content + Copy footer). Waku turn order: **Worked for** fold → answer → files-changed card → **Copy** (+ completion time when pi provides one); live turns keep a **Working** activity cluster and close with an activity-aware indicator (**Running cargo test · 12s**, **Reading src/auth.rs**, **Thinking…**). Left navigation ticks jump to user turns and show a hover preview card (turn number, prompt + response snippet); a one-time dismissable hint (`~/.orbit-pi/hints.json`) teaches the rail on first appearance; `cmd-shift-c` copies the latest response and `cmd-up`/`cmd-down` mirror the rail's jumps. Message footers (copy + timestamp) are persistent, not hover-gated. Markdown blocks parse once per content change (thread-local memo), so streaming never re-parses settled rows. Body prose uses a 14/26 line measure so wrapped lines and inline-code washes do not collide. Code blocks carry a language chip from the fence info string; settled blocks past 28 lines fold to a 24-line preview (**Show remaining N lines**), and copy always yields the full code. Tool rows expand into detail cards with Arguments/Output sections (Output streams live from accumulated `tool_execution_update` partials and is normalized out of pi's `{content:[…]}` envelope at `tool_execution_end`) and per-section copy buttons; sections past 16 lines collapse to a 12-line preview, expanded paint caps at 400 lines (copy stays complete); failed tools show a drawn stop glyph and their first error line inline; a turn the provider/agent rejected renders pi's `errorMessage` as an inline **Agent error** card; a cancelled turn keeps its partial answer under a quiet **Stopped** marker. Wide tables scroll horizontally instead of crushing columns. A status strip above the composer carries **Retrying — attempt N of M** (with Cancel → `abort_retry`) and **Preparing conversation context…** while compacting; the error banner has Copy and, once the process exits, Reconnect. Counts from edit/write args, not git; no fork |
| Composer | `composer.rs` + `mentions.rs` | **Multi-line** input (wraps, grows to 8 rows, then scrolls). Enter submits — while the agent is running as a **follow-up** (`follow_up`), queued and delivered once the current task finishes; as a `prompt` otherwise; `shift-enter` newline, `cmd-enter` submits. A sticky queue bar above the composer mirrors pi's `queue_update` (each pending message labelled, with a **Clear** action) until it is delivered; `escape` sends `clear_queue` (restoring the dropped text to the composer) before `abort`. `/`-command + `@`-file autocomplete, image attachments (paste / drop / "+" menu → Attach image) that ride `prompt` and `follow_up` alike, non-image files referenced by path at the caret, "+" add menu (keyboard-driven, `AddMenu` context). Failed sends keep the prompt and attachments |
| Side pane | `sidepane.rs` + `review.rs` + `checkpoint.rs` + `git.rs` + `highlight.rs` | Right panel (top-bar `panel-right` toggle, the top-bar `+N -M` diff-stat chip opens it on **Uncommitted**, drag-resizable left edge) showing **Review**: a source dropdown grouped like Waku — `Last Turn` (per-turn git snapshot refs under `refs/orbit/…`, captured at prompt start and `agent_settled` by `checkpoint.rs`; branch-switch-safe via a merge-tree diff base), then `Uncommitted` (`git diff <head>` + untracked via an isolated worktree commit), `Unstaged` (`<index tree>` → worktree), `Staged` (`<head>` → index tree), `Committed`/`Branch` (merge-base against `main`/`master`). `git.rs` captures `--numstat` + a full-context patch (falls back to `-U3` past the 32 MB cap) off-thread; `review.rs` parses it into a virtualized row list (file headers, hunk separators, tinted ±rows with a single new/old line-number gutter, deleted/binary handling, expandable context gaps) with a sticky file header and devicon marks; `highlight.rs` colors code-line tokens. A filterable changed-files tree (Waku indentation `7+depth*14` dirs / `23+depth*14` files, `A/M/D/B` badges, collapsible dirs, click-to-jump, keyboard cursor) sits alongside and auto-hides on narrow panes (tree ≥440 px, stats ≥380 px). Reloads on open, workspace/session change, source change, and turn settle. The transcript's changed-files cards' **Review** buttons open the pane on the latest **Last Turn** checkpoint diff via a `ReviewOpener` callback threaded from the app |
| Git page | `git_panel.rs` + `git.rs` + `commit_message.rs` | Full main-area surface (opened from the session-details **Commit or push** row; `escape`/Back returns) with three tabs. **Changes**: staged/unstaged lists from `git status --porcelain` with per-file stage/unstage and confirmed discard, an Include-unstaged toggle, and a commit bar (branch dropdown, message input, **Generate**). **History**: `git log` rows with ref badges, author, relative time, paged. **Graph**: an all-branches lane graph (`git log --all --date-order` + a client-side lane pass) with colored lanes, nodes, and merge links. The action cluster follows Git state: uncommitted changes → **Commit** / **Commit and push**; clean with unpushed commits → **Push** (**Publish branch** when there is no upstream); clean and behind → **Pull**; otherwise a quiet "Up to date". Push/pull never touch the message. A blank Commit message auto-generates. When unstaged changes exist (and Include-unstaged is off) Generate first shows a confirm popup — **Stage all and generate** / **Generate from staged** / Cancel. **Generate** runs `pi -p --no-tools --no-session --no-extensions --no-skills --no-context-files` (`commit_message.rs`) so no tool ever runs and the active session is untouched, falling back to a local `type(scope): …` heuristic. Clicking a changed file opens its diff in the Review pane (`SidePane::show_file`). Local branches checkout from the branch menu; new-branch creation is not wired yet |
| Catalog | `model_selector.rs` | Searchable popovers for `get_available_models` / `get_available_thinking_levels`; `set_model` / `set_thinking_level`. Two chips + a **static** "Full access" pill (not a control) |
| Settings | `app.rs` + `providers.rs` + `auth.rs` | In-app surface (`cmd-,`): General / Runtime / Agent / Appearance / Providers / About. Read-only facts from the live process + a real sidebar toggle; About shows the app icon. **Agent** controls pi's live behavior over RPC: follow-up delivery mode (`set_follow_up_mode`), auto-compaction and auto-retry toggles, **Compact now** (`compact`), an **Abort retry** action while `auto_retry_*` is in flight (`abort_retry`), and session rename (`set_session_name`). **Providers** lists every built-in pi provider individually (metadata introspected from pi-ai, curated table as fallback) plus models.json endpoints, with real status, API-key entry (`auth.json`, 0600), models.json overrides for custom providers, Sign out, Remove, search, and Refresh. Provider OAuth is a **first-class RPC capability**: `auth.list` discovers providers/methods (no hardcoded ids), `auth.login`/`auth.logout`/`auth.cancel` drive a login session id, and asynchronous `auth.*` events render Connect / Connecting / Device code / Success / Error / Disconnect / Cancel. Browser URLs open via the OS; tokens never cross RPC or touch GPUI state. A pi without `auth.*` falls back to the legacy `pi /login` Terminal path and the credentials-changed restart banner; run `node contrib/pi-auth-rpc/apply.mjs` once to add the server side to an installed pi and get browser OAuth |
| RPC | `orbit-rpc` | Spawn (`--mode rpc --approve`, `PI_SKIP_VERSION_CHECK=1` — Waku parity), JSONL I/O, response correlation, typed-enough events incl. `steer`/`follow_up` + the queue modes (`set_steering_mode`/`set_follow_up_mode`), `compact`/`set_auto_compaction`/`set_auto_retry`/`abort_retry`, `set_session_name`, fork family (`get_fork_messages`/`fork`/`clone`), `session_info_changed` (live auto-title), `queue_update` (`PendingQueue`), `auto_retry_*`, and the typed `auth.*` namespace (`Event::Auth` + `AuthProvider`/`AuthErrorCode`; contract in `docs/auth-rpc.md`). `SessionState` parses the `get_state` agent-control fields. UI does **not** yet answer `extension_ui_request` and has no rewind UI for fork |
| Stubs | `app.rs` | No transcript-content search. No branch-create UI yet. No workbench pages beyond the Git page |

**Not started (parity backlog):** full markdown (tables, highlight), rich tool/approval UI, fork rewind UI, mermaid, density persistence, packaging/CI.

## How the app runs

1. `OrbitApp::new` spawns `PiClient::spawn(cwd, None)` so sessions land in pi's default store (`~/.pi/agent/sessions/`), shared with the CLI. Tests pass `Some("/tmp/…")` via `--session-dir`.
2. On connect it sends `get_state`, `get_available_models`, `get_available_thinking_levels`.
3. `main.rs` starts a window `spawn` loop: every **90 ms** call `OrbitApp::tick` → `PiClient::drain_events` (`try_recv`, never blocking).
4. Transcript data has two inlets into the same model: `get_messages` rebuilds; `message_start` / `message_update` / `message_end` mutate live.
5. One `PiClient` = one child. Drop kills the process. `ProcessExited` is surfaced in the status line.

## GPUI 0.2.2 API notes (learned on this repo — save re-deriving them)

> **Reference: Zed.** Use Zed source code as a reference when a task concerns GPUI implementation — layout and styling idioms, focus and key dispatch, virtualized lists, menus and popovers, window and platform behavior — or when an in-house `src/ui` primitive needs a proven native precedent. Zed is the canonical GPUI codebase; read its crates rather than `gpui-component`, and read the gpui revision pinned in `Cargo.toml` so the APIs match what Waku builds against.

These compiled and ran against the pinned version. When in doubt, check
`~/.cargo/registry/src/.../gpui-0.2.2/` (source is the docs) and `examples/` inside it.

- **Bootstrap** (`main.rs`):
  ```rust
  Application::new().with_assets(assets::Assets).run(|cx: &mut App| {
      cx.text_system().add_fonts(vec![/* Cow<[u8]> font bytes */]).unwrap();
      let bounds = Bounds::centered(None, size(px(w), px(h)), cx);
      cx.open_window(WindowOptions {
          window_bounds: Some(WindowBounds::Windowed(bounds)),
          titlebar: Some(TitlebarOptions {
              title: Some(SharedString::from("Orbit Pi")),
              appears_transparent: true,
              traffic_light_position: Some(point(px(12.), px(13.))),
              ..Default::default()
          }),
          focus: true, ..Default::default()
      }, |window, cx| { /* Entity<OrbitApp> */ }).unwrap();
      cx.activate(true);
  });
  ```
  Size the window from `cx.primary_display()` (a hardcoded 1240×840 gets clamped top-left on small/scaled screens). `open_window`'s callback is `FnOnce(&mut Window, &mut App) -> Entity<V>`; create entities with `cx.new(|cx| …)`.
- **App icon:** source is `assets/icons/Icon-macOS-Default-1024x1024@1x.png`. `icon.icns` is for bundled `.app`s (`package.metadata.bundle`). `cargo run` has no bundle, so `app_icon::set_dock_icon()` calls AppKit `setApplicationIconImage` with the embedded 512 PNG. Settings → About paints the same PNG via `img("app-icon.png")`.
- **macOS chrome:** `appears_transparent` + `traffic_light_position` — no objc2. Drag regions: `.window_control_area(WindowControlArea::Drag)` on the sidebar strip and the top-bar spacer. When the sidebar is hidden, pad the top bar so controls clear the traffic lights (`pl` ≈ 76 px).
- **Virtualized lists** (transcript **and** sidebar):
  - Transcript uses `message_scroller.rs` (`ListAlignment::Bottom`, 400px overdraw) — 0.2.2 has no `FollowMode::Tail`, so a past-the-end `scroll_to` is the equivalent. Append while following sticks to the live edge; `remeasure_items` grows the streaming row; scroll away shows **Jump to latest**.
  - Rows are plain GPUI flex trees in `transcript_view.rs` (the `Message` component port was removed — no component library anywhere): Start/End alignment, avatar disc, content column, footer. The footer sits outside the avatar row so the 32px disc stays flush with content; the footer is indented `avatar + row gap` to line up with the content column.
  - Sidebar: `ListState::new(count, ListAlignment::Top, overdraw_px)`.
  - `list(state.clone(), move |ix, window, cx| … .into_any_element())` — the closure is `'static`, so capture `Rc<RefCell<_>>` / `Rc<Vec<_>>`, never `&self`.
  - Stick-to-latest: `scroll_to(ListOffset { item_ix: len, offset_in_item: px(0.) })` (past-the-end).
  - Pin tracking: `set_scroll_handler` — for `Bottom` alignment, `is_scrolled` is "not at bottom", and it only fires for real user scrolls, not programmatic `scroll_to`.
- **`list()` needs a definite height:** default `ListSizingBehavior` is **`Auto`** (no intrinsic height). As a plain block child it lays out at height 0 → `prepaint_items` clears its item list → **nothing paints** (while request_layout's overdraw probe still renders ~3 items — the telltale symptom). Fix: `.h_full()` on the list inside a `flex_1` + `min_h_0` parent. Never wrap a `list()` in an outer `overflow_y_scroll()` div — the list scrolls itself.
- **Heartbeat / async:** `window.spawn(cx, async move |cx: &mut AsyncWindowContext| { loop { Timer::after(dur).await; view.update(cx, |this, cx| this.tick(cx)).ok(); } }).detach()`. `AsyncWindowContext` derefs to `AsyncApp` and implements `AppContext`. `pub use smol::Timer` is at the gpui crate root.
- **Rendering:** `impl Render { fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement }`.
- **Events:** handlers are `cx.listener(Self::method)`; the listener bound is `Fn(&Event, &mut Window, &mut App)` (not `Context<T>`). `.hover(|s| …)` for hover styles.
- **Text wrapping:** `.whitespace_normal()` exists in 0.2.2 and is what the assistant transcript uses. There is still **no `WhiteSpace::PreWrap`**. For preformatted blocks (thinking, code), render **one `div()` per line** with a fixed height (e.g. `div().h(px(17.)).child(line)`).
- **Any-elements:** `.into_any()` is on `Element` (concrete types); on an opaque `impl IntoElement` return use `.into_any_element()`.
- **Custom text input:** there is no multi-line editor widget we use. `ComposerInput` is an `EntityInputHandler` adapted from gpui's `input` example (grapheme-aware caret, clipboard, IME stubbed). Keep it single-line until a deliberate composer upgrade (INTENT.md risk #3).
- **Key-binding precedence:** bindings are ordered by dispatch-tree depth (deepest/focused node first); ties at the same depth break by **registration order (later wins)**. Bindings with no context count as the deepest depth. The picker filter uses `.with_key_context("Composer Picker")` so backspace still edits, and picker `enter`/`escape`/`arrows` are registered **after** composer bindings in `main.rs` so they win over `Submit` / `AbortRun`.
- **Popovers:** `anchored().position_mode(AnchoredPositionMode::Local).anchor(Corner::BottomLeft).offset(…).snap_to_window()` + `deferred(entity)` paints a floating popup. `on_mouse_down_out` dismisses. **Click-through:** the same click's mouse-up would re-open the chip — `OrbitApp` swallows toggles for ~200 ms after an outside mouse-down dismiss (`menu_dismissed_at`). Layered `BoxShadow`es (`vec![contact, ambient]`) read as a real modal surface.
- **Picker lists:** **do not use `uniform_list` inside a `deferred` + `anchored` popover** — measured layout can collapse to height 0. Use `max_h` + `overflow_y_scroll` + `ScrollHandle` (Zed `ContextMenu` shape). Keyboard nav must `set_offset` to keep the highlighted row in view. `.on_hover` / `.on_click` live on *stateful* elements (`.id(...)` required).
- **Assets:** implement `gpui::AssetSource` over `include_dir!` so packaged builds don't depend on cwd. SVG icons: `img("icons/….svg")` via helpers in `app.rs`. Provider marks live under `assets/icons/providers/`.
- **Fonts:** `cx.text_system().add_fonts` for embedded `Symbols Nerd Font` (devicons). `font_family("Menlo")` resolves via font-kit on macOS for tool rows. Commit Mono / Iosevka are **not** in the tree yet (P1 leftover).

## Keybindings (current)

| Keys | Action | Context |
|---|---|---|
| `cmd-q` | Quit | global |
| `cmd-n` | New session | global |
| `cmd-r` | Reload session list from disk | global |
| `cmd-,` | Settings | global |
| `cmd-p` | Command palette (sessions / commands / settings) | global |
| `cmd-shift-c` | Copy the newest assistant response (footer copy mirror) | global |
| `cmd-up` / `cmd-down` | Jump to previous / next user turn (rail mirror) | global |
| `escape` / `cmd-.` | Close settings → close picker → `abort` | global / Composer |
| `enter` / `cmd-enter` | Submit prompt | `Composer` |
| `shift-enter` | Newline | `Composer` |
| `enter` / `escape` / `up` / `down` | Confirm / cancel / move | `Picker` (registered after Composer) |
| `enter` / `escape` / `up` / `down` | Run / close / move | `AddMenu` (composer "+" menu; registered after Picker) |

## Pi CLI RPC protocol (official spec)

- Start: `pi --mode rpc [--provider …] [--model …] [--name …]` — see `packages/coding-agent/docs/rpc.md` in the pi repo.
- **Framing is strict JSONL, LF only.** Split on `\n`; strip a trailing `\r`; never use a reader that splits on `U+2028/U+2029`. `PiClient` uses `read_until(b'\n')`.
- **Typed in `CommandBody` today:** `prompt` (with `images` + `streamingBehavior`), `abort`, `clear_queue`, `steer`/`follow_up` (with `images`), `set_steering_mode`/`set_follow_up_mode`, `compact`, `set_auto_compaction`/`set_auto_retry`/`abort_retry`, `set_session_name`, `new_session`, `switch_session`, `get_state`, `get_messages`, `get_available_models`, `set_model`, `cycle_model`, `get_available_thinking_levels`, `set_thinking_level`, `cycle_thinking_level`, `get_commands`, `get_session_stats`, `get_fork_messages`, `fork`, `clone`, `auth.list`, `auth.status`, `auth.login`, `auth.logout`, `auth.cancel`, plus `Raw(Value)` for anything else.
- **Used by the UI today:** `prompt`/`follow_up` (text + images), `abort`, `clear_queue`, `set_follow_up_mode`, `compact`, `set_auto_compaction`/`set_auto_retry`/`abort_retry`, `set_session_name`, `new_session`, `switch_session`, `get_state`, `get_messages`, `get_available_models`, `set_model`, `get_available_thinking_levels`, `set_thinking_level`, `get_session_stats`, and the `auth.*` family when the running pi advertises it (falls back to file/Terminal login otherwise). `SessionState` reads `steeringMode`/`followUpMode`/`autoCompactionEnabled`/`sessionName`/`isCompacting`/`pendingMessageCount` from `get_state`; `PendingQueue` mirrors `queue_update`. (`steer` is typed but not sent: while running, the composer always queues a follow-up.)
- **Not typed / not wired yet:** `get_entries`, `get_tree`, `bash`/`abort_bash`, `export_html`, `get_last_assistant_text`, and `new_session.parentSession`.
- **Error handling:** a `response` with `success: false` carries an `error` string. The app never swallows it: `on_command_failure` surfaces it in a dismissible error banner (chat and settings) and re-reads `get_state` to reconcile optimistic UI after `set_model`/thinking/rename failures. A `parse` command, `extension_error` events, failed compaction (`compaction_end.errorMessage`), exhausted `auto_retry_end`, and `send` write failures all route to the same banner. A successful retry of the same command clears its banner (`clear_error_for`). **Provider/LLM failures are not responses at all**: pi sets `stopReason: "error"` + `errorMessage` on the final assistant message and emits it via `message_start`/`message_end` (see pi-agent-core `agent-loop.js`). `transcript::message_error` extracts it; the app raises `Agent error: …` and `transcript_view` renders the same text as a red card inline, so a failed turn is never an empty row.
- Events → stdout JSON lines: `agent_start/end/settled`, `turn_start/end`, `message_start/update/end` (`assistantMessageEvent`: `text_delta`, `thinking_delta`, `toolcall_start/delta/end`), `tool_execution_*`, `bash_execution_update`, `queue_update`, `compaction_*`, `auto_retry_*`, `extension_ui_request`, `extension_error`, typed `auth.*` (`Event::Auth`), plus `Unknown` for forward-compat.
- **Provider authentication:** `auth.*` is a first-class RPC namespace. The client reducer is `orbit-pi/src/auth.rs`; the wire types are in `orbit-rpc/src/types.rs`; the server contract (which pi must implement by reusing its provider OAuth + `auth.json`) is `crates/orbit-rpc/docs/auth-rpc.md`. Tokens never cross RPC or GPUI state. **Stock pi 0.85.1 has no server side** — without it Orbit falls back to `pi /login` in Terminal. To get browser OAuth on an installed pi now, run `node contrib/pi-auth-rpc/apply.mjs` (idempotent; `--revert` restores the backup) and restart the agent. `live_auth.rs` skips on an unpatched pi.
- **User interaction:** `extension_ui_request` (`select`/`confirm`/`input`/`editor`; also fire-and-forget `notify`/`setStatus`/`setWidget`/`setTitle`) on stdout; answer with `extension_ui_response` on stdin (`PiClient::respond_dialog`). **The UI currently ignores these events.**
- Envelopes parse **loosely**; unknown shapes flow through as raw JSON so protocol additions don't crash the client.
- **Open item (P0 residual):** whether per-tool *permission* requests surface in RPC mode (pi's permission system is TUI/launch-flag territory; Waku sidesteps with `--approve`). If they don't, launch with `--approve` and implement the permission UI when the protocol supports it. Never fake an approval mode.

## Plan (P0–P6) — see `INTENT.md` for decisions, rationale, estimates, and phase gates

### P0 — Transport probe + foundation (mostly done)
1. ✅ RPC framing + `get_state` against real pi; live prompt streaming (`agent_start → text → agent_settled`).
2. ✅ Root `Cargo.toml` workspace (`orbit-pi`, `orbit-rpc`).
3. ✅ Process spawner: `crates/orbit-rpc/src/client.rs` (per-session process, kill-on-drop, stderr ring).
4. ✅ Rust RPC client: permissive serde types + JSONL reader/writer threads.
5. ✅ First live round-trip in the real window (no more `live.rs` spike / mock tab).
6. ⬜ Residual: trigger a tool permission/approval in RPC mode and record whether it surfaces as an event (or confirm `--approve` fallback). Wire `extension_ui_request` in the UI when that lands.

### P1 — Shell & theme (mostly done; theme persistence still open)
- ✅ Window, top bar, macOS traffic lights (`TitlebarOptions.traffic_light_position`).
- ✅ Dark zinc palette (hardcoded in `app.rs`, Waku-like). ⬜ `Theme` entity, light mode, accent, density, persist to pi settings.
- ✅ Sidebar + sessions grouped by workspace; ⬜ Search.
- ✅ Keybindings / focus for composer + picker; ⬜ broader native menus/dialogs.
- ⬜ Embed Commit Mono / Iosevka (Nerd Font is already embedded for file glyphs).

### P2 — Data layer (in progress)
- ✅ Permissive command/event models; graduate remaining shapes as live dumps arrive.
- ✅ Sessions: list, open, switch, new. ⬜ fork, rename, running detection.
- ✅ Catalog: `get_available_models`, thinking levels.
- ⬜ Workbench readers: skills, plugins, usage history, settings (on-disk).
- ⬜ Providers + scoped-models file I/O (settings Providers is catalog-derived, read-only).

### P3 — Core chat (started; this is the bulk)
- ✅ Virtualized transcript, stick-to-latest. ⬜ stream veil (Waku `md/veil.rs`).
- ✅ Event drain ~11 Hz. ⬜ coalesce paint commits as a dedicated streaming pipeline.
- ⬜ Markdown: pulldown-cmark, one `StyledText` per block, highlight-as-paint. Lightweight headings / bullets / inline code / fenced blocks already paint.
- ✅ Single-line composer. ⬜ multi-line, file attach → base64 images, suggestions, mentions.
- ⬜ Tool renderers (13): bash, edit, todo, plan, search, mcp, thinking, question + approval dialogs.
- ✅ Diff viewer (virtualized rows, sticky headers, gap expansion, highlighting) + per-turn `Last Turn` checkpoints; ⬜ file-change blocks in the transcript; image lightbox.

### P4 — Workbench (started)
- ✅ Git page (Changes / History / Graph + commit bar, one-shot commit-message generation). ✅ Settings → Providers: full built-in catalog, API-key + OAuth auth, models.json CRUD, refresh. ⬜ Usage charts, skills, plugins, settings persistence.

### P5 — Mermaid + accessibility (not started)
- Code-block fallback first (never fake a diagram). Keyboard operability, visible focus, reduce-motion.

### P6 — Test & ship (early)
- Some unit + live tests exist. ⬜ event-normalization edges, streaming state machine, 10k-message perf harness, `.app` bundle / notarization / CI.

## Waku reference file map (pattern reuse, not copy — their gpui is a fork)

| Need | Waku file(s) |
|---|---|
| Transcript + streaming + veil | `src/app/transcript_view.rs`, `src/app/streaming.rs`, `src/md/veil.rs` |
| Markdown renderer / highlight | `src/md/render.rs`, `src/md/highlight.rs`, `src/md/parser.rs` |
| pi RPC transport (adapt to raw protocol) | `crates/waku-core/src/driver/pi.rs` |
| Review diff sources, parsing, sticky headers, changed-files tree | `src/review_diff.rs`, `src/app/right_panel.rs`, `crates/waku-core/src/workspace.rs` (`resolve_diff_range`) |
| Per-turn checkpoints (Last Turn) | `crates/waku-core/src/checkpoint.rs` |
| Diff syntax highlighting | `src/md/highlight.rs` |
| Theme/chrome/sidebar/sessions | `src/theme.rs`, `src/app/window_chrome.rs`, `src/app/sidebar.rs`, `src/app/sessions.rs` |
| Usage/charts | `src/app/usage_page.rs`, `src/app/usage_meter.rs` |

## Definition of done (whole migration)

Every feature in `PRODUCT.md` → `Capabilities` runs natively in the GPUI app against the pi CLI with the same behavior as the legacy UI; legacy `src-tauri` deleted; `cargo run -p orbit-pi` is the only way to launch.

## Verification

- `cargo build --workspace` must stay clean (zero warnings) after every change.
- `cargo test --workspace` — unit tests + live pi integration tests. `live_pi.rs` skips if `pi` is missing; **`live_catalog.rs` currently does not skip** (needs `pi` on `PATH`). `orbit-pi` session tests assume a populated `~/.pi/agent/sessions` on the machine.
- Run the app: `cargo run -p orbit-pi`. Check: sessions list from disk, new session (`cmd-n`), prompt streams into the transcript, model/thinking pickers, abort (`escape`), settings (`cmd-,`), sidebar toggle.
- No `unsafe` without a comment; no new dependencies without a stated reason.
