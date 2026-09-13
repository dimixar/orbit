# Orbit

**Orbit** is a native desktop workbench for the [pi coding agent](https://github.com/earendil-works/pi) — a chat-style GUI rendered **entirely in Rust** on GPUI, the same GPU-accelerated UI framework Zed is built on. It speaks the pi CLI's native RPC protocol directly over stdio. **No browser, no webview, no Node.js — the repo is pure Rust.**

Same product philosophy as [Waku](https://github.com/egoist/waku): the UI layer is pure native Rust drawing to the GPU, and the agent underneath is pi itself — sessions you create in Orbit and in the terminal are the same sessions (`~/.pi/agent/sessions/`), managed by pi's own session manager.

## Features

- **Chat-style agent sessions** — prompt composer with model selection, thinking-effort control, follow-up queueing, mid-run **steering**, cancel, and streaming responses
- **GPU-rendered transcript** — virtualized message list whose cost is independent of message count; stick-to-latest streaming with coalesced commits (a 10k-message harness guards the model layer)
- **In-transcript find** — ⌘F search with matches, counts, and previous/next jump
- **Image lightbox** — click any attachment image for a full-window view
- **Sessions grouped by project** — persistent sessions organized per working directory, with reopen support, cross-workspace sessions, hidden workspaces, and **clone session**
- **Model catalog** — model selection from the pi runtime's own list (`get_available_models`), with favorites shared between the composer picker and a dedicated **Models** page
- **Thinking effort** — level selection derived from each model's supported levels (`get_available_thinking_levels`)
- **Tool activity** — bash, thinking, edit, and other tool rows rendered natively; tool rows expand into Arguments/Output detail cards with per-section copy. Per-tool *permission* dialogs are pending the RPC protocol (see below) and are not faked
- **Git diff panel** — review what the agent changed without leaving the app
- **Side pane** — right-hand **Review** panel with a live `git diff HEAD` of the workspace, refreshed when a run settles; toggle from the top bar
- **Workbench pages** — usage, skills, plugins, models, providers, and settings views backed by pi's on-disk data
- **Markdown rendering** — GFM and syntax-highlighted code; highlighting is paint-only so streaming code blocks never reflow. Mermaid fences stay copyable code blocks (no native renderer)
- **Theming** — dark/light palettes, font, density, and reduce-motion settings persisted to Orbit's own store (`~/.orbit-pi/`)
- **Desktop native** — keyboard operability, custom macOS window chrome, native dialogs, and a signed/notarizable app bundle

## Architecture

```
┌───────────────────────────────────────────┐
│  Orbit (Rust) — GPUI, Metal-rendered UI   │
│  crates/orbit-pi                        │
│  window, transcript, markdown, workbench  │
└──────────────┬────────────────────────────┘
               │ newline-delimited JSON RPC over stdio
┌──────────────▼────────────────────────────┐
│  pi CLI (child process, per open session) │
│  pi --mode rpc — sessions, models, tools  │
└──────────────┬────────────────────────────┘
               │
┌──────────────▼────────────────────────────┐
│  ~/.pi/agent/ — sessions, config, usage   │
│  (same data the pi CLI uses)              │
└───────────────────────────────────────────┘
```

The app spawns the `pi` CLI as a child process and speaks its RPC protocol: JSON requests on stdin, JSON-line events on stdout (text/thinking deltas, tool calls, question dialogs, settle signals). This is the same integration pattern Waku uses. Everything user-facing is rendered by GPUI on Metal — no DOM, no CSS, no browser engine anywhere.

## Status

The v0.1 web app (React 19 + Vite + Tauri 2 + pi SDK daemon) has been **removed**; the GPUI app is the only app. What works today:

- **Working now** — real pi process integration (`crates/orbit-rpc`), sessions sidebar grouped by project, live streaming transcript over the RPC, composer with enter-to-send, steering, follow-up queueing, model/thinking cycling, transcript find, image lightbox, virtualized rendering, markdown, diff/Review, Git page, and the workbench pages (usage, skills, plugins, models, providers, settings)
- **Pending the protocol** — per-tool *permission* dialogs: pi's permission system has not been confirmed to surface over RPC mode, so Orbit launches with full access rather than faking an approval UI
- **Still open** — conversation fork/rewind (clone is available; rewinding to an earlier turn needs entry-id plumbing), and on-device scroll-perf measurement

Feature work is tracked in `INTENT.md` (decisions + phase plan) and `AGENT.md` (conventions + protocol notes). Live behavior is covered by integration tests that spawn a real pi process (they skip cleanly when `pi` is not installed).

## Getting Started

### Prerequisites

- [Rust](https://rustup.rs/) (1.94+ preferred)
- Xcode command line tools (Metal rendering on macOS)
- The [pi coding agent CLI](https://github.com/earendil-works/pi) installed and authenticated — Orbit's sessions and agent runs are pi's own

### Run

```bash
cargo run -p orbit-pi
```

### Package a macOS .app + DMG

`scripts/make-dmg.sh` builds release binaries, assembles a signed `.app` bundle
(`dev.orbit.pi`), and wraps it in a drag-and-drop DMG.

**Prerequisites** — the Rust targets for both architectures:

```bash
rustup target add aarch64-apple-darwin x86_64-apple-darwin
```

**Usage:**

```bash
./scripts/make-dmg.sh            # arm64 + universal DMGs
./scripts/make-dmg.sh arm64      # Apple Silicon (aarch64) DMG
./scripts/make-dmg.sh universal  # universal (arm64 + x86_64) DMG
```

Output goes to `dist/`. The script signs with the `Developer ID Application`
identity (`SIGN_ID` overrides it) and requires `aarch64-apple-darwin` and
`x86_64-apple-darwin` to be installed. The `arm64` DMG contains an
Apple-Silicon-only binary; the `universal` DMG is a fat binary with both
architectures (`lipo`).

The full script:

```bash
#!/usr/bin/env bash
#
# Build and package Orbit as a macOS .app bundle, then wrap it in a DMG.
#
# Usage:
#   ./scripts/make-dmg.sh            # arm64 + universal DMGs
#   ./scripts/make-dmg.sh arm64      # Apple Silicon DMG only
#   ./scripts/make-dmg.sh universal  # universal (arm64 + x86_64) DMG only
#   ./scripts/make-dmg.sh both       # both (default)
#
# Overrides (env):
#   SIGN_ID    codesign identity (default: Developer ID Application)
#   VERSION    bundle version (default: read from Cargo.toml)
#
set -euo pipefail

cd "$(dirname "$0")/.."
ROOT="$(pwd)"

TARGET="${1:-both}"
VERSION="${VERSION:-$(sed -n 's/^version = "\(.*\)"/\1/p' crates/orbit-pi/Cargo.toml | head -1)}"
APP_NAME="Orbit Pi"
EXEC_NAME="orbit-pi"
BUNDLE_ID="dev.orbit.pi"
ICON="assets/icons/icon.icns"
SIGN_ID="${SIGN_ID:-Developer ID Application: One Man Wireless Inc. (RFBXG4V45C)}"

BUILD_ROOT="$ROOT/target/package"
STAGE="$BUILD_ROOT/stage"
DIST="$ROOT/dist"

info()  { printf '\033[1;34m==>\033[0m %s\n' "$*"; }
warn()  { printf '\033[1;33m!!>\033[0m %s\n' "$*"; }
die()   { printf '\033[1;31mXX>\033[0m %s\n' "$*" >&2; exit 1; }

# --- create the .app bundle from an already-built binary -------------------
make_app() {
  local binary="$1"
  local out="$2"

  info "Assembling .app: $out"
  rm -rf "$out"
  mkdir -p "$out/Contents/MacOS" "$out/Contents/Resources"

  cp "$binary" "$out/Contents/MacOS/$EXEC_NAME"
  cp "$ICON" "$out/Contents/Resources/icon.icns"
  printf 'APPL????' > "$out/Contents/PkgInfo"

  cat > "$out/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>en</string>
    <key>CFBundleExecutable</key>
    <string>${EXEC_NAME}</string>
    <key>CFBundleIconFile</key>
    <string>icon</string>
    <key>CFBundleIdentifier</key>
    <string>${BUNDLE_ID}</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleName</key>
    <string>${APP_NAME}</string>
    <key>CFBundleDisplayName</key>
    <string>${APP_NAME}</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>${VERSION}</string>
    <key>CFBundleVersion</key>
    <string>${VERSION}</string>
    <key>LSMinimumSystemVersion</key>
    <string>13.0</string>
    <key>LSApplicationCategoryType</key>
    <string>public.app-category.developer-tools</string>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>
EOF

  chmod +x "$out/Contents/MacOS/$EXEC_NAME"
  info "Signing .app with: $SIGN_ID"
  codesign --force --deep --options runtime --timestamp --sign "$SIGN_ID" "$out"
}

# --- wrap a signed .app into a DMG -----------------------------------------
make_dmg() {
  local app="$1"
  local dmg="$2"

  info "Creating DMG: $dmg"
  rm -f "$dmg"
  local dmg_stage="$BUILD_ROOT/dmg-stage"
  rm -rf "$dmg_stage"
  mkdir -p "$dmg_stage"
  cp -R "$app" "$dmg_stage/"
  # Drag-and-drop target.
  ln -s /Applications "$dmg_stage/Applications"

  hdiutil create \
    -volname "$APP_NAME" \
    -srcfolder "$dmg_stage" \
    -ov \
    -format UDZO \
    "$dmg"
  echo "  -> $dmg"
}

build_and_package() {
  local label="$1"   # e.g. arm64
  local arch="$2"    # rust target triple
  local binary="$3"  # path to the release binary for this arch

  local app="$BUILD_ROOT/${APP_NAME}-${label}.app"
  make_app "$binary" "$app"

  local dmg="$DIST/${APP_NAME}-${VERSION}-${label}.dmg"
  make_dmg "$app" "$dmg"
}

# --- build the release binaries -------------------------------------------
mkdir -p "$BUILD_ROOT" "$DIST"

# arm64 (Apple Silicon)
if [[ "$TARGET" == "arm64" || "$TARGET" == "both" ]]; then
  info "Building release (aarch64-apple-darwin)…"
  cargo build --release -p orbit-pi --target aarch64-apple-darwin
  build_and_package "arm64" "aarch64-apple-darwin" \
    "$ROOT/target/aarch64-apple-darwin/release/$EXEC_NAME"
fi

# universal (arm64 + x86_64)
if [[ "$TARGET" == "universal" || "$TARGET" == "both" ]]; then
  info "Building release (x86_64-apple-darwin)…"
  cargo build --release -p orbit-pi --target x86_64-apple-darwin

  info "Combining universal binary with lipo…"
  uni="$BUILD_ROOT/${EXEC_NAME}-universal"
  lipo -create \
    "$ROOT/target/aarch64-apple-darwin/release/$EXEC_NAME" \
    "$ROOT/target/x86_64-apple-darwin/release/$EXEC_NAME" \
    -output "$uni"

  build_and_package "universal" "universal" "$uni"
fi

info "Done. DMGs in: $DIST"
```

**Notarize** (recommended for public distribution) — a Developer-ID-signed DMG
still triggers a Gatekeeper warning on other Macs until it is notarized. Set a
`notarytool` keychain profile and `make-dmg.sh` notarizes and staples each
`.app` and DMG automatically:

```bash
xcrun notarytool store-credentials notarytool \
  --apple-id you@example.com --team-id TEAMID --password app-specific-password

NOTARY_PROFILE=notarytool ./scripts/make-dmg.sh universal
```

Without `NOTARY_PROFILE` the build still succeeds and prints a warning that the
artifacts are un-notarized.

> **Note on `pi` discovery:** a bundled `.app` launches with a minimal PATH, so
> Orbit now probes common install dirs (`/opt/homebrew/bin`, `/usr/local/bin`)
> and augments the child's PATH to find the `pi` CLI (a Node/Homebrew script)
> and `node` itself. This keeps packaged builds working without a wrapper.

### Test

```bash
cargo test --workspace          # unit tests + live pi integration tests
```

### Live-reload dev loop

Rust can't hot-swap code into a running process — changes need a rebuild and
relaunch. `bacon` automates it: the app is killed, the crate recompiles, and a
fresh window opens on every save.

```bash
cargo install bacon   # once
bacon run             # watch crates/, restart the app on save
```

`bacon.toml` holds the jobs (`run`, `check`) and the watched paths. Without
bacon: `cargo watch -w crates -x "run -p orbit-pi"` (needs
`cargo install cargo-watch`).

## Project Layout

```
crates/orbit-pi/   The GPUI app (Rust, renders to the GPU)
crates/orbit-rpc/    pi CLI RPC client (process lifecycle + JSONL protocol)
scripts/make-dmg.sh  Build a signed .app + DMG (arm64 / universal)
assets/icons/        App icon (1024 PNG source, icon.icns, icon.png)
assets/…
PRODUCT.md           Product definition, capabilities, constraints
INTENT.md            Architecture decisions + phase plan
AGENT.md             Conventions for agents/humans working on this repo
```

## Roadmap

Orbit is growing toward a full workbench: a file tree, terminal, and conversation fork/rewind. Parallel sessions already run (up to six parked processes); the view is single-active. A per-tool permission UI lands if and when the RPC protocol confirms the surface — it is not faked in the meantime.

## Contributing

Contributions are welcome! Please open an issue to discuss larger changes before submitting a pull request.

1. Fork the repository
2. Create your branch (`git checkout -b feat/my-feature`)
3. Commit your changes (`git commit -m 'feat: add my feature'`)
4. Push to the branch (`git push origin feat/my-feature`)
5. Open a Pull Request

## License

All rights reserved. See [LICENSE](LICENSE) for licensing details (to be added).

## Acknowledgments

- Built on the [pi coding agent](https://github.com/earendil-works/pi) and its CLI RPC protocol
- UI framework: [GPUI](https://github.com/zed-industries/zed/tree/main/crates/gpui) by Zed Industries
- Architecture reference: [Waku](https://github.com/egoist/waku) — the all-Rust, GPU-rendered agent workbench