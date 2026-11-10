use crate::constants::CONFIG_PATH;
use crate::ignore::{IgnoreRules, to_nfc};
use crate::utils::expand_tilde;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

// === TOML serde types (private) ===

#[derive(Deserialize, Serialize, Default)]
#[serde(deny_unknown_fields)]
struct ConfigFile {
    #[serde(default, skip_serializing_if = "GlobalSection::is_empty")]
    global: GlobalSection,
    #[serde(default)]
    paths: Vec<ConfigFileEntry>,
}

#[derive(Deserialize, Serialize, Default)]
#[serde(deny_unknown_fields)]
struct GlobalSection {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    ignore: Vec<String>,
}

impl GlobalSection {
    fn is_empty(&self) -> bool {
        self.ignore.is_empty()
    }
}

// deny_unknown_fields: a leftover nfd2nfc `action = "ignore"` line must fail
// loudly instead of silently turning an ignored path into a watched one.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ConfigFileEntry {
    path: String,
    mode: PathMode,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    ignore: Vec<String>,
}

// === Public types ===

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PathMode {
    Recursive,
    Children,
}

impl PathMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            PathMode::Recursive => "Recursive",
            PathMode::Children => "Children",
        }
    }

    pub fn toggle(&self) -> Self {
        match self {
            PathMode::Recursive => PathMode::Children,
            PathMode::Children => PathMode::Recursive,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum PathStatus {
    Active,
    NotFound,
    NotADirectory,
    PermissionDenied,
    Redundant(usize),
}

impl PathStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            PathStatus::Active => "Active",
            PathStatus::NotFound => "Not Found",
            PathStatus::NotADirectory => "Not a Dir",
            PathStatus::PermissionDenied => "No Access",
            PathStatus::Redundant(_) => "Redundant",
        }
    }

    pub fn symbol(&self) -> &'static str {
        match self {
            PathStatus::Active => "✓",
            PathStatus::Redundant(_) => "~",
            PathStatus::NotFound | PathStatus::NotADirectory | PathStatus::PermissionDenied => "✗",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PathEntry {
    pub raw: String,
    pub mode: PathMode,
    pub canonical: Option<PathBuf>,
    pub status: PathStatus,
    /// gitignore-style patterns relative to this entry's path.
    pub ignore: Vec<String>,
}

impl PathEntry {
    /// Create a new PathEntry from a PathBuf, running validate_path automatically.
    pub fn new(path: PathBuf, mode: PathMode) -> Self {
        let raw = path.to_string_lossy().to_string();
        let (canonical, status) = validate_path(&raw);
        Self {
            raw,
            mode,
            canonical,
            status,
            ignore: Vec::new(),
        }
    }

    fn from_config_entry(cfe: ConfigFileEntry) -> Self {
        let (canonical, status) = validate_path(&cfe.path);
        Self {
            raw: cfe.path,
            mode: cfe.mode,
            canonical,
            status,
            ignore: cfe.ignore,
        }
    }

    fn to_config_entry(&self) -> ConfigFileEntry {
        ConfigFileEntry {
            path: self.raw.clone(),
            mode: self.mode,
            ignore: self.ignore.clone(),
        }
    }
}

/// Active entry summary for watcher event filtering.
#[derive(Debug, Clone)]
pub struct ActiveEntry {
    pub canonical: PathBuf,
    pub mode: PathMode,
    /// Compiled global + entry patterns, rooted at `canonical`.
    pub ignore: IgnoreRules,
    /// NFC form of `canonical`. Matching in NFC keeps working after a parent
    /// directory is renamed NFD→NFC while the watcher is running.
    match_root: PathBuf,
}

impl ActiveEntry {
    fn covers(&self, nfc_path: &Path) -> bool {
        match self.mode {
            PathMode::Recursive => nfc_path.starts_with(&self.match_root),
            PathMode::Children => nfc_path.parent() == Some(&*self.match_root),
        }
    }
}

/// The most specific (longest root) active entry covering `path`.
/// Shared by the watcher and manual conversion so both route paths the same way.
pub fn effective_entry<'a>(path: &Path, entries: &'a [ActiveEntry]) -> Option<&'a ActiveEntry> {
    let path = to_nfc(path);
    entries
        .iter()
        .filter(|e| e.covers(&path))
        .max_by_key(|e| e.match_root.as_os_str().len())
}

