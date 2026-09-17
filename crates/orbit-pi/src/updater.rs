//! Background, signed, in-app updates.
//!
//! Orbit ships the same EdDSA-signed appcast contract on every platform, with
//! no Sparkle dependency: a per-architecture feed names a release, the
//! artifact beside it carries a signature, and this module verifies the
//! signature against the public key `build.rs` compiled in before anything is
//! staged. Checks run on their own threads and report back through a channel,
//! so the UI only ever sees [`UpdateStatus`] and [`UpdaterEvent`].
//!
//! Platform install seams:
//! - **macOS** — the feed points at a `.tar.gz` whose single top-level entry
//!   is `Orbit Pi.app`. The helper waits for the app to quit, swaps the
//!   bundle for the extracted one, relaunches it, and rolls back if the new
//!   build never signals a ready main window.
//! - **Linux** — the same swap against the managed install prefix.
//! - **Windows** — the feed points at an installer, which runs silently.
//!
//! Debug builds stay dormant so a dev `cargo run` never replaces itself with a
//! production install. `ORBIT_FORCE_UPDATER=1` arms the real flow; an empty or
//! invalid compiled-in key also leaves the updater off.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};

use base64::Engine as _;
use gpui::Global;

/// Environment variable the relaunched build reads to signal a live window.
/// The install helper that sets it is Unix-only (see [`run_install_helper`]).
#[cfg(unix)]
const RELAUNCH_READY_ENV: &str = "ORBIT_UPDATE_READY_FILE";
/// Hidden flag that turns the app binary into its own update helper.
#[cfg(unix)]
const INSTALL_HELPER_FLAG: &str = "--orbit-update-install";

/// App-wide handle to the updater, if this build can update itself.
pub struct UpdaterState(pub Option<Updater>);

impl Global for UpdaterState {}

/// The compact state the UI renders. Release details never enter a frame
/// path; they live on the worker thread that fetched them.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum UpdateStatus {
    #[default]
    Idle,
    Available,
    Updating,
}

#[derive(Clone, Debug)]
pub enum UpdaterEvent {
    StatusChanged(UpdateStatus),
    UpToDate,
    Failed(String),
    /// The helper has validated its inputs and waits for the app's normal
    /// quit handlers to finish before it swaps directories. The UI quits.
    /// Only a build whose updater can hand off (Unix) ever emits this.
    #[cfg(unix)]
    QuitAndInstall,
}

/// The public half of the release signing key, read from
/// `ORBIT_UPDATE_PUBLIC_KEY` (or a bundled `Info.plist`) by `build.rs`.
const PUBLIC_ED_KEY: &str = env!("ORBIT_UPDATE_PUBLIC_KEY");

/// The release feed for this platform and architecture. `ORBIT_UPDATE_FEED_URL`
/// overrides it at build time. A Sparkle appcast cannot say which binary an
/// item is for, so the client picks its feed by target.
const FEED_URL: &str = match option_env!("ORBIT_UPDATE_FEED_URL") {
    Some(url) => url,
    None => DEFAULT_FEED_URL,
};

// Appcasts are committed under `appcasts/` on `main` and served by GitHub's
// raw CDN. Keep these paths in sync with `.github/workflows/appcasts.yml`.
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
const DEFAULT_FEED_URL: &str =
    "https://raw.githubusercontent.com/imrj05/orbit/main/appcasts/appcast-macos-aarch64.xml";
#[cfg(all(target_os = "macos", not(target_arch = "aarch64")))]
const DEFAULT_FEED_URL: &str =
    "https://raw.githubusercontent.com/imrj05/orbit/main/appcasts/appcast-macos-x86_64.xml";
#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
const DEFAULT_FEED_URL: &str =
    "https://raw.githubusercontent.com/imrj05/orbit/main/appcasts/appcast-linux-aarch64.xml";
#[cfg(all(target_os = "linux", not(target_arch = "aarch64")))]
const DEFAULT_FEED_URL: &str =
    "https://raw.githubusercontent.com/imrj05/orbit/main/appcasts/appcast-linux-x86_64.xml";
#[cfg(all(target_os = "windows", target_arch = "aarch64"))]
const DEFAULT_FEED_URL: &str =
    "https://raw.githubusercontent.com/imrj05/orbit/main/appcasts/appcast-windows-aarch64.xml";
#[cfg(all(target_os = "windows", not(target_arch = "aarch64")))]
const DEFAULT_FEED_URL: &str =
    "https://raw.githubusercontent.com/imrj05/orbit/main/appcasts/appcast-windows-x86_64.xml";
#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
const DEFAULT_FEED_URL: &str = "";

/// A checked feed is a few kilobytes; an artifact may be a full bundle.
const MAX_FEED_BYTES: u64 = 1024 * 1024;
const MAX_ARTIFACT_BYTES: u64 = 512 * 1024 * 1024;
#[cfg(unix)]
const MAX_UNPACKED_BYTES: u64 = 1024 * 1024 * 1024;
#[cfg(unix)]
const MAX_ARCHIVE_ENTRIES: usize = 100_000;
#[cfg(unix)]
const MAX_ERROR_BYTES: u64 = 16 * 1024;

/// A verified artifact on disk. On Unix the payload is an extracted directory
/// that must outlive the helper's swap, so it cleans itself up unless handed
/// off.
struct StagedUpdate {
    /// Unix: the extraction root (the helper finds its single child). Windows:
    /// the installer executable.
    path: PathBuf,
    cleanup: Option<PathBuf>,
}

