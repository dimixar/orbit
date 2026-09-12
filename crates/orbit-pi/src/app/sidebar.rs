use super::helpers::*;
use super::*;

use gpui::{
    point, relative, App, Bounds, ContentMask, Element, GlobalElementId, InspectorElementId,
    LayoutId, Style, TextRun, Window, WrappedLine,
};
use unicode_segmentation::UnicodeSegmentation;

/// A non-interactive pill used for static meta in the composer row. Ghost
/// style: it states a fact (the access mode), so it carries no border or
/// fill and sits quieter than the interactive chips.
pub(crate) fn pill_static(
    icon_path: &'static str,
    label: &str,
    theme: Theme,
) -> impl IntoElement + use<> {
    div()
        .flex()
        .items_center()
        .gap_1p5()
        .px(px(7.))
        // Fixed height so the pill aligns exactly with the 24px chips and
        // the attach button in the composer row.
        .h(px(24.))
        .rounded_md()
        .text_size(theme.ui_px(12.))
        .text_color(theme.text_3)
        .child(icon(icon_path, 12., theme.text_3))
        .child(label.to_string())
}

/// Whether a workspace group is collapsed in the sidebar. The active
/// workspace is expanded by default; all others are collapsed unless the
/// user has toggled them.
pub(crate) fn is_workspace_group_collapsed(
    label: &str,
    working_label: &str,
    collapsed_workspaces: &HashSet<String>,
    expanded_workspace_groups: &HashSet<String>,
) -> bool {
    if label == working_label {
        collapsed_workspaces.contains(label)
    } else {
        !expanded_workspace_groups.contains(label)
    }
}

/// Toggle a workspace group's open/closed state against the defaults above.
pub(crate) fn toggle_workspace_group(
    label: String,
    working_label: &str,
    collapsed_workspaces: &mut HashSet<String>,
    expanded_workspace_groups: &mut HashSet<String>,
) {
    if label == working_label {
        if !collapsed_workspaces.remove(&label) {
            collapsed_workspaces.insert(label);
        }
    } else if !expanded_workspace_groups.remove(&label) {
        expanded_workspace_groups.insert(label);
    }
}

/// Sessions visible under one workspace group when it is not expanded.
pub(crate) fn visible_sessions_in_group(
    ixs: &[usize],
    sessions: &[SessionInfo],
    expanded: bool,
    active_path: &Option<PathBuf>,
) -> Vec<usize> {
    if expanded || ixs.len() <= SIDEBAR_GROUP_SESSIONS_VISIBLE {
        return ixs.to_vec();
    }

    let limit = SIDEBAR_GROUP_SESSIONS_VISIBLE;
    let mut indices: Vec<usize> = ixs.iter().take(limit).copied().collect();
    if let Some(active) = active_path {
        if let Some(active_ix) = sessions.iter().position(|s| &s.path == active) {
            if ixs.contains(&active_ix) && !indices.contains(&active_ix) {
                indices.pop();
                indices.push(active_ix);
                indices.sort_by_key(|ix| ixs.iter().position(|&i| i == *ix).unwrap_or(usize::MAX));
            }
        }
    }
    indices
}

/// Sidebar session list: `sessions` (newest-first, from disk) plus a
/// placeholder row for the open session when its file is not in the store
/// yet — see [`OrbitApp::sidebar_sessions`]. The placeholder carries the
/// workspace the pi process runs in, pi's live title when it has already
/// named the session, and a `now` stamp so it sorts to the top of the
/// sidebar. A session that hasn't started (`session_started` = false — no
/// user message sent yet) gets no placeholder: a draft is not listed
/// (Waku drafts parity).
pub(crate) fn sessions_with_placeholder(
    sessions: &[SessionInfo],
    current_path: Option<&Path>,
    current_title: Option<&str>,
    current_workspace: Option<&Path>,
    session_started: bool,
) -> Vec<SessionInfo> {
    let mut rows = sessions.to_vec();
    let Some(path) = current_path else {
        return rows;
    };
    if rows.iter().any(|s| s.path == path) {
        return rows;
    }
    if !session_started {
        return rows;
    }
    let workspace = current_workspace
        .map(Path::to_path_buf)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    rows.insert(
        0,
        SessionInfo {
            path: path.to_path_buf(),
            id: path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            cwd: workspace,
            title: current_title
                .filter(|title| !title.is_empty())
                .unwrap_or("New Task")
                .to_string(),
            first_message: String::new(),
            modified: SystemTime::now(),
        },
    );
    rows
}

