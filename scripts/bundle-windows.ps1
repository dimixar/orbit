# Package the Windows build as a zip: the self-contained orbit-pi.exe (assets
# and bundled pi extensions are compiled in), the app icon, and the license.
#
# Usage (from the repo root, in PowerShell):
#   pwsh scripts/bundle-windows.ps1
#
# Env:
#   VERSION   bundle version (default: crates/orbit-pi/Cargo.toml)

$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

$version = $env:VERSION
if (-not $version) {
    $match = Select-String -Path "crates/orbit-pi/Cargo.toml" -Pattern '^version = "(.+)"' |
        Select-Object -First 1
    $version = $match.Matches[0].Groups[1].Value
}

$triple = "x86_64-pc-windows-msvc"
$package = "orbit-pi-$version-$triple"

cargo build --locked --release -p orbit-pi --target $triple

$stage = "target/package/$package"
Remove-Item -Recurse -Force $stage -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $stage | Out-Null
Copy-Item "target/$triple/release/orbit-pi.exe" "$stage/orbit-pi.exe"
Copy-Item "assets/icons/icon.ico" "$stage/icon.ico"
Copy-Item "LICENSE" "$stage/LICENSE"

New-Item -ItemType Directory -Force dist | Out-Null
Compress-Archive -Path "$stage/*" -DestinationPath "dist/$package.zip" -Force

Write-Host "Created dist/$package.zip"
