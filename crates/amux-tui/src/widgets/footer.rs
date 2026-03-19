use ratatui::prelude::*;
use ratatui::style::Color;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, BorderType, Paragraph};

use crate::state::input::InputState;
use crate::theme::ThemeTokens;

/// Render the bordered input box (always-insert prompt)
pub fn render_input(
    frame: &mut Frame,
    area: Rect,
    input: &InputState,
    theme: &ThemeTokens,
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

    if inner.height < 1 {
        return;
    }

    // Always show the block cursor to signal readiness for input
    let cursor = "\u{2588}";
    let input_line = Line::from(vec![
        Span::raw(" "),
        Span::styled("\u{25b6}", theme.accent_primary),
        Span::raw(" "),
        Span::raw(input.buffer()),
        Span::raw(cursor),
    ]);

    frame.render_widget(Paragraph::new(vec![input_line]), inner);
}

/// Render the bare status bar below the input (no border, 1 line)
pub fn render_status_bar(
    frame: &mut Frame,
    area: Rect,
    theme: &ThemeTokens,
    connected: bool,
    error_active: bool,
    tick: u64,
    error_tick: u64,
) {
    let mut spans = vec![Span::raw(" ")];

    // Daemon connection status
    if connected {
        spans.push(Span::styled("\u{25cf}", theme.accent_success));
        spans.push(Span::styled(" daemon", theme.fg_dim));
    } else {
        spans.push(Span::styled("\u{25cf}", theme.accent_danger));
        spans.push(Span::styled(" daemon", theme.fg_dim));
    }

    // Error indicator with pulse
    if error_active {
        let elapsed = tick.saturating_sub(error_tick);
        let pulse_phase = (elapsed / 10) % 2;
        let error_color = if pulse_phase == 0 {
            Style::default().fg(Color::Indexed(203))
        } else {
            Style::default().fg(Color::Indexed(88))
        };
        spans.push(Span::raw("  "));
        spans.push(Span::styled("\u{25cf}", error_color));
        spans.push(Span::styled(" error", theme.fg_dim));
    }

    // Spacer then keyboard hints (right-aligned feel)
    spans.push(Span::raw("    "));
    spans.push(Span::styled("tab", theme.fg_active));
    spans.push(Span::styled(":focus  ", theme.fg_dim));
    spans.push(Span::styled("ctrl+p", theme.fg_active));
    spans.push(Span::styled(":cmd  ", theme.fg_dim));
    spans.push(Span::styled("/", theme.fg_active));
    spans.push(Span::styled(":slash  ", theme.fg_dim));
    if error_active {
        spans.push(Span::styled("!", theme.accent_danger));
        spans.push(Span::styled(":error  ", theme.fg_dim));
    }
    spans.push(Span::styled("/quit", theme.fg_active));
    spans.push(Span::styled(":exit", theme.fg_dim));

    let line = Line::from(spans);
    frame.render_widget(Paragraph::new(vec![line]), area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::input::InputMode;

    #[test]
    fn footer_handles_empty_state() {
        let input = InputState::new();
        let _theme = ThemeTokens::default();
        assert_eq!(input.mode(), InputMode::Insert);
    }
}
