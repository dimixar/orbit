//! Interface localization (Waku's `src/i18n.rs` ported and widened).
//!
//! Translations live in `crates/orbit-pi/locales/<locale>.yml` and are
//! compiled in by the `rust_i18n::i18n!` invocation in `main.rs`. Lookups go
//! through the [`tr!`](crate::tr) / [`tr_cow!`](crate::tr_cow) macros so a
//! missing key falls back to English and, ultimately, to the key itself.
//!
//! The persisted preference is [`AppLanguage`] — either an explicit locale or
//! `System`, which resolves through the OS's preferred languages.

use serde::{Deserialize, Serialize};

/// The language preference Orbit persists. `System` resolves to one of the
/// locales Orbit deliberately ships today.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AppLanguage {
    System,
    English,
    SimplifiedChinese,
    Japanese,
    Korean,
    Spanish,
    French,
    German,
    PortugueseBrazil,
    Russian,
    Italian,
}

impl AppLanguage {
    /// Every choice offered in Settings → Appearance, `System` first.
    pub const ALL: [Self; 11] = [
        Self::System,
        Self::English,
        Self::SimplifiedChinese,
        Self::Japanese,
        Self::Korean,
        Self::Spanish,
        Self::French,
        Self::German,
        Self::PortugueseBrazil,
        Self::Russian,
        Self::Italian,
    ];

    /// Explicit languages only (no `System`), in picker order. Used by the
    /// completeness tests and by callers that enumerate shipped locales.
    #[allow(dead_code)]
    pub const EXPLICIT: [Self; 10] = [
        Self::English,
        Self::SimplifiedChinese,
        Self::Japanese,
        Self::Korean,
        Self::Spanish,
        Self::French,
        Self::German,
        Self::PortugueseBrazil,
        Self::Russian,
        Self::Italian,
    ];

    /// The rust-i18n locale id this preference resolves to.
    pub fn locale(self) -> &'static str {
        match self.resolved() {
            Self::System => unreachable!("system language always resolves to a shipped locale"),
            Self::English => "en",
            Self::SimplifiedChinese => "zh-CN",
            Self::Japanese => "ja",
            Self::Korean => "ko",
            Self::Spanish => "es",
            Self::French => "fr",
            Self::German => "de",
            Self::PortugueseBrazil => "pt-BR",
            Self::Russian => "ru",
            Self::Italian => "it",
        }
    }

    /// Explicit language names are autonyms so the selector remains
    /// understandable even when the current locale is unfamiliar. `System`
    /// is the one translated label — it always reads in the active locale.
    pub fn label(self) -> String {
        match self {
            Self::System => translate("language.system"),
            Self::English => "English".to_owned(),
            Self::SimplifiedChinese => "简体中文".to_owned(),
            Self::Japanese => "日本語".to_owned(),
            Self::Korean => "한국어".to_owned(),
            Self::Spanish => "Español".to_owned(),
            Self::French => "Français".to_owned(),
            Self::German => "Deutsch".to_owned(),
            Self::PortugueseBrazil => "Português (Brasil)".to_owned(),
            Self::Russian => "Русский".to_owned(),
            Self::Italian => "Italiano".to_owned(),
        }
    }

    /// The persisted token (also the selector's stable identity).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::English => "en",
            Self::SimplifiedChinese => "zh-CN",
            Self::Japanese => "ja",
            Self::Korean => "ko",
            Self::Spanish => "es",
            Self::French => "fr",
            Self::German => "de",
            Self::PortugueseBrazil => "pt-BR",
            Self::Russian => "ru",
            Self::Italian => "it",
        }
    }

    /// Parse a persisted token. Accepts the historical `en` plus the
    /// rust-i18n locale ids; anything unknown is left for the caller to treat
    /// as a default.
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().replace('_', "-").to_ascii_lowercase().as_str() {
            "system" => Some(Self::System),
            "en" | "en-us" | "en-gb" => Some(Self::English),
            "zh-cn" | "zh-sg" | "zh-hans" => Some(Self::SimplifiedChinese),
            "ja" | "ja-jp" => Some(Self::Japanese),
            "ko" | "ko-kr" => Some(Self::Korean),
            "es" | "es-es" | "es-mx" | "es-419" => Some(Self::Spanish),
            "fr" | "fr-fr" | "fr-ca" => Some(Self::French),
            "de" | "de-de" => Some(Self::German),
            "pt-br" | "pt" => Some(Self::PortugueseBrazil),
            "ru" | "ru-ru" => Some(Self::Russian),
            "it" | "it-it" => Some(Self::Italian),
            _ => None,
        }
    }

    /// Collapse `System` onto the concrete locale the OS asks for.
    pub fn resolved(self) -> Self {
        match self {
            Self::System => Self::from_locale_id(&system_locale()),
            explicit => explicit,
        }
    }

    /// Map an arbitrary BCP-47 tag onto a shipped locale, defaulting to
    /// English. Only Simplified Chinese is enabled — Traditional tags stay
    /// English rather than reading as the wrong script.
    pub fn from_locale_id(locale: &str) -> Self {
        let locale = locale.replace('_', "-").to_ascii_lowercase();
        if locale == "zh-cn" || locale == "zh-sg" || locale.starts_with("zh-hans") {
            Self::SimplifiedChinese
        } else if locale == "ja" || locale.starts_with("ja-") {
            Self::Japanese
        } else if locale == "ko" || locale.starts_with("ko-") {
            Self::Korean
        } else if locale == "es" || locale.starts_with("es-") {
            Self::Spanish
        } else if locale == "fr" || locale.starts_with("fr-") {
            Self::French
        } else if locale == "de" || locale.starts_with("de-") {
            Self::German
        } else if locale == "pt-br" || locale == "pt" {
            Self::PortugueseBrazil
        } else if locale == "ru" || locale.starts_with("ru-") {
            Self::Russian
        } else if locale == "it" || locale.starts_with("it-") {
            Self::Italian
        } else {
            Self::English
        }
    }
}

