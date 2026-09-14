use super::*;

// ── settings select popup anchoring ───────────────────────────────────

/// A settings select control: a 26 px chip with its popup anchored to a
/// zero-size point at the chip's top-right, exactly like `select_control`.
/// `legacy` reproduces the old `Local` + snap anchoring; the fixed code
/// uses `Window` mode so `anchored` can flip the popup when it would
/// overflow the viewport.
struct SettingsSelectAnchorProbe {
    legacy: bool,
    /// Place the chip near the viewport bottom instead of the top.
    near_bottom: bool,
}

impl Render for SettingsSelectAnchorProbe {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let vp = window.viewport_size();
        let gap = if self.near_bottom {
            vp.height - px(80.)
        } else {
            px(120.)
        };
        let chip = div()
            .id("anchor-chip")
            .debug_selector(|| "anchor-chip".to_string())
            .h(px(26.))
            .w(px(120.))
            .bg(gpui::black());
        let popup = div()
            .id("anchor-popup")
            .debug_selector(|| "anchor-popup".to_string())
            .w(px(360.))
            .h(px(240.))
            .bg(gpui::black());
        let anchored_popup = if self.legacy {
            anchored()
                .position_mode(AnchoredPositionMode::Local)
                .anchor(Corner::BottomRight)
                .offset(point(px(0.), px(-4.)))
                .snap_to_window()
                .child(deferred(popup))
                .into_any_element()
        } else {
            anchored()
                .position_mode(AnchoredPositionMode::Window)
                .anchor(Corner::TopRight)
                .offset(point(px(0.), px(30.)))
                .child(deferred(popup))
                .into_any_element()
        };
        div()
            .flex()
            .flex_col()
            .items_end()
            .w(px(600.))
            .child(div().h(gap).flex_none())
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_end()
                    .child(
                        div()
                            .absolute()
                            .top_0()
                            .right_0()
                            .size(px(0.))
                            .child(anchored_popup),
                    )
                    .child(chip),
            )
    }
}

/// The old `Local` + `snap_to_window` anchor cannot flip: with no room
/// above the chip, the snap slides the popup down over the chip itself,
/// so a second click on the chip lands inside the popup instead of
/// toggling it closed.
#[gpui::test]
fn local_snap_anchor_covers_the_chip_near_the_viewport_top(cx: &mut gpui::TestAppContext) {
    let cx = cx.add_empty_window();
    let _ = cx.draw(
        point(px(0.), px(0.)),
        gpui::size(px(600.), px(800.)),
        |_, cx| {
            cx.new(|_| SettingsSelectAnchorProbe {
                legacy: true,
                near_bottom: false,
            })
        },
    );
    let chip = cx.debug_bounds("anchor-chip").expect("chip laid out");
    let popup = cx.debug_bounds("anchor-popup").expect("popup laid out");
    assert!(
        popup.top() < chip.bottom() && popup.bottom() > chip.top(),
        "expected the snapped popup to cover the chip: chip {chip:?}, popup {popup:?}"
    );
}

/// The fixed anchor drops the popup below the chip, right-aligned, with a
/// 4 px gap — the platform's combobox direction and the pattern every
/// other dropdown in the app follows.
#[gpui::test]
fn settings_select_popup_opens_below_its_chip(cx: &mut gpui::TestAppContext) {
    let cx = cx.add_empty_window();
    let _ = cx.draw(
        point(px(0.), px(0.)),
        gpui::size(px(600.), px(800.)),
        |_, cx| {
            cx.new(|_| SettingsSelectAnchorProbe {
                legacy: false,
                near_bottom: false,
            })
        },
    );
    let chip = cx.debug_bounds("anchor-chip").expect("chip laid out");
    let popup = cx.debug_bounds("anchor-popup").expect("popup laid out");
    assert_eq!(
        popup.right(),
        chip.right(),
        "popup right-aligns to the chip"
    );
    assert_eq!(
        popup.top() - chip.bottom(),
        px(4.),
        "4 px gap below the chip"
    );
}

