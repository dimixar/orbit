//! Onboarding / setup dependency check.
//!
//! Orbit drives the `pi` CLI as a child process. Before the app is usable,
//! the runtime pieces it needs must be installed: the `pi` coding agent
//! itself, the `node` runtime that runs it (pi is a Node script), and `git`
//! for the branch picker / diff panel. This module probes for each binary,
//! reports which are missing, and carries the install command to show the
//! user.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Absolute install dirs probed when a binary isn't on `PATH`. A bundled
/// `.app` launches with a minimal PATH, so we also check Homebrew-style dirs.
const SEARCH_DIRS: &[&str] = &[
    "/opt/homebrew/bin",
    "/usr/local/bin",
    "/opt/local/bin",
    "/usr/bin",
    "/bin",
];

/// Home-relative shim dirs (version managers, pnpm, bun, yarn, …). These are
/// where a manager puts a `node` launcher even though it isn't a real binary
/// in a system dir, so a bundled `.app` can't see it on PATH.
const HOME_SEARCH_SUBDIRS: &[&str] = &[
    ".volta/bin",
    ".local/share/mise/shims",
    ".asdf/shims",
    ".local/bin",
    "Library/pnpm/bin",
    ".bun/bin",
    ".npm-global/bin",
    ".yarn/bin",
];

/// Version managers that keep each Node version in its own numbered dir.
/// Each entry is `(base, subdir)`; the binary is `base/<version>/subdir/node`.
const VERSION_MANAGER_GLOBS: &[(&str, &str)] = &[
    (".nvm/versions/node", "bin"),
    (".local/share/fnm/node-versions", "installation/bin"),
];

/// One runtime dependency checked at startup.
#[derive(Debug, Clone)]
pub struct Dependency {
    /// Human name shown in the list.
    pub name: &'static str,
    /// Executable to probe on disk (may differ from `name`, e.g. `node`).
    /// Read by tests; in the UI the install hint already names the binary.
    #[allow(dead_code)]
    pub bin: &'static str,
    /// True when Orbit can't run without it (vs. a nice-to-have).
    pub required: bool,
    /// Whether the binary was found.
    pub installed: bool,
    /// First line of `<bin> --version`, when installed.
    pub version: Option<String>,
    /// Shell command that installs it.
    pub install_hint: &'static str,
    /// One-line explanation of what it's used for.
    pub detail: &'static str,
}

/// Probe every known dependency, in display order (required first).
pub fn check_dependencies() -> Vec<Dependency> {
    vec![
        dependency(
            "pi",
            "pi",
            true,
            "npm install -g @earendil-works/pi-coding-agent",
            "The pi coding agent — Orbit's agent runtime.",
        ),
        dependency(
            "node",
            "Node.js",
            true,
            "brew install node",
            "Runtime that runs the pi CLI (pi is a Node script).",
        ),
        dependency(
            "git",
            "git",
            false,
            "brew install git",
            "Used for the branch picker and diff panel.",
        ),
    ]
}

/// True when every required dependency is installed.
pub fn all_required_installed(deps: &[Dependency]) -> bool {
    deps.iter().filter(|d| d.required).all(|d| d.installed)
}

/// Number of required dependencies that are still missing.
pub fn missing_required_count(deps: &[Dependency]) -> usize {
    deps.iter().filter(|d| d.required && !d.installed).count()
}

fn dependency(
    bin: &'static str,
    name: &'static str,
    required: bool,
    install_hint: &'static str,
    detail: &'static str,
) -> Dependency {
    let found = locate(bin);
    let version = found.as_deref().and_then(version_of);
    Dependency {
        name,
        bin,
        required,
        installed: found.is_some(),
        version,
        install_hint,
        detail,
    }
}

