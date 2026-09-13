//! App icon — dock mark on macOS, PNG for in-app chrome.
//!
//! Source of truth: workspace `assets/icons/Icon-macOS-Default-1024x1024@1x.png`.
//! It follows the macOS icon grid — 824px of artwork centered on a 1024px
//! canvas with a transparent margin — so Dock/Finder render it at the same
//! visual size as native apps. The 512px copy in this crate is what the
//! running binary embeds.

/// 512×512 PNG used by the dock (macOS) and Settings → About.
pub const PNG: &[u8] = include_bytes!("../assets/app-icon.png");

/// Asset path for [`gpui::img`] via [`crate::assets::Assets`].
pub const ASSET: &str = "app-icon.png";

/// The "Orbit Pi" wordmark, shown as the sidebar brand header. Source of
/// truth: workspace `assets/icons/logo.png`; the copy in this crate is what
/// the running binary embeds. White ink, for dark sidebars.
pub const LOGO_ASSET: &str = "logo.png";

/// The wordmark for light sidebars — dark ink with a white halo, so it reads
/// on a light background where the white mark would vanish. Source of truth:
/// workspace `assets/icons/logo-dark.png`.
pub const LOGO_DARK_ASSET: &str = "logo-dark.png";

/// Set the macOS Dock icon for `cargo run` (no `.app` bundle).
///
/// Bundled builds pick the icon up from `icon.icns` / cargo-bundle metadata;
/// this still runs and is a no-op if AppKit already has a bundle icon.
pub fn set_dock_icon() {
    // Keep the PNG in the binary on every OS so About / tests share one embed.
    let _png = PNG;
    #[cfg(target_os = "macos")]
    unsafe {
        use cocoa::appkit::{NSApp, NSApplication, NSImage};
        use cocoa::base::{id, nil};
        use cocoa::foundation::NSData;
        use std::ffi::c_void;

        let data: id =
            NSData::dataWithBytes_length_(nil, _png.as_ptr() as *const c_void, _png.len() as u64);
        let image = NSImage::initWithData_(NSImage::alloc(nil), data);
        if image != nil {
            NSApp().setApplicationIconImage_(image);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_icon_is_png() {
        assert_eq!(&PNG[..4], b"\x89PNG");
        assert!(PNG.len() > 1024);
    }
}