/// Build the grouped sidebar session list. Each workspace shows at most
/// [`SIDEBAR_GROUP_SESSIONS_VISIBLE`] sessions until expanded. Workspaces in
/// `hidden` are pulled out of the main list into the "Hidden" disclosure at
/// the foot; their sessions stay in the list passed to the ⌘P selector.
pub(crate) fn build_sidebar_rows(
    sessions: &[SessionInfo],
    working_label: &str,
    collapsed_workspaces: &HashSet<String>,
    expanded_workspace_groups: &HashSet<String>,
    expanded_session_groups: &HashSet<String>,
    active_path: &Option<PathBuf>,
    hidden: &HashSet<String>,
    hidden_expanded: bool,
) -> Vec<SideRow> {
    let mut side_rows: Vec<SideRow> = Vec::new();
    let mut groups: Vec<(String, Vec<usize>)> = Vec::new();
    for (ix, session) in sessions.iter().enumerate() {
        let label = sessions::workspace_label(&session.cwd);
        match groups.iter_mut().find(|(l, _)| *l == label) {
            Some((_, ixs)) => ixs.push(ix),
            None => groups.push((label, vec![ix])),
        }
    }
    let mut hidden_groups: Vec<(String, Vec<usize>)> = Vec::new();
    for (label, ixs) in groups {
        if hidden.contains(&label) {
            hidden_groups.push((label, ixs));
            continue;
        }
        let collapsed = is_workspace_group_collapsed(
            &label,
            working_label,
            collapsed_workspaces,
            expanded_workspace_groups,
        );
        side_rows.push(SideRow::Workspace {
            label: label.clone(),
            count: ixs.len(),
            collapsed,
            cwd: sessions[ixs[0]].cwd.clone(),
        });
        if collapsed {
            // A collapsed group hides its sessions — except the open one,
            // which stays pinned under the header so the sidebar always
            // shows where the live session lives.
            if let Some(active) = active_path {
                if let Some(&ix) = ixs.iter().find(|&&ix| sessions[ix].path == *active) {
                    side_rows.push(SideRow::Session(ix));
                }
            }
            continue;
        }

        let sessions_expanded = expanded_session_groups.contains(&label);
        let visible = visible_sessions_in_group(&ixs, sessions, sessions_expanded, active_path);
        for ix in &visible {
            side_rows.push(SideRow::Session(*ix));
        }

        if sessions_expanded {
            if ixs.len() > SIDEBAR_GROUP_SESSIONS_VISIBLE {
                side_rows.push(SideRow::ShowLess { label });
            }
        } else if ixs.len() > SIDEBAR_GROUP_SESSIONS_VISIBLE {
            let hidden_count = ixs.len().saturating_sub(visible.len());
            if hidden_count > 0 {
                side_rows.push(SideRow::ShowMore {
                    label,
                    count: hidden_count,
                });
            }
        }
    }

    if !hidden_groups.is_empty() {
        side_rows.push(SideRow::HiddenHeader {
            count: hidden_groups.len(),
            expanded: hidden_expanded,
        });
        if hidden_expanded {
            for (label, ixs) in hidden_groups {
                side_rows.push(SideRow::HiddenWorkspace {
                    label,
                    count: ixs.len(),
                });
            }
        }
    }
    side_rows
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn render_side_row(
    rows: &Rc<Vec<SideRow>>,
    sessions_data: &Rc<Vec<SessionInfo>>,
    active_path: Option<&Path>,
    ix: usize,
    this: &Entity<OrbitApp>,
    agent_running: bool,
    running_paths: &Rc<HashSet<PathBuf>>,
    session_menu: Option<&SessionMenu>,
    workspace_menu: Option<&WorkspaceMenu>,
    theme: Theme,
) -> impl IntoElement {
    match &rows[ix] {
        SideRow::Workspace {
            label,
            count,
            collapsed,
            cwd,
        } => {
            let label = label.clone();
            let label_for_click = label.clone();
            let cwd_for_new = cwd.clone();
            let this_toggle = this.clone();
            let this_new = this.clone();
            let this_menu = this.clone();
            let menu_open = workspace_menu.is_some_and(|m| m.label == label);
            // Outer shell: inter-group spacing only — hover lives on the inner
            // card so the highlight doesn't bleed into the padding (same split
            // as session rows below). The header is a minimal label row:
            // chevron + folder + name, the session count pinned to the very
            // end, and the row actions (a `…` menu, then the new-session `+`)
            // fading in to the count's left on hover (all flex_none, so
            // nothing shifts when they appear).
            div()
                .w_full()
                .px_2()
                .pt(px(12.))
                .pb(px(2.))
                .group("workspace-row")
                .child(
                    div()
                        .w_full()
                        .h(px(24.))
                        .px(px(6.))
                        .rounded_md()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .cursor_pointer()
                        .hover(|s| s.bg(theme.bg_hover))
                        .on_mouse_up(MouseButton::Left, move |_, _, cx| {
                            let label = label_for_click.clone();
                            this_toggle.update(cx, |app, cx| {
                                let working = app.workspace_label();
                                toggle_workspace_group(
                                    label,
                                    &working,
                                    &mut app.collapsed_workspaces,
                                    &mut app.expanded_workspace_groups,
                                );
                                cx.notify();
                            });
                        })
                        .child(icon(
                            if *collapsed {
                                "icons/chevron-right.svg"
                            } else {
                                "icons/chevron-down.svg"
                            },
                            10.,
                            theme.text_3,
                        ))
                        .child(icon("icons/folder.svg", 13., theme.text_2))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .text_size(theme.ui_px(11.5))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme.text_2)
                                .child(label.clone()),
                        )
                        // Row actions, revealed on hover: a `…` menu (hide /
                        // copy path) beside the direct new-task `+`.
                        .child(workspace_menu_button(
                            label.clone(),
                            cwd.clone(),
                            menu_open,
                            this_menu.clone(),
                            theme,
                        ))
                        .child(
                            div()
                                .id(ElementId::Name(format!("workspace-new-{label}").into()))
                                .flex_none()
                                .size(px(18.))
                                .rounded(px(4.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                // Revealed on row hover — the quiet default
                                // keeps group headers to just label + count.
                                .opacity(0.)
                                .group_hover("workspace-row", |s| s.opacity(1.))
                                .hover(|s| s.bg(theme.overlay))
                                .on_mouse_up(MouseButton::Left, {
                                    let this = this_new.clone();
                                    move |_, window, cx| {
                                        cx.stop_propagation();
                                        let cwd = cwd_for_new.clone();
                                        this.update(cx, |app, cx| {
                                            app.on_new_session_in_workspace(cwd, window, cx);
                                        });
                                    }
                                })
                                .child(icon("icons/plus.svg", 13., theme.text_3)),
                        )
                        // Session count, pinned to the header's right edge.
                        .child(
                            div()
                                .flex_none()
                                .text_size(theme.ui_px(10.5))
                                .text_color(theme.text_3)
                                .child(format!("{count}")),
                        ),
                )
                .into_any_element()
        }
        SideRow::ShowMore { label, count } => {
            let this = this.clone();
            let label_for_click = label.clone();
            div()
                .w_full()
                .h(px(26.))
                .pl(px(22.))
                .pr_2()
                .flex()
                .items_center()
                .gap_1p5()
                .rounded_md()
                .cursor_pointer()
                .hover(|s| s.bg(theme.bg_hover))
                .on_mouse_up(MouseButton::Left, move |_, _, cx| {
                    let label = label_for_click.clone();
                    this.update(cx, |app, cx| {
                        app.expanded_session_groups.insert(label);
                        cx.notify();
                    });
                })
                .child(
                    div()
                        .text_size(theme.ui_px(11.5))
                        .text_color(theme.text_3)
                        .child(format!("Show {count} more")),
                )
                .into_any_element()
        }
        SideRow::ShowLess { label } => {
            let this = this.clone();
            let label_for_click = label.clone();
            div()
                .w_full()
                .h(px(26.))
                .pl(px(22.))
                .pr_2()
                .flex()
                .items_center()
                .gap_1p5()
                .rounded_md()
                .cursor_pointer()
                .hover(|s| s.bg(theme.bg_hover))
                .on_mouse_up(MouseButton::Left, move |_, _, cx| {
                    let label = label_for_click.clone();
                    this.update(cx, |app, cx| {
                        app.expanded_session_groups.remove(&label);
                        cx.notify();
                    });
                })
                .child(
                    div()
                        .text_size(theme.ui_px(11.5))
                        .text_color(theme.text_3)
                        .child("Show less"),
                )
                .into_any_element()
        }
        SideRow::Session(ix) => {
            let session = sessions_data[*ix].clone();
            let session_for_click = session.clone();
            let active = active_path == Some(session.path.as_path());
            // The open session runs live; parked (background) sessions run
            // in their own pi processes — both get the loader (Waku).
            let running = (active && agent_running) || running_paths.contains(&session.path);
            let this = this.clone();
            let this_for_row = this.clone();
            let this_for_menu = this.clone();
            let menu = session_menu.filter(|m| m.path == session.path);
            // Sessions with a live pi process must not be deleted — the
            // process would recreate the file mid-run.
            let deletable = !active && !running;
            // Running sessions lead with a small spinner and a shimmering
            // title (shadcn's Marker + `shimmer`); row actions stay available
            // on hover.
            let title = session_title(*ix, session.title.clone().into(), theme, active, running);
            // Two-line row (title + actions, then preview · age),
            // indented under its workspace group so the list reads as a
            // tree. The open session gets a raised fill and an accent bar
            // in the indent gutter; row actions are revealed on hover.
            // Outer item carries the inter-row spacing (padding) and the click
            // handler; the inner card holds the background/hover so the gap
            // between cards stays clear. Padding (not margin) is used because
            // the list measures each item's border-box — margins are dropped.
            let mut row = div()
                .id(ElementId::NamedInteger("side-session".into(), *ix as u64))
                .w_full()
                .py(px(1.))
                .cursor_pointer()
                .on_mouse_up(MouseButton::Left, move |_, _, cx| {
                    let session = session_for_click.clone();
                    this_for_row.update(cx, |app, cx| {
                        app.on_open_session(session, cx);
                    });
                });
            let mut card = div()
                .group("srow")
                .relative()
                .w_full()
                .pl(px(22.))
                .pr(px(8.))
                .py(theme.space(5.))
                .rounded_md()
                .flex()
                .items_center()
                .gap(px(6.))
                .when(active, |card| card.bg(theme.bg_raised))
                .when(!active, |card| card.hover(|s| s.bg(theme.bg_hover)));
            // Active marker: a short accent bar in the indent gutter.
            if active {
                card = card.child(
                    div()
                        .absolute()
                        .left(px(8.))
                        .top(px(8.))
                        .bottom(px(8.))
                        .w(px(2.))
                        .rounded_full()
                        .bg(theme.accent),
                );
            }
            // Two-line text column: title + actions on top, then the
            // preview with the age pinned to its right end.
            card = card.child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .justify_center()
                    .gap(px(2.))
                    // Line 1 — title with the row actions pinned to its
                    // right: a small running spinner leads, the hover-revealed
                    // `…` menu trails. Wrapped in a flex row so the
                    // `flex_1 min_w_0` title takes the full column width and
                    // paints the name — instead of collapsing to "…" as a bare
                    // flex-column child.
                    .child(
                        div()
                            .w_full()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .when(running, |line| line.child(running_loader(theme, *ix)))
                            .child(title)
                            .child(session_menu_button(
                                *ix,
                                menu,
                                session.path.clone(),
                                session.title.clone(),
                                deletable,
                                this_for_menu,
                                theme,
                            )),
                    )
                    // Line 2 — first-message preview with the age at the very
                    // end (accent at low opacity, so the timestamp reads as
                    // metadata, not content).
                    .child(
                        div()
                            .w_full()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_size(theme.ui_px(11.))
                                    .line_height(px(14.))
                                    .text_color(theme.text_3)
                                    .child(session.first_message.clone()),
                            )
                            .child(
                                div()
                                    .flex_none()
                                    .text_size(theme.ui_px(10.5))
                                    .text_color(theme.accent.opacity(0.75))
                                    .child(sessions::relative_time(session.modified)),
                            ),
                    ),
            );
            row = row.child(card);
            row.into_any_element()
        }
        SideRow::HiddenHeader { count, expanded } => {
            let this = this.clone();
            div()
                .w_full()
                .px_2()
                .pt(px(12.))
                .pb(px(2.))
                .child(
                    div()
                        .id("hidden-workspaces")
                        .w_full()
                        .h(px(24.))
                        .px(px(6.))
                        .rounded_md()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .cursor_pointer()
                        .hover(|s| s.bg(theme.bg_hover))
                        .on_mouse_up(MouseButton::Left, move |_, _, cx| {
                            this.update(cx, |app, cx| app.toggle_hidden_sidebar(cx));
                        })
                        .child(icon(
                            if *expanded {
                                "icons/chevron-down.svg"
                            } else {
                                "icons/chevron-right.svg"
                            },
                            10.,
                            theme.text_3,
                        ))
                        .child(icon("icons/eye-off.svg", 13., theme.text_3))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .text_size(theme.ui_px(11.5))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(theme.text_3)
                                .child("Hidden"),
                        )
                        .child(
                            div()
                                .flex_none()
                                .text_size(theme.ui_px(10.5))
                                .text_color(theme.text_3)
                                .child(format!("{count}")),
                        ),
                )
                .into_any_element()
        }
        SideRow::HiddenWorkspace { label, count, .. } => {
            let this = this.clone();
            let label_for_show = label.clone();
            let label_text = label.clone();
            div()
                .w_full()
                .px_2()
                .child(
                    div()
                        .id(ElementId::Name(format!("hidden-workspace-{label}").into()))
                        .w_full()
                        .h(px(26.))
                        .pl(px(22.))
                        .pr(px(6.))
                        .rounded_md()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .hover(|s| s.bg(theme.bg_hover))
                        .child(icon("icons/folder.svg", 13., theme.text_3))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .text_size(theme.ui_px(11.5))
                                .text_color(theme.text_3)
                                .child(label_text),
                        )
                        .child(
                            div()
                                .flex_none()
                                .text_size(theme.ui_px(10.5))
                                .text_color(theme.text_3)
                                .child(format!("{count}")),
                        )
                        .child(
                            div()
                                .id(ElementId::Name(
                                    format!("hidden-workspace-show-{label}").into(),
                                ))
                                .flex_none()
                                .size(px(20.))
                                .rounded(px(5.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .hover(|s| s.bg(theme.overlay))
                                .on_mouse_up(MouseButton::Left, move |_, _, cx| {
                                    cx.stop_propagation();
                                    let label = label_for_show.clone();
                                    this.update(cx, |app, cx| app.on_workspace_show(label, cx));
                                })
                                .child(icon("icons/eye.svg", 14., theme.text_2)),
                        ),
                )
                .into_any_element()
        }
    }
}

