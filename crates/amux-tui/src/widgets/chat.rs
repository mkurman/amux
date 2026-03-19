use crate::theme::{ThemeTokens, ROUNDED_BORDER, RESET};
use crate::state::chat::ChatState;

/// Render the chat pane (left pane in two-pane layout)
pub fn chat_widget(
    chat: &ChatState,
    theme: &ThemeTokens,
    focused: bool,
    width: usize,
    height: usize,
) -> Vec<String> {
    let border_color = if focused { theme.accent_primary } else { theme.fg_dim };
    let bc = border_color.fg();
    let b = &ROUNDED_BORDER;
    let inner_width = width.saturating_sub(2);
    let inner_height = height.saturating_sub(2); // top and bottom borders

    let mut result = Vec::new();

    // Title
    let title = " Conversation ";
    let title_colored = if focused {
        format!("{}{}{}", theme.fg_active.fg(), title, bc)
    } else {
        format!("{}{}{}", theme.fg_dim.fg(), title, bc)
    };
    let title_len = title.len();
    let remaining = inner_width.saturating_sub(title_len);
    result.push(format!(
        "{}{}{}{}{}{}{}",
        bc, b.top_left,
        super::repeat_char(b.horizontal, 2),
        title_colored,
        super::repeat_char(b.horizontal, remaining.saturating_sub(2)),
        b.top_right,
        RESET,
    ));

    // Content area
    let content_lines = if chat.active_thread().is_none() && chat.streaming_content().is_empty() {
        // Delegate to splash widget when no thread is active
        crate::widgets::splash::splash_widget(theme, inner_width, inner_height)
    } else {
        render_messages(chat, theme, inner_width, inner_height)
    };

    for line in &content_lines {
        let padded = super::pad_to_width(line, inner_width);
        result.push(format!("{}{}{}{}{}", bc, b.vertical, padded, b.vertical, RESET));
    }

    // Bottom border
    result.push(format!(
        "{}{}{}{}{}",
        bc, b.bottom_left,
        super::repeat_char(b.horizontal, inner_width),
        b.bottom_right,
        RESET,
    ));

    result
}

fn render_messages(
    chat: &ChatState,
    theme: &ThemeTokens,
    width: usize,
    height: usize,
) -> Vec<String> {
    let mode = chat.transcript_mode();

    // Collect all message lines
    let mut all_lines: Vec<String> = Vec::new();

    if let Some(thread) = chat.active_thread() {
        for msg in &thread.messages {
            let msg_lines = super::message::message_widget(msg, mode, theme, width);
            all_lines.extend(msg_lines);
            // Add spacing between messages
            if !all_lines.is_empty() {
                all_lines.push(String::new());
            }
        }
    }

    // Append streaming content if present
    if !chat.streaming_content().is_empty() {
        let streaming_line = format!(
            "  {}ASST{} {}{}\u{2588}{}",
            theme.accent_assistant.bg(),
            RESET,
            theme.fg_active.fg(),
            chat.streaming_content(),
            RESET,
        );
        all_lines.push(streaming_line);
    }

    // Apply scroll offset (0 = following tail = show last `height` lines)
    let scroll = chat.scroll_offset();
    let total = all_lines.len();

    if total <= height {
        // All lines fit — pad at top to push content to bottom
        let mut padded = all_lines;
        while padded.len() < height {
            padded.insert(0, String::new());
        }
        padded
    } else if scroll == 0 {
        // Following tail: show last `height` lines
        all_lines[total - height..].to_vec()
    } else {
        // Scrolled up: show lines from (end - height - scroll) to (end - scroll)
        let end = total.saturating_sub(scroll);
        let start = end.saturating_sub(height);
        all_lines[start..end].to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::ThemeTokens;

    #[test]
    fn chat_widget_returns_correct_height() {
        let chat = ChatState::new();
        let theme = ThemeTokens::default();
        let lines = chat_widget(&chat, &theme, true, 60, 20);
        assert_eq!(lines.len(), 20);
    }

    #[test]
    fn chat_widget_has_borders() {
        let chat = ChatState::new();
        let theme = ThemeTokens::default();
        let lines = chat_widget(&chat, &theme, true, 60, 20);
        // First line should contain top-left border char
        assert!(lines[0].contains('\u{256d}')); // ╭
        // Last line should contain bottom-left border char
        assert!(lines.last().unwrap().contains('\u{2570}')); // ╰
    }

    #[test]
    fn chat_widget_contains_title() {
        let chat = ChatState::new();
        let theme = ThemeTokens::default();
        let lines = chat_widget(&chat, &theme, true, 60, 20);
        assert!(lines[0].contains("Conversation"));
    }

    #[test]
    fn chat_widget_shows_splash_when_no_thread() {
        let chat = ChatState::new();
        let theme = ThemeTokens::default();
        let lines = chat_widget(&chat, &theme, false, 60, 20);
        // Should have correct height
        assert_eq!(lines.len(), 20);
        // Should show some content (splash) rather than empty
        let joined = lines.join("");
        // Splash includes TAMUX text
        assert!(joined.contains("T A M U X"));
    }

    #[test]
    fn chat_widget_unfocused_uses_dim_border() {
        let chat = ChatState::new();
        let theme = ThemeTokens::default();
        let focused_lines = chat_widget(&chat, &theme, true, 60, 20);
        let unfocused_lines = chat_widget(&chat, &theme, false, 60, 20);
        // Both should have the same number of lines
        assert_eq!(focused_lines.len(), unfocused_lines.len());
        // But the first lines (borders) should differ in color
        assert_ne!(focused_lines[0], unfocused_lines[0]);
    }

    #[test]
    fn chat_widget_minimum_size() {
        let chat = ChatState::new();
        let theme = ThemeTokens::default();
        // Very small dimensions should not panic
        let lines = chat_widget(&chat, &theme, true, 4, 3);
        assert_eq!(lines.len(), 3);
    }
}
