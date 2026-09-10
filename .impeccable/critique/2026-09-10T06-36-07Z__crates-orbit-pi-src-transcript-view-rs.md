---
timestamp: 2026-09-10T06-36-07Z
slug: crates-orbit-pi-src-transcript-view-rs
---
# Design Critique: Orbit Transcript View

Method: ⚠️ DEGRADED: single-context (no sub-agent tool exposed; native GPUI app, no browser automation available)

## Design Health Score

| # | Heuristic | Score | Key Issue |
|---|-----------|-------|-----------|
| 1 | Visibility of System Status | 2 | Streaming "Working for" is clear, but per-message copy/timestamp are hidden until hover. |
| 2 | Match System / Real World | 3 | Pi terminology and chat-turn model are natural; "Worked for" duration reads well. |
| 3 | User Control and Freedom | 2 | Can scroll, jump, review, copy; missing edit/retry/undo affordances for sent turns. |
| 4 | Consistency and Standards | 3 | Waku-style vocabulary is consistent internally; ghost footer conflicts with always-visible code-copy button. |
| 5 | Error Prevention | 2 | No destructive actions in the transcript itself, but hidden affordances increase slip errors. |
| 6 | Recognition Rather Than Recall | 2 | Copy, timestamps, and rail previews require remembering to hover; tool details rely on visible chevrons. |
| 7 | Flexibility and Efficiency of Use | 3 | Turn rail speeds long-transcript navigation; no keyboard shortcut to copy a message or jump turns. |
| 8 | Aesthetic and Minimalist Design | 3 | Clean overall, but assistant turns can stack tool cards + prose + files into dense vertical slabs. |
| 9 | Error Recovery | 2 | Failed tools show a red ✕ with no inline explanation; no retry/undo path. |
| 10 | Help and Documentation | 1 | No contextual help; first-time users get no guidance on the rail, folds, or hidden actions. |
| **Total** | | **23/40** | **Acceptable — significant UX improvements needed before users are happy** |

## Design Specificity Verdict

**Strongly product-specific.** The surface is clearly authored for Orbit/pi rather than a generic chat UI. The evidence: a Waku-style GPU-rendered transcript, pi-specific tool taxonomy (bash/read/edit/grep/search), turn-level "Worked for" folds, tool-call detail cards with Arguments/Output, end-of-task changed-files summaries with a Review button, and a left conversation rail keyed to user turns. The deterministic scan flagged one `border-l-2` at line 1634; in context this is a standard markdown blockquote rule, not a decorative side-tab, so treat it as a false positive.

## Overall Impression

The transcript is functionally solid and performance-minded: virtualized list, tail-following scroller, real-time tool activity, and honest pi data. The biggest opportunity is **making the interface legible without a mouse**: too many useful affordances (copy, timestamps, turn previews, rail ticks) are only discoverable on hover, which fails keyboard users, power users, and anyone who doesn't explore by waving a cursor. A close second is completing the markdown/code rendering and giving tool failures a human-readable voice.

## What's Working

1. **Conversation model matches the mental model.** User bubbles right-aligned, assistant prose left-aligned, tool/thinking activity interleaved above the answer, and a single "Worked for" fold per turn collapses the hidden work. This maps cleanly onto how pi actually streams.
2. **Navigation rail for long transcripts.** Ticks per user turn, active-turn highlighting, and hover previews give fast orientation in sessions with many turns. Tail-following plus a jump-to-latest control handles the streaming case well.
3. **Tool and changed-files cards.** Activity rows show action + path + diff count + live pulse; expandable detail sections with per-section copy are precise. The end-of-task summary aggregates edits across the whole run, which is a satisfying completion signal.

## Priority Issues

### [P1] Hover-only message footer hides critical affordances
**What:** The copy button and HH:MM timestamp under each message only appear when the row is hovered (opacity toggled 0 ↔ 1 in `render_message_footer`).
**Why it matters:** Keyboard users and accessibility-dependent users can never trigger them. Even mouse users won't know copy exists until they accidentally hover. This violates "Recognition Rather Than Recall" and makes the app feel thinner than it is.
**Fix:** Make the footer a persistent, subtle row (low-contrast icons by default, slightly stronger on hover/focus). Ensure it shows on keyboard focus and that each button has a focus target.
**Suggested command:** `$impeccable harden crates/orbit-pi/src/transcript_view.rs` for focus/keyboard behavior, then `$impeccable layout` if the always-visible footer needs spacing refinement.

### [P1] Conversation rail is invisible affordance
**What:** The turn rail only appears when the transcript is scrollable and the window is ≥1040px. The ticks have no labels, tooltips, or hover cursor differentiation; the preview card only appears on tick hover.
**Why it matters:** First-time users won't understand why thin bars live on the left, that they jump to turns, or that they show prompt/response snippets. It's a power feature masquerading as decoration.
**Fix:** Add a brief tooltip/label on first appearance (e.g., "Turns"), persist a subtle active-state treatment, and consider a keyboard shortcut (⌘↑/⌘↓ or ⌘1..9) that mirrors the rail's jumps.
**Suggested command:** `$impeccable onboard crates/orbit-pi/src/transcript_view.rs` for first-run discovery, `$impeccable layout` for rail labeling.

