#!/usr/bin/env bash
#
# Package the Linux build as a relocatable tarball: the self-contained
# `orbit-pi` binary (assets + bundled pi extensions are compiled in), a desktop
# entry, the app icon, and the license.
#
# Usage:
#   ./scripts/bundle-linux.sh
#
# Env:
#   VERSION   bundle version (default: crates/orbit-pi/Cargo.toml)
#
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

version="${VERSION:-$(sed -n 's/^version = "\(.*\)"/\1/p' crates/orbit-pi/Cargo.toml | head -1)}"
triple="$(rustc -vV | sed -n 's/^host: //p')"
package="orbit-pi-${version}-${triple}"
archive="dist/${package}.tar.gz"

staging="$(mktemp -d)"
trap 'rm -rf -- "$staging"' EXIT

cargo build --locked --release -p orbit-pi

dir="$staging/$package"
install -Dm755 target/release/orbit-pi "$dir/bin/orbit-pi"
install -Dm644 assets/icons/icon.png \
  "$dir/share/icons/hicolor/256x256/apps/dev.orbit.pi.png"
install -Dm644 LICENSE "$dir/share/licenses/orbit-pi/LICENSE"

mkdir -p "$dir/share/applications"
cat > "$dir/share/applications/dev.orbit.pi.desktop" <<'EOF'
[Desktop Entry]
Type=Application
Name=Orbit Pi
Comment=Native workbench for the pi coding agent
Exec=orbit-pi
Icon=dev.orbit.pi
Categories=Development;
Terminal=false
EOF

mkdir -p dist
tar -C "$staging" -czf "$archive" "$package"
printf 'Created %s\n' "$archive"
