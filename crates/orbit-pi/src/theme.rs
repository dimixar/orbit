//! App theme: a GPUI [`Global`] with a selectable palette.
//!
//! Colors are semantic roles (backgrounds, text, borders, transcript
//! surfaces) rather than raw steps, so every paint site reads from one
//! place. A [`ThemeId`] names the Orbit dark or light palette. The UI
//! face is Zed's bundled IBM Plex Sans (`.ZedSans`); code surfaces use
//! Zed's Lilex (`.ZedMono`).

use std::path::PathBuf;
use std::sync::{OnceLock, RwLock};

use gpui::{hsla, point, px, rgb, App, BoxShadow, Global, Hsla, Pixels, SharedString};
use serde_json::Value;

use crate::highlight::TokenClass;

/// Dark or light appearance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    Dark,
    Light,
}

/// One of the selectable Orbit palettes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeId {
    Orbit,
    OrbitLight,
    /// Ported Zed community themes (all dark).
    Vague,
    Batsignal,
    Ashwood,
    Obsidian,
    MatteBlack,
    AdwaitaPastel,
    Ashen,
    Discord,
}

impl ThemeId {
    /// Selectable themes, in the order shown in the settings dropdown
    /// (the two Orbit palettes first, then the ported Zed themes).
    pub const ALL: [ThemeId; 10] = [
        Self::Orbit,
        Self::OrbitLight,
        Self::Vague,
        Self::Batsignal,
        Self::Ashwood,
        Self::Obsidian,
        Self::MatteBlack,
        Self::AdwaitaPastel,
        Self::Ashen,
        Self::Discord,
    ];

    /// Persisted key; also accepts legacy theme names (mapped to Orbit).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Orbit => "orbit",
            Self::OrbitLight => "orbit-light",
            Self::Vague => "vague",
            Self::Batsignal => "batsignal-dark",
            Self::Ashwood => "ashwood",
            Self::Obsidian => "obsidian-dark",
            Self::MatteBlack => "matte-black",
            Self::AdwaitaPastel => "adwaita-pastel-dark",
            Self::Ashen => "ashen",
            Self::Discord => "discord-dark",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim() {
            // Current keys, plus legacy dark theme names → Orbit.
            "orbit"
            | "dark"
            | "one-dark"
            | "zedokai"
            | "zedokai-darker"
            | "zedokai-filter-octagon"
            | "zedokai-darker-filter-octagon"
            | "zedokai-filter-spectrum"
            | "zedokai-darker-filter-spectrum"
            | "flexoki-dark" => Some(Self::Orbit),
            // Current key, plus legacy light theme names → Orbit Light.
            "orbit-light" | "light" | "one-light" | "flexoki-light" => Some(Self::OrbitLight),
            // Ported Zed community themes (aliases included).
            "vague" => Some(Self::Vague),
            "batsignal-dark" | "batsignal" => Some(Self::Batsignal),
            "ashwood" => Some(Self::Ashwood),
            "obsidian-dark" | "obsidian" => Some(Self::Obsidian),
            "matte-black" | "matte-black-theme" => Some(Self::MatteBlack),
            "adwaita-pastel-dark" | "adwaita-pastel" => Some(Self::AdwaitaPastel),
            "ashen" => Some(Self::Ashen),
            "discord-dark" | "dark-discord" | "discord" => Some(Self::Discord),
            _ => None,
        }
    }

    /// Human label for the settings dropdown.
    pub fn label(self) -> &'static str {
        match self {
            Self::Orbit => "Orbit",
            Self::OrbitLight => "Orbit Light",
            Self::Vague => "Vague",
            Self::Batsignal => "Batsignal (Dark)",
            Self::Ashwood => "Ashwood",
            Self::Obsidian => "Obsidian Dark",
            Self::MatteBlack => "Matte Black",
            Self::AdwaitaPastel => "Adwaita Pastel Dark",
            Self::Ashen => "Ashen",
            Self::Discord => "Discord Dark",
        }
    }

    pub fn appearance(self) -> ThemeMode {
        match self {
            Self::OrbitLight => ThemeMode::Light,
            Self::Orbit
            | Self::Vague
            | Self::Batsignal
            | Self::Ashwood
            | Self::Obsidian
            | Self::MatteBlack
            | Self::AdwaitaPastel
            | Self::Ashen
            | Self::Discord => ThemeMode::Dark,
        }
    }

    fn load() -> Self {
        std::fs::read_to_string(persist_path())
            .ok()
            .and_then(|raw| Self::parse(&raw))
            .unwrap_or(Self::Orbit)
    }

    fn persist(self) {
        let path = persist_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(path, self.as_str());
    }
}

/// Semantic palette. Copy so list closures and hover styles can capture it
/// without a lifetime.
#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub theme_id: ThemeId,
    pub mode: ThemeMode,
    /// UI customization (Waku's General settings): language + font sizes.
    pub ui: UiPrefs,
    pub bg_main: Hsla,
    pub bg_sidebar: Hsla,
    pub bg_composer: Hsla,
    pub bg_raised: Hsla,
    pub bg_hover: Hsla,
    /// Background for pressed/active and highlighted (selected) elements.
    pub active: Hsla,
    /// Foreground (text/icon) drawn on [`Self::active`].
    pub active_fg: Hsla,
    pub border: Hsla,
    pub text: Hsla,
    pub text_2: Hsla,
    pub text_3: Hsla,
    pub ok_green: Hsla,
    pub stop_red: Hsla,
    pub stop_red_hover: Hsla,
    pub add_green: Hsla,
    pub del_red: Hsla,
    /// Single brand accent — selection, links, spark, checks, caret, etc.
    pub accent: Hsla,
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
    /// Syntax token colors (transcript code blocks + the review diff). A
    /// palette sets these explicitly so a ported editor theme keeps its own
    /// syntax identity instead of borrowing the UI's semantic roles.
    pub syn_string: Hsla,
    pub syn_number: Hsla,
    pub syn_function: Hsla,
    pub syn_type: Hsla,
    pub syn_comment: Hsla,
    pub syn_literal: Hsla,
    pub syn_meta: Hsla,
    pub syn_operator: Hsla,
    /// Inline-code chip wash (text uses [`Self::accent`]).
    pub inline_code_bg: Hsla,
    pub tool_border: Hsla,
    pub tool_meta: Hsla,
    pub ring_track: Hsla,
    pub ring_fill: Hsla,
    pub warn: Hsla,
    pub crit: Hsla,
    pub trough: Hsla,
    pub shadow_contact: Hsla,
    pub shadow_ambient: Hsla,
}

