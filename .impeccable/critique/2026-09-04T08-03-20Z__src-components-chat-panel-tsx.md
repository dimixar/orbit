---
target: composer dropdown/popover UI/UX
total_score: 26
max_score: 40
na_heuristics: 
p0_count: 0
p1_count: 2
timestamp: 2026-09-04T08-03-20Z
slug: src-components-chat-panel-tsx
---
# Critique: Composer dropdown/popover UI/UX (chat-panel.tsx)

Target: `src/components/chat-panel.tsx` — ContextMeter details popover + BranchPicker (context details + branch switcher popovers at the composer's bottom status row).

## Design Specificity Verdict
LLM: The popovers are product-appropriate (real daemon/git data, pi terminology) but the branch list was hand-rolled — generic buttons where the app owns a Menu/MenuItem system (used by the model selector, access selector, sidebar menus). Category-interchangeable list rendering; missed the system's keyboard/ARIA behaviors.
Detector: 0 findings (deterministic scan clean).
Browser: not available this session — no overlay evidence.

## Design Health Score
| # | Heuristic | Score | Key Issue |
|---|-----------|-------|-----------|
| 1 | Visibility of System Status | 2 | Checkout/create show no busy feedback; long checkouts feel frozen |
| 2 | Match System / Real World | 4 | Git terminology, pi language, truthful data |
| 3 | User Control and Freedom | 3 | Escape closes/cancels; checkout is git-reversible |
| 4 | Consistency and Standards | 2 | Hand-rolled list vs system MenuItem; trigger style diverges from chipTriggerClass |
| 5 | Error Prevention | 3 | Server validates names + rejects option-lookalikes; client lets invalid names reach git |
| 6 | Recognition Rather Than Recall | 3 | Search + visible list; no typeahead |
| 7 | Flexibility and Efficiency | 1 | No arrow-key nav, no Enter activation, no roving focus in branch list |
| 8 | Aesthetic and Minimalist Design | 3 | Clean; minor spacing/label drift |
| 9 | Error Recovery | 2 | Raw execFile error dumps the full command line + stderr |
| 10 | Help and Documentation | 3 | aria-labels present; no visible affordance that the chip is clickable |
| **Total** | | **26/40** | **Acceptable** |

## Priority Issues
- [P1] Branch list lacks keyboard navigation and list semantics — Tab-only through every branch, no arrows/Enter, screen readers announce N unlabeled buttons with no selection state. Fix: rebuild on RAC Autocomplete + system Menu/MenuItem (menuContentStyles).
- [P1] No busy feedback during checkout/create. Fix: per-item spinner (ArrowPathIcon animate-spin) + disabled in-flight row.
- [P2] Error text dumps execFile's full command line + stderr. Fix: server returns only git's fatal:/error: line.
- [P2] Trigger states diverge from composer chips (no data-pressed state). Fix: adopt chipTriggerClass behavior.
- [P2] Detached HEAD renders "HEAD" as if it were a branch name. Fix: show "detached".
- [P3] Create input accepts invalid git refname chars until git rejects. Server-side validation covers option injection; git rejects the rest.

## Persona Red Flags
- Alex (power user): no arrow keys, no Enter-to-select in branch list; must tab through every row.
- Sam (a11y): list is N bare buttons, no listbox/menu semantics, no aria-selected for current branch.
- Riley: duplicate create surfaces raw command dump instead of git's message.

## What's Working
- Truthful data only (real daemon events, real git state, hidden when unknown) — matches product principle 1.
- Consistent escalation logic shared between ring, bar, and figures via contextLevel().
- Server contract is clean: no shell, timeouts, option-injection guard.

## Minor Observations
- "Branches" section label was aria-only; restored visually.
- Search input is now uncontrolled — Autocomplete owns filtering; Escape closes the popover (standard combobox behavior).
