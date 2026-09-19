use super::*;
use crate::platform::WindowCommand;

/// Turn a wire command name into a short human label, e.g. `set_model` →
/// `Set model failed`.
pub(crate) fn humanize_command(command: &str) -> String {
    let spaced = command.replace(['_', '.'], " ");
    let mut chars = spaced.chars();
    let label = match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => tr!("helpers.command"),
    };
    tr!("helpers.command_failed", label = label)
}

/// One queued-message chip: a kind tag plus the message text.
pub(crate) fn queue_chip(kind: &str, text: &str, follow: bool, theme: Theme) -> AnyElement {
    div()
        .max_w(px(300.))
        .flex()
        .items_center()
        .gap(px(6.))
        .px(px(8.))
        .py(px(4.))
        .rounded(px(6.))
        .border_1()
        .border_color(theme.border)
        .bg(theme.bg_raised)
        .child(
            div()
                .flex_none()
                .text_size(theme.ui_px(10.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(if follow { theme.text_3 } else { theme.accent })
                .child(kind.to_string()),
        )
        .child(
            div()
                .min_w_0()
                .truncate()
                .text_size(theme.ui_px(11.5))
                .text_color(theme.text_2)
                .child(text.to_string()),
        )
        .into_any_element()
}

/// Render an embedded HugeIcons SVG tinted with the given color.
///
/// `flex_none` is load-bearing: an SVG defaults to `flex-shrink: 1`, so in a
/// flex row beside wide text it collapses to zero width (the "icons render
/// tiny" bug). Icons must always keep their declared size.
pub(crate) fn icon(path: &'static str, size: f32, color: Hsla) -> impl IntoElement + use<> {
    gpui::svg()
        .path(path)
        .flex_none()
        .size(px(size))
        .text_color(color)
}

/// Same as [`icon`] but for runtime-computed paths (per-provider marks).
pub(crate) fn icon_dyn(path: SharedString, size: f32, color: Hsla) -> impl IntoElement + use<> {
    gpui::svg()
        .path(path)
        .flex_none()
        .size(px(size))
        .text_color(color)
}

/// Width of one caption button, matching the metric Windows uses for its own
/// (`platform::WINDOW_CONTROLS_W` is three of them).
pub(crate) const CAPTION_BUTTON_W: f32 = 46.;

/// The window's own minimize / maximize / close buttons, drawn by the app on
/// the platforms where it paints the caption (`platform::draws_window_controls`).
///
/// Each button runs the system's own command through
/// [`platform::window_command`] — the same one a real caption button sends — so
/// nothing about window management is reimplemented here.
pub(crate) fn window_controls(theme: Theme, maximized: bool) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .h_full()
        .child(caption_button(
            theme,
            "window-minimize",
            "icons/minus.svg",
            WindowCommand::Minimize,
            false,
        ))
        .child(caption_button(
            theme,
            "window-maximize",
            // The glyph follows the window state, as the OS button does.
            if maximized {
                "icons/window-restore.svg"
            } else {
                "icons/window-maximize.svg"
            },
            WindowCommand::ToggleMaximize,
            false,
        ))
        .child(caption_button(
            theme,
            "window-close",
            "icons/x.svg",
            WindowCommand::Close,
            true,
        ))
}

/// One caption button: a flat 46px hit area with a centred glyph. Close is the
/// one that turns red under the pointer, the way Windows' own does, and its
/// glyph brightens with it.
fn caption_button(
    theme: Theme,
    id: &'static str,
    glyph: &'static str,
    command: WindowCommand,
    close: bool,
) -> impl IntoElement {
    let fill = if close {
        theme.stop_red
    } else {
        theme.bg_hover
    };
    let mut icon = gpui::svg()
        .path(glyph)
        .flex_none()
        .size(px(14.))
        .text_color(theme.text_2);
    if close {
        icon = icon.group_hover(id, move |style| style.text_color(theme.text));
    }
    div()
        .id(id)
        .group(id)
        .w(px(CAPTION_BUTTON_W))
        .h_full()
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .hover(move |style| style.bg(fill))
        .on_mouse_up(MouseButton::Left, move |_, window, _| {
            crate::platform::window_command(window, command)
        })
        .child(icon)
}

/// Turn a header strip into a window drag region.
///
/// The press is handed to the OS rather than to GPUI's `WindowControlArea::Drag`
/// on the platform where the app paints the caption — see
/// [`platform::start_window_drag`] for why that path is closed to this app.
/// Elsewhere the platform's own drag area is what moves the window.
pub(crate) fn window_drag_region(el: gpui::Div) -> gpui::Div {
    #[cfg(windows)]
    {
        el.on_mouse_down(MouseButton::Left, |_, window, _| {
            crate::platform::start_window_drag(window)
        })
    }
    #[cfg(not(windows))]
    {
        el.window_control_area(gpui::WindowControlArea::Drag)
    }
}

/// Render a compile-time-embedded raster image (PNG) at a fixed height, with
/// the width derived from the source's aspect ratio.
///
/// `img("name.png")` is a trap for embedded assets: gpui's `From<&str>` runs
/// the string through its URI heuristic, and a slashless filename parses as a
/// valid URI authority, so it is fetched over the network instead of loaded
/// from [`crate::assets::Assets`]. Passing [`Resource::Embedded`] explicitly
/// keeps it on the asset-source path.
pub(crate) fn embedded_image(path: &'static str, height: f32) -> impl IntoElement + use<> {
    img(ImageSource::Resource(Resource::Embedded(path.into())))
        .h(px(height))
        .flex_none()
}

/// Same as [`embedded_image`], but sized by width; the height follows the
/// source's aspect ratio. Use for the wordmark, which is laid out to span the
/// sidebar's content width.
pub(crate) fn embedded_image_w(path: &'static str, width: Pixels) -> impl IntoElement + use<> {
    img(ImageSource::Resource(Resource::Embedded(path.into())))
        .w(width)
        .flex_none()
}

// ── devicons (Nerd Font file glyphs) ───────────────────────────────

/// The Nerd Font family that covers devicons glyphs, detected once per
/// process (font availability doesn't change mid-run).
static NERD_FONT: std::sync::OnceLock<Option<SharedString>> = std::sync::OnceLock::new();

/// Preferred Nerd Font families, best first — used only to *rank* the
/// installed families; any family whose name contains "Nerd Font" (or the
/// short `NF` style, e.g. `MesloLGS NF`) is a candidate, verified by an
/// actual PUA-glyph probe.
const NERD_FONT_PREFERRED: [&str; 6] = [
    "SymbolsNerdFont",
    "Symbols Nerd Font",
    "JetBrainsMono Nerd Font",
    "Hack Nerd Font",
    "FiraCode Nerd Font",
    "CaskaydiaCove Nerd Font",
];

/// The Nerd Font family to paint devicons glyphs with, or `None` when no
/// Nerd Font is installed (callers fall back to extension text badges).
pub(crate) fn nerd_font_family(cx: &App) -> Option<SharedString> {
    NERD_FONT
        .get_or_init(|| {
            let installed = cx.text_system().all_font_names();
            // Any family named like a Nerd Font, best-known first.
            let mut candidates: Vec<String> = installed
                .iter()
                .filter(|name| {
                    let lower = name.to_lowercase();
                    lower.contains("nerd font")
                        || lower.contains("nerdfont")
                        || lower.ends_with(" nf")
                })
                .cloned()
                .collect();
            // Dedicated symbols fonts paint the widest glyph coverage first;
            // popular patched coding fonts next; the rest in name order.
            candidates.sort_by_key(|name| {
                let lower = name.to_lowercase();
                let pref = NERD_FONT_PREFERRED
                    .iter()
                    .position(|p| lower.eq_ignore_ascii_case(&p.to_lowercase()))
                    .unwrap_or(NERD_FONT_PREFERRED.len());
                (pref, !lower.starts_with("symbols"), name.clone())
            });
            candidates.into_iter().find_map(|family| {
                let font_id = cx.text_system().resolve_font(&gpui::font(family.clone()));
                // `\u{ea60}` sits in Nerd Font's Octicon range — present in any
                // complete Nerd Font, absent from plain coding fonts.
                cx.text_system()
                    .typographic_bounds(font_id, px(12.), '\u{ea60}')
                    .ok()?;
                Some(SharedString::from(family))
            })
        })
        .clone()
}

/// devicons glyph + color for a file path (`README.md` → its Nerd Font
/// markdown glyph). `dark` selects devicons' palette; `None` when the
/// color string is not parseable hex.
pub(crate) fn dev_file_icon(path: &str, dark: bool) -> Option<(char, Hsla)> {
    let theme = if dark {
        devicons::Theme::Dark
    } else {
        devicons::Theme::Light
    };
    let icon = devicons::icon_for_file(path, &Some(theme));
    let value = u32::from_str_radix(icon.color.trim_start_matches('#'), 16).ok()?;
    Some((icon.icon, gpui::rgb(value).into()))
}

/// Paint a devicons glyph (or fall back to `fallback` when no Nerd Font
/// is installed) — shared by the @-mention rows and attachment chips.
pub(crate) fn file_glyph(
    path: &str,
    dark: bool,
    nerd_family: Option<&SharedString>,
    font_size: f32,
    fallback: AnyElement,
) -> AnyElement {
    let Some(family) = nerd_family else {
        return fallback;
    };
    let Some((glyph, color)) = dev_file_icon(path, dark) else {
        return fallback;
    };
    div()
        .w(px(18.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .font_family(family)
        .text_size(px(font_size))
        .text_color(color)
        .child(glyph.to_string())
        .into_any_element()
}

/// Extension text badge (`MD`, `TSX`, `HTML`, …) — the no-Nerd-Font
/// fallback for [`file_glyph`], tinted with the language's brand color.
pub(crate) fn file_badge(path: &str, theme: Theme) -> AnyElement {
    let (label, dark_hex, light_hex) = mentions::file_type_badge(path);
    let color: Hsla = gpui::rgb(if theme.mode == ThemeMode::Dark {
        dark_hex
    } else {
        light_hex
    })
    .into();
    div()
        .size(px(17.))
        .flex_none()
        .rounded(px(4.))
        .bg(color.opacity(0.16))
        .flex()
        .items_center()
        .justify_center()
        .text_size(theme.ui_px(7.5))
        .line_height(theme.ui_px(8.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(color)
        .child(label.to_string())
        .into_any_element()
}

/// A Runtime detail value in the UI text color.
pub(crate) fn runtime_text(theme: Theme, text: String) -> AnyElement {
    div()
        .min_w_0()
        .truncate()
        .text_size(theme.ui_px(12.5))
        .text_color(theme.text)
        .child(text)
        .into_any_element()
}

/// A Runtime detail value rendered as a path (monospace, dimmed).
pub(crate) fn runtime_path(theme: Theme, text: String) -> AnyElement {
    div()
        .min_w_0()
        .truncate()
        .font_family(theme::code_font_family())
        .text_size(theme.code_px(11.5))
        .text_color(theme.text_2)
        .child(text)
        .into_any_element()
}

/// A Runtime error value (critical color).
pub(crate) fn runtime_error(theme: Theme, text: String) -> AnyElement {
    div()
        .min_w_0()
        .truncate()
        .text_size(theme.ui_px(12.))
        .text_color(theme.crit)
        .child(text)
        .into_any_element()
}

/// The title shown in the top bar and session lists. An explicit
/// `set_session_name` value wins over pi's live auto-title; neither
/// present is a new, unnamed task.
pub(crate) fn session_display_title(
    session_name: Option<&str>,
    current_title: Option<&str>,
) -> String {
    session_name
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .or_else(|| {
            current_title
                .map(str::trim)
                .filter(|title| !title.is_empty())
                .map(str::to_owned)
        })
        .unwrap_or_else(|| "New task".into())
}

/// "42s" / "3m 12s" / "2h 5m" / "4d 3h" for the Runtime uptime readout.
pub(crate) fn format_uptime(elapsed: Duration) -> String {
    let seconds = elapsed.as_secs();
    match seconds {
        s if s < 60 => format!("{s}s"),
        s if s < 3_600 => format!("{}m {}s", s / 60, s % 60),
        s if s < 86_400 => format!("{}h {}m", s / 3_600, (s % 3_600) / 60),
        s => format!("{}d {}h", s / 86_400, (s % 86_400) / 3_600),
    }
}