impl Global for Theme {}

/// Interface language for the workbench chrome. English strings ship
/// today; `System` follows macOS's preferred language once more locales
/// land (the setting persists either way).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    System,
    English,
}

impl Language {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::English => "en",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::System => "System",
            Self::English => "English",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim() {
            "system" => Some(Self::System),
            "en" => Some(Self::English),
            _ => None,
        }
    }
}

/// Waku General-settings customization: language + font sizes.
/// Sizes are px; defaults match Waku (14 UI / 13 code).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UiPrefs {
    pub language: Language,
    pub ui_font_size: f32,
    pub code_font_size: f32,
}

/// Selectable UI font sizes (`13 px` … `16 px` in the Waku dropdown).
pub const UI_FONT_SIZES: [f32; 4] = [13., 14., 15., 16.];
/// Selectable code font sizes (`12 px` … `15 px`).
pub const CODE_FONT_SIZES: [f32; 4] = [12., 13., 14., 15.];

impl Default for UiPrefs {
    fn default() -> Self {
        Self {
            language: Language::System,
            ui_font_size: 14.,
            code_font_size: 13.,
        }
    }
}

impl UiPrefs {
    fn persist_path() -> PathBuf {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        home.join(".orbit-pi").join("ui.json")
    }

    fn load() -> Self {
        let Ok(raw) = std::fs::read_to_string(Self::persist_path()) else {
            return Self::default();
        };
        serde_json::from_str::<Value>(&raw)
            .map(|value| Self::from_value(&value))
            .unwrap_or_default()
    }

    /// Parse prefs from JSON, falling back to defaults per field and
    /// rejecting sizes outside the selectable ranges.
    fn from_value(value: &Value) -> Self {
        let mut prefs = Self::default();
        if let Some(language) = value
            .get("language")
            .and_then(Value::as_str)
            .and_then(Language::parse)
        {
            prefs.language = language;
        }
        if let Some(size) = value.get("ui_font_size").and_then(Value::as_f64) {
            if UI_FONT_SIZES.contains(&(size as f32)) {
                prefs.ui_font_size = size as f32;
            }
        }
        if let Some(size) = value.get("code_font_size").and_then(Value::as_f64) {
            if CODE_FONT_SIZES.contains(&(size as f32)) {
                prefs.code_font_size = size as f32;
            }
        }
        prefs
    }

    fn persist(self) {
        let path = Self::persist_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(
            path,
            serde_json::json!({
                "language": self.language.as_str(),
                "ui_font_size": self.ui_font_size,
                "code_font_size": self.code_font_size,
            })
            .to_string(),
        );
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::for_id(ThemeId::Orbit)
    }
}

/// Current UI / code font families. Stored as statics (readable without an
/// app handle) so transcript helpers, which don't carry `cx`, can look them
/// up — keeping [`Theme`] itself `Copy`. Persisted to `~/.orbit-pi/fonts.json`.
static UI_FONT_FAMILY: RwLock<SharedString> = RwLock::new(SharedString::new_static(".ZedSans"));
static CODE_FONT_FAMILY: RwLock<SharedString> = RwLock::new(SharedString::new_static(".ZedMono"));

/// The persisted font-families payload.
#[derive(Debug, Clone, PartialEq)]
pub struct FontPrefs {
    /// UI face family (`.ZedSans` → bundled IBM Plex Sans by default).
    pub ui_font_family: SharedString,
    /// Code/monospace face family (`.ZedMono` → bundled Lilex by default).
    pub code_font_family: SharedString,
}

impl Default for FontPrefs {
    fn default() -> Self {
        Self {
            ui_font_family: ".ZedSans".into(),
            code_font_family: ".ZedMono".into(),
        }
    }
}

impl FontPrefs {
    fn persist_path() -> PathBuf {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        home.join(".orbit-pi").join("fonts.json")
    }

    fn load() -> Self {
        let Ok(raw) = std::fs::read_to_string(Self::persist_path()) else {
            return Self::default();
        };
        let Ok(value) = serde_json::from_str::<Value>(&raw) else {
            return Self::default();
        };
        let mut prefs = Self::default();
        if let Some(family) = value.get("ui_font_family").and_then(Value::as_str) {
            if !family.is_empty() {
                prefs.ui_font_family = family.to_string().into();
            }
        }
        if let Some(family) = value.get("code_font_family").and_then(Value::as_str) {
            if !family.is_empty() {
                prefs.code_font_family = family.to_string().into();
            }
        }
        prefs
    }

    fn persist(&self) {
        let path = Self::persist_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(
            path,
            serde_json::json!({
                "ui_font_family": self.ui_font_family.to_string(),
                "code_font_family": self.code_font_family.to_string(),
            })
            .to_string(),
        );
    }
}

/// Current UI face family (readable without `cx`).
pub fn ui_font_family() -> SharedString {
    UI_FONT_FAMILY.read().unwrap().clone()
}