/// Near the viewport bottom the popup flips above the chip rather than
/// snapping over it; it sits flush so the chip stays clickable.
#[gpui::test]
fn settings_select_popup_flips_above_near_the_viewport_bottom(cx: &mut gpui::TestAppContext) {
    let cx = cx.add_empty_window();
    let _ = cx.draw(
        point(px(0.), px(0.)),
        gpui::size(px(600.), px(800.)),
        |_, cx| {
            cx.new(|_| SettingsSelectAnchorProbe {
                legacy: false,
                near_bottom: true,
            })
        },
    );
    let chip = cx.debug_bounds("anchor-chip").expect("chip laid out");
    let popup = cx.debug_bounds("anchor-popup").expect("popup laid out");
    assert_eq!(
        popup.bottom(),
        chip.top(),
        "popup sits flush above the chip"
    );
    assert!(popup.top() >= px(0.), "flipped popup stays on screen");
}

/// The settings popup's list: a `uniform_list` with an explicit height
/// (30 px row stride + 8 px vertical padding, capped at 220 px), inside
/// the popup's flex column under a 34 px search field.
struct SettingsPopupListTestView(gpui::UniformListScrollHandle);

impl Render for SettingsPopupListTestView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let rows = 100;
        let list_h = (rows as f32 * 30. + 8.).min(220.);
        div().flex().flex_col().child(div().h(px(34.))).child(
            uniform_list("settings-select-list", rows, |range, _, _| {
                range
                    .map(|ix| {
                        div()
                            .h(px(30.))
                            .w_full()
                            .child(format!("row {ix}"))
                            .into_any_element()
                    })
                    .collect()
            })
            .track_scroll(self.0.clone())
            .w_full()
            .h(px(list_h))
            .px(px(4.))
            .py(px(4.)),
        )
    }
}

/// The settings popup lives inside `anchored` inside a 0×0 div, so its
/// available height is 0. `uniform_list`'s Infer sizing reads the
/// available height and collapses to nothing there — the explicit height
/// is what keeps the list visible. Regression test for the empty
/// font/theme dropdowns.
#[gpui::test]
fn settings_popup_list_keeps_a_viewport_in_zero_height_context(cx: &mut gpui::TestAppContext) {
    use gpui::size;

    let cx = cx.add_empty_window();
    let scroll = gpui::UniformListScrollHandle::new();
    let _ = cx.draw(point(px(0.), px(0.)), size(px(200.), px(0.)), |_, cx| {
        cx.new(|_| SettingsPopupListTestView(scroll.clone()))
    });
    let state = scroll.0.borrow();
    let size = state.last_item_size.expect("list was laid out");
    assert!(
        size.item.height > px(0.),
        "settings popup list collapsed to a 0-height viewport"
    );
}

/// The same list with `max_h` instead of an explicit height collapses to
/// 0 — this is the gpui behavior the explicit height works around.
#[gpui::test]
fn max_h_collapses_uniform_list_in_zero_height_context(cx: &mut gpui::TestAppContext) {
    use gpui::size;

    let cx = cx.add_empty_window();
    let scroll = gpui::UniformListScrollHandle::new();
    let _ = cx.draw(point(px(0.), px(0.)), size(px(200.), px(0.)), |_, cx| {
        cx.new(|_| MaxHeightListTestView(scroll.clone()))
    });
    let state = scroll.0.borrow();
    let size = state.last_item_size.expect("list was laid out");
    assert_eq!(
        size.item.height,
        px(0.),
        "max_h + Infer sizing collapses to 0 in a 0-height available space"
    );
}

/// Same list as [`SettingsPopupListTestView`] but with `max_h` — documents
/// the gpui sizing behavior the explicit height works around.
struct MaxHeightListTestView(gpui::UniformListScrollHandle);

impl Render for MaxHeightListTestView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        uniform_list("settings-select-list", 100, |range, _, _| {
            range
                .map(|ix| {
                    div()
                        .h(px(30.))
                        .w_full()
                        .child(format!("row {ix}"))
                        .into_any_element()
                })
                .collect()
        })
        .track_scroll(self.0.clone())
        .w_full()
        .max_h(px(220.))
        .px(px(4.))
        .py(px(4.))
    }
}
