//! App theme: a GPUI [`Global`] with dark and light palettes.
//!
//! Colors are semantic roles (backgrounds, text, borders, transcript
//! surfaces) rather than raw zinc steps, so every paint site reads from
//! one place. Dark keeps the existing warm-zinc look; light is a matching
//! warm zinc, not a mechanical invert.

use std::path::PathBuf;

use gpui::{hsla, point, px, rgba, rgb, App, BoxShadow, Global, Hsla};

/// Dark or light appearance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    Dark,
    Light,
}

impl ThemeMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
        }
    }

    fn parse(raw: &str) -> Option<Self> {
        match raw.trim() {
            "dark" => Some(Self::Dark),
            "light" => Some(Self::Light),
            _ => None,
        }
    }

    fn load() -> Self {
        std::fs::read_to_string(persist_path())
            .ok()
            .and_then(|raw| Self::parse(&raw))
            .unwrap_or(Self::Dark)
    }

    fn persist(self) {
        let path = persist_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(path, self.as_str());
    }
}

/// Warm-zinc workbench palette. Copy so list closures and hover styles
/// can capture it without a lifetime.
#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub mode: ThemeMode,
    pub bg_main: Hsla,
    pub bg_sidebar: Hsla,
    pub bg_composer: Hsla,
    pub bg_raised: Hsla,
    pub bg_hover: Hsla,
    pub border: Hsla,
    pub text: Hsla,
    pub text_2: Hsla,
    pub text_3: Hsla,
    pub ok_green: Hsla,
    pub stop_red: Hsla,
    pub stop_red_hover: Hsla,
    pub add_green: Hsla,
    pub del_red: Hsla,
    pub spark_orange: Hsla,
    pub menu_bg: Hsla,
    pub send_bg: Hsla,
    pub send_bg_hover: Hsla,
    pub send_fg: Hsla,
    pub overlay: Hsla,
    pub overlay_strong: Hsla,
    pub border_strong: Hsla,
    pub assistant_text: Hsla,
    pub code_bg: Hsla,
    pub code_text: Hsla,
    /// Inline-code chip: Waku `--code-text` / `--code-wash`.
    pub inline_code_text: Hsla,
    pub inline_code_bg: Hsla,
    pub tool_border: Hsla,
    pub tool_name: Hsla,
    pub tool_meta: Hsla,
    pub accent_bar: Hsla,
    pub ring_track: Hsla,
    pub ring_fill: Hsla,
    pub warn: Hsla,
    pub crit: Hsla,
    pub slice_convo: Hsla,
    pub slice_other: Hsla,
    pub trough: Hsla,
    pub shadow_contact: Hsla,
    pub shadow_ambient: Hsla,
}

impl Global for Theme {}

impl Default for Theme {
    fn default() -> Self {
        Self::dark()
    }
}

impl Theme {
    pub fn for_mode(mode: ThemeMode) -> Self {
        match mode {
            ThemeMode::Dark => Self::dark(),
            ThemeMode::Light => Self::light(),
        }
    }

    /// Current hardcoded dark zinc (Waku-like).
    pub fn dark() -> Self {
        Self {
            mode: ThemeMode::Dark,
            bg_main: hex(0x18181A),
            bg_sidebar: hex(0x121214),
            bg_composer: hex(0x1E1E20),
            bg_raised: hex(0x252528),
            bg_hover: hex(0x2C2C30),
            border: hex(0x2C2C2F),
            text: hex(0xE4E4E7),
            text_2: hex(0xA1A1AA),
            text_3: hex(0x7D7D7D),
            ok_green: hex(0x6FDC7F),
            stop_red: hex(0xC05050),
            stop_red_hover: hex(0xD06060),
            add_green: hex(0x7CC97F),
            del_red: hex(0xE06C5F),
            spark_orange: hex(0xE75F3B),
            menu_bg: hex(0x232323),
            send_bg: hex(0x2A2A2C),
            send_bg_hover: hex(0x343438),
            send_fg: hex(0xE4E4E7),
            overlay: hsla(220.0 / 360.0, 0.10, 0.90, 0.05),
            overlay_strong: hsla(220.0 / 360.0, 0.10, 0.90, 0.09),
            border_strong: hsla(220.0 / 360.0, 0.10, 0.90, 0.14),
            assistant_text: hex(0xD6D9E0),
            // Waku `pre`: bg-muted/45 over the background (no tint), text
            // inherits the foreground — only inline chips carry amber.
            code_bg: hex(0x1D1D1F),
            code_text: hex(0xD6D9E0),
            inline_code_text: hex(0xE0A882),
            inline_code_bg: rgba(0xE6E6E614).into(),
            tool_border: hex(0x29303A),
            tool_name: hex(0xD69C4C),
            tool_meta: hex(0x8B90A0),
            accent_bar: hex(0x4C8DFF),
            ring_track: hex(0x3A3A40),
            ring_fill: hex(0xD4D4D8),
            warn: hex(0xD4A054),
            crit: hex(0xE06C5F),
            slice_convo: hex(0xE07A6A),
            slice_other: hex(0x8B8EC8),
            trough: hex(0x2A2A2E),
            shadow_contact: hsla(0., 0., 0., 0.32),
            shadow_ambient: hsla(0., 0., 0., 0.40),
        }
    }

