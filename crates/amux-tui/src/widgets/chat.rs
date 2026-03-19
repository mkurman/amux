use ratatui::prelude::*;
use ratatui::style::{Color, Style};
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

    if let Some(thread) = chat.active_thread() {
        for msg in &thread.messages {
            let msg_lines = super::message::message_to_lines(msg, mode, theme, inner_width);
            all_lines.extend(msg_lines);
            // Add spacing between messages
            if !all_lines.is_empty() {
                all_lines.push(Line::raw(""));
            }
        }
    }

    // Append streaming content if present
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
    let scroll = chat.scroll_offset();
    let total = all_lines.len();

    let visible_lines = if total <= inner_height {
        // All lines fit -- pad at top to push content to bottom
        let mut padded = Vec::new();
        for _ in 0..(inner_height.saturating_sub(total)) {
            padded.push(Line::raw(""));
        }
        padded.extend(all_lines);
        padded
    } else if scroll == 0 {
        // Following tail: show last `height` lines
        all_lines[total - inner_height..].to_vec()
    } else {
        // Scrolled up: show lines from (end - height - scroll) to (end - scroll)
        let end = total.saturating_sub(scroll);
        let start = end.saturating_sub(inner_height);
        all_lines[start..end].to_vec()
    };

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