### [P1] Tool failures communicate too little
**What:** A failed tool call shows only a red "✕" glyph next to the tool name. The error text lives inside the expandable Output section, which is closed by default.
**Why it matters:** Users scanning a turn see "something broke" but not what or why. In a coding agent, a failing bash/read/edit is the central event; burying it behind a chevron wastes the user's attention.
**Fix:** Show a short error snippet inline under the failed tool row (truncated to one line), tint the row background subtly toward the error color, and auto-expand the Output section when `failed` is true.
**Suggested command:** `$impeccable clarify crates/orbit-pi/src/transcript_view.rs` for error copy, `$impeccable colorize` for failure state color.

### [P2] Markdown/code rendering is underbuilt for a coding agent
**What:** The custom parser handles basic GFM (headings, lists, inline styles, quotes, tables, fenced blocks) but has no syntax highlighting, no language label on code blocks, and edge cases in nested inline formatting. Code blocks render line-by-line without a language chip.
**Why it matters:** Orbit's entire reason to exist is reading code diffs, stack traces, and markdown explanations from an agent. Unhighlighted code is harder to scan and looks unpolished next to Waku/Zed.
**Fix:** Add a language chip to each fenced block (extract the info string from the fence), and integrate a paint-only highlighter so streaming never reflows. Consider replacing the hand-rolled inline parser with `pulldown-cmark` + a small custom mapper to reduce parsing edge cases.
**Suggested command:** `$impeccable typeset crates/orbit-pi/src/transcript_view.rs` for code typography, `$impeccable optimize` if highlighting must stay paint-only.

### [P2] Transcript has no keyboard navigation or screen-reader scaffolding
**What:** Message rows are not focusable, there are no arrow-key bindings to move between turns, and copy/timestamp rely on hover. GPUI doesn't give this for free.
**Why it matters:** macOS users expect keyboard operability; accessibility-dependent users require it. A dense, text-heavy surface without focus management is a hard wall.
**Fix:** Make each interactive message row focusable, add ↑/↓ to move between turns, Enter to toggle folds/tool details, and a shortcut to copy the focused message. Provide visible focus rings that match the app's accent.
**Suggested command:** `$impeccable harden crates/orbit-pi/src/transcript_view.rs` for a11y/keyboard, `$impeccable layout` for focus-ring spacing.

## Persona Red Flags

**Alex (Power User)**
- No keyboard shortcut to copy the latest assistant message.
- No transcript search (per AGENT.md stubs list).
- No retry/re-edit affordance on a sent prompt or failed tool.
- The rail helps, but only with a mouse; no keyboard jump-to-turn.
- Aborts require Esc with no visible transcript-level button.

**Jordan (First-Timer)**
- Copy button and timestamp are invisible until hovered.
- Thin rail ticks on the left have no label or tooltip explaining they jump to turns.
- "Worked for 5 minutes 41 seconds" fold looks like a status line, not a clickable expander.
- Tool cards show a chevron but no label saying "Arguments / Output" until clicked.
- No contextual help or empty-state guidance about what the agent just did.

**Sam (Accessibility-Dependent)**
- Hover-only interactions are effectively nonexistent for keyboard/VoiceOver users.
- No visible focus indicator on message rows or footer buttons.
- Failed tools rely on a red ✕ glyph with no accompanying text (color-only error indicator).
- Small text sizes in tool detail sections (10.5–11.5px) may be hard to read even with high contrast.

## Minor Observations

- **Time formatting inconsistency:** The settled fold says "Worked for 5 minutes 41 seconds" while the live indicator says "Working for 5m 41s". Pick one voice; the verbose form is more human, but keep the live form compact.
- **Working wave dots and pulse dots are decorative motion.** They communicate "alive" but add visual noise; ensure they respect reduce-motion.
- **Changed-files card duplication logic is subtle:** individual turns show file cards, but the last turn's card is suppressed when the end-of-task summary appears. Document this in the UI or users may wonder why some turns lack files.
- **Code-block copy button is always visible** — good; keep it that way and use it as the model for the message footer.
- **Avatar/identity:** User messages have no avatar or name; assistant messages appear author-less. For a coding workbench, this is fine, but it removes the social cue that distinguishes turns at a glance beyond alignment.

## Questions to Consider

- Should copy and timestamp become always-visible, or do you want a "show on selection/focus" compromise to preserve the clean look?
- Is the turn rail a power-user-only feature, or should it be taught on first use with a tooltip/label?
- How much of the markdown surface do you want to harden now — syntax highlighting + language chips only, or a full parser swap?
- Do you want message-level actions (edit prompt, retry, delete) in the transcript, or keep those in a command palette / context menu?
