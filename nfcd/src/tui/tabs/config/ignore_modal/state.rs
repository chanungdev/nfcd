use ratatui::widgets::ListState;

use crate::tui::text_input::TextInput;

/// Which pattern list the modal edits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IgnoreTarget {
    Global,
    /// Index into `Config::paths`.
    Entry(usize),
}

/// An in-progress edit of one pattern.
#[derive(Debug)]
pub struct PatternEdit {
    pub input: TextInput,
    /// Pattern being edited, or None when adding a new one.
    pub index: Option<usize>,
    pub error: Option<String>,
}

#[derive(Debug)]
pub struct IgnoreModalState {
    pub show: bool,
    pub target: IgnoreTarget,
    /// Display name of the target (path or "(global)").
    pub title: String,
    /// Working copy; written back to the config when the modal closes.
    pub patterns: Vec<String>,
    pub list_state: ListState,
    pub edit: Option<PatternEdit>,
}

impl IgnoreModalState {
    pub fn new() -> Self {
        Self {
            show: false,
            target: IgnoreTarget::Global,
            title: String::new(),
            patterns: Vec::new(),
            list_state: ListState::default(),
            edit: None,
        }
    }

    pub fn open(&mut self, target: IgnoreTarget, title: String, patterns: Vec<String>) {
        self.show = true;
        self.target = target;
        self.title = title;
        self.list_state = ListState::default();
        if !patterns.is_empty() {
            self.list_state.select(Some(0));
        }
        self.patterns = patterns;
        self.edit = None;
    }
}
