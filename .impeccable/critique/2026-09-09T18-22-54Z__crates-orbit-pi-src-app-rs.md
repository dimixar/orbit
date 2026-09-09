---
target: new-task workspace selector
total_score: 27
max_score: 32
na_heuristics: 9,10
p0_count: 0
p1_count: 1
p2_count: 2
timestamp: 2026-09-09T18-22-54Z
slug: crates-orbit-pi-src-app-rs
---
# Critique — new-task workspace selector (crates/orbit-pi/src/app.rs + workspace_picker.rs)

## Heuristic scores

| # | Heuristic | Score | Key issue |
|---|-----------|-------|-----------|
| 1 | Visibility of system status | 3 | Open state was border-only; chevron now answers in accent |
| 2 | Match system / real world | 4 | Native menu anatomy: recents, filter, browse escape hatch |
| 3 | User control & freedom | 4 | Esc, outside-down, click-current-to-close, wrap-around nav |
| 4 | Consistency & standards | 3 | Fixed 400px popover vs fluid field broke the app's own popover contract |
| 5 | Error prevention | 3 | Current row dismisses instead of re-picking |
| 6 | Recognition rather than recall | 4 | Recents pinned current-first; browse demoted but visible |
| 7 | Flexibility & efficiency | 3 | Full keyboard loop; no shortcut to open the picker |
| 8 | Aesthetic & minimalist | 3 | Redundant "Browse" tail label; double accent on current row |
| 9 | Error recovery | n/a | No error paths in this component |
| 10 | Help & documentation | n/a | Self-evident select field |

**Total: 27/32 (84% — Good)**

## Priority issues (as found, with fixes applied)

- **P1 popover overflowed the field/window at narrow widths** — fixed 400px vs fluid field. Fixed: popover width derived from the window (`width_for_window`), mirroring the field's `max_w(FIELD_MAX_W)` inside `PAGE_PAD`; both constants now shared from `workspace_picker.rs` so they cannot drift.
- **P2 "Choose folder…" row said itself twice** — trailing "Browse" label removed.
- **P2 popover could exceed space below the field at the 960×640 minimum window** — list cap reduced 6 → 4 rows so `snap_to_window` never shoves it over the card.
- **P3 open state half-told** — chevron tints accent while open, matching the border.
- **P3 double accent signal on the current row** — folder icon back to `text_3`; the check alone carries "current".

## Detector

Exit 0, 0 findings — structural silence: the detector's regex set targets web markup (Tailwind classes, CSS properties, img tags) and is blind to GPUI Rust. Browser overlay impossible for a native GPU-rendered app. Static review was the effective Assessment B.

## Deferred (documented, not fixed)

- Name column `max_w(140)` vs path squeeze only bites below the 240px popover floor — unreachable at the 960pt minimum window. Revisit if `window_min_size` ever drops below 448.
- Width sampled once at open; harmless while reachable widths always resolve to 400.
