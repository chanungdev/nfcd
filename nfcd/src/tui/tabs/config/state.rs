use std::path::PathBuf;
use std::time::Duration;

use ratatui::widgets::TableState;

use nfcd_core::config::{Config, PathEntry, PathMode, load_config};
use nfcd_core::constants::CONFIG_PATH;

use crate::tui::component::{SharedState, next_index, prev_index};
use crate::tui::tabs::Tab;
use crate::tui::tabs::config::ignore_modal::events::IgnoreEditResult;
use crate::tui::tabs::config::ignore_modal::state::{IgnoreModalState, IgnoreTarget};
use crate::tui::tabs::config::modal::state::AddModalState;
use crate::tui::tick_timer::TickTimer;

const STATUS_REFRESH_INTERVAL: Duration = Duration::from_millis(500);
const CONFIG_RELOAD_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Debug)]
pub struct ConfigState {
    pub config: Config,
    pub table_state: TableState,
    pub modal: AddModalState,
    pub ignore_modal: IgnoreModalState,
    pub has_changes: bool,
    status_refresh_timer: TickTimer,
    config_reload_timer: TickTimer,
}

impl ConfigState {
    pub fn from_config(config: Config) -> Self {
        let mut state = Self {
            config,
            table_state: TableState::default(),
            modal: AddModalState::new(),
            ignore_modal: IgnoreModalState::new(),
            has_changes: false,
            status_refresh_timer: TickTimer::new(STATUS_REFRESH_INTERVAL),
            config_reload_timer: TickTimer::new(CONFIG_RELOAD_INTERVAL),
        };

        // Row 0 is the global patterns row; start on the first path if any.
        let first = if state.config.paths.is_empty() { 0 } else { 1 };
        state.table_state.select(Some(first));

        state
    }

    /// Table rows: the global row plus one per path.
    pub fn row_count(&self) -> usize {
        self.config.paths.len() + 1
    }

    /// Index into `config.paths` of the selected row (None for the global row).
    pub fn selected_path(&self) -> Option<usize> {
        self.table_state.selected()?.checked_sub(1)
    }

    fn mark_changed(&mut self) {
        self.has_changes = true;
        self.config.refresh_statuses();
    }

    pub fn select_next(&mut self) {
        if let Some(i) = next_index(self.table_state.selected(), self.row_count()) {
            self.table_state.select(Some(i));
        }
    }

    pub fn select_previous(&mut self) {
        if let Some(i) = prev_index(self.table_state.selected(), self.row_count()) {
            self.table_state.select(Some(i));
        }
    }

    fn move_item(&mut self, offset: isize) {
        if let Some(i) = self.selected_path()
            && let Some(new_i) = i.checked_add_signed(offset)
            && new_i < self.config.paths.len()
        {
            self.config.paths.swap(i, new_i);
            self.table_state.select(Some(new_i + 1));
            self.mark_changed();
        }
    }

    pub fn move_up(&mut self) {
        self.move_item(-1);
    }
    pub fn move_down(&mut self) {
        self.move_item(1);
    }

    pub fn toggle_mode(&mut self) {
        if let Some(i) = self.selected_path() {
            self.config.paths[i].mode = self.config.paths[i].mode.toggle();
            self.mark_changed();
        }
    }

    pub fn delete_selected(&mut self) {
        if let Some(i) = self.selected_path() {
            self.config.paths.remove(i);
            // Stay on the same row, clamped; row 0 (global) always exists.
            self.table_state
                .select(Some((i + 1).min(self.row_count() - 1)));
            self.mark_changed();
        }
    }

    pub fn add_path(&mut self, path: PathBuf, mode: PathMode) {
        self.config.paths.push(PathEntry::new(path, mode));
        self.mark_changed();
        self.table_state.select(Some(self.config.paths.len()));
    }

    /// Open the pattern editor for the selected row (global or a path).
    pub fn open_ignore_editor(&mut self) {
        let (target, title, patterns) = match self.selected_path() {
            None => (
                IgnoreTarget::Global,
                "(global)".to_string(),
                self.config.global_ignore.clone(),
            ),
            Some(i) => {
                let entry = &self.config.paths[i];
                (
                    IgnoreTarget::Entry(i),
                    nfcd_core::utils::abbreviate_home_path(std::path::Path::new(&entry.raw)),
                    entry.ignore.clone(),
                )
            }
        };
        self.ignore_modal.open(target, title, patterns);
    }

