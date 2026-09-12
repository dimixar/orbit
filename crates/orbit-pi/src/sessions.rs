//! Session discovery — reads pi's own session store from disk.
//!
//! Layout: `~/.pi/agent/sessions/<workspace-slug>/<timestamp>_<uuid>.jsonl`
//! where `<workspace-slug>` is the workspace absolute path with `/` → `-`.
//! Each file is JSONL; the first line is the session header
//! (`{"type":"session","id":…,"cwd":…}`) followed by entries
//! (`message`, `model_change`, …). Titles come from the first user message.

use std::{
    fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver},
    time::{Duration, SystemTime},
};

use notify_debouncer_mini::{notify::RecommendedWatcher, Debouncer};
use serde_json::Value;

use crate::watch::debounced_watch;

#[derive(Debug, Clone)]
pub struct SessionInfo {
    /// Path to the `.jsonl` session file (used by `switch_session`).
    pub path: PathBuf,
    #[allow(dead_code)] // useful for future fork/clone wiring
    pub id: String,
    /// Workspace the session belongs to.
    pub cwd: PathBuf,
    /// First user message, truncated — the title shown in the sidebar.
    pub title: String,
    /// First user message preview (longer than the title line).
    pub first_message: String,
    pub modified: SystemTime,
}

/// The default pi session store.
pub fn sessions_dir() -> PathBuf {
    dirs_home().join(".pi/agent/sessions")
}

fn dirs_home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"))
}

/// Load every session in pi's store, newest-*activity* first.
///
/// Ordering is keyed on the session file's last-modified time, so the
/// session (and workspace) with the latest activity always sits at the
/// top of the sidebar and older ones follow — matching the age label
/// each row displays.
pub fn load_sessions() -> Vec<SessionInfo> {
    load_sessions_in(&sessions_dir())
}

/// Load every session under `dir` — the scan body of [`load_sessions`],
/// shared with [`SessionWatcher`] so a watched store reloads the same way.
fn load_sessions_in(dir: &Path) -> Vec<SessionInfo> {
    let mut out = Vec::new();
    let Ok(groups) = fs::read_dir(dir) else {
        return out;
    };
    for group in groups.flatten() {
        let Ok(files) = fs::read_dir(group.path()) else {
            continue;
        };
        for file in files.flatten() {
            let path = file.path();
            if path.extension().is_some_and(|e| e == "jsonl") {
                if let Some(info) = read_session(&path) {
                    out.push(info);
                }
            }
        }
    }
    // Newest activity first, with the path as a deterministic tiebreak.
    out.sort_by(|a, b| {
        b.modified
            .cmp(&a.modified)
            .then_with(|| b.path.cmp(&a.path))
    });
    out
}

/// Only session files matter — header/group-directory events are ignored.
fn is_session_file(path: &Path) -> bool {
    path.extension().is_some_and(|ext| ext == "jsonl")
}

/// Debounced watcher over pi's session store.
///
/// pi (and the CLI, and any other Orbit window) writes session files
/// directly, so the sidebar has no RPC event to hang a refresh on. OS file
/// events are coalesced by the debouncer, then the directory scan runs on a
/// dedicated thread — the app only ever drains a finished list on the
/// heartbeat, so no I/O ever blocks a frame.
pub struct SessionWatcher {
    /// Keeps the debounced backend alive; dropping it stops the watch.
    _debouncer: Debouncer<RecommendedWatcher>,
    reloads: Receiver<Vec<SessionInfo>>,
}

impl SessionWatcher {
    /// Watch the default pi session store. Returns `None` when the store is
    /// absent or the platform backend cannot start; the manual refresh paths
    /// (`cmd-r`, turn settle) still cover that case.
    pub fn start() -> Option<Self> {
        Self::watch(&sessions_dir(), Duration::from_millis(500))
    }

    fn watch(dir: &Path, debounce: Duration) -> Option<Self> {
        let (debouncer, changed) = debounced_watch(dir, debounce, is_session_file)?;

        let (reload_tx, reloads) = mpsc::channel();
        let scan_dir = dir.to_path_buf();
        std::thread::Builder::new()
            .name("orbit-session-watch".into())
            .spawn(move || {
                while changed.recv().is_ok() {
                    // Collapse a burst of appends into one scan.
                    while changed.try_recv().is_ok() {}
                    if reload_tx.send(load_sessions_in(&scan_dir)).is_err() {
                        break;
                    }
                }
            })
            .ok()?;

        Some(Self {
            _debouncer: debouncer,
            reloads,
        })
    }

    /// The newest session snapshot when the store changed since the last
    /// call; `None` when nothing new landed.
    pub fn take_reload(&self) -> Option<Vec<SessionInfo>> {
        let mut latest = None;
        while let Ok(sessions) = self.reloads.try_recv() {
            latest = Some(sessions);
        }
        latest
    }
}

