//! Local Git helpers: branch discovery for the status bar and the Review
//! panel's diff collection.
//!
//! Everything here is pure I/O and stays off the UI thread. The workspace is
//! compared through [`crate::checkpoint`] snapshots so untracked files, stage
//! state, and per-turn checkpoints are represented exactly.

use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::Path;
use std::process::{Command, Output};

use anyhow::{anyhow, bail};

use crate::checkpoint::{self, EMPTY_TREE};
use crate::review::Source;

/// One `git diff` capture: numstat summary plus the (possibly full-context)
/// patch text.
#[derive(Debug, Clone)]
pub struct ReviewDiff {
    pub numstat: String,
    pub patch: String,
    /// Whether the patch carries complete file context (enables gap
    /// expansion). False when the hydrated patch exceeded the safety cap and
    /// Git was re-run with 3 lines of context.
    pub complete_context: bool,
}

const MAX_HYDRATED_PATCH_BYTES: usize = 32 * 1024 * 1024;

/// A `git` invocation rooted at `cwd`, inheriting the environment.
pub(crate) fn command(cwd: &Path) -> Command {
    let mut command = Command::new("git");
    command.current_dir(cwd);
    command
}

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

/// Commit the staged changes with `message`. With `include_unstaged`, stage
/// everything first. Returns a short success note.
pub fn commit(cwd: &Path, message: &str, include_unstaged: bool) -> Result<String, String> {
    let message = message.trim();
    if message.is_empty() {
        return Err("enter a commit message".into());
    }
    if include_unstaged {
        run_git(cwd, &["add", "-A", "--", "."])?;
    } else {
        let staged = run_git(cwd, &["diff", "--cached", "--name-only"]).unwrap_or_default();
        if staged.trim().is_empty() {
            return Err("no staged changes to commit".into());
        }
    }
    run_git(cwd, &["commit", "-m", message])?;
    Ok("Committed".into())
}

/// Push the current branch, setting its upstream on the first push.
pub fn push(cwd: &Path) -> Result<String, String> {
    // With an upstream, a plain push is the only correct command (a failure is
    // auth/network, not a missing upstream).
    if run_git(cwd, &["rev-parse", "--abbrev-ref", "@{upstream}"]).is_ok() {
        run_git(cwd, &["push"])?;
        return Ok("Pushed".into());
    }
    match current_branch(cwd) {
        Some(branch) => {
            run_git(cwd, &["push", "-u", "origin", &branch])?;
            Ok("Pushed and set upstream".into())
        }
        None => Err("no branch to push".into()),
    }
}

fn read_head_branch(cwd: &Path) -> Option<String> {
    let head = std::fs::read_to_string(cwd.join(".git/HEAD")).ok()?;
    let head = head.trim();
    head.strip_prefix("ref: refs/heads/")
        .map(str::to_string)
        .or_else(|| head.get(..7).map(str::to_string))
}

/// Run a git command in `cwd`, returning trimmed stdout. Shared with the
/// branch picker and the Review side pane.
pub(crate) fn run_git(cwd: &Path, args: &[&str]) -> Result<String, String> {
    let output = command(cwd)
        .args(args)
        .output()
        .map_err(|err| format!("git not available: {err}"))?;
    if !output.status.success() {
        return Err(command_error(&output));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Run a git command, returning raw (untrimmed) stdout, or an error.
pub(crate) fn run_git_ok<I, S>(cwd: &Path, args: I) -> anyhow::Result<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = command(cwd).args(args).output()?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        bail!("{}", command_error(&output));
    }
}

fn command_error(output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    if stderr.is_empty() {
        format!("git exited with {}", output.status)
    } else {
        stderr
    }
}

// ── Review diff collection ─────────────────────────────────────────────────

/// Collect the workspace changes for `source` and parse them for render.
pub fn collect_review_diff(
    cwd: &Path,
    source: Source,
    session: Option<&str>,
) -> Result<ReviewDiff, String> {
    collect_review_diff_inner(cwd, source, session).map_err(|err| err.to_string())
}

