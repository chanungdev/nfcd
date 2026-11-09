use nfcd_core::constants::{HEARTBEAT_MAX_AGE, HEARTBEAT_PATH, PLIST_PATH, SERVICE_LABEL};
use std::process::Command;
use std::sync::LazyLock;
use std::time::Duration;

static GUI_DOMAIN: LazyLock<String> = LazyLock::new(|| {
    // SAFETY: getuid() is always safe and never fails.
    let uid = unsafe { libc::getuid() };
    format!("gui/{uid}")
});

/// Stop and remove a service from launchd by label (ignores errors).
fn bootout_service(label: &str) {
    let target = format!("{}/{label}", *GUI_DOMAIN);
    let _ = Command::new("launchctl")
        .args(["bootout", &target])
        .output();
}

/// Load a service plist into launchd.
fn bootstrap_service(action_desc: &str) -> Result<(), String> {
    let output = Command::new("launchctl")
        .args(["bootstrap", &*GUI_DOMAIN])
        .arg(&*PLIST_PATH)
        .output()
        .map_err(|e| format!("Failed to {action_desc}: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(format!("Failed to {action_desc}: {stderr}"));
    }
    Ok(())
}

/// Enable or disable the service for auto-start on login (ignores errors).
fn set_service_enabled(enabled: bool) {
    let subcmd = if enabled { "enable" } else { "disable" };
    let target = format!("{}/{SERVICE_LABEL}", *GUI_DOMAIN);
    let _ = Command::new("launchctl").args([subcmd, &target]).output();
}

/// Clean stale registration, enable, and bootstrap the service.
fn load_service(action_desc: &str) -> Result<(), String> {
    bootout_service(SERVICE_LABEL);
    set_service_enabled(true);
    bootstrap_service(action_desc)
}

fn launch_watcher_and_confirm() -> Result<(), String> {
    load_service("start watcher")?;

    // Wait for heartbeat file to be created/updated (max 5 seconds).
    for _ in 0..250 {
        std::thread::sleep(Duration::from_millis(20));
        if check_heartbeat_fresh(HEARTBEAT_MAX_AGE) {
            return Ok(());
        }
    }

    Err("Watcher started but heartbeat not detected".to_string())
}

/// Stop the service and disable auto-start.
fn unload_watcher_service() -> Result<(), String> {
    bootout_service(SERVICE_LABEL);
    set_service_enabled(false);
    Ok(())
}

/// Check if heartbeat file exists and was modified within max_age.
fn check_heartbeat_fresh(max_age: Duration) -> bool {
    std::fs::metadata(&*HEARTBEAT_PATH)
        .and_then(|m| m.modified())
        .map(|t| t.elapsed().is_ok_and(|e| e < max_age))
        .unwrap_or(false)
}

/// Check if plist file exists.
fn is_plist_installed() -> bool {
    PLIST_PATH.exists()
}

/// Check if the current binary is newer than the installed plist.
fn is_plist_stale() -> bool {
    let exe_mtime = std::env::current_exe()
        .and_then(std::fs::metadata)
        .and_then(|m| m.modified())
        .ok();
    let plist_mtime = std::fs::metadata(&*PLIST_PATH)
        .and_then(|m| m.modified())
        .ok();

    match (exe_mtime, plist_mtime) {
        (Some(exe), Some(plist)) => exe > plist,
        _ => false,
    }
}

/// Find the watcher binary path as a sibling of the current executable.
pub(crate) fn watcher_binary_path() -> Result<std::path::PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| format!("Failed to get current exe: {e}"))?;
    let dir = exe
        .parent()
        .ok_or_else(|| "Failed to get exe parent directory".to_string())?;
    let watcher = dir.join("nfcd-watcher");
    if !watcher.exists() {
        return Err(format!("Watcher binary not found: {}", watcher.display()));
    }
    Ok(watcher)
}

/// Generate the launchd plist file for the watcher service.
fn generate_plist() -> Result<(), String> {
    let watcher_path = watcher_binary_path()?;
    let plist_content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{label}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{watcher}</string>
    </array>
    <key>KeepAlive</key>
    <dict>
        <key>Crashed</key>
        <true/>
    </dict>
    <key>RunAtLoad</key>
    <true/>
</dict>
</plist>
"#,
        label = SERVICE_LABEL,
        watcher = watcher_path.display(),
    );

    // Ensure LaunchAgents directory exists.
    if let Some(parent) = PLIST_PATH.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create LaunchAgents directory: {e}"))?;
    }

    std::fs::write(&*PLIST_PATH, plist_content)
        .map_err(|e| format!("Failed to write plist: {e}"))?;

    Ok(())
}

/// Ensure the plist is installed and up to date.
/// - Plist exists but stale (binary newer): regenerate and reload
/// - Plist missing: generate and load (first install)
pub fn ensure_plist_up_to_date() -> Result<(), String> {
    if is_plist_installed() {
        if is_plist_stale() {
            let _ = unload_watcher_service();
            generate_plist()?;
            load_service("reload updated service")?;
        }
        return Ok(());
    }

    // Plist missing — first install, generate and start.
    generate_plist()?;
    load_service("start service")?;

    Ok(())
}

/// Check if the watcher is running by verifying heartbeat file mtime is recent.
pub fn check_watcher_status() -> bool {
    check_heartbeat_fresh(HEARTBEAT_MAX_AGE)
}

// TUI-specific functions that return Results instead of exiting

pub fn try_start_watcher() -> Result<(), String> {
    if check_watcher_status() {
        return Err("Watcher service is already running".to_string());
    }

    if !is_plist_installed() {
        generate_plist()?;
    }

    launch_watcher_and_confirm()
}

fn stop_watcher_internal() -> Result<(), String> {
    unload_watcher_service()?;
    let _ = std::fs::remove_file(&*HEARTBEAT_PATH);
    Ok(())
}

pub fn try_stop_watcher() -> Result<(), String> {
    if !check_watcher_status() {
        return Err("Watcher service is not running".to_string());
    }

    stop_watcher_internal()
}

pub fn try_restart_watcher() -> Result<(), String> {
    if !check_watcher_status() {
        return Err("Watcher service is not running".to_string());
    }

    stop_watcher_internal()?;
    launch_watcher_and_confirm()
}
