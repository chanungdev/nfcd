use crossterm::event::KeyCode;
use unicode_width::UnicodeWidthChar;

/// Single-line text input. `cursor` is a char index, so multi-byte input
/// (e.g. Korean from the terminal IME) is edited per character.
///
/// ponytail: no horizontal scrolling; text wider than the field is clipped.
/// Fine for short ignore patterns, add scrolling if longer inputs appear.
#[derive(Debug, Clone, Default)]
pub struct TextInput {
    value: String,
    cursor: usize,
}

impl TextInput {
    pub fn new(value: &str) -> Self {
        Self {
            value: value.to_string(),
            cursor: value.chars().count(),
        }
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    fn byte_index(&self, char_idx: usize) -> usize {
        self.value
            .char_indices()
            .nth(char_idx)
            .map_or(self.value.len(), |(i, _)| i)
    }

    /// Apply an editing key. Returns false for keys the input doesn't handle.
    pub fn handle_key(&mut self, key: KeyCode) -> bool {
        let len = self.value.chars().count();
        match key {
            KeyCode::Char(c) => {
                let i = self.byte_index(self.cursor);
                self.value.insert(i, c);
                self.cursor += 1;
            }
            KeyCode::Backspace if self.cursor > 0 => {
                self.cursor -= 1;
                let i = self.byte_index(self.cursor);
                self.value.remove(i);
            }
            KeyCode::Delete if self.cursor < len => {
                let i = self.byte_index(self.cursor);
                self.value.remove(i);
            }
            KeyCode::Left => self.cursor = self.cursor.saturating_sub(1),
            KeyCode::Right => self.cursor = (self.cursor + 1).min(len),
            KeyCode::Home => self.cursor = 0,
            KeyCode::End => self.cursor = len,
            KeyCode::Backspace | KeyCode::Delete => {}
            _ => return false,
        }
        true
    }

    /// Display width (terminal columns) before the cursor, for placing the cursor.
    pub fn cursor_width(&self) -> u16 {
        self.value
            .chars()
            .take(self.cursor)
            .map(|c| c.width().unwrap_or(0) as u16)
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typed(s: &str) -> TextInput {
        let mut input = TextInput::default();
        for c in s.chars() {
            input.handle_key(KeyCode::Char(c));
        }
        input
    }

    #[test]
    fn edits_korean_per_character() {
        let mut input = typed("/자료/");
        assert_eq!(input.cursor_width(), 6); // 1 + 2 + 2 + 1
        input.handle_key(KeyCode::Left);
        input.handle_key(KeyCode::Backspace);
        assert_eq!(input.value(), "/자/");
        assert_eq!(input.cursor_width(), 3);
    }

    #[test]
    fn insert_and_delete_in_middle() {
        let mut input = typed("ac");
        input.handle_key(KeyCode::Left);
        input.handle_key(KeyCode::Char('b'));
        assert_eq!(input.value(), "abc");
        input.handle_key(KeyCode::Home);
        input.handle_key(KeyCode::Delete);
        assert_eq!(input.value(), "bc");
    }

    #[test]
    fn bounds_are_safe() {
        let mut input = TextInput::new("x");
        input.handle_key(KeyCode::Delete); // cursor at end
        input.handle_key(KeyCode::Right);
        assert_eq!((input.value(), input.cursor_width()), ("x", 1));
        input.handle_key(KeyCode::Home);
        input.handle_key(KeyCode::Backspace);
        input.handle_key(KeyCode::Left);
        assert_eq!((input.value(), input.cursor_width()), ("x", 0));
        assert!(!input.handle_key(KeyCode::Enter));
    }
}
