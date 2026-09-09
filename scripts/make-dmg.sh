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
