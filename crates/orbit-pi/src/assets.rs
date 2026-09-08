//! App assets — embedded HugeIcons SVGs (free set, MIT) plus the app mark.
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
