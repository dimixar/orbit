use super::sidebar::*;
use super::*;

fn store_session(name: &str, cwd: &str) -> SessionInfo {
    SessionInfo {
        path: PathBuf::from(format!("/store/{name}.jsonl")),
        id: name.into(),
        cwd: PathBuf::from(cwd),
        title: format!("{name} title"),
        first_message: "preview".into(),
        modified: SystemTime::UNIX_EPOCH,
    }
}

fn session_row_paths(rows: &[SideRow], sessions: &[SessionInfo]) -> Vec<PathBuf> {
    rows.iter()
        .filter_map(|r| match r {
            SideRow::Session(ix) => Some(sessions[*ix].path.clone()),
            _ => None,
        })
        .collect()
}

fn workspace_paths(paths: &[&str]) -> Vec<PathBuf> {
    paths.iter().map(PathBuf::from).collect()
}

#[test]
fn collapsed_workspace_pins_the_open_session() {
    // "alpha" is the working workspace (expanded by default); "beta" is
    // a foreign workspace, collapsed unless explicitly expanded. It
    // holds the open session: the group stays collapsed (siblings
    // hidden) but the open session's row stays pinned under the header.
    let sessions = vec![
        store_session("a1", "/work/alpha"),
        store_session("b1", "/work/beta"),
        store_session("b2", "/work/beta"),
    ];
    let active = Some(sessions[1].path.clone());
    let rows = build_sidebar_rows(
        &sessions,
        &workspace_paths(&["/work/alpha", "/work/beta"]),
        "alpha",
        &HashSet::new(),
        &HashSet::new(),
        &HashSet::new(),
        &HashSet::new(),
        &active,
    );
    let visible = session_row_paths(&rows, &sessions);
    assert!(
        visible.contains(&sessions[1].path),
        "the open session stays visible in a collapsed workspace"
    );
    assert!(
        !visible.contains(&sessions[2].path),
        "its siblings stay hidden while the group is collapsed"
    );
    let beta_header = rows.iter().find_map(|r| match r {
        SideRow::Workspace {
            label, collapsed, ..
        } if label == "beta" => Some(*collapsed),
        _ => None,
    });
    assert_eq!(
        beta_header,
        Some(true),
        "the group header still reads as collapsed"
    );
}

#[test]
fn collapsed_workspace_without_the_open_session_stays_closed() {
    let sessions = vec![
        store_session("a1", "/work/alpha"),
        store_session("b1", "/work/beta"),
    ];
    let active = Some(sessions[0].path.clone());
    let rows = build_sidebar_rows(
        &sessions,
        &workspace_paths(&["/work/alpha", "/work/beta"]),
        "alpha",
        &HashSet::new(),
        &HashSet::new(),
        &HashSet::new(),
        &HashSet::new(),
        &active,
    );
    assert!(
        !session_row_paths(&rows, &sessions).contains(&sessions[1].path),
        "a collapsed foreign workspace keeps its sessions hidden"
    );
}

#[test]
fn manually_collapsed_working_workspace_pins_the_open_session() {
    // The user collapsed their own workspace by hand — the open session
    // still shows under the header; expanding brings back the rest.
    let sessions = vec![
        store_session("a1", "/work/alpha"),
        store_session("a2", "/work/alpha"),
    ];
    let active = Some(sessions[0].path.clone());
    let mut collapsed = HashSet::new();
    collapsed.insert("alpha".to_string());
    let rows = build_sidebar_rows(
        &sessions,
        &workspace_paths(&["/work/alpha"]),
        "alpha",
        &collapsed,
        &HashSet::new(),
        &HashSet::new(),
        &HashSet::new(),
        &active,
    );
    let visible = session_row_paths(&rows, &sessions);
    assert!(
        visible.contains(&sessions[0].path),
        "the open session stays visible even when its workspace was collapsed by hand"
    );
    assert!(
        !visible.contains(&sessions[1].path),
        "the other sessions stay hidden until the header is expanded"
    );
}