/// Ignore rules for a manual conversion: each path uses the rules of its
/// effective entry (as in the watcher), or global patterns if no entry covers it.
#[derive(Debug, Clone)]
pub struct IgnoreScope {
    entries: Vec<ActiveEntry>,
    fallback: IgnoreRules,
}

impl IgnoreScope {
    pub fn is_ignored(&self, path: &Path, is_dir: bool) -> bool {
        match effective_entry(path, &self.entries) {
            Some(e) => e.ignore.is_ignored(path, is_dir),
            None => self.fallback.is_ignored(path, is_dir),
        }
    }

    /// Whether an active entry lives strictly below `dir`. Such a directory is
    /// still descended into when ignored, since the nested entry re-includes it.
    pub fn has_entry_below(&self, dir: &Path) -> bool {
        let dir = to_nfc(dir);
        self.entries
            .iter()
            .any(|e| e.match_root != dir && e.match_root.starts_with(&dir))
    }
}

#[derive(Debug, Clone, Default)]
pub struct Config {
    pub paths: Vec<PathEntry>,
    /// gitignore-style patterns applied to every watch entry.
    pub global_ignore: Vec<String>,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Parse error: {0}")]
    Parse(#[from] toml::de::Error),

    #[error("Serialization error: {0}")]
    Serialize(#[from] toml::ser::Error),
}

impl ConfigError {
    pub fn user_message(&self) -> &'static str {
        match self {
            ConfigError::Io(_) => "Failed to read config file",
            ConfigError::Parse(_) => "Config file format is invalid",
            ConfigError::Serialize(_) => "Failed to save config file",
        }
    }
}

// === Functions ===

/// Map an IO error to the corresponding PathStatus.
fn io_error_to_status(e: &std::io::Error) -> PathStatus {
    match e.kind() {
        std::io::ErrorKind::PermissionDenied => PathStatus::PermissionDenied,
        _ => PathStatus::NotFound,
    }
}

/// Validate a single path string. Returns (canonical PathBuf if valid, status).
pub(crate) fn validate_path(path_str: &str) -> (Option<PathBuf>, PathStatus) {
    let trimmed = path_str.trim();
    if trimmed.is_empty() {
        return (None, PathStatus::NotFound);
    }

    let expanded = expand_tilde(trimmed);
    let canonical = match fs::canonicalize(&expanded) {
        Ok(p) => p,
        Err(e) => return (None, io_error_to_status(&e)),
    };

    match fs::metadata(&canonical) {
        Ok(meta) => {
            if !meta.is_dir() {
                (None, PathStatus::NotADirectory)
            } else {
                (Some(canonical), PathStatus::Active)
            }
        }
        Err(e) => (None, io_error_to_status(&e)),
    }
}

/// Build processing order: indices sorted by canonical path length (shortest first),
/// then by original index for stable ordering.
fn build_processing_order(entries: &[PathEntry]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..entries.len()).collect();
    let canonical_len = |idx: usize| {
        entries[idx]
            .canonical
            .as_ref()
            .map(|p| p.as_os_str().len())
            .unwrap_or(0)
    };
    order.sort_by(|&a, &b| canonical_len(a).cmp(&canonical_len(b)).then(a.cmp(&b)));
    order
}

/// Find the most specific Active Recursive parent entry that covers the given canonical path.
fn find_fallback_parent(
    entries: &[PathEntry],
    statuses: &[PathStatus],
    idx: usize,
    canonical: &Path,
) -> Option<usize> {
    let mut best_index: Option<usize> = None;
    let mut best_len: usize = 0;
    for (j, entry_j) in entries.iter().enumerate() {
        if j == idx || !matches!(statuses[j], PathStatus::Active) {
            continue;
        }
        if let Some(ref jc) = entry_j.canonical {
            let is_parent = matches!(entry_j.mode, PathMode::Recursive)
                && *jc != *canonical
                && canonical.starts_with(jc);
            if is_parent {
                let jlen = jc.as_os_str().len();
                if best_index.is_none() || jlen > best_len {
                    best_index = Some(j);
                    best_len = jlen;
                }
            }
        }
    }
    best_index
}

