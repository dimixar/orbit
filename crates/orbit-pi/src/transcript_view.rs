//! Transcript paint — Waku `transcript_view.rs` layout, honest Orbit data.
//!
//! Rows are plain GPUI flex trees (no component library): user turns are
//! End-aligned neutral bubbles, assistant turns are Start-aligned prose. A
//! ghost copy/timestamp footer reveals on row hover. Settled: **Worked for**
//! fold → thinking/tool cards → answer → files → copy footer. Live: thinking
//! + activity cards → answer → files → **Working for**.
//!
//! Timestamps render on the footer when pi provides one (snapshot
//! `timestamp` fields, or a wall-clock stamp taken at `message_end`).
//! Not painted (pi does not provide them): git Review, conversation fork,
//! per-tool checkpoint diffs.

use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    rc::Rc,
    time::{Duration, Instant},
};

use gpui::{
    div, img, linear_color_stop, linear_gradient, list, point, prelude::*, px, relative, svg,
    AnyElement, ClipboardItem, ElementId, Font, FontFeatures, FontStyle, FontWeight, Hsla,
    ImageSource, InteractiveText, ObjectFit, Pixels, ScrollHandle, SharedString,
    StrikethroughStyle, StyledText, TextRun, UnderlineStyle,
};

use std::ops::Range;

use unicode_segmentation::UnicodeSegmentation;

use serde_json::Value;

use crate::message_scroller::{self, MessageScrollerState};
use crate::theme::{self, Theme};
use crate::transcript::{changed_files, ChatMessage, Step, ToolCall};

/// Waku `CONTENT_MAX_WIDTH` (the `max-w-[760px]` transcript column).
const CONTENT_MAX_WIDTH: f32 = 760.0;
/// Extra space before a follow-up user message (Waku `pt-8`).
const FOLLOWUP_TURN_TOP_GAP: f32 = 32.0;
/// Waku user-bubble `max-w-[540px]`.
const USER_BUBBLE_MAX_WIDTH: f32 = 540.0;
/// Waku message-footer action button `size-[27px]`.
const FOOTER_BUTTON_SIZE: f32 = 27.0;
const COPY_FEEDBACK: Duration = Duration::from_secs(2);
const NAVIGATION_RAIL_LEFT: f32 = 16.0;
const NAVIGATION_RAIL_WIDTH: f32 = 44.0;
const NAVIGATION_RAIL_TICK_WIDTH: f32 = 32.0;
const NAVIGATION_RAIL_TICK_HEIGHT: f32 = 2.0;
const NAVIGATION_RAIL_TURN_HEIGHT: f32 = 12.0;
const NAVIGATION_RAIL_INACTIVE_OPACITY: f32 = 0.45;
/// Tick width by emphasis distance from the hovered turn (Waku rail).
const NAVIGATION_RAIL_EMPHASIS_SCALE: [f32; 4] = [1.0, 0.68, 0.44, 0.25];
/// Waku caps the rail at 80% of the viewport and hides it below an 872px
/// transcript container.
const NAVIGATION_RAIL_MAX_HEIGHT: f32 = 0.8;
const NAVIGATION_RAIL_MIN_MAIN_WIDTH: f32 = 872.0;
const CHANGED_FILES_PREVIEW_LIMIT: usize = 3;
const CONTENT_GAP: f32 = 10.0;
const NAVIGATION_RAIL_CONTENT_GAP: f32 = 12.0;
const NAVIGATION_RAIL_PREVIEW_WIDTH: f32 = 320.0;
const NAVIGATION_RAIL_PREVIEW_MAX_HEIGHT: f32 = 126.0;
/// Detail JSON/outputs are truncated to keep one virtualized row bounded.
const DETAIL_TEXT_CAP: usize = 4000;
/// Code-block copy feedback shares the detail-section feedback map; section
/// `2` never collides with Arguments (`0`) / Output (`1`).
const CODE_COPY_SECTION: u8 = 2;
const CODE_COPY_BUTTON: f32 = 24.0;

type ExpandedActivities = Rc<RefCell<HashMap<(usize, usize), bool>>>;
type ExpandedTools = Rc<RefCell<HashSet<(usize, usize)>>>;
type CopiedSections = Rc<RefCell<HashMap<(usize, usize, u8), Instant>>>;

pub(crate) struct TranscriptView {
    pub messages: Rc<RefCell<Vec<ChatMessage>>>,
    pub scroller: MessageScrollerState,
    pub streaming: Rc<Cell<Option<usize>>>,
    pub stream_started: Rc<Cell<Option<Instant>>>,
    pub expanded_turns: Rc<RefCell<HashSet<usize>>>,
    pub expanded_files: Rc<RefCell<HashSet<usize>>>,
    pub expanded_activities: ExpandedActivities,
    /// Per-tool detail-card open state, keyed `(message_ix, tool_ix)`.
    pub expanded_tools: ExpandedTools,
    pub copied: Rc<RefCell<HashMap<usize, Instant>>>,
    /// Per detail-section copy feedback, keyed `(message_ix, tool_ix, section)`.
    pub copied_sections: CopiedSections,
    /// Rail tick currently hovered (drives the turn preview card).
    pub hovered_turn: Rc<Cell<Option<usize>>>,
    /// Transcript row currently hovered (reveals the ghost footer).
    pub hovered_row: Rc<Cell<Option<usize>>>,
    /// Workspace of the open session — roots the Review git diff.
    pub workspace: Option<PathBuf>,
    /// Viewport height (caps the rail at 80%, like Waku).
    pub viewport_height: Pixels,
    /// Main-area width (gates the rail at 872px, like Waku).
    pub main_width: Pixels,
    /// Rail scroll position + last auto-scrolled turn.
    pub rail_scroll: ScrollHandle,
    pub rail_autoscroll: Rc<Cell<Option<usize>>>,
    /// End-of-task changed-files summary — `Some` renders one extra row
    /// after the last message (the list's tail slot).
    pub summary_files: Option<Vec<(String, u64, u64)>>,
    /// When the settled run finished (last message's timestamp) — shown in
    /// the summary card's footer next to the copy affordance.
    pub summary_finished_at: Option<i64>,
}

struct RowPaint {
    messages: Rc<RefCell<Vec<ChatMessage>>>,
    ix: usize,
    row_count: usize,
    theme: Theme,
    live: bool,
    live_elapsed: Option<Duration>,
    fold_open: bool,
    files_open: bool,
    copied: bool,
    expanded_turns: Rc<RefCell<HashSet<usize>>>,
    expanded_files: Rc<RefCell<HashSet<usize>>>,
    /// End-of-task summary row is shown — the last message's own
    /// changed-files card is then subsumed by it.
    tail_summary: bool,
    expanded_activities: ExpandedActivities,
    copied_at: Rc<RefCell<HashMap<usize, Instant>>>,
    hovered_row: Rc<Cell<Option<usize>>>,
    workspace: Option<PathBuf>,
    scroller: MessageScrollerState,
    expanded_tools: ExpandedTools,
    copied_sections: CopiedSections,
}

pub(crate) fn render_transcript(view: TranscriptView, cx: &gpui::App) -> impl IntoElement + use<> {
    let theme = *theme::get(cx);
    let messages = view.messages.clone();
    let streaming = view.streaming.clone();
    let stream_started = view.stream_started.clone();
    let expanded_turns = view.expanded_turns.clone();
    let expanded_files = view.expanded_files.clone();
    let expanded_activities = view.expanded_activities.clone();
    let expanded_tools = view.expanded_tools.clone();
    let copied = view.copied.clone();
    let copied_sections = view.copied_sections.clone();
    let hovered_turn = view.hovered_turn.clone();
    let hovered_row = view.hovered_row.clone();
    let workspace = view.workspace.clone();
    let rail_scroll = view.rail_scroll.clone();
    let rail_autoscroll = view.rail_autoscroll.clone();
    let scroller = view.scroller.clone();
    let summary_files = view.summary_files.clone();
    let summary_finished_at = view.summary_finished_at;

    let (user_turns, active_turn) = {
        let messages = messages.borrow();
        let user_turns: Vec<usize> = messages
            .iter()
            .enumerate()
            .filter(|(_, message)| message.user)
            .map(|(ix, _)| ix)
            .collect();
        // While the reader sits at the live edge the indicator tracks the
        // streaming/latest turn; once they scroll away it follows the
        // viewport instead (fixes the tick staying pinned to the newest
        // turn while reading earlier ones).
        let viewport_hint = if scroller.is_following_tail() {
            None
        } else {
            Some(scroller.first_visible_index())
        };
        let active_turn = active_user_index(&messages, streaming.get(), viewport_hint);
        (user_turns, active_turn)
    };
    let show_rail = user_turns.len() >= 2
        && view.main_width >= px(NAVIGATION_RAIL_MIN_MAIN_WIDTH)
        && scroller.is_scrollable();

    // One prompt/response preview pair per user turn (rail hover cards).
    let turn_snippets: Vec<(String, String)> = {
        let messages = messages.borrow();
        user_turns
            .iter()
            .map(|&ix| {
                let prompt = snippet(&messages[ix].text(), 100);
                let response = messages
                    .get(ix + 1..)
                    .unwrap_or(&[])
                    .iter()
                    .take_while(|message| !message.user)
                    .find(|message| !message.text().trim().is_empty())
                    .map(|message| snippet(&message.text(), 240))
                    .unwrap_or_default();
                (prompt, response)
            })
            .collect()
    };

    let list_el = list(view.scroller.list_state(), move |ix, _window, cx| {
        let live = streaming.get() == Some(ix);
        let live_elapsed = if live {
            stream_started.get().map(|started| started.elapsed())
        } else {
            None
        };
        let fold_open = expanded_turns.borrow().contains(&ix);
        let files_open = expanded_files.borrow().contains(&ix);
        let copied_now = copied
            .borrow()
            .get(&ix)
            .is_some_and(|at| at.elapsed() < COPY_FEEDBACK);
        let row_count = messages.borrow().len();
        // The list's tail slot (ix == row_count) is the end-of-task
        // changed-files summary — pinned after the last message, in the
        // same centered column as every other row.
        if ix >= row_count {
            let Some(files) = summary_files.as_ref() else {
                return div().into_any_element();
            };
            let card = render_changed_files(
                files,
                *theme::get(cx),
                ix,
                workspace.as_deref(),
                expanded_files.borrow().contains(&ix),
                expanded_files.clone(),
                scroller.clone(),
            );
            // Waku-style footer under the summary card: copy affordance
            // (duplicates the changed-file list) + settled timestamp.
            let copied_now = copied
                .borrow()
                .get(&ix)
                .is_some_and(|at| at.elapsed() < COPY_FEEDBACK);
            let stamp = summary_finished_at
                .map(|millis| {
                    div()
                        .text_size(theme.ui_px(11.5))
                        .text_color(theme.text_3)
                        .child(summary_time_label(millis))
                })
                .unwrap_or_else(|| div());
            let footer = div()
                .mt(px(10.))
                .px(px(4.))
                .flex()
                .items_center()
                .gap(px(12.))
                .child(
                    div()
                        .id(ElementId::NamedInteger(
                            "copy-changed-files".into(),
                            ix as u64,
                        ))
                        .flex()
                        .items_center()
                        .cursor_pointer()
                        .child(glyph(
                            if copied_now {
                                "icons/check.svg"
                            } else {
                                "icons/copy.svg"
                            },
                            13.,
                            if copied_now {
                                theme.ok_green
                            } else {
                                theme.text_3
                            },
                        ))
                        .on_click({
                            let files = files.clone();
                            let copied = copied.clone();
                            move |_, _, cx| {
                                let text = files
                                    .iter()
                                    .map(|(path, _, _)| path.as_str())
                                    .collect::<Vec<_>>()
                                    .join("\n");
                                cx.write_to_clipboard(ClipboardItem::new_string(text));
                                copied.borrow_mut().insert(ix, Instant::now());
                                cx.refresh_windows();
                            }
                        }),
                )
                .child(stamp);
            return div()
                .id(ElementId::NamedInteger("transcript-row".into(), ix as u64))
                .w_full()
                .flex()
                .justify_center()
                .px(px(20.))
                .pb(px(22.))
                .child(
                    div()
                        .w_full()
                        .max_w(px(CONTENT_MAX_WIDTH))
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(card)
                        .child(footer),
                )
                .into_any_element();
        }
        render_row(RowPaint {
            messages: messages.clone(),
            ix,
            row_count,
            theme: *theme::get(cx),
            live,
            live_elapsed,
            fold_open,
            files_open,
            copied: copied_now,
            expanded_turns: expanded_turns.clone(),
            expanded_files: expanded_files.clone(),
            tail_summary: summary_files.is_some(),
            expanded_activities: expanded_activities.clone(),
            copied_at: copied.clone(),
            hovered_row: hovered_row.clone(),
            workspace: workspace.clone(),
            scroller: scroller.clone(),
            expanded_tools: expanded_tools.clone(),
            copied_sections: copied_sections.clone(),
        })
        .into_any_element()
    })
    .w_full()
    .h_full();

    // Same height chain as the sessions sidebar: this panel is a `flex_1` +
    // `min_h_0` child of a column, so `h_full` on the list is a definite
    // height. Jump-to-latest and the bottom fade live on the scroller wrap.
    let scroller = message_scroller::render_scroller(view.scroller.clone(), theme, list_el);
    div()
        .id(ElementId::Name("transcript-panel".into()))
        .w_full()
        .h_full()
        .min_h_0()
        .relative()
        .child(scroller)
        .when(show_rail, |shell| {
            shell.child(render_navigation_rail(
                turn_snippets,
                user_turns,
                active_turn,
                hovered_turn.clone(),
                view.scroller.clone(),
                view.viewport_height,
                rail_scroll,
                rail_autoscroll,
                theme,
            ))
        })
}