/// The hover-revealed '…' button on a quiet session row. Clicking it opens
/// the row's actions popup; propagation stops so the row's open handler
/// doesn't also fire.
pub(crate) fn session_menu_button(
    ix: usize,
    menu: Option<&SessionMenu>,
    path: PathBuf,
    title: String,
    deletable: bool,
    this: Entity<OrbitApp>,
    theme: Theme,
) -> impl IntoElement + use<> {
    let menu_open = menu.is_some();
    let this_for_popup = this.clone();
    div()
        .id(ElementId::NamedInteger("side-more".into(), ix as u64))
        .relative()
        .flex_none()
        .size(px(18.))
        .rounded(px(4.))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        // Hidden until the row (or the button itself) is hovered, or while
        // this row's menu is open. Icon-only, no background — a filled hover
        // square reads as a patch covering the row's right edge.
        .opacity(if menu_open { 1.0 } else { 0.0 })
        .group_hover("srow", |s| s.opacity(1.))
        .on_mouse_up(MouseButton::Left, move |_, window, cx| {
            // Keep the click from also opening the session via the row.
            cx.stop_propagation();
            let (path, title, deletable) = (path.clone(), title.clone(), deletable);
            this.update(cx, |app, cx| {
                app.toggle_session_menu(
                    SessionMenu {
                        path,
                        title,
                        deletable,
                        confirm_delete: false,
                    },
                    window,
                    cx,
                );
            });
        })
        .child(icon("icons/more.svg", 14., theme.text_3))
        // The dropdown hangs off a zero-size anchor pinned to the button's
        // top-left corner. The button centers its icon (`items_center` +
        // `justify_center`), and Taffy lays absolutely-positioned children out
        // with the container's alignment — without the pin, the popup's
        // static position is pulled toward the button's center and it opens
        // up-left of the trigger instead of just below it.
        .children(menu.map(|menu| {
            div()
                .absolute()
                .top_0()
                .left_0()
                .size(px(0.))
                .child(session_menu_popup(menu, this_for_popup.clone(), theme))
        }))
}

