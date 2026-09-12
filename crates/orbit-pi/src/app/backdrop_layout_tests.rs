use super::*;
use gpui::size;

fn solid_image(width: u32, height: u32) -> std::sync::Arc<gpui::RenderImage> {
    let buffer = image::RgbaImage::from_pixel(width, height, image::Rgba([120, 120, 120, 255]));
    std::sync::Arc::new(gpui::RenderImage::new(vec![image::Frame::new(buffer)]))
}

/// gpui's `ObjectFit::Cover` scales the image up and centers it, so the
/// painted bounds overflow the element whenever the ratios differ (a 16:9
/// image in a narrower main area). Without a clip on the wrapper, that
/// overflow painted over the sessions sidebar. This documents the gpui
/// behavior the wrapper's `overflow_hidden` guards against.
#[test]
fn cover_bounds_overflow_the_element_on_a_ratio_mismatch() {
    let element = gpui::bounds(point(px(240.), px(0.)), size(px(1000.), px(700.)));
    let painted = ObjectFit::Cover.get_bounds(
        element,
        size(gpui::DevicePixels(1600), gpui::DevicePixels(900)),
    );
    assert!(
        painted.origin.x < element.origin.x,
        "cover must overflow left (painted {:?} vs element {:?})",
        painted.origin.x,
        element.origin.x
    );
    assert!(painted.size.width > element.size.width);
}

/// The backdrop wrapper fills the main area exactly — it must never claim
/// the sidebar's column, however the image inside it is fitted.
#[gpui::test]
fn backdrop_wrapper_stays_inside_the_main_area(cx: &mut gpui::TestAppContext) {
    let cx = cx.add_empty_window();
    let _ = cx.draw(point(px(0.), px(0.)), size(px(1240.), px(700.)), |_, _| {
        div()
            .size_full()
            .flex()
            .child(
                div()
                    .id("backdrop-test-sidebar")
                    .debug_selector(|| "backdrop-test-sidebar".to_string())
                    .flex_none()
                    .w(px(240.))
                    .h_full(),
            )
            .child(
                div()
                    .id("backdrop-test-main")
                    .debug_selector(|| "backdrop-test-main".to_string())
                    .flex_1()
                    .h_full()
                    .relative()
                    .children(OrbitApp::backdrop_image(
                        Some(solid_image(1600, 900)),
                        Theme::for_id(ThemeId::Orbit),
                    )),
            )
    });
    let sidebar = cx.debug_bounds("backdrop-test-sidebar").expect("sidebar");
    let main = cx.debug_bounds("backdrop-test-main").expect("main");
    assert_eq!(sidebar.origin.x, px(0.));
    assert_eq!(sidebar.size.width, px(240.));
    assert_eq!(main.origin.x, px(240.), "main starts after the sidebar");
    assert_eq!(main.size.width, px(1000.));
}

/// Regression: a provider card's name/id column must get a real width.
/// `truncate()` collapses to a bare ellipsis when the text element's width
/// resolves to 0 — which happens when `w_full()`/`min_w_0()` are set on the
/// text inside a `flex_1` column. Relying on the parent's stretch is what
/// keeps the name visible.
/// Regression: a provider card's name/id column must get a real width.
/// `truncate()` renders a bare ellipsis when its element resolves to zero
/// width, which happens when `w_full()` is used inside a flex-grow column.
/// The card relies on the parent's stretch and gives the name the full
/// width (the status pill lives on the badges row, not beside the name).
#[gpui::test]
fn provider_name_column_has_a_real_width(cx: &mut gpui::TestAppContext) {
    use gpui::{point, size};

    struct CardHeaderTestView;
    impl Render for CardHeaderTestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let theme = Theme::for_id(ThemeId::Orbit);
            // One 3-column grid cell's worth of width.
            div()
                .w(px(320.))
                .flex()
                .flex_col()
                .p(px(14.))
                .gap_2p5()
                .child(
                    div()
                        .flex()
                        .items_start()
                        .gap_3()
                        .child(div().size(px(52.)).flex_none())
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(
                                    div()
                                        .id("provider-name")
                                        .debug_selector(|| "provider-name".to_string())
                                        .text_size(theme.ui_px(14.))
                                        .truncate()
                                        .child("Amazon Bedrock"),
                                )
                                .child(
                                    div()
                                        .id("provider-id")
                                        .debug_selector(|| "provider-id".to_string())
                                        .text_size(theme.code_px(10.5))
                                        .truncate()
                                        .child("amazon-bedrock"),
                                ),
                        ),
                )
                .child(
                    div()
                        .id("provider-endpoint")
                        .debug_selector(|| "provider-endpoint".to_string())
                        .text_size(theme.code_px(10.5))
                        .truncate()
                        .child("https://api.example.com/v1 · openai-completions"),
                )
        }
    }

    let cx = cx.add_empty_window();
    let _ = cx.draw(point(px(0.), px(0.)), size(px(1000.), px(300.)), |_, cx| {
        cx.new(|_| CardHeaderTestView)
    });
    let name = cx.debug_bounds("provider-name").expect("name laid out");
    let id = cx.debug_bounds("provider-id").expect("id laid out");
    let endpoint = cx
        .debug_bounds("provider-endpoint")
        .expect("endpoint laid out");
    assert!(
        name.size.width > px(150.),
        "provider name collapsed to an ellipsis: {:?}",
        name.size
    );
    assert!(
        id.size.width > px(150.),
        "provider id collapsed: {:?}",
        id.size
    );
    assert!(
        endpoint.size.width > px(250.),
        "provider endpoint collapsed: {:?}",
        endpoint.size
    );
}

