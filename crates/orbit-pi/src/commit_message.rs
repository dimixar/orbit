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

use crate::git::StatusRow;

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
    rows: &[StatusRow],
) -> Result<String, String> {
    let recent: Vec<String> = crate::git::history(cwd, 10, 0)
        .unwrap_or_default()
        .into_iter()
        .map(|commit| commit.subject)
        .collect();
    let prompt = build_prompt(staged, unstaged, rows, &recent);
    let raw = run_pi(cwd, provider, model, &prompt)?;
    let message = parse_message(&raw).ok_or_else(|| "pi returned no commit message".to_string())?;
    Ok(normalize(&message))
}

/// A no-model fallback: a Conventional Commit `type(scope): description` plus a
/// bulleted body built from the changed file names — never a bare file count.
pub fn heuristic(rows: &[StatusRow]) -> String {
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
    let verb = if total > 0 && deleted == total {
        "remove"
    } else if total > 0 && added + untracked == total {
        "add"
    } else {
        "update"
    };
    let description = describe_change(rows, verb);
    let subject = match common_scope(rows) {
        Some(scope) => format!("{kind}({scope}): {description}"),
        None => format!("{kind}: {description}"),
    };
    match describe_body(rows) {
        Some(body) => format!("{subject}\n\n{body}"),
        None => subject,
    }
}

/// The fallback's subject tail: the changed areas named in prose, so it reads
/// as a description (`update git panel and commit message`) rather than a count.
fn describe_change(rows: &[StatusRow], verb: &str) -> String {
    let names = distinct_names(rows);
    match names.len() {
        0 => format!("{verb} files"),
        1 => format!("{verb} {}", names[0]),
        _ => format!("{verb} {} and {}", names[0], names[1]),
    }
}

/// The fallback's body: one past-tense bullet per change group, matching the
/// generated shape (subject, blank line, then `- ` bullets ending with a
/// period).
fn describe_body(rows: &[StatusRow]) -> Option<String> {
    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut changed = Vec::new();
    for row in rows {
        let name = humanize(&row.path);
        match if row.untracked() {
            'A'
        } else {
            row.staged_badge()
        } {
            'A' => added.push(name),
            'D' => removed.push(name),
            _ => changed.push(name),
        }
    }
    let mut lines: Vec<String> = Vec::new();
    if !changed.is_empty() {
        lines.push(format!("- Updated {}.", join_names(&changed)));
    }
    if !added.is_empty() {
        lines.push(format!("- Added {}.", join_names(&added)));
    }
    if !removed.is_empty() {
        lines.push(format!("- Removed {}.", join_names(&removed)));
    }
    if lines.is_empty() {
        return None;
    }
    Some(lines.join("\n"))
}

/// Distinct human-readable names, in first-seen order.
fn distinct_names(rows: &[StatusRow]) -> Vec<String> {
    let mut names = Vec::new();
    for row in rows {
        let name = humanize(&row.path);
        if !names.contains(&name) {
            names.push(name);
        }
    }
    names
}

/// A file path reduced to a spaced, human word (`commit_message.rs` →
/// `commit message`), falling back to the raw path when there is no stem.
fn humanize(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .map(|stem| stem.to_string_lossy().replace(['_', '-'], " "))
        .unwrap_or_else(|| path.to_string())
}

/// `a, b, and c` — an Oxford-comma list for the fallback prose.
fn join_names(names: &[String]) -> String {
    match names {
        [] => String::new(),
        [one] => one.clone(),
        [a, b] => format!("{a} and {b}"),
        [rest @ .., last] => format!("{}, and {last}", rest.join(", ")),
    }
}

/// One fallback summary line: `M path (+n -m)`, untracked files marked `U`.
fn describe_row(row: &StatusRow) -> String {
    let badge = if row.untracked() {
        'U'
    } else {
        row.staged_badge()
    };
    let (adds, dels) = if row.untracked() || row.staged_additions + row.staged_deletions == 0 {
        (row.unstaged_additions, row.unstaged_deletions)
    } else {
        (row.staged_additions, row.staged_deletions)
    };
    let stat = if adds > 0 || dels > 0 {
        format!(" (+{adds} -{dels})")
    } else {
        String::new()
    };
    format!("{badge} {}{stat}", row.path)
}