/// Waku's conversation rail: ticks per user turn, vertically centered,
/// capped at 80% of the viewport, wheel-scrollable with edge fades. Tick
/// widths fan out around the *hovered* turn only; the active turn is the
/// full-opacity tick while idle.
#[allow(clippy::too_many_arguments)]
fn render_navigation_rail(
    snippets: Vec<(String, String)>,
    user_turns: Vec<usize>,
    active_turn: Option<usize>,
    hovered: Rc<Cell<Option<usize>>>,
    scroller: MessageScrollerState,
    viewport_height: Pixels,
    rail_scroll: ScrollHandle,
    rail_autoscroll: Rc<Cell<Option<usize>>>,
    theme: Theme,
) -> AnyElement {
    let pitch = px(NAVIGATION_RAIL_TURN_HEIGHT);
    let content_height = px(user_turns.len() as f32 * NAVIGATION_RAIL_TURN_HEIGHT);
    let rail_height = content_height.min(viewport_height * NAVIGATION_RAIL_MAX_HEIGHT);
    let scrollable = content_height > rail_height + px(1.);
    let scroll_offset = rail_scroll.offset().y;

    // Waku scrolls the active tick into view whenever it changes.
    let active_pos =
        active_turn.and_then(|ix| user_turns.iter().position(|candidate| *candidate == ix));
    if scrollable {
        if let Some(pos) = active_pos {
            let top = px(pos as f32 * NAVIGATION_RAIL_TURN_HEIGHT);
            let visible_bottom = scroll_offset + rail_height;
            if rail_autoscroll.get() != Some(pos)
                && (top < scroll_offset || top + pitch > visible_bottom)
            {
                let target = (top + pitch / 2. - rail_height / 2.)
                    .clamp(px(0.), content_height - rail_height);
                rail_scroll.set_offset(point(px(0.), target));
            }
            rail_autoscroll.set(Some(pos));
        }
    }
    let scroll_offset = rail_scroll.offset().y;
    let at_top = !scrollable || scroll_offset <= px(0.5);
    let at_bottom = !scrollable || scroll_offset >= content_height - rail_height - px(0.5);

    // The emphasized (hovered) turn anchors the width fan-out; with nothing
    // hovered every tick rests at the 0.25 scale, like Waku's idle rail.
    let emphasized_turn = hovered.get();
    let emphasized_pos =
        emphasized_turn.and_then(|ix| user_turns.iter().position(|candidate| *candidate == ix));
    let hover_info =
        emphasized_pos.map(|pos| (pos, snippets[pos].0.clone(), snippets[pos].1.clone()));

    let ticks: Vec<(usize, String, f32, Hsla)> = user_turns
        .iter()
        .copied()
        .zip(snippets.iter().cloned())
        .enumerate()
        .map(|(turn_pos, (user_ix, (prompt, _response)))| {
            let distance = emphasized_pos.map(|pos| pos.abs_diff(turn_pos));
            let scale = distance
                .and_then(|d| NAVIGATION_RAIL_EMPHASIS_SCALE.get(d))
                .copied()
                .unwrap_or(0.25);
            let prominent = emphasized_turn == Some(user_ix) || active_turn == Some(user_ix);
            let width = NAVIGATION_RAIL_TICK_WIDTH * scale;
            let color = if prominent {
                theme.text
            } else {
                theme.text_3.opacity(NAVIGATION_RAIL_INACTIVE_OPACITY)
            };
            (user_ix, prompt, width, color)
        })
        .collect();

    let shell_hover = hovered.clone();
    let mut body = div()
        .relative()
        .w(px(NAVIGATION_RAIL_WIDTH))
        .h(rail_height)
        .child(
            div()
                .id(ElementId::Name("rail-ticks".into()))
                .absolute()
                .top_0()
                .left_0()
                .right_0()
                .bottom_0()
                .overflow_y_scroll()
                .track_scroll(&rail_scroll)
                .flex()
                .flex_col()
                .children(
                    ticks
                        .into_iter()
                        .map(move |(user_ix, _prompt, width, color)| {
                            let scroller = scroller.clone();
                            let hover_state = hovered.clone();
                            div()
                                .id(ElementId::NamedInteger("nav-tick".into(), user_ix as u64))
                                .w(px(NAVIGATION_RAIL_WIDTH))
                                .h(px(NAVIGATION_RAIL_TURN_HEIGHT))
                                .flex_none()
                                .flex()
                                .items_center()
                                .cursor_pointer()
                                .on_hover({
                                    let hover_state = hover_state.clone();
                                    move |hovering: &bool, _, cx| {
                                        // On leave, clear only when this tick
                                        // owns the hover state (moving between
                                        // ticks hands it to the next one).
                                        let next = if *hovering {
                                            Some(user_ix)
                                        } else if hover_state.get() == Some(user_ix) {
                                            None
                                        } else {
                                            hover_state.get()
                                        };
                                        if hover_state.get() != next {
                                            hover_state.set(next);
                                            cx.refresh_windows();
                                        }
                                    }
                                })
                                .on_click(move |_, _, cx| {
                                    scroller.scroll_to_item(user_ix);
                                    cx.refresh_windows();
                                })
                                .child(
                                    div()
                                        .id(ElementId::NamedInteger(
                                            "nav-tick-bar".into(),
                                            user_ix as u64,
                                        ))
                                        .h(px(NAVIGATION_RAIL_TICK_HEIGHT))
                                        .w(px(width))
                                        .rounded_full()
                                        .bg(color)
                                        .hover(|style| style.bg(theme.text)),
                                )
                        }),
                ),
        )
        .when(!at_top, |rail| rail.child(render_rail_fade(true, theme)))
        .when(!at_bottom, |rail| {
            rail.child(render_rail_fade(false, theme))
        });

    // Hover preview, clamped inside the rail body's vertical span (Waku
    // clamps `previewTop` against the rail bounds).
    if let Some((pos, prompt, response)) = hover_info {
        let visible_center =
            px(pos as f32 * NAVIGATION_RAIL_TURN_HEIGHT) + pitch / 2. - scroll_offset;
        let preview_top = (visible_center - px(NAVIGATION_RAIL_PREVIEW_MAX_HEIGHT / 2.))
            .max(px(0.))
            .min((rail_height - px(NAVIGATION_RAIL_PREVIEW_MAX_HEIGHT)).max(px(0.)));
        body = body.child(render_rail_preview(&prompt, &response, theme, preview_top));
    }

    div()
        .id(ElementId::Name("conversation-navigation-rail".into()))
        .absolute()
        .top_0()
        .left(px(NAVIGATION_RAIL_LEFT))
        .w(px(NAVIGATION_RAIL_WIDTH))
        .h_full()
        .flex()
        .flex_col()
        .justify_center()
        // Belt and braces: leaving the rail entirely clears the preview even
        // if an individual tick's leave event was missed (e.g. scrolling).
        .on_hover({
            let hover_state = shell_hover;
            move |hovering: &bool, _, cx| {
                if !hovering && hover_state.get().is_some() {
                    hover_state.set(None);
                    cx.refresh_windows();
                }
            }
        })
        .child(body)
        .into_any_element()
}

/// Waku's rail edge fade: a 20px gradient from the background so scrolling
/// ticks dissolve instead of clipping.
fn render_rail_fade(top: bool, theme: Theme) -> impl IntoElement {
    let base = div()
        .absolute()
        .left_0()
        .right_0()
        .h(px(20.))
        .bg(linear_gradient(
            180.,
            linear_color_stop(theme.bg_main.opacity(0.), 0.),
            linear_color_stop(theme.bg_main, 1.),
        ));
    if top {
        // Solid at the top edge, fading down: flip the stop order.
        base.top_0().bg(linear_gradient(
            180.,
            linear_color_stop(theme.bg_main, 0.),
            linear_color_stop(theme.bg_main.opacity(0.), 1.),
        ))
    } else {
        base.bottom_0()
    }
}