/// Regression: the Agent tab's long card descriptions blew the settings
/// content column past the window. A text element's min-content width is
/// its full unwrapped line in gpui, so a `flex_1` column with default
/// `min-width: auto` refuses to shrink and the card (and its toggle)
/// overflow off-screen. `min_w_0` on the settings root and content column
/// lets the column take the window width; the description then wraps.
#[gpui::test]
fn settings_cards_stay_inside_the_content_column(cx: &mut gpui::TestAppContext) {
    use gpui::{point, size};

    struct SettingsCardTestView;
    impl Render for SettingsCardTestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let theme = Theme::for_id(ThemeId::Orbit);
            let card = div()
                    .id("settings-card")
                    .debug_selector(|| "settings-card".to_string())
                    .bg(theme.bg_composer)
                    .border_1()
                    .border_color(theme.border)
                    .rounded_lg()
                    .px(px(14.))
                    .py(px(12.))
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
                                    .child("Follow-up messages"),
                            )
                            .child(
                                div()
                                    .id("settings-card-desc")
                                    .debug_selector(|| "settings-card-desc".to_string())
                                    .text_size(theme.ui_px(12.))
                                    .child("Messages sent while the agent is running wait in the queue above the composer and are delivered once the current task finishes. All delivers the whole queue at once; One at a time delivers one per run."),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(div().h(px(28.)).px(px(10.)).child("One at a time"))
                            .child(div().h(px(28.)).px(px(10.)).child("All")),
                    );

            div()
                .size_full()
                .flex()
                .child(div().flex_none().w(px(240.)).h_full())
                .child(
                    div()
                        .id("settings-content-column")
                        .debug_selector(|| "settings-content-column".to_string())
                        .flex_1()
                        .min_w_0()
                        .min_h_0()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .id("settings-scroll")
                                .flex_1()
                                .min_h_0()
                                .overflow_y_scroll()
                                .child(
                                    div()
                                        .w_full()
                                        .px(px(24.))
                                        .flex()
                                        .flex_col()
                                        .gap_3()
                                        .child(card),
                                ),
                        ),
                )
        }
    }

    let cx = cx.add_empty_window();
    let _ = cx.draw(point(px(0.), px(0.)), size(px(1240.), px(700.)), |_, cx| {
        cx.new(|_| SettingsCardTestView)
    });
    let card = cx.debug_bounds("settings-card").expect("card");
    let desc = cx.debug_bounds("settings-card-desc").expect("description");
    let content = cx
        .debug_bounds("settings-content-column")
        .expect("content column");
    assert_eq!(
        content.size.width,
        px(1000.),
        "content column must take the space beside the 240 px nav"
    );
    assert!(
        card.origin.x + card.size.width <= px(1240.),
        "card overflows the window: {:?}",
        card
    );
    assert!(
        desc.size.height > px(30.),
        "long description must wrap to multiple lines, got {:?}",
        desc.size
    );
    assert!(
        desc.size.width < card.size.width,
        "description must be narrower than the card so the control fits: {:?}",
        desc.size
    );
}

/// Regression: an SVG defaults to `flex-shrink: 1`, so beside wide text
/// (e.g. the transcript's activity label + copy button) it collapses to
/// zero width — the "icons render tiny" bug. `icon`/`icon_dyn`/`glyph`
/// set `flex_none`; this pins both halves of that behavior.
#[gpui::test]
fn flex_none_keeps_icons_from_collapsing(cx: &mut gpui::TestAppContext) {
    use gpui::{point, size};

    struct Probe;
    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
                .w(px(120.))
                .h(px(40.))
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .whitespace_nowrap()
                        .child("a very long unwrapping label that will not shrink at all"),
                )
                .child(
                    gpui::svg()
                        .path("icons/copy.svg")
                        .id("probe-icon-fixed")
                        .debug_selector(|| "probe-icon-fixed".to_string())
                        .flex_none()
                        .size(px(16.)),
                )
                .child(
                    gpui::svg()
                        .path("icons/copy.svg")
                        .id("probe-icon-shrunk")
                        .debug_selector(|| "probe-icon-shrunk".to_string())
                        .size(px(16.)),
                )
        }
    }

    let cx = cx.add_empty_window();
    let _ = cx.draw(point(px(0.), px(0.)), size(px(400.), px(200.)), |_, cx| {
        cx.new(|_| Probe)
    });
    let fixed = cx.debug_bounds("probe-icon-fixed").expect("fixed icon");
    let shrunk = cx.debug_bounds("probe-icon-shrunk").expect("shrunk icon");
    assert_eq!(
        fixed.size.width,
        px(16.),
        "flex_none icon must keep its size: {:?}",
        fixed
    );
    assert!(
        shrunk.size.width < px(16.),
        "without flex_none the icon collapses: {:?}",
        shrunk
    );
}
