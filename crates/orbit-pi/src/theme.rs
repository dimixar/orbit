//! App theme: a GPUI [`Global`] with a selectable palette.
//!
//! Colors are semantic roles (backgrounds, text, borders, transcript
//! surfaces) rather than raw steps, so every paint site reads from one
//! place. A [`ThemeId`] names a Zed-compatible palette (Zed's own `One
//! Dark`/`One Light`, plus the zedokai Monokai variants); each has a fixed
//! appearance. The UI face is Zed's bundled IBM Plex Sans (`.ZedSans`);
//! code surfaces use Zed's Lilex (`.ZedMono`).

use std::path::PathBuf;
use std::sync::{OnceLock, RwLock};

use gpui::{hsla, point, px, rgb, App, BoxShadow, Global, Hsla, Pixels, SharedString};
use serde_json::Value;

/// Dark or light appearance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    Dark,
    Light,
}

/// One of the selectable Zed-compatible palettes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeId {
    OneDark,
    OneLight,
    Zedokai,
    ZedokaiDarker,
    ZedokaiFilterOctagon,
    ZedokaiDarkerFilterOctagon,
    ZedokaiFilterSpectrum,
    ZedokaiDarkerFilterSpectrum,
    FlexokiDark,
    FlexokiLight,
    Orbit,
    OrbitLight,
}

impl ThemeId {
    /// Selectable themes, in the order shown in the settings dropdown.
    pub const ALL: [ThemeId; 12] = [
        Self::OneDark,
        Self::OneLight,
        Self::Zedokai,
        Self::ZedokaiDarker,
        Self::ZedokaiFilterOctagon,
        Self::ZedokaiDarkerFilterOctagon,
        Self::ZedokaiFilterSpectrum,
        Self::ZedokaiDarkerFilterSpectrum,
        Self::FlexokiDark,
        Self::FlexokiLight,
        Self::Orbit,
        Self::OrbitLight,
    ];

    /// Persisted key; also accepts the legacy `dark`/`light` mode names.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OneDark => "one-dark",
            Self::OneLight => "one-light",
            Self::Zedokai => "zedokai",
            Self::ZedokaiDarker => "zedokai-darker",
            Self::ZedokaiFilterOctagon => "zedokai-filter-octagon",
            Self::ZedokaiDarkerFilterOctagon => "zedokai-darker-filter-octagon",
            Self::ZedokaiFilterSpectrum => "zedokai-filter-spectrum",
            Self::ZedokaiDarkerFilterSpectrum => "zedokai-darker-filter-spectrum",
            Self::FlexokiDark => "flexoki-dark",
            Self::FlexokiLight => "flexoki-light",
            Self::Orbit => "orbit",
            Self::OrbitLight => "orbit-light",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim() {
            // `dark`/`light` are the pre-theme persisted values.
            "one-dark" | "dark" => Some(Self::OneDark),
            "one-light" | "light" => Some(Self::OneLight),
            "zedokai" => Some(Self::Zedokai),
            "zedokai-darker" => Some(Self::ZedokaiDarker),
            "zedokai-filter-octagon" => Some(Self::ZedokaiFilterOctagon),
            "zedokai-darker-filter-octagon" => Some(Self::ZedokaiDarkerFilterOctagon),
            "zedokai-filter-spectrum" => Some(Self::ZedokaiFilterSpectrum),
            "zedokai-darker-filter-spectrum" => Some(Self::ZedokaiDarkerFilterSpectrum),
            "flexoki-dark" => Some(Self::FlexokiDark),
            "flexoki-light" => Some(Self::FlexokiLight),
            "orbit" => Some(Self::Orbit),
            "orbit-light" => Some(Self::OrbitLight),
            _ => None,
        }
    }

    /// Human label for the settings dropdown.
    pub fn label(self) -> &'static str {
        match self {
            Self::OneDark => "One Dark",
            Self::OneLight => "One Light",
            Self::Zedokai => "Zedokai",
            Self::ZedokaiDarker => "Zedokai Darker",
            Self::ZedokaiFilterOctagon => "Zedokai (Filter Octagon)",
            Self::ZedokaiDarkerFilterOctagon => "Zedokai Darker (Filter Octagon)",
            Self::ZedokaiFilterSpectrum => "Zedokai (Filter Spectrum)",
            Self::ZedokaiDarkerFilterSpectrum => "Zedokai Darker (Filter Spectrum)",
            Self::FlexokiDark => "Flexoki Dark",
            Self::FlexokiLight => "Flexoki Light",
            Self::Orbit => "Orbit",
            Self::OrbitLight => "Orbit Light",
        }
    }

    pub fn appearance(self) -> ThemeMode {
        match self {
            Self::OneLight | Self::FlexokiLight | Self::OrbitLight => ThemeMode::Light,
            _ => ThemeMode::Dark,
        }
    }

    fn load() -> Self {
        std::fs::read_to_string(persist_path())
            .ok()
            .and_then(|raw| Self::parse(&raw))
            .unwrap_or(Self::OneDark)
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
        Self::for_id(ThemeId::OneDark)
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
    tool_border: u32,
    tool_meta: u32,
    ring_track: u32,
    ring_fill: u32,
    warn: u32,
    crit: u32,
    trough: u32,
}