/// Shimmer highlight width in pixels (shadcn's band is roughly 3ch + 40px).
const SHIMMER_BAND_PX: f32 = 56.0;

/// A small spinner at the start of a running session row — the shadcn
/// Marker + Spinner pattern. `with_animation` rotates it; no app tick.
fn running_loader(theme: Theme, id: usize) -> impl IntoElement + use<> {
    gpui::svg()
        .path("icons/loader.svg")
        .flex_none()
        .size(px(11.))
        .text_color(theme.accent)
        .with_animation(
            ElementId::NamedInteger("side-spin".into(), id as u64),
            Animation::new(Duration::from_millis(900)).repeat(),
            |svg, delta| {
                svg.with_transformation(Transformation::rotate(radians(
                    delta * std::f32::consts::TAU,
                )))
            },
        )
}

/// A session row's title. Quiet rows render as one truncated line; a running
/// row paints the shadcn `shimmer` — a highlight band sweeping across the
/// glyphs — driven by `with_animation` (self-repainting, no app tick).
fn session_title(
    id: usize,
    title: SharedString,
    theme: Theme,
    active: bool,
    running: bool,
) -> AnyElement {
    let color = if active { theme.text } else { theme.text_2 };
    let weight = if active {
        FontWeight::MEDIUM
    } else {
        FontWeight::NORMAL
    };
    let size = theme.ui_px(13.);
    let line_height = px(18.);
    if !running {
        return div()
            .flex_1()
            .min_w_0()
            .truncate()
            .text_size(size)
            .line_height(line_height)
            .font_weight(weight)
            .text_color(color)
            .child(title)
            .into_any_element();
    }
    // The sweep catches the accent: ink at the edges, ember through the band
    // (shadcn's highlight, spent on the one hue the system rations).
    let highlight = theme.accent;
    div()
        .flex_1()
        .min_w_0()
        .text_size(size)
        .line_height(line_height)
        .font_weight(weight)
        .child(
            ShimmerText {
                text: title,
                base: color,
                highlight,
                phase: 0.0,
            }
            .with_animation(
                ElementId::NamedInteger("side-shimmer".into(), id as u64),
                Animation::new(Duration::from_millis(2000)).repeat(),
                |mut text, delta| {
                    text.phase = delta;
                    text
                },
            ),
        )
        .into_any_element()
}