fn read_session(path: &Path) -> Option<SessionInfo> {
    let file = fs::File::open(path).ok()?;
    let mut reader = BufReader::new(file);
    let modified = fs::metadata(path).ok()?.modified().ok()?;

    // First line must be the session header.
    let mut header_line = String::new();
    reader.read_line(&mut header_line).ok()?;
    let header: Value = serde_json::from_str(header_line.trim()).ok()?;
    if header.get("type")?.as_str()? != "session" {
        return None;
    }
    let id = header.get("id")?.as_str()?.to_string();
    let cwd = PathBuf::from(header.get("cwd")?.as_str()?);

    // Scan a bounded number of lines for the first user message → title
    // and preview.
    let mut title = String::new();
    let mut first_message = String::new();
    for _ in 0..60 {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                let Ok(value) = serde_json::from_str::<Value>(line.trim()) else {
                    continue;
                };
                if value.get("type")?.as_str() == Some("message") {
                    let message = &value["message"];
                    if message["role"].as_str() == Some("user") {
                        let text = first_user_text(message);
                        title = cap_chars(&text, 80);
                        first_message = cap_chars(&text, 110);
                        break;
                    }
                }
            }
        }
    }
    // A header-only file is a draft: pi writes it at `new_session` time,
    // before anything is sent. Don't list it (Waku drafts parity) — the
    // session joins the sidebar once its first user message lands.
    if title.is_empty() {
        return None;
    }

    Some(SessionInfo {
        path: path.to_path_buf(),
        id,
        cwd,
        title,
        first_message,
        modified,
    })
}

fn cap_chars(text: &str, max: usize) -> String {
    let trimmed = text.trim();
    let mut out: String = trimmed.chars().take(max).collect();
    if trimmed.chars().count() > max {
        out.push('…');
    }
    out
}

/// Extract the first text block from a user message's content, as one line.
fn first_user_text(message: &Value) -> String {
    let content = &message["content"];
    let text = match content {
        Value::String(s) => s.clone(),
        Value::Array(blocks) => blocks
            .iter()
            .find(|b| b.get("type").and_then(Value::as_str) == Some("text"))
            .and_then(|b| b.get("text").and_then(Value::as_str))
            .unwrap_or("")
            .to_string(),
        _ => String::new(),
    };
    text.replace('\n', " ")
}

/// Basename of a workspace path, used as a group label in the sidebar.
pub fn workspace_label(cwd: &Path) -> String {
    cwd.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| cwd.to_string_lossy().to_string())
}

/// Compact age label for a session row (`2m`, `2h`, `1d`, `2mo`, `1y`).
pub fn relative_time(modified: SystemTime) -> String {
    let secs = SystemTime::now()
        .duration_since(modified)
        .unwrap_or_default()
        .as_secs();
    match secs {
        0..=59 => "now".into(),
        60..=3599 => format!("{}m", secs / 60),
        3600..=86_399 => format!("{}h", secs / 3600),
        86_400..=2_591_999 => format!("{}d", secs / 86_400),
        2_592_000..=31_535_999 => format!("{}mo", secs / 2_592_000),
        _ => format!("{}y", secs / 31_536_000),
    }
}

