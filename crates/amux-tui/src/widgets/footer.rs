use ratatui::prelude::*;
use ratatui::style::Color;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, BorderType, Paragraph, Wrap};

use crate::app::Attachment;
use crate::state::input::InputState;
use crate::theme::ThemeTokens;

/// Render the bordered input box (always-insert prompt)
pub fn render_input(
    frame: &mut Frame,
    area: Rect,
    input: &InputState,
    theme: &ThemeTokens,
    focused: bool,
    modal_open: bool,
    attachments: &[Attachment],
    tick: u64,
) {
    let border_style = if modal_open {
        theme.fg_dim
    } else if focused {
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

    if modal_open {
        // When a modal is open, show dimmed hint instead of actual input
        let input_line = Line::from(vec![
            Span::raw(" "),
            Span::styled("\u{25b6}", theme.fg_dim),
            Span::styled(" (modal open)", theme.fg_dim),
        ]);
        frame.render_widget(Paragraph::new(vec![input_line]), inner);
    } else {
        let cursor_char = "\u{2588}";
        let buf = input.buffer();
        let cursor_pos = input.cursor_pos();
        let mut lines: Vec<Line> = Vec::new();

        // Show attachments if any
        for att in attachments {
            let size_str = if att.size_bytes > 1024 {
                format!("{:.1} KB", att.size_bytes as f64 / 1024.0)
            } else {
                format!("{} B", att.size_bytes)
            };
            lines.push(Line::from(vec![
                Span::raw(" "),
                Span::styled("\u{1f4ce} ", theme.accent_secondary),
                Span::styled(att.filename.clone(), theme.fg_active),
                Span::styled(format!(" ({})", size_str), theme.fg_dim),
            ]));
        }

        // Animated placeholder when buffer is empty
        if buf.is_empty() && attachments.is_empty() {
            let placeholders = [
                "Ask anything... plan \u{00b7} solve \u{00b7} ship",
                "Try: /settings to configure your AI",
                "Shift+Enter for multi-line input",
                "/attach <file> to include context",
                "Ctrl+P for command palette",
                "/help to see all keyboard shortcuts",
                "Paste a file path to auto-attach it",
                "Ctrl+Z to undo, Ctrl+Y to redo",
                "What would you like to build today?",
                "Describe a bug and I'll investigate",
                "Ask me to explain any code file",
            ];
            // Cycle through placeholders every ~4 seconds (80 ticks at 50ms)
            let placeholder_idx = ((tick / 80) as usize) % placeholders.len();
            let placeholder = placeholders[placeholder_idx];

            // Typing animation: reveal chars progressively within the 4-second window
            let ticks_in_cycle = (tick % 80) as usize;
            let chars_to_show = (ticks_in_cycle * placeholder.chars().count() / 40)
                .min(placeholder.chars().count()); // fully revealed at tick 40 of 80

            let visible: String = placeholder.chars().take(chars_to_show).collect();
            let cursor_blink = if (tick / 10) % 2 == 0 { "\u{2588}" } else { " " };

            let dim_style = Style::default().fg(Color::Indexed(239));
            lines.push(Line::from(vec![
                Span::raw(" "),
                Span::styled("\u{25b6}", dim_style),
                Span::raw(" "),
                Span::styled(visible, dim_style),
                Span::styled(cursor_blink, dim_style),
            ]));

            let visible_height = inner.height as usize;
            let scroll_offset = if lines.len() > visible_height {
                (lines.len() - visible_height) as u16
            } else {
                0
            };
            frame.render_widget(
                Paragraph::new(lines)
                    .wrap(ratatui::widgets::Wrap { trim: false })
                    .scroll((scroll_offset, 0)),
                inner,
            );
            return;
        }

        // Build display string with cursor inserted at the correct position
        let before_cursor = &buf[..cursor_pos];
        let after_cursor = &buf[cursor_pos..];
        let mut display = String::with_capacity(buf.len() + cursor_char.len());
        display.push_str(before_cursor);
        display.push_str(cursor_char);
        display.push_str(after_cursor);

        let raw_lines: Vec<&str> = display.split('\n').collect();

        for (i, line_text) in raw_lines.iter().enumerate() {
            let is_first = i == 0;

            let mut spans = Vec::new();
            if is_first {
                spans.push(Span::raw(" "));
                spans.push(Span::styled("\u{25b6}", theme.accent_primary));
                spans.push(Span::raw(" "));
            } else {
                spans.push(Span::raw("   "));
            }
            spans.push(Span::raw(line_text.to_string()));
            lines.push(Line::from(spans));
        }

        // Scroll to keep cursor line visible
        let (cursor_line, _) = input.cursor_line_col_public();
        let attachment_lines = attachments.len();
        let cursor_display_line = attachment_lines + cursor_line;
        let visible_height = inner.height as usize;
        let scroll_offset = if cursor_display_line >= visible_height {
            (cursor_display_line - visible_height + 1) as u16
        } else if lines.len() > visible_height {
            // Default: show last lines but prefer cursor visibility
            (lines.len() - visible_height) as u16
        } else {
            0
        };

        frame.render_widget(
            Paragraph::new(lines)
                .wrap(Wrap { trim: false })
                .scroll((scroll_offset, 0)),
            inner,
        );
    }
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
    queued_count: usize,
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

    // Queued messages indicator
    if queued_count > 0 {
        spans.push(Span::raw("  "));
        spans.push(Span::styled("\u{25cf}", theme.accent_secondary));
        spans.push(Span::styled(format!(" queued({})", queued_count), theme.fg_dim));
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
