use ratatui::prelude::*;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, BorderType, List, ListItem, Paragraph};

use crate::state::modal::ModalState;
use crate::theme::ThemeTokens;

pub fn render(
    frame: &mut Frame,
    area: Rect,
    modal: &ModalState,
    theme: &ThemeTokens,
) {
    let block = Block::default()
        .title(" COMMANDS ")
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(theme.accent_secondary);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height < 4 {
        return;
    }

    // Split: search input (1) + separator (1) + list (flex) + hints (1)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // search
            Constraint::Length(1), // separator
            Constraint::Min(1),   // list
            Constraint::Length(1), // hints
        ])
        .split(inner);

    // Search input
    let query = modal.command_query();
    let input_line = Line::from(vec![
        Span::raw(" "),
        Span::styled(
            if query.is_empty() { "/" } else { query },
            theme.fg_active,
        ),
        Span::raw("\u{2588}"),
    ]);
    frame.render_widget(Paragraph::new(input_line), chunks[0]);

    // Separator
    let sep = Line::from(Span::styled(
        "\u{2500}".repeat(chunks[1].width as usize),
        theme.fg_dim,
    ));
    frame.render_widget(Paragraph::new(sep), chunks[1]);

    // Command list
    let filtered = modal.filtered_items();
    let items = modal.command_items();
    let cursor = modal.picker_cursor();
    let list_h = chunks[2].height as usize;

    let list_items: Vec<ListItem> = (0..list_h)
        .map(|i| {
            if i < filtered.len() {
                let idx = filtered[i];
                let item = &items[idx];
                let is_selected = i == cursor;

                if is_selected {
                    ListItem::new(Line::from(vec![
                        Span::raw(" > /"),
                        Span::raw(&item.command),
                        Span::raw("  "),
                        Span::raw(&item.description),
                    ]))
                    .style(
                        Style::default()
                            .bg(Color::Indexed(178))
                            .fg(Color::Black),
                    )
                } else {
                    ListItem::new(Line::from(vec![
                        Span::raw("   /"),
                        Span::styled(&item.command, theme.fg_active),
                        Span::raw("  "),
                        Span::styled(&item.description, theme.fg_dim),
                    ]))
                }
            } else {
                ListItem::new(Line::raw(""))
            }
        })
        .collect();

    let list = List::new(list_items);
    frame.render_widget(list, chunks[2]);

    // Hints
    let hints = Line::from(vec![
        Span::raw(" "),
        Span::styled("j/k", theme.fg_active),
        Span::styled(" navigate  ", theme.fg_dim),
        Span::styled("Enter", theme.fg_active),
        Span::styled(" select  ", theme.fg_dim),
        Span::styled("Esc", theme.fg_active),
        Span::styled(" close", theme.fg_dim),
    ]);
    frame.render_widget(Paragraph::new(hints), chunks[3]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_palette_handles_empty_state() {
        let modal = ModalState::new();
        let _theme = ThemeTokens::default();
        assert!(modal.command_query().is_empty());
    }
}