    /// Write back patterns from the editor; marks unsaved only if they changed.
    pub fn apply_ignore_edit(&mut self, result: IgnoreEditResult) {
        let slot = match result.target {
            IgnoreTarget::Global => &mut self.config.global_ignore,
            IgnoreTarget::Entry(i) => match self.config.paths.get_mut(i) {
                Some(entry) => &mut entry.ignore,
                None => return,
            },
        };
        if *slot != result.patterns {
            *slot = result.patterns;
            self.mark_changed();
        }
    }

    pub fn reload(&mut self) {
        let selected = self.table_state.selected();
        let (config, _) = load_config();
        *self = Self::from_config(config);
        if let Some(i) = selected
            && i < self.row_count()
        {
            self.table_state.select(Some(i));
        }
    }

    pub fn sort_paths(&mut self) {
        if self.config.paths.len() <= 1 {
            return;
        }
        let selected_raw = self
            .selected_path()
            .map(|i| self.config.paths[i].raw.clone());
        self.config.paths.sort_by(|a, b| a.raw.cmp(&b.raw));
        if let Some(raw) = selected_raw
            && let Some(new_idx) = self.config.paths.iter().position(|p| p.raw == raw)
        {
            self.table_state.select(Some(new_idx + 1));
        }
        self.mark_changed();
    }

    pub fn poll(&mut self, shared: &SharedState) {
        if shared.current_tab == Tab::Config {
            self.modal.tick();
            // Reloading replaces the whole state, which would close an open modal.
            if !self.modal.show && !self.ignore_modal.show {
                if self.status_refresh_timer.ready() {
                    self.config.refresh_statuses();
                }
                if !self.has_changes && self.config_reload_timer.ready() {
                    self.reload();
                }
            }
        }
    }

    pub fn save(&mut self) -> Result<(), String> {
        self.config
            .save_to_file(&CONFIG_PATH)
            .map_err(|e| format!("Failed to save config: {}", e))?;
        self.has_changes = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(paths: &[&str]) -> ConfigState {
        let config = Config {
            paths: paths
                .iter()
                .map(|p| PathEntry::new(PathBuf::from(p), PathMode::Recursive))
                .collect(),
            global_ignore: vec![],
        };
        ConfigState::from_config(config)
    }

    fn raws(s: &ConfigState) -> Vec<&str> {
        s.config.paths.iter().map(|p| p.raw.as_str()).collect()
    }

    #[test]
    fn global_row_offsets_path_selection() {
        let mut s = state(&["/a", "/b"]);
        assert_eq!(s.selected_path(), Some(0)); // starts on the first path
        s.select_previous();
        assert_eq!(s.selected_path(), None); // global row
        s.delete_selected(); // no-op on global row
        s.move_down();
        assert_eq!(raws(&s), ["/a", "/b"]);
        s.select_next();
        s.select_next();
        s.select_next(); // clamped at last row
        assert_eq!(s.selected_path(), Some(1));
    }

    #[test]
    fn move_and_delete_keep_selection_on_paths() {
        let mut s = state(&["/a", "/b"]);
        s.move_up(); // already the first path: can't move into the global row
        assert_eq!(raws(&s), ["/a", "/b"]);
        s.move_down();
        assert_eq!((raws(&s), s.selected_path()), (vec!["/b", "/a"], Some(1)));
        s.delete_selected();
        assert_eq!((raws(&s), s.selected_path()), (vec!["/b"], Some(0)));
        s.delete_selected();
        assert_eq!(s.selected_path(), None); // only the global row is left
    }

    #[test]
    fn ignore_editor_round_trip_marks_changes_only_when_edited() {
        let mut s = state(&["/a"]);
        s.open_ignore_editor();
        assert_eq!(s.ignore_modal.target, IgnoreTarget::Entry(0));
        s.apply_ignore_edit(IgnoreEditResult {
            target: IgnoreTarget::Entry(0),
            patterns: vec![],
        });
        assert!(!s.has_changes);

        s.select_previous();
        s.open_ignore_editor();
        assert_eq!(s.ignore_modal.target, IgnoreTarget::Global);
        s.apply_ignore_edit(IgnoreEditResult {
            target: IgnoreTarget::Global,
            patterns: vec![".git/".into()],
        });
        assert!(s.has_changes);
        assert_eq!(s.config.global_ignore, [".git/"]);
    }
}
