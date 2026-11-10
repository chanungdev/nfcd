//! gitignore-style ignore patterns for watch entries.
//!
//! Patterns come from the config file (NFC), while paths from FSEvents or
//! `canonicalize` may be NFD. Both sides are normalized to NFC before matching,
//! otherwise Korean patterns would silently never match.

use ignore::gitignore::{Gitignore, GitignoreBuilder};
use log::warn;
use std::path::{Path, PathBuf};
use unicode_normalization::UnicodeNormalization;

/// Compiled ignore patterns anchored at a root directory.
#[derive(Debug, Clone)]
pub struct IgnoreRules {
    root: PathBuf,
    matcher: Gitignore,
}

pub(crate) fn to_nfc(path: &Path) -> PathBuf {
    PathBuf::from(path.to_string_lossy().nfc().collect::<String>())
}

impl IgnoreRules {
    /// Compile `global` then `local` patterns (later lines take precedence, as in gitignore).
    /// Invalid patterns are logged and skipped so one typo doesn't disable the whole config.
    pub fn new(root: &Path, global: &[String], local: &[String]) -> Self {
        let root = to_nfc(root);
        let mut builder = GitignoreBuilder::new(&root);
        for pattern in global.iter().chain(local) {
            let pattern: String = pattern.nfc().collect();
            if let Err(e) = builder.add_line(None, &pattern) {
                warn!("Invalid ignore pattern {:?}: {}", pattern, e);
            }
        }
        let matcher = builder.build().unwrap_or_else(|e| {
            warn!("Failed to build ignore rules for {}: {}", root.display(), e);
            Gitignore::empty()
        });
        Self { root, matcher }
    }

    /// Whether `path` (or any of its parents up to the root) is ignored.
    /// Paths outside the root (or the root itself) are never ignored.
    pub fn is_ignored(&self, path: &Path, is_dir: bool) -> bool {
        let path = to_nfc(path);
        // matched_path_or_any_parents panics on paths outside the root.
        if path == self.root || !path.starts_with(&self.root) {
            return false;
        }
        self.matcher
            .matched_path_or_any_parents(&path, is_dir)
            .is_ignore()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules(global: &[&str], local: &[&str]) -> IgnoreRules {
        let s = |v: &[&str]| v.iter().map(|p| p.to_string()).collect::<Vec<_>>();
        IgnoreRules::new(Path::new("/w"), &s(global), &s(local))
    }

    fn nfd(s: &str) -> String {
        s.nfd().collect()
    }

    #[test]
    fn glob_and_parent_dir() {
        let r = rules(&["*.crdownload", "node_modules/"], &[]);
        assert!(r.is_ignored(Path::new("/w/a.crdownload"), false));
        assert!(r.is_ignored(Path::new("/w/x/node_modules/파일.txt"), false));
        assert!(!r.is_ignored(Path::new("/w/파일.txt"), false));
    }

    #[test]
    fn dir_only_rule() {
        let r = rules(&["build/"], &[]);
        assert!(r.is_ignored(Path::new("/w/build"), true));
        assert!(!r.is_ignored(Path::new("/w/build"), false));
    }

    #[test]
    fn anchored_and_negation() {
        let r = rules(&[], &["/torrents/*", "!/torrents/keep/"]);
        assert!(r.is_ignored(Path::new("/w/torrents/a.txt"), false));
        assert!(!r.is_ignored(Path::new("/w/torrents/keep/a.txt"), false));
        // Anchored: nested "torrents" is not matched.
        assert!(!r.is_ignored(Path::new("/w/x/torrents/a.txt"), false));
    }

    #[test]
    fn local_overrides_global() {
        let r = rules(&["*.tmp"], &["!keep.tmp"]);
        assert!(r.is_ignored(Path::new("/w/a.tmp"), false));
        assert!(!r.is_ignored(Path::new("/w/keep.tmp"), false));
    }

    #[test]
    fn nfc_pattern_matches_nfd_path() {
        let r = rules(&["/자료/"], &[]);
        let p = format!("/w/{}/파일.txt", nfd("자료"));
        assert!(r.is_ignored(Path::new(&p), false));
    }

    #[test]
    fn nfd_pattern_matches_nfc_path() {
        let r = rules(&[&nfd("*.한글")], &[]);
        assert!(r.is_ignored(Path::new("/w/a.한글"), false));
    }

    #[test]
    fn nfd_root_matches() {
        let root = format!("/{}", nfd("감시"));
        let r = IgnoreRules::new(Path::new(&root), &["*.tmp".into()], &[]);
        assert!(r.is_ignored(Path::new("/감시/a.tmp"), false));
    }

    #[test]
    fn outside_root_and_root_itself_not_ignored() {
        let r = rules(&["*"], &[]);
        assert!(!r.is_ignored(Path::new("/other/a.txt"), false));
        assert!(!r.is_ignored(Path::new("/w"), true));
        assert!(!r.is_ignored(Path::new("/wx/a.txt"), false));
    }

    #[test]
    fn invalid_pattern_skipped() {
        let r = rules(&["a{", "*.tmp"], &[]);
        assert!(r.is_ignored(Path::new("/w/a.tmp"), false));
    }
}