/// Zed `One Dark`: blue-gray canvas, editor panels, blue accent.
const ONE_DARK: Palette = Palette {
    bg_main: 0x3B414D,
    bg_sidebar: 0x2F343E,
    bg_raised: 0x2E343E,
    bg_hover: 0x363C46,
    active: 0x454A56,
    active_fg: 0xDCE0E5,
    border: 0x464B57,
    text: 0xDCE0E5,
    text_2: 0xA9AFBC,
    text_3: 0x878A98,
    ok_green: 0xA1C181,
    stop_red: 0xD07277,
    stop_red_hover: 0xE06C76,
    add_green: 0xA1C181,
    del_red: 0xD07277,
    accent: 0x74ADE8,
    menu_bg: 0x2F343E,
    send_bg: 0x2E343E,
    send_bg_hover: 0x363C46,
    send_fg: 0xDCE0E5,
    assistant_text: 0xDCE0E5,
    code_bg: 0x282C33,
    code_text: 0xACB2BE,
    tool_border: 0x363C46,
    tool_meta: 0xA9AFBC,
    ring_track: 0x363C46,
    ring_fill: 0xDCE0E5,
    warn: 0xDEC184,
    crit: 0xD07277,
    trough: 0x2E343E,
};

/// Zed `One Light`: warm off-white canvas, raised surfaces, blue accent.
const ONE_LIGHT: Palette = Palette {
    bg_main: 0xDCDCDD,
    bg_sidebar: 0xEBEBEC,
    bg_raised: 0xEBEBEC,
    bg_hover: 0xDFDFE0,
    active: 0xCACACA,
    active_fg: 0x242529,
    border: 0xC9C9CA,
    text: 0x242529,
    text_2: 0x58585A,
    text_3: 0x7E8086,
    ok_green: 0x669F59,
    stop_red: 0xD36151,
    stop_red_hover: 0xE05C4D,
    add_green: 0x669F59,
    del_red: 0xD36151,
    accent: 0x5C78E2,
    menu_bg: 0xEBEBEC,
    send_bg: 0xEBEBEC,
    send_bg_hover: 0xDFDFE0,
    send_fg: 0x242529,
    assistant_text: 0x242529,
    code_bg: 0xFAFAFA,
    code_text: 0x242529,
    tool_border: 0xDFDFE0,
    tool_meta: 0x58585A,
    ring_track: 0xDFDFE0,
    ring_fill: 0x242529,
    warn: 0xA48819,
    crit: 0xD36151,
    trough: 0xEBEBEC,
};

