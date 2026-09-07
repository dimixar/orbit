//! Filetype glyphs from the [`devicons`] crate, rendered as text with the
//! embedded Symbols Nerd Font. Used for folder/file icons in the sidebar,
//! status bar, and transcript tool rows.
//!
//! Glyph + color come from devicons' theme maps (e.g. `.rs` → `` in rust
//! orange). The Nerd Font binary itself is registered in `main.rs`.

use std::path::Path;

use gpui::{div, prelude::*, px, rgb, IntoElement};

pub const NERD_FONT_FAMILY: &str = "Symbols Nerd Font";

/// Render the devicons glyph for a path (file or directory) in its theme
/// color. `fallback` is used when devicons' color string can't be parsed.
pub fn dev_icon<P: AsRef<Path>>(path: P, size: f32, fallback: u32) -> impl IntoElement + use<P> {
    let file_icon = devicons::FileIcon::from(path);
    let color =
        u32::from_str_radix(file_icon.color.trim_start_matches('#'), 16).unwrap_or(fallback);
    div()
        .font_family(NERD_FONT_FAMILY)
        .text_size(px(size))
        .text_color(rgb(color))
        .child(file_icon.icon.to_string())
}
