use crate::handler;
use log::{debug, error, info, warn};
use nfcd_core::config::{self, ActiveEntry, Config, IgnoreScope, PathMode, effective_entry};
use nfcd_core::constants::{CONFIG_PATH, HEARTBEAT_INTERVAL, HEARTBEAT_PATH, PLIST_PATH};
use nfcd_core::ignore::NFCIGNORE;
use nfcd_core::normalizer::NormalizationTarget;
use notify::event::{CreateKind, EventKind, ModifyKind};
use notify::{Error as NotifyError, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::spawn;
use tokio::sync::Semaphore;

/// Maximum number of concurrent file event handler tasks.
const MAX_CONCURRENT_TASKS: usize = 200;

/// Debounce window for deduplicating events on the same path.
const DEBOUNCE_DURATION: Duration = Duration::from_millis(50);

/// Whether the event path is a directory. Uses the event kind when FSEvents
/// reports it (create events); otherwise falls back to lstat.
fn is_dir_event(event: &Event, path: &Path) -> bool {
    match event.kind {
        EventKind::Create(CreateKind::Folder) => true,
        EventKind::Create(CreateKind::File) => false,
        _ => std::fs::symlink_metadata(path).is_ok_and(|m| m.is_dir()),
    }
}

/// What to do with an event on `path`: None drops it, Some(scan) handles it, where
/// `scan` means a directory in a recursive entry whose subtree is converted too.
fn route(event: &Event, path: &Path, entries: &[ActiveEntry]) -> Option<bool> {
    let file_name = path.file_name()?.to_str()?;
    let needs_conversion = NormalizationTarget::NFC.needs_conversion(file_name);
    // An NFC-named folder can still hold NFD names (moved in as a single event),
    // so creates and renames are kept until we know whether they are directories.
    let may_be_dir = match event.kind {
        EventKind::Create(CreateKind::File) => false,
        EventKind::Create(_) | EventKind::Modify(ModifyKind::Name(_)) => true,
        _ => false,
    };
    // Early filter: skip NFC files before any path matching
    // (effective_entry allocates an NFC copy of the path).
    if !needs_conversion && !may_be_dir {
        return None;
    }
    // Use the most specific entry to determine if this event should be processed.
    let entry = effective_entry(path, entries)?;
    let is_dir = is_dir_event(event, path);
    let scan = is_dir && entry.mode == PathMode::Recursive;
    if !needs_conversion && !scan {
        return None;
    }
    if entry.is_ignored(path, is_dir) {
        return None;
    }
    Some(scan)
}

/// Scan registered roots in the background (startup and dropped-event recovery).
fn spawn_scans(roots: &[(PathBuf, bool)], scope: &Arc<IgnoreScope>) {
    for (root, recursive) in roots {
        spawn(handler::scan_dir(root.clone(), *recursive, scope.clone()));
    }
}

/// Register watch paths with the notify watcher. Returns (recursive_count, children_count,
/// registered roots as (path, recursive)).
///
/// Entries nested under another recursive entry are not registered: the parent's
/// stream already delivers their events, and effective_entry() routes each event
/// to the most specific entry. Registering both would deliver every event twice.
fn register_watch_paths(
    watcher: &mut RecommendedWatcher,
    entries: &[ActiveEntry],
) -> (usize, usize, Vec<(PathBuf, bool)>) {
    let mut roots = Vec::new();
    let mut recursive_count = 0usize;
    let mut children_count = 0usize;

    for entry in entries {
        match entry.mode {
            PathMode::Recursive => recursive_count += 1,
            PathMode::Children => children_count += 1,
        }
        let covered = entries.iter().any(|p| {
            p.mode == PathMode::Recursive
                && p.canonical != entry.canonical
                && entry.canonical.starts_with(&p.canonical)
        });
        if covered {
            debug!(
                "Nested path handled by parent stream: {}",
                entry.canonical.display()
            );
            continue;
        }
        let notify_mode = match entry.mode {
            PathMode::Recursive => RecursiveMode::Recursive,
            PathMode::Children => RecursiveMode::NonRecursive,
        };
        match watcher.watch(&entry.canonical, notify_mode) {
            Ok(()) => {
                debug!("Watching path: {}", entry.canonical.display());
                roots.push((entry.canonical.clone(), entry.mode == PathMode::Recursive));
            }
            Err(e) => error!(
                "Failed to watch path: {} - {}",
                entry.canonical.display(),
                e
            ),
        }
    }

    (recursive_count, children_count, roots)
}

/// Log a summary of the registered watch paths. `event` is e.g. "started".
fn log_watch_summary(event: &str, recursive_count: usize, children_count: usize) {
    if recursive_count == 0 && children_count == 0 {
        info!(
            "nfcd-watcher v{} {}. No paths configured.",
            env!("CARGO_PKG_VERSION"),
            event
        );
    } else {
        info!(
            "nfcd-watcher v{} {}. Paths: {} recursive, {} children.",
            env!("CARGO_PKG_VERSION"),
            event,
            recursive_count,
            children_count
        );
    }
}

/// Watch state derived from one config load.
struct Active {
    entries: Vec<ActiveEntry>,
    scope: Arc<IgnoreScope>,
    roots: Vec<(PathBuf, bool)>,
}

/// Register `config`'s paths and scan them (catches names created while they were
/// not watched, and paths newly un-ignored by a config change).
fn activate(watcher: &mut RecommendedWatcher, config: &Config, event: &str) -> Active {
    let entries = config.active_entries();
    let (recursive_count, children_count, roots) = register_watch_paths(watcher, &entries);
    log_watch_summary(event, recursive_count, children_count);
    let scope = Arc::new(IgnoreScope::new(entries.clone()));
    spawn_scans(&roots, &scope);
    Active {
        entries,
        scope,
        roots,
    }
}

/// Re-read config.toml. Returns None (keep the current config) when the file is
/// unchanged, missing (mid-save by some editors), or invalid.
fn reload_config(last_content: &mut String) -> Option<Config> {
    let content = std::fs::read_to_string(&*CONFIG_PATH).ok()?;
    if content == *last_content {
        return None;
    }
    let (config, err) = config::load_config();
    if let Some(e) = err {
        warn!("Config not reloaded, keeping the current one: {}", e);
        return None;
    }
    *last_content = content;
    Some(config)
}

/// Spawn a background task that writes to the heartbeat file at a regular interval.
fn spawn_heartbeat_task() {
    let heartbeat_dir = HEARTBEAT_PATH.parent().map(Path::to_path_buf);
    spawn(async move {
        let mut interval = tokio::time::interval(HEARTBEAT_INTERVAL);
        loop {
            interval.tick().await;
            if let Err(e) = std::fs::write(&*HEARTBEAT_PATH, "")
                && e.kind() == std::io::ErrorKind::NotFound
                && let Some(dir) = &heartbeat_dir
            {
                let _ = std::fs::create_dir_all(dir);
            }
        }
    });
}

/// Spawn a background task that checks if the watcher binary still exists on disk.
/// If removed (e.g. uninstallation), cleans up the plist and heartbeat file, then exits.
fn spawn_binary_watchdog() {
    let exe_path = match std::env::current_exe().ok() {
        Some(p) => p,
        None => return,
    };
    spawn(async move {
        let mut interval = tokio::time::interval(HEARTBEAT_INTERVAL);
        loop {
            interval.tick().await;
            if !exe_path.exists() {
                info!("Binary removed. Cleaning up plist and exiting.");
                let _ = std::fs::remove_file(&*PLIST_PATH);
                let _ = std::fs::remove_file(&*HEARTBEAT_PATH);
                std::process::exit(0);
            }
        }
    });
}

pub async fn start_watcher(rt_handle: tokio::runtime::Handle, config: Config) {
    let (tx, mut rx) = tokio::sync::mpsc::channel(1024);

    let mut watcher = match RecommendedWatcher::new(
        move |res: Result<Event, NotifyError>| {
            let tx = tx.clone();
            let rt_handle = rt_handle.clone();
            rt_handle.spawn(async move {
                if let Err(e) = tx.send(res).await {
                    error!("Failed to send file event: {}", e);
                }
            });
        },
        notify::Config::default(),
    ) {
        Ok(w) => {
            debug!("File system event watcher initialized.");
            w
        }
        Err(e) => {
            error!(
                "Failed to initialize file system event watcher: {}. Exiting normally (exit code 0).",
                e
            );
            std::process::exit(0);
        }
    };

    let mut active = activate(&mut watcher, &config, "started");

    // Watch the config directory (not the file: editors often save by renaming
    // a temp file over it) and reload on change.
    let mut last_content = std::fs::read_to_string(&*CONFIG_PATH).unwrap_or_default();
    let config_dir = CONFIG_PATH.parent().map(Path::to_path_buf);
    if let Some(dir) = &config_dir {
        let _ = std::fs::create_dir_all(dir);
        if let Err(e) = watcher.watch(dir, RecursiveMode::NonRecursive) {
            error!(
                "Failed to watch config directory: {} - {}",
                dir.display(),
                e
            );
        }
    }
    // FSEvents reports canonical paths; HOME may go through a symlink.
    let config_file = config_dir
        .and_then(|d| std::fs::canonicalize(d).ok())
        .map(|d| d.join(CONFIG_PATH.file_name().unwrap_or_default()));
    let mut config_changed = false;
    let mut nfcignore_changed = false;
    spawn_heartbeat_task();
    spawn_binary_watchdog();

    // Limit the number of concurrently executing tasks using a semaphore.
    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_TASKS));

    // Process events with debouncing to deduplicate rapid events on the same path.
    // Path -> whether to scan its subtree.
    let mut pending: HashMap<PathBuf, bool> = HashMap::new();
    let mut rescan = false;
    let mut debounce_timer = tokio::time::interval(DEBOUNCE_DURATION);

    loop {
        tokio::select! {
            Some(res) = rx.recv() => {
                match res {
                    // FSEvents dropped events (MustScanSubDirs / KernelDropped / UserDropped).
                    // ponytail: rescans every root, not just the reported path. Drops are rare;
                    // narrow it if large trees make this costly.
                    Ok(event) if event.need_rescan() => rescan = true,
                    Ok(event) => {
                        let Some(path) = event.paths.first() else {
                            continue;
                        };
                        if config_file.as_ref() == Some(path) {
                            config_changed = true;
                            continue;
                        }
                        if path.file_name().is_some_and(|n| n == NFCIGNORE) {
                            nfcignore_changed = true;
                            continue;
                        }
                        let Some(scan) = route(&event, path, &active.entries) else {
                            continue;
                        };
                        // Same path within the window is handled once.
                        *pending.entry(path.clone()).or_default() |= scan;
                    }
                    Err(e) => error!("FS watcher error: {}", e),
                }
            }
            _ = debounce_timer.tick() => {
                if std::mem::take(&mut config_changed)
                    && let Some(config) = reload_config(&mut last_content)
                {
                    for (root, _) in &active.roots {
                        let _ = watcher.unwatch(root);
                    }
                    // ponytail: rescans every root after a reload, not just what changed.
                    // Config edits are rare; diff old/new entries if this gets costly.
                    active = activate(&mut watcher, &config, "reloaded config");
                    pending.clear();
                } else if std::mem::take(&mut rescan) {
                    warn!("File system events were dropped. Rescanning watch paths.");
                    spawn_scans(&active.roots, &active.scope);
                } else if std::mem::take(&mut nfcignore_changed) {
                    // Removed patterns un-ignore files that were never converted.
                    // ponytail: rescans every root, not just the file's directory.
                    // .nfcignore edits are rare; scan only that subtree if this gets costly.
                    info!("{} changed. Rescanning watch paths.", NFCIGNORE);
                    spawn_scans(&active.roots, &active.scope);
                }
                // ponytail: renaming a folder emits another event for its new name, so its
                // subtree is scanned twice (the second pass only reads). Track recent scans
                // if this shows up as load.
                for (path, scan) in pending.drain() {
                    let sem_clone = semaphore.clone();
                    let scope = scan.then(|| active.scope.clone());
                    spawn(async move {
                        let _permit = sem_clone.acquire_owned().await.unwrap();
                        handler::handle_event(path, scope).await;
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nfcd_core::config::{Config, PathEntry};
    use notify::event::RenameMode;

    fn entries(root: &Path, mode: PathMode, ignore: &[&str]) -> Vec<ActiveEntry> {
        let mut entry = PathEntry::new(root.to_path_buf(), mode);
        entry.ignore = ignore.iter().map(|s| s.to_string()).collect();
        let mut config = Config {
            paths: vec![entry],
            global_ignore: vec![],
        };
        config.refresh_statuses();
        config.active_entries()
    }

    fn rename(path: &Path) -> Event {
        Event::new(EventKind::Modify(ModifyKind::Name(RenameMode::Any))).add_path(path.into())
    }

    #[test]
    fn route_scans_nfc_folder_in_recursive_entry() {
        let temp = tempfile::TempDir::new().unwrap();
        let root = std::fs::canonicalize(temp.path()).unwrap();
        let dir = root.join("moved");
        let skip = root.join("skip");
        std::fs::create_dir(&dir).unwrap();
        std::fs::create_dir(&skip).unwrap();
        std::fs::write(root.join("a.txt"), "").unwrap();
        let rec = entries(&root, PathMode::Recursive, &["/skip/"]);

        // NFC folder moved in: handled with a subtree scan.
        assert_eq!(route(&rename(&dir), &dir, &rec), Some(true));
        // NFC file rename and ignored folder: dropped.
        let file = root.join("a.txt");
        assert_eq!(route(&rename(&file), &file, &rec), None);
        assert_eq!(route(&rename(&skip), &skip, &rec), None);
        // NFD file: converted, no scan.
        let nfd = root.join("\u{1100}\u{1161}.txt");
        let create = Event::new(EventKind::Create(CreateKind::File)).add_path(nfd.clone());
        assert_eq!(route(&create, &nfd, &rec), Some(false));

        // Children mode never scans subtrees.
        let children = entries(&root, PathMode::Children, &[]);
        assert_eq!(route(&rename(&dir), &dir, &children), None);
    }
}
