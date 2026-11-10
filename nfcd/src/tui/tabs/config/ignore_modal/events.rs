use crossterm::event::KeyCode;
use nfcd_core::ignore::validate_pattern;

use crate::tui::component::{ScrollDirection, next_index, prev_index};
use crate::tui::tabs::config::ignore_modal::state::{IgnoreModalState, IgnoreTarget, PatternEdit};
use crate::tui::text_input::TextInput;

/// Patterns to write back when the modal closes.
pub struct IgnoreEditResult {
    pub target: IgnoreTarget,
    pub patterns: Vec<String>,
}

/// Handle a key while the modal is shown. Returns the edited patterns when it closes.
pub fn handle_key(modal: &mut IgnoreModalState, key: KeyCode) -> Option<IgnoreEditResult> {
    if modal.edit.is_some() {
        handle_edit_key(modal, key);
        return None;
    }

    let selected = modal.list_state.selected();
    match key {
        KeyCode::Esc => {
            modal.show = false;
            return Some(IgnoreEditResult {
                target: modal.target,
                patterns: std::mem::take(&mut modal.patterns),
            });
        }
        KeyCode::Up | KeyCode::Char('k') => {
            select(modal, prev_index(selected, modal.patterns.len()))
        }
        KeyCode::Down | KeyCode::Char('j') => {
            select(modal, next_index(selected, modal.patterns.len()))
        }
        KeyCode::Char('a') => {
            modal.edit = Some(PatternEdit {
                input: TextInput::default(),
                index: None,
                error: None,
            });
        }
        KeyCode::Char('e') | KeyCode::Enter => {
            if let Some(i) = selected {
                modal.edit = Some(PatternEdit {
                    input: TextInput::new(&modal.patterns[i]),
                    index: Some(i),
                    error: None,
                });
            }
        }
        KeyCode::Char('d') | KeyCode::Delete => {
            if let Some(i) = selected {
                modal.patterns.remove(i);
                let len = modal.patterns.len();
                select(modal, (len > 0).then(|| i.min(len - 1)));
            }
        }
        // Order matters: later patterns win, so `!` exceptions go after what they undo.
        KeyCode::Char('+') | KeyCode::Char('=') => move_selected(modal, -1),
        KeyCode::Char('-') | KeyCode::Char('_') => move_selected(modal, 1),
        _ => {}
    }
    None
}

fn handle_edit_key(modal: &mut IgnoreModalState, key: KeyCode) {
    let Some(edit) = modal.edit.as_mut() else {
        return;
    };
    match key {
        KeyCode::Esc => modal.edit = None,
        KeyCode::Enter => {
            let pattern = edit.input.value().trim().to_string();
            if pattern.is_empty() {
                modal.edit = None;
                return;
            }
            if let Err(e) = validate_pattern(&pattern) {
                edit.error = Some(e);
                return;
            }
            let index = edit.index;
            modal.edit = None;
            let i = match index {
                Some(i) => {
                    modal.patterns[i] = pattern;
                    i
                }
                None => {
                    modal.patterns.push(pattern);
                    modal.patterns.len() - 1
                }
            };
            select(modal, Some(i));
        }
        _ => {
            if edit.input.handle_key(key) {
                edit.error = None;
            }
        }
    }
}

fn select(modal: &mut IgnoreModalState, idx: Option<usize>) {
    modal.list_state.select(idx);
}

fn move_selected(modal: &mut IgnoreModalState, offset: isize) {
    if let Some(i) = modal.list_state.selected()
        && let Some(j) = i.checked_add_signed(offset)
        && j < modal.patterns.len()
    {
        modal.patterns.swap(i, j);
        select(modal, Some(j));
    }
}

pub fn handle_scroll(modal: &mut IgnoreModalState, direction: ScrollDirection) {
    if modal.edit.is_some() {
        return;
    }
    let selected = modal.list_state.selected();
    let len = modal.patterns.len();
    select(
        modal,
        match direction {
            ScrollDirection::Up => prev_index(selected, len),
            ScrollDirection::Down => next_index(selected, len),
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn modal(patterns: &[&str]) -> IgnoreModalState {
        let mut m = IgnoreModalState::new();
        m.open(
            IgnoreTarget::Entry(0),
            "x".into(),
            patterns.iter().map(|p| p.to_string()).collect(),
        );
        m
    }

    fn type_str(m: &mut IgnoreModalState, s: &str) {
        for c in s.chars() {
            handle_key(m, KeyCode::Char(c));
        }
    }

    #[test]
    fn add_edit_delete_move_and_close() {
        let mut m = modal(&["*.tmp"]);
        handle_key(&mut m, KeyCode::Char('a'));
        type_str(&mut m, "/자료/"); // 'd', 'a' etc. go to the input, not shortcuts
        handle_key(&mut m, KeyCode::Enter);
        assert_eq!(m.patterns, ["*.tmp", "/자료/"]);

        handle_key(&mut m, KeyCode::Char('+')); // move "/자료/" up
        assert_eq!(m.patterns, ["/자료/", "*.tmp"]);

        handle_key(&mut m, KeyCode::Char('e'));
        handle_key(&mut m, KeyCode::Backspace);
        handle_key(&mut m, KeyCode::Enter);
        assert_eq!(m.patterns, ["/자료", "*.tmp"]);

        handle_key(&mut m, KeyCode::Char('j'));
        handle_key(&mut m, KeyCode::Char('d'));
        assert_eq!(m.patterns, ["/자료"]);

        let result = handle_key(&mut m, KeyCode::Esc).unwrap();
        assert!(!m.show);
        assert_eq!(result.target, IgnoreTarget::Entry(0));
        assert_eq!(result.patterns, ["/자료"]);
    }

    #[test]
    fn invalid_pattern_keeps_editing_and_esc_cancels() {
        let mut m = modal(&[]);
        handle_key(&mut m, KeyCode::Char('a'));
        type_str(&mut m, "a{");
        handle_key(&mut m, KeyCode::Enter);
        assert!(m.edit.as_ref().is_some_and(|e| e.error.is_some()));
        assert!(m.patterns.is_empty());

        handle_key(&mut m, KeyCode::Esc); // cancel edit, modal stays open
        assert!(m.edit.is_none() && m.show);
    }

    #[test]
    fn empty_input_adds_nothing() {
        let mut m = modal(&[]);
        handle_key(&mut m, KeyCode::Char('a'));
        type_str(&mut m, "   ");
        handle_key(&mut m, KeyCode::Enter);
        assert!(m.patterns.is_empty() && m.edit.is_none());
    }
}