/// Current code/mono face family (readable without `cx`).
pub fn code_font_family() -> SharedString {
    CODE_FONT_FAMILY.read().unwrap().clone()
}

/// Current font families as a struct (for the settings UI).
pub fn font_prefs() -> FontPrefs {
    FontPrefs {
        ui_font_family: ui_font_family(),
        code_font_family: code_font_family(),
    }
}

/// The raw color tokens for one palette. Derived washes (hover overlay,
/// strong border, inline-code wash) and shadows are computed from these in
/// [`Theme::build`], so a palette only carries its actual colors.
#[derive(Clone, Copy)]
struct Palette {
    bg_main: u32,
    bg_sidebar: u32,
    bg_raised: u32,
    bg_hover: u32,
    active: u32,
    active_fg: u32,
    border: u32,
    text: u32,
    text_2: u32,
    text_3: u32,
    ok_green: u32,
    stop_red: u32,
    stop_red_hover: u32,
    add_green: u32,
    del_red: u32,
    accent: u32,
    menu_bg: u32,
    send_bg: u32,
    send_bg_hover: u32,
    send_fg: u32,
    assistant_text: u32,
    code_bg: u32,
    code_text: u32,
    syn_string: u32,
    syn_number: u32,
    syn_function: u32,
    syn_type: u32,
    syn_comment: u32,
    syn_literal: u32,
    syn_meta: u32,
    syn_operator: u32,
    tool_border: u32,
    tool_meta: u32,
    ring_track: u32,
    ring_fill: u32,
    warn: u32,
    crit: u32,
    trough: u32,
}

/// Waku dark: near-black canvas, light primary buttons, orange ring accent.
const ORBIT: Palette = Palette {
    bg_main: 0x1A1A1A,
    bg_sidebar: 0x181818,
    bg_raised: 0x232323,
    bg_hover: 0x262626,
    active: 0x2A2A2A,
    active_fg: 0xE2E2E2,
    border: 0x2A2A2A,
    text: 0xE2E2E2,
    text_2: 0xA3A3A3,
    text_3: 0x7D7D7D,
    ok_green: 0x62C987,
    stop_red: 0xE2726A,
    stop_red_hover: 0xEC8A82,
    add_green: 0x62C987,
    del_red: 0xE2726A,
    accent: 0xE2795B,
    menu_bg: 0x232323,
    send_bg: 0xE7E9EC,
    send_bg_hover: 0xF2F3F6,
    send_fg: 0x17181C,
    assistant_text: 0xE2E2E2,
    code_bg: 0x151515,
    code_text: 0xE2E2E2,
    // Syntax is semantic here: strings green, numbers amber, meta red.
    syn_string: 0x62C987,
    syn_number: 0xE0B36A,
    syn_function: 0xE2E2E2,
    syn_type: 0xE2E2E2,
    syn_comment: 0xA3A3A3,
    syn_literal: 0xE2795B,
    syn_meta: 0xE2726A,
    syn_operator: 0x7D7D7D,
    tool_border: 0x2A2A2A,
    tool_meta: 0xA3A3A3,
    ring_track: 0x232323,
    ring_fill: 0xE2E2E2,
    warn: 0xE0B36A,
    crit: 0xE2726A,
    trough: 0x232323,
};

/// Waku light: warm off-white canvas, dark primary buttons, orange accent.
const ORBIT_LIGHT: Palette = Palette {
    bg_main: 0xF6F5F6,
    bg_sidebar: 0xF3F3F3,
    bg_raised: 0xECECEC,
    bg_hover: 0xE8E8E8,
    active: 0xE2E2E2,
    active_fg: 0x242424,
    border: 0xE2E2E2,
    text: 0x242424,
    text_2: 0x666666,
    text_3: 0x858585,
    ok_green: 0x2F8F52,
    stop_red: 0xC64A42,
    stop_red_hover: 0xD5604F,
    add_green: 0x2F8F52,
    del_red: 0xC64A42,
    accent: 0xC85F44,
    menu_bg: 0xECECEC,
    send_bg: 0x202227,
    send_bg_hover: 0x34363C,
    send_fg: 0xF8F8F9,
    assistant_text: 0x242424,
    code_bg: 0xE6E6E6,
    code_text: 0x242424,
    syn_string: 0x2F8F52,
    syn_number: 0xA66B20,
    syn_function: 0x242424,
    syn_type: 0x242424,
    syn_comment: 0x666666,
    syn_literal: 0xC85F44,
    syn_meta: 0xC64A42,
    syn_operator: 0x858585,
    tool_border: 0xE2E2E2,
    tool_meta: 0x666666,
    ring_track: 0xECECEC,
    ring_fill: 0x242424,
    warn: 0xA66B20,
    crit: 0xC64A42,
    trough: 0xECECEC,
};

/// Vague (dark): a low-contrast, near-monochrome editor palette ported from
/// the Zed theme `Vague` (<https://github.com/vague-theme/vague-zed>), tuned
/// to Orbit's semantic roles. Muted blue is the lone accent; syntax uses the
/// theme's own token colors rather than the UI's greens/ambers.
const VAGUE: Palette = Palette {
    bg_main: 0x141415,
    bg_sidebar: 0x141415,
    bg_raised: 0x252530,
    bg_hover: 0x1C1C24,
    active: 0x252530,
    active_fg: 0xCDCDCD,
    border: 0x252530,
    text: 0xCDCDCD,
    text_2: 0x878787,
    text_3: 0x606079,
    ok_green: 0x7FA563,
    stop_red: 0xD8647E,
    stop_red_hover: 0xE08398,
    add_green: 0x7FA563,
    del_red: 0xD8647E,
    accent: 0x6E94B2,
    menu_bg: 0x252530,
    send_bg: 0xCDCDCD,
    send_bg_hover: 0xD7D7D7,
    send_fg: 0x141415,
    assistant_text: 0xCDCDCD,
    code_bg: 0x18181F,
    code_text: 0xCDCDCD,
    syn_string: 0xE8B589,
    syn_number: 0xE0A363,
    syn_function: 0xC48282,
    syn_type: 0x9BB4BC,
    syn_comment: 0x606079,
    syn_literal: 0xE0A363,
    syn_meta: 0xAEAED1,
    syn_operator: 0x90A0B5,
    tool_border: 0x252530,
    tool_meta: 0x878787,
    ring_track: 0x252530,
    ring_fill: 0xCDCDCD,
    warn: 0xF3BE7C,
    crit: 0xD8647E,
    trough: 0x252530,
};

