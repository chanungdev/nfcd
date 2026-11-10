use crate::config::{IgnoreScope, NfcIgnoreChain};
use rayon::prelude::*;
use std::collections::VecDeque;
use std::ffi::CStr;
use std::fs::{self, File};
use std::os::fd::AsRawFd;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use thiserror::Error;
use unicode_normalization::{UnicodeNormalization, is_nfc, is_nfd};

/// Target normalization form for filename conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::upper_case_acronyms)]
pub enum NormalizationTarget {
    NFC,
    NFD,
}

impl std::fmt::Display for NormalizationTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NormalizationTarget::NFC => write!(f, "NFC"),
            NormalizationTarget::NFD => write!(f, "NFD"),
        }
    }
}

impl NormalizationTarget {
    /// Convert a filename to this normalization form.
    pub fn convert(&self, name: &str) -> String {
        match self {
            NormalizationTarget::NFC => name.nfc().collect(),
            NormalizationTarget::NFD => name.nfd().collect(),
        }
    }

    /// Check if a name needs conversion to this normalization form.
    pub fn needs_conversion(&self, name: &str) -> bool {
        match self {
            NormalizationTarget::NFC => !is_nfc(name),
            NormalizationTarget::NFD => !is_nfd(name),
        }
    }
}

/// Errors that can occur during normalization operations.
#[derive(Debug, Error)]
pub enum NormalizerError {
    #[error("Invalid file/folder name: {0}")]
    InvalidName(String),

    #[error("Failed to rename '{from}' to '{to}': {source}")]
    RenameError {
        from: String,
        to: String,
        source: std::io::Error,
    },

    #[error("Failed to read directory '{0}': {1}")]
    ReadDirError(String, std::io::Error),

    #[error("Failed to open file '{0}': {1}")]
    OpenError(String, std::io::Error),

    #[error("Failed to get actual path: fcntl F_GETPATH failed")]
    FcntlError,

    #[error("Failed to convert path to UTF-8")]
    Utf8Error,
}

impl NormalizerError {
    pub fn is_not_found(&self) -> bool {
        matches!(self, NormalizerError::OpenError(_, e) if e.kind() == std::io::ErrorKind::NotFound)
    }
}

/// Result of a single file/folder name conversion.
#[derive(Debug)]
pub struct ConversionResult {
    pub from: PathBuf,
    pub to: PathBuf,
}

/// Error that occurred during conversion of a single entry (non-fatal in batch).
#[derive(Debug)]
pub struct ConversionError {
    pub path: PathBuf,
    pub error: String,
}

/// Result of a directory conversion operation.
#[derive(Debug)]
pub struct DirectoryResult {
    pub converted: Vec<ConversionResult>,
    pub errors: Vec<ConversionError>,
}

/// Get the actual file name as stored on disk using macOS fcntl F_GETPATH.
/// This is O(1) compared to O(n) directory scanning.
///
/// This is necessary because filesystem events may report paths in a different
/// Unicode normalization form than what's actually stored on disk.
pub fn get_actual_file_name(path: &Path) -> Result<String, NormalizerError> {
    let file =
        File::open(path).map_err(|e| NormalizerError::OpenError(path.display().to_string(), e))?;

    let mut buf = [0u8; libc::PATH_MAX as usize];
    let ret = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GETPATH, buf.as_mut_ptr()) };

    if ret == -1 {
        return Err(NormalizerError::FcntlError);
    }

    let c_str = unsafe { CStr::from_ptr(buf.as_ptr() as *const i8) };
    let full_path_str = c_str.to_str().map_err(|_| NormalizerError::Utf8Error)?;

    Path::new(full_path_str)
        .file_name()
        .and_then(|name| name.to_str())
        .map(|s| s.to_string())
        .ok_or_else(|| NormalizerError::InvalidName(path.display().to_string()))
}

/// Normalize a single file/folder name to the target normalization form.
///
/// This function uses `get_actual_file_name` to get the real filename from disk,
/// then renames it if conversion is needed.
pub fn normalize_single_file(
    target_path: &Path,
    target: NormalizationTarget,
) -> Result<Option<ConversionResult>, NormalizerError> {
    let actual_name = get_actual_file_name(target_path)?;

    if !target.needs_conversion(&actual_name) {
        return Ok(None);
    }

    let new_name = target.convert(&actual_name);
    let actual_path = target_path.with_file_name(&actual_name);
    let new_path = target_path.with_file_name(&new_name);

    fs::rename(&actual_path, &new_path).map_err(|e| NormalizerError::RenameError {
        from: actual_path.display().to_string(),
        to: new_path.display().to_string(),
        source: e,
    })?;

    Ok(Some(ConversionResult {
        from: actual_path,
        to: new_path,
    }))
}

