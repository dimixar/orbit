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

// ---------------------------------------------------------------------------
// Pi SSE server (agent/sse-server.ts) — the frontend talks to it over
// HTTP/SSE on localhost:8913. Auto-start it with the app if it isn't running.
// ---------------------------------------------------------------------------

const SSE_PORT: u16 = 8913;
const SSE_SERVER_ID: &str = "orbit-pi-sse";

/// Handle to the spawned SSE server so we can kill it when the app exits.
struct SseServerChild(Mutex<Option<Child>>);

fn sse_server_is_up() -> bool {
    let addr = match format!("127.0.0.1:{SSE_PORT}").to_socket_addrs() {
        Ok(mut addrs) => match addrs.next() {
            Some(addr) => addr,
            None => return false,
        },
        Err(_) => return false,
    };
    let mut stream = match TcpStream::connect_timeout(&addr, Duration::from_millis(300)) {
        Ok(stream) => stream,
        Err(_) => return false,
    };
    let _ = stream.set_read_timeout(Some(Duration::from_millis(700)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(700)));
    if stream
        .write_all(b"GET /health HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .is_err()
    {
        return false;
    }
    let mut response = String::new();
    if stream.read_to_string(&mut response).is_err() {
        return false;
    }
    response.starts_with("HTTP/1.1 200") && response.contains(SSE_SERVER_ID)
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

    match spawn_sse_server(app) {
        Ok((child, how)) => {
            eprintln!("[orbit] starting SSE server via {how} (pid {})", child.id());
            app.manage(SseServerChild(Mutex::new(Some(child))));
            if wait_for_sse_server(Duration::from_secs(15)) {
                eprintln!("[orbit] SSE server is up on port {SSE_PORT}");
            } else {
                eprintln!("[orbit] warning: SSE server did not come up within 15s");
            }
        }
        Err(err) => {
            eprintln!("[orbit] failed to start SSE server: {err}");
        }
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
        .invoke_handler(tauri::generate_handler![greet])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app_handle, event| {
        if let tauri::RunEvent::Exit = event {
            // Kill the SSE server we spawned so it doesn't outlive the app.
            if let Some(state) = app_handle.try_state::<SseServerChild>() {
                if let Ok(mut guard) = state.0.lock() {
                    if let Some(mut child) = guard.take() {
                        eprintln!("[orbit] stopping SSE server (pid {})", child.id());
                        let _ = child.kill();
                        let _ = child.wait();
                    }
                }
            }
        }
    });
}