fn common_scope(rows: &[StatusRow]) -> Option<String> {
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

/// A compact, ordered list of the staged files with their change kind and line
/// deltas. Gives the model the file-level map even when the raw diff is
/// truncated, and pairs with the diff so the body can name what actually
/// changed.
fn staged_summary(rows: &[StatusRow]) -> String {
    rows.iter()
        .map(|row| format!("  {}", describe_row(row)))
        .collect::<Vec<_>>()
        .join("\n")
}

fn build_prompt(staged: &str, unstaged: &str, rows: &[StatusRow], recent: &[String]) -> String {
    let staged = truncate(staged, MAX_DIFF_BYTES);
    let unstaged = truncate(unstaged, MAX_DIFF_BYTES);
    let mut prompt = String::from(
        "Analyze the staged changes below and write ONE commit message.\n\
         The changed-files list and the unified diff together are the source of truth.\n\
         Return the message in exactly this shape — a subject line, a blank line, then a\n\
         bulleted list with one line per change:\n\n\
         <type>: <summary>\n\n\
         - Past-tense sentence describing one change.\n\
         - Past-tense sentence describing another change.\n\n\
         Rules:\n\
         - Inspect the actual diff before generating the message.\n\
         - Identify the primary purpose of the changes.\n\
         - Subject: one line, concise and preferably under 72 characters, lowercase imperative\n\
           language (\"add\", not \"added\" or \"adds\"), with no trailing period.\n\
         - Match the type prefix and scope style of the recent commits below when they are consistent.\n\
         - When there is no consistent style, use <type>: <summary>, where type is one of feat, fix,\n\
           refactor, perf, ui, style, docs, test, build, chore, ci, revert.\n\
         - Body: one `- ` bullet per user-facing change, each a complete sentence starting with a\n\
           capital letter, written in the past tense (\"Added\", \"Updated\", \"Fixed\"), and ending\n\
           with a period.\n\
         - Start every body line with `- `; never write body prose without the bullet marker.\n\
         - Do not repeat the subject in the body, and never report a file count or list files line\n\
           by line.\n\
         - Do not include issue numbers unless they are present in the changes/context.\n\
         - Do not invent functionality that is not present in the diff.\n\
         - If changes contain unrelated work, describe the dominant change rather than listing everything.\n\
         - Omit the body only for a single trivial edit.\n\
         - Return ONLY the commit message: no code fences, quotes, or commentary.\n\n\
         Recent commits on this branch (newest first):\n",
    );
    if recent.is_empty() {
        prompt.push_str("(none)\n");
    } else {
        for subject in recent {
            prompt.push_str(subject);
            prompt.push('\n');
        }
    }
    prompt.push_str("\nStaged files:\n");
    prompt.push_str(&staged_summary(rows));
    prompt.push_str("\n\nStaged diff:\n");
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
    orbit_rpc::hide_console(&mut command);

    let mut child = command
        .spawn()
        .map_err(|err| tr!("errors.failed_to_run", bin = bin, error = err))?;
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
                    Err(tr!(
                        "errors.pi_exited_with",
                        status = status.to_string(),
                        stderr = first_line(&err)
                    ))
                };
            }
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(tr!("errors.commit_generation_timed_out"));
                }
                std::thread::sleep(Duration::from_millis(40));
            }
            Err(err) => return Err(tr!("errors.pi_process_error", error = err)),
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

/// Force the model output into the app's commit shape regardless of how closely
/// it followed instructions: one subject line, a blank line, then past-tense
/// body bullets that each start with `- ` and end with a period.
fn normalize(message: &str) -> String {
    let mut lines = message
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty());
    let Some(subject) = lines.next() else {
        return String::new();
    };
    let subject = strip_bullet(subject).trim_end_matches('.').trim();
    let mut body: Vec<String> = Vec::new();
    for line in lines {
        let line = strip_bullet(line).trim();
        if line.is_empty() {
            continue;
        }
        let mut sentence = capitalize(line);
        if !sentence.ends_with(['.', '!', '?']) {
            sentence.push('.');
        }
        body.push(format!("- {sentence}"));
    }
    if body.is_empty() {
        subject.to_string()
    } else {
        format!("{subject}\n\n{}", body.join("\n"))
    }
}

