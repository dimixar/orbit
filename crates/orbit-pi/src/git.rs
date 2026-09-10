//! Local Git helpers: branch discovery for the status bar and the Review
//! panel's diff collection.
//!
//! Everything here is pure I/O and stays off the UI thread. The workspace is
//! compared through [`crate::checkpoint`] snapshots so untracked files, stage
//! state, and per-turn checkpoints are represented exactly.

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
