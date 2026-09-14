# Contributing to Orbit

Thanks for your interest in Orbit. This guide covers how to set up the project,
what we expect from changes, and how to get them merged.

## Before you start

- **Open an issue first** for anything larger than a bug fix or a small
  documentation change. Architectural work is recorded in `INTENT.md`, so a
  quick discussion saves rework.
- Read [`AGENT.md`](AGENT.md) before touching the code. It is the project's
  convention and protocol reference. When it conflicts with
  [`INTENT.md`](INTENT.md) on architecture, `INTENT.md` wins.
- By participating you agree to the [Code of Conduct](CODE_OF_CONDUCT.md).

## Ways to contribute

- Report bugs with the [issue form](https://github.com/imrj05/orbit/issues/new/choose).
- Propose features with the feature-request form.
- Improve docs (`README.md`, `PRODUCT.md`, `AGENT.md`, `INTENT.md`).
- Submit pull requests for open roadmap items.

## Development setup

### Prerequisites

- [Rust](https://rustup.rs/) (1.94+ preferred)
- Xcode command line tools (the app renders with Metal on macOS)
- The [pi coding agent CLI](https://github.com/earendil-works/pi) installed and
  authenticated — Orbit drives pi as a child process, so live tests and the app
  need it. Tests that require `pi` skip cleanly when it is absent.

### Run

```bash
git clone https://github.com/imrj05/orbit.git
cd orbit
cargo run -p orbit-pi
```

### Verify before opening a PR

```bash
cargo build --workspace      # must stay clean: zero warnings
cargo test --workspace       # unit tests + live pi integration tests
cargo clippy --workspace --all-targets
```

`cargo build` with zero warnings is a stated project invariant. CI runs build,
test, and clippy on macOS (`.github/workflows/ci.yml`).

For UI changes, launch the app (`cargo run -p orbit-pi`) and confirm the
affected surface works. Notifications need an app bundle — use
`scripts/run-bundled.sh`. For a rebuild-and-relaunch loop on save,
`cargo install bacon && bacon run`.

## Project invariants

These are non-negotiable; a PR that breaks one will be asked to change.

1. **No web in the UI.** New UI is GPUI only. Do not reintroduce React, Vite,
   Tauri, webviews, a DOM, Tailwind, or Node tooling.
2. **The pi CLI is the only agent runtime.** Speak its JSONL RPC over stdio
   (`crates/orbit-rpc`). There is no Node daemon.
3. **GPUI is pinned.** `gpui = "0.2.2"`. Upgrade deliberately, never track `main`.
4. **Performance is a product requirement.** Virtualize large lists and never
   block a frame with I/O.
5. **Trust pi's truth.** Render real RPC events and on-disk state; never fake a
   control that does not work.
6. **No `unsafe` without a comment; no new dependency without a stated reason.**

Full detail lives in `AGENT.md` → *Hard rules*.

## Commit and branch conventions

The history follows [Conventional Commits](https://www.conventionalcommits.org/):
`type(scope): summary` in the imperative mood.

```
feat(access-guard): prompt inline with allowlistable tool approvals
fix(transcript): keep tail-follow after remeasure
docs: add contributing guide
chore: update 10 files
```

Common types: `feat`, `fix`, `docs`, `refactor`, `perf`, `test`, `chore`.
Use the module name as the scope where it helps (`transcript`, `review`,
`access-guard`, `git-panel`, …).

Branch names are short and descriptive:

```
feat/parallel-sessions
fix/sidebar-watcher
docs/open-source-files
```

## Pull requests

1. Fork the repository and branch from `main`.
2. Keep the change focused; unrelated cleanup belongs in its own PR.
3. Fill in the pull-request template checklist.
4. Make sure build, tests, and clippy pass locally.
5. Push and open the PR. Link the issue it addresses.

A maintainer will review. Expect comments on architecture fit, performance, and
the invariants above.

## Licensing of contributions

Orbit is licensed under the [Apache License 2.0](LICENSE). Unless you state
otherwise, any contribution you submit is licensed under the same terms, with
no additional conditions (Apache-2.0, section 5).

## Reporting security issues

Do **not** open a public issue for a security vulnerability. See
[`SECURITY.md`](SECURITY.md) for how to report it privately.
