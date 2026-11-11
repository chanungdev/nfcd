//! Commands called from the settings window.

use crate::nfcd_cli;
use nfcd_core::config::{Config, PathEntry, PathMode, PathStatus, load_config};
use nfcd_core::constants::CONFIG_PATH;
use nfcd_core::utils::expand_tilde;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::process::Command;

/// What the settings window edits: everything that is saved to config.toml.
#[derive(Deserialize)]
pub struct Draft {
    global_ignore: Vec<String>,
    paths: Vec<DraftPath>,
}

#[derive(Deserialize)]
struct DraftPath {
    raw: String,
    mode: PathMode,
    ignore: Vec<String>,
}

#[derive(Serialize)]
pub struct ConfigView {
    global_ignore: Vec<String>,
    paths: Vec<PathView>,
}

#[derive(Serialize)]
struct PathView {
    raw: String,
    mode: PathMode,
    ignore: Vec<String>,
    status: &'static str,
    /// 0-based index of the path this one is redundant under.
    redundant_of: Option<usize>,
}

impl From<Config> for ConfigView {
    fn from(c: Config) -> Self {
        Self {
            global_ignore: c.global_ignore,
            paths: c
                .paths
                .into_iter()
                .map(|p| PathView {
                    status: p.status.as_str(),
                    redundant_of: match p.status {
                        PathStatus::Redundant(i) => Some(i),
                        _ => None,
                    },
                    raw: p.raw,
                    mode: p.mode,
                    ignore: p.ignore,
                })
                .collect(),
        }
    }
}

fn to_config(draft: Draft) -> Config {
    let mut config = Config {
        global_ignore: draft.global_ignore,
        paths: draft
            .paths
            .into_iter()
            .map(|p| {
                let mut entry = PathEntry::new(p.raw.into(), p.mode);
                entry.ignore = p.ignore;
                entry
            })
            .collect(),
    };
    config.refresh_statuses();
    config
}

/// Fails on a broken config.toml so the window never overwrites it with an empty config.
#[tauri::command]
pub fn load() -> Result<ConfigView, String> {
    match load_config() {
        (_, Some(e)) => Err(format!("{}: {e}", e.user_message())),
        (config, None) => Ok(config.into()),
    }
}

/// Recompute path statuses for an unsaved draft.
#[tauri::command]
pub fn refresh(draft: Draft) -> ConfigView {
    to_config(draft).into()
}

/// Write config.toml. The watcher picks up the change by itself.
#[tauri::command]
pub fn save(draft: Draft) -> Result<ConfigView, String> {
    let config = to_config(draft);
    config
        .save_to_file(&CONFIG_PATH)
        .map_err(|e| format!("{}: {e}", e.user_message()))?;
    Ok(config.into())
}

/// Run `nfcd <args> --json`. The CLI already has this logic; don't duplicate it here.
fn cli_json(args: &[&str]) -> Result<Value, String> {
    let cli = nfcd_cli()
        .ok_or("nfcd CLI not found. Install it with `brew install chanungdev/tap/nfcd`.")?;
    let out = Command::new(&cli)
        .args(args)
        .arg("--json")
        .output()
        .map_err(|e| format!("Failed to run {}: {e}", cli.display()))?;
    // check-ignore exits 1 when nothing is ignored, so judge by the output, not the status.
    serde_json::from_slice(&out.stdout)
        .map_err(|_| String::from_utf8_lossy(&out.stderr).trim().to_string())
}

#[tauri::command]
pub async fn check_ignore(path: String) -> Result<Value, String> {
    // No shell in between, so expand `~` here.
    let path = expand_tilde(&path);
    let mut v = cli_json(&["check-ignore", &path.to_string_lossy()])?;
    Ok(v.get_mut(0).map(Value::take).unwrap_or(Value::Null))
}

#[tauri::command]
pub async fn status() -> Result<Value, String> {
    cli_json(&["status"])
}

#[tauri::command]
pub fn open_fda_settings() {
    let _ = Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles")
        .status();
}
