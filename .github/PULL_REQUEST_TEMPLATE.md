# Pull Request

## Summary

<!-- What does this change do, and why? -->

## Related issue

<!-- Closes #123, or "none". Open an issue first for larger changes. -->

## Type of change

- [ ] Bug fix
- [ ] New feature
- [ ] Refactor / performance
- [ ] Documentation
- [ ] Chore / tooling

## Checklist

- [ ] `cargo build --workspace` is clean (zero warnings)
- [ ] `cargo test --workspace` passes (`pi`-dependent tests may skip)
- [ ] `cargo clippy --workspace --all-targets` has no new warnings
- [ ] No web UI reintroduced (GPUI only — no React, Tauri, webview, DOM, Node tooling)
- [ ] `pi` remains the only agent runtime (JSONL RPC over stdio)
- [ ] No new dependency without a stated reason
- [ ] No `unsafe` without a comment
- [ ] Docs updated if behavior, architecture, or a decision changed

## Screenshots / recordings

<!-- Required for UI changes. Before/after if it is a visual tweak. -->
