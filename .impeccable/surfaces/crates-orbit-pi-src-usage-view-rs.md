---
version: 1
slug: "crates-orbit-pi-src-usage-view-rs"
primary_target: "crates/orbit-pi/src/usage/view.rs"
related_targets: ["crates/orbit-pi/src/usage/page.rs","crates/orbit-pi/src/usage/chart.rs","crates/orbit-pi/src/usage/table.rs"]
---

# Usage — surface brief

## Scope & mode
Full main-area page (`UsagePage`) reached from Workbench → Usage. Mode: **Operate**.
One active session's aggregate analytics over pi's own session store.

## Audience, job, action
pi users auditing their own agent usage. Job: read the current window at a glance,
then drill one dimension or one session. Action: scan summary → trend → rank a
breakdown → check token health → open a record table → scope/open a session.

## Content & proof
Everything is derived from pi session files via `UsageSnapshot`; no metric is
invented. Missing data reads as words ("Unavailable"), never a fabricated zero.
Breakdowns: Models / Workspaces / Providers / Tools. Records: Sessions / Daily / Failures.

## Chosen direction (committed)
**Instrument cluster as a card stack.** Each measure owns its own `bg_raised` card
(12px radius, 1px hairline border, 42px header strip) sitting on the canvas with a
16px gap. This deliberately deviates from DESIGN.md's "hairlines over boxes" rule per
the user's brief ("all sections have their individual card"). Charts: one **area**
chart for the trend (metric switcher in the body). Each Breakdown tab is a
ranked **bar chart on top** (top 8 by the dimension's ranking, accent bars) over
a plain **data table** — the same shared `data_table` as the Details → Sessions
table. The toolbar (search + Columns) separates chart from table, so the two
never read as one broken grid. Table parity with Sessions, scoped **per
dimension**: search box, Columns visibility picker, sortable headers, totals
line, pagination footer, each dimension remembering its own search/columns/page.
The earlier chart-in-a-table fold was rejected; the chart is its own band.
Details pairs its three tables under one selector. Nested cards are forbidden.

Memorable moment: switching a Breakdown tab swaps the chart, the table, and its
search/columns/page for that dimension, in place. Clicking a chart bar or a row
scopes the page to that model / workspace / provider.

## Constraints
- Colors/sizes/radii from `theme::get(cx)` only (Bridge Rule). One accent; no second hue.
- In-card controls use `bg_main` as the resting fill (recessed), since the card is `bg_raised`.
- Tables sit inside a card: column budgets subtract the 2px card edge (`table_width`).
- `card(..., clip)` clips full-bleed content (Summary/Breakdown) but stays off for the
  Details card, which must let the Columns popover escape.
- No motion added; state changes carry a static cue.

## Unresolved
- Narrow (< ~620px) columns still scroll tables horizontally rather than dropping
  columns; acceptable (matches the data-grid behavior) but not yet tuned per table.
- No 10%-speed visual walk-through was possible in this environment; needs a human pass.