impl StagedUpdate {
    fn new(path: PathBuf, cleanup: PathBuf) -> Self {
        Self {
            path,
            cleanup: Some(cleanup),
        }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    /// Keep the staged payload alive for the helper, dropping the cleanup.
    fn disarm(mut self) {
        self.cleanup = None;
    }
}

impl Drop for StagedUpdate {
    fn drop(&mut self) {
        if let Some(directory) = self.cleanup.take() {
            let _ = std::fs::remove_dir_all(directory);
        }
    }
}

pub struct Updater {
    status: Arc<Mutex<UpdateStatus>>,
    staged: Arc<Mutex<Option<StagedUpdate>>>,
    /// A check is running. Separate from `status` because a silent check is
    /// deliberately invisible, so the published status cannot keep two
    /// checks from overlapping.
    checking: Arc<AtomicBool>,
    /// Whether the in-flight check reports its outcome. An explicit request
    /// that lands while a silent one runs sets it rather than being dropped.
    explicit_check: Arc<AtomicBool>,
    automatic: Arc<AtomicBool>,
    preference_path: PathBuf,
    events: mpsc::Sender<UpdaterEvent>,
    receiver: mpsc::Receiver<UpdaterEvent>,
    #[cfg(unix)]
    layout: InstallLayout,
}

impl Updater {
    /// Load the embedded key and start the updater. Returns `None` when this
    /// build cannot update itself: debug builds unless forced, a bare
    /// `cargo run` binary outside a managed install, or a keyless build.
    pub fn init() -> Option<Self> {
        let forced = std::env::var_os("ORBIT_FORCE_UPDATER").is_some_and(|value| value == "1");
        if cfg!(debug_assertions) && !forced {
            return None;
        }
        if FEED_URL.is_empty() {
            return None;
        }
        if verifying_key().is_none() {
            eprintln!("Orbit updater: the compiled-in public key is not a valid ed25519 key");
            return None;
        }

        #[cfg(unix)]
        let layout = InstallLayout::discover()?;

        let preference_path = preference_path()?;
        let automatic = Arc::new(AtomicBool::new(read_automatic_preference(&preference_path)));
        let (events, receiver) = mpsc::channel();
        let updater = Self {
            status: Arc::new(Mutex::new(UpdateStatus::Idle)),
            staged: Arc::new(Mutex::new(None)),
            checking: Arc::new(AtomicBool::new(false)),
            explicit_check: Arc::new(AtomicBool::new(false)),
            automatic,
            preference_path,
            events,
            receiver,
            #[cfg(unix)]
            layout,
        };

        #[cfg(unix)]
        if let Some(error) = take_update_error(&updater.layout) {
            let _ = updater.events.send(UpdaterEvent::Failed(error));
        }

        // Sparkle arms a scheduled checker on macOS; here one silent check
        // per launch is the whole schedule.
        if updater.automatically_checks_for_updates() {
            updater.start_check(false);
        }
        Some(updater)
    }

    /// A user-initiated check. Unlike the silent one it reports both
    /// "already current" and failures.
    pub fn check_for_updates(&self) {
        self.start_check(true);
    }

    fn start_check(&self, user_initiated: bool) {
        if self.status() == UpdateStatus::Updating {
            return;
        }
        if self
            .checking
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            // A silent launch check is in flight. An explicit request adopts
            // it so the menu still gets an answer rather than being dropped.
            if user_initiated {
                self.explicit_check.store(true, Ordering::Relaxed);
            }
            return;
        }
        self.explicit_check.store(user_initiated, Ordering::Relaxed);

        let status = self.status.clone();
        let staged = self.staged.clone();
        let checking = self.checking.clone();
        let explicit_check = self.explicit_check.clone();
        let events = self.events.clone();
        let publish_events = events.clone();
        // The staged payload must sit on the same filesystem as the install
        // for the helper's swap to be a rename, so the check thread stages
        // beside the install rather than in the temp directory.
        #[cfg(unix)]
        let layout = self.layout.clone();
        let publish = move |next: UpdateStatus| {
            let mut status = status
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            // A check leaves the status alone while it runs, so most outcomes
            // are not transitions and must not repaint.
            if *status == next {
                return;
            }
            *status = next;
            let _ = publish_events.send(UpdaterEvent::StatusChanged(next));
        };
        let spawned = std::thread::Builder::new()
            .name("orbit-updater-check".into())
            .spawn(move || {
                #[cfg(unix)]
                let outcome = fetch_and_stage(&layout);
                #[cfg(not(unix))]
                let outcome = fetch_and_stage();
                let report = explicit_check.load(Ordering::Relaxed);
                match outcome {
                    Ok(Some(update)) => {
                        *staged
                            .lock()
                            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(update);
                        publish(UpdateStatus::Available);
                    }
                    Ok(None) => {
                        publish(UpdateStatus::Idle);
                        if report {
                            let _ = events.send(UpdaterEvent::UpToDate);
                        }
                    }
                    Err(error) => {
                        publish(UpdateStatus::Idle);
                        if report {
                            let _ = events.send(UpdaterEvent::Failed(error.to_string()));
                        } else {
                            eprintln!("Orbit updater: {error:#}");
                        }
                    }
                }
                checking.store(false, Ordering::Release);
            });
        if spawned.is_err() {
            self.checking.store(false, Ordering::Release);
        }
    }