/// Subdirectory to recurse into, and whether it is ignored (descended only for a nested entry).
type Subdir = Option<(PathBuf, bool)>;

/// Process a single directory entry: rename if needed, return subdirectory path for recursion.
fn process_entry(
    entry: &fs::DirEntry,
    target: NormalizationTarget,
    target_folder: &Path,
    recursive: bool,
    ignore: Option<(&IgnoreScope, &Path, &NfcIgnoreChain)>,
) -> (Result<Option<ConversionResult>, ConversionError>, Subdir) {
    let path = entry.path();

    let name = match path.file_name() {
        Some(n) => n,
        None => return (Ok(None), None),
    };

    if name == "." || name == ".." {
        return (Ok(None), None);
    }

    // Ignored entries are neither renamed nor descended into, except that an
    // ignored directory containing a nested entry is still descended into.
    // Rules are rooted at canonical paths, so match against the canonical form
    // of `path` (the caller may pass a relative or non-canonical target folder).
    // file_type() comes from readdir (d_type), so no extra syscall.
    let mut skip_rename = false;
    if let Some((scope, canonical_target, chain)) = ignore
        && let Ok(rel) = path.strip_prefix(target_folder)
    {
        let is_dir = entry.file_type().is_ok_and(|t| t.is_dir());
        let full = canonical_target.join(rel);
        if scope.is_ignored(&full, is_dir, chain) {
            if !(is_dir && scope.has_entry_below(&full)) {
                return (Ok(None), None);
            }
            skip_rename = true;
        }
    }

    let original_name = name.to_string_lossy();

    let (result, new_path) = if !skip_rename && target.needs_conversion(&original_name) {
        let new_name = target.convert(&original_name);
        let renamed_path = path.with_file_name(&new_name);
        match fs::rename(&path, &renamed_path) {
            Ok(_) => (
                Ok(Some(ConversionResult {
                    from: path.clone(),
                    to: renamed_path.clone(),
                })),
                renamed_path,
            ),
            Err(e) => (
                Err(ConversionError {
                    path: path.clone(),
                    error: e.to_string(),
                }),
                path,
            ),
        }
    } else {
        (Ok(None), path)
    };

    // Check if we should recurse into this directory
    if !(recursive && new_path.is_dir()) {
        return (result, None);
    }
    let metadata = match fs::symlink_metadata(&new_path) {
        Ok(m) => m,
        Err(_) => return (result, None),
    };
    if metadata.file_type().is_symlink() || !is_same_filesystem(target_folder, &new_path) {
        return (result, None);
    }
    // An ignored directory's own .nfcignore is not read (as in git).
    (result, Some((new_path, skip_rename)))
}

/// Normalize filenames in a directory to the target normalization form.
///
/// If `recursive` is true, subdirectories are also processed.
/// Symlinks and directories on different filesystems are skipped.
/// Entries matched by `ignore` are skipped along with everything under them.
pub fn normalize_directory(
    target_folder: &Path,
    recursive: bool,
    target: NormalizationTarget,
    ignore: Option<&IgnoreScope>,
) -> Result<DirectoryResult, NormalizerError> {
    let mut converted = Vec::new();
    let mut errors = Vec::new();

    // On failure, read_dir below reports the error as before.
    let canonical_target =
        fs::canonicalize(target_folder).unwrap_or_else(|_| target_folder.to_path_buf());
    // Each queued directory carries the .nfcignore chain of its ancestors and
    // whether to read its own file (not for ignored directories).
    let start_chain = ignore.map(|scope| scope.chain_for(&canonical_target));
    let mut queue = VecDeque::new();
    queue.push_back((target_folder.to_path_buf(), start_chain, true));

    while let Some((current_dir, chain, read_own)) = queue.pop_front() {
        let chain = match (chain, current_dir.strip_prefix(target_folder)) {
            // join("") would add a trailing slash.
            (Some(c), Ok(rel)) if read_own && rel.as_os_str().is_empty() => {
                Some(c.enter(&canonical_target))
            }
            (Some(c), Ok(rel)) if read_own => Some(c.enter(&canonical_target.join(rel))),
            (c, _) => c,
        };
        let scope = ignore.zip(chain.as_ref());
        let scope = scope.map(|(s, c)| (s, canonical_target.as_path(), c));
        let entries: Vec<_> = match fs::read_dir(&current_dir) {
            Ok(entries) => entries.filter_map(|entry| entry.ok()).collect(),
            Err(e) => {
                errors.push(ConversionError {
                    path: current_dir,
                    error: e.to_string(),
                });
                continue;
            }
        };

        let results: Vec<_> = entries
            .par_iter()
            .map(|entry| process_entry(entry, target, target_folder, recursive, scope))
            .collect();

        for (result, subdir) in results {
            match result {
                Ok(Some(c)) => converted.push(c),
                Ok(None) => {}
                Err(e) => errors.push(e),
            }
            if recursive && let Some((d, ignored)) = subdir {
                queue.push_back((d, chain.clone(), !ignored));
            }
        }
    }

    Ok(DirectoryResult { converted, errors })
}