/// Batsignal (Dark): ported from the Zed community theme of the same name
/// (via zed-themes.com). Dark appearance.
const BATSIGNAL: Palette = Palette {
    bg_main: 0x000000,
    bg_sidebar: 0x000000,
    bg_raised: 0x0F0F0F,
    bg_hover: 0x0F0F0F,
    active: 0x0F0F0F,
    active_fg: 0xB3B3B3,
    border: 0x121212,
    text: 0xB3B3B3,
    text_2: 0x767676,
    text_3: 0x5E5E5E,
    ok_green: 0x62C987,
    stop_red: 0xF44747,
    stop_red_hover: 0xF66868,
    add_green: 0x62C987,
    del_red: 0xF44747,
    accent: 0xFFFF00,
    menu_bg: 0x0F0F0F,
    send_bg: 0xB3B3B3,
    send_bg_hover: 0x989898,
    send_fg: 0x000000,
    assistant_text: 0xB3B3B3,
    code_bg: 0x070707,
    code_text: 0xB3B3B3,
    syn_string: 0xAAAAAA,
    syn_number: 0xAAAAAA,
    syn_function: 0xFFFF00,
    syn_type: 0xB3B3B3,
    syn_comment: 0x606060,
    syn_literal: 0xAAAAAA,
    syn_meta: 0x777777,
    syn_operator: 0xB3B3B3,
    tool_border: 0x121212,
    tool_meta: 0x767676,
    ring_track: 0x0F0F0F,
    ring_fill: 0xB3B3B3,
    warn: 0xCD9731,
    crit: 0xF44747,
    trough: 0x0F0F0F,
};

/// Ashwood: ported from the Zed community theme of the same name
/// (via zed-themes.com). Dark appearance.
const ASHWOOD: Palette = Palette {
    bg_main: 0x111111,
    bg_sidebar: 0x151515,
    bg_raised: 0x191919,
    bg_hover: 0x232323,
    active: 0x594242,
    active_fg: 0xE1E1E1,
    border: 0x3A3A3A,
    text: 0xA9A9A9,
    text_2: 0x7E7E7E,
    text_3: 0x656565,
    ok_green: 0x62C987,
    stop_red: 0xC4909A,
    stop_red_hover: 0xCFA4AC,
    add_green: 0x62C987,
    del_red: 0xC4909A,
    accent: 0xAAAAAA,
    menu_bg: 0x191919,
    send_bg: 0xA9A9A9,
    send_bg_hover: 0x909090,
    send_fg: 0x111111,
    assistant_text: 0xA9A9A9,
    code_bg: 0x161616,
    code_text: 0xA9A9A9,
    syn_string: 0x8DBBA3,
    syn_number: 0xABB6E0,
    syn_function: 0xC0B0DF,
    syn_type: 0xABB6E0,
    syn_comment: 0x8D909C,
    syn_literal: 0xABB6E0,
    syn_meta: 0xC0B0DF,
    syn_operator: 0xDEA8B3,
    tool_border: 0x3A3A3A,
    tool_meta: 0x7E7E7E,
    ring_track: 0x191919,
    ring_fill: 0xA9A9A9,
    warn: 0xE99696,
    crit: 0xC4909A,
    trough: 0x191919,
};

/// Obsidian Dark: ported from the Zed community theme of the same name
/// (via zed-themes.com). Dark appearance.
const OBSIDIAN: Palette = Palette {
    bg_main: 0x161616,
    bg_sidebar: 0x101010,
    bg_raised: 0x222222,
    bg_hover: 0x282828,
    active: 0x2E2E2E,
    active_fg: 0xDEDEDE,
    border: 0x222222,
    text: 0xDEDEDE,
    text_2: 0x888888,
    text_3: 0x686868,
    ok_green: 0x3DBA6F,
    stop_red: 0xD96B6B,
    stop_red_hover: 0xE08686,
    add_green: 0x3DBA6F,
    del_red: 0xD96B6B,
    accent: 0x3DBA6F,
    menu_bg: 0x222222,
    send_bg: 0xDEDEDE,
    send_bg_hover: 0xBDBDBD,
    send_fg: 0x161616,
    assistant_text: 0xDEDEDE,
    code_bg: 0x101010,
    code_text: 0xDEDEDE,
    syn_string: 0x5AAD7A,
    syn_number: 0x3DBA6F,
    syn_function: 0xE2E2E2,
    syn_type: 0xAAAAAA,
    syn_comment: 0x646464,
    syn_literal: 0x3DBA6F,
    syn_meta: 0x3DBA6F,
    syn_operator: 0x888888,
    tool_border: 0x222222,
    tool_meta: 0x888888,
    ring_track: 0x222222,
    ring_fill: 0xDEDEDE,
    warn: 0xC8964A,
    crit: 0xD96B6B,
    trough: 0x222222,
};

