mod commands;

use nfcd_core::constants::{CONFIG_PATH, HEARTBEAT_MAX_AGE, HEARTBEAT_PATH, HOME_DIR};
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;
use tauri::image::Image;
use tauri::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::tray::{TrayIcon, TrayIconBuilder};
use tauri::{AppHandle, Emitter, Manager, RunEvent, WebviewUrl, WebviewWindowBuilder, WindowEvent};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_window_state::{AppHandleExt, StateFlags};

const ICON_RUNNING: &[u8] = include_bytes!("../icons/tray/running.png");
const ICON_STOPPED: &[u8] = include_bytes!("../icons/tray/stopped.png");
const POLL_INTERVAL: Duration = Duration::from_secs(2);
const WINDOW_STATE: StateFlags = StateFlags::SIZE
    .union(StateFlags::POSITION)
    .union(StateFlags::MAXIMIZED);

/// Same check as `nfcd status`: the watcher touches the heartbeat file periodically.
fn watcher_running() -> bool {
    std::fs::metadata(&*HEARTBEAT_PATH)
        .and_then(|m| m.modified())
        .map(|t| t.elapsed().is_ok_and(|e| e < HEARTBEAT_MAX_AGE))
        .unwrap_or(false)
}

/// Apps launched from Finder don't get the shell PATH, so look in the usual install dirs.
pub(crate) fn nfcd_cli() -> Option<PathBuf> {
    [
        PathBuf::from("/opt/homebrew/bin/nfcd"),
        PathBuf::from("/usr/local/bin/nfcd"),
        HOME_DIR.join(".cargo/bin/nfcd"),
    ]
    .into_iter()
    .find(|p| p.exists())
}

/// Run `nfcd watcher <action>`. The CLI owns plist and launchctl handling.
fn watcher_command(action: &str) {
    let Some(cli) = nfcd_cli() else {
        eprintln!("nfcd CLI not found; install it with `brew install chanungdev/tap/nfcd`");
        return;
    };
    match Command::new(&cli).args(["watcher", action]).output() {
        Ok(out) if !out.status.success() => {
            eprintln!(
                "nfcd watcher {action} failed: {}",
                String::from_utf8_lossy(&out.stderr)
            )
        }
        Err(e) => eprintln!("Failed to run {}: {e}", cli.display()),
        Ok(_) => {}
    }
}

struct Tray {
    icon: TrayIcon,
    status: MenuItem<tauri::Wry>,
    toggle: MenuItem<tauri::Wry>,
}

impl Tray {
    fn show(&self, running: bool) {
        let (icon, status, toggle) = if running {
            (ICON_RUNNING, "Watcher: running", "Stop watcher")
        } else {
            (ICON_STOPPED, "Watcher: stopped", "Start watcher")
        };
        let _ = self.icon.set_icon(Image::from_bytes(icon).ok());
        let _ = self.icon.set_icon_as_template(true);
        let _ = self.status.set_text(status);
        let _ = self.toggle.set_text(toggle);
    }
}

fn open_settings(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("settings") {
        // Reopening a hidden window: let the page reload config.toml, which may have changed.
        let _ = w.emit("settings-shown", ());
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    // Built hidden; the page shows it after its first render (`main.tsx`) so the
    // user never sees an empty black window while WebKit starts up.
    // The window-state plugin restores the last size/position when the window is built.
    let Ok(w) = WebviewWindowBuilder::new(app, "settings", WebviewUrl::default())
        .title("nfcd Settings")
        .inner_size(720.0, 520.0)
        .visible(false)
        .build()
    else {
        return;
    };
    let fallback = w.clone();
    std::thread::spawn(move || {
        // In case the page never calls show() (e.g. a JS error), don't leave it invisible.
        std::thread::sleep(Duration::from_secs(3));
        if fallback.is_visible().is_ok_and(|v| !v) {
            let _ = fallback.show();
        }
    });
    let handle = app.clone();
    let win = w.clone();
    w.on_window_event(move |e| {
        if let WindowEvent::CloseRequested { api, .. } = e {
            // Hide instead of destroying it, so reopening skips the WebKit startup.
            api.prevent_close();
            let _ = win.hide();
            // The plugin writes to disk only on app exit; save now so a killed app keeps it.
            let _ = handle.save_window_state(WINDOW_STATE);
        }
    });
}

fn on_menu(app: &AppHandle, event: MenuEvent) {
    match event.id().as_ref() {
        "toggle" => {
            let action = if watcher_running() { "stop" } else { "start" };
            // launchctl can take a moment; keep the menu responsive.
            std::thread::spawn(move || watcher_command(action));
        }
        "config" => {
            let _ = Command::new("open").arg("-t").arg(&*CONFIG_PATH).status();
        }
        "settings" => open_settings(app),
        "quit" => app.exit(0),
        _ => {}
    }
}

/// Start at login by default: enable it on the first launch only, so turning it
/// off in Settings sticks. Debug builds skip this to keep dev binaries out of
/// ~/Library/LaunchAgents.
fn enable_autostart_once(app: &tauri::App) {
    if cfg!(debug_assertions) {
        return;
    }
    let Ok(dir) = app.path().app_data_dir() else {
        return;
    };
    let marker = dir.join("autostart-initialized");
    if marker.exists() {
        return;
    }
    if app.autolaunch().enable().is_ok() {
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(marker, "");
    }
}

fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "macos")]
    app.set_activation_policy(tauri::ActivationPolicy::Accessory);

    enable_autostart_once(app);

    let status = MenuItem::with_id(app, "status", "Watcher: …", false, None::<&str>)?;
    let toggle = MenuItem::with_id(app, "toggle", "Start watcher", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &status,
            &toggle,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, "settings", "Settings…", true, Some("CmdOrCtrl+,"))?,
            &MenuItem::with_id(app, "config", "Open config file", true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, "quit", "Quit nfcd", true, Some("CmdOrCtrl+Q"))?,
        ],
    )?;
    let icon = TrayIconBuilder::with_id("main")
        .menu(&menu)
        .tooltip("nfcd")
        .on_menu_event(on_menu)
        .build(app)?;

    let tray = Tray {
        icon,
        status,
        toggle,
    };
    let mut last = watcher_running();
    tray.show(last);
    // ponytail: polls the heartbeat file; switch to a file watcher if 2s lag matters.
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(POLL_INTERVAL);
            let now = watcher_running();
            if now != last {
                tray.show(now);
                last = now;
            }
        }
    });
    Ok(())
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            // Named after the bundle id; the default ("nfcd") would make a vague
            // ~/Library/LaunchAgents/nfcd.plist next to the watcher's plist.
            tauri_plugin_autostart::Builder::new()
                .app_name("io.github.chanungdev.nfcd.gui")
                .macos_launcher(tauri_plugin_autostart::MacosLauncher::LaunchAgent)
                .build(),
        )
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(WINDOW_STATE)
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            commands::load,
            commands::refresh,
            commands::save,
            commands::check_ignore,
            commands::status,
            commands::open_fda_settings,
        ])
        .setup(setup)
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app, event| {
            // Closing the settings window must not quit the tray app; only Quit (exit code set) does.
            if let RunEvent::ExitRequested {
                code: None, api, ..
            } = event
            {
                api.prevent_exit();
            }
        });
}
