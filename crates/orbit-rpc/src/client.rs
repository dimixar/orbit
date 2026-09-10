//! Process lifecycle + JSONL transport for the pi CLI RPC protocol.
//!
//! One `PiClient` owns one `pi --mode rpc` child process:
//! - a **writer thread** drains queued commands onto stdin (JSON lines)
//! - a **reader thread** parses stdout lines into [`Event`]s, forwarding them
//!   to the shared event stream and routing `response` events by `id` to the
//!   `Receiver` each `send_command` returns
//! - a **stderr drain thread** keeps a capped ring of recent stderr lines so a
//!   chatty child can never block on a full pipe
//!
//! The protocol mandates strict LF framing; we use `read_until(b'\n')` on the
//! byte level so U+2028/U+2029 inside JSON strings are never treated as line
//! breaks (`std::io::BufRead::lines` is safe too, but byte-exact is explicit).

use std::{
    collections::{HashMap, VecDeque},
    ffi::OsString,
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStderr, ChildStdin, ChildStdout, Stdio},
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc, Mutex,
    },
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{anyhow, Context as _, Result};

use crate::types::{CommandBody, Event};

/// Environment override for the pi binary (default: `pi` on PATH).
pub const PI_BIN_ENV: &str = "PI_BIN";
/// Cap on retained stderr lines (dropped oldest first).
const STDERR_RING_CAP: usize = 200;

/// Common install locations probed for `pi` when it isn't on `PATH`. `pi` is
/// a Node script installed via Homebrew, so the launcher and `node` both live
/// under these `bin` dirs. A bundled `.app` is launched with a minimal PATH
/// (`/usr/bin:/bin:/usr/sbin:/sbin`), so we probe these to keep packaged
/// builds working without the user exporting anything.
const PI_SEARCH_DIRS: &[&str] = &[
    "/opt/homebrew/bin",
    "/usr/local/bin",
    "/opt/local/bin",
    "/usr/bin",
    "/bin",
];
/// Dirs prepended to the child's PATH so `node` (and `pi`) resolve from a
/// bundled app launch.
const PATH_EXTRA_DIRS: &[&str] = &["/opt/homebrew/bin", "/usr/local/bin", "/opt/local/bin"];

pub struct PiClient {
    child: Child,
    commands_tx: Sender<Outgoing>,
    events_rx: Receiver<Event>,
    pending: Arc<Mutex<HashMap<String, Sender<Event>>>>,
    stderr_ring: Arc<Mutex<VecDeque<String>>>,
    next_id: std::sync::atomic::AtomicU64,
}

struct Outgoing {
    wire: String,
}