/// Waku's rail hover card: the turn's prompt and a short response snippet,
/// vertically positioned by the caller (clamped to the rail's span) at
/// Waku's 60px left offset (rail width + gap).
fn render_rail_preview(
    prompt: &str,
    response: &str,
    theme: Theme,
    top: Pixels,
) -> impl IntoElement {
    div()
        .absolute()
        .left(px(NAVIGATION_RAIL_WIDTH + NAVIGATION_RAIL_CONTENT_GAP))
        .top(top)
        .w(px(NAVIGATION_RAIL_PREVIEW_WIDTH))
        .max_h(px(NAVIGATION_RAIL_PREVIEW_MAX_HEIGHT))
        .overflow_hidden()
        .rounded(px(14.))
        .border_1()
        .border_color(theme.border_strong)
        .bg(theme.bg_raised)
        .shadow(theme.card_shadow())
        .px(px(15.))
        .py(px(12.))
        .flex()
        .flex_col()
        .gap(px(7.))
        .child(
            div()
                .w_full()
                .truncate()
                .text_size(theme.ui_px(14.))
                .line_height(theme.ui_px(20.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.text)
                .child(prompt.to_string()),
        )
        .when(!response.is_empty(), |card| {
            card.child(
                div()
                    .w_full()
                    .max_h(px(60.))
                    .overflow_hidden()
                    .whitespace_normal()
                    .text_size(theme.ui_px(13.))
                    .line_height(theme.ui_px(20.))
                    .text_color(theme.text_3)
                    .child(response.to_string()),
            )
        })
}

/// Whitespace-normalized grapheme snippet
/// (same presentation Waku's navigation previews use).
fn snippet(text: &str, max_graphemes: usize) -> String {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut graphemes = normalized.graphemes(true);
    let head: String = graphemes.by_ref().take(max_graphemes).collect();
    if graphemes.next().is_some() {
        format!("{head}…")
    } else {
        head
    }
}

fn render_row(paint: RowPaint) -> AnyElement {
    let messages = paint.messages.borrow();
    let Some(message) = messages.get(paint.ix) else {
        return div().into_any_element();
    };
    let followup = starts_followup_turn(&messages, paint.ix);
    let first = paint.ix == 0;
    let last = paint.ix + 1 == paint.row_count;

    let inner = if message.user {
        render_user_bubble(message, &paint).into_any_element()
    } else {
        render_assistant(message, &paint).into_any_element()
    };

    let hovered_row = paint.hovered_row.clone();
    let ix = paint.ix;
    div()
        .id(ElementId::NamedInteger(
            "transcript-row".into(),
            paint.ix as u64,
        ))
        .w_full()
        .flex()
        .justify_center()
        .px(px(20.))
        .py(px(8.))
        .when(first, |row| row.pt(px(22.)))
        .when(followup, |row| row.pt(px(FOLLOWUP_TURN_TOP_GAP)))
        .when(last, |row| row.pb(px(22.)))
        .on_hover(move |hovering: &bool, _, cx| {
            let next = if *hovering { Some(ix) } else { None };
            if hovered_row.get() != next {
                hovered_row.set(next);
                cx.refresh_windows();
            }
        })
        .child(
            div()
                .w_full()
                .max_w(px(CONTENT_MAX_WIDTH))
                .min_w_0()
                .child(inner),
        )
        .into_any_element()
}

/// End-aligned user row: Waku's neutral raised bubble, ghost footer below.
/// Attached images render as a tile grid above the text bubble.
fn render_user_bubble(message: &ChatMessage, paint: &RowPaint) -> impl IntoElement {
    let theme = paint.theme;
    let ix = paint.ix;
    let revealed = paint.hovered_row.get() == Some(ix);
    let text = message.text();
    div()
        .w_full()
        .min_w_0()
        .flex()
        .flex_col()
        .items_end()
        .gap(px(4.))
        // Attachment tiles (images queued with the prompt). Cover-cropped
        // squares, Waku-style; wrap when a message carries several.
        .when(!message.images.is_empty(), |column| {
            column.child(
                div()
                    .max_w(px(USER_BUBBLE_MAX_WIDTH))
                    .flex()
                    .flex_wrap()
                    .justify_end()
                    .gap(px(6.))
                    .children(message.images.iter().map(|image| {
                        div()
                            .size(px(112.))
                            .rounded(px(10.))
                            .border_1()
                            .border_color(theme.border)
                            .overflow_hidden()
                            .bg(theme.bg_raised)
                            .child(
                                img(ImageSource::Image(image.clone()))
                                    .size_full()
                                    .object_fit(ObjectFit::Cover),
                            )
                    })),
            )
        })
        .when(!text.is_empty(), |column| {
            column.child(
                div()
                    .max_w(px(USER_BUBBLE_MAX_WIDTH))
                    .rounded(px(12.))
                    .bg(theme.bg_raised)
                    .text_color(theme.text)
                    .px(px(12.))
                    .py(px(8.))
                    .text_size(theme.ui_px(14.))
                    .whitespace_normal()
                    .child(render_prose(
                        &text,
                        ix,
                        0,
                        theme,
                        paint.copied_sections.clone(),
                    )),
            )
        })
        .child(render_message_footer(
            text,
            ix,
            message.finished_at,
            revealed,
            paint.copied,
            true,
            theme,
            paint.copied_at.clone(),
        ))
}

fn render_assistant(message: &ChatMessage, paint: &RowPaint) -> impl IntoElement {
    let theme = paint.theme;
    let ix = paint.ix;
    let mut content = div()
        .w_full()
        .max_w_full()
        .min_w_0()
        .flex()
        .flex_col()
        .items_start()
        .gap(px(CONTENT_GAP));

    // The turn fold precedes the run's first work (Waku: collapsed turns
    // hide the pre-answer work behind a single "Worked for" divider).
    if !paint.live && message.has_hidden_work() {
        content = content.child(render_turn_fold(
            ix,
            paint.fold_open,
            message.elapsed,
            theme,
            paint.expanded_turns.clone(),
            paint.scroller.clone(),
        ));
    }

    // Steps render in sequence: each step's thought/tool group sits directly
    // above the text it produced (Waku's interleaved activity rows).
    let answer_start = message
        .steps
        .iter()
        .position(|step| !step.text.trim().is_empty());
    let last_step = message.steps.len().saturating_sub(1);

    for (step_ix, step) in message.steps.iter().enumerate() {
        let before_answer = answer_start.is_none_or(|answer| step_ix < answer);
        let is_live_step = paint.live && step_ix == last_step;
        let has_work = !step.thinking.is_empty() || !step.tools.is_empty();

        if has_work {
            // Pre-answer work hides behind the turn fold; later bursts stay
            // visible as collapsed, expandable groups.
            let show_work = paint.live || paint.fold_open || !before_answer;
            if show_work {
                let open = paint
                    .expanded_activities
                    .borrow()
                    .get(&(ix, step_ix))
                    .copied()
                    .unwrap_or(is_live_step);
                content = content.child(render_step_group(
                    ix,
                    step_ix,
                    step,
                    open,
                    is_live_step,
                    paint.live_elapsed.unwrap_or(Duration::ZERO),
                    theme,
                    paint.expanded_activities.clone(),
                    paint.expanded_tools.clone(),
                    paint.copied_sections.clone(),
                    paint.scroller.clone(),
                ));
            }
        }

        if !step.text.is_empty() {
            content = content.child(div().w_full().min_w_0().pt(px(4.)).child(render_prose(
                &step.text,
                ix,
                (step_ix as u64 + 1) * 4096,
                theme,
                paint.copied_sections.clone(),
            )));
        }
    }

    let files = changed_files(message);
    // The end-of-task summary card (shown after the last row) aggregates
    // every turn's files — the final turn's own card would just duplicate it.
    let tail_covers = paint.tail_summary && ix + 1 == paint.row_count;
    if !files.is_empty() && !tail_covers {
        content = content.child(
            div()
                .w_full()
                .min_w_0()
                .pt(px(8.))
                .child(render_changed_files(
                    &files,
                    theme,
                    ix,
                    paint.workspace.as_deref(),
                    paint.files_open,
                    paint.expanded_files.clone(),
                    paint.scroller.clone(),
                )),
        );
    }

    if paint.live {
        content = content.child(render_working_indicator(
            paint.live_elapsed.unwrap_or(Duration::ZERO),
            theme,
        ));
    }

    let mut row = div()
        .w_full()
        .min_w_0()
        .flex()
        .flex_col()
        .items_start()
        .child(content);

    if !paint.live && !message.text().is_empty() {
        let revealed = paint.hovered_row.get() == Some(ix);
        row = row.child(render_message_footer(
            message.text(),
            ix,
            message.finished_at,
            revealed,
            paint.copied,
            false,
            theme,
            paint.copied_at.clone(),
        ));
    }

    row
}

/// Waku reasoning row: "Thinking" while live, "Thought for <duration>" once
/// settled; the raw reasoning text is expandable detail (auto-open live).
#[allow(clippy::too_many_arguments)]
/// One step's activity group: a collapsed "Ran 7 commands · 4 thoughts"
/// summary line (Waku) that expands into the thought card and the step's
/// tool rows.
/// Waku's activity summary: counts the step's tool kinds and thoughts —
/// "Ran 7 commands · 4 thoughts", "Ran 1 file read · 1 thought".
fn step_activity_title(step: &Step, live: bool) -> String {
    let mut parts: Vec<String> = Vec::new();
    let (mut commands, mut reads, mut edits, mut other) = (0usize, 0usize, 0usize, 0usize);
    for tool in &step.tools {
        match tool.name.as_str() {
            "bash" | "shell" => commands += 1,
            "read" | "grep" | "find" | "glob" | "search" => reads += 1,
            "edit" | "write" => edits += 1,
            _ => other += 1,
        }
    }
    let units = |n: usize, word: &str| format!("{} {}{}", n, word, if n == 1 { "" } else { "s" });
    if commands > 0 {
        parts.push(format!("Ran {}", units(commands, "command")));
    }
    if reads > 0 {
        parts.push(format!("Ran {}", units(reads, "file read")));
    }
    if edits > 0 {
        parts.push(format!("Ran {}", units(edits, "file edit")));
    }
    if other > 0 {
        parts.push(format!("Ran {}", units(other, "tool")));
    }
    if !step.thinking.is_empty() {
        parts.push(if live {
            "Thinking".to_string()
        } else {
            units(1, "thought")
        });
    }
    if parts.is_empty() {
        return if live {
            "Working".into()
        } else {
            "Worked".into()
        };
    }
    parts.join(" \u{b} ")
}

#[allow(clippy::too_many_arguments)]
fn render_step_group(
    ix: usize,
    step_ix: usize,
    step: &Step,
    open: bool,
    live: bool,
    elapsed: Duration,
    theme: Theme,
    expanded_activities: ExpandedActivities,
    expanded_tools: ExpandedTools,
    copied_sections: CopiedSections,
    scroller: MessageScrollerState,
) -> impl IntoElement {
    let title = step_activity_title(step, live);
    // Flat tool index keeps per-tool detail/copy keys stable across steps.
    let tool_base = 0usize;
    let last = step.tools.len().saturating_sub(1);
    let mut group = div()
        .w_full()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(px(4.))
        .child(
            div()
                .id(ElementId::NamedInteger(
                    "activity-toggle".into(),
                    (ix as u64) << 16 | step_ix as u64,
                ))
                .w_full()
                .min_w_0()
                .h(px(26.))
                .flex()
                .items_center()
                .gap(px(6.))
                .cursor_pointer()
                .text_size(theme.ui_px(12.5))
                .line_height(theme.ui_px(16.))
                .hover(|style| style.text_color(theme.text))
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.text_2)
                        .child(title),
                )
                .child(glyph(
                    if open {
                        "icons/chevron-down.svg"
                    } else {
                        "icons/chevron-right.svg"
                    },
                    10.,
                    theme.text_3,
                ))
                .on_click({
                    let expanded_activities = expanded_activities.clone();
                    let scroller = scroller.clone();
                    move |_, _, cx| {
                        let key = (ix, step_ix);
                        let mut map = expanded_activities.borrow_mut();
                        let next = !map.get(&key).copied().unwrap_or(false);
                        map.insert(key, next);
                        scroller.remeasure_items(ix..ix + 1);
                        cx.refresh_windows();
                    }
                }),
        );

    if open {
        let mut body = div()
            .w_full()
            .min_w_0()
            .ml(px(6.))
            .pl(px(12.))
            .pb(px(2.))
            .border_l_1()
            .border_color(theme.border)
            .flex()
            .flex_col()
            .gap(px(8.));
        if !step.thinking.is_empty() {
            body = body.child(render_thinking_body(&step.thinking, live, theme));
        }
        body = body.children(step.tools.iter().enumerate().map(|(tool_ix, tool)| {
            let flat = tool_base + tool_ix;
            render_activity_card(
                tool,
                live && tool_ix == last,
                !live,
                elapsed,
                theme,
                (ix, flat),
                expanded_tools.borrow().contains(&(ix, flat)),
                expanded_tools.clone(),
                copied_sections.clone(),
                scroller.clone(),
            )
        }));
        group = group.child(body);
    }

    group
}

/// The reasoning card body inside a step group ("Thinking" live, "Thought
/// for <duration>" settled).
fn render_thinking_body(thinking: &str, live: bool, theme: Theme) -> impl IntoElement {
    let title = if live {
        "Thinking".to_string()
    } else {
        "Thought".to_string()
    };
    let detail = cap_chars(thinking, DETAIL_TEXT_CAP);
    div()
        .rounded(px(9.))
        .border_1()
        .border_color(theme.border_strong)
        .bg(theme.overlay)
        .px(px(10.))
        .py(px(6.))
        .flex()
        .flex_col()
        .gap(px(4.))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .text_size(theme.ui_px(12.5))
                .line_height(theme.ui_px(16.))
                .child(glyph("icons/spark.svg", 12., theme.text_3))
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.tool_name)
                        .child(title),
                ),
        )
        .child(
            div()
                .font_family("Menlo")
                .text_size(theme.code_px(10.5))
                .line_height(theme.code_px(15.))
                .text_color(theme.tool_meta)
                .whitespace_normal()
                .child(detail),
        )
}