/// Coarse age bucket (kept for potential future time-grouped views).
#[allow(dead_code)]
pub fn time_bucket(modified: SystemTime) -> &'static str {
    let hours = SystemTime::now()
        .duration_since(modified)
        .unwrap_or_default()
        .as_secs()
        / 3600;
    match hours {
        0..=23 => "Today",
        24..=47 => "Yesterday",
        48..=167 => "This Week",
        168..=719 => "This Month",
        _ => "Older",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_dir_exists_on_dev_machine() {
        // The dev machine runs pi; the store should exist.
        assert!(
            sessions_dir().exists(),
            "no pi session store at {:?}",
            sessions_dir()
        );
    }

    #[test]
    fn load_sessions_returns_real_sessions() {
        let sessions = load_sessions();
        assert!(!sessions.is_empty(), "expected sessions in pi store");
        // Newest-activity first, stable across reloads: modified times
        // descend, and only a session with newer activity may join the front.
        assert!(sessions.windows(2).all(|w| w[0].modified >= w[1].modified));
        assert!(sessions[0].id.len() > 10);
    }

    #[test]
    fn header_only_session_is_a_draft_and_not_listed() {
        // pi writes the session file at `new_session` time; it must not show
        // up in the sidebar until the first user message lands.
        let dir = std::env::temp_dir().join("orbit-draft-session-test");
        fs::create_dir_all(&dir).unwrap();
        let draft = dir.join("2026-01-01T00-00-00-000Z_draft.jsonl");
        fs::write(
            &draft,
            "{\"type\":\"session\",\"id\":\"draft\",\"cwd\":\"/tmp/ws\"}\n",
        )
        .unwrap();
        assert!(read_session(&draft).is_none());
        // Once a user message lands, the session is listable.
        fs::write(
            &draft,
            "{\"type\":\"session\",\"id\":\"draft\",\"cwd\":\"/tmp/ws\"}\n\
             {\"type\":\"message\",\"message\":{\"role\":\"user\",\"content\":\"hi\"}}\n",
        )
        .unwrap();
        assert_eq!(read_session(&draft).map(|s| s.title).as_deref(), Some("hi"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_sessions_orders_by_modified_not_creation() {
        // Two sessions whose creation stamps and modified times disagree:
        // the older-created but recently-touched one must come first.
        let dir = std::env::temp_dir().join("orbit-modified-order-test");
        fs::create_dir_all(&dir).unwrap();
        let older_created = dir.join("2026-01-01T00-00-00-000Z_aaa.jsonl");
        let newer_created = dir.join("2026-02-01T00-00-00-000Z_bbb.jsonl");
        for (path, id) in [(&older_created, "aaa"), (&newer_created, "bbb")] {
            fs::write(
                path,
                format!(
                    "{{\"type\":\"session\",\"id\":\"{id}\",\"cwd\":\"/tmp/ws\"}}\n\
                     {{\"type\":\"message\",\"message\":{{\"role\":\"user\",\"content\":\"hi\"}}}}\n"
                ),
            )
            .unwrap();
        }
        // Touch the older-created file so its mtime is now the newest.
        let new_time = std::time::SystemTime::now() + std::time::Duration::from_secs(10);
        fs::File::options()
            .write(true)
            .open(&older_created)
            .unwrap()
            .set_modified(new_time)
            .unwrap();

        // Scan just this directory with the same logic load_sessions uses.
        let mut rows: Vec<SessionInfo> = Vec::new();
        for file in fs::read_dir(&dir).unwrap().flatten() {
            let path = file.path();
            if path.extension().is_some_and(|e| e == "jsonl") {
                if let Some(info) = read_session(&path) {
                    rows.push(info);
                }
            }
        }
        rows.sort_by(|a, b| {
            b.modified
                .cmp(&a.modified)
                .then_with(|| b.path.cmp(&a.path))
        });
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].path, older_created);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn session_watcher_reloads_a_changed_store() {
        let dir = std::env::temp_dir().join("orbit-session-watch-test");
        let _ = fs::remove_dir_all(&dir);
        // Mirror pi's store: `<store>/<workspace-slug>/<timestamp>.jsonl`.
        fs::create_dir_all(dir.join("ws")).unwrap();
        let write = |name: &str, title: &str| {
            fs::write(
                dir.join("ws").join(name),
                format!(
                    "{{\"type\":\"session\",\"id\":\"{title}\",\"cwd\":\"/tmp/ws\"}}\n\
                     {{\"type\":\"message\",\"message\":{{\"role\":\"user\",\"content\":\"{title}\"}}}}\n"
                ),
            )
            .unwrap();
        };
        write("2026-01-01T00-00-00-000Z_a.jsonl", "hi");

        let watcher =
            SessionWatcher::watch(&dir, Duration::from_millis(50)).expect("watcher starts");
        write("2026-01-02T00-00-00-000Z_b.jsonl", "yo");

        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        let mut reloaded = None;
        while std::time::Instant::now() < deadline {
            if let Some(sessions) = watcher.take_reload() {
                reloaded = Some(sessions);
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let sessions = reloaded.expect("watcher reported the new session");
        assert_eq!(sessions.len(), 2);
        assert!(sessions.iter().any(|s| s.title == "yo"));
        let _ = fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod debug_tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    #[ignore] // manual: cargo test -p orbit-pi debug_groups -- --ignored --nocapture
    fn debug_groups() {
        let sessions = load_sessions();
        let mut groups: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
        for s in &sessions {
            groups.entry(workspace_label(&s.cwd)).or_default().push((
                relative_time(s.modified),
                s.title.chars().take(40).collect(),
            ));
        }
        let mut total = 0;
        for (label, rows) in &groups {
            total += rows.len();
            println!("{} ({}):", label, rows.len());
            for (t, title) in rows.iter().take(2) {
                println!("   [{}] {}", t, title);
            }
        }
        println!(
            "TOTAL sessions rendered: {} across {} groups",
            total,
            groups.len()
        );
        assert_eq!(total, sessions.len(), "grouping lost sessions!");
    }
}