/// Matte Black: ported from the Zed community theme of the same name
/// (via zed-themes.com). Dark appearance.
const MATTE_BLACK: Palette = Palette {
    bg_main: 0x1A1A1A,
    bg_sidebar: 0x1A1A1A,
    bg_raised: 0x252525,
    bg_hover: 0x3A3A3A,
    active: 0x3A3A3A,
    active_fg: 0xE0E0E0,
    border: 0x2A2A2A,
    text: 0xE0E0E0,
    text_2: 0x888888,
    text_3: 0x6A6A6A,
    ok_green: 0x69F0AE,
    stop_red: 0xFF5252,
    stop_red_hover: 0xFF7171,
    add_green: 0x69F0AE,
    del_red: 0xFF5252,
    accent: 0x40C4FF,
    menu_bg: 0x252525,
    send_bg: 0xE0E0E0,
    send_bg_hover: 0xBEBEBE,
    send_fg: 0x1A1A1A,
    assistant_text: 0xE0E0E0,
    code_bg: 0x252525,
    code_text: 0xE0E0E0,
    syn_string: 0xC3E88D,
    syn_number: 0xFF7043,
    syn_function: 0x0D7FD8,
    syn_type: 0xD4A574,
    syn_comment: 0x737373,
    syn_literal: 0xFF9E80,
    syn_meta: 0x80D8FF,
    syn_operator: 0xD0D0D0,
    tool_border: 0x2A2A2A,
    tool_meta: 0x888888,
    ring_track: 0x252525,
    ring_fill: 0xE0E0E0,
    warn: 0xFFD740,
    crit: 0xFF5252,
    trough: 0x252525,
};

/// Adwaita Pastel Dark: ported from the Zed community theme of the same name
/// (via zed-themes.com). Dark appearance.
const ADWAITA_PASTEL: Palette = Palette {
    bg_main: 0x1E1E1E,
    bg_sidebar: 0x303030,
    bg_raised: 0x303030,
    bg_hover: 0x444444,
    active: 0x444444,
    active_fg: 0xE7E7E7,
    border: 0x4F4F4F,
    text: 0xE7E7E7,
    text_2: 0xC7C7C7,
    text_3: 0x808080,
    ok_green: 0x57E389,
    stop_red: 0xED333B,
    stop_red_hover: 0xF0585E,
    add_green: 0x57E389,
    del_red: 0xED333B,
    accent: 0x1E78E4,
    menu_bg: 0x303030,
    send_bg: 0xE7E7E7,
    send_bg_hover: 0xC4C4C4,
    send_fg: 0x1E1E1E,
    assistant_text: 0xE7E7E7,
    code_bg: 0x303030,
    code_text: 0xE7E7E7,
    syn_string: 0xA6E3A1,
    syn_number: 0xFAB387,
    syn_function: 0x89B4FA,
    syn_type: 0xF9E2AF,
    syn_comment: 0x7F849C,
    syn_literal: 0xFAB387,
    syn_meta: 0xF9E2AF,
    syn_operator: 0x89DCEB,
    tool_border: 0x4F4F4F,
    tool_meta: 0xC7C7C7,
    ring_track: 0x303030,
    ring_fill: 0xE7E7E7,
    warn: 0xF8E45C,
    crit: 0xED333B,
    trough: 0x303030,
};

/// Ashen: ported from the Zed community theme of the same name
/// (via zed-themes.com). Dark appearance.
const ASHEN: Palette = Palette {
    bg_main: 0x121212,
    bg_sidebar: 0x121212,
    bg_raised: 0x212121,
    bg_hover: 0x323232,
    active: 0x323232,
    active_fg: 0xC0C0C0,
    border: 0x323232,
    text: 0xB4B4B4,
    text_2: 0x949494,
    text_3: 0x656565,
    ok_green: 0x629C7D,
    stop_red: 0xC53030,
    stop_red_hover: 0xCF5555,
    add_green: 0x629C7D,
    del_red: 0xC53030,
    accent: 0xDF6464,
    menu_bg: 0x212121,
    send_bg: 0xB4B4B4,
    send_bg_hover: 0x999999,
    send_fg: 0x121212,
    assistant_text: 0xB4B4B4,
    code_bg: 0x151515,
    code_text: 0xB4B4B4,
    syn_string: 0xDF6464,
    syn_number: 0x4A8B8B,
    syn_function: 0xE5E5E5,
    syn_type: 0xC4693D,
    syn_comment: 0x737373,
    syn_literal: 0x4A8B8B,
    syn_meta: 0xA7A7A7,
    syn_operator: 0xD87C4A,
    tool_border: 0x323232,
    tool_meta: 0x949494,
    ring_track: 0x212121,
    ring_fill: 0xB4B4B4,
    warn: 0xE5A72A,
    crit: 0xC53030,
    trough: 0x212121,
};

/// Discord Dark: ported from the Zed community theme of the same name
/// (via zed-themes.com). Dark appearance.
const DISCORD: Palette = Palette {
    bg_main: 0x121214,
    bg_sidebar: 0x121214,
    bg_raised: 0x1A1A1E,
    bg_hover: 0x242428,
    active: 0x242428,
    active_fg: 0xB5B4B4,
    border: 0x222225,
    text: 0xB5B4B4,
    text_2: 0x959DA5,
    text_3: 0x6D6D73,
    ok_green: 0x4D9375,
    stop_red: 0xCB7676,
    stop_red_hover: 0xD48F8F,
    add_green: 0x4D9375,
    del_red: 0xCB7676,
    accent: 0x5197ED,
    menu_bg: 0x1A1A1E,
    send_bg: 0xB5B4B4,
    send_bg_hover: 0x9A9999,
    send_fg: 0x121214,
    assistant_text: 0xB5B4B4,
    code_bg: 0x1A1A1E,
    code_text: 0xB5B4B4,
    syn_string: 0xC98A7D,
    syn_number: 0x4C9A91,
    syn_function: 0x80A665,
    syn_type: 0x5D99A9,
    syn_comment: 0x687668,
    syn_literal: 0x4D9375,
    syn_meta: 0xB8A965,
    syn_operator: 0xCB7676,
    tool_border: 0x222225,
    tool_meta: 0x959DA5,
    ring_track: 0x1A1A1E,
    ring_fill: 0xB5B4B4,
    warn: 0xE6CC77,
    crit: 0xCB7676,
    trough: 0x1A1A1E,
};