    /// Start the helper, then wait off the UI thread for its validation
    /// acknowledgement. Only that acknowledgement emits `QuitAndInstall`.
    #[cfg(unix)]
    pub fn install_available_update(&self) -> bool {
        let update = {
            let mut staged = self
                .staged
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            match staged.take() {
                Some(update) => update,
                None => return false,
            }
        };

        let ready_file = self.layout.parent.join(format!(
            ".{}.update-ready-{}-{nonce}",
            self.layout.prefix_name,
            std::process::id(),
            nonce = unique_nonce()
        ));
        let mut command = std::process::Command::new(
            std::env::current_exe().unwrap_or_else(|_| self.layout.relaunch_executable()),
        );
        command
            .arg(INSTALL_HELPER_FLAG)
            .arg("--install-dir")
            .arg(&self.layout.install_dir)
            .arg("--staged-dir")
            .arg(update.path())
            .arg("--parent-pid")
            .arg(std::process::id().to_string())
            .arg("--ready-file")
            .arg(&ready_file)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit());
        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(error) => {
                self.fail_install(format!("could not start the update helper: {error}"));
                return false;
            }
        };

        self.set_status(UpdateStatus::Updating);

        // A dedicated thread owns the child and the payload, so the staged
        // directory survives until the helper has it and is reclaimed if the
        // helper rejects the handoff.
        let status = self.status.clone();
        let events = self.events.clone();
        // The thread owns the child and the payload. If it cannot start, the
        // closure is dropped along with the staged update (and the orphaned
        // helper times itself out); the app keeps running the current build.
        let spawned = std::thread::Builder::new()
            .name("orbit-updater-handoff".into())
            .spawn(move || {
                use std::io::BufRead as _;
                let mut acknowledgement = String::new();
                let read = child
                    .stdout
                    .take()
                    .map(std::io::BufReader::new)
                    .and_then(|mut stdout| stdout.read_line(&mut acknowledgement).ok());
                if read.is_some() && acknowledgement.trim_end() == "READY" {
                    update.disarm();
                    let _ = events.send(UpdaterEvent::QuitAndInstall);
                    let _ = child.wait();
                    return;
                }

                let _ = child.kill();
                let _ = child.wait();
                *status
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner()) = UpdateStatus::Idle;
                let reason = acknowledgement
                    .strip_prefix("ERROR\t")
                    .map(str::trim)
                    .filter(|reason| !reason.is_empty())
                    .unwrap_or("the update helper did not accept the staged build");
                let _ = events.send(UpdaterEvent::Failed(reason.to_owned()));
            });
        if spawned.is_err() {
            self.fail_install("could not start the update handoff worker");
            return false;
        }
        true
    }

    /// Run the staged installer and leave. Inno Setup closes this process,
    /// replaces it in place, and starts the new build.
    #[cfg(windows)]
    pub fn install_available_update(&self) -> bool {
        let update = {
            let mut staged = self
                .staged
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            match staged.take() {
                Some(update) => update,
                None => return false,
            }
        };
        let installer = update.path().to_path_buf();
        let mut command = std::process::Command::new(&installer);
        command.args(["/SILENT", "/NORESTART", "/SP-"]);
        if let Some(directory) = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(Path::to_path_buf))
        {
            command.arg(format!("/DIR={}", directory.display()));
        }
        {
            use std::os::windows::process::CommandExt as _;
            command.creation_flags(0x0800_0000);
        }
        match command.spawn() {
            Ok(_) => {
                // The installer owns the file now; leave the staging directory
                // for it rather than deleting the running program.
                update.disarm();
                self.set_status(UpdateStatus::Updating);
                true
            }
            Err(error) => {
                self.fail_install(format!("{installer:?}: {error}"));
                false
            }
        }
    }

    #[cfg(not(any(unix, windows)))]
    pub fn install_available_update(&self) -> bool {
        false
    }

    pub fn status(&self) -> UpdateStatus {
        *self
            .status
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Drain one pending event. The heartbeat calls this each tick.
    pub fn try_recv_event(&self) -> Option<UpdaterEvent> {
        self.receiver.try_recv().ok()
    }

    pub fn automatically_checks_for_updates(&self) -> bool {
        self.automatic.load(Ordering::Relaxed)
    }

    pub fn set_automatically_checks_for_updates(&self, enabled: bool) {
        if self.automatic.swap(enabled, Ordering::Relaxed) == enabled {
            return;
        }
        let path = self.preference_path.clone();
        // A settings toggle must not wait on the filesystem.
        let _ = std::thread::Builder::new()
            .name("orbit-updater-preference".into())
            .spawn(move || write_automatic_preference(&path, enabled));
        if enabled {
            self.start_check(false);
        }
    }

    fn set_status(&self, next: UpdateStatus) {
        let mut status = self
            .status
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if *status == next {
            return;
        }
        *status = next;
        let _ = self.events.send(UpdaterEvent::StatusChanged(next));
    }

    fn fail_install(&self, error: impl Into<String>) {
        self.set_status(UpdateStatus::Idle);
        let _ = self.events.send(UpdaterEvent::Failed(error.into()));
    }
}

// ── the unix install seam ───────────────────────────────────────────────

/// A managed, user-writable install this process may replace.
#[cfg(unix)]
#[derive(Clone, Debug)]
struct InstallLayout {
    /// Directory replaced by the update: the `.app` on macOS, the prefix on
    /// Linux.
    install_dir: PathBuf,
    /// Directory the install lives in; staging siblings and the rollback and
    /// ready markers live here, so the swap never crosses filesystems.
    parent: PathBuf,
    prefix_name: String,
    exe_name: String,
}

#[cfg(unix)]
impl InstallLayout {
    fn discover() -> Option<Self> {
        // A root-owned session must not turn the app into a package manager.
        if unsafe { libc::geteuid() } == 0 {
            return None;
        }

        let executable = std::env::current_exe().ok()?.canonicalize().ok()?;
        let exe_name = executable.file_name()?.to_str()?.to_owned();

        #[cfg(target_os = "macos")]
        let install_dir = {
            let macos_dir = executable.parent()?;
            let contents = macos_dir.parent()?;
            let app = contents.parent()?;
            if macos_dir.file_name()? != "MacOS" || contents.file_name()? != "Contents" {
                return None;
            }
            if !app.file_name()?.to_str()?.ends_with(".app") {
                return None;
            }
            app.to_path_buf()
        };

        #[cfg(not(target_os = "macos"))]
        let install_dir = {
            if executable.parent()?.file_name()? != "bin" {
                return None;
            }
            executable.parent()?.parent()?.to_path_buf()
        };

        let install_dir = install_dir.canonicalize().ok()?;
        let parent = install_dir.parent()?.canonicalize().ok()?;
        let prefix_name = install_dir.file_name()?.to_str()?.to_owned();

        // The swap needs write access to the parent, not merely to files in
        // the current install. Probe once during initialization.
        if !probe_writable(&parent, &prefix_name) {
            return None;
        }

        Some(Self {
            install_dir,
            parent,
            prefix_name,
            exe_name,
        })
    }

