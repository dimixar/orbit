//! macOS helpers for detecting installed folder-capable apps and opening a
//! workspace path in one of them (editors, terminals, Finder, etc.).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui::Image;
use serde_json::Value;

/// A folder-capable application the header's "open in" control can target,
/// resolved against what is installed on this machine.
#[derive(Clone)]
pub struct ExternalApp {
    /// Stable identifier persisted as the user's preferred target.
    pub id: &'static str,
    pub label: &'static str,
    /// The bundle id that resolved here, for launching.
    pub bundle_id: &'static str,
    pub icon: Arc<Image>,
}

/// Known folder-capable apps in menu order — editors, the file manager,
/// terminals, IDEs. An entry lists every bundle id it ships under; the first
/// installed one wins.
#[cfg(target_os = "macos")]
const TERMY_BUNDLE_ID: &str = "com.lassevestergaard.termy";

#[cfg(target_os = "macos")]
const OPEN_IN_CATALOG: &[(&str, &str, &[&str])] = &[
    ("vscode", "VS Code", &["com.microsoft.VSCode"]),
    ("cursor", "Cursor", &["com.todesktop.230313mzl4w4u92"]),
    ("zed", "Zed", &["dev.zed.Zed", "dev.zed.Zed-Preview"]),
    ("finder", "Finder", &["com.apple.finder"]),
    ("terminal", "Terminal", &["com.apple.Terminal"]),
    ("termy", "Termy", &[TERMY_BUNDLE_ID]),
    ("iterm2", "iTerm2", &["com.googlecode.iterm2"]),
    ("kitty", "Kitty", &["net.kovidgoyal.kitty"]),
    ("ghostty", "Ghostty", &["com.mitchellh.ghostty"]),
    ("warp", "Warp", &["dev.warp.Warp-Stable", "dev.warp.Warp"]),
    ("xcode", "Xcode", &["com.apple.dt.Xcode"]),
    (
        "android-studio",
        "Android Studio",
        &["com.google.android.studio"],
    ),
];

#[cfg(target_os = "macos")]
fn app_icon_for_application_path(
    application_path: &objc2_foundation::NSString,
) -> Option<Arc<Image>> {
    use objc2_app_kit::{NSBitmapImageFileType, NSBitmapImageRep, NSWorkspace};
    use objc2_foundation::{NSDictionary, NSSize};

    let image = NSWorkspace::sharedWorkspace().iconForFile(application_path);
    image.setSize(NSSize::new(32.0, 32.0));
    let tiff = image.TIFFRepresentation()?;
    let bitmap_rep = NSBitmapImageRep::imageRepWithData(&tiff)?;
    let properties = NSDictionary::new();
    let png_data = unsafe {
        bitmap_rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &properties)
    }?;
    let bytes = unsafe { png_data.as_bytes_unchecked() };
    (!bytes.is_empty()).then(|| Arc::new(Image::from_bytes(gpui::ImageFormat::Png, bytes.to_vec())))
}

/// Resolve which catalog apps are installed, with their icons.
#[cfg(target_os = "macos")]
pub fn detect_open_in_apps() -> Vec<ExternalApp> {
    use objc2_app_kit::NSWorkspace;
    use objc2_foundation::NSString;

    let workspace = NSWorkspace::sharedWorkspace();
    OPEN_IN_CATALOG
        .iter()
        .filter_map(|&(id, label, bundle_ids)| {
            bundle_ids.iter().find_map(|&bundle_id| {
                let application_url = workspace
                    .URLForApplicationWithBundleIdentifier(&NSString::from_str(bundle_id))?;
                let application_path = application_url.path()?;
                Some(ExternalApp {
                    id,
                    label,
                    bundle_id,
                    icon: app_icon_for_application_path(&application_path)?,
                })
            })
        })
        .collect()
}

#[cfg(not(target_os = "macos"))]
pub fn detect_open_in_apps() -> Vec<ExternalApp> {
    Vec::new()
}

