use super::*;

/// Collapsed, there is no sidebar column to hug, so the controls take the
/// fixed lead past the OS window buttons — the one position that is correct
/// at any window width.
#[test]
fn collapsed_controls_take_the_fixed_lead() {
    assert_eq!(
        view::titlebar_controls_left(false, 0.),
        view::TRAFFIC_LIGHT_CLEARANCE + view::TITLEBAR_CONTROLS_LEAD
    );
    assert_eq!(
        view::titlebar_controls_left(false, SIDEBAR_DEFAULT_W),
        view::titlebar_controls_left(false, 0.),
        "a hidden sidebar must not shift the fixed lead"
    );
}

/// The title still has to clear the cluster once the sidebar is gone: its
/// leading inset must leave `TITLEBAR_TITLE_GAP` past the chips' edge on top
/// of the cluster's own trailing pad, or the two sit flush.
#[test]
fn collapsed_title_clears_the_controls() {
    let chips_end = view::titlebar_controls_left(false, 0.) + view::TITLEBAR_CONTROLS_W;
    assert_eq!(
        view::TITLEBAR_LEADING - chips_end,
        view::TITLEBAR_TITLE_GAP,
        "the title must start a real gap past the controls"
    );
}

/// Open, the cluster right-aligns inside the sidebar's strip: whatever the
/// sidebar's width, its right edge lands `SIDEBAR_EDGE_PAD` off the column's
/// edge (clearing the 6px resize handle that lives against it) and its left
/// edge clears the traffic lights.
#[test]
fn open_controls_hug_the_sidebar_edge() {
    for width in [SIDEBAR_MIN_W, SIDEBAR_DEFAULT_W, 420.] {
        let left = view::titlebar_controls_left(true, width);
        assert_eq!(
            left + view::TITLEBAR_CONTROLS_W + view::SIDEBAR_EDGE_PAD,
            width,
            "cluster must sit SIDEBAR_EDGE_PAD off the sidebar edge at {width}px"
        );
        assert!(
            left >= view::TRAFFIC_LIGHT_CLEARANCE,
            "at {width}px the chips would ride into the OS window buttons"
        );
    }
}
