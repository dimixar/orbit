use std::{
    io::{Read, Write},
    net::{TcpStream, ToSocketAddrs},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::Mutex,
    thread,
    time::{Duration, Instant},
};

use tauri::{path::BaseDirectory, AppHandle, Manager};

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

/// Read a user-picked or dropped file for the composer (images → data URLs).
#[tauri::command]
fn read_local_file(path: String) -> Result<Vec<u8>, String> {
    std::fs::read(&path).map_err(|e| format!("couldn't read {path}: {e}"))
}

// ---------------------------------------------------------------------------
// Pi SSE server (agent/sse-server.ts) — the frontend talks to it over
// HTTP/SSE on localhost:8913. Auto-start it with the app if it isn't running.
// ---------------------------------------------------------------------------

const SSE_PORT: u16 = 8913;
const SSE_SERVER_ID: &str = "orbit-pi-sse";

/// Handle to the spawned SSE server so we can kill it when the app exits.
struct SseServerChild(Mutex<Option<Child>>);

fn parse_health_pid(body: &str) -> Option<u32> {
    let key = "\"pid\"";
    let start = body.find(key)?;
    let rest = body[start + key.len()..].trim_start();
    let rest = rest.strip_prefix(':')?.trim_start();
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

fn sse_health_response() -> Option<String> {
    let addr = format!("127.0.0.1:{SSE_PORT}").to_socket_addrs().ok()?.next()?;
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_millis(300)).ok()?;
    let _ = stream.set_read_timeout(Some(Duration::from_millis(700)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(700)));
    stream
        .write_all(b"GET /health HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .ok()?;
    let mut response = String::new();
    stream.read_to_string(&mut response).ok()?;
    if response.starts_with("HTTP/1.1 200") && response.contains(SSE_SERVER_ID) {
        Some(response)
    } else {
        None
    }
}

fn sse_server_is_up() -> bool {
    sse_health_response().is_some()
}

fn kill_process(pid: u32) {
    #[cfg(windows)]
    {
        let _ = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/F"])
            .status();
    }
    #[cfg(not(windows))]
    {
        let _ = Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .status();
    }
}

fn take_stored_sse_child(app: &AppHandle) -> Option<Child> {
    let state = app.try_state::<SseServerChild>()?;
    let mut guard = state.0.lock().ok()?;
    guard.take()
}

fn store_sse_child(app: &AppHandle, child: Child) {
    if let Some(state) = app.try_state::<SseServerChild>() {
        if let Ok(mut guard) = state.0.lock() {
            *guard = Some(child);
            return;
        }
    }
    app.manage(SseServerChild(Mutex::new(Some(child))));
}

fn wait_for_sse_server_down(timeout: Duration) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if !sse_server_is_up() {
            return true;
        }
        thread::sleep(Duration::from_millis(150));
    }
    false
}

fn stop_sse_server(app: &AppHandle) {
    let health_pid = sse_health_response().as_deref().and_then(parse_health_pid);
    if let Some(mut child) = take_stored_sse_child(app) {
        eprintln!("[orbit] stopping SSE server (pid {})", child.id());
        let _ = child.kill();
        let _ = child.wait();
    }
    if let Some(pid) = health_pid {
        if sse_server_is_up() {
            eprintln!("[orbit] stopping SSE server (pid {pid})");
            kill_process(pid);
        }
    }
}

fn start_sse_server(app: &AppHandle) -> Result<(), String> {
    let (child, how) = spawn_sse_server(app)?;
    eprintln!("[orbit] starting SSE server via {how} (pid {})", child.id());
    store_sse_child(app, child);
    if wait_for_sse_server(Duration::from_secs(15)) {
        eprintln!("[orbit] SSE server is up on port {SSE_PORT}");
        Ok(())
    } else {
        Err("SSE server did not come up within 15s".to_string())
    }
}

#[tauri::command]
fn restart_sse_server(app: AppHandle) -> Result<(), String> {
    stop_sse_server(&app);
    if !wait_for_sse_server_down(Duration::from_secs(8)) {
        return Err("could not stop the pi agent".to_string());
    }
    start_sse_server(&app)
}

/// Poll until the SSE server accepts connections (or we give up).
fn wait_for_sse_server(timeout: Duration) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if sse_server_is_up() {
            return true;
        }
        thread::sleep(Duration::from_millis(250));
    }
    false
}

fn spawn_source_sse_server() -> Result<Option<(Child, String)>, String> {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let tsx_cli = repo.join("node_modules/tsx/dist/cli.mjs");
    let server = repo.join("agent/sse-server.ts");
    if tsx_cli.is_file() && server.is_file() {
        let child = spawn_node(&[&tsx_cli, &server], Some(&repo))?;
        return Ok(Some((child, "tsx agent/sse-server.ts".to_string())));
    }
    Ok(None)
}

fn spawn_bundled_sse_server(app: &AppHandle) -> Result<Option<(Child, String)>, String> {
    if let Ok(bundled) = app
        .path()
        .resolve("bin/pi-sse.mjs", BaseDirectory::Resource)
    {
        if bundled.is_file() {
            let child = spawn_node(&[&bundled], None)?;
            return Ok(Some((child, format!("bundled {}", bundled.display()))));
        }
    }
    Ok(None)
}

/// Spawns the SSE server. Dev prefers the TypeScript source so Pi can load
/// extension packages exactly like `pnpm agent:sse`; release uses the bundled
/// build created by `pnpm agent:sse:build`.
fn spawn_sse_server(app: &AppHandle) -> Result<(Child, String), String> {
    if cfg!(debug_assertions) {
        if let Some(spawned) = spawn_source_sse_server()? {
            return Ok(spawned);
        }
        if let Some(spawned) = spawn_bundled_sse_server(app)? {
            return Ok(spawned);
        }
    } else {
        if let Some(spawned) = spawn_bundled_sse_server(app)? {
            return Ok(spawned);
        }
        if let Some(spawned) = spawn_source_sse_server()? {
            return Ok(spawned);
        }
    }

    Err("could not locate pi-sse.mjs resource or agent/sse-server.ts".to_string())
}

fn spawn_node(args: &[&PathBuf], cwd: Option<&PathBuf>) -> Result<Child, String> {
    let mut cmd = Command::new("node");
    for arg in args {
        cmd.arg(arg);
    }
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    cmd.stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    cmd.spawn()
        .map_err(|e| format!("failed to spawn `node` ({args:?}): {e}"))
}

/// Called from the Tauri setup hook: starts the SSE server if it isn't
/// already listening on localhost:8913.
fn ensure_sse_server(app: &AppHandle) {
    if sse_server_is_up() {
        eprintln!("[orbit] SSE server already running on port {SSE_PORT}");
        return;
    }

    if let Err(err) = start_sse_server(app) {
        eprintln!("[orbit] failed to start SSE server: {err}");
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            ensure_sse_server(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            read_local_file,
            restart_sse_server
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app_handle, event| {
        if let tauri::RunEvent::Exit = event {
            // Kill the SSE server we spawned so it doesn't outlive the app.
            stop_sse_server(app_handle);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::parse_health_pid;

    #[test]
    fn parse_health_pid_reads_json_field() {
        assert_eq!(
            parse_health_pid(
                "HTTP/1.1 200 OK\r\n\r\n{\"id\":\"orbit-pi-sse\",\"ok\":true,\"pid\":4321}"
            ),
            Some(4321)
        );
        assert_eq!(parse_health_pid("no pid here"), None);
    }
}