/// Open `path` in the application `bundle_id`, activating it. Launch Services
/// delivers the open asynchronously, so this never blocks.
#[cfg(target_os = "macos")]
pub fn open_path_in_app(path: &Path, bundle_id: &str) {
    use objc2_app_kit::{NSWorkspace, NSWorkspaceOpenConfiguration};
    use objc2_foundation::{NSArray, NSString, NSURL};

    let workspace = NSWorkspace::sharedWorkspace();
    let Some(application_url) =
        workspace.URLForApplicationWithBundleIdentifier(&NSString::from_str(bundle_id))
    else {
        return;
    };
    let url = if bundle_id == TERMY_BUNDLE_ID {
        let Some(url) = NSURL::URLWithString(&NSString::from_str(&termy_open_url(path))) else {
            return;
        };
        url
    } else {
        NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()))
    };
    workspace.openURLs_withApplicationAtURL_configuration_completionHandler(
        &NSArray::from_retained_slice(&[url]),
        &application_url,
        &NSWorkspaceOpenConfiguration::configuration(),
        None,
    );
}

#[cfg(target_os = "macos")]
fn termy_open_url(path: &Path) -> String {
    let mut url = url::Url::parse("termy://new").expect("static Termy URL should be valid");
    url.query_pairs_mut()
        .append_pair("dir", &path.to_string_lossy());
    url.into()
}

#[cfg(not(target_os = "macos"))]
pub fn open_path_in_app(_: &Path, _: &str) {}

/// Reveal `path` in the OS file manager, selecting it. Used by the skills
/// page to open a skill's directory.
#[cfg(target_os = "macos")]
pub fn reveal_in_file_manager(path: &Path) {
    let _ = std::process::Command::new("/usr/bin/open")
        .arg("-R")
        .arg(path)
        .spawn();
}

#[cfg(not(target_os = "macos"))]
pub fn reveal_in_file_manager(path: &Path) {
    // Most Linux desktops have no "select" verb; open the containing folder.
    let target = path.parent().unwrap_or(path);
    let _ = std::process::Command::new("xdg-open").arg(target).spawn();
}

/// Open `path` in the OS default application (the user's editor for a
/// `SKILL.md`). Used by the skills page's **Open SKILL.md** action.
#[cfg(target_os = "macos")]
pub fn open_path_default(path: &Path) {
    let _ = std::process::Command::new("/usr/bin/open")
        .arg(path)
        .spawn();
}

#[cfg(not(target_os = "macos"))]
pub fn open_path_default(path: &Path) {
    let _ = std::process::Command::new("xdg-open").arg(path).spawn();
}

fn open_in_prefs_path() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join(".orbit-pi").join("open-in.json")
}

/// Load the persisted preferred open-in app id, if any.
pub fn load_preferred_open_in_app() -> Option<String> {
    let raw = std::fs::read_to_string(open_in_prefs_path()).ok()?;
    let value: Value = serde_json::from_str(&raw).ok()?;
    value
        .get("open_in_app")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
        .map(str::to_owned)
}

/// Remember the user's preferred open-in app id.
pub fn persist_preferred_open_in_app(app_id: &str) {
    let path = open_in_prefs_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(
        path,
        serde_json::json!({ "open_in_app": app_id }).to_string(),
    );
}