/// Locate a binary by name, honoring `PI_BIN` for `pi`, then `PATH`, then
/// the common install dirs, Home-relative shim dirs, and version-manager
/// version dirs. This keeps detection working from a bundled `.app` whose
/// PATH doesn't include a user's node install (e.g. nvm, volta, mise).
fn locate(name: &str) -> Option<PathBuf> {
    if name == "pi" {
        if let Ok(bin) = std::env::var("PI_BIN") {
            if !bin.is_empty() {
                let p = PathBuf::from(&bin);
                if p.is_file() {
                    return Some(p);
                }
            }
        }
    }
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    for dir in SEARCH_DIRS {
        let candidate = Path::new(dir).join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    if let Some(home) = home_dir() {
        if let Some(found) = locate_in_home(&home, name) {
            return Some(found);
        }
    }
    None
}

/// Search the home-relative shim dirs and version-manager version dirs for a
/// binary. Split out so tests can point it at a temp dir.
fn locate_in_home(home: &Path, name: &str) -> Option<PathBuf> {
    for sub in HOME_SEARCH_SUBDIRS {
        let candidate = home.join(sub).join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    for (base, sub) in VERSION_MANAGER_GLOBS {
        let base = home.join(base);
        let Ok(versions) = std::fs::read_dir(&base) else {
            continue;
        };
        for version in versions.flatten() {
            let candidate = version.path().join(sub).join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

/// The user's home directory.
fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("USERPROFILE").map(PathBuf::from))
}

/// First line of `<bin> --version`, trimmed; `None` if it fails or is empty.
fn version_of(bin: &Path) -> Option<String> {
    let output = Command::new(bin).arg("--version").output().ok()?;
    let text = if output.stdout.is_empty() {
        output.stderr
    } else {
        output.stdout
    };
    String::from_utf8_lossy(&text)
        .lines()
        .next()
        .map(str::trim)
        .map(str::to_string)
        .filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_expected_dependencies() {
        let deps = check_dependencies();
        assert_eq!(deps.len(), 3);
        assert!(deps.iter().any(|d| d.name == "pi"));
        assert!(deps.iter().any(|d| d.name == "Node.js"));
        assert!(deps.iter().any(|d| d.name == "git"));
        // Only pi and Node.js are required.
        assert!(deps
            .iter()
            .filter(|d| d.required)
            .all(|d| d.name == "pi" || d.name == "Node.js"));
        // Every installed dep has a version line from `<bin> --version`.
        for d in &deps {
            if d.installed {
                assert!(d.version.is_some(), "{} missing version", d.name);
            } else {
                assert!(!d.install_hint.is_empty());
            }
        }
    }

    #[test]
    fn node_probes_the_node_binary() {
        // The display name is "Node.js" but the executable is "node". If the
        // binary name were used as the probe name, node would always look
        // missing even when installed.
        let deps = check_dependencies();
        let node = deps.iter().find(|d| d.name == "Node.js").unwrap();
        assert_eq!(node.bin, "node");
        // With node on PATH, the probe must resolve it.
        assert!(
            locate("node").is_some(),
            "expected to find node on this machine"
        );
    }

    #[test]
    fn ready_only_when_all_required_installed() {
        let mut deps = check_dependencies();
        for d in deps.iter_mut() {
            d.installed = true;
        }
        assert!(all_required_installed(&deps));
        assert_eq!(missing_required_count(&deps), 0);

        // Missing a required dep blocks readiness.
        deps.iter_mut().find(|d| d.name == "pi").unwrap().installed = false;
        assert!(!all_required_installed(&deps));
        assert_eq!(missing_required_count(&deps), 1);

        // A missing optional dep (git) never blocks readiness.
        deps.iter_mut().find(|d| d.name == "pi").unwrap().installed = true;
        deps.iter_mut().find(|d| d.name == "git").unwrap().installed = false;
        assert!(all_required_installed(&deps));
        assert_eq!(missing_required_count(&deps), 0);
    }

    #[test]
    fn finds_node_in_version_manager_dirs() {
        // A fake HOME with an nvm-style version dir: node at
        // ~/.nvm/versions/node/v20.0.0/bin/node.
        let home = std::env::temp_dir().join("orbit-onboarding-test");
        let _ = std::fs::remove_dir_all(&home);
        let nvm = home
            .join(".nvm")
            .join("versions")
            .join("node")
            .join("v20.0.0")
            .join("bin");
        std::fs::create_dir_all(&nvm).unwrap();
        let node = nvm.join("node");
        std::fs::write(&node, "#!/bin/sh\necho node\n").unwrap();

        assert_eq!(locate_in_home(&home, "node"), Some(node));

        let _ = std::fs::remove_dir_all(&home);
    }
}
