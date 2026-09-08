# Orbit

**Orbit** is a native desktop workbench for the [pi coding agent](https://github.com/earendil-works/pi) — a chat-style GUI rendered **entirely in Rust** on GPUI, the same GPU-accelerated UI framework Zed is built on. It speaks the pi CLI's native RPC protocol directly over stdio. **No browser, no webview, no Node.js — the repo is pure Rust.**

Same product philosophy as [Waku](https://github.com/egoist/waku): the UI layer is pure native Rust drawing to the GPU, and the agent underneath is pi itself — sessions you create in Orbit and in the terminal are the same sessions (`~/.pi/agent/sessions/`), managed by pi's own session manager.

## Features

- **Chat-style agent sessions** — prompt composer with model selection, thinking-effort control, steer/cancel, and streaming responses
- **GPU-rendered transcript** — virtualized message list; 10k-message sessions scroll at frame rate; stick-to-latest streaming with coalesced commits
- **Sessions grouped by project** — persistent sessions organized per working directory, with reopen support; cross-workspace sessions included
- **Model catalog** — model selection from the pi runtime's own list (`get_available_models`), backed by providers on disk
- **Thinking effort** — level selection derived from each model's supported levels (`get_available_thinking_levels`)
- **Tool activity** — bash, edit, todo, plan, search, mcp, thinking, and question rows plus approval dialogs, all rendered natively; tool rows expand into Arguments/Output detail cards with per-section copy
- **Git diff panel** — review what the agent changed without leaving the app
- **Workbench pages** — usage, skills, plugins, models, providers, and settings views backed by pi's on-disk data
- **Markdown rendering** — GFM and syntax-highlighted code; highlighting is paint-only so streaming code blocks never reflow
- **Theming** — dark/light, accent, font, and density settings persisted to pi's own settings
- **Desktop native** — keyboard operability, native menus/dialogs, custom macOS window chrome

## Architecture

```
┌───────────────────────────────────────────┐
│  Orbit (Rust) — GPUI, Metal-rendered UI   │
│  crates/orbit-pi                        │
│  window, transcript, markdown, workbench  │
└──────────────┬────────────────────────────┘
               │ newline-delimited JSON RPC over stdio
┌──────────────▼────────────────────────────┐
│  pi CLI (child process, per open session) │
│  pi --mode rpc — sessions, models, tools  │
└──────────────┬────────────────────────────┘
               │
┌──────────────▼────────────────────────────┐
│  ~/.pi/agent/ — sessions, config, usage   │
│  (same data the pi CLI uses)              │
└───────────────────────────────────────────┘
```

The app spawns the `pi` CLI as a child process and speaks its RPC protocol: JSON requests on stdin, JSON-line events on stdout (text/thinking deltas, tool calls, question dialogs, settle signals). This is the same integration pattern Waku uses. Everything user-facing is rendered by GPUI on Metal — no DOM, no CSS, no browser engine anywhere.

## Status

The v0.1 web app (React 19 + Vite + Tauri 2 + pi SDK daemon) has been **removed**; the GPUI app is the only app. What works today:

- **Working now** — real pi process integration (`crates/orbit-rpc`), sessions sidebar grouped by project, live streaming transcript over the RPC, composer with enter-to-send, model/thinking cycling, virtualized rendering
- **In progress** — markdown rendering, rich tool renderers, diff panel, workbench pages, theming

Feature work is tracked in `INTENT.md` (decisions + phase plan) and `AGENT.md` (conventions + protocol notes). Live behavior is covered by integration tests that spawn a real pi process.

## Getting Started

### Prerequisites

- [Rust](https://rustup.rs/) (1.94+ preferred)
- Xcode command line tools (Metal rendering on macOS)
- The [pi coding agent CLI](https://github.com/earendil-works/pi) installed and authenticated — Orbit's sessions and agent runs are pi's own

### Run

```bash
cargo run -p orbit-pi
```

### Test

```bash
cargo test --workspace          # unit tests + live pi integration tests
```

### Live-reload dev loop

Rust can't hot-swap code into a running process — changes need a rebuild and
relaunch. `bacon` automates it: the app is killed, the crate recompiles, and a
fresh window opens on every save.

```bash
cargo install bacon   # once
bacon run             # watch crates/, restart the app on save
```

`bacon.toml` holds the jobs (`run`, `check`) and the watched paths. Without
bacon: `cargo watch -w crates -x "run -p orbit-pi"` (needs
`cargo install cargo-watch`).

## Project Layout

```
crates/orbit-pi/   The GPUI app (Rust, renders to the GPU)
crates/orbit-rpc/    pi CLI RPC client (process lifecycle + JSONL protocol)
assets/icons/        App icon (1024 PNG source, icon.icns, icon.png)
assets/…
PRODUCT.md           Product definition, capabilities, constraints
INTENT.md            Architecture decisions + phase plan
AGENT.md             Conventions for agents/humans working on this repo
```

## Roadmap

Orbit is growing toward a full workbench: diff review, file tree, terminal, parallel sessions, and a per-tool permission UI once the RPC probe confirms the surface. The current single-session chat is a waypoint, not the destination.

## Contributing

Contributions are welcome! Please open an issue to discuss larger changes before submitting a pull request.

1. Fork the repository
2. Create your branch (`git checkout -b feat/my-feature`)
3. Commit your changes (`git commit -m 'feat: add my feature'`)
4. Push to the branch (`git push origin feat/my-feature`)
5. Open a Pull Request

## License

All rights reserved. See [LICENSE](LICENSE) for licensing details (to be added).

## Acknowledgments

- Built on the [pi coding agent](https://github.com/earendil-works/pi) and its CLI RPC protocol
- UI framework: [GPUI](https://github.com/zed-industries/zed/tree/main/crates/gpui) by Zed Industries
- Architecture reference: [Waku](https://github.com/egoist/waku) — the all-Rust, GPU-rendered agent workbench