fn collect_review_diff_inner(
    cwd: &Path,
    source: Source,
    session: Option<&str>,
) -> anyhow::Result<ReviewDiff> {
    ensure_repository(cwd)?;
    let (from, to) = resolve_range(cwd, source, session)?;
    let numstat = diff_output(cwd, &from, &to, &["--numstat"])?;
    let hydrated = diff_output(cwd, &from, &to, &["--unified=2147483647"])?;
    let (patch, complete_context) = if hydrated.len() <= MAX_HYDRATED_PATCH_BYTES {
        (hydrated, true)
    } else {
        (diff_output(cwd, &from, &to, &["--unified=3"])?, false)
    };
    Ok(ReviewDiff {
        numstat,
        patch,
        complete_context,
    })
}

fn resolve_range(
    cwd: &Path,
    source: Source,
    session: Option<&str>,
) -> anyhow::Result<(String, String)> {
    let head = head_or_empty(cwd);
    Ok(match source {
        Source::LastTurn { turn_count } => {
            let session =
                session.ok_or_else(|| anyhow!("no session is open for turn checkpoints"))?;
            if turn_count == 0 {
                bail!("the first checkpoint is a baseline, not a completed turn");
            }
            let from = [
                checkpoint::turn_diff_base_ref(session, turn_count),
                checkpoint::turn_start_ref(session, turn_count),
                checkpoint::checkpoint_ref(session, turn_count - 1),
            ]
            .into_iter()
            .find_map(|rev| checkpoint::resolve(cwd, &rev))
            .ok_or_else(|| anyhow!("the turn's starting checkpoint is unavailable"))?;
            let to = checkpoint::resolve(cwd, &checkpoint::checkpoint_ref(session, turn_count))
                .ok_or_else(|| anyhow!("the turn's ending checkpoint is unavailable"))?;
            (from, to)
        }
        Source::Uncommitted => (head, checkpoint::capture_worktree_commit(cwd)?),
        Source::Unstaged => (index_tree(cwd)?, checkpoint::capture_worktree_commit(cwd)?),
        Source::Staged => (head, index_tree(cwd)?),
        Source::Committed => (branch_base(cwd), head_or_empty(cwd)),
        Source::Branch => (branch_base(cwd), checkpoint::capture_worktree_commit(cwd)?),
    })
}

/// The tree the index would write — the "staged" snapshot. Does not modify
/// the index or the working tree.
fn index_tree(cwd: &Path) -> anyhow::Result<String> {
    let output = command(cwd)
        .args(["write-tree"])
        .output()
        .map_err(|err| anyhow!("failed to snapshot the Git index: {err}"))?;
    if output.status.success() {
        let tree = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        if !tree.is_empty() {
            return Ok(tree);
        }
    }
    // An unborn branch has no HEAD but may still have a usable empty tree.
    if checkpoint::has_head(cwd) {
        bail!("{}", command_error(&output));
    }
    Ok(EMPTY_TREE.to_owned())
}

/// The merge base of `HEAD` and the repository's default branch (`main` /
/// `master`), when the current branch is not itself the default. Falls back
/// to `HEAD` when there is nothing to compare against.
fn branch_base(cwd: &Path) -> String {
    let current = current_branch(cwd);
    let branches = list_branches(cwd).unwrap_or_default();
    let default_branch = ["main", "master"].into_iter().find(|candidate| {
        current.as_deref() != Some(*candidate) && branches.iter().any(|b| b == candidate)
    });
    let Some(default_branch) = default_branch else {
        return head_or_empty(cwd);
    };
    match run_git_ok(cwd, ["merge-base", "HEAD", default_branch]) {
        Ok(base) if !base.trim().is_empty() => base.trim().to_owned(),
        _ => head_or_empty(cwd),
    }
}

