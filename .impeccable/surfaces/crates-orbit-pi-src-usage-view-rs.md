---
version: 1
slug: "crates-orbit-pi-src-usage-view-rs"
primary_target: "crates/orbit-pi/src/usage/view.rs"
related_targets: ["crates/orbit-pi/src/usage/page.rs","crates/orbit-pi/src/usage/chart.rs","crates/orbit-pi/src/usage/table.rs","crates/orbit-pi/src/usage/heatmap.rs"]
---

# Usage — surface brief

## Scope & mode
Full main-area page (`UsagePage`) reached from Workbench → Usage. Mode: **Operate**.
One active session's aggregate analytics over pi's own session store.

## Audience, job, action
pi users auditing their own agent usage. Job: read the current window at a glance,
then drill one dimension or one session. Action: scan the four headline figures →
read the trend → scan signals → expand a secondary measure (calendar, token
health, breakdown, records) only when the question asks.

## Content & proof
Everything is derived from pi session files via `UsageSnapshot`; no metric is
invented. Missing data reads as words ("Unavailable"), never a fabricated zero.
Breakdowns: Models / Workspaces / Providers / Tools. Records: Sessions / Daily / Failures.

## Chosen direction (committed)
**Metric board over hairline bands.** The page is a reading surface, not an
instrument rack. The one card is the Summary board (12px radius, 1px hairline,
42px header strip) whose four cells are divided by internal hairlines —
Requests, Total tokens, Cache hit rate, Avg response. Everything else lives on
the canvas behind a hairline, per DESIGN.md's "hairlines over boxes" rule.

Three top-level bands follow, each a `section()` (quiet uppercase label + meta,
hairline rule, then content), separated by 28px of canvas:

- **Activity** — the primary read. The metric switcher and the area chart sit at
  the top (the switcher is a quiet bordered control, not a card header). The
  derived signals render as a compact list directly under the chart. The daily
  calendar and the token-health panels (composition + cache) are demoted into
  `disclosure()` rows that are **collapsed by default**, so the first viewport
  stays short; expanding one reveals the full graphic. This deliberately
  replaces the earlier card-per-measure stack.
- **Breakdown** — four dimensions behind a segmented tab; each shows its ranked
  bar chart, search, the shared data table with sortable headers, totals, and
  pagination.
- **Details** — Sessions / Daily / Failures tables behind a segmented tab,
  searchable, sortable, pageable, on the same shared table.

Memorable moment: the default page reads in a single screen — four numbers, one
chart, one line of signals — and the calendar or token health unfolds in place
when asked, without losing the operator's scroll position.

## Deliberate deviations
- DESIGN.md says "hairlines over boxes". The Summary board is the sanctioned
  exception (metric cells are "a single bordered board"). The calendar and the
  token-health panels keep a *subtle raised frame* (`bg_raised`, 10px radius, no
  header strip) because their troughs and empty-day cells are painted in
  `bg_main`/`trough` and need a surface distinct from the canvas. This is the
  smallest frame that keeps them legible, not a card.

## Constraints
- Colors/sizes/radii from `theme::get(cx)` only (Bridge Rule). One accent; no second hue.
- `card()` is reserved for the Summary board; all other bands use `section()`.
- `subpanel()` is the quiet frame for calendar / token-health graphics.
- Tables sit on the canvas now: column budgets subtract only the page padding
  (`table_width`), no card edge.
- No motion added; disclosure state changes carry a static chevron cue.
- The two new disclosure flags (`daily_open`, `health_open`) are session-only and
  intentionally not persisted, so the page always opens calm.

## Unresolved
- Narrow (< ~620px) columns still scroll tables horizontally rather than dropping
  columns; acceptable (matches the data-grid behavior) but not yet tuned per table.
- No 10%-speed visual walk-through was possible in this environment; needs a human pass.
