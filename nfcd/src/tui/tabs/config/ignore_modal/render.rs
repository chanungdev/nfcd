use crossterm::event::KeyCode;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Position},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, Paragraph, Wrap},
};

use crate::tui::app::events::MouseState;
use crate::tui::shortcuts::{ShortcutBlock, gap, shortcut, shortcut_bracketed, space};
use crate::tui::styles::{inactive_style, key_style, label_style};
use crate::tui::tabs::config::ignore_modal::state::{IgnoreModalState, IgnoreTarget};
use crate::tui::tabs::config::modal::modal_area;

const HELP: &str = "gitignore syntax: *  **  !negate  dir/  /anchored-to-path. Later patterns win.";

pub fn render_ignore_modal(modal: &mut IgnoreModalState, f: &mut Frame, mouse: &mut MouseState) {
    let full_area = f.area();
    f.render_widget(
        Block::default().style(Style::default().bg(Color::DarkGray)),
        full_area,
    );
    let area = modal_area(full_area);
    f.render_widget(Clear, area);
    f.render_widget(
        Block::default().style(Style::default().bg(Color::Black).fg(Color::White)),
        area,
    );

    let mut items: Vec<(Vec<Span>, Option<KeyCode>)> = vec![space()];
    if modal.edit.is_some() {
        items.extend([
            shortcut_bracketed("↵", "Apply", KeyCode::Enter),
            gap(),
            shortcut_bracketed("⎋", "Cancel", KeyCode::Esc),
        ]);
    } else {
        items.extend([
            shortcut("A", "dd", KeyCode::Char('a')),
            gap(),
            shortcut("E", "dit", KeyCode::Char('e')),
            gap(),
            shortcut("D", "elete", KeyCode::Char('d')),
            gap(),
            (vec![Span::styled("[", label_style())], None),
            (
                vec![Span::styled("+", key_style())],
                Some(KeyCode::Char('+')),
            ),
            (
                vec![Span::styled("-", key_style())],
                Some(KeyCode::Char('-')),
            ),
            (vec![Span::styled("]Move", label_style())], None),
            gap(),
            shortcut_bracketed("⎋", "Done", KeyCode::Esc),
        ]);
    }
    items.push(space());

    let title = format!(" Ignore patterns: {} ", modal.title);
    let inner = ShortcutBlock::new(Line::from(Span::styled(
        title,
        Style::default().fg(Color::White),
    )))
    .items(items)
    .render(f, area, mouse);

    let scope = match modal.target {
        IgnoreTarget::Global => "Applied to every watch path, before each path's own patterns.",
        IgnoreTarget::Entry(_) => "Relative to this path. Global patterns also apply.",
    };
    let chunks = Layout::vertical([
        Constraint::Length(2), // help
        Constraint::Min(3),    // pattern list
        Constraint::Length(if modal.edit.is_some() { 3 } else { 0 }),
    ])
    .split(inner);

    f.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(scope, inactive_style())),
            Line::from(Span::styled(HELP, inactive_style())),
        ])
        .wrap(Wrap { trim: false }),
        chunks[0],
    );

    let list_items: Vec<ListItem> = if modal.patterns.is_empty() {
        vec![ListItem::new(Span::styled(
            " (no patterns)",
            inactive_style(),
        ))]
    } else {
        modal
            .patterns
            .iter()
            .enumerate()
            .map(|(i, p)| ListItem::new(format!(" {:>2}  {}", i + 1, p)))
            .collect()
    };
    let list = List::new(list_items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(" Patterns "),
        )
        .highlight_style(Style::default().bg(Color::DarkGray));
    f.render_stateful_widget(list, chunks[1], &mut modal.list_state);

    if let Some(edit) = &modal.edit {
        let (title, color) = match (&edit.error, edit.index) {
            (Some(e), _) => (format!(" Invalid: {e} "), Color::Red),
            (None, Some(i)) => (format!(" Edit pattern #{} ", i + 1), Color::Cyan),
            (None, None) => (" New pattern ".to_string(), Color::Cyan),
        };
        let input_block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(color))
            .title(title);
        let input_area = chunks[2];
        let text_area = input_block.inner(input_area);
        f.render_widget(
            Paragraph::new(edit.input.value()).block(input_block),
            input_area,
        );
        let x = (text_area.x + edit.input.cursor_width()).min(text_area.right().saturating_sub(1));
        f.set_cursor_position(Position::new(x, text_area.y));
    }
}