    fn relaunch_executable(&self) -> PathBuf {
        relaunch_executable(&self.install_dir, &self.exe_name)
    }

    fn error_path(&self) -> PathBuf {
        self.parent
            .join(format!(".{}.update-error", self.prefix_name))
    }
}

/// The executable to relaunch inside an installed tree.
#[cfg(unix)]
fn relaunch_executable(install_dir: &Path, exe_name: &str) -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        install_dir.join("Contents").join("MacOS").join(exe_name)
    }
    #[cfg(not(target_os = "macos"))]
    {
        install_dir.join("bin").join(exe_name)
    }
}

#[cfg(unix)]
fn probe_writable(parent: &Path, prefix_name: &str) -> bool {
    let candidate = parent.join(format!(
        ".{prefix_name}.update-probe-{}-{}",
        std::process::id(),
        unique_nonce()
    ));
    match std::fs::create_dir(&candidate) {
        Ok(()) => {
            let _ = std::fs::remove_dir(&candidate);
            true
        }
        Err(_) => false,
    }
}

/// A hidden helper invocation, run before the UI starts. Returns the process
/// exit code when the arguments name an install handoff.
#[cfg(unix)]
pub fn run_install_helper() -> Option<i32> {
    let args: Vec<std::ffi::OsString> = std::env::args_os().collect();
    let index = args.iter().position(|arg| arg == INSTALL_HELPER_FLAG)?;
    // A malformed helper invocation must not fall through to a second UI.
    let Some(flags) = HelperFlags::parse(&args[index + 1..]) else {
        eprintln!("Orbit updater: malformed install-helper arguments");
        return Some(1);
    };
    Some(run_install(&flags))
}

#[cfg(not(unix))]
pub fn run_install_helper() -> Option<i32> {
    None
}

#[cfg(unix)]
struct HelperFlags {
    install_dir: PathBuf,
    staged_dir: PathBuf,
    parent_pid: u32,
    ready_file: PathBuf,
}

#[cfg(unix)]
impl HelperFlags {
    fn parse(args: &[std::ffi::OsString]) -> Option<Self> {
        let mut install_dir = None;
        let mut staged_dir = None;
        let mut parent_pid = None;
        let mut ready_file = None;
        let mut iter = args.iter();
        while let Some(flag) = iter.next() {
            let value = iter.next()?;
            match flag.to_str()? {
                "--install-dir" => install_dir = Some(PathBuf::from(value)),
                "--staged-dir" => staged_dir = Some(PathBuf::from(value)),
                "--parent-pid" => parent_pid = value.to_str()?.parse().ok(),
                "--ready-file" => ready_file = Some(PathBuf::from(value)),
                _ => return None,
            }
        }
        Some(Self {
            install_dir: install_dir?,
            staged_dir: staged_dir?,
            parent_pid: parent_pid?,
            ready_file: ready_file?,
        })
    }
}

#[cfg(unix)]
fn run_install(flags: &HelperFlags) -> i32 {
    let exe_name = match std::env::current_exe().ok().and_then(|exe| {
        exe.file_name()
            .map(|name| name.to_string_lossy().into_owned())
    }) {
        Some(name) => name,
        None => return helper_error(flags, "the running executable has no name"),
    };

    // The staged directory is the extracted release with its archive root
    // stripped, so it *is* the new install's contents.
    if !flags.staged_dir.is_dir() {
        return helper_error(flags, "the staged update is missing");
    }
    if !flags.install_dir.is_dir() {
        return helper_error(flags, "the install directory is missing");
    }
    if !staged_executable(&flags.staged_dir, &exe_name).is_file() {
        return helper_error(flags, "the staged update is missing its executable");
    }

    // Acknowledge the handoff before waiting, so the app can run its normal
    // quit handlers and release the filesystem.
    println!("READY");
    use std::io::Write as _;
    let _ = std::io::stdout().flush();

    if !wait_for_exit(flags.parent_pid, PARENT_EXIT_TIMEOUT) {
        return helper_error(flags, "the app did not quit in time");
    }

    let prefix_name = flags
        .install_dir
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Orbit".into());
    let backup = flags.parent_dir().join(format!(
        ".{prefix_name}.rollback-{}-{}",
        std::process::id(),
        unique_nonce()
    ));
    let _ = std::fs::remove_dir_all(&backup);

    if let Err(error) = std::fs::rename(&flags.install_dir, &backup) {
        return helper_error(
            flags,
            &format!("could not move the running install aside: {error}"),
        );
    }
    if let Err(error) = std::fs::rename(&flags.staged_dir, &flags.install_dir) {
        let _ = std::fs::rename(&backup, &flags.install_dir);
        return helper_error(
            flags,
            &format!("could not move the update into place: {error}"),
        );
    }

    let executable = relaunch_executable(&flags.install_dir, &exe_name);
    let mut child = match std::process::Command::new(&executable)
        .env(RELAUNCH_READY_ENV, &flags.ready_file)
        .stdin(std::process::Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(error) => {
            let _ = std::fs::remove_dir_all(&flags.install_dir);
            let _ = std::fs::rename(&backup, &flags.install_dir);
            return helper_error(flags, &format!("could not relaunch the new build: {error}"));
        }
    };

    if wait_for_ready(&flags.ready_file, RELAUNCH_READY_TIMEOUT) {
        let _ = std::fs::remove_file(&flags.ready_file);
        let _ = std::fs::remove_dir_all(&backup);
        return 0;
    }

    // The replacement never opened a window. Put the old build back and
    // relaunch it, so a bad release cannot strand the user.
    let _ = child.kill();
    let _ = child.wait();
    let _ = std::fs::remove_dir_all(&flags.install_dir);
    let _ = std::fs::rename(&backup, &flags.install_dir);
    let _ = std::process::Command::new(relaunch_executable(&flags.install_dir, &exe_name)).spawn();
    helper_error(flags, "the new build did not open its main window")
}