impl Default for AppLanguage {
    fn default() -> Self {
        Self::System
    }
}

/// Point the global lookup at a preference's concrete locale.
pub fn set_language(language: AppLanguage) {
    rust_i18n::set_locale(language.locale());
}

/// Translate a key in the active locale, returning an owned `String`.
///
/// Prefer the [`tr!`](crate::tr) macro at call sites; this exists so
/// non-literal keys (e.g. from a table) can still be looked up.
pub fn translate(key: &str) -> String {
    rust_i18n::t!(key).into_owned()
}

/// Whether the active locale reads dates in an East-Asian order
/// (`2026年2月3日`), which some call sites format by hand. Reserved for the
/// date formatters; unused until those read it.
#[allow(dead_code)]
pub fn uses_east_asian_date_format() -> bool {
    locale_uses_east_asian_date_format(&rust_i18n::locale())
}

#[allow(dead_code)]
fn locale_uses_east_asian_date_format(locale: &str) -> bool {
    matches!(locale, "zh-CN" | "ja" | "ko")
}

/// The OS's preferred UI language as a BCP-47 tag.
#[cfg(target_os = "macos")]
fn system_locale() -> String {
    use objc2_foundation::NSLocale;

    NSLocale::preferredLanguages()
        .firstObject()
        .map(|locale| locale.to_string())
        .unwrap_or_else(|| "en".to_owned())
}