#[test]
fn unlisted_workspace_stays_out_of_the_sidebar() {
    // Orbit owns the project list: a folder pi has sessions in is omitted
    // until the user adds it. Its sessions stay on disk, untouched.
    let sessions = vec![
        store_session("a1", "/work/alpha"),
        store_session("b1", "/work/beta"),
    ];
    let rows = build_sidebar_rows(
        &sessions,
        &workspace_paths(&["/work/alpha"]),
        "alpha",
        &HashSet::new(),
        &HashSet::new(),
        &HashSet::new(),
        &HashSet::new(),
        &None,
    );
    assert!(
        !rows
            .iter()
            .any(|r| matches!(r, SideRow::Workspace { label, .. } if label == "beta")),
        "an unlisted workspace leaves no header behind"
    );
    assert!(
        !session_row_paths(&rows, &sessions).contains(&sessions[1].path),
        "an unlisted workspace's sessions stay out of the sidebar"
    );
}

#[test]
fn listed_workspace_without_sessions_gets_a_header() {
    // A project with no sessions yet still lists, so a task can be started
    // there from its `+`.
    let sessions = vec![store_session("a1", "/work/alpha")];
    let rows = build_sidebar_rows(
        &sessions,
        &workspace_paths(&["/work/alpha", "/work/empty"]),
        "alpha",
        &HashSet::new(),
        &HashSet::new(),
        &HashSet::new(),
        &HashSet::new(),
        &None,
    );
    assert!(
        rows.iter().any(|r| matches!(
            r,
            SideRow::Workspace { label, count, cwd, .. }
                if label == "empty" && *count == 0 && cwd == &PathBuf::from("/work/empty")
        )),
        "an empty listed workspace still gets a header with its cwd"
    );
}

#[test]
fn groups_follow_the_project_list_order() {
    // The sidebar is Orbit's own folder listing: groups keep the order the
    // user added them, not newest-session order.
    let sessions = vec![
        store_session("b1", "/work/beta"),
        store_session("a1", "/work/alpha"),
    ];
    let rows = build_sidebar_rows(
        &sessions,
        &workspace_paths(&["/work/alpha", "/work/beta"]),
        "none",
        &HashSet::new(),
        &HashSet::new(),
        &HashSet::new(),
        &HashSet::new(),
        &None,
    );
    let labels: Vec<&str> = rows
        .iter()
        .filter_map(|r| match r {
            SideRow::Workspace { label, .. } => Some(label.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(labels, vec!["alpha", "beta"]);
}

#[test]
fn pinned_sessions_lead_their_group_and_beat_truncation() {
    // More sessions in the group than fit collapsed. The pinned session is
    // the oldest row, so without pinning it would be truncated away; the pin
    // must lift it to the top of its group and keep it visible, while the
    // rest stay in recency order.
    let sessions: Vec<SessionInfo> = (0..SIDEBAR_GROUP_SESSIONS_VISIBLE + 2)
        .map(|i| store_session(&format!("a{i}"), "/work/alpha"))
        .collect();
    let oldest = sessions.last().unwrap().path.clone();
    let pinned: HashSet<PathBuf> = HashSet::from([oldest.clone()]);
    let rows = build_sidebar_rows(
        &sessions,
        &workspace_paths(&["/work/alpha"]),
        "alpha",
        &HashSet::new(),
        &HashSet::new(),
        &HashSet::new(),
        &pinned,
        &None,
    );
    let visible = session_row_paths(&rows, &sessions);
    assert_eq!(visible.first(), Some(&oldest), "the pin leads the group");
    assert_eq!(
        visible.len(),
        SIDEBAR_GROUP_SESSIONS_VISIBLE,
        "the group still shows its visible cap"
    );
    assert_eq!(
        visible[1], sessions[0].path,
        "unpinned rows keep recency order after the pin"
    );
}

#[test]
fn pinned_sessions_stay_visible_in_a_collapsed_workspace() {
    // Foreign projects are collapsed by default; a pin is a deliberate mark
    // and must survive that — under its header, alongside the open row.
    let sessions = vec![
        store_session("b1", "/work/beta"),
        store_session("b2", "/work/beta"),
    ];
    let pinned: HashSet<PathBuf> = HashSet::from([sessions[1].path.clone()]);
    let rows = build_sidebar_rows(
        &sessions,
        &workspace_paths(&["/work/alpha", "/work/beta"]),
        "alpha",
        &HashSet::new(),
        &HashSet::new(),
        &HashSet::new(),
        &pinned,
        &None,
    );
    let visible = session_row_paths(&rows, &sessions);
    assert_eq!(visible, vec![sessions[1].path.clone()]);
}