/// Smooth band falloff: 1 at the sweep center, easing to 0 at its edges.
fn band_intensity(pos: f32, center: f32, half_band: f32) -> f32 {
    let d = (pos - center).abs();
    if d >= half_band {
        return 0.0;
    }
    let t = 1.0 - d / half_band;
    t * t * (3.0 - 2.0 * t)
}

/// Mix two colors by `t` (0 = `a`, 1 = `b`). Hue takes the shortest arc, so
/// the sweep from gray ink to the accent never loops the color wheel.
fn blend_hsla(a: Hsla, b: Hsla, t: f32) -> Hsla {
    let t = t.clamp(0.0, 1.0);
    let mut dh = b.h - a.h;
    if dh > 0.5 {
        dh -= 1.0;
    } else if dh < -0.5 {
        dh += 1.0;
    }
    Hsla {
        h: (a.h + dh * t).rem_euclid(1.0),
        s: a.s + (b.s - a.s) * t,
        l: a.l + (b.l - a.l) * t,
        a: a.a + (b.a - a.a) * t,
    }
}

/// A single-line title whose ink catches a moving highlight. GPUI has no
/// `background-clip: text`, so the sweep is painted run-by-run — the shaped
/// layout is unchanged by the per-run colors. It ellipsizes like gpui's own
/// text element, so a long title still ends in `…`.
struct ShimmerText {
    text: SharedString,
    base: Hsla,
    highlight: Hsla,
    phase: f32,
}

impl IntoElement for ShimmerText {
    type Element = Self;
    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for ShimmerText {
    type RequestLayoutState = ();
    type PrepaintState = Vec<WrappedLine>;

    fn id(&self) -> Option<gpui::ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = window.line_height().into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        _cx: &mut App,
    ) -> Self::PrepaintState {
        let line_height = window.line_height();
        let text_style = window.text_style();
        let font = text_style.font();
        let font_size = text_style.font_size.to_pixels(window.rem_size());
        let run = |len: usize, color: Hsla| TextRun {
            len,
            font: font.clone(),
            color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };

        // Truncate to the row width with gpui's own helper, so the ellipsis
        // matches the rest of the UI. These runs are only shaped for glyph
        // geometry; the painted colors are rebuilt below.
        let mut runs = vec![run(self.text.len(), self.base)];
        let display = window
            .text_system()
            .line_wrapper(font.clone(), font_size)
            .truncate_line(self.text.clone(), bounds.size.width, "\u{2026}", &mut runs);

        let Ok(measured) =
            window
                .text_system()
                .shape_text(display.clone(), font_size, &runs, None, None)
        else {
            return Vec::new();
        };
        let Some(line) = measured.first() else {
            return Vec::new();
        };
        let total = f32::from(line.width()).max(1.);
        let half_band = (SHIMMER_BAND_PX / total).clamp(0.08, 0.5);
        let center = -half_band + self.phase * (1.0 + 2.0 * half_band);

        let mut colored = Vec::new();
        for (ix, grapheme) in display.grapheme_indices(true) {
            let x = line
                .position_for_index(ix, line_height)
                .map(|p| f32::from(p.x))
                .unwrap_or(0.);
            let t = band_intensity(x / total, center, half_band);
            colored.push(run(
                grapheme.len(),
                blend_hsla(self.base, self.highlight, t),
            ));
        }

        window
            .text_system()
            .shape_text(display, font_size, &colored, None, None)
            .map(|lines| lines.to_vec())
            .unwrap_or_default()
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let line_height = window.line_height();
        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            for line in prepaint.iter() {
                line.paint(
                    point(bounds.origin.x, bounds.origin.y),
                    line_height,
                    TextAlign::Left,
                    None,
                    window,
                    cx,
                )
                .ok();
            }
        });
    }
}

// ── sidebar row-actions popup ──────────────────────────────────────────