/// Compute redundancy statuses for all entries.
///
/// Phase 1 — Exact duplicates (same canonical path): one winner per path,
///   Recursive preferred, then lowest index. Others = Redundant(winner).
///
/// Phase 2 — Nested entries, processed shortest path first:
///   For each Active entry E, find the most specific Active Recursive parent P.
///   E is Redundant(P) when P would handle E's subtree identically, i.e. E has
///   no patterns of its own and P's rules don't ignore E's path. Otherwise E
///   stays Active and takes over its subtree (global + its own patterns only).
pub(crate) fn compute_statuses(entries: &mut [PathEntry], global_ignore: &[String]) {
    let mut statuses: Vec<PathStatus> = entries
        .iter()
        .map(|e| {
            if e.canonical.is_some() {
                PathStatus::Active // tentative
            } else {
                e.status
            }
        })
        .collect();

    // Phase 1: Resolve exact-path duplicates
    for i in 0..entries.len() {
        if !matches!(statuses[i], PathStatus::Active) {
            continue;
        }
        let Some(canonical_i) = entries[i].canonical.as_ref() else {
            continue;
        };
        let group: Vec<usize> = (i..entries.len())
            .filter(|&j| {
                matches!(statuses[j], PathStatus::Active)
                    && entries[j].canonical.as_ref() == Some(canonical_i)
            })
            .collect();
        if group.len() < 2 {
            continue;
        }
        let winner = *group
            .iter()
            .min_by_key(|&&j| (entries[j].mode != PathMode::Recursive, j))
            .unwrap(); // group is non-empty
        for &j in &group {
            if j != winner {
                statuses[j] = PathStatus::Redundant(winner);
            }
        }
    }

    // Phase 2: Resolve nested entries
    for idx in build_processing_order(entries) {
        if !matches!(statuses[idx], PathStatus::Active) {
            continue;
        }
        let Some(canonical) = entries[idx].canonical.as_ref() else {
            continue;
        };
        let Some(p) = find_fallback_parent(entries, &statuses, idx, canonical) else {
            continue;
        };
        let parent = &entries[p];
        let Some(parent_root) = parent.canonical.as_ref() else {
            continue;
        };
        let ignored_by_parent = IgnoreRules::new(parent_root, global_ignore, &parent.ignore)
            .is_ignored(canonical, true);
        if entries[idx].ignore.is_empty() && !ignored_by_parent {
            statuses[idx] = PathStatus::Redundant(p);
        }
    }

    for (entry, status) in entries.iter_mut().zip(statuses) {
        entry.status = status;
    }
}

/// Load configuration from the default config path.
/// Returns (Config, Option<ConfigError>).
/// File not found → empty config, no error (normal for new installs).
/// Parse error → empty config with error.
pub fn load_config() -> (Config, Option<ConfigError>) {
    let path = &*CONFIG_PATH;

    if !path.exists() {
        return (Config::default(), None);
    }

    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => return (Config::default(), Some(ConfigError::Io(e))),
    };

    let config_file: ConfigFile = match toml::from_str(&content) {
        Ok(c) => c,
        Err(e) => return (Config::default(), Some(ConfigError::Parse(e))),
    };

    let mut entries: Vec<PathEntry> = config_file
        .paths
        .into_iter()
        .map(PathEntry::from_config_entry)
        .collect();

    let global_ignore = config_file.global.ignore;
    compute_statuses(&mut entries, &global_ignore);

    (
        Config {
            paths: entries,
            global_ignore,
        },
        None,
    )
}

impl Config {
    /// Re-validate all paths and recompute redundancy statuses.
    pub fn refresh_statuses(&mut self) {
        for entry in &mut self.paths {
            let (canonical, status) = validate_path(&entry.raw);
            entry.canonical = canonical;
            entry.status = status;
        }
        compute_statuses(&mut self.paths, &self.global_ignore);
    }

    /// Extract active entries for watcher event filtering.
    pub fn active_entries(&self) -> Vec<ActiveEntry> {
        self.paths
            .iter()
            .filter_map(|e| {
                if !matches!(e.status, PathStatus::Active) {
                    return None;
                }
                e.canonical.as_ref().map(|c| ActiveEntry {
                    canonical: c.clone(),
                    mode: e.mode,
                    ignore: IgnoreRules::new(c, &self.global_ignore, &e.ignore),
                    match_root: to_nfc(c),
                })
            })
            .collect()
    }

