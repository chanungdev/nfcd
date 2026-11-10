use crate::handler;
use log::{debug, error, info};
use nfcd_core::config::{ActiveEntry, PathMode, effective_entry};
use nfcd_core::constants::{HEARTBEAT_INTERVAL, HEARTBEAT_PATH, PLIST_PATH};
use nfcd_core::normalizer::NormalizationTarget;
use notify::event::{CreateKind, EventKind};
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

/// Register watch paths with the notify watcher. Returns (recursive_count, children_count).
///
/// Entries nested under another recursive entry are not registered: the parent's
/// stream already delivers their events, and effective_entry() routes each event
/// to the most specific entry. Registering both would deliver every event twice.
fn register_watch_paths(
    watcher: &mut RecommendedWatcher,
    entries: &[ActiveEntry],
) -> (usize, usize) {
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
            Ok(()) => debug!("Watching path: {}", entry.canonical.display()),
            Err(e) => error!(
                "Failed to watch path: {} - {}",
                entry.canonical.display(),
                e
            ),
        }
    }

    (recursive_count, children_count)
}

/// Log a summary of the registered watch paths.
fn log_watch_summary(recursive_count: usize, children_count: usize) {
    if recursive_count == 0 && children_count == 0 {
        info!(
            "nfcd-watcher v{} started. No paths configured.",
            env!("CARGO_PKG_VERSION")
        );
    } else {
        info!(
            "nfcd-watcher v{} started. Paths: {} recursive, {} children.",
            env!("CARGO_PKG_VERSION"),
            recursive_count,
            children_count
        );
    }
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

pub async fn start_watcher(rt_handle: tokio::runtime::Handle, entries: Vec<ActiveEntry>) {
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

    let (recursive_count, children_count) = register_watch_paths(&mut watcher, &entries);
    log_watch_summary(recursive_count, children_count);
    spawn_heartbeat_task();
    spawn_binary_watchdog();

    // Limit the number of concurrently executing tasks using a semaphore.
    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_TASKS));

    // Process events with debouncing to deduplicate rapid events on the same path.
    let mut pending: HashMap<PathBuf, Event> = HashMap::new();
    let mut debounce_timer = tokio::time::interval(DEBOUNCE_DURATION);

    loop {
        tokio::select! {
            Some(res) = rx.recv() => {
                match res {
                    Ok(event) => {
                        let event_path = match event.paths.first() {
                            Some(path) => path,
                            None => continue,
                        };

                        let file_name = match event_path.file_name().and_then(|s| s.to_str()) {
                            Some(name) => name,
                            None => continue,
                        };
                        // Early filter: skip files already in NFC before any path matching
                        // (effective_entry allocates an NFC copy of the path).
                        if !NormalizationTarget::NFC.needs_conversion(file_name) {
                            continue;
                        }
                        // Use the most specific entry to determine if this event should be processed.
                        let Some(entry) = effective_entry(event_path, &entries) else {
                            continue;
                        };
                        // Pattern check only for names that need conversion (may lstat).
                        if entry.ignore.is_ignored(event_path, is_dir_event(&event, event_path)) {
                            continue;
                        }

                        // Same path overwrites previous event (deduplication).
                        pending.insert(event_path.clone(), event);
                    }
                    Err(e) => error!("FS watcher error: {}", e),
                }
            }
            _ = debounce_timer.tick() => {
                for (_path, event) in pending.drain() {
                    let sem_clone = semaphore.clone();
                    spawn(async move {
                        let _permit = sem_clone.acquire_owned().await.unwrap();
                        handler::handle_event(event).await;
                    });
                }
            }
        }
    }
}