#[cfg(unix)]
impl HelperFlags {
    fn parent_dir(&self) -> &Path {
        self.install_dir.parent().unwrap_or(Path::new("."))
    }
}

#[cfg(unix)]
const PARENT_EXIT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(300);
#[cfg(unix)]
const RELAUNCH_READY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);
#[cfg(unix)]
const POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(200);

#[cfg(unix)]
fn wait_for_exit(pid: u32, timeout: std::time::Duration) -> bool {
    let deadline = std::time::Instant::now() + timeout;
    while process_alive(pid) {
        if std::time::Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(POLL_INTERVAL);
    }
    true
}

#[cfg(unix)]
fn process_alive(pid: u32) -> bool {
    let result = unsafe { libc::kill(pid as libc::pid_t, 0) };
    if result == 0 {
        return true;
    }
    std::io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH)
}

#[cfg(unix)]
fn wait_for_ready(path: &Path, timeout: std::time::Duration) -> bool {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        if path.exists() {
            return true;
        }
        if std::time::Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(POLL_INTERVAL);
    }
}

/// The main executable inside an install's *contents* (an extracted archive
/// with its root stripped, or a live install directory).
#[cfg(unix)]
fn staged_executable(install_contents: &Path, exe_name: &str) -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        install_contents
            .join("Contents")
            .join("MacOS")
            .join(exe_name)
    }
    #[cfg(not(target_os = "macos"))]
    {
        install_contents.join("bin").join(exe_name)
    }
}

#[cfg(unix)]
fn helper_error(flags: &HelperFlags, reason: &str) -> i32 {
    eprintln!("Orbit updater: {reason}");
    let prefix_name = flags
        .install_dir
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Orbit".into());
    let path = flags
        .parent_dir()
        .join(format!(".{prefix_name}.update-error"));
    let _ = std::fs::write(path, format!("{reason}\n"));
    println!("ERROR\t{reason}");
    1
}

/// Write the readiness marker the helper waits on. The path comes from the
/// helper through the environment, but is only honored when it sits beside
/// this install and names it, so the environment cannot redirect the write.
#[cfg(unix)]
pub(crate) fn signal_relaunch_ready() {
    let Some(path) = std::env::var_os(RELAUNCH_READY_ENV).map(PathBuf::from) else {
        return;
    };
    let Some((parent, prefix_name)) = current_ready_scope() else {
        return;
    };
    if path.parent() != Some(parent.as_path()) {
        return;
    }
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return;
    };
    if !name.starts_with(&format!(".{prefix_name}.update-ready-")) {
        return;
    }
    let _ = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .and_then(|mut file| {
            use std::io::Write as _;
            file.write_all(b"ready\n")
        });
}

#[cfg(not(unix))]
pub(crate) fn signal_relaunch_ready() {}

#[cfg(unix)]
fn current_ready_scope() -> Option<(PathBuf, String)> {
    let executable = std::env::current_exe().ok()?.canonicalize().ok()?;
    let install_dir = install_dir_for(&executable)?;
    let parent = install_dir.parent()?.canonicalize().ok()?;
    let prefix_name = install_dir.file_name()?.to_str()?.to_owned();
    Some((parent, prefix_name))
}

/// The install directory a given executable lives in, or `None` when the
/// executable is not inside a managed install.
#[cfg(unix)]
fn install_dir_for(executable: &Path) -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let macos_dir = executable.parent()?;
        let contents = macos_dir.parent()?;
        let app = contents.parent()?;
        if macos_dir.file_name()? != "MacOS" || contents.file_name()? != "Contents" {
            return None;
        }
        if !app.file_name()?.to_str()?.ends_with(".app") {
            return None;
        }
        Some(app.to_path_buf())
    }
    #[cfg(not(target_os = "macos"))]
    {
        if executable.parent()?.file_name()? != "bin" {
            return None;
        }
        Some(executable.parent()?.parent()?.to_path_buf())
    }
}

// ── staging ─────────────────────────────────────────────────────────────

#[cfg(unix)]
fn fetch_and_stage(layout: &InstallLayout) -> anyhow::Result<Option<StagedUpdate>> {
    fetch_and_stage_impl(|archive, directory| stage_artifact(archive, directory, layout))
}

#[cfg(not(unix))]
fn fetch_and_stage() -> anyhow::Result<Option<StagedUpdate>> {
    fetch_and_stage_impl(stage_artifact)
}

/// Fetch the feed, and when it names a newer release, download and verify the
/// artifact on a worker thread. `stage` turns the verified artifact into the
/// platform's staged update.
fn fetch_and_stage_impl<F>(stage: F) -> anyhow::Result<Option<StagedUpdate>>
where
    F: FnOnce(PathBuf, PathBuf) -> anyhow::Result<Option<StagedUpdate>>,
{
    let document = http_get(FEED_URL)?;
    let Some(item) = feed::newest_item(&document) else {
        anyhow::bail!("the update feed has no signed release");
    };
    if !feed::is_newer(&item.version, env!("CARGO_PKG_VERSION")) {
        return Ok(None);
    }
    validate_release_version(&item.version)?;
    validate_download_url(&item.url)?;
    let declared_length = item
        .length
        .filter(|length| *length > 0 && *length <= MAX_ARTIFACT_BYTES);

    let directory = unique_temp_directory("orbit-update")?;
    let archive = directory.join(artifact_name());
    download_to(&item.url, &archive, 600, MAX_ARTIFACT_BYTES)?;
    let actual_length = std::fs::metadata(&archive)?.len();
    if let Some(declared_length) = declared_length {
        anyhow::ensure!(
            actual_length == declared_length,
            "the update artifact does not match the length the feed declared"
        );
    }
    verify_artifact(&archive, &item)?;
    stage(archive, directory)
}

