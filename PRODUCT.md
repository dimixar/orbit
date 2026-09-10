# Product

<!-- impeccable:product-schema 1 -->

## Platform

adaptive

A native desktop surface (macOS first, Windows/Linux later) drawn with GPUI — Zed's GPU-composited, keyboard-first workbench language. No webview, no DOM. The `adaptive` value marks it as one cross-platform native codebase rather than a web app.

## Users

Pi coding agent users — developers who already run the pi CLI and want a fast desktop GUI for their agent sessions. They arrive with existing agent knowledge (models, thinking effort, sessions, tool access); the UI can use pi's own terminology without teaching it. Released publicly, not a personal-only tool, so it cannot assume access to the author's machine, providers, or habits.

## Product Purpose

Orbit is a native desktop GUI client for the pi coding agent — a chat-style workbench rendered entirely in Rust by GPUI, the same GPU-accelerated UI framework Zed is built on. Every pixel (transcript, markdown, diffs, charts, chrome) is drawn by the GPU: no browser, no webview, no Node daemon. The app speaks the pi CLI's native RPC protocol directly over stdio, so the agent runtime is the same `pi` binary users already know. Success: pi users reach for Orbit instead of (or alongside) the terminal — and feel the difference, from sub-frame scroll on 10k-message transcripts to zero web overhead.

## Positioning

A native, Waku-style workbench for pi — all-Rust, GPU-first, pi's own data underneath. Sessions created in Orbit and in the CLI are the same sessions (`~/.pi/agent/sessions/`); the model catalog, thinking levels, and persistence are pi's own, presented through a pure-Rust client instead of the SDK daemon. Not a generic chat frontend wrapping an API; not an embedded browser.

## Operating Context

Local-first, long-running desktop sessions. The app spawns the pi CLI as a child process and speaks its newline-delimited JSON RPC over stdio — one process per open session — with full local tool access (file edits, commands). Long streaming tasks are the norm: the rendering path is built for them (virtualized transcript, coalesced stream commits, paint-only streaming highlights).

## Capabilities and Constraints

**Current:** single active session per app view (parallel sessions on the roadmap); prompt / steer / abort with streaming; session list grouped by project with reopen; model selection from the pi runtime's catalog (`get_available_models`); thinking-effort selection from the model's supported levels (`get_available_thinking_levels`); tool, bash, and question activity rendered per-tool. Access is "full access" via the CLI launch flags; a per-tool permission UI is pending the transport probe (see Roadmap), until then it is not faked.

**Roadmap (confirmed direction):** complete the all-Rust migration — P0 transport probe (verify pi CLI RPC covers tool-approval events so the Node daemon can retire), P1 shell/theme, P2 RPC client + session data, P3 chat/markdown/tools, P4 workbench, P5 mermaid/a11y, P6 tests/packaging — then grow toward the workbench: diff review, file tree, terminal, multiple agents/sessions in parallel. The single-session chat is a waypoint, not the destination.

**Constraints:** the pi CLI must be installed and authenticated — it is the only agent runtime (the Node daemon and SSE surface are removed). GPUI is pre-1.0: pin the crates.io release and expect deliberate, scheduled API upgrades rather than always-latest. Mermaid diagram rendering is not native to GPUI; until a renderer lands (fallback code block; graphviz or snapshot-based options under evaluation) diagrams are not presented natively. Session truth lives in pi's session files; released publicly, so nothing may hardcode the author's providers, models, or machine paths.

**Terminology:** model, thinking effort, full access, session, project — as used in pi.

## Brand Commitments

Name: **Orbit** (final). Sidebar lockup reads "Orbit Pi".

## Evidence on Hand

None in-repo beyond the product's own UI copy. The sidebar still carries Intent UI template placeholder assets (stock avatar image, intentui.com logo URL) — these are **not** brand assets and are marked for replacement. No testimonials, screenshots, or launch material exist yet; future work must not fabricate any.

## Product Principles

1. **Trust the agent's truth.** Every control and indicator reflects the real pi process/session state (RPC events, on-disk session files); nothing decorative that pretends to be functional.
2. **Speak pi's language.** Users already know models, thinking levels, and sessions — no re-explaining, no renamed concepts.
3. **Respect the operator.** This is a work tool for long sessions: scanability, density, and native expectations outrank expression. Rendering performance is a product requirement, not a follow-up.
4. **Grow toward the workbench.** Decisions should leave room for parallel sessions, diff review, file tree, and terminal rather than baking in a single-chat assumption.