#[allow(clippy::too_many_arguments)]
fn render_activity_card(
    tool: &ToolCall,
    pulse: bool,
    complete: bool,
    elapsed: Duration,
    theme: Theme,
    key: (usize, usize),
    tool_open: bool,
    expanded_tools: ExpandedTools,
    copied_sections: CopiedSections,
    scroller: MessageScrollerState,
) -> AnyElement {
    let action = activity_action_label(&tool.name);
    let detail = activity_preview(tool);
    let has_diff = tool.added > 0 || tool.removed > 0;
    let added = tool.added;
    let removed = tool.removed;
    // Expandable like Waku's activity rows: full arguments and, when captured
    // live, the tool result.
    let has_detail =
        tool.args.as_ref().is_some_and(|args| !args.is_null()) || tool.output.is_some();

    let mut card = div()
        .id(ElementId::NamedInteger(
            "activity-card".into(),
            (key.0 as u64) << 16 | key.1 as u64,
        ))
        .w_full()
        .min_w_0()
        .overflow_hidden()
        .rounded(px(9.))
        .border_1()
        .border_color(theme.border_strong)
        .bg(theme.overlay)
        .child(
            div()
                .h(px(28.))
                .h(px(28.))
                .px(px(8.))
                .flex()
                .items_center()
                .gap(px(8.))
                .text_size(theme.ui_px(12.5))
                .line_height(theme.ui_px(16.))
                .when(has_detail, |row| row.cursor_pointer())
                .hover(|style| style.bg(theme.overlay_strong))
                .child(glyph(activity_icon(&tool.name), 12., theme.text_3))
                .child(
                    div()
                        .flex_none()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.tool_name)
                        .child(action),
                )
                .when(!detail.is_empty(), |row| {
                    row.child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_color(theme.tool_meta)
                            .child(detail),
                    )
                })
                .when(has_diff, |row| {
                    row.child(render_line_delta(added, removed, theme, 12.5))
                })
                .when(pulse, |row| {
                    row.child(pulse_dot(theme, elapsed.as_millis()))
                })
                .when(tool.failed, |row| {
                    row.child(
                        div()
                            .flex_none()
                            .text_size(theme.ui_px(11.))
                            .line_height(theme.ui_px(16.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.del_red)
                            .child("✕"),
                    )
                })
                .when(complete && !tool.failed, |row| {
                    row.child(glyph("icons/check.svg", 10., theme.text_3))
                })
                .child(glyph(
                    if has_detail && tool_open {
                        "icons/chevron-down.svg"
                    } else if has_detail {
                        "icons/chevron-right.svg"
                    } else {
                        // Placeholder keeps the row height stable either way.
                        "icons/chevron-right.svg"
                    },
                    10.,
                    if has_detail {
                        theme.text_3
                    } else {
                        theme.text_3.opacity(0.)
                    },
                )),
        )
        .when(has_detail, |row| {
            row.on_click(move |_, _, cx| {
                let mut open = expanded_tools.borrow_mut();
                if !open.remove(&key) {
                    open.insert(key);
                }
                drop(open);
                scroller.remeasure_items(key.0..key.0 + 1);
                cx.refresh_windows();
            })
        });

    if tool_open && has_detail {
        card = card.child(render_tool_detail(tool, key, copied_sections, theme));
    }
    card.into_any_element()
}

/// The expandable detail card: labeled Arguments / Output sections, each with
/// its own copy button (Waku `activity_disclosure_sections` parity).
fn render_tool_detail(
    tool: &ToolCall,
    key: (usize, usize),
    copied_sections: CopiedSections,
    theme: Theme,
) -> impl IntoElement {
    let sections: [(u8, &str, Option<String>); 2] = [
        (0, "Arguments", tool.args.as_ref().map(display_value)),
        (1, "Output", tool.output.as_ref().map(display_value)),
    ];
    div()
        .w_full()
        .min_w_0()
        .border_t_1()
        .border_color(theme.border_strong)
        .px(px(10.))
        .py(px(6.))
        .flex()
        .flex_col()
        .gap(px(6.))
        .children(
            sections
                .into_iter()
                .filter_map(|(section, label, content)| {
                    let content = content?;
                    if content.trim().is_empty() {
                        return None;
                    }
                    Some(render_detail_section(
                        key,
                        section,
                        label,
                        content,
                        copied_sections.clone(),
                        theme,
                    ))
                }),
        )
}

fn render_detail_section(
    key: (usize, usize),
    section: u8,
    label: &str,
    content: String,
    copied_sections: CopiedSections,
    theme: Theme,
) -> impl IntoElement {
    let copied = copied_sections
        .borrow()
        .get(&(key.0, key.1, section))
        .is_some_and(|at| at.elapsed() < COPY_FEEDBACK);
    let copy_content = content.clone();
    div()
        .w_full()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(px(3.))
        .child(
            div()
                .h(px(18.))
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .font_weight(FontWeight::MEDIUM)
                        .text_size(theme.ui_px(10.5))
                        .line_height(theme.ui_px(14.))
                        .text_color(theme.text_2)
                        .child(label.to_string()),
                )
                .child(
                    div()
                        .id(ElementId::NamedInteger(
                            "copy-activity-section".into(),
                            ((key.0 as u64) << 16 | key.1 as u64) << 8 | section as u64,
                        ))
                        .size(px(18.))
                        .rounded(px(4.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .hover(|style| style.bg(theme.overlay_strong))
                        .child(glyph(
                            if copied {
                                "icons/check.svg"
                            } else {
                                "icons/copy.svg"
                            },
                            10.,
                            if copied { theme.ok_green } else { theme.text_3 },
                        ))
                        .on_click(move |_, _, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(copy_content.clone()));
                            copied_sections
                                .borrow_mut()
                                .insert((key.0, key.1, section), Instant::now());
                            cx.refresh_windows();
                        }),
                ),
        )
        .child(
            div()
                .w_full()
                .min_w_0()
                .font_family("Menlo")
                .text_size(theme.code_px(10.5))
                .line_height(theme.code_px(15.))
                .text_color(theme.tool_meta)
                .whitespace_normal()
                .child(content),
        )
}

/// Arguments / result text: strings as-is, everything else pretty-printed.
fn display_value(value: &Value) -> String {
    let text = match value {
        Value::String(text) => text.clone(),
        other => serde_json::to_string_pretty(other).unwrap_or_else(|_| other.to_string()),
    };
    cap_chars(&text, DETAIL_TEXT_CAP)
}

fn cap_chars(text: &str, max: usize) -> String {
    if text.chars().count() > max {
        let mut out: String = text.chars().take(max).collect();
        out.push('…');
        out
    } else {
        text.to_string()
    }
}

/// Waku message footer: ghost copy button + `HH:MM` timestamp, revealed on
/// row hover (and pinned while the copy feedback is showing).
#[allow(clippy::too_many_arguments)]
fn render_message_footer(
    copy_text: String,
    ix: usize,
    finished_at: Option<i64>,
    revealed: bool,
    copied: bool,
    align_right: bool,
    theme: Theme,
    copied_at: Rc<RefCell<HashMap<usize, Instant>>>,
) -> impl IntoElement {
    let button = div()
        .id(ElementId::NamedInteger("copy-response".into(), ix as u64))
        .size(px(FOOTER_BUTTON_SIZE))
        .rounded(px(8.))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .hover(|style| style.bg(theme.overlay_strong))
        .child(glyph(
            if copied {
                "icons/check.svg"
            } else {
                "icons/copy.svg"
            },
            14.,
            if copied { theme.ok_green } else { theme.text_3 },
        ))
        .on_click(move |_, _, cx| {
            cx.write_to_clipboard(ClipboardItem::new_string(copy_text.clone()));
            copied_at.borrow_mut().insert(ix, Instant::now());
            cx.refresh_windows();
        });
    let time = finished_at.and_then(format_time).map(|time| {
        div()
            .h(px(FOOTER_BUTTON_SIZE))
            .px(px(4.))
            .flex()
            .items_center()
            .text_size(theme.ui_px(11.5))
            .line_height(theme.ui_px(16.))
            .text_color(theme.text_3)
            .child(time)
    });
    let mut footer = div()
        .h(px(FOOTER_BUTTON_SIZE))
        .flex()
        .items_center()
        .gap(px(1.))
        .opacity(if revealed || copied { 1. } else { 0. })
        .when(align_right, |row| row.justify_end());
    if align_right {
        // Waku's right-aligned footer: timestamp first, then actions.
        if let Some(time) = time {
            footer = footer.child(time);
        }
        footer = footer.child(button);
    } else {
        footer = footer.child(button);
        if let Some(time) = time {
            footer = footer.child(time);
        }
    }
    footer
}

/// Local `HH:MM` for an epoch-millis stamp; `None` when it cannot be resolved.
fn format_time(millis: i64) -> Option<String> {
    chrono::DateTime::from_timestamp_millis(millis)
        .map(|dt| dt.with_timezone(&chrono::Local).format("%H:%M").to_string())
}

fn glyph(path: &'static str, size: f32, color: Hsla) -> impl IntoElement {
    svg().path(path).size(px(size)).text_color(color)
}

fn pulse_dot(theme: Theme, elapsed_ms: u128) -> impl IntoElement {
    let on = elapsed_ms.is_multiple_of(400);
    div().size(px(5.)).rounded_full().bg(if on {
        theme.accent_bar
    } else {
        theme.accent_bar.opacity(0.35)
    })
}

fn fold_label(elapsed: Option<Duration>) -> String {
    match elapsed {
        Some(duration) => format!("Worked for {}", format_duration(duration)),
        None => "Worked".to_string(),
    }
}

/// Waku `formatDuration` spoken units: `5 minutes 41 seconds`.
fn format_duration(duration: Duration) -> String {
    let secs = duration.as_secs().max(1);
    if secs < 60 {
        return format!("{} {}", secs, plural_unit("second", secs));
    }
    if secs < 3_600 {
        let minutes = secs / 60;
        let remaining = secs % 60;
        let first = format!("{} {}", minutes, plural_unit("minute", minutes));
        return if remaining > 0 {
            format!(
                "{} {} {}",
                first,
                remaining,
                plural_unit("second", remaining)
            )
        } else {
            first
        };
    }
    let hours = secs / 3_600;
    let minutes = (secs % 3_600) / 60;
    let first = format!("{} {}", hours, plural_unit("hour", hours));
    if minutes > 0 {
        format!("{} {} {}", first, minutes, plural_unit("minute", minutes))
    } else {
        first
    }
}

fn plural_unit(word: &str, count: u64) -> String {
    if count == 1 {
        word.to_string()
    } else {
        format!("{word}s")
    }
}

