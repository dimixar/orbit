//! GPUI Kit (gpui-component) bridge.
//!
//! This app draws its own chrome, but two things are better borrowed than
//! hand-built: a **data table** with resizable columns, sorting and virtual
//! scrolling, and a **plot** with real axes, ticks and grids. Both come from
//! `gpui_component::table` and `gpui_component::chart`.
//!
//! Those widgets read their palette from gpui-component's own global theme, so
//! this module projects Orbit's theme onto it: same canvas, same hairlines,
//! same accent. Without this bridge the table would arrive wearing another
//! product's colors — the failure mode the design system exists to prevent.
//!
//! # Version note
//!
//! gpui-kit 0.6 (what gpui-kit.com documents) is built on `gpui-pre`, a
//! *renamed republish* of Zed's gpui. Two gpui crate identities cannot
//! interoperate — `gpui_pre::Element` is not `gpui::Element` — so a 0.6 widget
//! cannot be returned from this app's `Render` impl. `gpui-component 0.5.1` is
//! the last release built on the same `gpui 0.2.2` this app pins, so that is
//! the version in `Cargo.toml`.

use gpui::{hsla, px, App};
use gpui_component::theme::{ThemeColor, ThemeMode as KitThemeMode};
use gpui_component::Theme as KitTheme;

use crate::theme::{self, Theme, ThemeMode};

/// Install the component framework. Called once at startup, after
/// [`crate::theme::init`], and followed by [`sync`].
pub fn init(cx: &mut App) {
    gpui_component::init(cx);
}

/// Project Orbit's palette onto the component framework's theme.
///
/// Fields with no Orbit equivalent keep the framework's default, so nothing
/// renders as a transparent hole if a widget reaches for one we never mapped.
pub fn sync(cx: &mut App, theme: &Theme) {
    let kit = KitTheme::global_mut(cx);
    kit.mode = match theme.mode {
        ThemeMode::Dark => KitThemeMode::Dark,
        ThemeMode::Light => KitThemeMode::Light,
    };
    kit.font_family = theme::ui_font_family();
    kit.mono_font_family = theme::code_font_family();
    kit.font_size = theme.ui_px(13.);
    kit.mono_font_size = theme.code_px(13.);
    kit.radius = px(8.);
    kit.radius_lg = px(12.);
    // Only the live palette is projected. The framework's light/dark *configs*
    // (which `Theme::change` would re-apply) are deliberately left alone: this
    // app never calls `change`, it calls `sync` on every palette switch, so the
    // live colors are always the app's own.
    kit.colors = colors(theme);
}