/// zedokai (Monokai Pro): warm gray canvas, yellow accent, pink/green hues.
const ZEDOKAI: Palette = Palette {
    bg_main: 0x2D2A2E,
    bg_sidebar: 0x333034,
    bg_raised: 0x403E41,
    bg_hover: 0x333034,
    active: 0x403E41,
    active_fg: 0xFCFCFA,
    border: 0x474448,
    text: 0xFCFCFA,
    text_2: 0x939293,
    text_3: 0x727072,
    ok_green: 0xA9DC76,
    stop_red: 0xFF6188,
    stop_red_hover: 0xFF7A94,
    add_green: 0xA9DC76,
    del_red: 0xFF6188,
    accent: 0xFFD866,
    menu_bg: 0x333034,
    send_bg: 0x403E41,
    send_bg_hover: 0x333034,
    send_fg: 0xFCFCFA,
    assistant_text: 0xFCFCFA,
    code_bg: 0x2D2A2E,
    code_text: 0xFCFCFA,
    tool_border: 0x474448,
    tool_meta: 0x939293,
    ring_track: 0x333034,
    ring_fill: 0xFCFCFA,
    warn: 0xFC9867,
    crit: 0xFF6188,
    trough: 0x333034,
};

/// zedokai Darker: Monokai Pro with near-black panels and borders.
const ZEDOKAI_DARKER: Palette = Palette {
    bg_main: 0x2D2A2E,
    bg_sidebar: 0x262427,
    bg_raised: 0x403E41,
    bg_hover: 0x262427,
    active: 0x403E41,
    active_fg: 0xFCFCFA,
    border: 0x19181A,
    text: 0xFCFCFA,
    text_2: 0x939293,
    text_3: 0x727072,
    ok_green: 0xA9DC76,
    stop_red: 0xFF6188,
    stop_red_hover: 0xFF7A94,
    add_green: 0xA9DC76,
    del_red: 0xFF6188,
    accent: 0xFFD866,
    menu_bg: 0x262427,
    send_bg: 0x403E41,
    send_bg_hover: 0x262427,
    send_fg: 0xFCFCFA,
    assistant_text: 0xFCFCFA,
    code_bg: 0x2D2A2E,
    code_text: 0xFCFCFA,
    tool_border: 0x19181A,
    tool_meta: 0x939293,
    ring_track: 0x262427,
    ring_fill: 0xFCFCFA,
    warn: 0xFC9867,
    crit: 0xFF6188,
    trough: 0x262427,
};

/// zedokai (Filter Octagon): cool blue-gray canvas, teal/green hues.
const ZEDOKAI_FILTER_OCTAGON: Palette = Palette {
    bg_main: 0x282A3A,
    bg_sidebar: 0x2E3040,
    bg_raised: 0x3A3D4B,
    bg_hover: 0x2E3040,
    active: 0x3A3D4B,
    active_fg: 0xEAF2F1,
    border: 0x424454,
    text: 0xEAF2F1,
    text_2: 0x888D94,
    text_3: 0x696D77,
    ok_green: 0xBAD761,
    stop_red: 0xFF657A,
    stop_red_hover: 0xFF7A8D,
    add_green: 0xBAD761,
    del_red: 0xFF657A,
    accent: 0xFFD76D,
    menu_bg: 0x2E3040,
    send_bg: 0x3A3D4B,
    send_bg_hover: 0x2E3040,
    send_fg: 0xEAF2F1,
    assistant_text: 0xEAF2F1,
    code_bg: 0x282A3A,
    code_text: 0xEAF2F1,
    tool_border: 0x424454,
    tool_meta: 0x888D94,
    ring_track: 0x2E3040,
    ring_fill: 0xEAF2F1,
    warn: 0xFF9B5E,
    crit: 0xFF657A,
    trough: 0x2E3040,
};

