use ratatui::prelude::*;
use ratatui::style::Color;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, BorderType, Paragraph, Wrap};

use crate::app::Attachment;
use crate::state::input::{InputState, PasteBlock};
use crate::theme::ThemeTokens;

/// Parse `text` for `\x00PASTE:N\x00` tokens and return a list of styled
/// spans, rendering paste blocks as amber summary labels.
fn render_buffer_with_paste_blocks<'a>(
    text: &str,
    paste_blocks: &[PasteBlock],
    theme: &ThemeTokens,
) -> Vec<Span<'a>> {
    let mut spans: Vec<Span<'a>> = Vec::new();
    let mut remaining = text;
    let amber = Style::default().fg(Color::Indexed(178));

    while let Some(start) = remaining.find("\x00PASTE:") {
        // Text before the placeholder
        if start > 0 {
            spans.push(Span::styled(remaining[..start].to_string(), theme.fg_active));
        }

        // Find the closing NUL after "PASTE:N"
        let inner = &remaining[start + 1..]; // skip leading NUL
        if let Some(end_offset) = inner.find('\x00') {
            let tag = &inner[..end_offset]; // e.g. "PASTE:3"
            if let Some(id_str) = tag.strip_prefix("PASTE:") {
                if let Ok(id) = id_str.parse::<usize>() {
                    if let Some(label) = InputState::paste_block_display(id, paste_blocks) {
                        spans.push(Span::styled(label, amber));
                    }
                }
            }
            remaining = &remaining[start + 1 + end_offset + 1..];
        } else {
            // Malformed token — emit the rest verbatim
            spans.push(Span::styled(remaining[start..].to_string(), theme.fg_active));
            return spans;
        }
    }

    // Trailing text after the last placeholder (or the whole string when no tokens)
    if !remaining.is_empty() {
        spans.push(Span::styled(remaining.to_string(), theme.fg_active));
    }

    spans
}

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

        // Build display string with cursor inserted at the correct position.
        // Note: paste-block placeholders (\x00PASTE:N\x00) must not be split
        // across the cursor insertion point, so we work with the raw buffer
        // (no '\n' inside placeholders) and just inject the cursor glyph.
        let before_cursor = &buf[..cursor_pos];
        let after_cursor = &buf[cursor_pos..];
        let mut display = String::with_capacity(buf.len() + cursor_char.len());
        display.push_str(before_cursor);
        display.push_str(cursor_char);
        display.push_str(after_cursor);

        // Split only on real newlines (placeholders don't contain '\n')
        let raw_lines: Vec<&str> = display.split('\n').collect();
        let paste_blocks = input.paste_blocks();

        for (i, line_text) in raw_lines.iter().enumerate() {
            let is_first = i == 0;

            let mut spans: Vec<Span<'_>> = Vec::new();
            if is_first {
                spans.push(Span::raw(" "));
                spans.push(Span::styled("\u{25b6}", theme.accent_primary));
                spans.push(Span::raw(" "));
            } else {
                spans.push(Span::raw("   "));
            }
            // Render paste-block placeholders as styled amber labels
            spans.extend(render_buffer_with_paste_blocks(line_text, paste_blocks, theme));
            lines.push(Line::from(spans));
        }

        // Calculate visual line of cursor (accounting for wrapping)
        // The cursor is embedded in the display string — find which Line it's in
        let cursor_visual_line = {
            let mut found = attachments.len(); // start after attachment lines
            let mut char_count = 0;
            let cursor_in_display = input.cursor_pos(); // byte offset where █ was inserted
            for (i, line_text) in raw_lines.iter().enumerate() {
                let line_chars = line_text.chars().count();
                if char_count + line_chars >= cursor_in_display || i == raw_lines.len() - 1 {
                    found = attachments.len() + i;
                    break;
                }
                char_count += line_chars + 1; // +1 for \n
            }
            found
        };

        let visible_height = inner.height as usize;
        let total_lines = lines.len();
        let scroll_offset = if visible_height == 0 || total_lines <= visible_height {
            0u16
        } else if cursor_visual_line >= visible_height {
            // Cursor is below visible area — scroll to show it
            (cursor_visual_line - visible_height + 1).min(total_lines - visible_height) as u16
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