#[cfg(unix)]
fn artifact_name() -> &'static str {
    "update.tar.gz"
}

#[cfg(windows)]
fn artifact_name() -> &'static str {
    "Orbit-Setup.exe"
}

#[cfg(not(any(unix, windows)))]
fn artifact_name() -> &'static str {
    "update.bin"
}

/// Extract the verified archive into a staging directory beside the install,
/// so the helper's swap is a same-filesystem rename.
#[cfg(unix)]
fn stage_artifact(
    archive: PathBuf,
    temporary: PathBuf,
    layout: &InstallLayout,
) -> anyhow::Result<Option<StagedUpdate>> {
    let staging = create_unique_sibling(layout, "update")?;
    let payload = staging.join("payload");
    std::fs::create_dir_all(&payload)?;
    let extracted = extract_release_archive(&archive, &payload).and_then(|_| {
        anyhow::ensure!(
            staged_executable(&payload, &layout.exe_name).is_file(),
            "the update archive is missing its executable"
        );
        Ok(())
    });
    let _ = std::fs::remove_dir_all(&temporary);
    if let Err(error) = extracted {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(error);
    }
    Ok(Some(StagedUpdate::new(payload, staging)))
}

#[cfg(windows)]
fn stage_artifact(archive: PathBuf, directory: PathBuf) -> anyhow::Result<Option<StagedUpdate>> {
    Ok(Some(StagedUpdate::new(archive, directory)))
}

#[cfg(not(any(unix, windows)))]
fn stage_artifact(_archive: PathBuf, _directory: PathBuf) -> anyhow::Result<Option<StagedUpdate>> {
    Ok(None)
}

/// Reserve an unused sibling of the install for staging or probing.
#[cfg(unix)]
fn create_unique_sibling(layout: &InstallLayout, kind: &str) -> anyhow::Result<PathBuf> {
    for _ in 0..100 {
        let candidate = layout.parent.join(format!(
            ".{}.{kind}-{}-{}",
            layout.prefix_name,
            std::process::id(),
            unique_nonce()
        ));
        match std::fs::create_dir(&candidate) {
            Ok(()) => return Ok(candidate),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
    anyhow::bail!("could not reserve an update staging directory")
}

/// Extract a signed release archive, stripping its single top-level directory
/// and refusing anything that could escape the staging tree.
#[cfg(unix)]
fn extract_release_archive(path: &Path, destination: &Path) -> anyhow::Result<()> {
    use std::collections::HashSet;
    use std::ffi::OsString;

    let decoder = flate2::read::GzDecoder::new(std::fs::File::open(path)?);
    let mut archive = tar::Archive::new(decoder);
    let mut root: Option<OsString> = None;
    let mut seen = HashSet::new();
    let mut unpacked_bytes = 0_u64;
    let mut entry_count = 0_usize;

    for entry in archive.entries()? {
        let mut entry = entry?;
        entry_count += 1;
        anyhow::ensure!(
            entry_count <= MAX_ARCHIVE_ENTRIES,
            "the update archive contains too many entries"
        );

        let archived_path = entry.path()?.into_owned();
        let mut components = archived_path.components();
        let top = match components.next() {
            Some(std::path::Component::Normal(top)) => top.to_os_string(),
            _ => anyhow::bail!("the update archive contains an invalid path"),
        };
        if let Some(expected) = &root {
            anyhow::ensure!(
                expected == &top,
                "the update archive has more than one top-level directory"
            );
        } else {
            root = Some(top);
        }

        let mut relative = PathBuf::new();
        for component in components {
            match component {
                std::path::Component::Normal(component) => relative.push(component),
                _ => anyhow::bail!("the update archive contains an unsafe path"),
            }
        }
        if relative.as_os_str().is_empty() {
            anyhow::ensure!(
                entry.header().entry_type().is_dir(),
                "the update archive root is not a directory"
            );
            continue;
        }
        anyhow::ensure!(
            seen.insert(relative.clone()),
            "the update archive contains a duplicate path"
        );
        let entry_type = entry.header().entry_type();
        anyhow::ensure!(
            entry_type.is_file() || entry_type.is_dir(),
            "the update archive contains a link or special file"
        );
        unpacked_bytes = unpacked_bytes
            .checked_add(entry.header().size()?)
            .ok_or_else(|| anyhow::anyhow!("the update archive size overflowed"))?;
        anyhow::ensure!(
            unpacked_bytes <= MAX_UNPACKED_BYTES,
            "the update archive expands beyond the safety limit"
        );

        let output = destination.join(relative);
        if let Some(parent) = output.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut file = std::fs::File::create(&output)?;
        std::io::copy(&mut entry, &mut file)?;
    }

    anyhow::ensure!(root.is_some(), "the update archive is empty");
    Ok(())
}

fn verify_artifact(path: &Path, item: &feed::AppcastItem) -> anyhow::Result<()> {
    let key = verifying_key().ok_or_else(|| anyhow::anyhow!("the public key is unusable"))?;
    let signature = base64::engine::general_purpose::STANDARD
        .decode(item.signature.trim())
        .ok()
        .and_then(|bytes| <[u8; 64]>::try_from(bytes).ok())
        .map(|bytes| ed25519_dalek::Signature::from_bytes(&bytes))
        .ok_or_else(|| anyhow::anyhow!("the update signature is malformed"))?;
    let bytes = std::fs::read(path)?;
    key.verify_strict(&bytes, &signature)
        .map_err(|_| anyhow::anyhow!("the update artifact failed signature verification"))
}

fn verifying_key() -> Option<ed25519_dalek::VerifyingKey> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(PUBLIC_ED_KEY.trim())
        .ok()?;
    ed25519_dalek::VerifyingKey::from_bytes(&<[u8; 32]>::try_from(bytes).ok()?).ok()
}

fn validate_release_version(version: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        !version.is_empty()
            && version.len() <= 64
            && version.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'+' | b'_')
            }),
        "the update feed contains an invalid version"
    );
    Ok(())
}