fn render_turn_fold(
    ix: usize,
    expanded: bool,
    elapsed: Option<Duration>,
    theme: Theme,
    expanded_turns: Rc<RefCell<HashSet<usize>>>,
    scroller: MessageScrollerState,
) -> impl IntoElement {
    let label = fold_label(elapsed);
    div()
        .w_full()
        .h(px(24.))
        .flex()
        .items_center()
        .gap(px(10.))
        .child(div().h(px(1.)).flex_1().bg(theme.border))
        .child(
            div()
                .id(ElementId::NamedInteger("turn-fold".into(), ix as u64))
                .h(px(24.))
                .px(px(2.))
                .flex_none()
                .flex()
                .items_center()
                .gap(px(5.))
                .cursor_pointer()
                .text_size(theme.ui_px(13.5))
                .line_height(theme.ui_px(18.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(theme.text_3)
                .hover(|style| style.text_color(theme.text_2))
                .child(label)
                .child(glyph(
                    if expanded {
                        "icons/chevron-down.svg"
                    } else {
                        "icons/chevron-right.svg"
                    },
                    11.5,
                    theme.text_3,
                ))
                .on_click(move |_, _, cx| {
                    toggle_index(&expanded_turns, ix);
                    scroller.remeasure_items(ix..ix + 1);
                    cx.refresh_windows();
                }),
        )
        .child(div().h(px(1.)).flex_1().bg(theme.border))
}

fn working_wave_dots(theme: Theme, elapsed_ms: u128) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(px(3.))
        .children((0..3).map(move |i| {
            let phase = ((elapsed_ms / 180) + i as u128 * 2) % 6;
            let on = phase < 3;
            div()
                .size(px(4.))
                .rounded_full()
                .bg(if on { theme.accent_bar } else { theme.border })
        }))
}

fn render_working_indicator(elapsed: Duration, theme: Theme) -> impl IntoElement {
    div()
        .h(px(22.))
        .flex()
        .items_center()
        .gap(px(8.))
        .child(working_wave_dots(theme, elapsed.as_millis()))
        .child(
            div()
                .text_size(theme.ui_px(13.5))
                .line_height(theme.ui_px(18.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(theme.text_3)
                .child(format!("Working for {}", format_working_elapsed(elapsed))),
        )
}

/// Waku `formatWorkingElapsed` compact form: `12s` / `5m 41s` / `1h 2m`.
fn format_working_elapsed(duration: Duration) -> String {
    let secs = duration.as_secs();
    if secs < 60 {
        return format!("{secs}s");
    }
    if secs < 3_600 {
        let minutes = secs / 60;
        let remaining = secs % 60;
        return if remaining > 0 {
            format!("{minutes}m {remaining}s")
        } else {
            format!("{minutes}m")
        };
    }
    let hours = secs / 3_600;
    let minutes = (secs % 3_600) / 60;
    if minutes > 0 {
        format!("{hours}h {minutes}m")
    } else {
        format!("{hours}h")
    }
}

fn activity_preview(tool: &ToolCall) -> String {
    if let Some(path) = &tool.path {
        return path.clone();
    }
    if tool.summary.len() > 72 {
        let mut out: String = tool.summary.chars().take(72).collect();
        out.push('…');
        out
    } else {
        tool.summary.clone()
    }
}

fn render_line_delta(added: u64, removed: u64, theme: Theme, size: f32) -> impl IntoElement {
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(6.))
        .text_size(theme.ui_px(size))
        .line_height(theme.ui_px(size + 3.))
        .child(
            div()
                .flex_none()
                .text_color(theme.add_green)
                .child(format!("+{added}")),
        )
        .child(
            div()
                .flex_none()
                .text_color(theme.del_red)
                .child(format!("-{removed}")),
        )
}

// ── Markdown ────────────────────────────────────────────────────────────────
// Waku `.markdown` parity: 14px/22px body, 0.9rem rhythm between blocks,
// h1/h2/h3 at 20/18/16px semibold, `ml-5` lists with hanging indents,
// `border-l-2` blockquotes, mono inline-code chips, rounded pre blocks,
// and clickable underlined links.

/// Flattened inline runs: the body text, one [`TextRun`] per styled span,
/// and the link `(byte range, url)` pairs.
type InlineRuns = (SharedString, Vec<TextRun>, Vec<(Range<usize>, String)>);

/// One styled inline span produced by [`parse_inline`].
#[derive(Clone, Default)]
struct InlineSpan {
    text: String,
    bold: bool,
    italic: bool,
    strikethrough: bool,
    code: bool,
    link: Option<String>,
}

fn flush_span(spans: &mut Vec<InlineSpan>, current: &mut InlineSpan) {
    if !current.text.is_empty() {
        spans.push(std::mem::take(current));
    }
}

/// Parse GFM inline syntax: `` `code` ``, `**bold**`, `*italic*`, `_italic_`,
/// `~~strike~~`, `[links](url)`. `\*`-style escapes render literally.
fn parse_inline(text: &str) -> Vec<InlineSpan> {
    let mut spans: Vec<InlineSpan> = Vec::new();
    let mut current = InlineSpan::default();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let ch = chars[i];
        match ch {
            '\\' if i + 1 < chars.len() && "*_`~[]".contains(chars[i + 1]) => {
                current.text.push(chars[i + 1]);
                i += 2;
            }
            '`' => match (i + 1..chars.len()).find(|&j| chars[j] == '`') {
                Some(close) => {
                    flush_span(&mut spans, &mut current);
                    spans.push(InlineSpan {
                        text: chars[i + 1..close].iter().collect(),
                        code: true,
                        bold: current.bold,
                        ..InlineSpan::default()
                    });
                    i = close + 1;
                }
                None => {
                    current.text.push('`');
                    i += 1;
                }
            },
            '*' if i + 1 < chars.len() && chars[i + 1] == '*' => {
                flush_span(&mut spans, &mut current);
                current.bold = !current.bold;
                i += 2;
            }
            '_' if i + 1 < chars.len()
                && chars[i + 1] == '_'
                && (i == 0 || !chars[i - 1].is_alphanumeric()) =>
            {
                flush_span(&mut spans, &mut current);
                current.bold = !current.bold;
                i += 2;
            }
            '~' if i + 1 < chars.len() && chars[i + 1] == '~' => {
                flush_span(&mut spans, &mut current);
                current.strikethrough = !current.strikethrough;
                i += 2;
            }
            '*' => {
                // Italic: an opening `*` needs a non-space next char, a
                // closing one a non-space previous char.
                let opens = i + 1 < chars.len() && !chars[i + 1].is_whitespace();
                let closes = i > 0 && !chars[i - 1].is_whitespace();
                if (current.italic && closes) || (!current.italic && opens) {
                    flush_span(&mut spans, &mut current);
                    current.italic = !current.italic;
                } else {
                    current.text.push('*');
                }
                i += 1;
            }
            '_' if i == 0 || !chars[i - 1].is_alphanumeric() => {
                let opens = i + 1 < chars.len() && !chars[i + 1].is_whitespace();
                let closes = i > 0 && !chars[i - 1].is_whitespace();
                if (current.italic && closes) || (!current.italic && opens) {
                    flush_span(&mut spans, &mut current);
                    current.italic = !current.italic;
                } else {
                    current.text.push('_');
                }
                i += 1;
            }
            '[' => {
                // Try `[label](url)` — fall back to a literal `[`.
                let mut label_end = None;
                let mut j = i + 1;
                while j < chars.len() && chars[j] != '\n' {
                    if chars[j] == ']' && j + 1 < chars.len() && chars[j + 1] == '(' {
                        label_end = Some(j);
                        break;
                    }
                    j += 1;
                }
                let mut matched = false;
                if let Some(label_end) = label_end {
                    if let Some(close) = (label_end + 2..chars.len())
                        .take_while(|&k| chars[k] != '\n')
                        .find(|&k| chars[k] == ')')
                    {
                        flush_span(&mut spans, &mut current);
                        let mut span = current.clone();
                        span.text = chars[i + 1..label_end].iter().collect();
                        span.link = Some(chars[label_end + 2..close].iter().collect());
                        spans.push(span);
                        i = close + 1;
                        matched = true;
                    }
                }
                if !matched {
                    current.text.push('[');
                    i += 1;
                }
            }
            other => {
                current.text.push(other);
                i += 1;
            }
        }
    }
    flush_span(&mut spans, &mut current);
    spans
}

/// Build `(text, runs, links)` for a [`StyledText`] from inline spans. Each
/// link is a `(byte range, url)` pair into the flattened body.
fn inline_runs(
    spans: &[InlineSpan],
    base_weight: FontWeight,
    base_color: Hsla,
    theme: Theme,
) -> InlineRuns {
    let mut body = String::new();
    let mut runs: Vec<TextRun> = Vec::new();
    let mut links: Vec<(Range<usize>, String)> = Vec::new();
    for span in spans {
        if span.text.is_empty() {
            continue;
        }
        let start = body.len();
        body.push_str(&span.text);
        let len = body.len() - start;
        let mut font = ui_font();
        if span.code {
            font.family = "Menlo".into();
            font.weight = FontWeight::NORMAL;
            font.style = FontStyle::Normal;
        } else {
            font.weight = if span.bold {
                FontWeight::BOLD
            } else {
                base_weight
            };
            font.style = if span.italic {
                FontStyle::Italic
            } else {
                FontStyle::Normal
            };
        }
        runs.push(TextRun {
            len,
            font,
            color: if span.code {
                theme.inline_code_text
            } else {
                base_color
            },
            background_color: span.code.then_some(theme.inline_code_bg),
            underline: span.link.is_some().then(|| UnderlineStyle {
                thickness: px(1.),
                color: None,
                wavy: false,
            }),
            strikethrough: span.strikethrough.then(|| StrikethroughStyle {
                thickness: px(1.),
                color: None,
            }),
        });
        if let Some(url) = &span.link {
            links.push((start..body.len(), url.clone()));
        }
    }
    (body.into(), runs, links)
}

/// The window's default UI face, explicit for [`TextRun`] construction.
fn ui_font() -> Font {
    Font {
        family: ".SystemUIFont".into(),
        features: FontFeatures::default(),
        fallbacks: None,
        weight: FontWeight::NORMAL,
        style: FontStyle::Normal,
    }
}

/// A styled, word-wrapping paragraph. Links open via `cx.open_url`.
fn paragraph_text(
    text: &str,
    size: f32,
    line_height: f32,
    weight: FontWeight,
    color: Hsla,
    key: ElementId,
    theme: Theme,
) -> impl IntoElement {
    let spans = parse_inline(text);
    let (body, runs, links) = inline_runs(&spans, weight, color, theme);
    let ranges: Vec<Range<usize>> = links.iter().map(|(range, _)| range.clone()).collect();
    div()
        .w_full()
        .min_w_0()
        .text_size(theme.ui_px(size))
        .line_height(theme.ui_px(line_height))
        .text_color(color)
        .child(
            InteractiveText::new(key, StyledText::new(body).with_runs(runs)).on_click(
                ranges,
                move |range_ix: usize, _, cx| {
                    if let Some((_, url)) = links.get(range_ix) {
                        cx.open_url(url);
                    }
                },
            ),
        )
}

fn md_id(ix: usize, salt: u64, block_ix: usize, sub: usize) -> ElementId {
    ElementId::NamedInteger(
        "md".into(),
        ((ix as u64) << 40) | ((salt & 0xffff) << 24) | ((block_ix as u64) << 8) | sub as u64,
    )
}

// ── block model ─────────────────────────────────────────────────────────────

enum Block {
    Paragraph(Vec<String>),
    Heading(u8, String),
    Code(Vec<String>),
    List(Vec<ListItem>),
    Quote(Vec<String>),
    Rule,
    Table {
        header: Vec<String>,
        rows: Vec<Vec<String>>,
    },
}

struct ListItem {
    depth: usize,
    ordered: bool,
    number: u64,
    text: String,
}

fn flush_paragraph(paragraph: &mut Vec<String>, blocks: &mut Vec<Block>) {
    if !paragraph.is_empty() {
        blocks.push(Block::Paragraph(std::mem::take(paragraph)));
    }
}

fn heading_line(trimmed: &str) -> Option<(u8, String)> {
    let level = trimmed.chars().take_while(|&c| c == '#').count();
    if (1..=6).contains(&level) {
        if let Some(body) = trimmed[level..].strip_prefix(' ') {
            let body = body.trim();
            if !body.is_empty() {
                // Waku styles h1-h3; deeper levels fall back to body text.
                return Some((level.min(3) as u8, body.to_string()));
            }
        }
    }
    None
}

fn is_rule(trimmed: &str) -> bool {
    let compact: String = trimmed.chars().filter(|c| !c.is_whitespace()).collect();
    match compact.chars().next() {
        Some(first @ ('-' | '*' | '_')) => {
            compact.len() >= 3 && compact.chars().all(|c| c == first)
        }
        _ => false,
    }
}

fn list_item_line(line: &str) -> Option<(usize, bool, u64, String)> {
    let indent = line.len() - line.trim_start().len();
    let t = line.trim_start();
    let (ordered, number, body) = if let Some(rest) = t
        .strip_prefix("- ")
        .or_else(|| t.strip_prefix("* "))
        .or_else(|| t.strip_prefix("+ "))
    {
        (false, 0u64, rest)
    } else {
        let digits = t.chars().take_while(|c| c.is_ascii_digit()).count();
        if digits == 0 {
            return None;
        }
        let after = t[digits..]
            .strip_prefix(". ")
            .or_else(|| t[digits..].strip_prefix(") "))?;
        (true, t[..digits].parse().ok()?, after)
    };
    Some((indent.div_ceil(2).min(2), ordered, number, body.to_string()))
}

fn split_table_row(line: &str) -> Vec<String> {
    let s = line.trim();
    let s = s.strip_prefix('|').unwrap_or(s);
    let s = s.strip_suffix('|').unwrap_or(s);
    s.split('|').map(|cell| cell.trim().to_string()).collect()
}

fn is_table_separator(trimmed: &str) -> bool {
    trimmed.contains('-')
        && trimmed.contains('|')
        && trimmed.chars().all(|c| matches!(c, '|' | '-' | ':' | ' '))
}

fn parse_blocks(text: &str) -> Vec<Block> {
    let mut blocks: Vec<Block> = Vec::new();
    let mut paragraph: Vec<String> = Vec::new();
    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim_start();

        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            flush_paragraph(&mut paragraph, &mut blocks);
            let marker = &trimmed[..trimmed.len().min(3)];
            let mut body = Vec::new();
            i += 1;
            while i < lines.len() && !lines[i].trim_start().starts_with(marker) {
                body.push(lines[i].to_string());
                i += 1;
            }
            i += 1; // closing fence (or EOF)
            blocks.push(Block::Code(body));
            continue;
        }

        if trimmed.is_empty() {
            flush_paragraph(&mut paragraph, &mut blocks);
            i += 1;
            continue;
        }

        if let Some((level, body)) = heading_line(trimmed) {
            flush_paragraph(&mut paragraph, &mut blocks);
            blocks.push(Block::Heading(level, body));
            i += 1;
            continue;
        }

        if is_rule(trimmed) {
            flush_paragraph(&mut paragraph, &mut blocks);
            blocks.push(Block::Rule);
            i += 1;
            continue;
        }

        if trimmed.contains('|') && i + 1 < lines.len() && is_table_separator(lines[i + 1].trim()) {
            flush_paragraph(&mut paragraph, &mut blocks);
            let header = split_table_row(trimmed);
            i += 2;
            let mut rows = Vec::new();
            while i < lines.len() {
                let row_line = lines[i].trim();
                if row_line.is_empty() || !row_line.contains('|') {
                    break;
                }
                rows.push(split_table_row(row_line));
                i += 1;
            }
            blocks.push(Block::Table { header, rows });
            continue;
        }

        if trimmed.starts_with('>') {
            flush_paragraph(&mut paragraph, &mut blocks);
            let mut body = Vec::new();
            while i < lines.len() {
                match lines[i].trim_start().strip_prefix('>') {
                    Some(rest) => {
                        body.push(rest.strip_prefix(' ').unwrap_or(rest).to_string());
                        i += 1;
                    }
                    None => break,
                }
            }
            blocks.push(Block::Quote(body));
            continue;
        }

        if let Some((depth, ordered, number, body)) = list_item_line(line) {
            flush_paragraph(&mut paragraph, &mut blocks);
            let mut items = Vec::new();
            let mut pending = ListItem {
                depth,
                ordered,
                number,
                text: body,
            };
            i += 1;
            while i < lines.len() {
                let l = lines[i];
                if l.trim().is_empty() {
                    // A blank line keeps the list open only when another
                    // item follows (loose list).
                    let mut look = i + 1;
                    while look < lines.len() && lines[look].trim().is_empty() {
                        look += 1;
                    }
                    if look < lines.len() && list_item_line(lines[look]).is_some() {
                        i = look;
                        continue;
                    }
                    break;
                }
                if let Some((depth, ordered, number, body)) = list_item_line(l) {
                    // A marker-type change starts a new list.
                    if ordered != pending.ordered {
                        break;
                    }
                    items.push(std::mem::replace(
                        &mut pending,
                        ListItem {
                            depth,
                            ordered,
                            number,
                            text: body,
                        },
                    ));
                    i += 1;
                } else if l.starts_with("  ") || l.starts_with('\t') {
                    // Indented continuation of the pending item.
                    pending.text.push(' ');
                    pending.text.push_str(l.trim());
                    i += 1;
                } else {
                    break;
                }
            }
            items.push(pending);
            blocks.push(Block::List(items));
            continue;
        }

        paragraph.push(line.trim().to_string());
        i += 1;
    }
    flush_paragraph(&mut paragraph, &mut blocks);
    blocks
}