/// Remove a leading `- `, `* `, `• `, or `1.`/`1)` list marker.
fn strip_bullet(line: &str) -> &str {
    let trimmed = line
        .trim_start_matches(['-', '*', '•', '–', '—'])
        .trim_start();
    let digits = trimmed.len()
        - trimmed
            .trim_start_matches(|c: char| c.is_ascii_digit())
            .len();
    if digits > 0 {
        let rest = &trimmed[digits..];
        if let Some(rest) = rest.strip_prefix('.').or_else(|| rest.strip_prefix(')')) {
            return rest.trim_start();
        }
    }
    trimmed
}

fn capitalize(line: &str) -> String {
    let mut chars = line.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
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

    #[test]
    fn heuristic_reads_as_a_conventional_description() {
        let rows = vec![
            crate::git::StatusRow {
                path: "crates/orbit-pi/src/git_panel.rs".into(),
                orig_path: None,
                index: 'M',
                worktree: ' ',
                staged_additions: 4,
                staged_deletions: 1,
                unstaged_additions: 0,
                unstaged_deletions: 0,
            },
            crate::git::StatusRow {
                path: "crates/orbit-pi/src/commit_message.rs".into(),
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
        // Conventional subject with a description, not a file count.
        assert_eq!(
            message.lines().next().unwrap(),
            "feat(src): update git panel and commit message"
        );
        // One past-tense bullet per change group, each ending with a period.
        assert!(
            message.contains("\n\n- Updated git panel.\n- Added commit message."),
            "{message}"
        );
        assert!(!message.contains("- M "), "{message}");
        assert!(!message.contains("2 files"), "{message}");
    }

    #[test]
    fn normalizes_bullets_and_trailing_periods_into_the_commit_shape() {
        let raw = "- feat: animate the sidebar\n\n* the sidebar now slides open\n1. the toggle floats above\nGit status no longer marks unstaged edits as staged.";
        assert_eq!(
            normalize(raw),
            "feat: animate the sidebar\n\n\
             - The sidebar now slides open.\n\
             - The toggle floats above.\n\
             - Git status no longer marks unstaged edits as staged."
        );
    }

    #[test]
    fn normalize_keeps_a_subject_only_message() {
        assert_eq!(
            normalize("fix: guard empty input."),
            "fix: guard empty input"
        );
        assert_eq!(normalize("   \n "), "");
    }

    #[test]
    fn prompt_carries_the_file_summary_and_anti_count_rule() {
        let rows = vec![crate::git::StatusRow {
            path: "src/lib.rs".into(),
            orig_path: None,
            index: 'M',
            worktree: ' ',
            staged_additions: 2,
            staged_deletions: 0,
            unstaged_additions: 0,
            unstaged_deletions: 0,
        }];
        let recent = vec!["feat: add the review pane".to_string()];
        let prompt = build_prompt("diff --git a/src/lib.rs b/src/lib.rs\n", "", &rows, &recent);
        assert!(prompt.contains("Staged files:"), "{prompt}");
        assert!(prompt.contains("  M src/lib.rs (+2 -0)"), "{prompt}");
        assert!(prompt.contains("never report a file count"), "{prompt}");
        assert!(prompt.contains("Staged diff:"), "{prompt}");
        // New rule set: subject + blank line + one bullet per change, style matching.
        assert!(prompt.contains("<type>: <summary>"), "{prompt}");
        assert!(prompt.contains("ui"), "{prompt}");
        assert!(
            prompt.contains("bulleted list with one line per change"),
            "{prompt}"
        );
        assert!(prompt.contains("- Past-tense sentence"), "{prompt}");
        assert!(prompt.contains("past tense"), "{prompt}");
        assert!(prompt.contains("Recent commits on this branch"), "{prompt}");
        assert!(prompt.contains("feat: add the review pane"), "{prompt}");
        assert!(prompt.contains("Do not invent functionality"), "{prompt}");
        assert!(prompt.contains("dominant change"), "{prompt}");
    }
}
