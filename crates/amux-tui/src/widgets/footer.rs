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
    connected: bool,
    has_error: bool,
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

    // Line 2: status dots + keyboard hints
    let mut spans = vec![Span::raw(" ")];

    // Daemon connection status (always shown)
    if connected {
        spans.push(Span::styled("\u{25cf}", theme.accent_success)); // green ●
        spans.push(Span::styled(" daemon  ", theme.fg_dim));
    } else {
        spans.push(Span::styled("\u{25cf}", theme.accent_danger)); // red ●
        spans.push(Span::styled(" daemon  ", theme.fg_dim));
    }

    // Error indicator (shown only when there's an error)
    if has_error {
        spans.push(Span::styled("\u{25cf}", theme.accent_danger)); // red ●
        spans.push(Span::styled(" error  ", theme.fg_dim));
    }

    // Keyboard hints
    spans.push(Span::styled("tab", theme.fg_active));
    spans.push(Span::styled(":focus  ", theme.fg_dim));
    spans.push(Span::styled("ctrl+p", theme.fg_active));
    spans.push(Span::styled(":cmd  ", theme.fg_dim));
    spans.push(Span::styled("/", theme.fg_active));
    spans.push(Span::styled(":slash  ", theme.fg_dim));
    spans.push(Span::styled("q", theme.fg_active));
    spans.push(Span::styled(":quit", theme.fg_dim));

    let line2 = Line::from(spans);

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
        assert_eq!(input.mode(), InputMode::Insert);
    }
}