fn head_or_empty(cwd: &Path) -> String {
    checkpoint::resolve(cwd, "HEAD").unwrap_or_else(|| EMPTY_TREE.to_owned())
}

fn diff_output(cwd: &Path, from: &str, to: &str, modes: &[&str]) -> anyhow::Result<String> {
    let output = command(cwd)
        .args([
            "-c",
            "core.quotePath=false",
            "diff",
            "--no-ext-diff",
            "--no-color",
        ])
        .args(modes)
        .arg("--no-renames")
        .arg(from)
        .arg(to)
        .args(["--", "."])
        .output()
        .map_err(|err| anyhow!("failed to generate Git diff: {err}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        bail!("{}", command_error(&output));
    }
}

fn ensure_repository(cwd: &Path) -> anyhow::Result<()> {
    let output = match command(cwd)
        .args(["rev-parse", "--is-inside-work-tree"])
        .output()
    {
        Ok(output) => output,
        // A missing directory or unusable git binary is, for our purposes,
        // "not a repository".
        Err(_) => bail!("not a git repository"),
    };
    if output.status.success() {
        Ok(())
    } else {
        bail!("not a git repository");
    }
}

// ── Git panel: working tree, staging, history, graph ───────────────────────

/// One changed file from `git status --porcelain`, with separate staged and
/// unstaged line deltas so it can appear in either list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusRow {
    pub path: String,
    /// Previous path for a rename/copy.
    pub orig_path: Option<String>,
    /// Index (staged) status byte.
    pub index: char,
    /// Worktree (unstaged) status byte.
    pub worktree: char,
    pub staged_additions: u64,
    pub staged_deletions: u64,
    pub unstaged_additions: u64,
    pub unstaged_deletions: u64,
}

impl StatusRow {
    pub fn untracked(&self) -> bool {
        self.index == '?' || self.worktree == '?'
    }

    pub fn staged(&self) -> bool {
        matches!(self.index, 'M' | 'A' | 'D' | 'R' | 'C' | 'T')
    }

    pub fn unstaged(&self) -> bool {
        self.untracked() || matches!(self.worktree, 'M' | 'D' | 'T')
    }

    /// Badge for the unstaged list (`U` for untracked).
    pub fn change_badge(&self) -> char {
        if self.untracked() {
            'U'
        } else if self.worktree == ' ' {
            'M'
        } else {
            self.worktree
        }
    }

    /// Badge for the staged list.
    pub fn staged_badge(&self) -> char {
        if self.index == ' ' {
            'M'
        } else {
            self.index
        }
    }
}

/// Changed files with per-file line deltas. Staged and unstaged counts come
/// from separate `--numstat` runs; untracked files are counted from disk.
pub fn status_rows(cwd: &Path) -> Result<Vec<StatusRow>, String> {
    let out = run_git(
        cwd,
        &[
            "-c",
            "core.quotePath=false",
            "status",
            "--porcelain",
            "--untracked-files=all",
        ],
    )?;
    let unstaged_stats = numstat_map(cwd, false);
    let staged_stats = numstat_map(cwd, true);
    let mut rows = Vec::new();
    for line in out.lines() {
        let bytes = line.as_bytes();
        if bytes.len() < 3 {
            continue;
        }
        let index = bytes[0] as char;
        let worktree = bytes[1] as char;
        if index == '!' {
            continue;
        }
        let rest = &line[3..];
        let (path, orig_path) = match rest.split_once(" -> ") {
            Some((from, to)) => (to.to_string(), Some(from.to_string())),
            None => (rest.to_string(), None),
        };
        let (staged_additions, staged_deletions) =
            staged_stats.get(&path).copied().unwrap_or((0, 0));
        let (unstaged_additions, unstaged_deletions) = if index == '?' {
            (untracked_line_count(cwd, &path), 0)
        } else {
            unstaged_stats.get(&path).copied().unwrap_or((0, 0))
        };
        rows.push(StatusRow {
            path,
            orig_path,
            index,
            worktree,
            staged_additions,
            staged_deletions,
            unstaged_additions,
            unstaged_deletions,
        });
    }
    Ok(rows)
}