/// zedokai Darker (Filter Octagon): Octagon palette with darker panels.
const ZEDOKAI_DARKER_FILTER_OCTAGON: Palette = Palette {
    bg_main: 0x282A3A,
    bg_sidebar: 0x232532,
    bg_raised: 0x3A3D4B,
    bg_hover: 0x232532,
    active: 0x3A3D4B,
    active_fg: 0xEAF2F1,
    border: 0x181A23,
    text: 0xEAF2F1,
    text_2: 0x888D94,
    text_3: 0x696D77,
    ok_green: 0xBAD761,
    stop_red: 0xFF657A,
    stop_red_hover: 0xFF7A8D,
    add_green: 0xBAD761,
    del_red: 0xFF657A,
    accent: 0xFFD76D,
    menu_bg: 0x232532,
    send_bg: 0x3A3D4B,
    send_bg_hover: 0x232532,
    send_fg: 0xEAF2F1,
    assistant_text: 0xEAF2F1,
    code_bg: 0x282A3A,
    code_text: 0xEAF2F1,
    tool_border: 0x181A23,
    tool_meta: 0x888D94,
    ring_track: 0x232532,
    ring_fill: 0xEAF2F1,
    warn: 0xFF9B5E,
    crit: 0xFF657A,
    trough: 0x232532,
};

/// zedokai (Filter Spectrum): neutral gray canvas, warm yellow accent.
const ZEDOKAI_FILTER_SPECTRUM: Palette = Palette {
    bg_main: 0x222222,
    bg_sidebar: 0x282828,
    bg_raised: 0x363537,
    bg_hover: 0x282828,
    active: 0x363537,
    active_fg: 0xF7F1FF,
    border: 0x3C3C3C,
    text: 0xF7F1FF,
    text_2: 0x8B888F,
    text_3: 0x69676C,
    ok_green: 0x7BD88F,
    stop_red: 0xFC618D,
    stop_red_hover: 0xFC7B9E,
    add_green: 0x7BD88F,
    del_red: 0xFC618D,
    accent: 0xFCE566,
    menu_bg: 0x282828,
    send_bg: 0x363537,
    send_bg_hover: 0x282828,
    send_fg: 0xF7F1FF,
    assistant_text: 0xF7F1FF,
    code_bg: 0x222222,
    code_text: 0xF7F1FF,
    tool_border: 0x3C3C3C,
    tool_meta: 0x8B888F,
    ring_track: 0x282828,
    ring_fill: 0xF7F1FF,
    warn: 0xFD9353,
    crit: 0xFC618D,
    trough: 0x282828,
};

/// zedokai Darker (Filter Spectrum): Spectrum palette with darker panels.
const ZEDOKAI_DARKER_FILTER_SPECTRUM: Palette = Palette {
    bg_main: 0x222222,
    bg_sidebar: 0x1C1C1C,
    bg_raised: 0x363537,
    bg_hover: 0x1C1C1C,
    active: 0x363537,
    active_fg: 0xF7F1FF,
    border: 0x0F0F0F,
    text: 0xF7F1FF,
    text_2: 0x8B888F,
    text_3: 0x69676C,
    ok_green: 0x7BD88F,
    stop_red: 0xFC618D,
    stop_red_hover: 0xFC7B9E,
    add_green: 0x7BD88F,
    del_red: 0xFC618D,
    accent: 0xFCE566,
    menu_bg: 0x1C1C1C,
    send_bg: 0x363537,
    send_bg_hover: 0x1C1C1C,
    send_fg: 0xF7F1FF,
    assistant_text: 0xF7F1FF,
    code_bg: 0x222222,
    code_text: 0xF7F1FF,
    tool_border: 0x0F0F0F,
    tool_meta: 0x8B888F,
    ring_track: 0x1C1C1C,
    ring_fill: 0xF7F1FF,
    warn: 0xFD9353,
    crit: 0xFC618D,
    trough: 0x1C1C1C,
};

