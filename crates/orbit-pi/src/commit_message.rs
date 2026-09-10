//! Conventional commit message generation.
//!
//! The primary path is a **one-shot, tool-free pi call**: `pi -p --no-tools
//! --no-session …` processes the prompt and exits, so the diff never touches
//! the user's active session and no tool can run in the workspace. If that
//! fails (pi missing, no credentials, timeout), [`heuristic`] derives a plain
//! message from the staged file statuses so the button always produces
//! something editable.

use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const MAX_DIFF_BYTES: usize = 96 * 1024;
const TIMEOUT: Duration = Duration::from_secs(180);

/// Generate a conventional commit message from the staged diff (and the
/// unstaged diff as context). Blocking; call on the background executor.
pub fn generate(
    cwd: &Path,
    provider: Option<&str>,
    model: Option<&str>,
    staged: &str,
    unstaged: &str,
) -> Result<String, String> {
    let prompt = build_prompt(staged, unstaged);
    let raw = run_pi(cwd, provider, model, &prompt)?;
    parse_message(&raw).ok_or_else(|| "pi returned no commit message".to_string())
}

/// A no-model fallback: `type(scope): summary` from the staged statuses.
pub fn heuristic(rows: &[crate::git::StatusRow]) -> String {
    let total = rows.len();
    let mut added = 0usize;
    let mut deleted = 0usize;
    let mut untracked = 0usize;
    for row in rows {
        if row.untracked() {
            untracked += 1;
        } else {
            match row.staged_badge() {
                'A' => added += 1,
                'D' => deleted += 1,
                _ => {}
            }
        }
    }
    let kind = if deleted > 0 && added == 0 && untracked == 0 && total == deleted {
        "chore"
    } else if added > 0 || untracked > 0 {
        "feat"
    } else {
        "chore"
    };
    let subject = format!("update {total} file{}", if total == 1 { "" } else { "s" });
    match common_scope(rows) {
        Some(scope) => format!("{kind}({scope}): {subject}"),
        None => format!("{kind}: {subject}"),
    }
}

fn common_scope(rows: &[crate::git::StatusRow]) -> Option<String> {
    let mut parts = rows
        .iter()
        .map(|row| row.path.split('/').collect::<Vec<_>>());
    let first = parts.next()?;
    if first.len() < 2 {
        return None;
    }
    let mut common = first.len() - 1;
    for parts in parts {
        let mut shared = 0;
        while shared < common && shared + 1 < parts.len() && parts[shared] == first[shared] {
            shared += 1;
        }
        common = shared;
        if common == 0 {
            return None;
        }
    }
    (common >= 1).then(|| first[common - 1].to_string())
}

fn build_prompt(staged: &str, unstaged: &str) -> String {
    let staged = truncate(staged, MAX_DIFF_BYTES);
    let unstaged = truncate(unstaged, MAX_DIFF_BYTES);
    let mut prompt = String::from(
        "Write ONE Conventional Commits message for the staged changes below.\n\
         Rules:\n\
         - First line: `type(scope): imperative summary`, at most 72 characters.\n\
         - type is one of feat, fix, docs, style, refactor, perf, test, build, ci, chore, revert.\n\
         - Add a blank line and a short body only when it adds real information.\n\
         - Output ONLY the commit message: no code fences, quotes, or commentary.\n\n\
         Staged changes:\n",
    );
    prompt.push_str(&staged);
    if !unstaged.trim().is_empty() {
        prompt.push_str("\n\nUnstaged changes (context only, not part of this commit):\n");
        prompt.push_str(&unstaged);
    }
    prompt
}

fn truncate(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_string();
    }
    let mut end = max;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}\n… [diff truncated]", &text[..end])
}

fn run_pi(
    cwd: &Path,
    provider: Option<&str>,
    model: Option<&str>,
    prompt: &str,
) -> Result<String, String> {
    let bin = orbit_rpc::pi_binary();
    let mut command = Command::new(&bin);
    command
        .arg("-p")
        .args([
            "--no-tools",
            "--no-session",
            "--no-extensions",
            "--no-skills",
            "--no-context-files",
            "--no-approve",
        ])
        .env("PI_SKIP_VERSION_CHECK", "1")
        .env("NO_COLOR", "1")
        .env("CI", "1")
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(provider) = provider.filter(|value| !value.is_empty()) {
        command.args(["--provider", provider]);
    }
    if let Some(model) = model.filter(|value| !value.is_empty()) {
        command.args(["--model", model]);
    }
    command.arg("--").arg(prompt);

    let mut child = command
        .spawn()
        .map_err(|err| format!("failed to run {bin}: {err}"))?;
    let stdout = child.stdout.take().ok_or("pi stdout unavailable")?;
    let stderr = child.stderr.take().ok_or("pi stderr unavailable")?;
    let out_handle = std::thread::spawn(move || read_to_end(stdout));
    let err_handle = std::thread::spawn(move || read_to_end(stderr));

    let deadline = Instant::now() + TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let out = out_handle.join().unwrap_or_default();
                let err = err_handle.join().unwrap_or_default();
                return if status.success() {
                    Ok(out)
                } else {
                    Err(format!("pi exited with {status}: {}", first_line(&err)))
                };
            }
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err("commit message generation timed out".into());
                }
                std::thread::sleep(Duration::from_millis(40));
            }
            Err(err) => return Err(format!("pi process error: {err}")),
        }
    }
}

fn read_to_end(mut reader: impl Read) -> String {
    let mut buffer = Vec::new();
    let _ = reader.read_to_end(&mut buffer);
    String::from_utf8_lossy(&buffer).into_owned()
}

fn first_line(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("no error output")
        .to_string()
}

/// Strip an optional code fence and surrounding quotes from the model output.
fn parse_message(raw: &str) -> Option<String> {
    let mut text = raw.trim();
    if text.starts_with("```") {
        if let Some(newline) = text.find('\n') {
            text = &text[newline + 1..];
        }
        if let Some(end) = text.rfind("```") {
            text = &text[..end];
        }
        text = text.trim();
    }
    let text = text.trim_matches('"').trim();
    (!text.is_empty()).then(|| text.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_fences_and_quotes() {
        assert_eq!(
            parse_message("```\nfeat(ui): add git panel\n```").as_deref(),
            Some("feat(ui): add git panel")
        );
        assert_eq!(
            parse_message("\"fix: guard empty input\"").as_deref(),
            Some("fix: guard empty input")
        );
        assert_eq!(parse_message("   \n  "), None);
    }

    #[test]
    fn heuristic_names_the_shared_scope() {
        let rows = vec![
            crate::git::StatusRow {
                path: "crates/orbit-pi/src/a.rs".into(),
                orig_path: None,
                index: 'M',
                worktree: ' ',
                staged_additions: 1,
                staged_deletions: 0,
                unstaged_additions: 0,
                unstaged_deletions: 0,
            },
            crate::git::StatusRow {
                path: "crates/orbit-pi/src/b.rs".into(),
                orig_path: None,
                index: 'A',
                worktree: ' ',
                staged_additions: 3,
                staged_deletions: 0,
                unstaged_additions: 0,
                unstaged_deletions: 0,
            },
        ];
        let message = heuristic(&rows);
        assert!(message.starts_with("feat(src):"), "{message}");
    }
}