fn numstat_map(cwd: &Path, cached: bool) -> HashMap<String, (u64, u64)> {
    let mut args = vec![
        "-c",
        "core.quotePath=false",
        "diff",
        "--numstat",
        "--no-renames",
    ];
    if cached {
        args.push("--cached");
    }
    args.push("--");
    let out = run_git(cwd, &args).unwrap_or_default();
    out.lines()
        .filter_map(|line| {
            let mut columns = line.splitn(3, '\t');
            let added = columns.next()?;
            let removed = columns.next()?;
            let path = columns.next()?.to_string();
            Some((path, (added.parse().ok()?, removed.parse().ok()?)))
        })
        .collect()
}

/// Lines in an untracked file (bounded; binary and oversized files read 0).
fn untracked_line_count(cwd: &Path, path: &str) -> u64 {
    const CAP: u64 = 2 * 1024 * 1024;
    let full = cwd.join(path);
    let Ok(metadata) = std::fs::metadata(&full) else {
        return 0;
    };
    if !metadata.is_file() || metadata.len() > CAP {
        return 0;
    }
    let Ok(bytes) = std::fs::read(&full) else {
        return 0;
    };
    if bytes.contains(&0) {
        return 0;
    }
    let newlines = bytes.iter().filter(|byte| **byte == b'\n').count() as u64;
    if bytes.last().is_some_and(|byte| *byte != b'\n') {
        newlines + 1
    } else {
        newlines
    }
}

pub fn stage_paths(cwd: &Path, paths: &[String]) -> Result<(), String> {
    if paths.is_empty() {
        return Ok(());
    }
    let mut args = vec!["add", "--"];
    args.extend(paths.iter().map(String::as_str));
    run_git(cwd, &args).map(|_| ())
}

pub fn stage_all(cwd: &Path) -> Result<(), String> {
    run_git(cwd, &["add", "-A", "--", "."]).map(|_| ())
}

pub fn unstage_paths(cwd: &Path, paths: &[String]) -> Result<(), String> {
    if paths.is_empty() {
        return Ok(());
    }
    let mut args = vec!["reset", "-q", "--"];
    args.extend(paths.iter().map(String::as_str));
    run_git(cwd, &args).map(|_| ())
}

pub fn unstage_all(cwd: &Path) -> Result<(), String> {
    run_git(cwd, &["reset", "-q"]).map(|_| ())
}

/// Discard working-tree changes: tracked files restore from the index, and
/// untracked files are removed. The UI confirms before calling this.
pub fn discard_paths(cwd: &Path, paths: &[String]) -> Result<(), String> {
    if paths.is_empty() {
        return Ok(());
    }
    let mut restore = vec!["checkout", "--"];
    restore.extend(paths.iter().map(String::as_str));
    let _ = run_git(cwd, &restore);
    let mut clean = vec!["clean", "-fdq", "--"];
    clean.extend(paths.iter().map(String::as_str));
    let _ = run_git(cwd, &clean);
    Ok(())
}

/// The staged patch text (for commit-message generation).
pub fn staged_patch(cwd: &Path) -> Result<String, String> {
    run_git(
        cwd,
        &[
            "-c",
            "core.quotePath=false",
            "diff",
            "--cached",
            "--no-color",
            "--no-renames",
            "--",
        ],
    )
}

