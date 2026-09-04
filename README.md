# Orbit

**Orbit** is a native-feeling desktop GUI client for the [pi coding agent](https://github.com/earendil-works/pi) — a chat-style workbench for agent sessions, built directly on the official `pi-coding-agent` SDK.

Sessions you create in Orbit and sessions you create in the pi CLI are the **same sessions** (`~/.pi/agent/sessions/`), managed by pi's own session manager. Orbit is not a generic chat frontend wrapping an API — the daemon, session persistence, model catalog, and thinking levels are all pi's own.

## Features

- **Chat-style agent sessions** — prompt composer with model selection, thinking-effort control, and streaming responses
- **Sessions grouped by project** — persistent sessions organized per working directory, with reopen support; cross-workspace sessions included
- **Model catalog** — model selection from the runtime's auth-checked catalog (no hardcoded providers)
- **Thinking effort** — level selection derived from each model's supported levels
- **Git diff panel** — review what the agent changed without leaving the app
- **Workbench pages** — usage, skills, plugins, models, and settings views backed by pi's on-disk data
- **Markdown rendering** — syntax-highlighted code (Shiki), GFM, and Mermaid diagram support in responses

## Architecture

```
┌─────────────────────────────┐
│  Tauri 2 desktop app        │
│  React 19 + Vite + Tailwind │
│  (webview UI)               │
└──────────┬──────────────────┘
           │ WebSocket (ws://localhost:8912) / HTTP + SSE
┌──────────▼──────────────────┐
│  Pi agent daemon (Node)     │
│  agent/index.ts             │
│  hosts the pi-coding-agent  │
│  SDK as a sidecar           │
└──────────┬──────────────────┘
           │
┌──────────▼──────────────────┐
│  pi-coding-agent SDK        │
│  sessions, model runtime,   │
│  tools (~/.pi/agent/)       │
└─────────────────────────────┘
```

- **`agent/index.ts`** — the WebSocket daemon: hosts the pi SDK in Node and exposes it to the Tauri webview over JSON messages (run with `pnpm agent`, bundled for prod with `pnpm agent:build`)
- **`agent/sse-server.ts`** — an HTTP + SSE server implementing the `@assistant-ui/react-pi` `PiClient` contract (`pnpm agent:sse`); auto-started by the Tauri app in production
- **`agent/workbench.ts`** — read-only views over pi's on-disk data (`~/.pi/agent/sessions`, skills, settings), shared by both servers
- **`src/`** — the React UI: chat components, sidebar, git diff panel, and workbench pages
- **`src-tauri/`** — the Tauri 2 shell (Rust), including the SSE sidecar autostart

> The pi SDK is Node-only and stays behind the sidecar boundary — the webview never imports it directly.

## Getting Started

### Prerequisites

- [Node.js](https://nodejs.org/) (LTS)
- [pnpm](https://pnpm.io/)
- [Rust](https://rustup.rs/) and the [Tauri 2 prerequisites](https://v2.tauri.app/start/prerequisites/) for your platform
- The [pi coding agent CLI](https://github.com/earendil-works/pi) — Orbit reads/writes pi's session data and requires an authenticated model provider

### Development

```bash
# Install dependencies
pnpm install

# Terminal 1 — start the pi agent daemon
pnpm agent

# Terminal 2 — start the Tauri dev app
pnpm tauri dev
```

The daemon listens on `ws://localhost:8912` (override with `PI_AGENT_PORT`). The frontend runs on `http://localhost:1420`.

### Production build

```bash
# Bundle the agent sidecar
pnpm agent:build
pnpm agent:sse:build

# Build the desktop app
pnpm tauri build
```

### Tests

```bash
pnpm test         # run once (Vitest)
pnpm test:watch   # watch mode
```

## Scripts

| Script | Description |
|---|---|
| `pnpm dev` | Vite dev server (frontend only) |
| `pnpm build` | Type-check and build the frontend |
| `pnpm tauri dev` | Run the desktop app in dev mode |
| `pnpm tauri build` | Build installers for your platform |
| `pnpm agent` | Run the WebSocket agent daemon (dev) |
| `pnpm agent:build` | Bundle the daemon into `src-tauri/bin/pi-agent.cjs` |
| `pnpm agent:sse` | Run the HTTP/SSE server (dev) |
| `pnpm agent:sse:build` | Bundle the SSE server into `src-tauri/bin/pi-sse.mjs` |
| `pnpm test` | Run Vitest test suite |

## Project Layout

```
agent/            Pi agent daemon + SSE server + shared workbench helpers
src/              React UI (chat, sidebar, workbench, hooks, tests)
src-tauri/        Tauri 2 shell (Rust), sidecar binaries, icons
scripts/          Dev/verification scripts
```

## Roadmap

Orbit is growing toward a full workbench: diff review, file tree, terminal, and multiple agents/sessions running in parallel. The current single-session architecture is a waypoint, not the destination.

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

- Built on the [pi coding agent](https://github.com/earendil-works/pi) SDK
- UI components via [assistant-ui](https://www.assistant-ui.com/), Base UI, React Aria, and Tailwind CSS
- Desktop shell by [Tauri](https://v2.tauri.app/)