/// Waku `.markdown`: 14px/22px body with a 0.9rem rhythm between blocks.
fn render_prose(
    text: &str,
    ix: usize,
    salt: u64,
    theme: Theme,
    copied_sections: CopiedSections,
) -> impl IntoElement + use<> {
    let blocks = parse_blocks(text);
    div()
        .w_full()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(px(14.))
        .children(
            blocks
                .into_iter()
                .enumerate()
                .map(move |(block_ix, block)| {
                    render_block(block, ix, salt, block_ix, theme, copied_sections.clone())
                }),
        )
}

fn render_block(
    block: Block,
    ix: usize,
    salt: u64,
    block_ix: usize,
    theme: Theme,
    copied_sections: CopiedSections,
) -> AnyElement {
    match block {
        Block::Paragraph(lines) => paragraph_text(
            &lines.join(" "),
            14.,
            22.,
            FontWeight::NORMAL,
            theme.assistant_text,
            md_id(ix, salt, block_ix, 0),
            theme,
        )
        .into_any_element(),
        Block::Heading(level, text) => {
            let (size, line_height) = match level {
                1 => (20., 28.),
                2 => (18., 28.),
                _ => (16., 24.),
            };
            paragraph_text(
                &text,
                size,
                line_height,
                FontWeight::SEMIBOLD,
                theme.text,
                md_id(ix, salt, block_ix, 0),
                theme,
            )
            .into_any_element()
        }
        Block::Code(lines) => {
            render_code_block(&lines, ix, block_ix, theme, copied_sections).into_any_element()
        }
        Block::Rule => div().w_full().h(px(1.)).bg(theme.border).into_any_element(),
        Block::Quote(lines) => div()
            .w_full()
            .min_w_0()
            .border_l_2()
            .border_color(theme.border_strong)
            .pl(px(16.))
            .flex()
            .flex_col()
            .gap(px(4.))
            .children(lines.into_iter().enumerate().map(move |(sub, line)| {
                paragraph_text(
                    &line,
                    14.,
                    22.,
                    FontWeight::NORMAL,
                    theme.text_2,
                    md_id(ix, salt, block_ix, sub),
                    theme,
                )
            }))
            .into_any_element(),
        Block::List(items) => div()
            .w_full()
            .min_w_0()
            .flex()
            .flex_col()
            .gap(px(4.))
            .children(
                items
                    .into_iter()
                    .enumerate()
                    .map(move |(sub, item)| render_list_item(item, ix, salt, block_ix, sub, theme)),
            )
            .into_any_element(),
        Block::Table { header, rows } => {
            render_table(&header, &rows, ix, salt, block_ix, theme).into_any_element()
        }
    }
}

/// `ml-5` bullet / numbered item with a hanging indent on wrap.
fn render_list_item(
    item: ListItem,
    ix: usize,
    salt: u64,
    block_ix: usize,
    sub: usize,
    theme: Theme,
) -> AnyElement {
    let marker = if item.ordered {
        format!("{}.", item.number)
    } else {
        "•".to_string()
    };
    div()
        .w_full()
        .min_w_0()
        .flex()
        .items_start()
        .pl(px(20. + 20. * item.depth as f32))
        .child(
            div()
                .flex_none()
                .w(px(20.))
                .text_size(theme.ui_px(14.))
                .line_height(theme.ui_px(22.))
                .text_color(if item.ordered {
                    theme.text
                } else {
                    theme.text_3
                })
                .child(marker),
        )
        .child(div().min_w_0().flex_1().child(paragraph_text(
            &item.text,
            14.,
            22.,
            FontWeight::NORMAL,
            theme.assistant_text,
            md_id(ix, salt, block_ix, sub),
            theme,
        )))
        .into_any_element()
}

/// Waku `pre`: rounded-xl bordered mono block, 13px/24px, p-4 — plus a
/// ghost copy button pinned to the top-right (feedback via the shared
/// detail-section map).
fn render_code_block(
    lines: &[String],
    ix: usize,
    block_ix: usize,
    theme: Theme,
    copied_sections: CopiedSections,
) -> impl IntoElement {
    let code = lines.join("\n");
    let copied = copied_sections
        .borrow()
        .get(&(ix, block_ix, CODE_COPY_SECTION))
        .is_some_and(|at| at.elapsed() < COPY_FEEDBACK);
    let copy_button = div()
        .id(ElementId::NamedInteger(
            "copy-code".into(),
            (ix as u64) << 16 | block_ix as u64,
        ))
        .absolute()
        .top(px(6.))
        .right(px(6.))
        .size(px(CODE_COPY_BUTTON))
        .rounded(px(6.))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .bg(theme.bg_main.opacity(0.6))
        .hover(|style| style.bg(theme.overlay_strong))
        .child(glyph(
            if copied {
                "icons/check.svg"
            } else {
                "icons/copy.svg"
            },
            12.,
            if copied { theme.ok_green } else { theme.text_3 },
        ))
        .on_click(move |_, _, cx| {
            cx.write_to_clipboard(ClipboardItem::new_string(code.clone()));
            copied_sections
                .borrow_mut()
                .insert((ix, block_ix, CODE_COPY_SECTION), Instant::now());
            cx.refresh_windows();
        });
    div()
        .relative()
        .w_full()
        .min_w_0()
        .child(
            div()
                .w_full()
                .min_w_0()
                .rounded(px(12.))
                .border_1()
                .border_color(theme.border)
                .bg(theme.code_bg)
                .px(px(16.))
                .py(px(12.))
                .overflow_hidden()
                .font_family("Menlo")
                .text_size(theme.code_px(13.))
                .line_height(theme.code_px(24.))
                .text_color(theme.code_text)
                .children(lines.iter().enumerate().map(|(line_ix, line)| {
                    // Clear the top-right copy button on the first line only.
                    let clear = if line_ix == 0 { px(30.) } else { px(0.) };
                    div().pr(clear).child(if line.is_empty() {
                        " ".to_string()
                    } else {
                        line.clone()
                    })
                })),
        )
        .child(copy_button)
}