/// The unstaged patch text (for commit-message generation).
pub fn unstaged_patch(cwd: &Path) -> Result<String, String> {
    run_git(
        cwd,
        &[
            "-c",
            "core.quotePath=false",
            "diff",
            "--no-color",
            "--no-renames",
            "--",
        ],
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefKind {
    Head,
    Branch,
    Remote,
    Tag,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefLabel {
    pub name: String,
    pub kind: RefKind,
}

/// One commit for the History list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitEntry {
    pub hash: String,
    pub short: String,
    pub author: String,
    pub relative: String,
    pub subject: String,
    pub refs: Vec<RefLabel>,
}

/// One commit for the Graph list, with its lane layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphRow {
    pub hash: String,
    pub short: String,
    pub author: String,
    pub relative: String,
    pub subject: String,
    pub refs: Vec<RefLabel>,
    /// The lane this commit's node sits on.
    pub lane: usize,
    pub lane_count: usize,
    /// Lanes carrying a line into this row.
    pub before: Vec<bool>,
    /// Lanes carrying a line out of this row.
    pub after: Vec<bool>,
    /// Merge/branch links from this commit's lane to a parent's lane.
    pub links: Vec<(usize, usize)>,
}

pub fn history(cwd: &Path, limit: usize, skip: usize) -> Result<Vec<CommitEntry>, String> {
    let format = "%H%x1f%h%x1f%an%x1f%ar%x1f%s%x1f%D";
    let out = run_git(
        cwd,
        &[
            "-c",
            "core.quotePath=false",
            "log",
            "--date-order",
            &format!("--max-count={limit}"),
            &format!("--skip={skip}"),
            &format!("--format={format}"),
        ],
    )?;
    Ok(out
        .lines()
        .filter_map(|line| {
            let mut fields = line.split('\u{1f}');
            let hash = fields.next()?.to_string();
            if hash.is_empty() {
                return None;
            }
            let short = fields.next()?.to_string();
            let author = fields.next()?.to_string();
            let relative = fields.next()?.to_string();
            let subject = fields.next()?.to_string();
            let refs = parse_refs(fields.next().unwrap_or(""));
            Some(CommitEntry {
                hash,
                short,
                author,
                relative,
                subject,
                refs,
            })
        })
        .collect())
}

pub fn graph(cwd: &Path, limit: usize) -> Result<Vec<GraphRow>, String> {
    let format = "%H%x1f%h%x1f%P%x1f%an%x1f%ar%x1f%s%x1f%D";
    let out = run_git(
        cwd,
        &[
            "-c",
            "core.quotePath=false",
            "log",
            "--all",
            "--date-order",
            &format!("--max-count={limit}"),
            &format!("--format={format}"),
        ],
    )?;
    let mut active: Vec<Option<String>> = Vec::new();
    let mut rows = Vec::new();
    for line in out.lines() {
        let mut fields = line.split('\u{1f}');
        let hash = fields.next().unwrap_or("").to_string();
        if hash.is_empty() {
            continue;
        }
        let short = fields.next().unwrap_or("").to_string();
        let parents = fields
            .next()
            .unwrap_or("")
            .split_whitespace()
            .map(str::to_string)
            .collect::<Vec<_>>();
        let author = fields.next().unwrap_or("").to_string();
        let relative = fields.next().unwrap_or("").to_string();
        let subject = fields.next().unwrap_or("").to_string();
        let refs = parse_refs(fields.next().unwrap_or(""));

        let lane = active
            .iter()
            .position(|candidate| candidate.as_deref() == Some(hash.as_str()))
            .unwrap_or_else(|| {
                if let Some(slot) = active.iter().position(Option::is_none) {
                    slot
                } else {
                    active.push(None);
                    active.len() - 1
                }
            });
        let before = active.iter().map(Option::is_some).collect::<Vec<_>>();

        active[lane] = parents.first().cloned();
        let mut links = Vec::new();
        for parent in parents.iter().skip(1) {
            let slot = if let Some(existing) = active
                .iter()
                .position(|h| h.as_deref() == Some(parent.as_str()))
            {
                existing
            } else if let Some(empty) = active.iter().position(Option::is_none) {
                empty
            } else {
                active.push(None);
                active.len() - 1
            };
            active[slot] = Some(parent.clone());
            links.push((lane, slot));
        }
        while active.last().is_some_and(Option::is_none) {
            active.pop();
        }
        let after = active.iter().map(Option::is_some).collect::<Vec<_>>();
        let lane_count = before.len().max(after.len()).max(lane + 1);
        rows.push(GraphRow {
            hash,
            short,
            author,
            relative,
            subject,
            refs,
            lane,
            lane_count,
            before,
            after,
            links,
        });
    }
    Ok(rows)
}

fn parse_refs(decor: &str) -> Vec<RefLabel> {
    decor
        .split(", ")
        .filter(|part| !part.is_empty())
        .map(|raw| {
            if let Some(branch) = raw.strip_prefix("HEAD -> ") {
                RefLabel {
                    name: branch.to_string(),
                    kind: RefKind::Head,
                }
            } else if let Some(tag) = raw.strip_prefix("tag: ") {
                RefLabel {
                    name: tag.to_string(),
                    kind: RefKind::Tag,
                }
            } else if raw.contains('/') {
                RefLabel {
                    name: raw.to_string(),
                    kind: RefKind::Remote,
                }
            } else {
                RefLabel {
                    name: raw.to_string(),
                    kind: RefKind::Branch,
                }
            }
        })
        .collect()
}

/// `(ahead, behind)` relative to the branch's upstream, or `None`.
pub fn ahead_behind(cwd: &Path) -> Option<(usize, usize)> {
    let out = run_git(
        cwd,
        &["rev-list", "--left-right", "--count", "HEAD...@{upstream}"],
    )
    .ok()?;
    let mut parts = out.split_whitespace();
    let ahead = parts.next()?.parse().ok()?;
    let behind = parts.next()?.parse().ok()?;
    Some((ahead, behind))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checkpoint;
    use std::fs;

    fn git_ok(cwd: &Path, args: &[&str]) {
        let status = command(cwd).args(args).status().unwrap();
        assert!(status.success(), "git {args:?} failed");
    }

    fn repository() -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "orbit-git-collect-{}",
            std::process::id() as u64
                + std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos() as u64
        ));
        fs::create_dir_all(&root).unwrap();
        git_ok(&root, &["init", "--quiet", "--initial-branch=main"]);
        git_ok(&root, &["config", "user.name", "Orbit Test"]);
        git_ok(&root, &["config", "user.email", "orbit@example.com"]);
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/lib.rs"), "fn baseline() {}\n").unwrap();
        git_ok(&root, &["add", "."]);
        git_ok(&root, &["commit", "--quiet", "-m", "baseline"]);
        root
    }

    #[test]
    fn status_rows_split_staged_and_unstaged_with_counts() {
        let root = repository();
        fs::write(root.join("src/lib.rs"), "fn staged() {}\n").unwrap();
        git_ok(&root, &["add", "src/lib.rs"]);
        fs::write(root.join("src/other.rs"), "fn untracked() {}\n").unwrap();

        let rows = status_rows(&root).unwrap();
        let staged = rows.iter().find(|row| row.path == "src/lib.rs").unwrap();
        assert!(staged.staged());
        assert_eq!(staged.staged_badge(), 'M');
        assert_eq!(staged.staged_additions, 1);
        let untracked = rows.iter().find(|row| row.path == "src/other.rs").unwrap();
        assert!(untracked.untracked());
        assert_eq!(untracked.change_badge(), 'U');
        assert_eq!(untracked.unstaged_additions, 1);

        // Stage everything, then everything is staged and nothing unstaged.
        stage_all(&root).unwrap();
        let rows = status_rows(&root).unwrap();
        assert!(rows.iter().all(StatusRow::staged));
        assert!(!rows.iter().any(StatusRow::unstaged));
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn history_and_graph_include_the_baseline_commit() {
        let root = repository();
        let history = history(&root, 20, 0).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].subject, "baseline");
        assert!(!history[0].short.is_empty());

        let graph = graph(&root, 20).unwrap();
        assert_eq!(graph.len(), 1);
        assert_eq!(graph[0].lane, 0);
        assert_eq!(graph[0].lane_count, 1);
        fs::remove_dir_all(root).ok();
    }

    fn summary(data: &ReviewDiff) -> (usize, u64, u64) {
        let mut files = 0;
        let mut additions = 0;
        let mut deletions = 0;
        for line in data.numstat.lines().filter(|line| !line.is_empty()) {
            let mut fields = line.splitn(3, '\t');
            additions += fields.next().and_then(|v| v.parse().ok()).unwrap_or(0);
            deletions += fields.next().and_then(|v| v.parse().ok()).unwrap_or(0);
            files += 1;
        }
        (files, additions, deletions)
    }

    fn collect(root: &Path, source: Source, session: Option<&str>) -> ReviewDiff {
        collect_review_diff(root, source, session).unwrap()
    }

    #[test]
    fn source_modes_compare_consistent_git_snapshots() {
        let root = repository();
        git_ok(&root, &["switch", "-c", "feature"]);
        fs::write(root.join("src/lib.rs"), "fn committed() {}\n").unwrap();
        git_ok(&root, &["add", "src/lib.rs"]);
        git_ok(&root, &["commit", "--quiet", "-m", "feature"]);
        fs::write(
            root.join("src/lib.rs"),
            "fn committed() {}\nfn staged() {}\n",
        )
        .unwrap();
        git_ok(&root, &["add", "src/lib.rs"]);
        fs::write(
            root.join("src/lib.rs"),
            "fn committed() {}\nfn staged() {}\nfn unstaged() {}\n",
        )
        .unwrap();
        fs::write(root.join("new file.txt"), "untracked\n").unwrap();

        let committed = collect(&root, Source::Committed, None);
        let staged = collect(&root, Source::Staged, None);
        let unstaged = collect(&root, Source::Unstaged, None);
        let uncommitted = collect(&root, Source::Uncommitted, None);
        let branch = collect(&root, Source::Branch, None);

        assert_eq!(summary(&committed), (1, 1, 1));
        assert_eq!(summary(&staged), (1, 1, 0));
        assert_eq!(summary(&unstaged), (2, 2, 0), "unstaged includes untracked");
        assert_eq!(summary(&uncommitted), (2, 3, 0));
        assert_eq!(summary(&branch), (2, 4, 1));
        assert!(complete_patch(&uncommitted).contains("fn unstaged"));
        fs::remove_dir_all(root).ok();
    }

    fn complete_patch(data: &ReviewDiff) -> String {
        data.patch.clone()
    }

    #[test]
    fn last_turn_uses_captured_checkpoints_not_the_live_worktree() {
        let root = repository();
        let session = "git-collect-session";
        checkpoint::capture_turn(&root, session, 0).unwrap();
        checkpoint::capture_turn_start(&root, session, 1).unwrap();
        fs::write(
            root.join("src/lib.rs"),
            "fn baseline() {}\nfn from_turn() {}\n",
        )
        .unwrap();
        checkpoint::capture_turn(&root, session, 1).unwrap();
        fs::write(
            root.join("src/lib.rs"),
            "fn baseline() {}\nfn from_turn() {}\nfn after_turn() {}\n",
        )
        .unwrap();

        let data = collect(&root, Source::LastTurn { turn_count: 1 }, Some(session));
        assert_eq!(summary(&data), (1, 1, 0));
        assert!(data.patch.contains("from_turn"));
        assert!(!data.patch.contains("after_turn"));
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn last_turn_without_checkpoints_reports_an_error() {
        let root = repository();
        let error = collect_review_diff(
            &root,
            Source::LastTurn { turn_count: 1 },
            Some("missing-session"),
        )
        .unwrap_err();
        assert!(error.contains("checkpoint"), "{error}");
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn off_repo_reports_not_a_repository() {
        let error = collect_review_diff(
            Path::new("/definitely/not/a/repo/orbit"),
            Source::Uncommitted,
            None,
        )
        .unwrap_err();
        assert!(error.contains("not a git repository"), "{error}");
    }
}