/// The actions popup anchored to a session row: Copy path / Reveal in
/// Finder / Delete, or the delete confirmation once armed. Painted via
/// `deferred` + `anchored` (same convention as the composer pickers),
/// dismissed by any outside mouse-down.
pub(crate) fn session_menu_popup(
    menu: &SessionMenu,
    this: Entity<OrbitApp>,
    theme: Theme,
) -> AnyElement {
    let deletable = menu.deletable;
    let confirm = menu.confirm_delete;

    let body: AnyElement = if confirm {
        // Delete confirmation — the destructive step gets a named victim.
        div()
            .w_full()
            .flex()
            .flex_col()
            .gap(px(6.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(
                        div()
                            .text_size(theme.ui_px(12.5))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.text)
                            .child("Delete this session?"),
                    )
                    .child(
                        div()
                            .text_size(theme.ui_px(11.))
                            .text_color(theme.text_2)
                            .child("Removes the session file from disk."),
                    ),
            )
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(6.))
                    .child(
                        div()
                            .id("menu-cancel")
                            .h(px(24.))
                            .px(px(10.))
                            .flex()
                            .items_center()
                            .rounded(px(6.))
                            .bg(theme.bg_raised)
                            .cursor_pointer()
                            .hover(|s| s.bg(theme.bg_hover))
                            .text_size(theme.ui_px(11.5))
                            .text_color(theme.text_2)
                            .on_mouse_down(MouseButton::Left, {
                                let this = this.clone();
                                move |_, _, cx| {
                                    cx.stop_propagation();
                                    this.update(cx, |app, cx| app.on_menu_cancel(cx));
                                }
                            })
                            .child("Cancel"),
                    )
                    .child(
                        div()
                            .id("menu-confirm-delete")
                            .h(px(24.))
                            .px(px(10.))
                            .flex()
                            .items_center()
                            .rounded(px(6.))
                            .bg(theme.stop_red)
                            .cursor_pointer()
                            .hover(|s| s.bg(theme.stop_red_hover))
                            .text_size(theme.ui_px(11.5))
                            .text_color(theme.send_fg)
                            .on_mouse_down(MouseButton::Left, {
                                let this = this.clone();
                                move |_, _, cx| {
                                    cx.stop_propagation();
                                    this.update(cx, |app, cx| app.on_menu_delete_confirm(cx));
                                }
                            })
                            .child("Delete"),
                    ),
            )
            .into_any_element()
    } else {
        div()
            .w_full()
            .flex()
            .flex_col()
            .child(menu_item(
                "menu-copy-path",
                "icons/copy.svg",
                "Copy path",
                theme,
                this.clone(),
                false,
                |app, cx| app.on_menu_copy_path(cx),
            ))
            .child(menu_item(
                "menu-reveal",
                "icons/folder.svg",
                "Reveal in Finder",
                theme,
                this.clone(),
                false,
                |app, cx| app.on_menu_reveal(cx),
            ))
            .when(deletable, |menu| {
                menu.child(div().h(px(1.)).w_full().bg(theme.border).my(px(4.)))
                    .child(menu_item(
                        "menu-delete",
                        "icons/trash.svg",
                        "Delete session…",
                        theme,
                        this.clone(),
                        true,
                        |app, cx| app.on_menu_delete_request(cx),
                    ))
            })
            .into_any_element()
    };

    let popup = div()
        .w(px(190.))
        .p(px(4.))
        .when(confirm, |pop| pop.w(px(210.)).p(px(10.)))
        .rounded(px(10.))
        .border_1()
        .border_color(theme.border_strong)
        .bg(theme.menu_bg)
        .shadow(theme.popover_shadow())
        .flex()
        .flex_col()
        .overflow_hidden()
        .occlude()
        // Any mouse-down outside dismisses; clicks inside are stopped by
        // the item handlers, so they never read as "outside".
        .on_mouse_down_out({
            let this = this.clone();
            move |_, _, cx| {
                this.update(cx, |app, cx| {
                    // Arm the gesture guard so this same click's mouse-up
                    // cannot immediately re-open the menu.
                    app.menu_dismissed_at = Some(Instant::now());
                    app.session_menu = None;
                    cx.notify();
                })
            }
        })
        .child(body);

    // Float the popup: `anchored` takes it out of the layout (no other row
    // moves) and pins its top-left corner just below the '…' button (the
    // button is 18px, so +20px drops the top edge 2px under it, left-aligned
    // to the trigger — the standard app dropdown position). The wrapping
    // zero-size anchor in `session_menu_button` guarantees the static origin
    // is the button's top-left regardless of the button's own centering.
    // `deferred` paints it above the rest of the list — same convention as
    // the composer chip pickers. `snap_to_window` keeps it inside the window
    // near the edges.
    anchored()
        .position_mode(AnchoredPositionMode::Local)
        .anchor(Corner::TopLeft)
        .offset(point(px(0.), px(20.)))
        .snap_to_window()
        .child(deferred(popup))
        .into_any_element()
}

/// The hover-revealed '…' button on a workspace group header. Clicking it
/// opens the header's actions popup; propagation stops so the header's
/// collapse toggle doesn't also fire.
pub(crate) fn workspace_menu_button(
    label: String,
    cwd: PathBuf,
    menu_open: bool,
    this: Entity<OrbitApp>,
    theme: Theme,
) -> impl IntoElement + use<> {
    let this_for_popup = this.clone();
    let label_for_click = label.clone();
    let cwd_for_click = cwd.clone();
    div()
        .id(ElementId::Name(format!("workspace-more-{label}").into()))
        .relative()
        .flex_none()
        .size(px(18.))
        .rounded(px(4.))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        // Revealed on row hover, or while this header's menu is open.
        .opacity(if menu_open { 1.0 } else { 0.0 })
        .group_hover("workspace-row", |s| s.opacity(1.))
        .on_mouse_up(MouseButton::Left, move |_, window, cx| {
            cx.stop_propagation();
            let menu = WorkspaceMenu {
                label: label_for_click.clone(),
                cwd: cwd_for_click.clone(),
            };
            this.update(cx, |app, cx| app.toggle_workspace_menu(menu, window, cx));
        })
        .child(icon("icons/more.svg", 14., theme.text_3))
        .children(menu_open.then(|| {
            div()
                .absolute()
                .top_0()
                .left_0()
                .size(px(0.))
                .child(workspace_menu_popup(this_for_popup.clone(), theme))
        }))
}

