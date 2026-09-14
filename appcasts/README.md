# Appcasts

Signed update feeds for the in-app updater
(`crates/orbit-pi/src/updater.rs`). The app fetches them from GitHub's raw CDN:

```
https://raw.githubusercontent.com/imrj05/orbit/main/appcasts/appcast-<os>-<arch>.xml
```

| File | Consumed by |
|---|---|
| `appcast-macos-aarch64.xml` | macOS arm64 (Apple Silicon) |
| `appcast-macos-x86_64.xml` | macOS Intel |
| `appcast-linux-aarch64.xml` | Linux arm64 |
| `appcast-linux-x86_64.xml` | Linux x86_64 |
| `appcast-windows-aarch64.xml` | Windows arm64 |
| `appcast-windows-x86_64.xml` | Windows x86_64 |

**Do not edit these by hand.** `.github/workflows/appcasts.yml` regenerates and
signs them when a GitHub Release is published, then commits the result to
`main`. Each `<enclosure>` points at the release asset and carries a base64
Ed25519 signature over that asset's exact bytes; the app verifies it against the
public key compiled in via `ORBIT_UPDATE_PUBLIC_KEY`.

See **CONTRIBUTING.md → Releasing** for the signing-key setup.

Because GitHub's raw CDN caches for a few minutes, a freshly committed appcast
can take a short while to appear. The app simply reports "no update" until then.
