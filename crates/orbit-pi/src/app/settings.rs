use super::helpers::*;
use super::*;

impl OrbitApp {
    // ── settings surface ───────────────────────────────────────────
    // Waku-style: left nav (Back + sections), right column of setting
    // rows. Every control maps to real app state; read-only rows show
    // real pi/runtime facts (PRODUCT.md: nothing decorative that
    // pretends to be functional).

    pub(super) fn render_settings(&self, cx: &Context<Self>) -> impl IntoElement + use<> {
        let this = cx.entity();
        let theme = *theme::get(cx);
        let sections: [(SettingsSection, &'static str, &'static str); 6] = [
            (SettingsSection::General, "icons/settings.svg", "General"),
            (
                SettingsSection::Runtime,
                "icons/server-stack.svg",
                "Runtime",
            ),
            (SettingsSection::Agent, "icons/spark.svg", "Agent"),
            (
                SettingsSection::Appearance,
                "icons/contrast.svg",
                "Appearance",
            ),
            (SettingsSection::Providers, "icons/cloud.svg", "Providers"),
            (SettingsSection::About, "icons/info.svg", "About"),
        ];

        div()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .flex()
            .relative()
            .bg(theme.bg_main)
            .font_family(theme::ui_font_family())
            // ── nav column ──
            .child(
                div()
                    .w(px(240.))
                    .h_full()
                    .flex_shrink_0()
                    .bg(theme.bg_sidebar)
                    .border_r_1()
                    .border_color(theme.border)
                    .flex()
                    .flex_col()
                    // traffic-light strip (drag region)
                    .child(
                        div()
                            .h(px(38.))
                            .w_full()
                            .window_control_area(WindowControlArea::Drag),
                    )
                    // Back — inset like the sessions nav, breathing room below
                    .child(
                        div().px_2().pt_1().pb_3().child(
                            div()
                                .w_full()
                                .px_2()
                                .py(px(5.))
                                .rounded_md()
                                .flex()
                                .items_center()
                                .gap_1p5()
                                .cursor_pointer()
                                .hover(|s| s.bg(theme.bg_hover))
                                .on_mouse_up(MouseButton::Left, cx.listener(Self::on_settings_back))
                                .child(icon("icons/arrow-left.svg", 14., theme.text_2))
                                .child(
                                    div()
                                        .text_size(theme.ui_px(13.))
                                        .text_color(theme.text_2)
                                        .child("Back"),
                                ),
                        ),
                    )
                    // section rows — inset wrapper so hover/selected pills
                    // don't bleed to the window edge (matches sessions nav)
                    .child(
                        div()
                            .px_2()
                            .flex()
                            .flex_col()
                            .gap_0p5()
                            .children(sections.map(|(section, section_icon, label)| {
                                let this = this.clone();
                                let selected = self.settings_section == section;
                                div()
                                    .w_full()
                                    .px_2()
                                    .py(px(5.))
                                    .rounded_md()
                                    .text_size(theme.ui_px(13.))
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .cursor_pointer()
                                    .when(selected, |row| row.bg(theme.bg_raised))
                                    .when(!selected, |row| row.hover(|s| s.bg(theme.bg_hover)))
                                    .on_mouse_up(MouseButton::Left, move |_, _, cx| {
                                        this.update(cx, |app, cx| {
                                            app.set_settings_section(section, cx);
                                        });
                                    })
                                    .child(icon(
                                        section_icon,
                                        15.,
                                        if selected { theme.text } else { theme.text_3 },
                                    ))
                                    .child(
                                        div()
                                            .text_color(if selected {
                                                theme.text
                                            } else {
                                                theme.text_2
                                            })
                                            .child(label.to_string()),
                                    )
                            })),
                    ),
            )
            // ── content column: fixed header + scrolling body ──
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    // The title (and, on Providers, the search/toolbar) stay
                    // pinned; gpui has no sticky positioning, so they live
                    // outside the scroll container.
                    .child(
                        div()
                            .w_full()
                            .flex_shrink_0()
                            .px(px(24.))
                            .pt(px(44.))
                            .pb(px(12.))
                            .when(
                                self.settings_section == SettingsSection::Providers,
                                |header| header.border_b_1().border_color(theme.border),
                            )
                            .flex()
                            .flex_col()
                            .gap_3()
                            .child(self.settings_header(theme))
                            .children(
                                (self.settings_section == SettingsSection::Providers)
                                    .then(|| self.provider_toolbar(theme, this.clone(), cx)),
                            ),
                    )
                    .child(
                        div()
                            .id("settings-content")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .child(
                                div()
                                    .w_full()
                                    .px(px(24.))
                                    .pb(px(12.))
                                    .flex()
                                    .flex_col()
                                    .gap(theme.space(12.))
                                    .children(self.error_banner(theme, cx))
                                    .children(self.settings_rows(&this, theme, cx)),
                            ),
                    ),
            )
            // ── provider editor modals (models.json + API key) ──
            .children(self.provider_editor_layer(theme, this.clone(), cx))
            .children(self.provider_key_layer(theme, this, cx))
    }

    pub(super) fn settings_header(&self, theme: Theme) -> impl IntoElement + use<> {
        let (title, subtitle) = match self.settings_section {
            SettingsSection::General => (
                "General",
                "How Orbit connects to the pi agent and stores your data.",
            ),
            SettingsSection::Runtime => (
                "Runtime",
                "The pi agent process Orbit spawns — stdio transport, no host or port.",
            ),
            SettingsSection::Agent => (
                "Agent",
                "How pi queues your messages, compacts context, and retries errors.",
            ),
            SettingsSection::Appearance => ("Appearance", "Window, layout, and color preferences."),
            SettingsSection::Providers => (
                "Providers",
                "The live catalog plus custom providers from ~/.pi/agent/models.json.",
            ),
            SettingsSection::About => ("About", "Versions and the rendering stack."),
        };
        div()
            .flex()
            .flex_col()
            .gap_1()
            .pb_1()
            .when(self.settings_section == SettingsSection::About, |header| {
                header.child(img(crate::app_icon::ASSET).size(px(72.)).flex_none())
            })
            .child(
                div()
                    .text_size(theme.ui_px(20.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.text)
                    .child(title.to_string()),
            )
            .child(
                div()
                    .text_size(theme.ui_px(12.5))
                    .text_color(theme.text_2)
                    .child(subtitle.to_string()),
            )
    }

    /// The rows for the active section, as card elements. `this` rides
    /// along for closures in interactive controls (rows themselves are
    /// built read-only from app state).
    pub(super) fn settings_rows(
        &self,
        this: &Entity<OrbitApp>,
        theme: Theme,
        cx: &Context<Self>,
    ) -> Vec<AnyElement> {
        match self.settings_section {
            SettingsSection::General => vec![
                self.card(
                    theme,
                    "pi agent",
                    "Spawned as a child process — newline-delimited JSON over stdio.",
                    Some(self.connection_status(theme)),
                ),
                self.card_with_path(
                    theme,
                    "Local by default",
                    "Sessions live in pi's own store on this computer — no daemon, no cloud.",
                    Some(&sessions::sessions_dir().to_string_lossy()),
                    None,
                ),
                self.card_with_path(
                    theme,
                    "Workspace",
                    "New tasks start in this directory.",
                    Some(
                        &self
                            .current_workspace
                            .clone()
                            .or_else(|| std::env::current_dir().ok())
                            .unwrap_or_default()
                            .to_string_lossy(),
                    ),
                    None,
                ),
            ],
            SettingsSection::Runtime => self.runtime_rows(theme, this.clone(), cx),
            SettingsSection::Agent => self.agent_rows(theme, this.clone(), cx),
            SettingsSection::Appearance => vec![
                self.card(
                    theme,
                    "Theme",
                    "Pick a Zed-compatible palette for the workbench.",
                    Some(self.theme_select(theme, this.clone(), cx)),
                ),
                self.background_card(theme, this.clone(), cx),
                self.card(
                    theme,
                    "Language",
                    "Choose the language used throughout Orbit.",
                    Some(self.language_select(theme, this.clone(), cx)),
                ),
                self.density_type_card(theme, this.clone(), cx),
                self.card(
                    theme,
                    "Show sidebar",
                    "Show the sessions sidebar. Also toggleable from the top bar.",
                    Some(self.sidebar_toggle(theme, this.clone())),
                ),
                self.card(
                    theme,
                    "GPU-rendered streaming",
                    "Stream commits are coalesced (~8 Hz) and highlighting is paint-only, so long tasks never reflow the transcript.",
                    None,
                ),
            ],
            SettingsSection::Providers => self.provider_rows(theme, this.clone(), cx),
            SettingsSection::About => vec![
                self.card(
                    theme,
                    "Orbit Pi",
                    "Native workbench for the pi coding agent.",
                    Some(
                        div()
                            .text_size(theme.ui_px(12.))
                            .text_color(theme.text_2)
                            .child(format!("v{}", env!("CARGO_PKG_VERSION")))
                            .into_any_element(),
                    ),
                ),
                self.card(
                    theme,
                    "GPUI",
                    "GPU-accelerated UI framework (pinned; runtime shaders).",
                    Some(
                        div()
                            .text_size(theme.ui_px(12.))
                            .text_color(theme.text_2)
                            .child("0.2.2")
                            .into_any_element(),
                    ),
                ),
                self.card(
                    theme,
                    "pi CLI",
                    "The only agent runtime — pi speaks its own RPC protocol over stdio.",
                    Some(self.connection_status(theme)),
                ),
            ],
        }
    }

    /// The sticky provider header: the search field plus the
    /// count / Refresh / Add toolbar. Rendered outside the scroll container
    /// so it stays put while the grid scrolls (gpui 0.2.2 has no
    /// `position: sticky`).
    pub(super) fn provider_toolbar(
        &self,
        theme: Theme,
        this: Entity<OrbitApp>,
        _cx: &Context<Self>,
    ) -> AnyElement {
        let all = self.provider_views();
        let active = all.iter().filter(|view| view.active).count();
        let connected = all.iter().filter(|view| view.connected()).count();

        let search = div()
            .w_full()
            .h(px(34.))
            .px(px(10.))
            .rounded_md()
            .border_1()
            .border_color(theme.border)
            .bg(theme.bg_main)
            .flex()
            .items_center()
            .gap_2()
            .text_size(theme.ui_px(13.))
            .child(icon("icons/search.svg", 14., theme.text_3))
            .child(self.provider_filter.clone());

        let refresh_icon: AnyElement = if self.providers_refreshing {
            gpui::svg()
                .path("icons/loader.svg")
                .flex_none()
                .size(px(13.))
                .text_color(theme.text_2)
                .with_animation(
                    "providers-refresh-spin",
                    Animation::new(Duration::from_millis(900)).repeat(),
                    |svg, delta| {
                        svg.with_transformation(Transformation::rotate(radians(
                            delta * std::f32::consts::TAU,
                        )))
                    },
                )
                .into_any_element()
        } else {
            icon("icons/refresh.svg", 13., theme.text_2).into_any_element()
        };
        let refresh_button = div()
            .id("providers-refresh")
            .h(px(30.))
            .px(px(12.))
            .rounded_md()
            .border_1()
            .border_color(theme.border)
            .bg(theme.bg_raised)
            .flex()
            .items_center()
            .gap_1p5()
            .cursor_pointer()
            .when(self.providers_refreshing, |button| button.opacity(0.6))
            .hover(|style| style.bg(theme.bg_hover))
            .on_mouse_up(MouseButton::Left, {
                let this = this.clone();
                move |_, _, cx| {
                    this.update(cx, |app, cx| app.provider_refresh(cx));
                }
            })
            .child(refresh_icon)
            .child(
                div()
                    .text_size(theme.ui_px(12.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.text_2)
                    .child("Refresh"),
            );

        let add_button = div()
            .id("providers-add")
            .h(px(30.))
            .px(px(12.))
            .rounded_md()
            .bg(theme.send_bg)
            .flex()
            .items_center()
            .gap_1p5()
            .cursor_pointer()
            .hover(|style| style.bg(theme.send_bg_hover))
            .on_mouse_up(MouseButton::Left, {
                let this = this.clone();
                move |_, window, cx| {
                    this.update(cx, |app, cx| app.provider_editor_open(None, window, cx));
                }
            })
            .child(icon("icons/plus.svg", 13., theme.send_fg))
            .child(
                div()
                    .text_size(theme.ui_px(12.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.send_fg)
                    .child("Add provider"),
            );

        let toolbar = div()
            .w_full()
            .flex()
            .items_center()
            .gap_2()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(theme.ui_px(12.))
                    .text_color(theme.text_3)
                    .child(format!(
                        "{} providers · {} active · {} connected",
                        all.len(),
                        active,
                        connected
                    )),
            )
            .child(refresh_button)
            .child(add_button);

        div()
            .w_full()
            .flex()
            .flex_col()
            .gap_3()
            .child(search)
            .child(toolbar)
            .into_any_element()
    }

    /// The Providers page: every provider pi ships with (plus models.json-only
    /// endpoints) as a 3-column grid, each with real auth status and actions.
    pub(super) fn provider_rows(
        &self,
        theme: Theme,
        this: Entity<OrbitApp>,
        cx: &Context<Self>,
    ) -> Vec<AnyElement> {
        let all = self.provider_views();
        let needle = self.provider_filter.read(cx).text().trim().to_lowercase();

        let mut views: Vec<&ProviderView> = all
            .iter()
            .filter(|view| {
                needle.is_empty()
                    || view.name.to_lowercase().contains(&needle)
                    || view.id.to_lowercase().contains(&needle)
                    || view
                        .env_names
                        .iter()
                        .any(|name| name.to_lowercase().contains(&needle))
            })
            .collect();
        let rank = |view: &ProviderView| -> u8 {
            if view.active {
                0
            } else if view.connected() {
                1
            } else if view.custom {
                2
            } else {
                3
            }
        };
        views.sort_by(|a, b| {
            rank(a)
                .cmp(&rank(b))
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });

        let mut rows: Vec<AnyElement> = Vec::new();

        // ── credential-changed banner ──
        if self.provider_auth_dirty {
            rows.push(
                div()
                    .w_full()
                    .bg(theme.accent.opacity(0.1))
                    .border_1()
                    .border_color(theme.accent.opacity(0.35))
                    .rounded_lg()
                    .px(px(14.))
                    .py(px(10.))
                    .flex()
                    .items_center()
                    .gap_2p5()
                    .child(icon("icons/info.svg", 15., theme.accent))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap_0p5()
                            .child(
                                div()
                                    .text_size(theme.ui_px(12.5))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme.text)
                                    .child("Credentials changed"),
                            )
                            .child(
                                div()
                                    .text_size(theme.ui_px(11.5))
                                    .text_color(theme.text_2)
                                    .child(
                                        "pi reads auth.json at startup — restart it to load the new credentials.",
                                    ),
                            ),
                    )
                    .child(self.provider_button(
                        "providers-apply".into(),
                        "Restart pi",
                        ProviderButtonStyle::Primary,
                        theme,
                        this.clone(),
                        ProviderAction::Restart,
                    ))
                    .into_any_element(),
            );
        }

        if let Some(error) = &self.custom_providers_error {
            rows.push(self.provider_error_card(theme, "models.json could not be read", error));
        }
        if let Some(error) = &self.provider_auth_error {
            rows.push(self.provider_error_card(theme, "auth.json could not be read", error));
        }

        // ── grid ──
        if views.is_empty() {
            rows.push(
                div()
                    .w_full()
                    .bg(theme.bg_composer)
                    .border_1()
                    .border_color(theme.border)
                    .rounded_lg()
                    .px(px(14.))
                    .py(px(40.))
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_2()
                    .child(icon("icons/search.svg", 26., theme.text_3))
                    .child(
                        div()
                            .text_size(theme.ui_px(13.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.text)
                            .child("No providers match"),
                    )
                    .child(
                        div()
                            .text_size(theme.ui_px(12.))
                            .text_color(theme.text_2)
                            .child("Try a different search."),
                    )
                    .into_any_element(),
            );
        } else {
            let cards: Vec<AnyElement> = views
                .iter()
                .map(|view| self.provider_card(view, theme, this.clone()))
                .collect();
            rows.push(
                div()
                    .w_full()
                    .grid()
                    .grid_cols(3)
                    .gap_3()
                    .children(cards)
                    .into_any_element(),
            );
        }

        rows
    }

    /// A red note card for a config read error.
    pub(super) fn provider_error_card(&self, theme: Theme, title: &str, error: &str) -> AnyElement {
        div()
            .w_full()
            .bg(theme.crit.opacity(0.08))
            .border_1()
            .border_color(theme.crit.opacity(0.35))
            .rounded_lg()
            .px(px(14.))
            .py(px(12.))
            .flex()
            .items_start()
            .gap_2p5()
            .child(icon("icons/info.svg", 15., theme.crit))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_size(theme.ui_px(13.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.text)
                            .child(title.to_string()),
                    )
                    .child(
                        div()
                            .text_size(theme.ui_px(12.))
                            .text_color(theme.text_2)
                            .child(error.to_string()),
                    )
                    .child(
                        div()
                            .text_size(theme.ui_px(11.5))
                            .text_color(theme.text_3)
                            .child("Fix or remove the file before editing providers here."),
                    ),
            )
            .into_any_element()
    }

    /// Every provider pi ships with, merged with the live catalog, models.json,
    /// and auth.json. Built-ins come first so nothing is hidden behind auth.
    pub(super) fn provider_views(&self) -> Vec<ProviderView> {
        let mut views: Vec<ProviderView> = Vec::new();
        // Prefer pi's own provider metadata (introspected from pi-ai). Fall
        // back to the curated table when node/pi-ai aren't reachable.
        if !self.provider_metadata.is_empty() {
            for provider in &self.provider_metadata {
                views.push(self.provider_view_for(
                    &provider.id,
                    Some(provider.name.clone()),
                    provider.oauth,
                    provider.api_key,
                    &provider.env_vars,
                    providers::provider_note(&provider.id),
                    true,
                ));
            }
        } else {
            for builtin in providers::BUILTIN_PROVIDERS {
                let env_vars: Vec<String> = if builtin.env_var.is_empty() {
                    Vec::new()
                } else {
                    vec![builtin.env_var.to_string()]
                };
                views.push(self.provider_view_for(
                    builtin.id,
                    Some(builtin.name.to_string()),
                    builtin.oauth,
                    builtin.api_key,
                    &env_vars,
                    builtin.note,
                    true,
                ));
            }
        }
        // Providers pi ships but neither source knows yet — discovered from
        // pi's bundled catalog data, so a new pi release lists its providers
        // without an Orbit change.
        for id in self.provider_catalog_counts.keys() {
            if !views.iter().any(|view| &view.id == id) {
                views.push(self.provider_view_for(
                    id,
                    None,
                    false,
                    true,
                    &[],
                    providers::provider_note(id),
                    true,
                ));
            }
        }
        // Catalog providers that aren't built-ins (extension providers, or
        // custom endpoints pi has already loaded).
        for model in &self.available_models {
            if !views.iter().any(|view| view.id == model.provider) {
                views.push(self.provider_view_for(
                    &model.provider,
                    None,
                    false,
                    true,
                    &[],
                    "",
                    false,
                ));
            }
        }
        // models.json providers pi hasn't loaded yet.
        for provider in &self.custom_providers {
            if !views.iter().any(|view| view.id == provider.id) {
                views.push(self.provider_view_for(&provider.id, None, false, true, &[], "", false));
            }
        }
        // Providers the running pi advertises through `auth.list` but no other
        // source knows about still get a card and capability-driven buttons.
        for capability in self.auth.providers() {
            if !views.iter().any(|view| view.id == capability.id) {
                views.push(self.provider_view_for(
                    &capability.id,
                    Some(capability.name.clone()),
                    capability.supports_oauth(),
                    capability.supports_api_key(),
                    &[],
                    providers::provider_note(&capability.id),
                    true,
                ));
            }
        }
        views
    }

    /// Build one grid row from all sources.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn provider_view_for(
        &self,
        id: &str,
        name: Option<String>,
        oauth: bool,
        api_key: bool,
        env_names: &[String],
        note: &'static str,
        builtin: bool,
    ) -> ProviderView {
        let custom = self
            .custom_providers
            .iter()
            .find(|provider| provider.id == id);
        let live_count = self
            .available_models
            .iter()
            .filter(|model| model.provider == id)
            .count();
        let catalog_count = self.provider_catalog_counts.get(id).copied().unwrap_or(0);
        let custom_count = custom.map(|provider| provider.model_ids.len()).unwrap_or(0);
        let env_hit = env_names
            .iter()
            .find(|name| std::env::var_os(name.as_str()).is_some())
            .cloned();
        ProviderView {
            id: id.to_string(),
            name: custom
                .and_then(|provider| provider.name.clone())
                .or(name)
                .unwrap_or_else(|| providers::provider_display_name(id)),
            active: live_count > 0,
            model_count: if live_count > 0 {
                live_count
            } else if catalog_count > 0 {
                catalog_count
            } else {
                custom_count
            },
            catalog_count,
            custom: custom.is_some(),
            has_api_key: custom.is_some_and(|provider| provider.has_api_key),
            base_url: custom
                .map(|provider| provider.base_url.clone())
                .unwrap_or_default(),
            api: custom
                .map(|provider| provider.api.clone())
                .unwrap_or_default(),
            builtin,
            oauth,
            api_key,
            env_names: env_names.to_vec(),
            env_authed: env_hit.is_some(),
            env_var: env_hit,
            note,
            auth: self.provider_auth.get(id).copied(),
            live_status: self.auth.status(id).cloned(),
            quota: self.quota.report(id).cloned(),
        }
    }

    /// The live auth block shown on a card while a login is Connecting,
    /// waiting on a device code, or showing a Success/Error/Cancelled result.
    /// `None` means the provider has no session and the card renders its
    /// normal Connect / Disconnect actions.
    pub(super) fn provider_auth_section(
        &self,
        view: &ProviderView,
        theme: Theme,
        this: Entity<OrbitApp>,
    ) -> Option<AnyElement> {
        let session = self.auth.login_for(&view.id)?;
        let (headline, detail, tint) = match session.phase {
            LoginPhase::Connecting => (
                "Connecting…",
                "Asking pi to start the sign-in flow.".to_string(),
                theme.warn,
            ),
            LoginPhase::AwaitingBrowser => (
                "Waiting for your browser…",
                "Finish signing in there, then return to Orbit.".to_string(),
                theme.accent,
            ),
            LoginPhase::AwaitingDeviceCode => (
                "Enter this device code",
                "Approve the request in your browser to continue.".to_string(),
                theme.accent,
            ),
            LoginPhase::Succeeded => (
                "Connected",
                "pi saved the credential to auth.json.".to_string(),
                theme.ok_green,
            ),
            LoginPhase::Error => (
                "Sign-in failed",
                session
                    .error
                    .as_ref()
                    .map(|(_, message)| message.clone())
                    .filter(|message| !message.is_empty())
                    .unwrap_or_else(|| "The sign-in did not complete.".to_string()),
                theme.crit,
            ),
            LoginPhase::Cancelled => (
                "Cancelled",
                session
                    .message
                    .clone()
                    .unwrap_or_else(|| "The sign-in was cancelled.".to_string()),
                theme.text_3,
            ),
        };

        let mut body = div()
            .w_full()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().size(px(7.)).rounded_full().bg(tint))
                    .child(
                        div()
                            .text_size(theme.ui_px(12.5))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(tint)
                            .child(headline),
                    ),
            )
            .child(
                div()
                    .text_size(theme.ui_px(11.5))
                    .text_color(theme.text_2)
                    .child(detail),
            );

        if let Some(device) = &session.device_code {
            body = body.child(
                div()
                    .w_full()
                    .px_3()
                    .py_2p5()
                    .rounded_md()
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.bg_main)
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_size(theme.ui_px(10.5))
                            .text_color(theme.text_3)
                            .child("Device code"),
                    )
                    .child(
                        div()
                            .font_family(theme::code_font_family())
                            .text_size(theme.code_px(16.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.text)
                            .child(device.user_code.clone()),
                    )
                    .child(
                        div()
                            .font_family(theme::code_font_family())
                            .text_size(theme.code_px(10.5))
                            .text_color(theme.text_3)
                            .truncate()
                            .child(device.verification_uri.clone()),
                    ),
            );
        }
        if session.device_code.is_none() {
            if let Some(url) = &session.url {
                body = body.child(
                    div()
                        .truncate()
                        .font_family(theme::code_font_family())
                        .text_size(theme.code_px(10.5))
                        .text_color(theme.text_3)
                        .child(url.clone()),
                );
            }
        }

        let id = view.id.clone();
        let name = view.name.clone();
        let method = session.method.clone();
        let mut buttons = div().flex().flex_wrap().items_center().gap_2();
        match session.phase {
            LoginPhase::Connecting
            | LoginPhase::AwaitingBrowser
            | LoginPhase::AwaitingDeviceCode => {
                if let Some(device) = &session.device_code {
                    let open = device
                        .verification_uri_complete
                        .clone()
                        .unwrap_or_else(|| device.verification_uri.clone());
                    buttons = buttons
                        .child(self.provider_button(
                            format!("provider-auth-open-{}", view.id),
                            "Open page",
                            ProviderButtonStyle::Primary,
                            theme,
                            this.clone(),
                            ProviderAction::AuthOpenUrl(open),
                        ))
                        .child(self.provider_button(
                            format!("provider-auth-copy-{}", view.id),
                            "Copy code",
                            ProviderButtonStyle::Ghost,
                            theme,
                            this.clone(),
                            ProviderAction::AuthCopy(device.user_code.clone()),
                        ));
                }
                buttons = buttons.child(self.provider_button(
                    format!("provider-auth-cancel-{}", view.id),
                    "Cancel",
                    ProviderButtonStyle::Ghost,
                    theme,
                    this.clone(),
                    ProviderAction::AuthCancel,
                ));
            }
            LoginPhase::Succeeded => {
                buttons = buttons.child(self.provider_button(
                    format!("provider-auth-done-{}", view.id),
                    "Done",
                    ProviderButtonStyle::Primary,
                    theme,
                    this.clone(),
                    ProviderAction::AuthDismiss,
                ));
            }
            LoginPhase::Error | LoginPhase::Cancelled => {
                buttons = buttons
                    .child(self.provider_button(
                        format!("provider-auth-retry-{}", view.id),
                        "Try again",
                        ProviderButtonStyle::Primary,
                        theme,
                        this.clone(),
                        ProviderAction::AuthStart {
                            id: id.clone(),
                            name: name.clone(),
                            method: method.clone(),
                        },
                    ))
                    .child(self.provider_button(
                        format!("provider-auth-dismiss-{}", view.id),
                        "Dismiss",
                        ProviderButtonStyle::Ghost,
                        theme,
                        this.clone(),
                        ProviderAction::AuthDismiss,
                    ));
            }
        }
        body = body.child(buttons);
        Some(body.into_any_element())
    }

    /// Account quota/balance/spend for a connected provider, from the
    /// `quota.*` RPC namespace. `None` when there is nothing to render, so a
    /// provider with no usage surface adds no empty block to its card.
    pub(super) fn provider_quota_section(
        &self,
        view: &ProviderView,
        theme: Theme,
    ) -> Option<AnyElement> {
        let report = view.quota.as_ref()?;
        if !report.has_data() && report.error.is_none() && report.note.is_none() {
            return None;
        }

        let amount = |value: f64| -> String {
            if value.fract() == 0.0 {
                format!("{value:.0}")
            } else {
                format!("{value:.2}")
            }
        };
        let reset_label = |ms: i64| format!("resets {}", format_epoch_ms(ms));

        let mut head = div().flex().items_center().gap_2().child(
            div()
                .text_size(theme.ui_px(10.5))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.text_3)
                .child("USAGE"),
        );
        if let Some(plan) = &report.plan {
            head = head.child(self.provider_badge(
                plan,
                theme.accent,
                theme.accent.opacity(0.12),
                theme,
            ));
        }

        let mut body = div()
            .w_full()
            .flex()
            .flex_col()
            .gap_2()
            .px_3()
            .py_2p5()
            .rounded_md()
            .border_1()
            .border_color(theme.border)
            .bg(theme.bg_main)
            .child(head);

        for window in &report.windows {
            let value = if let Some(percent) = window.used_percent {
                format!("{percent:.0}% used")
            } else if let (Some(used), Some(limit)) = (window.used, window.limit) {
                format!("{} / {}", amount(used), amount(limit))
            } else if let Some(used) = window.used {
                match &window.unit {
                    Some(unit) => format!("{} {unit}", amount(used)),
                    None => amount(used),
                }
            } else {
                continue;
            };

            let mut row = div().w_full().flex().flex_col().gap_1().child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(theme.ui_px(11.))
                            .text_color(theme.text_2)
                            .child(window.label.clone()),
                    )
                    .child(
                        div()
                            .flex_none()
                            .text_size(theme.ui_px(11.))
                            .text_color(theme.text)
                            .child(value),
                    ),
            );

            if let Some(fraction) = window.fraction() {
                let tint = if fraction >= 0.9 {
                    theme.crit
                } else if fraction >= 0.75 {
                    theme.warn
                } else {
                    theme.ok_green
                };
                row = row.child(
                    div()
                        .w_full()
                        .h(px(4.))
                        .rounded_full()
                        .overflow_hidden()
                        .bg(theme.overlay_strong)
                        .child(div().h_full().rounded_full().bg(tint).w(relative(fraction))),
                );
            }
            if let Some(resets_at) = window.resets_at {
                row = row.child(
                    div()
                        .text_size(theme.ui_px(10.))
                        .text_color(theme.text_3)
                        .child(reset_label(resets_at)),
                );
            }
            body = body.child(row);
        }

        for balance in &report.balances {
            let text = if balance.currency.is_empty() {
                amount(balance.amount)
            } else {
                format!("{} {}", amount(balance.amount), balance.currency)
            };
            body = body.child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(theme.ui_px(11.))
                            .text_color(theme.text_2)
                            .child(balance.label.clone()),
                    )
                    .child(
                        div()
                            .flex_none()
                            .text_size(theme.ui_px(11.))
                            .text_color(theme.text)
                            .child(text),
                    ),
            );
        }

        if let Some(error) = &report.error {
            body = body.child(
                div()
                    .text_size(theme.ui_px(10.5))
                    .text_color(theme.crit)
                    .child(error.to_string()),
            );
        } else if !report.has_data() {
            if let Some(note) = &report.note {
                body = body.child(
                    div()
                        .text_size(theme.ui_px(10.5))
                        .text_color(theme.text_3)
                        .child(note.to_string()),
                );
            }
        }

        Some(body.into_any_element())
    }

    /// One large provider card: huge brand mark, status, auth facts, actions.
    pub(super) fn provider_card(
        &self,
        view: &ProviderView,
        theme: Theme,
        this: Entity<OrbitApp>,
    ) -> AnyElement {
        let confirming = self.provider_remove_confirm.as_deref() == Some(view.id.as_str());
        let session = self.auth.login_for(&view.id);
        let (status_label, status_color) = if let Some(session) = session {
            match session.phase {
                LoginPhase::Connecting => ("Connecting", theme.warn),
                LoginPhase::AwaitingBrowser | LoginPhase::AwaitingDeviceCode => {
                    ("Connecting", theme.accent)
                }
                LoginPhase::Succeeded => ("Connected", theme.ok_green),
                LoginPhase::Error => ("Sign-in failed", theme.crit),
                LoginPhase::Cancelled => ("Cancelled", theme.text_3),
            }
        } else if view.active {
            ("Active", theme.ok_green)
        } else if view.connected() {
            ("Connected", theme.accent)
        } else if view.custom {
            ("Not loaded", theme.warn)
        } else {
            ("Not configured", theme.text_3)
        };

        let tile = div()
            .size(px(52.))
            .flex_none()
            .rounded(px(14.))
            .bg(theme.bg_raised)
            .border_1()
            .border_color(theme.border)
            .flex()
            .items_center()
            .justify_center()
            .child(icon_dyn(
                provider_icon(&view.id),
                30.,
                if view.active || view.connected() {
                    theme.text
                } else {
                    theme.text_2
                },
            ));

        let status_pill = div()
            .h(px(22.))
            .px(px(8.))
            .rounded_full()
            .flex_none()
            .flex()
            .items_center()
            .gap_1p5()
            .bg(status_color.opacity(0.12))
            .child(div().size(px(6.)).rounded_full().bg(status_color))
            .child(
                div()
                    .text_size(theme.ui_px(11.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(status_color)
                    .child(status_label),
            );

        let header = div().flex().items_start().gap_3().child(tile).child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_size(theme.ui_px(14.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.text)
                        .truncate()
                        .child(view.name.clone()),
                )
                .child(
                    div()
                        .font_family(theme::code_font_family())
                        .text_size(theme.code_px(10.5))
                        .text_color(theme.text_3)
                        .truncate()
                        .child(view.id.clone()),
                ),
        );

        // Status and source badges share a row under the name so the name
        // column keeps the card's full width (a right-aligned pill in the
        // header squeezes it on narrow columns).
        let mut badges = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_1p5()
            .child(status_pill);
        if view.oauth {
            badges = badges.child(self.provider_badge(
                "OAuth",
                theme.accent,
                theme.accent.opacity(0.12),
                theme,
            ));
        }
        if view.api_key {
            badges = badges.child(self.provider_badge(
                "API key",
                theme.text_3,
                theme.overlay_strong,
                theme,
            ));
        }
        badges = badges.child(self.provider_badge(
            if view.custom {
                "Custom"
            } else if view.builtin {
                "Built-in"
            } else {
                "Provider"
            },
            if view.custom {
                theme.accent
            } else {
                theme.text_3
            },
            if view.custom {
                theme.accent.opacity(0.12)
            } else {
                theme.overlay_strong
            },
            theme,
        ));

        // Active providers show what pi is serving; unconfigured built-ins show
        // their full built-in catalog size (pi's RPC never reports those).
        let count_label = if !view.active && view.catalog_count > 0 {
            format!("{} in catalog", view.catalog_count)
        } else {
            format!(
                "{} model{}",
                view.model_count,
                if view.model_count == 1 { "" } else { "s" }
            )
        };
        let mut facts = div()
            .flex()
            .items_center()
            .gap_1p5()
            .text_size(theme.ui_px(11.5))
            .text_color(theme.text_3)
            .child(count_label);
        if let Some(kind) = view.credential_kind() {
            facts = facts
                .child(div().size(px(3.)).rounded_full().bg(theme.text_3))
                .child(match kind {
                    "oauth" => "OAuth (auth.json)".to_string(),
                    "api_key" => "API key (auth.json)".to_string(),
                    other => other.to_string(),
                });
            if let Some(status) = &view.live_status {
                if let Some(account) = &status.account {
                    facts = facts
                        .child(div().size(px(3.)).rounded_full().bg(theme.text_3))
                        .child(account.clone());
                }
                if let Some(expires_at) = status.expires_at {
                    facts = facts
                        .child(div().size(px(3.)).rounded_full().bg(theme.text_3))
                        .child(format!("expires {}", format_epoch_ms(expires_at)));
                }
            }
        } else if let Some(env_var) = &view.env_var {
            facts = facts
                .child(div().size(px(3.)).rounded_full().bg(theme.text_3))
                .child(format!("via {env_var}"));
        }
        if view.has_api_key {
            facts = facts
                .child(div().size(px(3.)).rounded_full().bg(theme.text_3))
                .child("models.json key");
        }

        let endpoint = if view.base_url.is_empty() {
            if view.note.is_empty() {
                "pi default endpoint".to_string()
            } else {
                view.note.to_string()
            }
        } else if view.api.is_empty() {
            view.base_url.clone()
        } else {
            format!("{} · {}", view.base_url, view.api)
        };
        let base_url = div()
            .truncate()
            .font_family(theme::code_font_family())
            .text_size(theme.code_px(10.5))
            .text_color(theme.text_3)
            .child(endpoint);

        let actions: AnyElement = if confirming {
            div()
                .w_full()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_size(theme.ui_px(11.5))
                        .text_color(theme.crit)
                        .child("Remove this provider?"),
                )
                .child(self.provider_button(
                    format!("provider-remove-confirm-{}", view.id),
                    "Remove",
                    ProviderButtonStyle::Danger,
                    theme,
                    this.clone(),
                    ProviderAction::ConfirmRemove {
                        id: view.id.clone(),
                    },
                ))
                .child(self.provider_button(
                    format!("provider-remove-cancel-{}", view.id),
                    "Cancel",
                    ProviderButtonStyle::Ghost,
                    theme,
                    this.clone(),
                    ProviderAction::CancelRemove,
                ))
                .into_any_element()
        } else if let Some(section) = self.provider_auth_section(view, theme, this.clone()) {
            section
        } else {
            // Methods come from pi's `auth.list` capability discovery — the
            // UI never hardcodes provider ids or assumes a login flow.
            let capability = if self.auth.support() == AuthSupport::Supported {
                self.auth.provider(&view.id)
            } else {
                None
            };
            let mut primary = div().flex().flex_wrap().items_center().gap_2();
            if let Some(capability) = capability {
                for method in &capability.methods {
                    if method.id == "api_key" {
                        let update = view.credential_kind() == Some("api_key");
                        primary = primary.child(self.provider_button(
                            format!("provider-key-{}", view.id),
                            if update { "Update key" } else { "Add API key" },
                            ProviderButtonStyle::Ghost,
                            theme,
                            this.clone(),
                            ProviderAction::EditKey {
                                id: view.id.clone(),
                                name: view.name.clone(),
                                oauth: view.oauth,
                                note: view.note,
                            },
                        ));
                    } else {
                        let label = method_label(&method.id, &method.label, view.connected());
                        primary = primary.child(self.provider_button(
                            format!("provider-auth-{}-{}", method.id, view.id),
                            &label,
                            ProviderButtonStyle::Primary,
                            theme,
                            this.clone(),
                            ProviderAction::AuthStart {
                                id: view.id.clone(),
                                name: view.name.clone(),
                                method: method.id.clone(),
                            },
                        ));
                    }
                }
                if capability.methods.is_empty() {
                    primary = primary.child(
                        div()
                            .text_size(theme.ui_px(11.))
                            .text_color(theme.text_3)
                            .child("No sign-in methods available"),
                    );
                }
            } else {
                // File-based fallback for a pi without the auth RPC.
                if view.oauth {
                    let reconnect = view.credential_kind() == Some("oauth");
                    primary = primary.child(self.provider_button(
                        format!("provider-signin-{}", view.id),
                        if reconnect { "Reconnect" } else { "Sign in" },
                        ProviderButtonStyle::Primary,
                        theme,
                        this.clone(),
                        ProviderAction::SignIn {
                            id: view.id.clone(),
                            name: view.name.clone(),
                        },
                    ));
                }
                if view.api_key {
                    let update = view.credential_kind() == Some("api_key");
                    primary = primary.child(self.provider_button(
                        format!("provider-key-{}", view.id),
                        if update { "Update key" } else { "Add API key" },
                        if view.oauth && !view.connected() {
                            ProviderButtonStyle::Ghost
                        } else {
                            ProviderButtonStyle::Primary
                        },
                        theme,
                        this.clone(),
                        ProviderAction::EditKey {
                            id: view.id.clone(),
                            name: view.name.clone(),
                            oauth: view.oauth,
                            note: view.note,
                        },
                    ));
                }
            }
            // Ollama Cloud usage is separate from the local endpoint's API key:
            // current accounts use a real cloud key (monthly credits); legacy
            // accounts expose session/weekly usage only behind a signed-in
            // settings page, which needs the user's own session cookie. Offer
            // the session editor explicitly so neither is auto-scraped.
            if view.id == "ollama" {
                primary = primary.child(self.provider_button(
                    "provider-ollama-session".to_string(),
                    "Usage session",
                    ProviderButtonStyle::Ghost,
                    theme,
                    this.clone(),
                    ProviderAction::EditOllamaSession {
                        name: view.name.clone(),
                    },
                ));
            }
            let connected = view.auth.is_some()
                || view
                    .live_status
                    .as_ref()
                    .is_some_and(|status| status.authenticated);
            // Secondary actions: Configure/Remove only exist when there is a
            // models.json entry to edit; Sign out only when a credential is
            // stored. Built-ins with neither show just the auth button.
            let mut secondary = div().flex().flex_wrap().items_center().gap_1p5();
            if view.custom {
                secondary = secondary.child(self.provider_button(
                    format!("provider-configure-{}", view.id),
                    "Configure",
                    ProviderButtonStyle::Ghost,
                    theme,
                    this.clone(),
                    ProviderAction::Configure {
                        id: view.id.clone(),
                    },
                ));
            }
            if connected {
                secondary = secondary.child(self.provider_button(
                    format!("provider-signout-{}", view.id),
                    "Disconnect",
                    ProviderButtonStyle::Ghost,
                    theme,
                    this.clone(),
                    ProviderAction::SignOut {
                        id: view.id.clone(),
                    },
                ));
            }
            if view.custom {
                secondary = secondary.child(self.provider_button(
                    format!("provider-remove-{}", view.id),
                    "Remove",
                    ProviderButtonStyle::Danger,
                    theme,
                    this.clone(),
                    ProviderAction::Remove {
                        id: view.id.clone(),
                    },
                ));
            }
            let mut actions = div().w_full().flex().flex_col().gap_2().child(primary);
            if view.custom || connected {
                actions = actions.child(secondary);
            }
            actions.into_any_element()
        };

        div()
            .w_full()
            .min_w_0()
            .bg(theme.bg_composer)
            .border_1()
            .border_color(theme.border)
            .rounded_lg()
            .p(px(14.))
            .flex()
            .flex_col()
            .gap_2p5()
            .child(header)
            .child(badges)
            .child(facts)
            .when_some(self.provider_quota_section(view, theme), |card, section| {
                card.child(section)
            })
            .child(base_url)
            .child(div().h(px(1.)).w_full().bg(theme.border))
            .child(actions)
            .into_any_element()
    }

    /// A small status/source pill on a provider card.
    pub(super) fn provider_badge(
        &self,
        label: &str,
        fg: Hsla,
        bg: Hsla,
        theme: Theme,
    ) -> AnyElement {
        div()
            .h(px(20.))
            .px(px(7.))
            .rounded(px(6.))
            .flex_none()
            .flex()
            .items_center()
            .bg(bg)
            .child(
                div()
                    .text_size(theme.ui_px(10.5))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(fg)
                    .child(label.to_string()),
            )
            .into_any_element()
    }

    /// A provider-card action button. One closure per card would be a lot of
    /// near-identical code, so every button routes through
    /// [`Self::apply_provider_action`]. The glyph is derived from the action.
    pub(super) fn provider_button(
        &self,
        id: String,
        label: &str,
        style: ProviderButtonStyle,
        theme: Theme,
        this: Entity<OrbitApp>,
        action: ProviderAction,
    ) -> AnyElement {
        let icon_path = Self::provider_action_icon(&action);
        let mut button = div()
            .id(ElementId::Name(id.into()))
            .h(px(28.))
            .px(px(10.))
            .rounded_md()
            .flex()
            .items_center()
            .justify_center()
            .gap_1p5()
            .cursor_pointer()
            .text_size(theme.ui_px(11.5))
            .font_weight(FontWeight::MEDIUM);
        button = match style {
            ProviderButtonStyle::Primary => button
                .bg(theme.send_bg)
                .text_color(theme.send_fg)
                .hover(|style| style.bg(theme.send_bg_hover)),
            ProviderButtonStyle::Ghost => button
                .border_1()
                .border_color(theme.border)
                .bg(theme.bg_raised)
                .text_color(theme.text_2)
                .hover(|style| style.bg(theme.bg_hover)),
            ProviderButtonStyle::Danger => button
                .border_1()
                .border_color(theme.border)
                .bg(theme.bg_raised)
                .text_color(theme.crit)
                .hover(|style| {
                    style
                        .border_color(theme.crit.opacity(0.6))
                        .bg(theme.crit.opacity(0.08))
                }),
        };
        let icon_color = match style {
            ProviderButtonStyle::Primary => theme.send_fg,
            ProviderButtonStyle::Danger => theme.crit,
            ProviderButtonStyle::Ghost => theme.text_2,
        };
        button
            .when_some(icon_path, |button, path| {
                button.child(icon(path, 12., icon_color))
            })
            .child(div().child(label.to_string()))
            .on_mouse_up(MouseButton::Left, move |_, window, cx| {
                let action = action.clone();
                this.update(cx, |app, cx| app.apply_provider_action(action, window, cx));
            })
            .into_any_element()
    }

    /// The glyph for a provider action (none for plain text buttons).
    pub(super) fn provider_action_icon(action: &ProviderAction) -> Option<&'static str> {
        match action {
            ProviderAction::SignIn { .. } | ProviderAction::AuthStart { .. } => {
                Some("icons/lock.svg")
            }
            ProviderAction::AuthOpenUrl(_) => Some("icons/arrow-up-right.svg"),
            ProviderAction::AuthCopy(_) => Some("icons/copy.svg"),
            ProviderAction::AuthCancel => Some("icons/x.svg"),
            ProviderAction::AuthDismiss => Some("icons/check.svg"),
            ProviderAction::EditKey { .. } => Some("icons/at-sign.svg"),
            ProviderAction::EditOllamaSession { .. } => Some("icons/clock.svg"),
            ProviderAction::SignOut { .. } => Some("icons/stop.svg"),
            ProviderAction::Configure { .. } => Some("icons/settings.svg"),
            ProviderAction::Remove { .. } | ProviderAction::ConfirmRemove { .. } => {
                Some("icons/trash.svg")
            }
            ProviderAction::Restart => Some("icons/refresh.svg"),
            ProviderAction::CancelRemove | ProviderAction::SaveKey => None,
        }
    }

    /// Single dispatch point for every provider-card button.
    pub(super) fn apply_provider_action(
        &mut self,
        action: ProviderAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match action {
            ProviderAction::SignIn { id, name } => {
                // Signing in from the key modal replaces it.
                self.provider_key_editor = None;
                self.provider_oauth_login(id, name, cx);
            }
            ProviderAction::AuthStart { id, name, method } => {
                self.provider_key_editor = None;
                self.auth_start_login(id, name, method, cx);
            }
            ProviderAction::AuthCancel => self.auth_cancel_login(cx),
            ProviderAction::AuthDismiss => {
                self.auth.dismiss_result();
                cx.notify();
            }
            ProviderAction::AuthOpenUrl(url) => {
                if let Err(err) = platform::open_url(&url) {
                    self.set_status(format!("Could not open the browser: {err}"));
                }
            }
            ProviderAction::AuthCopy(value) => {
                cx.write_to_clipboard(gpui::ClipboardItem::new_string(value));
                self.set_status("Copied to clipboard");
            }
            ProviderAction::EditKey {
                id,
                name,
                oauth,
                note,
            } => self.provider_key_open(id, name, oauth, note, window, cx),
            ProviderAction::EditOllamaSession { name } => self.provider_credential_open(
                "ollama".to_string(),
                name,
                false,
                "",
                ProviderKeyKind::OllamaCloudSession,
                window,
                cx,
            ),
            ProviderAction::SignOut { id } => self.provider_sign_out(id, cx),
            ProviderAction::Configure { id } => self.provider_editor_open(Some(id), window, cx),
            ProviderAction::Remove { id } => {
                self.provider_remove_confirm = Some(id);
                cx.notify();
            }
            ProviderAction::ConfirmRemove { id } => self.provider_remove(id, cx),
            ProviderAction::CancelRemove => {
                self.provider_remove_confirm = None;
                cx.notify();
            }
            ProviderAction::SaveKey => self.provider_key_save(window, cx),
            ProviderAction::Restart => self.provider_apply_credentials(cx),
        }
    }

    /// The API-key modal, or `None` when closed.
    pub(super) fn provider_key_layer(
        &self,
        theme: Theme,
        this: Entity<OrbitApp>,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let editor = self.provider_key_editor.as_ref()?;

        let (field_label, hint) = match editor.kind {
            ProviderKeyKind::ApiKey => (
                "API key",
                "A literal key, `$ENV_VAR`, or `!command` — stored in auth.json (0600).",
            ),
            ProviderKeyKind::OllamaCloudSession => (
                "Session cookie",
                "Paste the Cookie header from ollama.com/settings (e.g. `__Secure-session=…`). Stored in auth.json (0600); only sent to ollama.com.",
            ),
        };

        let mut body = div().w_full().flex().flex_col().gap_3().child(
            div()
                .flex()
                .flex_col()
                .gap_1p5()
                .child(
                    div()
                        .text_size(theme.ui_px(12.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.text_2)
                        .child(field_label),
                )
                .child(
                    div()
                        .w_full()
                        .px_2p5()
                        .py_1p5()
                        .rounded_md()
                        .border_1()
                        .border_color(theme.border)
                        .bg(theme.bg_main)
                        .text_size(theme.ui_px(13.))
                        .child(editor.key.clone()),
                )
                .child(
                    div()
                        .text_size(theme.ui_px(11.))
                        .text_color(theme.text_3)
                        .child(hint),
                ),
        );
        if !editor.note.is_empty() {
            body = body.child(
                div()
                    .text_size(theme.ui_px(11.))
                    .text_color(theme.text_3)
                    .child(editor.note.to_string()),
            );
        }
        if editor.oauth && editor.kind == ProviderKeyKind::ApiKey {
            let id = editor.provider_id.clone();
            let name = editor.provider_name.clone();
            let this_signin = this.clone();
            body = body.child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .text_size(theme.ui_px(11.))
                            .text_color(theme.text_3)
                            .child("Prefer a subscription? Sign in with OAuth instead."),
                    )
                    .child(self.provider_button(
                        format!("provider-key-signin-{id}"),
                        "Sign in",
                        ProviderButtonStyle::Ghost,
                        theme,
                        this_signin,
                        ProviderAction::SignIn { id, name },
                    )),
            );
        }
        if let Some(error) = &editor.error {
            body = body.child(
                div()
                    .px(px(10.))
                    .py(px(8.))
                    .rounded_md()
                    .bg(theme.crit.opacity(0.1))
                    .text_size(theme.ui_px(11.5))
                    .text_color(theme.crit)
                    .child(error.clone()),
            );
        }

        let cancel = div()
            .id("provider-key-close")
            .h(px(32.))
            .px(px(14.))
            .rounded_md()
            .border_1()
            .border_color(theme.border)
            .bg(theme.bg_raised)
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .hover(|style| style.bg(theme.bg_hover))
            .on_mouse_up(MouseButton::Left, {
                let this = this.clone();
                move |_, _, cx| {
                    this.update(cx, |app, cx| {
                        app.provider_key_editor = None;
                        cx.notify();
                    });
                }
            })
            .child(
                div()
                    .text_size(theme.ui_px(12.))
                    .text_color(theme.text_2)
                    .child("Cancel"),
            );
        let save = self.provider_button(
            "provider-key-save".into(),
            "Save key",
            ProviderButtonStyle::Primary,
            theme,
            this.clone(),
            ProviderAction::SaveKey,
        );

        let card = div()
            .w_full()
            .max_w(px(460.))
            .rounded(px(14.))
            .border_1()
            .border_color(theme.border_strong)
            .bg(theme.menu_bg)
            .shadow(theme.popover_shadow())
            .flex()
            .flex_col()
            .overflow_hidden()
            .occlude()
            .font_family(theme::ui_font_family())
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_action(cx.listener(Self::provider_editor_cancel))
            .on_action(cx.listener(Self::provider_editor_confirm))
            .child(
                div()
                    .px(px(18.))
                    .py(px(14.))
                    .flex_none()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .text_size(theme.ui_px(15.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.text)
                            .child(format!("API key — {}", editor.provider_name)),
                    )
                    .child(
                        div()
                            .font_family(theme::code_font_family())
                            .text_size(theme.code_px(11.))
                            .text_color(theme.text_3)
                            .child(editor.provider_id.clone()),
                    ),
            )
            .child(div().px(px(18.)).py(px(16.)).child(body))
            .child(
                div()
                    .flex_none()
                    .px(px(18.))
                    .py(px(12.))
                    .flex()
                    .items_center()
                    .justify_end()
                    .gap_2()
                    .border_t_1()
                    .border_color(theme.border)
                    .child(cancel)
                    .child(save),
            );

        let scrim = match theme.mode {
            ThemeMode::Dark => Hsla {
                h: 0.,
                s: 0.,
                l: 0.,
                a: 0.42,
            },
            ThemeMode::Light => Hsla {
                h: 0.,
                s: 0.,
                l: 0.,
                a: 0.22,
            },
        };
        Some(
            div()
                .id("provider-key-layer")
                .absolute()
                .inset_0()
                .occlude()
                .bg(scrim)
                .px(px(24.))
                .flex()
                .items_center()
                .justify_center()
                .on_mouse_down(MouseButton::Left, cx.listener(Self::provider_editor_scrim))
                .child(card)
                .into_any_element(),
        )
    }

    /// The provider editor modal (add / configure), or `None` when closed.
    pub(super) fn provider_editor_layer(
        &self,
        theme: Theme,
        this: Entity<OrbitApp>,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let editor = self.provider_editor.as_ref()?;
        let editing = editor.original_id.is_some();
        let is_custom_entry = editor.original_id.as_ref().is_some_and(|id| {
            self.custom_providers
                .iter()
                .any(|provider| &provider.id == id)
        });
        let subtitle = if editing && editor.in_catalog && !is_custom_entry {
            "Built-in provider — entries here override pi's defaults. Sign in with `pi /login`."
        } else {
            "Saved to ~/.pi/agent/models.json — shared with the pi CLI."
        };

        let field = |label: &str, hint: Option<&str>, input: Entity<ComposerInput>| -> AnyElement {
            let mut column = div()
                .flex()
                .flex_col()
                .gap_1p5()
                .child(
                    div()
                        .text_size(theme.ui_px(12.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.text_2)
                        .child(label.to_string()),
                )
                .child(
                    div()
                        .w_full()
                        .px_2p5()
                        .py_1p5()
                        .rounded_md()
                        .border_1()
                        .border_color(theme.border)
                        .bg(theme.bg_main)
                        .text_size(theme.ui_px(13.))
                        .child(input),
                );
            if let Some(hint) = hint {
                column = column.child(
                    div()
                        .text_size(theme.ui_px(11.))
                        .text_color(theme.text_3)
                        .child(hint.to_string()),
                );
            }
            column.into_any_element()
        };

        // API family chips.
        let mut api_chips = div().flex().flex_wrap().gap_1p5();
        for api in PROVIDER_APIS {
            let selected = editor.api == api;
            let this = this.clone();
            api_chips = api_chips.child(
                div()
                    .id(ElementId::Name(format!("provider-api-{api}").into()))
                    .px(px(8.))
                    .py(px(3.))
                    .rounded(px(6.))
                    .border_1()
                    .border_color(if selected {
                        theme.accent.opacity(0.5)
                    } else {
                        theme.border
                    })
                    .bg(if selected {
                        theme.accent.opacity(0.12)
                    } else {
                        theme.bg_raised
                    })
                    .cursor_pointer()
                    .hover(|style| style.bg(theme.bg_hover))
                    .on_mouse_up(MouseButton::Left, move |_, _, cx| {
                        this.update(cx, |app, cx| {
                            if let Some(editor) = app.provider_editor.as_mut() {
                                editor.api = api.to_string();
                                cx.notify();
                            }
                        });
                    })
                    .child(
                        div()
                            .font_family(theme::code_font_family())
                            .text_size(theme.code_px(10.5))
                            .text_color(if selected { theme.accent } else { theme.text_3 })
                            .child(api),
                    ),
            );
        }

        // Identity: editable only when adding.
        let identity: AnyElement = if editing {
            div()
                .flex()
                .flex_col()
                .gap_1p5()
                .child(
                    div()
                        .text_size(theme.ui_px(12.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.text_2)
                        .child("Provider id"),
                )
                .child(
                    div()
                        .font_family(theme::code_font_family())
                        .text_size(theme.code_px(12.))
                        .text_color(theme.text)
                        .child(editor.original_id.clone().unwrap_or_default()),
                )
                .into_any_element()
        } else {
            field(
                "Provider id",
                Some("The key pi addresses the provider by — models become <id>/<model>."),
                editor.id.clone(),
            )
        };

        let api_key_hint = if editor.had_api_key {
            "A key is stored in models.json — leave blank to keep it."
        } else {
            "Optional. `$ENV_VAR`, `!command`, or a literal key."
        };

        let body = div()
            .w_full()
            .flex()
            .flex_col()
            .gap_3p5()
            .child(identity)
            .child(field(
                "Display name",
                Some("Optional."),
                editor.name.clone(),
            ))
            .child(field(
                "Base URL",
                Some("Required for custom endpoints. Empty keeps pi's default."),
                editor.base_url.clone(),
            ))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1p5()
                    .child(
                        div()
                            .text_size(theme.ui_px(12.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.text_2)
                            .child("API type"),
                    )
                    .child(api_chips),
            )
            .child(field("API key", Some(api_key_hint), editor.api_key.clone()))
            .child(field(
                "Models",
                Some(if editor.in_catalog {
                    "Comma-separated ids. Empty keeps pi's built-in models for this provider."
                } else {
                    "Comma-separated ids. At least one is required for a custom provider."
                }),
                editor.models.clone(),
            ));

        let mut card = div()
            .w_full()
            .max_w(px(520.))
            .max_h(px(560.))
            .rounded(px(14.))
            .border_1()
            .border_color(theme.border_strong)
            .bg(theme.menu_bg)
            .shadow(theme.popover_shadow())
            .flex()
            .flex_col()
            .overflow_hidden()
            .occlude()
            .font_family(theme::ui_font_family())
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_action(cx.listener(Self::provider_editor_cancel))
            .on_action(cx.listener(Self::provider_editor_confirm))
            .child(
                div()
                    .px(px(18.))
                    .py(px(14.))
                    .flex_none()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .text_size(theme.ui_px(15.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.text)
                            .child(if editing {
                                "Configure provider"
                            } else {
                                "Add provider"
                            }),
                    )
                    .child(
                        div()
                            .text_size(theme.ui_px(11.5))
                            .text_color(theme.text_3)
                            .child(subtitle),
                    ),
            )
            .child(
                div()
                    .id("provider-editor-body")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px(px(18.))
                    .py(px(16.))
                    .child(body),
            );

        if let Some(error) = &editor.error {
            card = card.child(
                div()
                    .mx(px(18.))
                    .mb(px(4.))
                    .px(px(10.))
                    .py(px(8.))
                    .rounded_md()
                    .bg(theme.crit.opacity(0.1))
                    .text_size(theme.ui_px(11.5))
                    .text_color(theme.crit)
                    .child(error.clone()),
            );
        }

        let cancel = {
            let this = this.clone();
            div()
                .id("provider-editor-cancel")
                .h(px(32.))
                .px(px(14.))
                .rounded_md()
                .border_1()
                .border_color(theme.border)
                .bg(theme.bg_raised)
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .hover(|style| style.bg(theme.bg_hover))
                .on_mouse_up(MouseButton::Left, move |_, _, cx| {
                    this.update(cx, |app, cx| {
                        app.provider_editor = None;
                        cx.notify();
                    });
                })
                .child(
                    div()
                        .text_size(theme.ui_px(12.))
                        .text_color(theme.text_2)
                        .child("Cancel"),
                )
        };
        let save = {
            let this = this.clone();
            div()
                .id("provider-editor-save")
                .h(px(32.))
                .px(px(16.))
                .rounded_md()
                .bg(theme.send_bg)
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .hover(|style| style.bg(theme.send_bg_hover))
                .on_mouse_up(MouseButton::Left, move |_, window, cx| {
                    this.update(cx, |app, cx| app.provider_save(window, cx));
                })
                .child(
                    div()
                        .text_size(theme.ui_px(12.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.send_fg)
                        .child(if editing {
                            "Save changes"
                        } else {
                            "Add provider"
                        }),
                )
        };
        card = card.child(
            div()
                .flex_none()
                .px(px(18.))
                .py(px(12.))
                .flex()
                .items_center()
                .justify_end()
                .gap_2()
                .border_t_1()
                .border_color(theme.border)
                .child(cancel)
                .child(save),
        );

        let scrim = match theme.mode {
            ThemeMode::Dark => Hsla {
                h: 0.,
                s: 0.,
                l: 0.,
                a: 0.42,
            },
            ThemeMode::Light => Hsla {
                h: 0.,
                s: 0.,
                l: 0.,
                a: 0.22,
            },
        };
        Some(
            div()
                .id("provider-editor-layer")
                .absolute()
                .inset_0()
                .occlude()
                .bg(scrim)
                .px(px(24.))
                .flex()
                .items_center()
                .justify_center()
                .on_mouse_down(MouseButton::Left, cx.listener(Self::provider_editor_scrim))
                .child(card)
                .into_any_element(),
        )
    }

    /// A setting card: title + description on the left, optional control on
    /// the right.
    pub(super) fn card(
        &self,
        theme: Theme,
        title: &str,
        desc: &str,
        control: Option<AnyElement>,
    ) -> AnyElement {
        self.card_with_path(theme, title, desc, None, control)
    }

    /// Same as [`card`] with an optional dimmed third line (paths).
    pub(super) fn card_with_path(
        &self,
        theme: Theme,
        title: &str,
        desc: &str,
        path: Option<&str>,
        control: Option<AnyElement>,
    ) -> AnyElement {
        div()
            .bg(theme.bg_composer)
            .border_1()
            .border_color(theme.border)
            .rounded_lg()
            .px(theme.space(14.))
            .py(theme.space(12.))
            .flex()
            .items_center()
            .gap_3()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_size(theme.ui_px(13.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.text)
                            .child(title.to_string()),
                    )
                    .child(
                        div()
                            .text_size(theme.ui_px(12.))
                            .text_color(theme.text_2)
                            .child(desc.to_string()),
                    )
                    .children(path.map(|p| {
                        div()
                            .text_size(theme.ui_px(11.5))
                            .text_color(theme.text_3)
                            .truncate()
                            .child(p.to_string())
                    })),
            )
            .children(control)
            .into_any_element()
    }

    /// Connection state: green dot + "Connected" / red dot + "Not running".
    pub(super) fn connection_status(&self, theme: Theme) -> AnyElement {
        div()
            .flex()
            .items_center()
            .gap_1p5()
            .child(
                div()
                    .size(px(7.))
                    .rounded_full()
                    .bg(if self.client.is_some() {
                        theme.ok_green
                    } else {
                        theme.stop_red
                    }),
            )
            .child(
                div()
                    .text_size(theme.ui_px(12.))
                    .text_color(theme.text_2)
                    .child(if self.client.is_some() {
                        "Connected"
                    } else {
                        "Not running"
                    }),
            )
            .into_any_element()
    }

    // ── Settings → Runtime ─────────────────────────────────────────────

    /// The Runtime section: the live pi process, its details, and
    /// start/stop/restart controls. Orbit has no socket server — the
    /// transport is stdio, so there is no host or port to report.
    pub(super) fn runtime_rows(
        &self,
        theme: Theme,
        this: Entity<OrbitApp>,
        _cx: &Context<Self>,
    ) -> Vec<AnyElement> {
        let state = self.runtime_state();
        let (state_label, state_color) = match state {
            RuntimeState::Running => ("Running", theme.ok_green),
            RuntimeState::Exited => ("Exited", theme.crit),
            RuntimeState::Stopped => ("Stopped", theme.text_3),
            RuntimeState::Failed => ("Failed to start", theme.crit),
        };
        let running = state == RuntimeState::Running;
        let has_client = self.client.is_some();

        let description = match state {
            RuntimeState::Running => {
                "Spawned as a child process — newline-delimited JSON over stdio."
            }
            RuntimeState::Exited => "The process exited on its own. Restart to reconnect.",
            RuntimeState::Stopped => "No pi process is running — start it to use the agent.",
            RuntimeState::Failed => "The last start failed. See the error below.",
        };

        let mut controls = div().flex().items_center().gap_2();
        if has_client {
            controls = controls
                .child(self.runtime_button(
                    "runtime-restart",
                    "Restart",
                    false,
                    theme,
                    this.clone(),
                    OrbitApp::runtime_restart,
                ))
                .child(self.runtime_button(
                    "runtime-stop",
                    "Stop",
                    false,
                    theme,
                    this.clone(),
                    OrbitApp::runtime_stop,
                ));
        } else {
            controls = controls.child(self.runtime_button(
                "runtime-start",
                "Start",
                true,
                theme,
                this.clone(),
                OrbitApp::runtime_start,
            ));
        }

        let mut rows = vec![self.card(
            theme,
            "Active process",
            description,
            Some(controls.into_any_element()),
        )];

        let pid = self
            .client
            .as_ref()
            .map(|client| client.child_pid().to_string())
            .unwrap_or_else(|| "—".to_string());
        let uptime = if running {
            self.runtime
                .started_at
                .map(|started| format_uptime(started.elapsed()))
                .unwrap_or_else(|| "—".to_string())
        } else {
            "—".to_string()
        };
        let workspace = self
            .current_workspace
            .clone()
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();

        let status = div()
            .flex()
            .items_center()
            .gap_1p5()
            .child(div().size(px(7.)).rounded_full().bg(state_color))
            .child(
                div()
                    .text_size(theme.ui_px(12.5))
                    .text_color(theme.text)
                    .child(state_label),
            )
            .into_any_element();

        let mut details = div()
            .bg(theme.bg_composer)
            .border_1()
            .border_color(theme.border)
            .rounded_lg()
            .px(px(14.))
            .py(px(12.))
            .flex()
            .flex_col()
            .gap(px(9.))
            .child(self.runtime_detail(theme, "Status", status))
            .child(self.runtime_detail(theme, "Process ID", runtime_text(theme, pid)))
            .child(self.runtime_detail(
                theme,
                "Binary",
                runtime_path(theme, orbit_rpc::pi_binary()),
            ))
            .child(self.runtime_detail(theme, "Uptime", runtime_text(theme, uptime)))
            .child(self.runtime_detail(
                theme,
                "Transport",
                runtime_text(
                    theme,
                    "stdio — newline-delimited JSON (no host or port)".to_string(),
                ),
            ))
            .child(self.runtime_detail(theme, "Workspace", runtime_path(theme, workspace)))
            .child(self.runtime_detail(
                theme,
                "Session store",
                runtime_path(
                    theme,
                    sessions::sessions_dir().to_string_lossy().into_owned(),
                ),
            ));
        if let Some(error) = &self.runtime.error {
            details = details.child(self.runtime_detail(
                theme,
                "Last error",
                runtime_error(theme, error.clone()),
            ));
        }
        rows.push(details.into_any_element());

        // Background sessions — each owns its own pi process.
        if !self.lives.is_empty() {
            let count = self.lives.len();
            let noun = if count == 1 { "session" } else { "sessions" };
            let mut card = div()
                .bg(theme.bg_composer)
                .border_1()
                .border_color(theme.border)
                .rounded_lg()
                .px(px(14.))
                .py(px(12.))
                .flex()
                .flex_col()
                .gap(px(9.))
                .child(
                    div()
                        .text_size(theme.ui_px(12.))
                        .text_color(theme.text_2)
                        .child(format!(
                            "{count} background {noun} running in their own pi processes"
                        )),
                );
            for (path, parked) in &self.lives {
                let name = path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_else(|| path.to_string_lossy().into_owned());
                card = card.child(self.runtime_detail(
                    theme,
                    &name,
                    runtime_text(
                        theme,
                        format!(
                            "pid {} · {}",
                            parked.client.child_pid(),
                            if parked.busy { "busy" } else { "idle" }
                        ),
                    ),
                ));
            }
            rows.push(card.into_any_element());
        }

        // Recent stderr — visible failures for "if any issue, show status".
        let stderr = self
            .client
            .as_ref()
            .map(|client| client.recent_stderr(8))
            .unwrap_or_default();
        let body: AnyElement = if stderr.is_empty() {
            div()
                .text_size(theme.ui_px(12.))
                .text_color(theme.text_3)
                .child("No output from the pi process.")
                .into_any_element()
        } else {
            let mut block = div().flex().flex_col().gap(px(2.));
            for line in &stderr {
                block = block.child(
                    div()
                        .min_w_0()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .font_family(theme::code_font_family())
                        .text_size(theme.code_px(11.5))
                        .text_color(theme.code_text)
                        .child(line.clone()),
                );
            }
            div()
                .bg(theme.code_bg)
                .border_1()
                .border_color(theme.border)
                .rounded_md()
                .px(px(10.))
                .py(px(8.))
                .child(block)
                .into_any_element()
        };
        rows.push(
            div()
                .bg(theme.bg_composer)
                .border_1()
                .border_color(theme.border)
                .rounded_lg()
                .px(px(14.))
                .py(px(12.))
                .flex()
                .flex_col()
                .gap(px(8.))
                .child(
                    div()
                        .text_size(theme.ui_px(13.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.text)
                        .child("Recent stderr"),
                )
                .child(body)
                .into_any_element(),
        );

        rows
    }

    // ── Settings → Agent ───────────────────────────────────────────────

    /// The Agent section: queue delivery modes, auto-compaction, auto-retry,
    /// manual compaction, and the session name. Every control sends a real pi
    /// RPC command.
    pub(super) fn agent_rows(
        &self,
        theme: Theme,
        this: Entity<OrbitApp>,
        _cx: &Context<Self>,
    ) -> Vec<AnyElement> {
        let name_control = div()
            .flex()
            .items_center()
            .gap_2()
            .child(
                div()
                    .w(px(200.))
                    .h(px(28.))
                    .px(px(8.))
                    .rounded_md()
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.bg_raised)
                    .flex()
                    .items_center()
                    .child(self.session_name_input.clone()),
            )
            .child(self.runtime_button(
                "session-rename-save",
                "Rename",
                false,
                theme,
                this.clone(),
                Self::rename_session,
            ))
            .into_any_element();

        let mut rows = vec![
            self.card(
                theme,
                "Follow-up messages",
                "Messages sent while the agent is running wait in the queue above the composer and are delivered once the current task finishes. All delivers the whole queue at once; One at a time delivers one per run.",
                Some(self.follow_up_mode_toggle(theme, this.clone())),
            ),
            self.card(
                theme,
                "Auto-compaction",
                "Compact conversation context automatically when it nears the model's window.",
                Some(self.settings_toggle(
                    "auto-compaction-toggle",
                    self.auto_compaction,
                    theme,
                    this.clone(),
                    Self::toggle_auto_compaction,
                )),
            ),
            self.card(
                theme,
                "Auto-retry",
                "Retry automatically on transient errors (overloaded, rate limit, 5xx). pi does not report this setting back, so the switch reflects the last value Orbit sent.",
                Some(self.settings_toggle(
                    "auto-retry-toggle",
                    self.auto_retry,
                    theme,
                    this.clone(),
                    Self::toggle_auto_retry,
                )),
            ),
            self.card(
                theme,
                "Compact now",
                if self.is_compacting {
                    "pi is compacting this session's context…"
                } else {
                    "Manually compact the conversation to free up context window."
                },
                Some(self.runtime_button(
                    "compact-now",
                    if self.is_compacting {
                        "Compacting…"
                    } else {
                        "Compact"
                    },
                    false,
                    theme,
                    this.clone(),
                    Self::compact_now,
                )),
            ),
            self.card(
                theme,
                "Session name",
                "The display name pi stores with this session; shown in session listings.",
                Some(name_control),
            ),
        ];

        if self.retrying {
            rows.insert(
                0,
                self.card(
                    theme,
                    "Retrying",
                    "pi is waiting out a transient provider error before retrying.",
                    Some(self.runtime_button(
                        "abort-retry",
                        "Abort retry",
                        false,
                        theme,
                        this.clone(),
                        Self::abort_retry,
                    )),
                ),
            );
        }

        if self.client.is_none() {
            rows.insert(
                0,
                self.card(
                    theme,
                    "pi is not connected",
                    "Start the runtime from Settings → Runtime to change agent behavior.",
                    None,
                ),
            );
        }
        rows
    }

    /// Two-button segmented control for the follow-up delivery mode.
    pub(super) fn follow_up_mode_toggle(&self, theme: Theme, this: Entity<OrbitApp>) -> AnyElement {
        let all = self.follow_up_mode == "all";
        let (one_id, all_id) = ("follow-up-mode-one", "follow-up-mode-all");
        let button =
            |label: &'static str, value_all: bool, id: &'static str, this: Entity<OrbitApp>| {
                let active = all == value_all;
                div()
                    .id(id)
                    .h(px(28.))
                    .px(px(10.))
                    .rounded_md()
                    .flex()
                    .items_center()
                    .text_size(theme.ui_px(12.))
                    .font_weight(FontWeight::MEDIUM)
                    .cursor_pointer()
                    .when(active, |b| b.bg(theme.send_bg).text_color(theme.send_fg))
                    .when(!active, |b| {
                        b.border_1()
                            .border_color(theme.border)
                            .bg(theme.bg_raised)
                            .text_color(theme.text_2)
                            .hover(|s| s.bg(theme.bg_hover))
                    })
                    .on_mouse_up(MouseButton::Left, move |_, _, cx| {
                        this.update(cx, |app, cx| app.set_follow_up_mode(value_all, cx));
                    })
                    .child(label)
            };
        div()
            .flex()
            .items_center()
            .gap_2()
            .child(button("One at a time", false, one_id, this.clone()))
            .child(button("All", true, all_id, this))
            .into_any_element()
    }

    /// Update the follow-up mode locally and push it to pi.
    pub(super) fn set_follow_up_mode(&mut self, all: bool, cx: &mut Context<Self>) {
        let mode = if all { "all" } else { "one-at-a-time" }.to_string();
        self.follow_up_mode = mode.clone();
        self.send(CommandBody::SetFollowUpMode { mode }, "set_follow_up_mode");
        cx.notify();
    }

    /// A real toggle switch (accent when on), parameterized by its action.
    pub(super) fn settings_toggle(
        &self,
        id: &'static str,
        on: bool,
        theme: Theme,
        this: Entity<OrbitApp>,
        action: fn(&mut OrbitApp, &mut Context<OrbitApp>),
    ) -> AnyElement {
        div()
            .id(id)
            .w(px(36.))
            .h(px(20.))
            .rounded_full()
            .p(px(2.))
            .border_1()
            .border_color(theme.border)
            .flex()
            .items_center()
            .cursor_pointer()
            .when(on, |t| t.bg(theme.accent).justify_end())
            .when(!on, |t| t.bg(theme.bg_raised).justify_start())
            .on_mouse_up(MouseButton::Left, move |_, _, cx| {
                this.update(cx, |app, cx| action(app, cx));
            })
            .child(div().size(px(14.)).rounded_full().bg(theme.text))
            .into_any_element()
    }

    pub(super) fn toggle_auto_compaction(&mut self, cx: &mut Context<Self>) {
        self.auto_compaction = !self.auto_compaction;
        self.send(
            CommandBody::SetAutoCompaction {
                enabled: self.auto_compaction,
            },
            "set_auto_compaction",
        );
        self.set_status(if self.auto_compaction {
            "Auto-compaction on"
        } else {
            "Auto-compaction off"
        });
        cx.notify();
    }

    pub(super) fn toggle_auto_retry(&mut self, cx: &mut Context<Self>) {
        self.auto_retry = !self.auto_retry;
        self.send(
            CommandBody::SetAutoRetry {
                enabled: self.auto_retry,
            },
            "set_auto_retry",
        );
        self.set_status(if self.auto_retry {
            "Auto-retry on"
        } else {
            "Auto-retry off"
        });
        cx.notify();
    }

    pub(super) fn compact_now(&mut self, cx: &mut Context<Self>) {
        if self.is_compacting {
            return;
        }
        self.is_compacting = true;
        // The run-status strip shows the in-progress state.
        if !self.send(
            CommandBody::Compact {
                custom_instructions: None,
            },
            "compact",
        ) {
            self.is_compacting = false;
        }
        cx.notify();
    }

    pub(super) fn abort_retry(&mut self, cx: &mut Context<Self>) {
        self.send(CommandBody::AbortRetry, "abort_retry");
        self.retrying = false;
        self.retry_detail = None;
        self.set_status("Retry aborted");
        cx.notify();
    }

    pub(super) fn rename_session(&mut self, cx: &mut Context<Self>) {
        let name = self.session_name_input.read(cx).text().trim().to_string();
        if name.is_empty() {
            self.set_status("Enter a session name first");
            cx.notify();
            return;
        }
        self.session_name = Some(name.clone());
        self.send(CommandBody::SetSessionName { name }, "set_session_name");
        self.set_status("Session renamed");
        cx.notify();
    }

    /// One label/value row inside a Runtime card.
    pub(super) fn runtime_detail(
        &self,
        theme: Theme,
        label: &str,
        value: AnyElement,
    ) -> AnyElement {
        div()
            .flex()
            .items_center()
            .gap_3()
            .child(
                div()
                    .w(px(110.))
                    .flex_none()
                    .min_w_0()
                    .truncate()
                    .text_size(theme.ui_px(12.))
                    .text_color(theme.text_3)
                    .child(label.to_string()),
            )
            .child(div().flex_1().min_w_0().child(value))
            .into_any_element()
    }

    /// A Runtime action button (Start / Stop / Restart).
    pub(super) fn runtime_button(
        &self,
        id: &'static str,
        label: &str,
        primary: bool,
        theme: Theme,
        this: Entity<OrbitApp>,
        action: fn(&mut OrbitApp, &mut Context<OrbitApp>),
    ) -> AnyElement {
        let mut button = div()
            .id(id)
            .h(px(28.))
            .px(px(12.))
            .rounded_md()
            .flex()
            .items_center()
            .justify_center()
            .gap(px(6.))
            .cursor_pointer()
            .text_size(theme.ui_px(12.))
            .font_weight(FontWeight::MEDIUM);
        if primary {
            button = button
                .bg(theme.send_bg)
                .text_color(theme.send_fg)
                .hover(|s| s.bg(theme.send_bg_hover));
        } else {
            button = button
                .border_1()
                .border_color(theme.border)
                .bg(theme.bg_raised)
                .text_color(theme.text_2)
                .hover(|s| s.bg(theme.bg_hover));
        }
        button
            .on_mouse_up(MouseButton::Left, move |_, _, cx| {
                this.update(cx, |app, cx| action(app, cx));
            })
            .child(label.to_string())
            .into_any_element()
    }

    /// Appearance → "Background image": pick an image for the dithered
    /// page backdrop, or reset to the dot grid.
    pub(super) fn background_card(
        &self,
        theme: Theme,
        this: Entity<OrbitApp>,
        _cx: &Context<Self>,
    ) -> AnyElement {
        let label = crate::dither::configured_label();
        let mut controls = div()
            .flex()
            .items_center()
            .gap_2()
            .child(self.runtime_button(
                "background-choose",
                if label.is_some() {
                    "Replace…"
                } else {
                    "Choose image…"
                },
                false,
                theme,
                this.clone(),
                OrbitApp::background_choose,
            ));
        if label.is_some() {
            controls = controls.child(self.runtime_button(
                "background-reset",
                "Reset",
                false,
                theme,
                this,
                OrbitApp::background_reset,
            ));
        }
        self.card_with_path(
            theme,
            "Background image",
            "A dithered image behind the new-task and chat pages.",
            label.as_deref(),
            Some(controls.into_any_element()),
        )
    }

    /// Pick the background image (native dialog), copy + process it, and
    /// warm the dither cache so the first paint of the new-task page is
    /// costless.
    pub(super) fn background_choose(&mut self, cx: &mut Context<Self>) {
        let picked = rfd::FileDialog::new()
            .set_title("Choose a background image")
            .add_filter(
                "Images",
                &["png", "jpg", "jpeg", "webp", "gif", "bmp", "tiff"],
            )
            .pick_file();
        let Some(path) = picked else {
            return;
        };
        match crate::dither::choose_file(&path) {
            Ok(label) => {
                crate::dither::background();
                self.set_status(format!("Background set to {label}"));
            }
            Err(err) => self.set_status(err),
        }
        cx.notify();
    }

    /// Drop the background (the dot grid returns) and forget the cache.
    pub(super) fn background_reset(&mut self, cx: &mut Context<Self>) {
        crate::dither::clear_all();
        crate::dither::background();
        self.set_status("Background reset");
        cx.notify();
    }

    /// The real sidebar toggle, wired to the same state as the top bar.
    pub(super) fn sidebar_toggle(&self, theme: Theme, this: Entity<OrbitApp>) -> AnyElement {
        let on = self.sidebar_visible;
        div()
            .id("settings-sidebar-toggle")
            .w(px(36.))
            .h(px(20.))
            .rounded_full()
            .p(px(2.))
            .border_1()
            .border_color(theme.border)
            .flex()
            .items_center()
            .cursor_pointer()
            .when(on, |t| t.bg(theme.accent).justify_end())
            .when(!on, |t| t.bg(theme.bg_raised).justify_start())
            .on_mouse_up(MouseButton::Left, move |_, _, cx| {
                this.update(cx, |app, cx| {
                    app.sidebar_visible = !app.sidebar_visible;
                    cx.notify();
                });
            })
            .child(div().size(px(14.)).rounded_full().bg(theme.text))
            .into_any_element()
    }

    /// Theme dropdown on Appearance — lists every selectable palette.
    pub(super) fn theme_select(
        &self,
        theme: Theme,
        this: Entity<OrbitApp>,
        cx: &Context<Self>,
    ) -> AnyElement {
        let all = ThemeId::ALL;
        let selected = all.iter().position(|id| *id == theme.theme_id).unwrap_or(0);
        self.select_control(
            "theme-select",
            SettingsSelect::Theme,
            theme.theme_id.label().to_string(),
            all.iter().map(|id| id.label().to_string()).collect(),
            selected,
            theme,
            this,
            cx,
        )
    }

    // ── Waku General-settings selects (language / font sizes) ──────────

    /// The Language dropdown (System / English).
    pub(super) fn language_select(
        &self,
        theme: Theme,
        this: Entity<OrbitApp>,
        cx: &Context<Self>,
    ) -> AnyElement {
        let selected = match theme.ui.language {
            crate::theme::Language::System => 0,
            crate::theme::Language::English => 1,
        };
        self.select_control(
            "language-select",
            SettingsSelect::Language,
            theme.ui.language.label().to_string(),
            vec!["System".to_string(), "English".to_string()],
            selected,
            theme,
            this,
            cx,
        )
    }

    // ── Appearance → Density & type ────────────────────────────────────

    /// Waku's grouped "Density & type" card: typefaces on the first row,
    /// their sizes / densities beneath, laid out two-up so the whole
    /// surface reads as one instrument cluster rather than six cards.
    pub(super) fn density_type_card(
        &self,
        theme: Theme,
        this: Entity<OrbitApp>,
        cx: &Context<Self>,
    ) -> AnyElement {
        let field = |label: &'static str, control: AnyElement| {
            div()
                .flex()
                .flex_col()
                .gap(theme.space(6.))
                .min_w_0()
                .child(
                    div()
                        .text_size(theme.ui_px(11.5))
                        .text_color(theme.text_2)
                        .child(label),
                )
                .child(control)
                .into_any_element()
        };
        let pair = |left: AnyElement, right: AnyElement| {
            div()
                .flex()
                .gap(theme.space(16.))
                .child(div().flex_1().min_w_0().child(left))
                .child(div().flex_1().min_w_0().child(right))
        };
        div()
            .bg(theme.bg_composer)
            .border_1()
            .border_color(theme.border)
            .rounded_lg()
            .p(theme.space(14.))
            .flex()
            .flex_col()
            .gap(theme.space(14.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_size(theme.ui_px(13.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.text)
                            .child("Density & type"),
                    )
                    .child(
                        div()
                            .text_size(theme.ui_px(12.))
                            .text_color(theme.text_2)
                            .child("Typefaces, sizes, and how much air the workbench keeps."),
                    ),
            )
            .child(pair(
                field(
                    "Interface Font",
                    self.font_family_select(SettingsSelect::UiFontFamily, theme, this.clone(), cx),
                ),
                field(
                    "Code Font",
                    self.font_family_select(
                        SettingsSelect::CodeFontFamily,
                        theme,
                        this.clone(),
                        cx,
                    ),
                ),
            ))
            .child(pair(
                field(
                    "Interface Font Size",
                    self.preset_select(SettingsSelect::InterfaceScale, theme, this.clone(), cx),
                ),
                field(
                    "Terminal Font Size",
                    self.preset_select(SettingsSelect::TerminalFont, theme, this.clone(), cx),
                ),
            ))
            .child(pair(
                field(
                    "Editor Font Size",
                    self.preset_select(SettingsSelect::EditorFont, theme, this.clone(), cx),
                ),
                field(
                    "Spacing Density",
                    self.preset_select(SettingsSelect::SpacingDensity, theme, this.clone(), cx),
                ),
            ))
            .into_any_element()
    }

    /// The percentage / px preset dropdowns in the Density & type card.
    pub(super) fn preset_select(
        &self,
        kind: SettingsSelect,
        theme: Theme,
        this: Entity<OrbitApp>,
        cx: &Context<Self>,
    ) -> AnyElement {
        use crate::theme::{FONT_SIZES, INTERFACE_SCALES, SPACING_DENSITIES};
        let (values, current, suffix): (Vec<f32>, f32, &str) = match kind {
            SettingsSelect::InterfaceScale => (
                INTERFACE_SCALES.iter().map(|v| *v as f32).collect(),
                theme.ui.interface_scale as f32,
                "%",
            ),
            SettingsSelect::TerminalFont => {
                (FONT_SIZES.to_vec(), theme.ui.terminal_font_size, "px")
            }
            SettingsSelect::EditorFont => (FONT_SIZES.to_vec(), theme.ui.editor_font_size, "px"),
            SettingsSelect::SpacingDensity => (
                SPACING_DENSITIES.iter().map(|v| *v as f32).collect(),
                theme.ui.spacing_density as f32,
                "%",
            ),
            SettingsSelect::Language
            | SettingsSelect::Theme
            | SettingsSelect::UiFontFamily
            | SettingsSelect::CodeFontFamily => unreachable!(),
        };
        let selected = values
            .iter()
            .position(|v| (*v - current).abs() < 0.01)
            .unwrap_or(0);
        self.select_control(
            match kind {
                SettingsSelect::InterfaceScale => "interface-scale-select",
                SettingsSelect::TerminalFont => "terminal-font-select",
                SettingsSelect::EditorFont => "editor-font-select",
                SettingsSelect::SpacingDensity => "spacing-density-select",
                _ => "settings-select",
            },
            kind,
            format!("{} {}", current as u32, suffix),
            values
                .iter()
                .map(|v| format!("{} {}", *v as u32, suffix))
                .collect(),
            selected,
            theme,
            this,
            cx,
        )
    }

    /// The Interface / Code font-family dropdowns — Orbit's curated catalog.
    pub(super) fn font_family_select(
        &self,
        kind: SettingsSelect,
        theme: Theme,
        this: Entity<OrbitApp>,
        cx: &Context<Self>,
    ) -> AnyElement {
        use crate::theme::{FontChoice, CODE_FONTS, UI_FONTS};
        let prefs = theme::font_prefs();
        let (id, current, choices): (&'static str, SharedString, &[FontChoice]) = match kind {
            SettingsSelect::UiFontFamily => (
                "ui-font-family-select",
                prefs.ui_font_family.clone(),
                &UI_FONTS,
            ),
            SettingsSelect::CodeFontFamily => (
                "code-font-family-select",
                prefs.code_font_family.clone(),
                &CODE_FONTS,
            ),
            _ => unreachable!(),
        };
        let selected = choices
            .iter()
            .position(|f| f.family == current.as_ref())
            .unwrap_or(0);
        self.select_control(
            id,
            kind,
            theme::font_choice_label(current.as_ref()),
            choices.iter().map(|f| f.label.to_string()).collect(),
            selected,
            theme,
            this,
            cx,
        )
    }

    /// A Waku-style select: value chip + caret, dropdown above when open.
    pub(super) fn select_control(
        &self,
        id: &'static str,
        kind: SettingsSelect,
        label: String,
        options: Vec<String>,
        selected: usize,
        theme: Theme,
        this: Entity<OrbitApp>,
        cx: &Context<Self>,
    ) -> AnyElement {
        let open = self.settings_select == Some(kind);
        let chip_this = this.clone();
        div()
            .flex()
            .flex_col()
            .items_end()
            // dropdown anchored above the chip when open
            .children(self.settings_select_popup(kind, options, selected, theme, &this, cx))
            .child(
                div()
                    .id(ElementId::Name(id.into()))
                    .h(px(26.))
                    .px(px(10.))
                    .rounded(px(7.))
                    .border_1()
                    .border_color(if open {
                        theme.border_strong
                    } else {
                        theme.border
                    })
                    .bg(if open { theme.overlay } else { theme.bg_raised })
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .cursor_pointer()
                    .text_size(theme.ui_px(12.))
                    .text_color(theme.text_2)
                    .hover(|s| s.bg(theme.overlay))
                    .on_mouse_up(MouseButton::Left, move |_, window, cx| {
                        chip_this.update(cx, |app, cx| {
                            if app.settings_select == Some(kind) {
                                app.settings_select = None;
                            } else {
                                app.settings_select = Some(kind);
                                app.settings_filter
                                    .update(cx, |filter, cx| filter.clear(cx));
                                let handle = app.settings_filter.read(cx).focus_handle(cx);
                                window.focus(&handle);
                            }
                            cx.notify();
                        });
                    })
                    .child(label)
                    .child(icon("icons/chevron-down.svg", 10., theme.text_3)),
            )
            .into_any_element()
    }

    /// The open dropdown's option list, anchored above its chip.
    pub(super) fn settings_select_popup(
        &self,
        kind: SettingsSelect,
        options: Vec<String>,
        selected: usize,
        theme: Theme,
        this: &Entity<OrbitApp>,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        if self.settings_select != Some(kind) {
            return None;
        }
        let needle = self.settings_filter.read(cx).text().to_lowercase();
        // Filter options, keeping the original index for click dispatch.
        let rows: Vec<(usize, String)> = options
            .iter()
            .enumerate()
            .filter(|(_, option)| needle.is_empty() || option.to_lowercase().contains(&needle))
            .map(|(ix, option)| (ix, option.clone()))
            .collect();
        let empty = rows.is_empty();
        // Virtualized list — only the visible rows are laid out and painted.
        // The font-family dropdowns can have a thousand+ entries, and the
        // whole app re-renders on every scroll tick, so rendering every row
        // per frame is what made the theme/font selectors lag while scrolling.
        //
        // `uniform_list`'s Infer sizing reads the *available* height, and this
        // popup lives inside a 0×0 anchor div, so the available height is 0 and
        // the list collapses to nothing. Give it an explicit height computed
        // from the row count instead (30 px stride + 8 px vertical padding,
        // capped at the old 220 px max) — same visual, real viewport.
        let list_h = (rows.len() as f32 * 30. + 8.).min(220.);
        let list: AnyElement = if empty {
            div()
                .w_full()
                .px(px(4.))
                .py(px(4.))
                .child(
                    div()
                        .h(px(28.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(theme.ui_px(12.))
                        .text_color(theme.text_3)
                        .child("No matches"),
                )
                .into_any_element()
        } else {
            let this = this.clone();
            uniform_list(
                "settings-select-list",
                rows.len(),
                move |range, _window, _cx| {
                    let mut children = Vec::new();
                    for ix in range {
                        let (orig_ix, option) = &rows[ix];
                        let orig_ix = *orig_ix;
                        let selected_row = orig_ix == selected;
                        let this = this.clone();
                        // 30 px stride = 28 px row + the 2 px gap the old
                        // flex list had between rows (uniform_list has no
                        // gap support, so the gap rides on the row shell).
                        children.push(
                            div().h(px(30.)).w_full().child(
                                div()
                                    .id(ElementId::NamedInteger(
                                        "settings-select-row".into(),
                                        orig_ix as u64,
                                    ))
                                    .h(px(28.))
                                    .w_full()
                                    .px(px(10.))
                                    .rounded(px(6.))
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .gap(px(10.))
                                    .cursor_pointer()
                                    .when(selected_row, |row| row.bg(theme.active))
                                    .hover(|style| style.bg(theme.overlay))
                                    .text_size(theme.ui_px(12.))
                                    .text_color(if selected_row {
                                        theme.active_fg
                                    } else {
                                        theme.text_2
                                    })
                                    .child(option.clone())
                                    .when(selected_row, |row| {
                                        row.child(icon("icons/check.svg", 11., theme.accent))
                                    })
                                    .on_mouse_up(MouseButton::Left, move |_, _, cx| {
                                        this.update(cx, |app, cx| {
                                            app.apply_settings_select(kind, orig_ix, cx);
                                            app.settings_select = None;
                                            cx.notify();
                                        });
                                    }),
                            ),
                        );
                    }
                    children
                },
            )
            .w_full()
            .h(px(list_h))
            .px(px(4.))
            .py(px(4.))
            .into_any_element()
        };
        let popup = div()
            .min_w(px(360.))
            .rounded(px(8.))
            .border_1()
            .border_color(theme.border_strong)
            .bg(theme.menu_bg)
            .shadow(theme.popover_shadow())
            .flex()
            .flex_col()
            .overflow_hidden()
            .occlude()
            .on_mouse_down_out({
                let this = this.clone();
                move |_: &MouseDownEvent, _, cx: &mut App| {
                    this.update(cx, |app, cx| {
                        if app.settings_select.take().is_some() {
                            cx.notify();
                        }
                    });
                }
            })
            // Search field — filters the options below.
            .child(
                div()
                    .h(px(34.))
                    .px(px(10.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .border_b_1()
                    .border_color(theme.border)
                    .text_size(theme.ui_px(12.))
                    .child(icon("icons/search.svg", 13., theme.text_3))
                    .child(self.settings_filter.clone()),
            )
            .child(list);
        // Anchor to the chip's top-right corner (via a zero-size point), so the
        // popup opens above the chip, right-aligned — like the open-in menu.
        Some(
            div()
                .absolute()
                .top_0()
                .right_0()
                .size(px(0.))
                .child(
                    anchored()
                        .position_mode(AnchoredPositionMode::Local)
                        .anchor(Corner::BottomRight)
                        .offset(point(px(0.), px(-4.)))
                        .snap_to_window()
                        .child(deferred(popup)),
                )
                .into_any_element(),
        )
    }

    /// Apply a dropdown choice to the persisted UI customization.
    pub(super) fn apply_settings_select(
        &mut self,
        kind: SettingsSelect,
        ix: usize,
        cx: &mut Context<Self>,
    ) {
        match kind {
            SettingsSelect::Theme => {
                let id = ThemeId::ALL.get(ix).copied().unwrap_or(ThemeId::Orbit);
                theme::set_theme(cx, id);
                return;
            }
            SettingsSelect::UiFontFamily | SettingsSelect::CodeFontFamily => {
                use crate::theme::{CODE_FONTS, UI_FONTS};
                let choices = match kind {
                    SettingsSelect::UiFontFamily => &UI_FONTS[..],
                    SettingsSelect::CodeFontFamily => &CODE_FONTS[..],
                    _ => unreachable!(),
                };
                let Some(choice) = choices.get(ix) else {
                    return;
                };
                let mut prefs = theme::font_prefs();
                match kind {
                    SettingsSelect::UiFontFamily => prefs.ui_font_family = choice.family.into(),
                    SettingsSelect::CodeFontFamily => prefs.code_font_family = choice.family.into(),
                    _ => unreachable!(),
                }
                theme::set_font_prefs(prefs);
                return;
            }
            _ => {}
        }
        use crate::theme::{Language, FONT_SIZES, INTERFACE_SCALES, SPACING_DENSITIES};
        let mut ui = theme::get(cx).ui;
        match kind {
            SettingsSelect::Language => {
                ui.language = if ix == 1 {
                    Language::English
                } else {
                    Language::System
                };
            }
            SettingsSelect::InterfaceScale => {
                ui.interface_scale = INTERFACE_SCALES.get(ix).copied().unwrap_or(100);
            }
            SettingsSelect::TerminalFont => {
                ui.terminal_font_size = FONT_SIZES.get(ix).copied().unwrap_or(13.);
            }
            SettingsSelect::EditorFont => {
                ui.editor_font_size = FONT_SIZES.get(ix).copied().unwrap_or(13.);
            }
            SettingsSelect::SpacingDensity => {
                ui.spacing_density = SPACING_DENSITIES.get(ix).copied().unwrap_or(100);
            }
            SettingsSelect::Theme
            | SettingsSelect::UiFontFamily
            | SettingsSelect::CodeFontFamily => unreachable!(),
        }
        theme::set_ui_prefs(cx, ui);
    }
}

// ── controller ────────────────────────────────────────────────────
impl OrbitApp {
    pub(super) fn open_settings(&mut self, cx: &mut Context<Self>) {
        self.settings_open = true;
        self.set_settings_section(SettingsSection::General, cx);
    }

    pub(super) fn on_open_settings(
        &mut self,
        _: &crate::OpenSettings,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_settings(cx);
    }

    pub(super) fn on_settings_gear_click(
        &mut self,
        _: &MouseUpEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Toggle: the gear sits in the sessions sidebar, which stays visible
        // while settings is open, so clicking it again should go back.
        if self.settings_open {
            self.settings_open = false;
            self.provider_editor = None;
            self.provider_key_editor = None;
        } else {
            self.open_settings(cx);
            return;
        }
        cx.notify();
    }

    pub(super) fn on_settings_back(
        &mut self,
        _: &MouseUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.settings_open = false;
        self.provider_editor = None;
        self.provider_key_editor = None;
        self.input.read(cx).focus(window);
        cx.notify();
    }

    /// Switch sections, loading models.json when Providers is shown so CLI
    /// edits appear without a restart.
    pub(super) fn set_settings_section(
        &mut self,
        section: SettingsSection,
        cx: &mut Context<Self>,
    ) {
        self.settings_section = section;
        self.provider_remove_confirm = None;
        self.provider_editor = None;
        self.provider_key_editor = None;
        if section == SettingsSection::Providers {
            self.reload_custom_providers(cx);
            self.refresh_auth();
        }
        cx.notify();
    }

    /// Re-read `~/.pi/agent/models.json` and `~/.pi/agent/auth.json`. Read
    /// errors are kept, not fatal, so a broken file can be seen and fixed
    /// rather than overwritten.
    pub(super) fn reload_custom_providers(&mut self, cx: &mut Context<Self>) {
        match providers::read_custom() {
            Ok(list) => {
                self.custom_providers = list;
                self.custom_providers_error = None;
            }
            Err(err) => {
                self.custom_providers = Vec::new();
                self.custom_providers_error = Some(err);
            }
        }
        match providers::read_auth() {
            Ok(auth) => {
                self.provider_auth = auth;
                self.provider_auth_error = None;
            }
            Err(err) => {
                self.provider_auth = HashMap::new();
                self.provider_auth_error = Some(err);
            }
        }
        self.ensure_provider_metadata(cx);
        cx.notify();
    }

    /// Load pi's built-in catalog sizes and authoritative provider metadata
    /// once, off the UI thread (node introspection + ~600 KB of JSON parsing
    /// would otherwise hitch the first Providers render).
    pub(super) fn ensure_provider_metadata(&mut self, cx: &mut Context<Self>) {
        if self.provider_metadata_loaded {
            return;
        }
        self.provider_metadata_loaded = true;
        cx.spawn(async move |this, cx| {
            let (counts, metadata) = cx
                .background_executor()
                .spawn(async {
                    let counts = providers::builtin_catalog_counts().clone();
                    let metadata = providers::dynamic_providers()
                        .map(<[providers::DynamicProvider]>::to_vec)
                        .unwrap_or_default();
                    (counts, metadata)
                })
                .await;
            let _ = this.update(cx, |app, cx| {
                app.provider_catalog_counts = counts;
                app.provider_metadata = metadata;
                cx.notify();
            });
        })
        .detach();
    }

    /// Refresh: re-query the running agent's catalog and re-read both files.
    pub(super) fn provider_refresh(&mut self, cx: &mut Context<Self>) {
        self.providers_refreshing = true;
        self.refresh_catalogs();
        self.reload_custom_providers(cx);
        self.refresh_auth();
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(700))
                .await;
            let _ = this.update(cx, |app, cx| {
                app.providers_refreshing = false;
                cx.notify();
            });
        })
        .detach();
        self.set_status("Refreshed provider catalog");
        cx.notify();
    }

    /// Restart pi so a newly written credential is loaded. pi reads
    /// `auth.json` only at startup, so this is the "use it" step. Resumes the
    /// active session on the fresh process when one exists.
    pub(super) fn provider_apply_credentials(&mut self, cx: &mut Context<Self>) {
        // Never yank the process out from under a live turn.
        if self.busy || self.transcript.is_streaming() {
            self.set_status("Finish the current turn, then Restart pi to load credentials");
            cx.notify();
            return;
        }
        let resume = self
            .current_session_path
            .clone()
            .and_then(|path| self.sessions.iter().find(|s| s.path == path).cloned());
        self.drop_client();
        self.current_session_path = None;
        if let Some(session) = resume {
            self.switch_to_session(session, false, cx);
        } else {
            self.runtime_start(cx);
        }
        self.provider_auth_dirty = false;
        self.reload_custom_providers(cx);
        self.set_status("pi restarted — credentials loaded");
        cx.notify();
    }

    /// Open the API-key editor for a provider.
    pub(super) fn provider_key_open(
        &mut self,
        provider_id: String,
        provider_name: String,
        oauth: bool,
        note: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.provider_credential_open(
            provider_id,
            provider_name,
            oauth,
            note,
            ProviderKeyKind::ApiKey,
            window,
            cx,
        );
    }

    /// Open the credential editor for `kind` (API key or an Ollama Cloud
    /// session). One modal handles both; the field, hint, and save path differ.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn provider_credential_open(
        &mut self,
        provider_id: String,
        provider_name: String,
        oauth: bool,
        note: &'static str,
        kind: ProviderKeyKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let placeholder = match kind {
            ProviderKeyKind::ApiKey => "sk-…",
            ProviderKeyKind::OllamaCloudSession => "__Secure-session=…",
        };
        let key = cx.new(|cx| {
            ComposerInput::new(cx)
                .with_element_id("provider-key-input")
                .with_key_context("Composer Picker")
                .with_max_lines(1)
                .with_placeholder(placeholder)
        });
        let focus = key.read(cx).focus_handle(cx);
        window.focus(&focus);
        self.provider_key_editor = Some(ProviderKeyEditor {
            provider_id,
            provider_name,
            oauth,
            note,
            kind,
            key,
            error: None,
        });
        cx.notify();
    }

    /// Save the API key, then flag that pi needs a restart to load it.
    pub(super) fn provider_key_save(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(editor) = self.provider_key_editor.as_ref() else {
            return;
        };
        let id = editor.provider_id.clone();
        let name = editor.provider_name.clone();
        let kind = editor.kind;
        let key = editor.key.read(cx).text();
        let result = match kind {
            ProviderKeyKind::ApiKey => providers::write_api_key(&id, &key),
            ProviderKeyKind::OllamaCloudSession => providers::write_ollama_cloud_session(&key),
        };
        match result {
            Ok(()) => {
                self.provider_key_editor = None;
                self.provider_auth_dirty = true;
                self.reload_custom_providers(cx);
                let what = match kind {
                    ProviderKeyKind::ApiKey => "API key",
                    ProviderKeyKind::OllamaCloudSession => "Ollama Cloud session",
                };
                self.set_status(format!("{what} saved for {name} — Restart pi to use it"));
            }
            Err(err) => {
                if let Some(editor) = self.provider_key_editor.as_mut() {
                    editor.error = Some(err);
                }
            }
        }
        cx.notify();
    }

    /// Sign in with OAuth by handing `pi /login <id>` to the user's terminal.
    pub(super) fn provider_oauth_login(
        &mut self,
        id: String,
        name: String,
        cx: &mut Context<Self>,
    ) {
        // The id reaches a shell script; only builtin ids are ever passed, but
        // validate anyway so a hand-edited file can never inject a command.
        if !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        {
            self.set_status(format!(
                "Refusing to run login for invalid provider id {id}"
            ));
            cx.notify();
            return;
        }
        let command = format!("pi /login {id}");
        match platform::open_terminal_command(&command) {
            Ok(()) => {
                self.provider_auth_dirty = true;
                self.set_status(format!(
                    "Finish signing in to {name} in Terminal, then Restart pi"
                ));
            }
            Err(err) => {
                self.set_status(format!("Could not open Terminal: {err} — run `{command}`"));
            }
        }
        cx.notify();
    }

    /// Sign out. With auth RPC available pi owns the credential store and
    /// removes it live; otherwise fall back to dropping the auth.json entry
    /// (env credentials are outside Orbit's reach and left alone).
    pub(super) fn provider_sign_out(&mut self, id: String, cx: &mut Context<Self>) {
        if self.auth.support() == AuthSupport::Supported {
            self.auth.on_logout_response(true, &id);
            self.send(
                CommandBody::AuthLogout {
                    provider: id.clone(),
                },
                "auth.logout",
            );
            self.set_status(format!("Signing out of {id}…"));
            cx.notify();
            return;
        }
        match providers::remove_auth(&id) {
            Ok(()) => {
                self.provider_auth_dirty = true;
                self.reload_custom_providers(cx);
                self.set_status(format!("Signed out of {id} — Restart pi to apply"));
            }
            Err(err) => {
                self.provider_auth_error = Some(err);
            }
        }
        cx.notify();
    }

    /// Open the editor for an existing provider id, or a blank add form.
    pub(super) fn provider_editor_open(
        &mut self,
        provider_id: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let existing = provider_id.as_ref().and_then(|id| {
            self.custom_providers
                .iter()
                .find(|provider| &provider.id == id)
        });
        let in_catalog = provider_id.as_ref().is_some_and(|id| {
            self.available_models
                .iter()
                .any(|model| &model.provider == id)
        });
        let id_text = provider_id.clone().unwrap_or_default();
        let name_text = existing
            .and_then(|provider| provider.name.clone())
            .unwrap_or_default();
        let base_url_text = existing
            .map(|provider| provider.base_url.clone())
            .unwrap_or_default();
        let api = existing
            .map(|provider| provider.api.clone())
            .filter(|api| !api.is_empty())
            .unwrap_or_else(|| {
                if in_catalog {
                    // Built-in: no override unless the user picks one.
                    String::new()
                } else {
                    "openai-completions".to_string()
                }
            });
        let models_text = existing
            .map(|provider| provider.model_ids.join(", "))
            .unwrap_or_default();
        let had_api_key = existing.is_some_and(|provider| provider.has_api_key);

        let id_input = cx.new(|cx| {
            ComposerInput::new(cx)
                .with_element_id("provider-editor-id")
                .with_key_context("Composer Picker")
                .with_max_lines(1)
                .with_placeholder("my-gateway")
                .with_text(id_text)
        });
        let name_input = cx.new(|cx| {
            ComposerInput::new(cx)
                .with_element_id("provider-editor-name")
                .with_key_context("Composer Picker")
                .with_max_lines(1)
                .with_placeholder("Optional display name")
                .with_text(name_text)
        });
        let base_url_input = cx.new(|cx| {
            ComposerInput::new(cx)
                .with_element_id("provider-editor-base-url")
                .with_key_context("Composer Picker")
                .with_max_lines(1)
                .with_placeholder("https://api.example.com/v1")
                .with_text(base_url_text)
        });
        let api_key_input = cx.new(|cx| {
            ComposerInput::new(cx)
                .with_element_id("provider-editor-api-key")
                .with_key_context("Composer Picker")
                .with_max_lines(1)
                .with_placeholder(if had_api_key {
                    "••••••••"
                } else {
                    "sk-…"
                })
        });
        let models_input = cx.new(|cx| {
            ComposerInput::new(cx)
                .with_element_id("provider-editor-models")
                .with_key_context("Composer Picker")
                .with_max_lines(4)
                .with_placeholder("model-id, model-id-2")
                .with_text(models_text)
        });

        let focus = if provider_id.is_some() {
            name_input.read(cx).focus_handle(cx)
        } else {
            id_input.read(cx).focus_handle(cx)
        };
        window.focus(&focus);
        self.provider_editor = Some(ProviderEditor {
            original_id: provider_id,
            in_catalog,
            id: id_input,
            name: name_input,
            base_url: base_url_input,
            api_key: api_key_input,
            models: models_input,
            api,
            had_api_key,
            error: None,
        });
        cx.notify();
    }

    pub(super) fn provider_editor_cancel(
        &mut self,
        _: &crate::PickerCancel,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let closed =
            self.provider_editor.take().is_some() | self.provider_key_editor.take().is_some();
        if closed {
            cx.notify();
        }
    }

    pub(super) fn provider_editor_confirm(
        &mut self,
        _: &crate::PickerConfirm,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.provider_key_editor.is_some() {
            self.provider_key_save(window, cx);
        } else {
            self.provider_save(window, cx);
        }
    }

    /// Dismiss when the scrim (outside the card) is pressed.
    pub(super) fn provider_editor_scrim(
        &mut self,
        _: &MouseDownEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let closed =
            self.provider_editor.take().is_some() | self.provider_key_editor.take().is_some();
        if closed {
            cx.notify();
        }
    }

    /// Validate and write the editor's values to models.json.
    pub(super) fn provider_save(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(editor) = self.provider_editor.as_ref() else {
            return;
        };
        let id = editor.id.read(cx).text().trim().to_string();
        let name = editor.name.read(cx).text().trim().to_string();
        let base_url = editor.base_url.read(cx).text().trim().to_string();
        let api_key = editor.api_key.read(cx).text();
        let api = editor.api.clone();
        let in_catalog = editor.in_catalog;
        let models: Vec<String> = editor
            .models
            .read(cx)
            .text()
            .split(['\n', ','])
            .map(|model| model.trim().to_string())
            .filter(|model| !model.is_empty())
            .collect();

        let error = if id.is_empty() {
            Some("Provider id is required.".to_string())
        } else if !id
            .chars()
            .next()
            .is_some_and(|first| first.is_ascii_alphanumeric())
            || !id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        {
            Some(
                "Id must start with a letter or number — letters, numbers, dots, dashes and underscores only."
                    .to_string(),
            )
        } else if !(base_url.is_empty()
            || base_url.starts_with("http://")
            || base_url.starts_with("https://"))
        {
            Some("Base URL must start with http:// or https://.".to_string())
        } else if base_url.is_empty() && !in_catalog {
            Some("Base URL is required for a custom provider.".to_string())
        } else if models.is_empty() && !in_catalog {
            Some("Add at least one model id.".to_string())
        } else {
            None
        };

        if let Some(error) = error {
            if let Some(editor) = self.provider_editor.as_mut() {
                editor.error = Some(error);
            }
            cx.notify();
            return;
        }

        let key = (!api_key.trim().is_empty()).then_some(api_key.trim());
        match providers::write_provider(
            &id,
            (!name.is_empty()).then_some(name.as_str()),
            &base_url,
            &api,
            key,
            &models,
        ) {
            Ok(()) => {
                self.provider_editor = None;
                self.reload_custom_providers(cx);
                self.refresh_catalogs();
                self.set_status(format!(
                    "Saved {id} — restart pi if it doesn't appear in the catalog"
                ));
            }
            Err(err) => {
                if let Some(editor) = self.provider_editor.as_mut() {
                    editor.error = Some(err);
                }
            }
        }
        cx.notify();
    }

    /// Remove a provider entry from models.json.
    pub(super) fn provider_remove(&mut self, id: String, cx: &mut Context<Self>) {
        match providers::remove_provider(&id) {
            Ok(()) => {
                self.provider_remove_confirm = None;
                self.reload_custom_providers(cx);
                self.refresh_catalogs();
                self.set_status(format!("Removed provider {id}"));
            }
            Err(err) => {
                self.custom_providers_error = Some(err);
            }
        }
        cx.notify();
    }
}