    /// Ignore scope for a manual conversion of `target`. Paths not covered by
    /// any entry fall back to global patterns rooted at `target`.
    pub fn ignore_scope_for(&self, target: &Path) -> IgnoreScope {
        let target = fs::canonicalize(target).unwrap_or_else(|_| target.to_path_buf());
        IgnoreScope {
            entries: self.active_entries(),
            fallback: IgnoreRules::new(&target, &self.global_ignore, &[]),
        }
    }

    /// Save config to file in [[paths]] TOML format.
    pub fn save_to_file(&self, path: &Path) -> Result<(), ConfigError> {
        let config_file = ConfigFile {
            global: GlobalSection {
                ignore: self.global_ignore.clone(),
            },
            paths: self.paths.iter().map(PathEntry::to_config_entry).collect(),
        };

        let toml_content = toml::to_string_pretty(&config_file)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, toml_content)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a PathEntry with a synthetic canonical path (no filesystem access needed).
    fn make_entry(canonical: &str, mode: PathMode) -> PathEntry {
        PathEntry {
            raw: canonical.to_string(),
            mode,
            canonical: Some(PathBuf::from(canonical)),
            status: PathStatus::Active,
            ignore: Vec::new(),
        }
    }

    fn with_ignore(mut entry: PathEntry, patterns: &[&str]) -> PathEntry {
        entry.ignore = patterns.iter().map(|p| p.to_string()).collect();
        entry
    }

    fn make_not_found(raw: &str) -> PathEntry {
        PathEntry {
            raw: raw.to_string(),
            mode: PathMode::Recursive,
            canonical: None,
            status: PathStatus::NotFound,
            ignore: Vec::new(),
        }
    }

    fn statuses(entries: &mut [PathEntry], global: &[&str]) -> Vec<PathStatus> {
        let global: Vec<String> = global.iter().map(|p| p.to_string()).collect();
        compute_statuses(entries, &global);
        entries.iter().map(|e| e.status).collect()
    }

    use PathMode::{Children, Recursive};
    use PathStatus::{Active, Redundant};

    // --- Same path ---

    #[test]
    fn duplicate_same_mode() {
        let mut e = vec![make_entry("/a", Recursive), make_entry("/a", Recursive)];
        assert_eq!(statuses(&mut e, &[]), [Active, Redundant(0)]);
    }

    #[test]
    fn duplicate_recursive_over_children() {
        let mut e = vec![make_entry("/a", Children), make_entry("/a", Recursive)];
        assert_eq!(statuses(&mut e, &[]), [Redundant(1), Active]);
    }

    #[test]
    fn duplicate_children_after_recursive() {
        let mut e = vec![make_entry("/a", Recursive), make_entry("/a", Children)];
        assert_eq!(statuses(&mut e, &[]), [Active, Redundant(0)]);
    }

    #[test]
    fn triple_children_children_recursive() {
        let mut e = vec![
            make_entry("/a", Children),
            make_entry("/a", Children),
            make_entry("/a", Recursive),
        ];
        assert_eq!(statuses(&mut e, &[]), [Redundant(2), Redundant(2), Active]);
    }

    // --- Nested entries ---

    #[test]
    fn nested_without_patterns_is_redundant() {
        let mut e = vec![make_entry("/a", Recursive), make_entry("/a/b", Recursive)];
        assert_eq!(statuses(&mut e, &[]), [Active, Redundant(0)]);
    }

    #[test]
    fn nested_with_own_patterns_is_active() {
        let mut e = vec![
            make_entry("/a", Recursive),
            with_ignore(make_entry("/a/b", Recursive), &["*.tmp"]),
        ];
        assert_eq!(statuses(&mut e, &[]), [Active, Active]);
    }

    #[test]
    fn nested_reincludes_path_ignored_by_parent() {
        let mut e = vec![
            with_ignore(make_entry("/a", Recursive), &["/b/"]),
            make_entry("/a/b/c", Children),
        ];
        assert_eq!(statuses(&mut e, &[]), [Active, Active]);
    }