/// The whole semantic map: Orbit token → framework token.
///
/// Written as field assignments rather than one literal: this is a translation
/// table read top to bottom, grouped by surface, and the framework's 100+
/// fields make the literal form unreadable.
#[allow(clippy::field_reassign_with_default)]
fn colors(theme: &Theme) -> ThemeColor {
    // Start from the framework's defaults: any field we do not own stays a
    // sane color rather than an unset one.
    let mut c = ThemeColor::default();

    // ── surfaces and ink ──
    c.background = theme.bg_main;
    c.foreground = theme.text;
    c.border = theme.border;
    c.muted = theme.bg_raised;
    c.muted_foreground = theme.text_3;
    c.popover = theme.menu_bg;
    c.popover_foreground = theme.text;
    c.group_box = theme.bg_raised;
    c.group_box_foreground = theme.text;
    c.overlay = hsla(0., 0., 0., 0.45);
    c.window_border = theme.border;

    // ── accent: one hue, used for selection and focus only ──
    c.accent = theme.active;
    c.accent_foreground = theme.active_fg;
    c.caret = theme.accent;
    c.ring = theme.accent;
    c.selection = theme.active;
    c.link = theme.accent;
    c.link_hover = theme.accent;
    c.link_active = theme.accent;
    c.drag_border = theme.accent;
    c.drop_target = theme.accent.opacity(0.18);

    // ── filled controls (Orbit's primary button is a light fill) ──
    c.primary = theme.send_bg;
    c.primary_hover = theme.send_bg_hover;
    c.primary_active = theme.send_bg;
    c.primary_foreground = theme.send_fg;
    c.secondary = theme.bg_raised;
    c.secondary_hover = theme.bg_hover;
    c.secondary_active = theme.active;
    c.secondary_foreground = theme.text;
    c.input = theme.bg_main;

    // ── state colors ──
    c.danger = theme.stop_red;
    c.danger_hover = theme.stop_red_hover;
    c.danger_active = theme.stop_red;
    c.danger_foreground = theme.bg_main;
    c.success = theme.ok_green;
    c.success_hover = theme.ok_green;
    c.success_active = theme.ok_green;
    c.success_foreground = theme.bg_main;
    c.warning = theme.warn;
    c.warning_hover = theme.warn;
    c.warning_active = theme.warn;
    c.warning_foreground = theme.bg_main;
    c.info = theme.accent;
    c.info_hover = theme.accent;
    c.info_active = theme.accent;
    c.info_foreground = theme.bg_main;
    c.bullish = theme.add_green;
    c.bearish = theme.del_red;
    c.red = theme.crit;
    c.red_light = theme.crit.opacity(0.2);
    c.green = theme.ok_green;
    c.green_light = theme.ok_green.opacity(0.2);
    c.yellow = theme.warn;
    c.yellow_light = theme.warn.opacity(0.2);
    c.blue = theme.accent;
    c.blue_light = theme.accent.opacity(0.2);
    c.magenta = theme.accent.opacity(0.8);
    c.magenta_light = theme.accent.opacity(0.2);
    c.cyan = theme.accent.opacity(0.6);
    c.cyan_light = theme.accent.opacity(0.2);

    // ── tables and lists: hairline rows, hover wash, no stripes ──
    c.table = theme.bg_main;
    // The header sits on the page surface with a hairline under it (the
    // framework's own header row carries that rule), rather than arriving as a
    // filled band this design system does not use.
    c.table_head = theme.bg_main;
    c.table_head_foreground = theme.text_3;
    c.table_hover = theme.bg_hover;
    c.table_active = theme.active;
    c.table_active_border = theme.border_strong;
    c.table_even = theme.bg_main;
    c.table_row_border = theme.border;
    c.list = theme.bg_main;
    c.list_head = theme.bg_raised;
    c.list_hover = theme.bg_hover;
    c.list_active = theme.active;
    c.list_active_border = theme.border_strong;
    c.list_even = theme.bg_main;
    c.description_list_label = theme.bg_raised;
    c.description_list_label_foreground = theme.text_3;

    // ── tabs, switches, progress ──
    c.tab_bar = theme.bg_raised;
    c.tab_bar_segmented = theme.bg_raised;
    c.tab = theme.bg_raised;
    c.tab_active = theme.bg_main;
    c.tab_active_foreground = theme.text;
    c.tab_foreground = theme.text_3;
    c.switch = theme.active;
    c.switch_thumb = theme.text;
    c.progress_bar = theme.accent;
    c.slider_bar = theme.accent;
    c.slider_thumb = theme.text;
    c.skeleton = theme.bg_raised;
    c.accordion = theme.bg_raised;
    c.accordion_hover = theme.bg_hover;

    // ── sidebar and title bar ──
    c.sidebar = theme.bg_sidebar;
    c.sidebar_foreground = theme.text_2;
    c.sidebar_border = theme.border;
    c.sidebar_accent = theme.active;
    c.sidebar_accent_foreground = theme.active_fg;
    c.sidebar_primary = theme.accent;
    c.sidebar_primary_foreground = theme.bg_main;
    c.title_bar = theme.bg_sidebar;
    c.title_bar_border = theme.border;
    c.tiles = theme.bg_raised;

    // ── scrollbars ──
    c.scrollbar = hsla(0., 0., 0., 0.);
    c.scrollbar_thumb = theme.border_strong;
    c.scrollbar_thumb_hover = theme.text_3;

    // ── chart series: one hue at four weights, then neutral ink ──
    c.chart_1 = theme.accent;
    c.chart_2 = theme.accent.opacity(0.62);
    c.chart_3 = theme.accent.opacity(0.40);
    c.chart_4 = theme.accent.opacity(0.24);
    c.chart_5 = theme.text_2;

    c
}
