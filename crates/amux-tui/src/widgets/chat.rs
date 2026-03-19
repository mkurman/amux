use ratatui::prelude::*;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, BorderType, Paragraph};

use crate::state::chat::ChatState;
use crate::theme::ThemeTokens;

pub fn render(
    frame: &mut Frame,
    area: Rect,
    chat: &ChatState,
    theme: &ThemeTokens,
    focused: bool,
) {
    let border_style = if focused {
        theme.accent_primary
    } else {
        theme.fg_dim
    };
    let title_style = if focused {
        theme.fg_active
    } else {
        theme.fg_dim
    };
    let block = Block::default()
        .title(Span::styled(" Conversation ", title_style))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_style);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if chat.active_thread().is_none() && chat.streaming_content().is_empty() {
        // Render splash
        super::splash::render(frame, inner, theme);
        return;
    }

    // Render messages
    let mode = chat.transcript_mode();
    let inner_width = inner.width as usize;
    let inner_height = inner.height as usize;

    let mut all_lines: Vec<Line> = Vec::new();
    // Track which line indices belong to which message (for selected highlight)
    let mut message_line_ranges: Vec<(usize, usize)> = Vec::new(); // (start, end) in all_lines

    let expanded = chat.expanded_reasoning();
    let expanded_tools = chat.expanded_tools();
    let selected_msg = chat.selected_message();
    if let Some(thread) = chat.active_thread() {
        for (idx, msg) in thread.messages.iter().enumerate() {
            let start = all_lines.len();
            let msg_lines = super::message::message_to_lines(msg, idx, mode, theme, inner_width, expanded, expanded_tools);
            all_lines.extend(msg_lines);
            let end = all_lines.len();
            message_line_ranges.push((start, end));
            if !all_lines.is_empty() {
                all_lines.push(Line::raw(""));
            }
        }
    }

    // Apply selection highlight
    if let Some(sel_idx) = selected_msg {
        if let Some(&(start, end)) = message_line_ranges.get(sel_idx) {
            let sel_style = Style::default()
                .bg(Color::Indexed(236))
                .add_modifier(Modifier::empty());
            for line_idx in start..end {
                if let Some(line) = all_lines.get_mut(line_idx) {
                    // Prepend selection marker on first line, indent on continuation
                    if line_idx == start {
                        let mut new_spans = vec![Span::styled("> ", Style::default().fg(Color::Indexed(178)))];
                        new_spans.extend(line.spans.iter().cloned());
                        *line = Line::from(new_spans).style(sel_style);
                    } else {
                        *line = line.clone().style(sel_style);
                    }
                }
            }
        }
    }

    // Append streaming reasoning FIRST (thinking happens before tool calls)
    if !chat.streaming_reasoning().is_empty() {
        all_lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled("\u{25be} Reasoning...", theme.fg_dim),
        ]));
        for reasoning_line in chat.streaming_reasoning().lines() {
            all_lines.push(Line::from(vec![
                Span::styled("  \u{2502} ", Style::default().fg(Color::Indexed(24))),
                Span::styled(reasoning_line, theme.fg_dim),
            ]));
        }
    }

    // Append active tool calls (after reasoning, before final response)
    for tc in chat.active_tool_calls() {
        let status_span = match tc.status {
            crate::state::chat::ToolCallStatus::Running => {
                Span::styled(" \u{25cf} running", theme.accent_secondary)
            }
            crate::state::chat::ToolCallStatus::Done => {
                Span::styled(" \u{2713} done", theme.accent_success)
            }
            crate::state::chat::ToolCallStatus::Error => {
                Span::styled(" \u{2717} error", theme.accent_danger)
            }
        };
        all_lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled("\u{2699}", theme.accent_assistant),
            Span::raw(" "),
            Span::styled(&tc.name, theme.fg_active),
            status_span,
        ]));
    }

    // Append streaming content last (final response)
    if !chat.streaming_content().is_empty() {
        all_lines.push(Line::from(vec![
            Span::styled(
                " ASST ",
                Style::default()
                    .bg(Color::Indexed(183))
                    .fg(Color::Black),
            ),
            Span::raw(" "),
            Span::styled(chat.streaming_content(), theme.fg_active),
            Span::raw("\u{2588}"),
        ]));
    }

    // Apply scroll offset (0 = following tail = show last `height` lines)
    // Clamp scroll to prevent overscroll past the top of content
    let total = all_lines.len();
    let max_scroll = total.saturating_sub(inner_height);

    // Auto-scroll to keep selected message in view
    let mut scroll = chat.scroll_offset().min(max_scroll);
    if let Some(sel_idx) = selected_msg {
        if let Some(&(sel_start, sel_end)) = message_line_ranges.get(sel_idx) {
            // Visible window: when scroll=0, window is [total-inner_height, total)
            // When scroll=N, window is [total-inner_height-N, total-N)
            let window_end = total.saturating_sub(scroll);
            let window_start = window_end.saturating_sub(inner_height);

            if sel_start < window_start {
                // Selected message is above the visible window -- scroll up
                scroll = total.saturating_sub(sel_start + inner_height).min(max_scroll);
            } else if sel_end > window_end {
                // Selected message is below the visible window -- scroll down
                scroll = total.saturating_sub(sel_end).min(max_scroll);
            }
        }
    }

    let visible_lines = if total <= inner_height {
        // All lines fit -- pad at top to push content to bottom
        let mut padded = Vec::new();
        for _ in 0..(inner_height.saturating_sub(total)) {
            padded.push(Line::raw(""));
        }
        padded.extend(all_lines);
        padded
    } else if scroll == 0 {
        // Following tail: show last `inner_height` lines, no top padding
        all_lines[total - inner_height..].to_vec()
    } else {
        // Scrolled up: show lines from (end - height - scroll) to (end - scroll)
        let end = total.saturating_sub(scroll);
        let start = end.saturating_sub(inner_height);
        all_lines[start..end].to_vec()
    };

    // Do not use .wrap() -- lines are already individual Line objects; wrapping
    // would re-wrap the manually-sliced visible lines and cause double-wrapping.
    let paragraph = Paragraph::new(visible_lines);
    frame.render_widget(paragraph, inner);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_handles_empty_state() {
        let chat = ChatState::new();
        assert!(chat.active_thread().is_none());
        assert!(chat.streaming_content().is_empty());
    }
}