/// GFM table: outer border, semibold header, dividers, content-weighted
/// columns (`th`/`td` styling from Waku's `.markdown table` rules).
fn render_table(
    header: &[String],
    rows: &[Vec<String>],
    ix: usize,
    salt: u64,
    block_ix: usize,
    theme: Theme,
) -> AnyElement {
    let columns = header
        .len()
        .max(rows.iter().map(Vec::len).max().unwrap_or(0));
    // Weight each column by its longest cell (bounded) so wide columns win.
    let mut weights = vec![1usize; columns];
    for (col, weight) in weights.iter_mut().enumerate() {
        let mut longest = 1usize;
        if let Some(cell) = header.get(col) {
            longest = longest.max(cell.chars().count());
        }
        for row in rows {
            if let Some(cell) = row.get(col) {
                longest = longest.max(cell.chars().count());
            }
        }
        *weight = longest.clamp(1, 60);
    }
    let total = weights.iter().sum::<usize>().max(1) as f32;
    let make_cell =
        |text: &str, weight: usize, strong: bool, sub: usize, salt: u64, theme: Theme| {
            div()
                .flex_basis(relative(weight as f32 / total))
                .flex_grow()
                .min_w_0()
                .px(px(12.))
                .py(px(8.))
                .child(paragraph_text(
                    text,
                    13.,
                    20.,
                    if strong {
                        FontWeight::SEMIBOLD
                    } else {
                        FontWeight::NORMAL
                    },
                    theme.assistant_text,
                    md_id(ix, salt, block_ix, sub),
                    theme,
                ))
        };
    let mut table = div()
        .w_full()
        .min_w_0()
        .overflow_hidden()
        .rounded(px(12.))
        .border_1()
        .border_color(theme.border);
    table = table.child(
        div().w_full().min_w_0().flex().bg(theme.overlay).children(
            header
                .iter()
                .enumerate()
                .map(|(col, text)| make_cell(text, weights[col], true, col, salt, theme)),
        ),
    );
    for (row_ix, row) in rows.iter().enumerate() {
        let weights = weights.clone();
        table = table.child(
            div()
                .w_full()
                .min_w_0()
                .flex()
                .border_t_1()
                .border_color(theme.border)
                .children(row.iter().enumerate().map(move |(col, text)| {
                    make_cell(
                        text,
                        weights[col],
                        false,
                        64 + (row_ix * 64) + col,
                        salt,
                        theme,
                    )
                })),
        );
    }
    table.into_any_element()
}
/// Waku-style footer stamp for the changed-files summary: `Today 1:15 PM`,
/// `Yesterday 6:07 PM`, then a short date (`Sep 6`) once past yesterday.
fn summary_time_label(millis: i64) -> String {
    summary_time_label_at(millis, chrono::Local::now())
}

fn summary_time_label_at(millis: i64, now: chrono::DateTime<chrono::Local>) -> String {
    use chrono::{DateTime, Local, TimeZone};
    let Some(utc) = DateTime::from_timestamp_millis(millis) else {
        return String::new();
    };
    let dt: DateTime<Local> = Local.from_utc_datetime(&utc.naive_local());
    let days = (now.date_naive() - dt.date_naive()).num_days();
    let clock = dt.format("%-I:%M %p");
    match days {
        0 => format!("Today {clock}"),
        1 => format!("Yesterday {clock}"),
        _ => dt.format("%b %-d").to_string(),
    }
}

fn changed_files_title(count: usize) -> String {
    if count == 1 {
        "Changed 1 file".to_string()
    } else {
        format!("Changed {count} files")
    }
}

/// Waku `ChangedFilesCard`: raised tile, "Changed N files" with a ±delta
/// underneath, a Review affordance, and roomy file rows with right-aligned
/// line counts. Shows 3 rows; expanded shows up to 12 with a clip note.
fn render_changed_files(
    files: &[(String, u64, u64)],
    theme: Theme,
    message_ix: usize,
    workspace: Option<&Path>,
    expanded: bool,
    expanded_files: Rc<RefCell<HashSet<usize>>>,
    scroller: MessageScrollerState,
) -> impl IntoElement {
    const EXPANDED_PREVIEW_LIMIT: usize = 12;
    let additions: u64 = files.iter().map(|(_, added, _)| *added).sum();
    let deletions: u64 = files.iter().map(|(_, _, removed)| *removed).sum();
    let title = changed_files_title(files.len());
    let can_expand = files.len() > CHANGED_FILES_PREVIEW_LIMIT;
    let visible = if expanded {
        files.len().min(EXPANDED_PREVIEW_LIMIT)
    } else {
        files.len().min(CHANGED_FILES_PREVIEW_LIMIT)
    };

    let mut rows = div()
        .w_full()
        .min_w_0()
        .flex()
        .flex_col()
        .border_t_1()
        .border_color(theme.tool_border);
    for (path, added, removed) in files.iter().take(visible) {
        rows = rows.child(
            div()
                .h(px(31.))
                .px(px(12.))
                .flex()
                .items_center()
                .gap(px(8.))
                .child(
                    div()
                        .min_w_0()
                        .flex_1()
                        .truncate()
                        .text_size(theme.ui_px(11.5))
                        .line_height(theme.ui_px(16.))
                        .text_color(theme.text_2)
                        .child(path.clone()),
                )
                .child(
                    div()
                        .flex_none()
                        .text_size(theme.ui_px(10.5))
                        .text_color(theme.add_green)
                        .child(format!("+{added}")),
                )
                .child(
                    div()
                        .flex_none()
                        .text_size(theme.ui_px(10.5))
                        .text_color(theme.del_red)
                        .child(format!("-{removed}")),
                ),
        );
    }

    // Review affordance: pi's RPC exposes no diff, so build one from the
    // workspace's git repo and open it in the default text editor.
    let review = workspace.map(|workspace| {
        let workspace = workspace.to_path_buf();
        let files = files.to_vec();
        div()
            .id(ElementId::NamedInteger(
                "review-changes".into(),
                message_ix as u64,
            ))
            .h(px(28.))
            .px(px(10.))
            .rounded(px(7.))
            .border_1()
            .border_color(theme.border)
            .bg(theme.bg_composer)
            .flex()
            .items_center()
            .gap(px(4.))
            .cursor_pointer()
            .text_size(theme.ui_px(11.5))
            .font_weight(FontWeight::MEDIUM)
            .text_color(theme.text_2)
            .hover(|style| style.bg(theme.bg_raised))
            .child(glyph("icons/file-diff.svg", 12., theme.text_3))
            .child("Review")
            .on_click(move |_, _, _| open_review_diff(&workspace, &files))
    });

    let mut header = div()
        .min_h(px(58.))
        .px(px(12.))
        .py(px(9.))
        .flex()
        .items_center()
        .gap(px(10.))
        .child(
            div()
                .size(px(36.))
                .flex_none()
                .rounded(px(9.))
                .bg(theme.bg_raised)
                .flex()
                .items_center()
                .justify_center()
                .child(glyph("icons/file-diff.svg", 16., theme.text_3)),
        )
        .child(
            div()
                .min_w_0()
                .flex_1()
                .flex()
                .flex_col()
                .child(
                    div()
                        .truncate()
                        .text_size(theme.ui_px(12.5))
                        .line_height(theme.ui_px(16.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.text)
                        .child(title),
                )
                .child(
                    div()
                        .mt(px(2.))
                        .flex()
                        .gap(px(6.))
                        .text_size(theme.ui_px(11.))
                        .line_height(theme.ui_px(14.))
                        .child(
                            div()
                                .text_color(theme.add_green)
                                .child(format!("+{additions}")),
                        )
                        .child(
                            div()
                                .text_color(theme.del_red)
                                .child(format!("-{deletions}")),
                        ),
                ),
        );
    if let Some(review) = review {
        header = header.child(review);
    }

    let mut card = div()
        .w_full()
        .min_w_0()
        .rounded(px(12.))
        .border_1()
        .border_color(theme.border)
        .bg(theme.overlay)
        .overflow_hidden()
        .child(header)
        .child(rows);

    if can_expand {
        let remaining = files.len() - CHANGED_FILES_PREVIEW_LIMIT;
        let label = if expanded {
            "Show fewer files".to_string()
        } else if remaining == 1 {
            "Show 1 more file".to_string()
        } else {
            format!("Show {remaining} more files")
        };
        let clipped = expanded && files.len() > EXPANDED_PREVIEW_LIMIT;
        let mut toggle = div()
            .id(ElementId::NamedInteger(
                "changed-files-toggle".into(),
                message_ix as u64,
            ))
            .h(px(34.))
            .px(px(12.))
            .border_t_1()
            .border_color(theme.border)
            .flex()
            .items_center()
            .gap(px(6.))
            .cursor_pointer()
            .text_size(theme.ui_px(11.5))
            .font_weight(FontWeight::MEDIUM)
            .text_color(theme.text_2)
            .hover(|style| style.bg(theme.overlay_strong).text_color(theme.text))
            .child(label);
        if clipped {
            toggle = toggle.child(
                div()
                    .min_w_0()
                    .flex_1()
                    .truncate()
                    .font_weight(FontWeight::NORMAL)
                    .text_size(theme.ui_px(11.5))
                    .text_color(theme.text_3)
                    .child(format!(
                        "Showing first {EXPANDED_PREVIEW_LIMIT} of {}",
                        files.len()
                    )),
            );
        }
        card = card.child(
            toggle
                .child(div().flex_1())
                .child(glyph(
                    if expanded {
                        "icons/chevron-down.svg"
                    } else {
                        "icons/chevron-right.svg"
                    },
                    11.,
                    theme.text_3,
                ))
                .on_click(move |_, _, cx| {
                    toggle_index(&expanded_files, message_ix);
                    scroller.remeasure_items(message_ix..message_ix + 1);
                    cx.refresh_windows();
                }),
        );
    }

    card
}

/// Review action: `git diff HEAD` over the task's changed files, opened in
/// the default text editor. Workspaces without an uncommitted diff still
/// get the per-file change list the transcript tracked.
fn open_review_diff(workspace: &Path, files: &[(String, u64, u64)]) {
    let mut command = std::process::Command::new("git");
    command
        .current_dir(workspace)
        .arg("diff")
        .arg("HEAD")
        .arg("--");
    for (path, _, _) in files {
        command.arg(path);
    }
    let text = match command.output().ok().filter(|out| out.status.success()) {
        Some(out) if !String::from_utf8_lossy(&out.stdout).trim().is_empty() => {
            String::from_utf8_lossy(&out.stdout).into_owned()
        }
        _ => {
            let mut fallback = String::from("Changes in this task (no uncommitted git diff):\n\n");
            for (path, added, removed) in files {
                fallback.push_str(&format!("{path}  +{added} -{removed}\n"));
            }
            fallback
        }
    };
    let path = std::env::temp_dir().join("orbit-review.diff");
    if std::fs::write(&path, text).is_ok() {
        let _ = std::process::Command::new("open")
            .arg("-t")
            .arg(&path)
            .spawn();
    }
}

fn toggle_index(set: &Rc<RefCell<HashSet<usize>>>, ix: usize) {
    let mut set = set.borrow_mut();
    if !set.remove(&ix) {
        set.insert(ix);
    }
}

fn toggle_activity_cluster(map: &Rc<RefCell<HashMap<usize, bool>>>, ix: usize, live: bool) {
    let mut map = map.borrow_mut();
    let next = !map.get(&ix).copied().unwrap_or(live);
    map.insert(ix, next);
}

pub(crate) fn activity_header_title(tool_count: usize, live: bool) -> String {
    if live {
        "Working".to_string()
    } else if tool_count == 1 {
        "Used 1 tool".to_string()
    } else {
        format!("Used {tool_count} tools")
    }
}

pub(crate) fn activity_icon(name: &str) -> &'static str {
    match name {
        "edit" | "write" => "icons/file-diff.svg",
        "read" | "grep" | "find" | "glob" | "search" => "icons/search.svg",
        "bash" | "shell" => "icons/spark.svg",
        _ => "icons/task.svg",
    }
}

fn activity_action_label(name: &str) -> String {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => "Tool".to_string(),
    }
}

