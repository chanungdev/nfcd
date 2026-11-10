use log::{error, info, warn};
use nfcd_core::config::IgnoreScope;
use nfcd_core::normalizer::{NormalizationTarget, get_actual_file_name, normalize_directory};
use nfcd_core::utils::abbreviate_home_path;
use std::path::{Path, PathBuf};
use std::sync::Arc;

async fn get_actual_name(path: &Path) -> Option<String> {
    let p = path.to_path_buf();
    match tokio::task::spawn_blocking(move || get_actual_file_name(&p)).await {
        Ok(Ok(name)) => Some(name),
        Ok(Err(ref e)) => {
            if !e.is_not_found() {
                error!(
                    "Failed to get file name: {} — {}",
                    abbreviate_home_path(path),
                    e
                );
            }
            None
        }
        Err(e) => {
            error!("Task join error: {}", e);
            None
        }
    }
}

/// Convert the name of `path` to NFC if needed. Returns the path it now lives
/// at, or None if it is gone or the rename failed.
async fn convert_name(path: &Path) -> Option<PathBuf> {
    let actual_name = get_actual_name(path).await?;

    let target = NormalizationTarget::NFC;
    if !target.needs_conversion(&actual_name) {
        return Some(path.to_path_buf());
    }

    let nfc_file_name = target.convert(&actual_name);
    let new_path = path.with_file_name(&nfc_file_name);

    match tokio::fs::rename(path, &new_path).await {
        Ok(()) => {
            info!("NFD→NFC: {}", abbreviate_home_path(&new_path));
            Some(new_path)
        }
        Err(e) => {
            if e.kind() != std::io::ErrorKind::NotFound {
                error!(
                    "Failed to convert {} to NFC: {}",
                    abbreviate_home_path(&new_path),
                    e
                );
            }
            None
        }
    }
}

/// Handle one debounced event path. With `scan`, the path is a directory in a
/// recursive entry: after its own name, everything below it is converted too
/// (a folder moved in arrives as a single event).
pub async fn handle_event(path: PathBuf, scan: Option<Arc<IgnoreScope>>) {
    let Some(path) = convert_name(&path).await else {
        return;
    };
    if let Some(scope) = scan {
        scan_dir(path, true, scope).await;
    }
}

/// Convert every name under `dir`, skipping ignored paths.
pub async fn scan_dir(dir: PathBuf, recursive: bool, scope: Arc<IgnoreScope>) {
    let d = dir.clone();
    let result = tokio::task::spawn_blocking(move || {
        normalize_directory(&d, recursive, NormalizationTarget::NFC, Some(&scope))
    })
    .await;
    let result = match result {
        Ok(Ok(r)) => r,
        Ok(Err(e)) => {
            error!("Scan failed: {} — {}", abbreviate_home_path(&dir), e);
            return;
        }
        Err(e) => {
            error!("Task join error: {}", e);
            return;
        }
    };
    for c in &result.converted {
        info!("NFD→NFC: {}", abbreviate_home_path(&c.to));
    }
    // One line per scan: a startup scan of a large tree can hit many unreadable dirs.
    if let Some(first) = result.errors.first() {
        warn!(
            "Scan of {}: {} error(s), first: {} — {}",
            abbreviate_home_path(&dir),
            result.errors.len(),
            abbreviate_home_path(&first.path),
            first.error
        );
    }
}