fn validate_download_url(value: &str) -> anyhow::Result<()> {
    let url = url::Url::parse(value)?;
    anyhow::ensure!(
        url.scheme() == "https",
        "the update feed points at a non-https url"
    );
    Ok(())
}

// ── http ────────────────────────────────────────────────────────────────

fn http_client(timeout_seconds: u64) -> anyhow::Result<reqwest::blocking::Client> {
    Ok(reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(timeout_seconds))
        .build()?)
}

fn http_get(url: &str) -> anyhow::Result<String> {
    let response = http_client(30)?
        .get(url)
        .header("User-Agent", concat!("Orbit/", env!("CARGO_PKG_VERSION")))
        .send()?
        .error_for_status()?;
    let bytes = response.bytes()?;
    anyhow::ensure!(
        bytes.len() as u64 <= MAX_FEED_BYTES,
        "the update feed is implausibly large"
    );
    Ok(String::from_utf8(bytes.to_vec())?)
}

fn download_to(
    url: &str,
    destination: &Path,
    timeout_seconds: u64,
    limit: u64,
) -> anyhow::Result<()> {
    use std::io::Read as _;

    let mut response = http_client(timeout_seconds)?
        .get(url)
        .header("User-Agent", concat!("Orbit/", env!("CARGO_PKG_VERSION")))
        .send()?
        .error_for_status()?;
    let mut file = std::fs::File::create(destination)?;
    let copied = std::io::copy(&mut response.by_ref().take(limit + 1), &mut file)?;
    anyhow::ensure!(
        copied <= limit,
        "the downloaded update exceeds its safety limit"
    );
    file.sync_all()?;
    Ok(())
}

// ── preferences and errors ──────────────────────────────────────────────

fn preference_path() -> Option<PathBuf> {
    Some(home_dir()?.join(".orbit-pi").join("updater.json"))
}

fn home_dir() -> Option<PathBuf> {
    crate::platform::home_dir_opt()
}

/// Orbit's default is to check automatically; treat an absent or unreadable
/// file as "not answered yet".
fn read_automatic_preference(path: &Path) -> bool {
    let Ok(contents) = std::fs::read_to_string(path) else {
        return true;
    };
    serde_json::from_str::<serde_json::Value>(&contents)
        .ok()
        .and_then(|value| value.get("automatic")?.as_bool())
        .unwrap_or(true)
}

fn write_automatic_preference(path: &Path, enabled: bool) {
    use std::io::Write as _;
    let Some(directory) = path.parent() else {
        return;
    };
    if std::fs::create_dir_all(directory).is_err() {
        return;
    }
    // Replace through a temporary file so a crash mid-write cannot leave the
    // preference unreadable.
    let temporary = path.with_extension("json.tmp");
    let written = std::fs::File::create(&temporary).and_then(|mut file| {
        file.write_all(format!("{{\n  \"automatic\": {enabled}\n}}\n").as_bytes())?;
        file.sync_all()
    });
    if written.is_ok() {
        let _ = std::fs::rename(&temporary, path);
    } else {
        let _ = std::fs::remove_file(&temporary);
    }
}

#[cfg(unix)]
fn take_update_error(layout: &InstallLayout) -> Option<String> {
    use std::io::Read as _;
    let path = layout.error_path();
    let metadata = std::fs::symlink_metadata(&path).ok()?;
    if !metadata.file_type().is_file() || metadata.len() > MAX_ERROR_BYTES {
        return None;
    }
    let mut contents = String::new();
    std::fs::File::open(&path)
        .ok()?
        .take(MAX_ERROR_BYTES)
        .read_to_string(&mut contents)
        .ok()?;
    let _ = std::fs::remove_file(path);
    let contents = contents.trim();
    (!contents.is_empty()).then(|| contents.to_owned())
}

// ── small utilities ─────────────────────────────────────────────────────

