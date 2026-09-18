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

#[test]
fn no_placeholder_before_the_first_message_is_sent() {
    // A fresh `new_session` is a draft: nothing sent, nothing listed.
    let store = vec![store_session("old", "/work/alpha")];
    let fresh = PathBuf::from("/store/2026-09-09_new.jsonl");
    let rows = sessions_with_placeholder(
        &store,
        Some(&fresh),
        None,
        Some(Path::new("/work/beta")),
        false,
    );
    assert_eq!(rows.len(), 1, "a draft session must not be listed");
    assert_eq!(rows[0].path, store[0].path);
}

#[test]
fn placeholder_prepends_missing_current_session() {
    // pi flushes a fresh session's file lazily, so the active session is
    // absent from the store right after the first prompt is sent. The
    // sidebar must still show it — instantly, at the top, under its
    // workspace.
    let store = vec![store_session("old", "/work/alpha")];
    let fresh = PathBuf::from("/store/2026-09-09_new.jsonl");
    let rows = sessions_with_placeholder(
        &store,
        Some(&fresh),
        None,
        Some(Path::new("/work/beta")),
        true,
    );
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].path, fresh);
    assert_eq!(rows[0].cwd, PathBuf::from("/work/beta"));
    assert_eq!(rows[0].title, "New Task");
    // The disk rows are untouched behind it.
    assert_eq!(rows[1].path, store[0].path);
}

#[test]
fn placeholder_uses_pis_title_when_already_named() {
    let fresh = PathBuf::from("/store/new.jsonl");
    let rows = sessions_with_placeholder(&[], Some(&fresh), Some("Fix login bug"), None, true);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].title, "Fix login bug");
    assert_eq!(rows[0].first_message, "");
}

#[test]
fn no_placeholder_when_current_session_is_on_disk() {
    let fresh = PathBuf::from("/store/new.jsonl");
    let store = vec![store_session("new", "/work/alpha")];
    let rows = sessions_with_placeholder(
        &store,
        Some(&fresh),
        None,
        Some(Path::new("/work/alpha")),
        true,
    );
    assert_eq!(rows.len(), 1, "duplicate row for the same session");
    assert_eq!(rows[0].id, "new", "the real on-disk row must win");
}

#[test]
fn no_placeholder_without_an_open_session() {
    let store = vec![store_session("old", "/work/alpha")];
    let rows = sessions_with_placeholder(&store, None, None, None, false);
    assert_eq!(rows.len(), 1);
}

#[test]
fn empty_store_with_started_session_is_not_empty() {
    // A brand-new store plus a started session: the sidebar must render
    // the session row, not the "No sessions yet" empty state.
    let rows = sessions_with_placeholder(
        &[],
        Some(&PathBuf::from("/store/new.jsonl")),
        None,
        Some(Path::new("/work/beta")),
        true,
    );
    assert!(!rows.is_empty());
}

#[test]
fn empty_store_with_draft_session_shows_empty_state() {
    // Nothing sent yet: the sidebar renders its empty state, not a row.
    let rows = sessions_with_placeholder(
        &[],
        Some(&PathBuf::from("/store/new.jsonl")),
        None,
        Some(Path::new("/work/beta")),
        false,
    );
    assert!(rows.is_empty());
}

#[test]
fn preview_is_redundant_when_it_repeats_the_title() {
    assert!(sidebar_preview_redundant(
        "fix the login bug",
        "fix the login bug"
    ));
    assert!(sidebar_preview_redundant(
        "fix the login",
        "fix the login bug in auth.rs"
    ));
    assert!(sidebar_preview_redundant("", "   "));
    assert!(!sidebar_preview_redundant(
        "login session",
        "the oauth redirect is looping"
    ));
}
