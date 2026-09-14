//! Build-time configuration for the in-app updater.
//!
//! The public half of the release signing key is compiled in as
//! `ORBIT_UPDATE_PUBLIC_KEY` so every platform verifies the same EdDSA
//! signatures. Prefer the `ORBIT_UPDATE_PUBLIC_KEY` environment variable; when
//! it is absent, fall back to `SUPublicEDKey` in a bundled `Info.plist` (the
//! same key macOS tooling reads), and finally to an empty string, which leaves
//! the updater dormant.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=ORBIT_UPDATE_PUBLIC_KEY");

    let key = public_key();
    println!("cargo:rustc-env=ORBIT_UPDATE_PUBLIC_KEY={key}");
}

fn public_key() -> String {
    if let Ok(value) = std::env::var("ORBIT_UPDATE_PUBLIC_KEY") {
        let value = value.trim().to_owned();
        if !value.is_empty() {
            return value;
        }
    }

    const PLIST: &str = "resources/Info.plist";
    const MARKER: &str = "<key>SUPublicEDKey</key>";
    if !std::path::Path::new(PLIST).exists() {
        return String::new();
    }
    println!("cargo:rerun-if-changed={PLIST}");

    let Ok(plist) = std::fs::read_to_string(PLIST) else {
        return String::new();
    };
    plist
        .split_once(MARKER)
        .and_then(|(_, rest)| rest.split_once("<string>"))
        .and_then(|(_, rest)| rest.split_once("</string>"))
        .map(|(value, _)| value.trim().to_owned())
        .unwrap_or_default()
}