fn unique_temp_directory(kind: &str) -> anyhow::Result<PathBuf> {
    for _ in 0..100 {
        let candidate =
            std::env::temp_dir().join(format!("{kind}-{}-{}", std::process::id(), unique_nonce()));
        match std::fs::create_dir(&candidate) {
            Ok(()) => return Ok(candidate),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
    anyhow::bail!("could not reserve an update staging directory")
}

static NONCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn unique_nonce() -> u64 {
    NONCE.fetch_add(1, Ordering::Relaxed)
}

/// Reading a Sparkle appcast.
///
/// Kept off the platform seams so the ordering and parsing rules — the part
/// that decides which build a user is offered — run under `cargo test` on
/// every host, not only the one that ships an updater.
mod feed {
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub(super) struct AppcastItem {
        pub(super) version: String,
        pub(super) url: String,
        pub(super) signature: String,
        pub(super) length: Option<u64>,
    }

    /// The newest signed item in a Sparkle appcast. The native feed writers
    /// emit one `<item>` per release, each with a `sparkle:shortVersionString`
    /// and a signed `<enclosure>`; items without a signature are ignored.
    pub(super) fn newest_item(feed: &str) -> Option<AppcastItem> {
        feed.split("<item>")
            .skip(1)
            .filter_map(|item| {
                let item = item.split("</item>").next()?;
                let enclosure = item.split_once("<enclosure")?.1.split_once('>')?.0;
                Some(AppcastItem {
                    version: element(item, "sparkle:shortVersionString")
                        .or_else(|| attribute(enclosure, "sparkle:shortVersionString"))?,
                    url: attribute(enclosure, "url")?,
                    signature: attribute(enclosure, "sparkle:edSignature")?,
                    length: attribute(enclosure, "length").and_then(|it| it.parse().ok()),
                })
            })
            .max_by(|left, right| compare_versions(&left.version, &right.version))
    }

    fn attribute(tag: &str, name: &str) -> Option<String> {
        let needle = format!("{name}=\"");
        let value = tag.split_once(&needle)?.1.split_once('"')?.0;
        (!value.is_empty()).then(|| value.to_owned())
    }

    fn element(item: &str, name: &str) -> Option<String> {
        let value = item
            .split_once(&format!("<{name}>"))?
            .1
            .split_once(&format!("</{name}>"))?
            .0
            .trim();
        (!value.is_empty()).then(|| value.to_owned())
    }

    pub(super) fn is_newer(candidate: &str, current: &str) -> bool {
        compare_versions(candidate, current) == std::cmp::Ordering::Greater
    }

    /// Compare dotted release numbers field by field; anything after a `-` or
    /// `+` is build metadata and is not ordered.
    fn compare_versions(left: &str, right: &str) -> std::cmp::Ordering {
        fn fields(version: &str) -> impl Iterator<Item = u64> + '_ {
            version
                .split(['-', '+'])
                .next()
                .unwrap_or(version)
                .split('.')
                .map(|field| field.trim().parse::<u64>().unwrap_or(0))
        }

        let mut left = fields(left);
        let mut right = fields(right);
        loop {
            match (left.next(), right.next()) {
                (None, None) => return std::cmp::Ordering::Equal,
                (left, right) => {
                    let ordering = left.unwrap_or(0).cmp(&right.unwrap_or(0));
                    if ordering != std::cmp::Ordering::Equal {
                        return ordering;
                    }
                }
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        const FEED: &str = r#"<?xml version="1.0" standalone="yes"?>
<rss xmlns:sparkle="http://www.andymatuschak.org/xml-namespaces/sparkle" version="2.0">
  <channel>
    <item>
      <title>0.1.4</title>
      <sparkle:shortVersionString>0.1.4</sparkle:shortVersionString>
      <enclosure url="https://releases.orbit.dev/Orbit-0.1.4.tar.gz" length="1024" type="application/octet-stream" sparkle:edSignature="oldsig" />
    </item>
    <item>
      <title>0.2.0</title>
      <sparkle:shortVersionString>0.2.0</sparkle:shortVersionString>
      <enclosure url="https://releases.orbit.dev/Orbit-0.2.0.tar.gz" length="2048" type="application/octet-stream" sparkle:edSignature="newsig" />
    </item>
  </channel>
</rss>"#;

        #[test]
        fn the_newest_signed_item_wins_regardless_of_feed_order() {
            let item = newest_item(FEED).expect("feed has signed items");
            assert_eq!(item.version, "0.2.0");
            assert_eq!(item.signature, "newsig");
            assert_eq!(item.length, Some(2048));
            assert!(item.url.ends_with("Orbit-0.2.0.tar.gz"));
        }

        #[test]
        fn an_unsigned_enclosure_is_never_offered() {
            let unsigned = FEED.replace(" sparkle:edSignature=\"newsig\"", "");
            let item = newest_item(&unsigned).expect("the signed item remains");
            assert_eq!(item.version, "0.1.4");
        }

        #[test]
        fn a_feed_without_signed_items_offers_nothing() {
            assert_eq!(newest_item("<rss></rss>"), None);
            assert_eq!(newest_item(""), None);
        }

        #[test]
        fn versions_compare_field_by_field_not_lexically() {
            assert!(is_newer("0.10.0", "0.9.0"));
            assert!(is_newer("0.1.10", "0.1.9"));
            assert!(!is_newer("0.1.4", "0.1.4"));
            assert!(!is_newer("0.1.3", "0.1.4"));
            assert!(!is_newer("1.2", "1.2.0"));
            assert!(is_newer("1.2.1", "1.2"));
            assert!(!is_newer("1.2.0+build.7", "1.2.0"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_absent_preference_file_leaves_automatic_checks_on() {
        let directory =
            std::env::temp_dir().join(format!("orbit-updater-preference-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        let path = directory.join("updater.json");

        assert!(read_automatic_preference(&path));
        write_automatic_preference(&path, false);
        assert!(!read_automatic_preference(&path));
        write_automatic_preference(&path, true);
        assert!(read_automatic_preference(&path));

        let _ = std::fs::remove_dir_all(&directory);
    }

    #[test]
    fn release_versions_cannot_become_paths() {
        assert!(validate_release_version("1.2.3-beta.1").is_ok());
        assert!(validate_release_version("../../tmp/payload").is_err());
        assert!(validate_release_version("1.2.3/evil").is_err());
        assert!(validate_release_version("").is_err());
    }

    /// The interop the whole update path rests on: a signature produced by
    /// the release tooling verifies here, and a tampered payload does not.
    #[test]
    fn a_release_signature_verifies_and_tampering_does_not() {
        const PUBLIC: &str = "7gZ3dbx+MPQD4vc2dk7olL9QU66JIjpJ1iqNNafU2lQ=";
        const SIGNATURE: &str = "eBIPKGvQSxFIVNwOzNjzHYs/AGiYFIe3pGulv0TeocoMN0+0l28OJZrlJ2ZuQnNBfif10VW3virGo+7GP3TwCw==";
        const PAYLOAD: &[u8] = b"Waku-0.0.0-x86_64-Setup.exe contents";

        let decode = |value: &str| {
            base64::engine::general_purpose::STANDARD
                .decode(value)
                .expect("the test vector is valid base64")
        };
        let key = ed25519_dalek::VerifyingKey::from_bytes(
            &<[u8; 32]>::try_from(decode(PUBLIC)).expect("32-byte public key"),
        )
        .expect("the test vector is a valid key");
        let signature = ed25519_dalek::Signature::from_bytes(
            &<[u8; 64]>::try_from(decode(SIGNATURE)).expect("64-byte signature"),
        );

        key.verify_strict(PAYLOAD, &signature)
            .expect("a release signature must verify");
        assert!(
            key.verify_strict(b"tampered installer", &signature)
                .is_err(),
            "a modified download must not verify"
        );
    }
}
