# Changelog

All notable changes to Orbit Pi are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed

- Search and filter placeholders (provider, model, settings, plugins, skills,
  side pane, git panel, usage, in-transcript find, branch/workspace pickers)
  now follow a language change immediately instead of keeping the language
  they were created in. Placeholders are resolved from translation keys at
  paint time.
- With no saved language preference, `System` now walks the OS's ordered
  preferred-language list and picks the first language Orbit ships (falling
  back through unshipped choices such as Hindi) instead of defaulting to
  English. The macOS bundle also declares its supported localizations.
- The setup page's dependency descriptions follow a language change without
  re-probing the host.

## [0.0.9] - 2026-09-21

### Added

- **Version History** in the update modal, opened from the update and
  up-to-date dialogs: every release the feed carries, newest first, each with
  its changelog and the running build marked **Current**. Appcasts now
  accumulate across releases (the newest 20 signed items), so the list grows
  with each release.

### Changed

- The update modal leads with the app icon, checks against an indeterminate
  progress bar, and names the current version when up to date.
- **Check for Updates** always opens the modal. A build that can't check
  (debug, or a binary outside a managed install) says so there instead of
  posting a toast; with `ORBIT_FORCE_UPDATER=1` and the signing key compiled
  in, such a build runs a real check-only flow without **Update now**.
- The update flow now runs through a single modal. A staged release and
  **Check for Updates…** both open it: the check shows a search progress bar,
  then the release's changelog with **Cancel** / **Update now**, while an
  already-staged release offers **Later** / **Update now**. The Settings
  update control is an icon-only download button, and the sidebar shows a
  20px download glyph that expands to **Update** on hover (the reference
  app's pattern); the changelog rides in each appcast item's `<description>`
  (written from `CHANGELOG.md` at release time).

## [0.0.8] - 2026-09-20

## [0.0.7] - 2026-09-20

### Added

- Interface localization via `rust-i18n`, shipped with ten locales — English,
  Simplified Chinese, Japanese, Korean, Spanish, French, German, Brazilian
  Portuguese, Russian, and Italian — plus a `System` option that follows the
  OS preferred language. Settings → Appearance → Language switches it live
  (including the native menu bar); `locales/en.yml` is the source of truth and
  `scripts/gen_locales.py` regenerates the other files from the glossary.

## [0.0.6] - 2026-09-19

### Added

- Clone a session straight from the sidebar — each session row's `…` menu
  gained **Clone session**, which duplicates that session on disk (fresh id,
  pi's `<timestamp>_<id>.jsonl` naming, entries copied verbatim) and drops the
  copy into the list to branch off. Unlike Delete, it only reads the source, so
  it works on any row — including one with a live pi process — without
  switching away from what you're doing. The ⌘P "Clone Session" command still
  clones the open session through pi.
- Usage page — a full-width **Daily activity** calendar heatmap: one cell per
  day across a trailing 12 months, shaded by the active metric (tokens,
  requests, cost, …), with month/weekday axes, a less→more legend, a hover
  readout, and click-to-scope to a single day (click again to restore the
  range). It is independent of the date range — a contribution graph needs a
  year to read — but the active workspace / model / provider / errors filters
  apply to every cell.

## [0.0.5] - 2026-09-17

### Added

- Windows releases attach the bare `orbit-pi.exe` alongside the `.zip`, so the
  executable is a direct single-file download (`scripts/bundle-windows.ps1`).
- Linux releases attach a native `.deb` alongside the `.tar.gz`, installable
  with `apt install ./orbit-pi_*.deb` (`scripts/bundle-linux.sh`).

## [0.0.4] - 2026-09-17

## [0.0.3] - 2026-09-16

### Added

- Integrated terminal — a real login shell in a resizable bottom panel (⌘J or
  the top-bar toggle), independent of the right side pane. Built on
  `alacritty_terminal` for PTY and VT/ANSI emulation, rendered natively by
  GPUI: scrollback, click-drag selection with ⌘C/⌘V, bracketed paste, a
  blinking cursor, and a per-theme ANSI palette. The shell follows the active
  workspace and restarts when it changes.

## [0.0.2] - 2026-09-16

### Added

- Native macOS menu bar (Orbit / File / Edit / View) wired to the app's
  existing actions; ⌘N and the About surface now have a real home.

## [0.0.1] - 2026-09-14

### Added

- Chat-style `pi` sessions — streaming transcript, model selection, thinking
  effort, follow-up queueing, mid-run steering, and cancel.
- Sessions grouped by project, reopenable and clonable, with an Orbit-owned
  project list that leaves `pi`'s own session store untouched.
- Access guard — Supervised / Auto-accept edits / Full access, enforced by a
  bundled `tool_call` extension with an inline Allow once / Always allow this
  tool / Deny bar.
- Review pane with a live `git diff HEAD`, and a Git page for Changes /
  History / Graph with staging and commit.
- Workbench pages — usage, skills, plugins, models, providers, and appearance,
  read from `pi`'s on-disk data.
- In-transcript find (⌘F) and a full-window image lightbox.
- Native macOS app bundle, Developer-ID signed and notarizable.

[Unreleased]: https://github.com/imrj05/orbit/compare/v0.0.9...HEAD
[0.0.1]: https://github.com/imrj05/orbit/releases/tag/v0.0.1
[0.0.2]: https://github.com/imrj05/orbit/releases/tag/v0.0.2
[0.0.3]: https://github.com/imrj05/orbit/releases/tag/v0.0.3
[0.0.4]: https://github.com/imrj05/orbit/releases/tag/v0.0.4
[0.0.5]: https://github.com/imrj05/orbit/releases/tag/v0.0.5
[0.0.6]: https://github.com/imrj05/orbit/releases/tag/v0.0.6
[0.0.7]: https://github.com/imrj05/orbit/releases/tag/v0.0.7
[0.0.8]: https://github.com/imrj05/orbit/releases/tag/v0.0.8
[0.0.9]: https://github.com/imrj05/orbit/releases/tag/v0.0.9
