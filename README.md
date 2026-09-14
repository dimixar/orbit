# Orbit

[![CI](https://github.com/imrj05/orbit/actions/workflows/ci.yml/badge.svg)](https://github.com/imrj05/orbit/actions/workflows/ci.yml)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.94%2B-orange.svg)](https://rustup.rs/)

**Orbit** is a native desktop workbench for the [pi coding agent](https://github.com/earendil-works/pi) — a chat-style GUI rendered entirely in Rust on GPUI, the GPU-accelerated UI framework Zed is built on. It speaks the pi CLI's RPC protocol directly over stdio: no browser, no webview, no Node.

Sessions you create in Orbit and in the terminal are the same sessions (`~/.pi/agent/sessions/`), managed by pi's own session manager.

## Features

**Chat and transcript**

- Chat-style agent sessions — composer with model selection, thinking effort, follow-up queueing, mid-run steering, cancel, and streaming replies
- GPU-rendered transcript — virtualized so cost is independent of message count, with stick-to-latest streaming and coalesced commits
- GFM markdown and syntax-highlighted code; highlighting is paint-only so streaming code blocks never reflow, and Mermaid fences stay copyable code blocks
- In-transcript find (⌘F) and a full-window image lightbox
- Tool activity — bash, edit, read, and thinking rows drawn natively, expanding into Arguments/Output cards with per-section copy

**Sessions**

- Grouped by project, persistent, reopenable, and cross-workspace
- Orbit-owned project list — remove a workspace from the sidebar without deleting pi's sessions
- Clone a session to branch off it

**Safeguards**

- Access modes — Supervised, Auto-accept edits, or Full access, chosen from the composer
- Enforced by a bundled pi extension that hooks `tool_call` and confirms mutating calls the mode does not auto-approve
- An inline bar above the composer offers **Allow once / Always allow this tool / Deny** (↑ ↓ ⏎ esc), with the allowlist recorded per mode
- A confirmation guard, not a sandbox — pi ships no sandbox, and Orbit does not add one

**Review and Git**

- Side pane with a live `git diff HEAD` of the workspace, refreshed when a run settles
- Git page for Changes / History / Graph, staging, and commit

**Workbench**

- Usage, skills, plugins, models, providers, and appearance pages, backed by pi's on-disk data
- Providers — pi's live catalog plus custom endpoints, with API-key or OAuth sign-in and usage meters
- Plugins — install pi packages from npm, git, or a local path, global or per project, and update or remove them in place
- Models and thinking effort read from pi's own runtime
- Appearance — theme palette, background image, fonts, sizes, spacing density, and language

**Native**

- Keyboard operability, custom macOS window chrome, native dialogs, and reduce-motion
- A signed, notarizable `.app` bundle

## Structure

```
┌───────────────────────────────────────────┐
│  Orbit (Rust) — GPUI, Metal-rendered UI   │
│  crates/orbit-pi                          │
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

Orbit spawns the `pi` CLI as a child process per open session and speaks its RPC protocol: JSON requests on stdin, JSON-line events on stdout (text/thinking deltas, tool calls, question dialogs, settle signals). Everything user-facing is painted by GPUI on Metal — no DOM, no CSS, no browser engine. The agent runtime stays `pi` itself, so the model catalog, thinking levels, and persistence remain pi's own.

### Project layout

```
crates/orbit-pi/      The GPUI app — window, transcript, markdown, workbench
crates/orbit-rpc/     pi CLI RPC client (process lifecycle + JSONL protocol)
contrib/              Bundled pi extensions (access guard, auth/quota bridges)
scripts/make-dmg.sh   Build a signed .app + DMG (arm64 / universal)
assets/icons/         App icon (logo-icon.png source, icon.icns, icon.png)
marketing/            Next.js landing page
PRODUCT.md            Product definition, capabilities, constraints
INTENT.md             Architecture decisions + phase plan
AGENT.md              Conventions for agents and humans working on this repo
```

---

Setup, checks, and commit conventions live in [CONTRIBUTING.md](CONTRIBUTING.md). Orbit is licensed under [Apache-2.0](LICENSE); report vulnerabilities per [SECURITY.md](SECURITY.md).
