//! The bundled provider-quota bridge extension.
//!
//! Orbit ships a small pi extension (`contrib/orbit-quota-extension/`) that
//! fetches each configured provider's account quota through pi's own
//! credential resolution and appends normalized snapshots as custom session
//! entries. Every pi process Orbit spawns is started with
//! `pi --extension <path>` pointing at a materialized copy, and the app reads
//! the snapshots back over RPC (`get_entries` with a cursor) into the existing
//! [`crate::quota::QuotaManager`].
//!
//! Nothing is installed and no settings file is touched: the extension lives
//! under `~/.orbit-pi/quota-extension/` and is rewritten only when its
//! contents change, so app updates can never leave a stale registration
//! behind. The payload contract is `crates/orbit-rpc/docs/quota-rpc.md`.

use std::fs;
use std::path::{Path, PathBuf};

use orbit_rpc::PiClient;

const INDEX_JS: &str = include_str!("../../../contrib/orbit-quota-extension/index.js");
const ADAPTERS_JS: &str = include_str!("../../../contrib/orbit-quota-extension/adapters.js");

/// The bridge extension materialized on disk, kept for the process lifetime.
#[derive(Default)]
pub(crate) struct QuotaBridge {
    /// The `index.js` path passed to pi; `None` when the bridge could not be
    /// written (quota then depends on the `quota.*` RPC, as before).
    extension: Option<PathBuf>,
}

impl QuotaBridge {
    /// Materialize the bundled extension under `~/.orbit-pi/quota-extension/`.
    /// Best-effort: a filesystem failure degrades to no bridge, never a
    /// blocked launch.
    pub(crate) fn install() -> Self {
        Self {
            extension: install_extension(),
        }
    }

    /// The `index.js` path passed to pi; `None` when the bridge could not be
    /// written (quota then depends on the `quota.*` RPC, as before).
    pub(crate) fn extension(&self) -> Option<&Path> {
        self.extension.as_deref()
    }

    /// Spawn a pi session process with the bridge loaded, when available.
    /// Every session spawn in the app goes through here.
    pub(crate) fn spawn(&self, workspace: &Path) -> anyhow::Result<PiClient> {
        match self.extension.as_deref() {
            Some(extension) => {
                PiClient::spawn_with_extensions(workspace, None, &[extension.to_path_buf()])
            }
            None => PiClient::spawn(workspace, None),
        }
    }
}

fn install_extension() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("USERPROFILE").map(PathBuf::from))?;
    let dir = home.join(".orbit-pi").join("quota-extension");
    fs::create_dir_all(&dir).ok()?;
    write_if_changed(&dir.join("index.js"), INDEX_JS).ok()?;
    write_if_changed(&dir.join("adapters.js"), ADAPTERS_JS).ok()?;
    Some(dir.join("index.js"))
}

/// Write only when the bytes differ, so relaunches don't churn mtimes (and a
/// running pi process is never surprised mid-session).
fn write_if_changed(path: &Path, contents: &str) -> std::io::Result<()> {
    if fs::read_to_string(path).is_ok_and(|current| current == contents) {
        return Ok(());
    }
    fs::write(path, contents)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch_dir() -> PathBuf {
        let unique = format!(
            "orbit-quota-bridge-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let dir = std::env::temp_dir().join(unique);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn writes_only_when_contents_change() {
        let dir = scratch_dir();
        let path = dir.join("index.js");
        write_if_changed(&path, "one").unwrap();
        let first = fs::metadata(&path).unwrap().modified().unwrap();
        write_if_changed(&path, "one").unwrap();
        assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), first);
        write_if_changed(&path, "two").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "two");
        fs::remove_dir_all(&dir).ok();
    }
}