/// Check if two paths are on the same filesystem.
#[cfg(unix)]
fn is_same_filesystem(original_path: &Path, new_path: &Path) -> bool {
    let original_dev = fs::metadata(original_path).map(|m| m.dev()).unwrap_or(0);
    let new_dev = fs::metadata(new_path).map(|m| m.dev()).unwrap_or(0);
    original_dev == new_dev
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Create a file with NFD filename in the given directory.
    fn create_nfd_file(dir: &Path, name: &str) -> PathBuf {
        let nfd_name: String = name.nfd().collect();
        let path = dir.join(&nfd_name);
        File::create(&path).unwrap();
        path
    }

    #[test]
    fn test_get_actual_file_name_returns_nfd() {
        let temp = TempDir::new().unwrap();
        let nfd_name: String = "카페.txt".nfd().collect();
        let path = temp.path().join(&nfd_name);
        File::create(&path).unwrap();

        let actual = get_actual_file_name(&path).unwrap();
        assert!(!is_nfc(&actual), "Should return NFD name from disk");
    }

    #[test]
    fn test_normalize_single_file_converts_to_nfc() {
        let temp = TempDir::new().unwrap();
        let path = create_nfd_file(temp.path(), "테스트.txt");

        normalize_single_file(&path, NormalizationTarget::NFC).unwrap();

        let entries: Vec<_> = fs::read_dir(temp.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();
        assert_eq!(entries.len(), 1);

        let name = entries[0].file_name().to_string_lossy().to_string();
        assert!(is_nfc(&name), "File should be NFC after conversion");
    }

    #[test]
    fn test_normalize_directory_recursive() {
        let temp = TempDir::new().unwrap();

        // Create nested structure with NFD names
        let sub = temp.path().join("서브폴더".nfd().collect::<String>());
        fs::create_dir(&sub).unwrap();
        create_nfd_file(temp.path(), "파일1.txt");
        create_nfd_file(&sub, "파일2.txt");

        normalize_directory(temp.path(), true, NormalizationTarget::NFC, None).unwrap();

        // Verify all entries are NFC
        fn check_all_nfc(dir: &Path) -> bool {
            for entry in fs::read_dir(dir).unwrap().filter_map(|e| e.ok()) {
                let name = entry.file_name().to_string_lossy().to_string();
                if !is_nfc(&name) {
                    return false;
                }
                if entry.path().is_dir() && !check_all_nfc(&entry.path()) {
                    return false;
                }
            }
            true
        }

        assert!(check_all_nfc(temp.path()));
    }

    #[test]
    fn test_no_conversion_needed() {
        let temp = TempDir::new().unwrap();
        let nfc_name = "already_nfc.txt";
        let path = temp.path().join(nfc_name);
        File::create(&path).unwrap();

        // Should succeed without error
        normalize_single_file(&path, NormalizationTarget::NFC).unwrap();

        // File should still exist with same name
        assert!(path.exists());
    }

    #[test]
    fn test_normalize_directory_skips_ignored() {
        use crate::config::{Config, PathEntry, PathMode};

        // w/
        //   건너뛰기/안쪽.txt      ignored by /건너뛰기/
        //   건너뛰기/살림/가.txt   re-included by a nested entry (살림 itself converted)
        //   임시.tmp              ignored by global *.tmp
        //   변환.txt              converted
        let temp = TempDir::new().unwrap();
        let nfd = |s: &str| s.nfd().collect::<String>();
        let skip = temp.path().join(nfd("건너뛰기"));
        let keep = skip.join(nfd("살림"));
        fs::create_dir_all(&keep).unwrap();
        create_nfd_file(&skip, "안쪽.txt");
        create_nfd_file(&keep, "가.txt");
        create_nfd_file(temp.path(), "임시.tmp");
        create_nfd_file(temp.path(), "변환.txt");

        // Patterns and config paths are NFC; on-disk names are NFD.
        let mut root = PathEntry::new(temp.path().to_path_buf(), PathMode::Recursive);
        root.ignore = vec!["/건너뛰기/".to_string()];
        let nested = PathEntry::new(temp.path().join("건너뛰기/살림"), PathMode::Recursive);
        let mut config = Config {
            paths: vec![root, nested],
            global_ignore: vec!["*.tmp".to_string()],
        };
        config.refresh_statuses();
        let scope = config.ignore_scope_for(temp.path());

        // Traverse via the raw temp path (/var/...); entries are canonical (/private/var/...).
        normalize_directory(temp.path(), true, NormalizationTarget::NFC, Some(&scope)).unwrap();

        let names = |dir: &Path| -> Vec<String> {
            let mut v: Vec<String> = fs::read_dir(dir)
                .unwrap()
                .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
                .collect();
            v.sort();
            v
        };
        let top = names(temp.path());
        assert!(top.contains(&"변환.txt".to_string()));
        assert!(top.contains(&nfd("임시.tmp")));
        assert!(top.contains(&nfd("건너뛰기")), "ignored dir is not renamed");
        // The nested entry's own root is not ignored by its rules, so it is converted
        // (same as the watcher); its ignored parent and siblings are left alone.
        let mut expected = vec!["살림".to_string(), nfd("안쪽.txt")];
        expected.sort();
        assert_eq!(names(&skip), expected);
        assert_eq!(
            names(&keep),
            vec!["가.txt".to_string()],
            "nested entry re-includes"
        );
    }

    #[test]
    fn test_normalize_directory_reads_nfcignore() {
        use crate::config::{Config, PathEntry, PathMode};

        // w/.nfcignore        *.tmp, /skip/
        // w/sub/임시.tmp       ignored by the parent's .nfcignore, also when scanning sub/
        // w/sub/변환.txt       converted
        // w/skip/.nfcignore   !*  (not read: skip/ is ignored)
        let temp = TempDir::new().unwrap();
        let nfd = |s: &str| s.nfd().collect::<String>();
        let w = fs::canonicalize(temp.path()).unwrap();
        fs::create_dir_all(w.join("sub")).unwrap();
        fs::create_dir_all(w.join("skip")).unwrap();
        fs::write(w.join(".nfcignore"), "*.tmp\n/skip/\n").unwrap();
        fs::write(w.join("skip/.nfcignore"), "!*\n").unwrap();
        create_nfd_file(&w.join("sub"), "임시.tmp");
        create_nfd_file(&w.join("sub"), "변환.txt");
        create_nfd_file(&w.join("skip"), "안쪽.txt");

        let mut config = Config {
            paths: vec![PathEntry::new(w.clone(), PathMode::Recursive)],
            global_ignore: vec![],
        };
        config.refresh_statuses();
        let scope = config.ignore_scope_for(&w);

        // Scanning only sub/ (as the watcher does for a moved-in folder).
        normalize_directory(&w.join("sub"), true, NormalizationTarget::NFC, Some(&scope)).unwrap();
        assert!(w.join("sub").join(nfd("임시.tmp")).exists());
        assert!(
            fs::read_dir(w.join("sub"))
                .unwrap()
                .any(|e| e.unwrap().file_name() == "변환.txt")
        );

        normalize_directory(&w, true, NormalizationTarget::NFC, Some(&scope)).unwrap();
        let skip_names: Vec<_> = fs::read_dir(w.join("skip"))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
            .collect();
        assert!(
            skip_names.contains(&nfd("안쪽.txt")),
            "ignored dir's .nfcignore is not read"
        );
    }
}
