---
target: scoped models page
total_score: 22
max_score: 40
na_heuristics: 
p0_count: 1
p1_count: 2
timestamp: 2026-09-04T20-25-09Z
slug: src-components-workbench-models-page-tsx
---
#### Design Health Score

| # | Heuristic | Score | Key Issue |
|---|-----------|-------|-----------|
| 1 | Visibility of System Status | 2 | Footer/Note report save, but first paint could lie before scope loaded |
| 2 | Match System / Real World | 3 | Speaks pi fluently; over-explains for operators who already know /scoped-models |
| 3 | User Control and Freedom | 2 | Enable all wipes a custom scope with no undo |
| 4 | Consistency and Standards | 3 | Same workbench scaffold; heading skip was h1→h3 |
| 5 | Error Prevention | 2 | Last model can be unchecked to an empty scope; Enable all is unconfirmed |
| 6 | Recognition Rather Than Recall | 3 | Names, ids, provider counts, and raw patterns are visible |
| 7 | Flexibility and Efficiency | 1 | No search, no provider select-all, one network save per click |
| 8 | Aesthetic and Minimalist Design | 2 | Dense list is right; status chrome used to repeat the same lecture five times |
| 9 | Error Recovery | 2 | Failed toggle reverts; no retry control on the error line |
| 10 | Help and Documentation | 2 | Always-on documentation rather than task-timed help |
| **Total** | | **22/40** | **Acceptable** |

#### Design Specificity Verdict

**LLM assessment**: Authored in semantics, category-interchangeable in composition. The page is unmistakably Orbit/pi (`enabledModels`, `/scoped-models`, Ctrl+P, `settings.json`). The chrome is stock workbench-settings: title, prose, outline buttons, Note, uppercase section, bordered checkbox card.

**Deterministic scan**: Detector returned 0 findings on `models-page.tsx`, `page.tsx`, `note.tsx`, and `checkbox.tsx`.

**Visual overlays**: No reliable user-visible overlay. Fallback: Tauri desktop workbench; no mutable browser overlay available in this session.

#### Overall Impression

A correct pi surface whose primary action was broken. Toggling a checkbox remounted status chrome, disabled every row, and inserted a patterns block at the bottom of an overflow scroller — so the column jumped to the bottom and cut the list you were editing.

#### What's Working

1. Pi-correct scope semantics: unscoped null = all enabled; uncheck one scopes the rest; re-enable all clears the setting.
2. Dense provider-grouped catalog with display name + `provider/modelId`.
3. Honest save failure: optimistic flip, revert, explicit danger copy.

#### Priority Issues

- **[P0] Toggle scroll-jump**
  - **Why it matters**: The only task on the page (enable/disable a model) threw the operator off the row they clicked.
  - **Fix**: Keep the focused checkbox enabled; pin `scrollTop`; stop mounting/unmounting the patterns section; reserve a same-height status line; disable scroll anchoring on the workbench column.
  - **Suggested command**: `$impeccable layout`

- **[P1] Scope paint is a lie until loaded**
  - **Why it matters**: Initial `scopedIds` is null. A click before GET lands can persist “all except this one” and wipe the real set.
  - **Fix**: Don’t accept toggles until `loaded`. Show a pending status line.
  - **Suggested command**: `$impeccable harden`

- **[P1] Status chrome steals the task**
  - **Why it matters**: The same fact was taught in the header, info Note, card footer, patterns blurb, and CLI paragraph. The info Note was the loudest object and it restyled on first uncheck.
  - **Fix**: One reserved status line; shorter footer; patterns always present.
  - **Suggested command**: `$impeccable distill`

- **[P2] No expert path through a long catalog**
  - **Why it matters**: Alex cannot scope “all Anthropic except two” without walking the whole card.
  - **Fix**: Provider-row select-all and a filter field.
  - **Suggested command**: `$impeccable polish`

- **[P2] Empty/error states collapse two worlds**
  - **Why it matters**: Unreachable server and empty catalog share one “No models available” card.
  - **Fix**: Split unreachable vs empty, with Retry on the error line.
  - **Suggested command**: `$impeccable harden`

#### Persona Red Flags

**Alex (Power User)**: No filter, no provider bulk, no shortcut. Each click used to disable the list and dump scroll. Enable all is the only accelerator and it is nuclear, with no undo.

**Jordan (First-Timer)**: “No scope set” as an info callout read as something missing. First uncheck used to rewrite the page and open “Patterns in settings.json” — looked like a settings dump.

**Sam (Keyboard / a11y)**: Heading skip h1→h3. Note had no live region. Toggle disabled the focused checkbox and ejected keyboard focus. After the jump the focused object was not the row Sam toggled.

#### Minor Observations

- Section title “Models” on a page titled “Scoped models”.
- Refresh replaces the list with three skeletons that don’t match row height.
- `saveState` stays `'saved'` forever after the first success.
- 30s interval + focus refetch can snap patterns without a user action.

#### Questions to Consider

- If the default is “everything enabled,” why was that an info callout instead of a quiet count?
- Should the provider band be the real control (toggle the house, then nibble exceptions)?
- If `settings.json` globs are the source of truth, why do checkboxes create a resolved id list on first uncheck?