impl PiClient {
    /// Spawn `pi --mode rpc` rooted at `workspace_dir`.
    ///
    /// `session_dir: None` uses pi's default storage (`~/.pi/agent/sessions/`)
    /// so sessions created here are the same ones the CLI sees. Pass an
    /// explicit dir to isolate (tests, throwaway demos).
    pub fn spawn(workspace_dir: &Path, session_dir: Option<&Path>) -> Result<Self> {
        let bin = resolve_pi_bin();
        let mut command = std::process::Command::new(&bin);
        // Waku parity: `--approve` auto-approves tool calls so the RPC
        // session never stalls on an approval dialog it cannot render, and
        // the version check is noise for a child we just spawned.
        command
            .args(["--mode", "rpc", "--approve"])
            .env("PI_SKIP_VERSION_CHECK", "1")
            // Augment PATH with Homebrew-style dirs so `node` (required by
            // pi's `#!/usr/bin/env node` shebang) resolves when launched from
            // a bundled `.app`.
            .env("PATH", augmented_path())
            .current_dir(workspace_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(dir) = session_dir {
            command.arg("--session-dir").arg(dir);
        }
        let mut child = command
            .spawn()
            .with_context(|| format!("failed to spawn `{bin} --mode rpc`"))?;

        let stdin = child.stdin.take().context("pi stdin unavailable")?;
        let stdout = child.stdout.take().context("pi stdout unavailable")?;
        let stderr = child.stderr.take().context("pi stderr unavailable")?;

        let (commands_tx, commands_rx) = mpsc::channel::<Outgoing>();
        let (events_tx, events_rx) = mpsc::channel::<Event>();
        let pending = Arc::new(Mutex::new(HashMap::<String, Sender<Event>>::new()));
        let stderr_ring: Arc<Mutex<VecDeque<String>>> = Arc::new(Mutex::new(VecDeque::new()));

        Self::spawn_writer(stdin, commands_rx);
        Self::spawn_reader(stdout, &events_tx, &pending);
        Self::spawn_stderr_drain(stderr, &stderr_ring);

        Ok(Self {
            child,
            commands_tx,
            events_rx,
            pending,
            stderr_ring,
            next_id: std::sync::atomic::AtomicU64::new(1),
        })
    }

    /// Send a command and get the receiver for its matched `response` event.
    pub fn send(&self, body: CommandBody) -> Result<Receiver<Event>> {
        let id = self.next_id_fetch();
        let command = crate::types::Command::new(id.clone(), body);
        let wire = command.to_wire()?;
        let (tx, rx) = mpsc::channel();
        self.pending.lock().unwrap().insert(id, tx);
        self.commands_tx
            .send(Outgoing { wire })
            .map_err(|_| anyhow!("pi process is not running"))?;
        Ok(rx)
    }

    /// Convenience for commands whose response we inspect immediately.
    pub fn send_blocking(&self, body: CommandBody, timeout: std::time::Duration) -> Result<Event> {
        let rx = self.send(body)?;
        rx.recv_timeout(timeout)
            .map_err(|e| anyhow!("no response within {timeout:?}: {e}"))
    }

    /// The shared stream of all events (responses included).
    pub fn events(&self) -> &Receiver<Event> {
        &self.events_rx
    }

    /// Drain all events currently buffered on the shared stream.
    pub fn drain_events(&self) -> Vec<Event> {
        let mut out = Vec::new();
        while let Ok(event) = self.events_rx.try_recv() {
            out.push(event);
        }
        out
    }

    /// Drain recent stderr lines written by pi.
    pub fn drain_stderr(&self) -> Vec<String> {
        let mut ring = self.stderr_ring.lock().unwrap();
        ring.drain(..).collect()
    }

    pub fn child_pid(&self) -> u32 {
        self.child.id()
    }

    /// True while the pi process is still running.
    pub fn is_alive(&mut self) -> bool {
        self.child.try_wait().map(|s| s.is_none()).unwrap_or(false)
    }

    fn next_id_fetch(&self) -> String {
        let n = self
            .next_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos();
        format!("orbit-{n}-{nanos:x}")
    }

    fn spawn_writer(mut stdin: ChildStdin, commands_rx: Receiver<Outgoing>) {
        thread::Builder::new()
            .name("orbit-pi-writer".into())
            .spawn(move || {
                while let Ok(outgoing) = commands_rx.recv() {
                    if write_json_line(&mut stdin, &outgoing.wire).is_err() {
                        break;
                    }
                }
            })
            .expect("spawn pi writer thread");
    }

    fn spawn_reader(
        stdout: ChildStdout,
        events_tx: &Sender<Event>,
        pending: &Arc<Mutex<HashMap<String, Sender<Event>>>>,
    ) {
        let events_tx = events_tx.clone();
        // The pending map survives in the client; the reader needs its own Arc.
        let pending = Arc::clone(pending);
        let routed_tx = events_tx.clone();
        thread::Builder::new()
            .name("orbit-pi-reader".into())
            .spawn(move || {
                let mut reader = BufReader::new(stdout);
                let mut line = Vec::new();
                loop {
                    line.clear();
                    match reader.read_until(b'\n', &mut line) {
                        Ok(0) => break, // EOF — pi exited
                        Ok(_) => {
                            // Strip a single trailing \r per spec (accept \r\n),
                            // then any dangling \n.
                            let mut line = line.as_slice();
                            while line.last().is_some_and(|b| *b == b'\n' || *b == b'\r') {
                                line = &line[..line.len() - 1];
                            }
                            if line.is_empty() {
                                continue;
                            }
                            let line = String::from_utf8_lossy(line);
                            let event = Event::parse_line(&line);
                            if let Event::Response { id, .. } = &event {
                                if let Some(tx) = pending.lock().unwrap().remove(id) {
                                    let _ = tx.send(event.clone());
                                }
                            }
                            if let Event::Response { .. } = &event {
                                let _ = routed_tx.send(event);
                            } else {
                                let _ = events_tx.send(event);
                            }
                        }
                        Err(_) => break,
                    }
                }
                let _ = events_tx.send(Event::ProcessExited);
            })
            .expect("spawn pi reader thread");
    }

    fn spawn_stderr_drain(stderr: ChildStderr, ring: &Arc<Mutex<VecDeque<String>>>) {
        let ring = Arc::clone(ring);
        thread::Builder::new()
            .name("orbit-pi-stderr".into())
            .spawn(move || {
                let reader = BufReader::new(stderr);
                for line in reader.lines().map_while(Result::ok) {
                    let mut ring = ring.lock().unwrap();
                    if ring.len() >= STDERR_RING_CAP {
                        ring.pop_front();
                    }
                    ring.push_back(line);
                }
            })
            .expect("spawn pi stderr thread");
    }

    /// Send a dialog answer for `extension_ui_request` ids (select/confirm/input).
    pub fn respond_dialog(&self, id: &str, answer: serde_json::Value) -> Result<()> {
        let wire =
            serde_json::json!({ "type": "extension_ui_response", "id": id, "value": answer });
        self.commands_tx
            .send(Outgoing {
                wire: serde_json::to_string(&wire)?,
            })
            .map_err(|_| anyhow!("pi process is not running"))
    }
}

fn write_json_line(stdin: &mut ChildStdin, line: &str) -> std::io::Result<()> {
    stdin.write_all(line.as_bytes())?;
    stdin.write_all(b"\n")?;
    stdin.flush()
}

impl Drop for PiClient {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        // Break the writer thread out of its channel recv; the write to the
        // dead child's stdin errors and the thread exits.
        let _ = self.commands_tx.send(Outgoing {
            wire: String::new(),
        });
    }
}

