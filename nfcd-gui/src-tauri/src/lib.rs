use nfcd_core::constants::{CONFIG_PATH, HEARTBEAT_MAX_AGE, HEARTBEAT_PATH, HOME_DIR};
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;
use tauri::image::Image;
use tauri::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::tray::{TrayIcon, TrayIconBuilder};
use tauri::{AppHandle, Manager, RunEvent, WebviewUrl, WebviewWindowBuilder};

const ICON_RUNNING: &[u8] = include_bytes!("../icons/tray/running.png");
const ICON_STOPPED: &[u8] = include_bytes!("../icons/tray/stopped.png");
const POLL_INTERVAL: Duration = Duration::from_secs(2);

/// Same check as `nfcd status`: the watcher touches the heartbeat file periodically.
fn watcher_running() -> bool {
    std::fs::metadata(&*HEARTBEAT_PATH)
        .and_then(|m| m.modified())
        .map(|t| t.elapsed().is_ok_and(|e| e < HEARTBEAT_MAX_AGE))
        .unwrap_or(false)
}

/// Apps launched from Finder don't get the shell PATH, so look in the usual install dirs.
fn nfcd_cli() -> Option<PathBuf> {
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
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    let _ = WebviewWindowBuilder::new(app, "settings", WebviewUrl::default())
        .title("nfcd Settings")
        .inner_size(720.0, 520.0)
        .build()
        .map(|w| w.set_focus());
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

fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "macos")]
    app.set_activation_policy(tauri::ActivationPolicy::Accessory);

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