fn palette(id: ThemeId) -> Palette {
    match id {
        ThemeId::Orbit => ORBIT,
        ThemeId::OrbitLight => ORBIT_LIGHT,
        ThemeId::Vague => VAGUE,
        ThemeId::Batsignal => BATSIGNAL,
        ThemeId::Ashwood => ASHWOOD,
        ThemeId::Obsidian => OBSIDIAN,
        ThemeId::MatteBlack => MATTE_BLACK,
        ThemeId::AdwaitaPastel => ADWAITA_PASTEL,
        ThemeId::Ashen => ASHEN,
        ThemeId::Discord => DISCORD,
    }
}

impl Theme {
    pub fn for_id(id: ThemeId) -> Self {
        Self::build(id.appearance(), UiPrefs::default(), id, palette(id))
    }

    #[cfg(test)]
    pub fn dark() -> Self {
        Self::for_id(ThemeId::Orbit)
    }

    #[cfg(test)]
    pub fn light() -> Self {
        Self::for_id(ThemeId::OrbitLight)
    }

    /// Keep the current UI customization across a palette switch.
    pub fn with_ui(mut self, ui: UiPrefs) -> Self {
        self.ui = ui;
        self
    }

    /// Scale an interface text size by the UI font-size setting
    /// (`ui_font_size / 14`, the Waku default).
    pub fn ui_px(&self, value: f32) -> Pixels {
        px(value * (self.ui.ui_font_size / 14.))
    }

    /// Scale a code-surface size (code blocks, diffs, tool output) by the
    /// code font-size setting (`code_font_size / 13`, the Waku default).
    pub fn code_px(&self, value: f32) -> Pixels {
        px(value * (self.ui.code_font_size / 13.))
    }

    /// Paint color for one syntax token, shared by transcript code blocks
    /// and the review diff so both read as the same editor. Each palette
    /// carries its own `syn_*` colors (so a ported editor theme keeps its
    /// syntax identity); comments are explicitly legible on the code wash.
    pub fn token_color(self, class: TokenClass) -> Hsla {
        match class {
            TokenClass::Keyword => self.accent,
            TokenClass::Literal => self.syn_literal,
            TokenClass::String => self.syn_string,
            TokenClass::Comment => self.syn_comment,
            TokenClass::Number => self.syn_number,
            TokenClass::Type => self.syn_type,
            TokenClass::Function => self.syn_function,
            TokenClass::Meta => self.syn_meta,
            // Terminal vocabulary: command, flag, path, control operator.
            TokenClass::Command => self.accent,
            TokenClass::Flag => self.syn_number,
            TokenClass::Path => self.text,
            TokenClass::Operator => self.syn_operator,
            TokenClass::Added => self.add_green,
            TokenClass::Removed => self.del_red,
        }
    }