pub(crate) fn active_user_index(
    messages: &[ChatMessage],
    streaming: Option<usize>,
    viewport_hint: Option<usize>,
) -> Option<usize> {
    if messages.is_empty() {
        return None;
    }
    // The reader scrolled away from the live edge: the active tick is the
    // turn the viewport is reading — the newest user turn at or above the
    // first visible row — not the latest turn in the transcript.
    if streaming.is_none() {
        if let Some(first_visible) = viewport_hint {
            let clamped = first_visible.min(messages.len() - 1);
            if let Some(ix) = messages[..clamped + 1]
                .iter()
                .enumerate()
                .rev()
                .find(|(_, message)| message.user)
                .map(|(ix, _)| ix)
            {
                return Some(ix);
            }
        }
    }
    let end = streaming
        .unwrap_or(messages.len() - 1)
        .min(messages.len() - 1);
    messages[..end + 1]
        .iter()
        .enumerate()
        .rev()
        .find(|(_, message)| message.user)
        .map(|(ix, _)| ix)
}

fn starts_followup_turn(messages: &[ChatMessage], ix: usize) -> bool {
    ix > 0 && messages[ix].user && !messages[ix - 1].user
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fold_label_falls_back_without_elapsed() {
        assert_eq!(fold_label(None), "Worked");
        assert_eq!(
            fold_label(Some(Duration::from_secs(341))),
            "Worked for 5 minutes 41 seconds"
        );
    }

    #[test]
    fn duration_format_matches_waku_spoken_forms() {
        assert_eq!(format_duration(Duration::from_secs(1)), "1 second");
        assert_eq!(format_duration(Duration::from_secs(12)), "12 seconds");
        assert_eq!(format_duration(Duration::from_secs(60)), "1 minute");
        assert_eq!(
            format_duration(Duration::from_secs(100)),
            "1 minute 40 seconds"
        );
        assert_eq!(
            format_duration(Duration::from_secs(341)),
            "5 minutes 41 seconds"
        );
        assert_eq!(format_duration(Duration::from_secs(3_600)), "1 hour");
        assert_eq!(
            format_duration(Duration::from_secs(3_721)),
            "1 hour 2 minutes"
        );
    }

    #[test]
    fn working_elapsed_uses_waku_short_form() {
        assert_eq!(format_working_elapsed(Duration::from_secs(0)), "0s");
        assert_eq!(format_working_elapsed(Duration::from_secs(12)), "12s");
        assert_eq!(format_working_elapsed(Duration::from_secs(60)), "1m");
        assert_eq!(format_working_elapsed(Duration::from_secs(341)), "5m 41s");
        assert_eq!(format_working_elapsed(Duration::from_secs(3_600)), "1h");
        assert_eq!(format_working_elapsed(Duration::from_secs(3_721)), "1h 2m");
    }

    #[test]
    fn changed_files_title_uses_screenshot_copy() {
        assert_eq!(changed_files_title(1), "Changed 1 file");
        assert_eq!(changed_files_title(2), "Changed 2 files");
    }

    #[test]
    fn summary_time_label_matches_screenshot_copy() {
        use chrono::{Duration, TimeZone, Timelike, Utc};
        let at = |y: i32, m: u32, d: u32, h: u32, min: u32| {
            Utc.with_ymd_and_hms(y, m, d, h, min, 0)
                .single()
                .unwrap()
                .with_timezone(&chrono::Local)
        };
        // Bucket boundaries are relative to `now`, so the test is
        // timezone-independent.
        let now = at(2026, 9, 8, 16, 0);
        let millis = |dt: chrono::DateTime<chrono::Local>| dt.timestamp_millis();
        // 18:07 local yesterday — the screenshot's "Yesterday 6:07 PM".
        let yesterday = (now - Duration::days(1))
            .with_hour(18)
            .unwrap()
            .with_minute(7)
            .unwrap();
        assert_eq!(
            summary_time_label_at(millis(yesterday), now),
            "Yesterday 6:07 PM"
        );
        // Same day → Today + 12-hour clock, no leading zero on the hour.
        let today = now.with_hour(13).unwrap().with_minute(5).unwrap();
        assert_eq!(summary_time_label_at(millis(today), now), "Today 1:05 PM");
        // Older than yesterday falls back to a short date.
        let older = now - Duration::days(3);
        assert_eq!(
            summary_time_label_at(millis(older), now),
            older.format("%b %-d").to_string()
        );
    }

    #[test]
    fn activity_header_matches_waku_copy() {
        assert_eq!(activity_header_title(3, true), "Working");
        assert_eq!(activity_header_title(1, false), "Used 1 tool");
        assert_eq!(activity_header_title(4, false), "Used 4 tools");
    }

    #[test]
    fn activity_icon_maps_pi_tools() {
        assert_eq!(activity_icon("edit"), "icons/file-diff.svg");
        assert_eq!(activity_icon("bash"), "icons/spark.svg");
        assert_eq!(activity_icon("grep"), "icons/search.svg");
        assert_eq!(activity_icon("mcp"), "icons/task.svg");
    }

    #[test]
    fn active_user_index_tracks_streaming_turn() {
        let messages = vec![
            ChatMessage {
                user: true,
                steps: vec![Step {
                    text: "a".into(),
                    ..Step::default()
                }],
                elapsed: None,
                images: Vec::new(),
                finished_at: None,
            },
            ChatMessage {
                user: false,
                steps: vec![Step {
                    text: "b".into(),
                    ..Step::default()
                }],
                elapsed: None,
                images: Vec::new(),
                finished_at: None,
            },
            ChatMessage {
                user: true,
                steps: vec![Step {
                    text: "c".into(),
                    ..Step::default()
                }],
                elapsed: None,
                images: Vec::new(),
                finished_at: None,
            },
            ChatMessage {
                user: false,
                steps: vec![Step {
                    text: "d".into(),
                    ..Step::default()
                }],
                elapsed: None,
                images: Vec::new(),
                finished_at: None,
            },
        ];
        assert_eq!(active_user_index(&messages, Some(1), None), Some(0));
        assert_eq!(active_user_index(&messages, Some(3), None), Some(2));
        assert_eq!(active_user_index(&messages, None, None), Some(2));
    }

    #[test]
    fn active_user_index_follows_viewport_when_scrolled() {
        let messages = vec![
            ChatMessage {
                user: true,
                steps: vec![Step {
                    text: "a".into(),
                    ..Step::default()
                }],
                elapsed: None,
                images: Vec::new(),
                finished_at: None,
            },
            ChatMessage {
                user: false,
                steps: vec![Step {
                    text: "b".into(),
                    ..Step::default()
                }],
                elapsed: None,
                images: Vec::new(),
                finished_at: None,
            },
            ChatMessage {
                user: true,
                steps: vec![Step {
                    text: "c".into(),
                    ..Step::default()
                }],
                elapsed: None,
                images: Vec::new(),
                finished_at: None,
            },
            ChatMessage {
                user: false,
                steps: vec![Step {
                    text: "d".into(),
                    ..Step::default()
                }],
                elapsed: None,
                images: Vec::new(),
                finished_at: None,
            },
        ];
        // Reader scrolled back to the first turn: the tick for turn 0 is
        // active even though the newest turn is turn 2.
        assert_eq!(active_user_index(&messages, None, Some(0)), Some(0));
        // Still reading within the first turn's run (row 1).
        assert_eq!(active_user_index(&messages, None, Some(1)), Some(0));
        // Reached the second user turn: its tick takes over.
        assert_eq!(active_user_index(&messages, None, Some(2)), Some(2));
        assert_eq!(active_user_index(&messages, None, Some(3)), Some(2));
        // Out-of-range hint clamps to the last row.
        assert_eq!(active_user_index(&messages, None, Some(99)), Some(2));
    }

    #[test]
    fn parse_inline_extracts_bold_code_and_links() {
        let spans = parse_inline("plain **bold** and `code` and [x](https://a.b)");
        assert_eq!(spans.len(), 6);
        assert_eq!(spans[0].text, "plain ");
        assert!(spans[1].bold);
        assert_eq!(spans[1].text, "bold");
        assert!(spans[3].code);
        assert_eq!(spans[3].text, "code");
        assert_eq!(spans[5].link.as_deref(), Some("https://a.b"));
        assert_eq!(spans[5].text, "x");
    }

    #[test]
    fn parse_inline_keeps_unmatched_markers_literal() {
        let spans = parse_inline("a * b ` c [d");
        let joined: String = spans.iter().map(|s| s.text.as_str()).collect();
        assert_eq!(joined, "a * b ` c [d");
        assert!(spans.iter().all(|s| !s.bold && !s.code && s.link.is_none()));
    }

    #[test]
    fn parse_inline_escapes_render_literally() {
        let spans = parse_inline(r"\*not bold\*");
        let joined: String = spans.iter().map(|s| s.text.as_str()).collect();
        assert_eq!(joined, "*not bold*");
        assert!(spans.iter().all(|s| !s.bold));
    }

    #[test]
    fn parse_blocks_splits_paragraphs_headings_and_fences() {
        let blocks = parse_blocks("one\ntwo\n\n## Title\n\n```rs\nlet a = 1;\n```");
        assert_eq!(blocks.len(), 3);
        assert!(matches!(&blocks[0], Block::Paragraph(lines) if lines.join(" ") == "one two"));
        assert!(matches!(&blocks[1], Block::Heading(2, title) if title == "Title"));
        assert!(matches!(&blocks[2], Block::Code(lines) if lines == &["let a = 1;".to_string()]));
    }

    #[test]
    fn parse_blocks_collects_loose_and_nested_lists() {
        let blocks = parse_blocks("- a\n- b\n\n  - nested\n\n3. third\n4. fourth");
        assert_eq!(blocks.len(), 2);
        let Block::List(items) = &blocks[0] else {
            panic!("expected list");
        };
        assert_eq!(items.len(), 3);
        assert!(!items[0].ordered);
        assert_eq!(items[2].depth, 1);
        assert_eq!(items[2].text, "nested");
        let Block::List(ordered) = &blocks[1] else {
            panic!("expected ordered list");
        };
        assert!(ordered[0].ordered);
        assert_eq!(ordered[0].number, 3);
    }

    #[test]
    fn parse_blocks_handles_rules_quotes_and_tables() {
        let blocks =
            parse_blocks("---\n\n> quoted\n> lines\n\n| a | b |\n| --- | --- |\n| 1 | 2 |");
        assert!(matches!(blocks[0], Block::Rule));
        assert!(matches!(&blocks[1], Block::Quote(lines) if lines.len() == 2));
        let Block::Table { header, rows } = &blocks[2] else {
            panic!("expected table");
        };
        assert_eq!(header, &["a", "b"]);
        assert_eq!(rows, &[vec!["1".to_string(), "2".to_string()]]);
    }

    #[test]
    fn inline_runs_pairs_link_ranges_with_urls() {
        let theme = Theme::dark();
        let spans = parse_inline("see [docs](https://pi.dev) now");
        let (body, runs, links) = inline_runs(&spans, FontWeight::NORMAL, theme.text, theme);
        assert_eq!(body.as_ref(), "see docs now");
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].1, "https://pi.dev");
        assert_eq!(&body[links[0].0.clone()], "docs");
        // Underline style rides on the link run.
        assert!(runs.iter().any(|run| run.underline.is_some()));
    }

    #[test]
    fn followup_gap_is_user_after_assistant() {
        let messages = vec![
            ChatMessage {
                user: true,
                steps: vec![Step {
                    text: "a".into(),
                    ..Step::default()
                }],
                elapsed: None,
                images: Vec::new(),
                finished_at: None,
            },
            ChatMessage {
                user: false,
                steps: vec![Step {
                    text: "b".into(),
                    ..Step::default()
                }],
                elapsed: None,
                images: Vec::new(),
                finished_at: None,
            },
            ChatMessage {
                user: true,
                steps: vec![Step {
                    text: "c".into(),
                    ..Step::default()
                }],
                elapsed: None,
                images: Vec::new(),
                finished_at: None,
            },
        ];
        assert!(!starts_followup_turn(&messages, 0));
        assert!(!starts_followup_turn(&messages, 1));
        assert!(starts_followup_turn(&messages, 2));
    }
}
