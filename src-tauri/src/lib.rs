use std::{
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
    TcpStream::connect_timeout(&addr, Duration::from_millis(300)).is_ok()
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

/// Spawns the SSE server, preferring the bundled production build
/// (`src-tauri/bin/pi-sse.mjs`, created by `pnpm agent:sse:build`) and falling
/// back to running the TypeScript source via tsx in the dev repo.
fn spawn_sse_server(app: &AppHandle) -> Result<(Child, String), String> {
    // 1. Bundled build (release).
    if let Ok(bundled) = app
        .path()
        .resolve("bin/pi-sse.mjs", BaseDirectory::Resource)
    {
        if bundled.is_file() {
            let child = spawn_node(&[&bundled], None)?;
            return Ok((child, format!("bundled {}", bundled.display())));
        }
    }

    // 2. Dev repo — check the repo first so we don't double-spawn when the
    //    production resource is simply absent.
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let tsx_cli = repo.join("node_modules/tsx/dist/cli.mjs");
    let server = repo.join("agent/sse-server.ts");
    if tsx_cli.is_file() && server.is_file() {
        let child = spawn_node(&[&tsx_cli, &server], Some(&repo))?;
        return Ok((child, "tsx agent/sse-server.ts".to_string()));
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