    #[test]
    fn nested_reincludes_path_ignored_globally() {
        let mut e = vec![
            make_entry("/a", Recursive),
            make_entry("/a/x/node_modules/m", Recursive),
        ];
        assert_eq!(statuses(&mut e, &["node_modules/"]), [Active, Active]);
    }

    #[test]
    fn nested_compares_against_nearest_active_parent() {
        // /a/b is redundant, so /a/b/c is compared against /a.
        let mut e = vec![
            make_entry("/a", Recursive),
            make_entry("/a/b", Recursive),
            make_entry("/a/b/c", Recursive),
        ];
        assert_eq!(statuses(&mut e, &[]), [Active, Redundant(0), Redundant(0)]);
    }

    #[test]
    fn children_mode_parent_no_coverage() {
        let mut e = vec![make_entry("/a", Children), make_entry("/a/b", Recursive)];
        assert_eq!(statuses(&mut e, &[]), [Active, Active]);
    }

    // --- effective_entry ---

    #[test]
    fn effective_entry_picks_longest_and_matches_across_nfc_rename() {
        use unicode_normalization::UnicodeNormalization;
        let nfd: String = "자료".nfd().collect();
        // Canonical paths were captured while "자료" was NFD on disk.
        let config = Config {
            paths: vec![
                make_entry("/w", Recursive),
                make_entry(&format!("/w/{nfd}/b"), Recursive),
                make_entry("/c", Children),
            ],
            global_ignore: vec![],
        };
        let entries = config.active_entries();
        // After "자료" is renamed to NFC, events arrive with NFC paths.
        let hit = effective_entry(Path::new("/w/자료/b/x.txt"), &entries).unwrap();
        assert_eq!(hit.canonical, PathBuf::from(format!("/w/{nfd}/b")));
        assert_eq!(
            effective_entry(Path::new("/w/y.txt"), &entries)
                .unwrap()
                .canonical,
            PathBuf::from("/w")
        );
        assert!(effective_entry(Path::new("/c/d/x.txt"), &entries).is_none());
        assert!(effective_entry(Path::new("/c/x.txt"), &entries).is_some());
    }

    // --- Edge cases ---

    #[test]
    fn empty_entries() {
        let mut e: Vec<PathEntry> = vec![];
        assert!(statuses(&mut e, &[]).is_empty());
    }

    #[test]
    fn not_found_entry_keeps_status() {
        let mut e = vec![make_entry("/a", Recursive), make_not_found("/nonexistent")];
        assert_eq!(statuses(&mut e, &[]), [Active, PathStatus::NotFound]);
    }

    #[test]
    fn ignore_patterns_round_trip() {
        let src = r#"[global]
ignore = [".git/", "*.app/"]

[[paths]]
path = "~/Downloads"
mode = "recursive"
ignore = ["*.crdownload", "!/torrents/한글/"]

[[paths]]
path = "~/Documents"
mode = "children"
"#;
        let parsed: ConfigFile = toml::from_str(src).unwrap();
        assert_eq!(parsed.global.ignore, [".git/", "*.app/"]);
        assert_eq!(parsed.paths[0].ignore, ["*.crdownload", "!/torrents/한글/"]);
        assert!(parsed.paths[1].ignore.is_empty());
        let out = toml::to_string_pretty(&parsed).unwrap();
        let reparsed: ConfigFile = toml::from_str(&out).unwrap();
        assert_eq!(reparsed.global.ignore, parsed.global.ignore);
        assert_eq!(reparsed.paths[0].ignore, parsed.paths[0].ignore);
        assert!(!out.contains("ignore = []"), "empty lists must be omitted");
    }

    #[test]
    fn legacy_action_field_is_rejected() {
        let src = "[[paths]]\npath = \"~/a\"\naction = \"ignore\"\nmode = \"recursive\"\n";
        assert!(toml::from_str::<ConfigFile>(src).is_err());
    }

    #[test]
    fn config_without_ignore_still_parses() {
        let src = "[[paths]]\npath = \"~/a\"\nmode = \"recursive\"\n";
        let parsed: ConfigFile = toml::from_str(src).unwrap();
        assert!(parsed.global.ignore.is_empty());
        assert_eq!(toml::to_string_pretty(&parsed).unwrap(), src);
    }
}