/// Flexoki Dark: warm paper-black canvas, teal accent (#3AA99F).
const FLEXOKI_DARK: Palette = Palette {
    bg_main: 0x100F0F,
    bg_sidebar: 0x1C1B1A,
    bg_raised: 0x1C1B1A,
    bg_hover: 0x282726,
    active: 0x343331,
    active_fg: 0xCECDC3,
    border: 0x282726,
    text: 0xCECDC3,
    text_2: 0x878580,
    text_3: 0x575653,
    ok_green: 0x879A39,
    stop_red: 0xD14D41,
    stop_red_hover: 0xF89A8A,
    add_green: 0x879A39,
    del_red: 0xD14D41,
    accent: 0x3AA99F,
    menu_bg: 0x1C1B1A,
    send_bg: 0x1C1B1A,
    send_bg_hover: 0x282726,
    send_fg: 0xCECDC3,
    assistant_text: 0xCECDC3,
    code_bg: 0x100F0F,
    code_text: 0xCECDC3,
    tool_border: 0x343331,
    tool_meta: 0x878580,
    ring_track: 0x282726,
    ring_fill: 0xCECDC3,
    warn: 0xD0A215,
    crit: 0xD14D41,
    trough: 0x1C1B1A,
};

/// Flexoki Light: warm paper-white canvas, dark ink, teal accent (#24837B).
const FLEXOKI_LIGHT: Palette = Palette {
    bg_main: 0xFFFCF0,
    bg_sidebar: 0xF2F0E5,
    bg_raised: 0xF2F0E5,
    bg_hover: 0xE6E4D9,
    active: 0xDAD8CE,
    active_fg: 0x100F0F,
    border: 0xE6E4D9,
    text: 0x100F0F,
    text_2: 0x6F6E69,
    text_3: 0xB7B5AC,
    ok_green: 0x66800B,
    stop_red: 0xAF3029,
    stop_red_hover: 0xD14D41,
    add_green: 0x66800B,
    del_red: 0xAF3029,
    accent: 0x24837B,
    menu_bg: 0xF2F0E5,
    send_bg: 0xF2F0E5,
    send_bg_hover: 0xE6E4D9,
    send_fg: 0x100F0F,
    assistant_text: 0x100F0F,
    code_bg: 0xFFFCF0,
    code_text: 0x100F0F,
    tool_border: 0xDAD8CE,
    tool_meta: 0x6F6E69,
    ring_track: 0xE6E4D9,
    ring_fill: 0x100F0F,
    warn: 0xAD8301,
    crit: 0xAF3029,
    trough: 0xF2F0E5,
};

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
    tool_border: 0xE2E2E2,
    tool_meta: 0x666666,
    ring_track: 0xECECEC,
    ring_fill: 0x242424,
    warn: 0xA66B20,
    crit: 0xC64A42,
    trough: 0xECECEC,
};

fn palette(id: ThemeId) -> Palette {
    match id {
        ThemeId::OneDark => ONE_DARK,
        ThemeId::OneLight => ONE_LIGHT,
        ThemeId::Zedokai => ZEDOKAI,
        ThemeId::ZedokaiDarker => ZEDOKAI_DARKER,
        ThemeId::ZedokaiFilterOctagon => ZEDOKAI_FILTER_OCTAGON,
        ThemeId::ZedokaiDarkerFilterOctagon => ZEDOKAI_DARKER_FILTER_OCTAGON,
        ThemeId::ZedokaiFilterSpectrum => ZEDOKAI_FILTER_SPECTRUM,
        ThemeId::ZedokaiDarkerFilterSpectrum => ZEDOKAI_DARKER_FILTER_SPECTRUM,
        ThemeId::FlexokiDark => FLEXOKI_DARK,
        ThemeId::FlexokiLight => FLEXOKI_LIGHT,
        ThemeId::Orbit => ORBIT,
        ThemeId::OrbitLight => ORBIT_LIGHT,
    }
}

impl Theme {
    pub fn for_id(id: ThemeId) -> Self {
        Self::build(id.appearance(), UiPrefs::default(), id, palette(id))
    }

    #[cfg(test)]
    pub fn dark() -> Self {
        Self::for_id(ThemeId::OneDark)
    }

