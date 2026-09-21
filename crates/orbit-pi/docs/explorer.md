# Explorer — project panel + file viewer

Design for Orbit's file-and-folder explorer: a Zed-style project panel for
understanding a workspace's structure, plus a read-only viewer for seeing file
contents. Approved direction: **right dock + full-page Files surface**,
**read-only browse + view** for the first pass, **gitignore-aware** walking with
a hidden-files toggle.

Status: **implemented** (phases 1–4). Later ideas (quick-open, directory
folding, sticky scroll, file operations) remain open — see [Rollout](#rollout).

## Goals

- Browse the active workspace as a folder tree — expand/collapse, folder and
  file glyphs, git status badges, filter, hidden-files toggle.
- Open a file and read it: syntax highlighting for code, rendered Markdown for
  `.md`, line numbers, with honest handling of binary and oversized files.
- Stay inside Orbit's hard rules: GPUI only, no I/O on a frame, virtualized
  lists, paint-only syntax highlighting, every user-facing string in
  `locales/en.yml`.
- Leave room for Zed-parity later (quick-open, git decorations, file
  operations) without re-architecting.

Non-goals for this pass: editing, saving, file operations (new/rename/delete),
multi-select, drag-and-drop, multi-worktree. The panel is a viewer, not an
editor — and it must not pretend otherwise.

## Where it lives

```
┌──────────┬────────────────────────────────┬───────────┬──────────┐
│ Sessions │ chat transcript  /  Files page  │ Explorer  │  Review  │
│ sidebar  │                                 │ (right    │  side    │
│          │  ┌ tabs ─────────────────────┐  │  dock)    │  pane    │
│          │  │ main.rs ×   app.rs ×      │  │  ▾ src    │          │
│          │  ├───────────────────────────┤  │    main.rs│          │
│          │  │  1  use gpui::*;          │  │    app.rs │          │
│          │  │  2  fn main() {           │  │  ▸ assets │          │
│          │  │  3      ...               │  │           │          │
└──────────┴────────────────────────────────┴───────────┴──────────┘
```

- **Explorer** is a second right dock between the main column and the Review
  pane, with its own resize handle (mirrors the sidebar's drag pattern).
  Toggle: top-bar button, `cmd-shift-e` (Zed's binding), command palette row.
- **Files page** is a full-page surface in the main area, dispatched like
  `git_open` / `usage_open` / `settings_open`. It owns a tab strip so several
  files stay open; the chat is one toggle away, exactly as Git and Usage work.
- The Review side pane keeps its current dock and is unaffected.

The explorer follows the active session's workspace (`OrbitApp::current_workspace`)
and shows a workspace header with a switcher when several projects exist.

## Layering

Mirrors `src/usage/` so no I/O ever touches a frame:

```
explorer/
  walk.rs     off-thread snapshot: gitignore-aware tree + git-status map
  tree.rs     DirNode/Row model, expand/collapse, filter, visible-row flatten
  panel.rs    ProjectPanel entity — virtualized list, keyboard nav, context menu
  viewer.rs   FileViewer entity — off-thread read, highlight/markdown paint
```

```
workspace dir
    ↓  walk::snapshot   (background thread; ignore crate)
TreeIndex { nodes, status badges, truncated }
    ↓  tree::visible_rows(expanded, filter)
[Row]  →  panel.rs  (GPUI list(), fixed row height)
file path
    ↓  viewer::load     (background thread; size + binary guards)
FileContent { lines, lang, mode }  →  viewer.rs (StyledText, line numbers)
```

## Data layer

### Walking (`walk.rs`)

Use the `ignore` crate (ripgrep's walker) — already a transitive dependency in
`Cargo.lock`, promoted to a direct one with a stated reason in `Cargo.toml`:

- `.hidden(!show_hidden)` — dotfiles and dot-dirs hidden by default, toggled
  from the panel toolbar.
- `.git_ignore(true)`, `.git_global(true)`, `.git_exclude(true)` — honors the
  repo's own `.gitignore` (including nested ones), global ignore, and
  `.git/info/exclude`.
- `.follow_links(false)` — no symlink cycles, ever.
- Caps: `max_depth` and a hard entry cap (`MAX_ENTRIES`) so a pathological tree
  stays bounded; `TreeIndex.truncated` surfaces the cap honestly.

The snapshot runs once per (workspace, show_hidden) change on the background
executor. Unlike lazy per-directory reads, one snapshot is what makes nested
`.gitignore` correct: the walker reads every level's ignore file as it descends.
Rebuilds are debounced through the existing `watch::WorkspaceWatcher` (already
filters `.git` internals, `target`, `node_modules`, editor cruft), so typing in
a file does not rebuild on every keystroke.

Directories are inserted into the tree even when they contain no visible files,
so empty-but-real folders are not silently dropped.

### Tree model (`tree.rs`)

```rust
pub struct TreeIndex {
    pub root: Node,
    pub badges: HashMap<String, StatusBadge>, // workspace-relative path → M/A/D/U
    pub truncated: bool,
}

pub struct Node {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub children: Vec<Node>, // dirs first, then files, both case-insensitive
}

pub enum Row { Dir { .. expanded }, File { .. } }

pub fn visible_rows(index: &TreeIndex, expanded: &HashSet<PathBuf>, filter: &str) -> Vec<Row>;
```

Flattening is the same shape as `review::tree_rows`: a directory emits a row,
then its children when expanded; a non-empty filter auto-expands matches and
prunes non-matching subtrees. Pure functions over immutable state, unit-tested.

Git badges come from `git::status_rows` (already off-thread), reduced to a
`path → char` map (`U` untracked, `M` modified, `A` added, `D` deleted, `R`
renamed). Files inherit an "inside a changed dir" dot on their parent directory,
like Zed.

### Viewer (`viewer.rs`)

```rust
pub enum Mode { Code { lang: Lang }, Markdown, Text, Image, Binary, TooLarge }

pub struct FileContent {
    pub path: PathBuf,
    pub mode: Mode,
    pub lines: Vec<String>,      // empty for Image/Binary/TooLarge
    pub tokens: Option<Vec<Vec<Token>>>, // paint-only, computed off-thread
    pub bytes: u64,
    pub truncated: bool,
}
```

- **Guards before reading:** reject directories, cap at `MAX_FILE_BYTES`
  (e.g. 2 MiB) → `TooLarge` with the size shown; NUL-byte sniff → `Binary` with
  a hex/“cannot preview” notice. Never allocate a multi-GB string.
- **Code:** `highlight::tokenize` per line at load time (background thread),
  painted with the same `StyledText`/`TextRun` path `sidepane.rs` uses for diff
  rows, so highlighting is paint-only. Line numbers in a fixed gutter.
- **Markdown:** `transcript_view::render_markdown_document` (the Skills page
  already renders `SKILL.md` this way).
- **Images:** reuse the existing `gpui::Image` loader used by attachments.
- **Read-only** is stated in the UI (breadcrumb + a lock/read-only hint), not
  implied.

## UI

### ProjectPanel

- Header: workspace name, hidden-files toggle, collapse-all, refresh.
- Filter field: a `ComposerInput` (the app's input entity) that filters rows.
- Body: `list()` virtualization, fixed 28 px rows, reusing `file_glyph` for
  icons. Row hover/selection colors match `sidepane.rs` (`overlay` /
  `overlay_strong`).
- Keyboard: `↑`/`↓` move, `→` expand, `←` collapse, `home`/`end` jump,
  `enter` opens (or toggles a dir), `cmd-→`/`cmd-←` expand/collapse all.
  Focus handle carries a `ProjectPanel` key context.
- Footer/status: entry count, "truncated" note when the cap hit.
- Context menu: Open, Open in editor (existing `open_in` apps), Reveal in
  Finder, Copy Path, Copy Relative Path, Toggle Hidden Files. Read-only — no
  destructive rows in this pass.

### FileViewer (Files page)

- Tab strip: open paths, active tab, close button per tab, dirty markers are
  `None` (read-only). Middle-click/`cmd-w` closes.
- Toolbar: breadcrumb of the path, file size, line count, “Read-only” hint,
  Open in editor, Reveal.
- Body: virtualized `list()` of lines with a gutter; Markdown renders in a
  centered column like the Skills page; Binary/TooLarge render a centered
  notice.
- Empty state: “Select a file to view” with the tree’s keyboard hint.

## Integration points

| Concern | File | Change |
|---|---|---|
| Module registration | `src/main.rs` | `mod explorer;` |
| Actions + keys | `src/main.rs` | `ToggleProjectPanel`, `OpenFileFinder`; `cmd-shift-e` |
| App state | `src/app.rs` | `project_panel: Entity<ProjectPanel>`, `file_viewer: Entity<FileViewer>`, `files_open: bool`, `explorer_width: Pixels`, `explorer_visible: bool` |
| Toggle/open/close | `src/app/session.rs` | helpers beside `open_git` / `close_git` |
| Layout + dispatch | `src/app/view.rs` | dock after sidebar; resize drag-move listener; Files branch in the `:612` dispatch chain; top-bar toggle |
| Command palette | `src/command_palette.rs` | `PaletteCommand::ToggleProjectPanel`, `OpenFileFinder`; snapshot flags |
| Watching | `src/app/events.rs` | consume `workspace_watcher` dirty → mark explorer stale |
| i18n | `locales/en.yml` + `scripts/gen_locales.py` | new `explorer.*` keys |
| Docs | `AGENT.md`, `PRODUCT.md`, `CHANGELOG.md` | file map + capability + entry |

## Performance rules

- Snapshot, git status, and file reads are background-executor jobs; the UI
  only ever indexes in-memory state.
- Rows are a flat `Vec` rendered through `list()`; frame cost is independent of
  tree size.
- Tokens are computed once per file (off-thread at load) and stored with the
  content; rendering only indexes them.
- The `WorkspaceWatcher` already debounces and filters; reuse it rather than
  starting a second watch. A dirty signal marks the tree and the open file
  stale; both reload off-thread.

## Risks

- **Huge repositories** → entry cap + `truncated` flag; never walk without a
  bound.
- **Symlinks** → never followed.
- **Non-UTF-8 / binary / huge files** → explicit `Binary` / `TooLarge` modes
  with the real size, never a lossy dump.
- **Watch storms** → debounced watcher, filtered paths (existing behaviour).
- **GPUI pre-1.0** → only `list()`/`uniform_list`, `ComposerInput`, and the
  existing icon/highlight helpers; no new GPUI surface area.
- **`ignore` dependency** → already in the lock file; documented reason.

## Testing

- `tree.rs`: flattening with expand/collapse, dirs-first ordering, filter
  pruning, empty-dir retention, badge mapping.
- `walk.rs`: `.gitignore` respected, hidden toggle, entry cap/truncation,
  symlinks not followed.
- `viewer.rs`: binary sniff, size cap, UTF-8-lossy decode, language mapping.
- i18n completeness (`cargo test -p orbit-pi i18n`).
- `cargo build --workspace` clean (zero warnings); `cargo test --workspace`.

## Rollout

1. ✅ **Data layer** — `explorer/walk.rs` + `explorer/tree.rs`, unit-tested in
   isolation (`cargo test -p orbit-pi explorer`).
2. ✅ **Project panel** — `explorer/panel.rs` + `OrbitApp` field + right dock +
   resize + toggle + filter + keyboard nav + git badges.
3. ✅ **File viewer** — `explorer/viewer.rs` + Files page dispatch + tab strip +
   highlight/markdown/image/binary modes.
4. ✅ **Integration** — command palette, `cmd-shift-e` / `cmd-w`, watcher-driven
   refresh, open-in-editor/reveal, i18n keys, docs.
5. ⬜ **Later (out of scope)** — quick-open (`cmd-p`), directory folding, sticky
   scroll, git-diff-vs-head on file select, then file operations.
