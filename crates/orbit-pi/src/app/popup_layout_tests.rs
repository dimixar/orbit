use super::*;

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