/// The actions popup anchored to a workspace header: Copy path / Hide from
/// sidebar. Painted with the same deferred + anchored convention as the
/// session row menu, dismissed by any outside mouse-down.
pub(crate) fn workspace_menu_popup(this: Entity<OrbitApp>, theme: Theme) -> AnyElement {
    let popup = div()
        .w(px(200.))
        .p(px(4.))
        .rounded(px(10.))
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
            move |_, _, cx| {
                this.update(cx, |app, cx| {
                    // Arm the gesture guard so this same click's mouse-up
                    // cannot immediately re-open the menu.
                    app.menu_dismissed_at = Some(Instant::now());
                    app.workspace_menu = None;
                    cx.notify();
                })
            }
        })
        .child(menu_item(
            "wm-copy-path",
            "icons/copy.svg",
            "Copy path",
            theme,
            this.clone(),
            false,
            |app, cx| app.on_workspace_copy_path(cx),
        ))
        .child(div().h(px(1.)).w_full().bg(theme.border).my(px(4.)))
        .child(menu_item(
            "wm-hide",
            "icons/eye-off.svg",
            "Hide from sidebar",
            theme,
            this.clone(),
            false,
            |app, cx| app.on_workspace_hide(cx),
        ));

    anchored()
        .position_mode(AnchoredPositionMode::Local)
        .anchor(Corner::TopLeft)
        .offset(point(px(0.), px(20.)))
        .snap_to_window()
        .child(deferred(popup))
        .into_any_element()
}

pub(crate) fn menu_item<C: Fn(&mut OrbitApp, &mut Context<OrbitApp>) + 'static>(
    id: &'static str,
    icon_path: &'static str,
    label: &'static str,
    theme: Theme,
    this: Entity<OrbitApp>,
    danger: bool,
    on_click: C,
) -> impl IntoElement + use<C> {
    let on_click = on_click;
    let (hover_bg, text_color, icon_color) = if danger {
        (theme.stop_red_hover, theme.send_fg, theme.send_fg)
    } else {
        (theme.bg_hover, theme.text_2, theme.text_3)
    };
    div()
        .id(id)
        .h(px(30.))
        .px(px(8.))
        .rounded(px(6.))
        .flex()
        .items_center()
        .gap(px(8.))
        .cursor_pointer()
        .when(danger, |s| s.bg(theme.stop_red))
        .hover(move |s| s.bg(hover_bg))
        .text_size(theme.ui_px(12.5))
        .text_color(text_color)
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            cx.stop_propagation();
            this.update(cx, |app, cx| (on_click)(app, cx));
        })
        .child(icon(icon_path, 13., icon_color))
        .child(label.to_string())
}

/// Reveal a session file in the OS file manager (macOS first, matching the
/// product's platform stance; other platforms get the containing folder).
pub(crate) fn reveal_in_file_manager(path: PathBuf) {
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open")
        .arg("-R")
        .arg(path)
        .spawn();
    #[cfg(not(target_os = "macos"))]
    if let Some(dir) = path.parent() {
        #[cfg(target_os = "linux")]
        let _ = std::process::Command::new("xdg-open").arg(dir).spawn();
        #[cfg(target_os = "windows")]
        let _ = std::process::Command::new("explorer").arg(dir).spawn();
        let _ = dir;
    }
}

/// Empty-state panel for a fresh pi store: what this space is for and how
/// to fill it. No cards, no illustration — one calm statement.
pub(crate) fn empty_sessions_state(theme: Theme) -> impl IntoElement + use<> {
    div()
        .flex_1()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(6.))
        .px(px(20.))
        .pb(px(40.))
        .child(
            div()
                .size(px(36.))
                .rounded_full()
                .bg(theme.bg_raised)
                .flex()
                .items_center()
                .justify_center()
                .child(icon("icons/spark.svg", 16., theme.accent)),
        )
        .child(
            div()
                .text_size(theme.ui_px(12.5))
                .font_weight(FontWeight::MEDIUM)
                .text_color(theme.text_2)
                .child("No sessions yet"),
        )
        .child(
            div()
                .text_size(theme.ui_px(11.5))
                .text_color(theme.text_3)
                .text_align(TextAlign::Center)
                .child("Start a task and it will show up here."),
        )
        .child(
            div()
                .mt(px(6.))
                .px(px(6.))
                .h(px(18.))
                .flex()
                .items_center()
                .rounded(px(4.))
                .border_1()
                .border_color(theme.border)
                .text_size(theme.ui_px(10.5))
                .text_color(theme.text_3)
                .child("\u{2318}N"),
        )
}

// ── controller ────────────────────────────────────────────────────
impl OrbitApp {
    /// Open (or toggle closed) the row-actions popup for a session row.
    pub(super) fn toggle_session_menu(
        &mut self,
        menu: SessionMenu,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Same-click dismissal must not re-open (see toggle_session_picker).
        const GESTURE: Duration = Duration::from_millis(200);
        if let Some(dismissed) = self.menu_dismissed_at.take() {
            if dismissed.elapsed() < GESTURE {
                return;
            }
        }
        if self.session_menu.as_ref() == Some(&menu) {
            self.session_menu = None;
            cx.notify();
            return;
        }
        if self.command_palette.take().is_some() {
            self.input.read(cx).focus(window);
        }
        if self.branch_picker.take().is_some() {
            self.input.read(cx).focus(window);
        }
        if self.workspace_picker.take().is_some() {
            self.input.read(cx).focus(window);
        }
        if self.model_selector.is_some() {
            self.close_model_selector(window, cx);
        }
        self.workspace_menu = None;
        self.session_menu = Some(menu);
        cx.notify();
    }