/// Run an interactive shell command in the user's terminal. Used for
/// `pi /login <provider>`, whose OAuth/device flow is a terminal UI Orbit
/// cannot render. Best-effort: failures leave the caller to surface a hint.
#[cfg(target_os = "macos")]
pub fn open_terminal_command(command: &str) -> Result<(), String> {
    // A `.command` file opened with `open` runs in Terminal.app without
    // requiring Automation (AppleScript) permission, and works even when a
    // non-default terminal is installed.
    let path = std::env::temp_dir().join(format!("orbit-{}.command", std::process::id()));
    let script = format!("#!/bin/zsh\n{command}\n");
    std::fs::write(&path, script).map_err(|err| format!("could not write login script: {err}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700));
    }
    std::process::Command::new("/usr/bin/open")
        .arg(&path)
        .spawn()
        .map(|_| ())
        .map_err(|err| format!("could not open Terminal: {err}"))
}

#[cfg(not(target_os = "macos"))]
pub fn open_terminal_command(command: &str) -> Result<(), String> {
    for terminal in ["x-terminal-emulator", "gnome-terminal", "konsole", "xterm"] {
        let mut cmd = std::process::Command::new(terminal);
        match terminal {
            "gnome-terminal" => cmd.arg("--").args(["sh", "-c", command]),
            "konsole" => cmd.args(["-e", "sh", "-c", command]),
            _ => cmd.args(["-e", "sh", "-c", command]),
        };
        if cmd.spawn().is_ok() {
            return Ok(());
        }
    }
    Err("no terminal emulator found — run the command manually".into())
}

/// Open an `http(s)` URL in the user's default browser. Used for the OAuth
/// authorization leg of a browser login; the URL comes from pi's `auth.*`
/// events, so the scheme is validated before launching.
#[cfg(target_os = "macos")]
pub fn open_url(url: &str) -> Result<(), String> {
    if !is_safe_browser_url(url) {
        return Err("refusing to open a non-http(s) URL".into());
    }
    std::process::Command::new("/usr/bin/open")
        .arg(url)
        .spawn()
        .map(|_| ())
        .map_err(|err| format!("could not open the browser: {err}"))
}

#[cfg(not(target_os = "macos"))]
pub fn open_url(url: &str) -> Result<(), String> {
    if !is_safe_browser_url(url) {
        return Err("refusing to open a non-http(s) URL".into());
    }
    let opener = if cfg!(target_os = "windows") {
        "cmd"
    } else {
        "xdg-open"
    };
    let mut command = std::process::Command::new(opener);
    if cfg!(target_os = "windows") {
        command.args(["/C", "start", "", url]);
    } else {
        command.arg(url);
    }
    command
        .spawn()
        .map(|_| ())
        .map_err(|err| format!("could not open the browser: {err}"))
}

/// Only `http`/`https` URLs reach the OS opener, so a compromised or buggy
/// server can never hand the shell a `file:`/`javascript:` target.
fn is_safe_browser_url(url: &str) -> bool {
    let trimmed = url.trim();
    trimmed.starts_with("https://") || trimmed.starts_with("http://")
}

/// Open System Settings → Notifications so the user can unblock Orbit's
/// banners. macOS 13+ (the app's floor) uses the Notifications extension
/// URL; the legacy `com.apple.preference` pane is the older fallback.
#[cfg(target_os = "macos")]
pub fn open_notification_settings() -> Result<(), String> {
    const PANES: [&str; 2] = [
        "x-apple.systempreferences:com.apple.Notifications-Settings.extension",
        "x-apple.systempreferences:com.apple.preference.notifications",
    ];
    for pane in PANES {
        let opened = std::process::Command::new("/usr/bin/open")
            .arg(pane)
            .status()
            .map(|status| status.success())
            .unwrap_or(false);
        if opened {
            return Ok(());
        }
    }
    Err("could not open System Settings".into())
}

#[cfg(not(target_os = "macos"))]
pub fn open_notification_settings() -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::is_safe_browser_url;

    #[test]
    fn only_http_urls_are_opened() {
        assert!(is_safe_browser_url("https://claude.ai/oauth"));
        assert!(is_safe_browser_url("http://127.0.0.1:1455/callback"));
        assert!(!is_safe_browser_url("file:///etc/passwd"));
        assert!(!is_safe_browser_url("javascript:alert(1)"));
        assert!(!is_safe_browser_url("  "));
    }
}
