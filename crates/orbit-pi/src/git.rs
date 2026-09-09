//! Local Git helpers for the status-bar branch picker.

use std::path::Path;
use std::process::Command;

/// Whether `cwd` sits inside a Git work tree.
pub fn is_repo(cwd: &Path) -> bool {
    run_git(cwd, &["rev-parse", "--is-inside-work-tree"])
        .map(|out| out == "true")
        .unwrap_or(false)
}

/// Current branch name, if any.
pub fn current_branch(cwd: &Path) -> Option<String> {
    run_git(cwd, &["branch", "--show-current"])
        .ok()
        .filter(|name| !name.is_empty())
        .or_else(|| read_head_branch(cwd))
}

/// Local branches, newest commit first.
pub fn list_branches(cwd: &Path) -> Result<Vec<String>, String> {
    let out = run_git(
        cwd,
        &[
            "for-each-ref",
            "--sort=-committerdate",
            "refs/heads/",
            "--format=%(refname:short)",
        ],
    )?;
    Ok(out
        .lines()
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect())
}

/// Switch to an existing branch, keeping the working tree when possible.
pub fn checkout_branch(cwd: &Path, branch: &str) -> Result<(), String> {
    if run_git(cwd, &["switch", branch]).is_ok() {
        return Ok(());
    }
    run_git(cwd, &["checkout", branch]).map(|_| ())
}

/// Create a branch from HEAD and check it out, carrying uncommitted changes.
pub fn create_and_checkout_branch(cwd: &Path, name: &str) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("branch name is empty".into());
    }
    if name.contains([' ', '\t', '~', '^', ':', '?', '*', '[']) || name.contains("..") {
        return Err("branch name contains invalid characters".into());
    }
    if run_git(cwd, &["switch", "-c", name]).is_ok() {
        return Ok(());
    }
    run_git(cwd, &["checkout", "-b", name]).map(|_| ())
}

/// Stage all changes, commit with `message`, and push to the remote. Returns
/// an error (instead of a failed `git commit`) when the tree is clean.
pub fn commit_and_push(cwd: &Path, message: &str) -> Result<String, String> {
    let dirty = run_git(cwd, &["status", "--porcelain"])?;
    if dirty.trim().is_empty() {
        return Err("No changes to commit".into());
    }
    run_git(cwd, &["add", "-A"])?;
    run_git(cwd, &["commit", "-m", message])?;
    run_git(cwd, &["push"])?;
    Ok("Committed and pushed".into())
}

fn read_head_branch(cwd: &Path) -> Option<String> {
    let head = std::fs::read_to_string(cwd.join(".git/HEAD")).ok()?;
    let head = head.trim();
    head.strip_prefix("ref: refs/heads/")
        .map(str::to_string)
        .or_else(|| head.get(..7).map(str::to_string))
}

/// Run a git command in `cwd`, returning trimmed stdout. Shared with the
/// Review side pane (see `sidepane.rs`).
pub(crate) fn run_git(cwd: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .map_err(|err| format!("git not available: {err}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
