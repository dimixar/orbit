---
version: 1
slug: "src-components-workbench-page-tsx"
primary_target: "src/components/workbench/page.tsx"
related_targets: ["src/App.tsx","src/components/workbench/usage-page.tsx","src/components/workbench/skills-page.tsx","src/components/workbench/plugins-page.tsx","src/components/workbench/settings-page.tsx","agent/index.ts","src/lib/pi-agent.ts"]
---

# Workbench pages (Usage / Skills / Plugins / Settings)

Mode: **Operate**. Audience: pi CLI users in a long local work session — scanability, density, honest state over expression.

## Structure
- Four top-level views switched by `WorkbenchView` state in `src/App.tsx` (no router — single Tauri window). Chat remains the primary surface; the four workbench pages are reached from the sidebar footer's Agent menu (check mark shows the active view).
- All pages render inside `src/components/workbench/page.tsx` scaffold: `WorkbenchPage` (title + description + actions, max-w-3xl column), `WorkbenchSection` (uppercase muted section labels), `WorkbenchCard` (border/radius from tokens), `Skeleton` (content-sized loading, never spinners).

## Data channels (daemon → webview, all read-only)
Added in `agent/index.ts` + `src/lib/pi-agent.ts`: `get_usage` (aggregates *.jsonl under ~/.pi/agent/sessions — per-file mtime/size cache, 16-way concurrency, pre-filters lines before JSON.parse; usage is at `entry.message.usage`), `list_skills` (SKILL.md frontmatter; entries may be symlinks — stat, don't trust Dirent), `list_plugins` (settings.json packages + enabledModels), `get_settings_info` (defaults + auth presence only, never auth contents).
Pages request on mount in a `useEffect`, subscribe via `piAgent.on(...)`, and show Skeleton → empty-state → data. Usage page also refreshes on `agent_end` via its Refresh button.

## Principles
- Trust the agent's truth: every number/label on these pages comes from the daemon or pi's own files. No fabricated metrics, no decorative controls.
- Daemon gotcha fixed during build: register `ws.on("message", ...)` before the `ensureSession()` warm-up await, or early client messages are silently dropped by the EventEmitter.

Unresolved: Usage totals are lifetime-only (no date-range filter); Plugins page is read-only (install/remove stays in the CLI for now).
