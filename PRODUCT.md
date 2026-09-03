# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

Pi coding agent users — developers who already run the pi CLI and want a desktop GUI for their agent sessions. They arrive with existing agent knowledge (models, thinking effort, sessions, tool access); the UI can use pi's own terminology without teaching it. Released publicly, not a personal-only tool, so it cannot assume access to the author's machine, providers, or habits.

## Product Purpose

Orbit is a desktop GUI client for the pi coding agent. It hosts chat-style agent sessions — prompt composer with model / thinking-effort / access controls, streaming responses, and persistent sessions grouped by project — in a Tauri app backed by the pi-coding-agent SDK running as a local sidecar daemon. Success: pi users reach for Orbit instead of (or alongside) the terminal for everyday agent work.

## Positioning

A native-feeling desktop workbench for pi, built directly on the official SDK and its session format (`~/.pi/agent/sessions/`) — sessions created in Orbit and in the CLI are the same sessions. It is not a generic chat frontend wrapping an API; the daemon, session persistence, model catalog, and thinking levels are pi's own.

## Operating Context

Local-first developer machine use: the app talks to a pi agent daemon over WebSocket (`ws://localhost:8912` in dev), sessions persist per project working directory, and the agent runs with full local tool access (file edits, commands). Long-running streaming tasks are the norm, not the exception.

## Capabilities and Constraints

**Current:** single active session per app view; prompt/abort with streaming; session list grouped by project with reopen; model selection from the runtime's auth-checked catalog; thinking-effort selection from the model's supported levels. Access is always "full access" — the SDK exposes no permission mode, so the UI presents alternative modes as unavailable rather than fake.

**Roadmap (confirmed direction):** fuller workbench — diff review, file tree, terminal, multiple agents/sessions in parallel. Current single-session architecture is a waypoint, not the destination.

**Constraints:** pi-coding-agent SDK is Node-only and must stay behind the sidecar boundary; session truth lives in pi's session manager; released publicly, so nothing may hardcode the author's providers, models, or machine paths.

**Terminology:** model, thinking effort, full access, session, project — as used in pi.

## Brand Commitments

Name: **Orbit** (final). Sidebar lockup reads "Orbit Pi".

## Evidence on Hand

None in-repo beyond the product's own UI copy. The sidebar still carries Intent UI template placeholder assets (stock avatar image, intentui.com logo URL) — these are **not** brand assets and are marked for replacement. No testimonials, screenshots, or launch material exist yet; future work must not fabricate any.

## Product Principles

1. **Trust the agent's truth.** Every control and indicator reflects the real daemon/session state; nothing decorative that pretends to be functional.
2. **Speak pi's language.** Users already know models, thinking levels, and sessions — no re-explaining, no renamed concepts.
3. **Respect the operator.** This is a work tool for long sessions: scanability, density, and native expectations outrank expression.
4. **Grow toward the workbench.** Decisions should leave room for parallel sessions, diff review, file tree, and terminal rather than baking in a single-chat assumption.