    /// Build a full [`Theme`] from its appearance + raw palette tokens.
    fn build(mode: ThemeMode, ui: UiPrefs, id: ThemeId, p: Palette) -> Self {
        let text = hex(p.text);
        let accent = hex(p.accent);
        let (wash, wash_strong, border_strong, contact, ambient) = match mode {
            ThemeMode::Dark => (0.05, 0.09, 0.14, 0.32, 0.40),
            ThemeMode::Light => (0.06, 0.10, 0.16, 0.10, 0.14),
        };
        Self {
            theme_id: id,
            mode,
            ui,
            bg_main: hex(p.bg_main),
            bg_sidebar: hex(p.bg_sidebar),
            bg_composer: hex(p.bg_sidebar),
            bg_raised: hex(p.bg_raised),
            bg_hover: hex(p.bg_hover),
            active: hex(p.active),
            active_fg: hex(p.active_fg),
            border: hex(p.border),
            text,
            text_2: hex(p.text_2),
            text_3: hex(p.text_3),
            ok_green: hex(p.ok_green),
            stop_red: hex(p.stop_red),
            stop_red_hover: hex(p.stop_red_hover),
            add_green: hex(p.add_green),
            del_red: hex(p.del_red),
            accent,
            menu_bg: hex(p.menu_bg),
            send_bg: hex(p.send_bg),
            send_bg_hover: hex(p.send_bg_hover),
            send_fg: hex(p.send_fg),
            overlay: text.opacity(wash),
            overlay_strong: text.opacity(wash_strong),
            border_strong: text.opacity(border_strong),
            assistant_text: hex(p.assistant_text),
            code_bg: hex(p.code_bg),
            code_text: hex(p.code_text),
            syn_string: hex(p.syn_string),
            syn_number: hex(p.syn_number),
            syn_function: hex(p.syn_function),
            syn_type: hex(p.syn_type),
            syn_comment: hex(p.syn_comment),
            syn_literal: hex(p.syn_literal),
            syn_meta: hex(p.syn_meta),
            syn_operator: hex(p.syn_operator),
            inline_code_bg: accent.opacity(0.10),
            tool_border: hex(p.tool_border),
            tool_meta: hex(p.tool_meta),
            ring_track: hex(p.ring_track),
            ring_fill: hex(p.ring_fill),
            warn: hex(p.warn),
            crit: hex(p.crit),
            trough: hex(p.trough),
            shadow_contact: hsla(0., 0., 0., contact),
            shadow_ambient: hsla(0., 0., 0., ambient),
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

    /// Soft lift for the floating composer: one tight contact layer. The
    /// hairline border carries definition, so no wide ambient here (a
    /// border under a broad shadow reads as a ghost card).
    pub fn composer_shadow(self) -> Vec<BoxShadow> {
        vec![BoxShadow {
            color: self.shadow_contact,
            offset: point(px(0.), px(2.)),
            blur_radius: px(10.),
            spread_radius: px(-3.),
        }]
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

/// Install the persisted (or default Orbit) theme as a GPUI global and
/// load the persisted font families into the statics.
pub fn init(cx: &mut App) {
    cx.set_global(Theme::for_id(ThemeId::load()).with_ui(UiPrefs::load()));
    let prefs = FontPrefs::load();
    *UI_FONT_FAMILY.write().unwrap() = prefs.ui_font_family;
    *CODE_FONT_FAMILY.write().unwrap() = prefs.code_font_family;
}

/// Current theme. Panics if [`init`] has not run.
pub fn get(cx: &App) -> &Theme {
    cx.global::<Theme>()
}

/// Cached list of available font families. Font availability doesn't change
/// mid-run (bundled faces are registered at startup), so the CoreText
/// enumeration happens once instead of on every settings render — the
/// settings panel re-renders on every dropdown scroll tick, and enumerating
/// all system fonts per frame is what made the theme/font selectors lag.
static AVAILABLE_FONTS: OnceLock<Vec<String>> = OnceLock::new();

/// Available font families (installed + bundled Zed faces) for the settings
/// dropdowns.
pub fn available_fonts(cx: &App) -> Vec<String> {
    AVAILABLE_FONTS
        .get_or_init(|| cx.text_system().all_font_names())
        .clone()
}

/// Switch to a palette, persist the choice, and notify global observers.
pub fn set_theme(cx: &mut App, id: ThemeId) {
    if get(cx).theme_id == id {
        return;
    }
    id.persist();
    // The palette switches; UI customization carries over.
    let ui = get(cx).ui;
    cx.set_global(Theme::for_id(id).with_ui(ui));
}

/// Apply new UI / code font families to the statics and persist them.
pub fn set_font_prefs(prefs: FontPrefs) {
    *UI_FONT_FAMILY.write().unwrap() = prefs.ui_font_family.clone();
    *CODE_FONT_FAMILY.write().unwrap() = prefs.code_font_family.clone();
    prefs.persist();
}

/// Apply new UI customization (language / font sizes), persist it, and
/// notify global observers — every paint site reads sizes through the
/// theme, so the change is live.
pub fn set_ui_prefs(cx: &mut App, ui: UiPrefs) {
    if get(cx).ui == ui {
        return;
    }
    ui.persist();
    let id = get(cx).theme_id;
    cx.set_global(Theme::for_id(id).with_ui(ui));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_theme_id() {
        assert_eq!(ThemeId::parse("orbit"), Some(ThemeId::Orbit));
        assert_eq!(ThemeId::parse("vague"), Some(ThemeId::Vague));
        assert_eq!(ThemeId::parse("orbit-light"), Some(ThemeId::OrbitLight));
        assert_eq!(ThemeId::parse("system"), None);
        assert_eq!(ThemeId::Vague.as_str(), "vague");
        assert_eq!(ThemeId::Vague.appearance(), ThemeMode::Dark);
        // Legacy theme names map onto Orbit / Orbit Light.
        assert_eq!(ThemeId::parse("dark"), Some(ThemeId::Orbit));
        assert_eq!(ThemeId::parse("light"), Some(ThemeId::OrbitLight));
        assert_eq!(ThemeId::parse("one-dark"), Some(ThemeId::Orbit));
        assert_eq!(ThemeId::parse("one-light"), Some(ThemeId::OrbitLight));
        assert_eq!(ThemeId::parse("zedokai"), Some(ThemeId::Orbit));
        assert_eq!(ThemeId::parse("flexoki-dark"), Some(ThemeId::Orbit));
        assert_eq!(ThemeId::parse("flexoki-light"), Some(ThemeId::OrbitLight));
        // Ported Zed themes + their aliases.
        assert_eq!(ThemeId::parse("batsignal-dark"), Some(ThemeId::Batsignal));
        assert_eq!(ThemeId::parse("batsignal"), Some(ThemeId::Batsignal));
        assert_eq!(ThemeId::parse("ashwood"), Some(ThemeId::Ashwood));
        assert_eq!(ThemeId::parse("obsidian-dark"), Some(ThemeId::Obsidian));
        assert_eq!(ThemeId::parse("obsidian"), Some(ThemeId::Obsidian));
        assert_eq!(ThemeId::parse("matte-black"), Some(ThemeId::MatteBlack));
        assert_eq!(ThemeId::parse("matte-black-theme"), Some(ThemeId::MatteBlack));
        assert_eq!(
            ThemeId::parse("adwaita-pastel-dark"),
            Some(ThemeId::AdwaitaPastel)
        );
        assert_eq!(ThemeId::parse("ashen"), Some(ThemeId::Ashen));
        assert_eq!(ThemeId::parse("discord-dark"), Some(ThemeId::Discord));
        // Every selectable theme round-trips through its persisted key.
        for id in ThemeId::ALL {
            assert_eq!(ThemeId::parse(id.as_str()), Some(id));
        }
    }

    #[test]
    fn ported_zed_themes_are_dark_and_labelled() {
        let ported = [
            ThemeId::Batsignal,
            ThemeId::Ashwood,
            ThemeId::Obsidian,
            ThemeId::MatteBlack,
            ThemeId::AdwaitaPastel,
            ThemeId::Ashen,
            ThemeId::Discord,
        ];
        for id in ported {
            assert_eq!(id.appearance(), ThemeMode::Dark);
            assert_eq!(Theme::for_id(id).mode, ThemeMode::Dark);
            assert!(!id.label().is_empty());
        }
        assert_eq!(ThemeId::Batsignal.label(), "Batsignal (Dark)");
        assert_eq!(ThemeId::Obsidian.label(), "Obsidian Dark");
        assert_eq!(ThemeId::AdwaitaPastel.label(), "Adwaita Pastel Dark");
        assert_eq!(ThemeId::Discord.label(), "Discord Dark");
    }

    /// Every palette must keep its foreground readable against the surfaces
    /// it is painted on — the ported Zed themes are normalised into the same
    /// contrast band as Orbit so none reads as washed out or blown out.
    #[test]
    fn every_theme_has_readable_foregrounds() {
        fn rel_lum(v: u32) -> f64 {
            let f = |c: u32| {
                let c = c as f64 / 255.;
                if c <= 0.03928 {
                    c / 12.92
                } else {
                    ((c + 0.055) / 1.055).powf(2.4)
                }
            };
            0.2126 * f((v >> 16) & 0xff) + 0.7152 * f((v >> 8) & 0xff) + 0.0722 * f(v & 0xff)
        }
        fn contrast(a: u32, b: u32) -> f64 {
            let (l1, l2) = (rel_lum(a), rel_lum(b));
            let (hi, lo) = if l1 > l2 { (l1, l2) } else { (l2, l1) };
            (hi + 0.05) / (lo + 0.05)
        }
        for id in ThemeId::ALL {
            let p = palette(id);
            assert!(contrast(p.text, p.bg_main) >= 4.5, "{id:?}: body text on bg");
            assert!(
                contrast(p.text, p.bg_raised) >= 3.5,
                "{id:?}: body text on raised"
            );
            assert!(contrast(p.active_fg, p.active) >= 4.5, "{id:?}: active row");
            assert!(contrast(p.text_2, p.bg_main) >= 3.0, "{id:?}: secondary text");
            assert!(contrast(p.text_3, p.bg_main) >= 2.5, "{id:?}: tertiary text");
            assert!(contrast(p.send_fg, p.send_bg) >= 4.5, "{id:?}: send button");
        }
    }

    #[test]
    fn theme_appearance_matches_label() {
        assert_eq!(ThemeId::Orbit.appearance(), ThemeMode::Dark);
        assert_eq!(ThemeId::OrbitLight.appearance(), ThemeMode::Light);
        assert_eq!(ThemeId::Orbit.label(), "Orbit");
        assert_eq!(ThemeId::OrbitLight.label(), "Orbit Light");
    }

    #[test]
    fn parse_language() {
        assert_eq!(Language::parse("system"), Some(Language::System));
        assert_eq!(Language::parse("en"), Some(Language::English));
        assert_eq!(Language::parse("fr"), None);
        assert_eq!(Language::English.label(), "English");
    }

    #[test]
    fn ui_prefs_parse_rejects_out_of_range_sizes() {
        use serde_json::json;
        let prefs = UiPrefs::from_value(&json!({
            "language": "en",
            "ui_font_size": 22,
            "code_font_size": 15,
        }));
        // 22 px isn't selectable — falls back to the default; 15 is valid.
        assert_eq!(prefs.language, Language::English);
        assert_eq!(prefs.ui_font_size, 14.);
        assert_eq!(prefs.code_font_size, 15.);
        // Malformed payload → all defaults.
        assert_eq!(
            UiPrefs::from_value(&json!({"language": 3})),
            UiPrefs::default()
        );
    }

    #[test]
    fn font_scale_helpers() {
        let mut theme = Theme::dark();
        // Defaults are the Waku values — no scaling.
        assert_eq!(theme.ui_px(14.), px(14.));
        assert_eq!(theme.code_px(13.), px(13.));
        theme.ui.ui_font_size = 16.;
        theme.ui.code_font_size = 12.;
        assert_eq!(theme.ui_px(14.), px(16.));
        assert_eq!(theme.code_px(13.), px(12.));
        // A palette switch carries UI customization over.
        let light = Theme::for_id(ThemeId::OrbitLight).with_ui(theme.ui);
        assert_eq!(light.theme_id, ThemeId::OrbitLight);
        assert_eq!(light.ui, theme.ui);
    }

    #[test]
    fn palettes_differ() {
        let dark = Theme::dark();
        let light = Theme::light();
        assert_ne!(dark.bg_main, light.bg_main);
        assert_ne!(dark.text, light.text);
        assert_eq!(dark.theme_id, ThemeId::Orbit);
        assert_eq!(light.theme_id, ThemeId::OrbitLight);
        // Every palette is distinct from the default.
        for id in ThemeId::ALL {
            assert_eq!(Theme::for_id(id).theme_id, id);
        }
        // Orbit is dark; Orbit Light is light.
        for id in ThemeId::ALL {
            assert_eq!(
                Theme::for_id(id).mode,
                if id == ThemeId::OrbitLight {
                    ThemeMode::Light
                } else {
                    ThemeMode::Dark
                }
            );
        }
    }

    #[test]
    fn vague_keeps_its_own_syntax_palette() {
        let vague = Theme::for_id(ThemeId::Vague);
        let orbit = Theme::dark();
        assert_eq!(vague.mode, ThemeMode::Dark);
        // Vague's syntax is its own identity — warm strings, muted-blue
        // keywords — not Orbit's semantic green/amber reuse.
        assert_ne!(
            vague.token_color(TokenClass::String),
            orbit.token_color(TokenClass::String)
        );
        assert_eq!(vague.token_color(TokenClass::String), hex(0xE8B589));
        assert_eq!(vague.token_color(TokenClass::Keyword), hex(0x6E94B2));
        assert_eq!(vague.token_color(TokenClass::Function), hex(0xC48282));
        assert_eq!(vague.token_color(TokenClass::Type), hex(0x9BB4BC));
    }
}
