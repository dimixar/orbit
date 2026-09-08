//! App assets — embedded HugeIcons SVGs (free set, MIT) plus the app mark
//! and the Zed-bundled typefaces (IBM Plex Sans for UI, Lilex for code).
//!
//! `Assets` implements gpui's `AssetSource` over a compile-time-embedded
//! directory so packaged builds don't depend on filesystem layout.

use std::borrow::Cow;

use include_dir::{include_dir, Dir};

static ASSETS: Dir = include_dir!("$CARGO_MANIFEST_DIR/assets");

pub struct Assets;

impl gpui::AssetSource for Assets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        Ok(ASSETS
            .get_file(path)
            .map(|f| Cow::Owned(f.contents().to_vec())))
    }

    fn list(&self, path: &str) -> anyhow::Result<Vec<gpui::SharedString>> {
        Ok(ASSETS
            .get_dir(path)
            .map(|dir| {
                dir.files()
                    .map(|f| gpui::SharedString::from(f.path().to_string_lossy().to_string()))
                    .collect()
            })
            .unwrap_or_default())
    }
}

/// The bundled typefaces that back Zed's `IBM Plex Sans` / `Lilex` aliases.
/// gpui maps `.ZedSans` → `IBM Plex Sans` and `.ZedMono` → `Lilex`, so these
/// are loaded once at startup and the UI can reference them by those names.
const ZED_FONTS: [&str; 8] = [
    "fonts/zed/IBMPlexSans-Regular.ttf",
    "fonts/zed/IBMPlexSans-Italic.ttf",
    "fonts/zed/IBMPlexSans-SemiBold.ttf",
    "fonts/zed/IBMPlexSans-SemiBoldItalic.ttf",
    "fonts/zed/Lilex-Regular.ttf",
    "fonts/zed/Lilex-Bold.ttf",
    "fonts/zed/Lilex-Italic.ttf",
    "fonts/zed/Lilex-BoldItalic.ttf",
];

/// Load the bundled Zed fonts into gpui's text system so the `.ZedSans` /
/// `.ZedMono` family names resolve to real faces. Call once at startup.
pub fn register_zed_fonts(cx: &mut gpui::App) -> anyhow::Result<()> {
    let mut fonts = Vec::new();
    for path in ZED_FONTS {
        if let Some(bytes) = cx.asset_source().load(path)? {
            fonts.push(bytes);
        }
    }
    cx.text_system().add_fonts(fonts)
}
