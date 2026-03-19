use ratatui::prelude::*;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, BorderType, Paragraph};

use crate::state::input::{InputMode, InputState};
use crate::theme::ThemeTokens;

pub fn render(
    frame: &mut Frame,
    area: Rect,
    input: &InputState,
    theme: &ThemeTokens,
    status_line: &str,
    focused: bool,
) {
    let border_style = if focused {
        theme.accent_primary
    } else {
        theme.fg_dim
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_style);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height < 2 {
        return;
    }

    // Line 1: mode + input
    let mode_span = match input.mode() {
        InputMode::Normal => Span::styled("NORMAL", theme.fg_dim),
        InputMode::Insert => Span::styled("INSERT", theme.accent_primary),
    };
    let cursor = if input.mode() == InputMode::Insert {
        "\u{2588}"
    } else {
        ""
    };
    let input_line = Line::from(vec![
        Span::raw(" "),
        mode_span,
        Span::raw(" "),
        Span::styled("\u{25b6}", theme.accent_primary),
        Span::raw(" "),
        Span::raw(input.buffer()),
        Span::raw(cursor),
    ]);

    // Line 2: status or hints
    let line2 = if !status_line.is_empty() {
        Line::from(vec![
            Span::raw(" "),
            Span::styled(status_line, theme.accent_success),
        ])
    } else {
        Line::from(vec![
            Span::raw(" "),
            Span::styled("tab", theme.fg_active),
            Span::styled(":focus  ", theme.fg_dim),
            Span::styled("ctrl+p", theme.fg_active),
            Span::styled(":commands  ", theme.fg_dim),
            Span::styled("ctrl+t", theme.fg_active),
            Span::styled(":threads  ", theme.fg_dim),
            Span::styled("/", theme.fg_active),
            Span::styled(":slash  ", theme.fg_dim),
            Span::styled("q", theme.fg_active),
            Span::styled(":quit", theme.fg_dim),
        ])
    };

    let paragraph = Paragraph::new(vec![input_line, line2]);
    frame.render_widget(paragraph, inner);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn footer_handles_empty_state() {
        let input = InputState::new();
        let _theme = ThemeTokens::default();
        // InputState starts in Insert mode
        assert_eq!(input.mode(), InputMode::Insert);
    }
}