    /// Warm zinc light: off-white canvas, white raised surfaces, dark text.
    /// Accent / git / spark hues stay the same family, darkened for contrast.
    pub fn light() -> Self {
        Self {
            mode: ThemeMode::Light,
            bg_main: hex(0xF7F7F8),
            bg_sidebar: hex(0xF0F0F2),
            bg_composer: hex(0xFFFFFF),
            bg_raised: hex(0xECECEE),
            bg_hover: hex(0xE4E4E7),
            border: hex(0xD4D4D8),
            text: hex(0x18181B),
            text_2: hex(0x52525B),
            text_3: hex(0x71717A),
            ok_green: hex(0x16A34A),
            stop_red: hex(0xDC4C4C),
            stop_red_hover: hex(0xC43C3C),
            add_green: hex(0x15803D),
            del_red: hex(0xDC2626),
            spark_orange: hex(0xE75F3B),
            menu_bg: hex(0xFFFFFF),
            send_bg: hex(0x18181B),
            send_bg_hover: hex(0x27272A),
            send_fg: hex(0xFAFAFA),
            overlay: hsla(220.0 / 360.0, 0.08, 0.12, 0.06),
            overlay_strong: hsla(220.0 / 360.0, 0.08, 0.12, 0.10),
            border_strong: hsla(220.0 / 360.0, 0.10, 0.20, 0.16),
            assistant_text: hex(0x18181B),
            code_bg: hex(0xF2F2F2),
            code_text: hex(0x18181B),
            inline_code_text: hex(0x9A5528),
            inline_code_bg: rgba(0x00000012).into(),
            tool_border: hex(0xE4E4E7),
            tool_name: hex(0xB45309),
            tool_meta: hex(0x71717A),
            accent_bar: hex(0x3B82F6),
            ring_track: hex(0xD4D4D8),
            ring_fill: hex(0x52525B),
            warn: hex(0xB45309),
            crit: hex(0xDC2626),
            slice_convo: hex(0xE07A6A),
            slice_other: hex(0x6366A8),
            trough: hex(0xE4E4E7),
            shadow_contact: hsla(0., 0., 0., 0.10),
            shadow_ambient: hsla(0., 0., 0., 0.14),
        }
    }

    /// Layered drop shadow (tight contact + wide ambient), like Zed's
    /// `ElevationIndex::ModalSurface`.
    pub fn popover_shadow(self) -> Vec<BoxShadow> {
        vec![
            BoxShadow {
                color: self.shadow_contact,
                offset: point(px(0.), px(4.)),
                blur_radius: px(12.),
                spread_radius: px(-2.),
            },
            BoxShadow {
                color: self.shadow_ambient,
                offset: point(px(0.), px(12.)),
                blur_radius: px(32.),
                spread_radius: px(-8.),
            },
        ]
    }

    /// Compact hover-card shadow (slightly tighter than the picker).
    pub fn card_shadow(self) -> Vec<BoxShadow> {
        vec![
            BoxShadow {
                color: self.shadow_contact,
                offset: point(px(0.), px(4.)),
                blur_radius: px(12.),
                spread_radius: px(-2.),
            },
            BoxShadow {
                color: self.shadow_ambient,
                offset: point(px(0.), px(8.)),
                blur_radius: px(24.),
                spread_radius: px(-6.),
            },
        ]
    }
}

fn hex(value: u32) -> Hsla {
    rgb(value).into()
}

fn persist_path() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join(".orbit-pi").join("theme")
}

/// Install the persisted (or default dark) theme as a GPUI global.
pub fn init(cx: &mut App) {
    cx.set_global(Theme::for_mode(ThemeMode::load()));
}

/// Current theme. Panics if [`init`] has not run.
pub fn get(cx: &App) -> &Theme {
    cx.global::<Theme>()
}

/// Switch appearance, persist the choice, and notify global observers.
pub fn set_mode(cx: &mut App, mode: ThemeMode) {
    if get(cx).mode == mode {
        return;
    }
    mode.persist();
    cx.set_global(Theme::for_mode(mode));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_mode() {
        assert_eq!(ThemeMode::parse("dark"), Some(ThemeMode::Dark));
        assert_eq!(ThemeMode::parse("light"), Some(ThemeMode::Light));
        assert_eq!(ThemeMode::parse("  light\n"), Some(ThemeMode::Light));
        assert_eq!(ThemeMode::parse("system"), None);
    }

    #[test]
    fn palettes_differ() {
        let dark = Theme::dark();
        let light = Theme::light();
        assert_ne!(dark.bg_main, light.bg_main);
        assert_ne!(dark.text, light.text);
        assert_eq!(dark.mode, ThemeMode::Dark);
        assert_eq!(light.mode, ThemeMode::Light);
    }
}
