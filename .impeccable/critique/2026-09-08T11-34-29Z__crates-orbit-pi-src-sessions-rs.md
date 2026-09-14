---
target: left sidebar session list
total_score: 26
max_score: 40
na_heuristics: 
p0_count: 0
p1_count: 2
timestamp: 2026-09-08T11-34-29Z
slug: crates-orbit-pi-src-sessions-rs
---
# Critique: Sidebar session list (crates/orbit-pi/src/sessions.rs + app.rs render_side_row)

⚠️ DEGRADED: single-context (no sub-agent tool exposed in this session) — Assessment A (design review) ran inline, then Assessment B (detector) ran sequentially.

## Design Health Score (Nielsen heuristics, 0–4 each)

| # | Heuristic | Score | Key Issue |
|---|-----------|-------|-----------|
| 1 | Visibility of System Status | 3 | Animated per-row running loader + footer connection dot are good; running sessions get no other elevation |
| 2 | Match System / Real World | 4 | pi terminology, workspace (folder) grouping, activity-ordered — exactly right for pi users |
| 3 | User Control and Freedom | 2 | No delete, rename, reveal, or close from the list; a growing junk drawer of sessions |
| 4 | Consistency and Standards | 3 | Row language consistent with settings nav; dead "Search" row breaks the pattern |
| 5 | Error Prevention | 3 | Collapse is safe and reversible; nothing destructive exists to guard |
| 6 | Recognition Rather Than Recall | 2 | Truncated titles/previews with no tooltips; "Search" promises filtering it can't deliver |
| 7 | Flexibility and Efficiency | 2 | cmd-n / cmd-r exist, but no keyboard navigation of the list and no context menus |
| 8 | Aesthetic and Minimalist Design | 3 | Clean zinc density; footer dot is cryptic decoration |
| 9 | Help Users Recognize/Diagnose/Recover Errors | 2 | Red connection dot gives no diagnosis or repair path |
| 10 | Help and Documentation | 2 | No contextual help; only the gear → About path |
| **Total** | | **26/40** | **Acceptable — significant improvements needed** |

## Design Specificity Verdict

**LLM assessment:** The list is competent category furniture — a solid agent-session list, but not yet authored for *Orbit*. The workspace-grouping + activity ordering is genuinely product-specific (sessions belong to projects; last-touched leads). But the row anatomy (two lines of grey text + a spinner) could belong to any chat app, and the sidebar leaves Orbit's strongest assets on the table: pi's own session truth (model, cwd, message counts) and Zed-grade keyboard ergonomics.

**Deterministic scan:** `detect.mjs` returned `[]` on `app.rs` / `sessions.rs`. Expected — it scans web markup, and this is GPUI Rust. Not a clean bill of health; it is a not-applicable scan (fallback signal: code-level review only).

## Overall Impression

The bones are right: workspace grouping, activity-first ordering, virtualized list, live loader. What's missing is everything that turns a list into a *manager*: actions, active-state anchoring, keyboard access, and honest UI (no dead rows, no unexplained dots).

## What's Working

1. **Grouped-by-workspace tree with collapse** — matches how coding sessions actually accumulate (per project), and collapse state persists in-memory per group.
2. **Activity-first ordering** — newest-modified first means the session you just streamed is always at top; labels agree with the sort.
3. **Per-row running loader** — a real, animated indicator of which pi processes are live (active + parked), driven by actual state, not decoration.

## Priority Issues

1. **[P1] No session management actions** — cannot delete, rename, or reveal a session. Why: the list only grows; users will hit 100+ rows with no way to prune. Fix: hover-revealed "…" button → popup (Delete with confirm, Copy path, Reveal in Finder). Suggested: `$impeccable harden` (or shape the menu first).
2. **[P1] Weak active-session anchor** — active row is only a slightly lighter bg; no accent bar, title emphasis barely differs. Why: after alt-tabbing back you scan for where you were. Fix: 3px accent_bar (theme token exists) + text emphasis. Suggested: `$impeccable polish`.
3. **[P2] Dead "Search" nav row** — dimmed, unclickable. Why: violates Product Principle 1 (nothing decorative pretending to be functional). Fix: remove, or wire to a real filter/command palette. Suggested: `$impeccable distill` or `$impeccable shape`.
4. **[P2] No empty state** — fresh pi install (no sessions) shows a bare void. Fix: guidance card with "New Task" CTA. Suggested: `$impeccable onboard`.
5. **[P2] No keyboard navigation** — list is mouse-only. Fix: focus handle + up/down + enter, cmd-1..9 quick switch. Suggested: `$impeccable harden`.

## Persona Red Flags

**Alex (power user):** cmd-n/cmd-r work, but no keyboard way to *move between* sessions; no right-click menu; can't prune sessions without finding the jsonl on disk. Will open Finder and start deleting files — behind the app's back.
**Sam (accessibility):** session rows are mouse-only (no focusable targets, no arrow-key nav); running/connected status is color-and-animation only; truncated text has no accessible full-text path.
**Riley (stress tester):** empty store → blank panel with no guidance; 1000 sessions → fine (virtualized ✓); a session with no user message shows "(empty session)" ✓.

## Minor Observations

- Footer connection dot (7px, color-only) is cryptic; pair with a label or tooltip.
- Workspace count is a bare number — a small count chip would read better.
- Age label (10.5px) and preview (11px) are close in size; hierarchy could be clearer.
- Group header click area toggles collapse, but its hover state mimics a navigation row — affordance ambiguity.

## Questions to Consider

- What if the sidebar showed each session's model + running state as part of its identity, not just its age?
- What would a Zed-grade session switcher feel like — filter-as-you-type instead of a static list?
- Should workspace groups earn their click (open workspace, not just collapse)?