    pub(super) fn on_menu_copy_path(&mut self, cx: &mut Context<Self>) {
        if let Some(menu) = &self.session_menu {
            cx.write_to_clipboard(ClipboardItem::new_string(
                menu.path.to_string_lossy().into_owned(),
            ));
        }
        self.session_menu = None;
        cx.notify();
    }

    pub(super) fn on_menu_reveal(&mut self, cx: &mut Context<Self>) {
        if let Some(menu) = &self.session_menu {
            reveal_in_file_manager(menu.path.clone());
        }
        self.session_menu = None;
        cx.notify();
    }

    /// First Delete click: swap the popup to the confirmation state.
    pub(super) fn on_menu_delete_request(&mut self, cx: &mut Context<Self>) {
        if let Some(menu) = self.session_menu.as_mut() {
            menu.confirm_delete = true;
            cx.notify();
        }
    }

    /// Confirmed: remove the session file from disk and refresh the list.
    pub(super) fn on_menu_delete_confirm(&mut self, cx: &mut Context<Self>) {
        if let Some(menu) = self.session_menu.take() {
            if let Err(err) = fs::remove_file(&menu.path) {
                self.set_status(format!("delete failed: {err}"));
            } else {
                self.set_status("Session deleted");
            }
            self.sessions = sessions::load_sessions();
            cx.notify();
        }
    }

    pub(super) fn on_menu_cancel(&mut self, cx: &mut Context<Self>) {
        self.session_menu = None;
        cx.notify();
    }

    /// Open (or toggle closed) the row-actions popup for a workspace header.
    pub(super) fn toggle_workspace_menu(
        &mut self,
        menu: WorkspaceMenu,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Same-click dismissal must not re-open (see toggle_session_menu).
        const GESTURE: Duration = Duration::from_millis(200);
        if let Some(dismissed) = self.menu_dismissed_at.take() {
            if dismissed.elapsed() < GESTURE {
                return;
            }
        }
        if self.workspace_menu.as_ref() == Some(&menu) {
            self.workspace_menu = None;
            cx.notify();
            return;
        }
        if self.command_palette.take().is_some() {
            self.input.read(cx).focus(window);
        }
        if self.branch_picker.take().is_some() {
            self.input.read(cx).focus(window);
        }
        if self.workspace_picker.take().is_some() {
            self.input.read(cx).focus(window);
        }
        self.session_menu = None;
        if self.model_selector.is_some() {
            self.close_model_selector(window, cx);
        }
        self.workspace_menu = Some(menu);
        cx.notify();
    }

    pub(super) fn on_workspace_copy_path(&mut self, cx: &mut Context<Self>) {
        if let Some(menu) = &self.workspace_menu {
            cx.write_to_clipboard(ClipboardItem::new_string(
                menu.cwd.to_string_lossy().into_owned(),
            ));
        }
        self.workspace_menu = None;
        cx.notify();
    }

    /// Hide the workspace the open header menu belongs to. Its sessions stay
    /// on disk and in the ⌘P selector; only the sidebar drops the group.
    pub(super) fn on_workspace_hide(&mut self, cx: &mut Context<Self>) {
        let Some(menu) = self.workspace_menu.take() else {
            return;
        };
        if self.hidden_workspaces.insert(menu.label.clone()) {
            persist_hidden_workspaces(&self.hidden_workspaces);
            self.set_status(format!("Hid {} from the sidebar", menu.label));
        }
        cx.notify();
    }

    /// Restore a hidden workspace to the sidebar (from the Hidden section).
    pub(super) fn on_workspace_show(&mut self, label: String, cx: &mut Context<Self>) {
        if self.hidden_workspaces.remove(&label) {
            persist_hidden_workspaces(&self.hidden_workspaces);
            self.set_status(format!("Showed {label} in the sidebar"));
        }
        cx.notify();
    }

    /// Toggle the "Hidden" disclosure at the foot of the sidebar.
    pub(super) fn toggle_hidden_sidebar(&mut self, cx: &mut Context<Self>) {
        self.hidden_sidebar_expanded = !self.hidden_sidebar_expanded;
        cx.notify();
    }

    /// Keep the row menu honest: close it when its session vanishes from
    /// the store (deleted externally, session ended, …).
    pub(super) fn sync_session_menu(&mut self, cx: &mut Context<Self>) {
        if let Some(menu) = &self.session_menu {
            if !menu.path.exists() {
                self.session_menu = None;
                cx.notify();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shimmer_band_peaks_at_center_and_fades_out() {
        // Full highlight at the sweep center, nothing past the band edge.
        assert_eq!(band_intensity(0.5, 0.5, 0.2), 1.0);
        assert_eq!(band_intensity(0.25, 0.5, 0.2), 0.0);
        // Smooth, monotonic falloff toward the edge.
        let near = band_intensity(0.45, 0.5, 0.2);
        let far = band_intensity(0.38, 0.5, 0.2);
        assert!(near > far && far > 0.0);
    }

    #[test]
    fn blend_hsla_hits_both_ends() {
        let ink = Hsla {
            h: 0.0,
            s: 0.0,
            l: 0.6,
            a: 1.0,
        };
        let accent = Hsla {
            h: 0.04,
            s: 0.7,
            l: 0.62,
            a: 1.0,
        };
        assert_eq!(blend_hsla(ink, accent, 0.0), ink);
        assert_eq!(blend_hsla(ink, accent, 1.0), accent);
        // Halfway gains saturation and meets the midpoint lightness.
        let mid = blend_hsla(ink, accent, 0.5);
        assert!(mid.s > ink.s && mid.s < accent.s);
        assert!((mid.l - 0.61).abs() < 1e-6);
    }
}