#[cfg(not(target_os = "macos"))]
fn system_locale() -> String {
    std::env::var("LC_ALL")
        .or_else(|_| std::env::var("LC_MESSAGES"))
        .or_else(|_| std::env::var("LANG"))
        .unwrap_or_else(|_| "en".to_owned())
        .split('.')
        .next()
        .unwrap_or("en")
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_locale_ids_are_supported() {
        assert_eq!(AppLanguage::English.locale(), "en");
        assert_eq!(AppLanguage::SimplifiedChinese.locale(), "zh-CN");
        assert_eq!(AppLanguage::Japanese.locale(), "ja");
        assert_eq!(AppLanguage::Korean.locale(), "ko");
        assert_eq!(AppLanguage::Spanish.locale(), "es");
        assert_eq!(AppLanguage::French.locale(), "fr");
        assert_eq!(AppLanguage::German.locale(), "de");
        assert_eq!(AppLanguage::PortugueseBrazil.locale(), "pt-BR");
        assert_eq!(AppLanguage::Russian.locale(), "ru");
        assert_eq!(AppLanguage::Italian.locale(), "it");

        let locales = rust_i18n::available_locales!();
        assert_eq!(locales.len(), 10);
        for language in AppLanguage::EXPLICIT {
            assert!(
                locales.iter().any(|locale| locale.as_ref() == language.locale()),
                "locale file missing for {}",
                language.locale()
            );
        }
    }

    #[test]
    fn language_names_are_autonyms() {
        assert_eq!(AppLanguage::English.label(), "English");
        assert_eq!(AppLanguage::SimplifiedChinese.label(), "简体中文");
        assert_eq!(AppLanguage::Japanese.label(), "日本語");
        assert_eq!(AppLanguage::Korean.label(), "한국어");
        assert_eq!(AppLanguage::German.label(), "Deutsch");
    }

    #[test]
    fn system_is_the_default_persisted_preference() {
        assert_eq!(AppLanguage::default(), AppLanguage::System);
        assert_eq!(
            serde_json::to_string(&AppLanguage::System).unwrap(),
            r#""system""#
        );
        assert!(matches!(
            AppLanguage::System.locale(),
            "en" | "zh-CN" | "ja" | "ko" | "es" | "fr" | "de" | "pt-BR" | "ru" | "it"
        ));
    }

    #[test]
    fn persisted_tokens_round_trip() {
        for language in AppLanguage::ALL {
            assert_eq!(AppLanguage::parse(language.as_str()), Some(language));
        }
        // Legacy / regional tags fold onto a shipped locale.
        assert_eq!(AppLanguage::parse("zh_CN"), Some(AppLanguage::SimplifiedChinese));
        assert_eq!(AppLanguage::parse("pt"), Some(AppLanguage::PortugueseBrazil));
        assert_eq!(AppLanguage::parse("fr-CA"), Some(AppLanguage::French));
    }

    #[test]
    fn system_locales_are_detected() {
        assert_eq!(AppLanguage::from_locale_id("ja_JP"), AppLanguage::Japanese);
        assert_eq!(AppLanguage::from_locale_id("ko-KR"), AppLanguage::Korean);
        assert_eq!(AppLanguage::from_locale_id("de-DE"), AppLanguage::German);
        assert_eq!(
            AppLanguage::from_locale_id("zh-Hans-CN"),
            AppLanguage::SimplifiedChinese
        );
        // Traditional Chinese is deliberately not enabled.
        assert_eq!(AppLanguage::from_locale_id("zh-Hant-TW"), AppLanguage::English);
        assert_eq!(AppLanguage::from_locale_id("nl-NL"), AppLanguage::English);
    }

    #[test]
    fn east_asian_locales_use_east_asian_dates() {
        assert!(locale_uses_east_asian_date_format("ja"));
        assert!(locale_uses_east_asian_date_format("ko"));
        assert!(locale_uses_east_asian_date_format("zh-CN"));
        assert!(!locale_uses_east_asian_date_format("en"));
    }

    #[test]
    fn core_keys_resolve_in_every_locale() {
        // A canary key that must exist in every shipped locale file.
        for language in AppLanguage::EXPLICIT {
            let value = rust_i18n::t!("language.title", locale = language.locale());
            assert!(!value.is_empty(), "missing language.title for {}", language.locale());
        }
    }

    /// Every key in `en.yml` must exist in every generated locale file. The
    /// generator (`scripts/gen_locales.py`) guarantees this, but a manual edit
    /// or a stale file would silently fall back to English — this catches it.
    #[test]
    fn locale_files_cover_every_english_key() {
        fn keys(src: &str) -> std::collections::BTreeSet<&str> {
            src.lines()
                .filter(|line| !line.trim().is_empty() && !line.starts_with("_version"))
                .filter_map(|line| line.split_once(':').map(|(key, _)| key.trim()))
                .collect()
        }
        let en = keys(include_str!("../locales/en.yml"));
        for (locale, source) in [
            ("zh-CN", include_str!("../locales/zh-CN.yml")),
            ("ja", include_str!("../locales/ja.yml")),
            ("ko", include_str!("../locales/ko.yml")),
            ("es", include_str!("../locales/es.yml")),
            ("fr", include_str!("../locales/fr.yml")),
            ("de", include_str!("../locales/de.yml")),
            ("pt-BR", include_str!("../locales/pt-BR.yml")),
            ("ru", include_str!("../locales/ru.yml")),
            ("it", include_str!("../locales/it.yml")),
        ] {
            let have = keys(source);
            let missing: Vec<_> = en.difference(&have).collect();
            assert!(
                missing.is_empty(),
                "{locale} is missing {} key(s), e.g. {:?}",
                missing.len(),
                &missing[..missing.len().min(5)]
            );
        }
    }
}