/// The resolved `pi` executable path, for one-shot subprocesses (e.g. the
/// commit-message generator) that do not go through [`PiClient`].
pub fn pi_binary() -> String {
    resolve_pi_bin()
}

/// Resolve the `pi` executable to spawn.
///
/// `PI_BIN` (if set) wins. Otherwise we look for `pi` on `PATH`, then in the
/// common Homebrew/install dirs, and finally fall back to the bare name so
/// any spawn error still names something meaningful.
fn resolve_pi_bin() -> String {
    if let Ok(bin) = std::env::var(PI_BIN_ENV) {
        if !bin.is_empty() {
            return bin;
        }
    }
    let name = if cfg!(windows) { "pi.exe" } else { "pi" };
    if let Some(found) = find_on_path(name) {
        return found;
    }
    for dir in PI_SEARCH_DIRS {
        let candidate = Path::new(dir).join(name);
        if candidate.is_file() {
            return candidate.to_string_lossy().into_owned();
        }
    }
    name.to_string()
}

/// Walk `PATH` and return the first executable named `name` found on it.
fn find_on_path(name: &str) -> Option<String> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate.to_string_lossy().into_owned());
        }
    }
    None
}

/// The PATH handed to the child: common install dirs prepended to whatever
/// PATH the parent process has.
fn augmented_path() -> OsString {
    let mut dirs: Vec<PathBuf> = PATH_EXTRA_DIRS.iter().map(PathBuf::from).collect();
    if let Some(path) = std::env::var_os("PATH") {
        dirs.extend(std::env::split_paths(&path));
    }
    std::env::join_paths(dirs).unwrap_or_else(|_| std::env::var_os("PATH").unwrap_or_default())
}