    #[cfg(test)]
    pub fn light() -> Self {
        Self::for_id(ThemeId::OneLight)
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

/// Install the persisted (or default One Dark) theme as a GPUI global and
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
        assert_eq!(ThemeId::parse("one-dark"), Some(ThemeId::OneDark));
        assert_eq!(ThemeId::parse("  zedokai\n"), Some(ThemeId::Zedokai));
        assert_eq!(
            ThemeId::parse("zedokai-filter-octagon"),
            Some(ThemeId::ZedokaiFilterOctagon)
        );
        assert_eq!(
            ThemeId::parse("zedokai-darker-filter-spectrum"),
            Some(ThemeId::ZedokaiDarkerFilterSpectrum)
        );
        assert_eq!(ThemeId::parse("flexoki-dark"), Some(ThemeId::FlexokiDark));
        assert_eq!(ThemeId::parse("flexoki-light"), Some(ThemeId::FlexokiLight));
        assert_eq!(ThemeId::parse("orbit"), Some(ThemeId::Orbit));
        assert_eq!(ThemeId::parse("orbit-light"), Some(ThemeId::OrbitLight));
        assert_eq!(ThemeId::parse("system"), None);
        // Legacy mode names still resolve to the One palettes.
        assert_eq!(ThemeId::parse("dark"), Some(ThemeId::OneDark));
        assert_eq!(ThemeId::parse("light"), Some(ThemeId::OneLight));
    }

    #[test]
    fn theme_appearance_matches_label() {
        assert_eq!(ThemeId::OneDark.appearance(), ThemeMode::Dark);
        assert_eq!(ThemeId::OneLight.appearance(), ThemeMode::Light);
        assert_eq!(ThemeId::Zedokai.appearance(), ThemeMode::Dark);
        assert_eq!(ThemeId::ZedokaiDarker.appearance(), ThemeMode::Dark);
        assert_eq!(ThemeId::Zedokai.label(), "Zedokai");
        assert_eq!(ThemeId::ZedokaiDarker.label(), "Zedokai Darker");
        assert_eq!(
            ThemeId::ZedokaiFilterOctagon.label(),
            "Zedokai (Filter Octagon)"
        );
        assert_eq!(
            ThemeId::ZedokaiDarkerFilterSpectrum.label(),
            "Zedokai Darker (Filter Spectrum)"
        );
        assert_eq!(ThemeId::FlexokiDark.appearance(), ThemeMode::Dark);
        assert_eq!(ThemeId::FlexokiLight.appearance(), ThemeMode::Light);
        assert_eq!(ThemeId::FlexokiDark.label(), "Flexoki Dark");
        assert_eq!(ThemeId::Orbit.appearance(), ThemeMode::Dark);
        assert_eq!(ThemeId::OrbitLight.appearance(), ThemeMode::Light);
        assert_eq!(ThemeId::Orbit.label(), "Orbit");
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
        let light = Theme::for_id(ThemeId::OneLight).with_ui(theme.ui);
        assert_eq!(light.theme_id, ThemeId::OneLight);
        assert_eq!(light.ui, theme.ui);
    }

    #[test]
    fn palettes_differ() {
        let dark = Theme::dark();
        let light = Theme::light();
        assert_ne!(dark.bg_main, light.bg_main);
        assert_ne!(dark.text, light.text);
        assert_eq!(dark.theme_id, ThemeId::OneDark);
        assert_eq!(light.theme_id, ThemeId::OneLight);
        // Every palette is distinct from the default.
        for id in ThemeId::ALL {
            assert_eq!(Theme::for_id(id).theme_id, id);
        }
        // Dark themes stay dark; One Light, Flexoki Light, Orbit Light are light.
        for id in ThemeId::ALL {
            assert_eq!(
                Theme::for_id(id).mode,
                if id == ThemeId::OneLight
                    || id == ThemeId::FlexokiLight
                    || id == ThemeId::OrbitLight
                {
                    ThemeMode::Light
                } else {
                    ThemeMode::Dark
                }
            );
        }
    }
}
