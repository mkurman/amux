use crate::theme::{ThemeTokens, RESET};
use crate::state::chat::{AgentMessage, MessageRole, TranscriptMode};

/// Render a single message as lines
pub fn message_widget(
    msg: &AgentMessage,
    mode: TranscriptMode,
    theme: &ThemeTokens,
    width: usize,
) -> Vec<String> {
    let mut lines = Vec::new();
    let indent = 7; // "  XXXX " = 7 chars for role badge + spacing

    match mode {
        TranscriptMode::Compact => render_compact(msg, theme, width, indent, &mut lines),
        TranscriptMode::Tools => render_tools_only(msg, theme, width, indent, &mut lines),
        TranscriptMode::Full => render_full(msg, theme, width, indent, &mut lines),
    }

    lines
}

fn render_compact(msg: &AgentMessage, theme: &ThemeTokens, width: usize, indent: usize, lines: &mut Vec<String>) {
    let content_width = width.saturating_sub(indent + 1);

    // Role badge
    let (badge, badge_color) = role_badge(msg.role, theme);

    // Skip empty tool messages in compact mode — show tool status one-liner
    if msg.role == MessageRole::Tool && msg.content.is_empty() {
        if let Some(name) = &msg.tool_name {
            let status = msg.tool_status.as_deref().unwrap_or("running");
            let status_colored = format_tool_status(status, theme);
            lines.push(format!(
                "  {}\u{2699}{} {}{} {}",
                theme.accent_assistant.fg(), RESET,
                theme.fg_dim.fg(), name,
                status_colored,
            ));
        }
        return;
    }

    // First line: badge + first line of content
    let content = &msg.content;
    if content.is_empty() && msg.role != MessageRole::Tool {
        return;
    }

    let content_lines = wrap_text(content, content_width);

    if let Some(first) = content_lines.first() {
        lines.push(format!(
            "  {}{}{} {}{}{}",
            badge_color.bg(), badge, RESET,
            theme.fg_active.fg(), first, RESET,
        ));
    }

    // Continuation lines with indent
    for line in content_lines.iter().skip(1) {
        lines.push(format!(
            "{}{}{}",
            " ".repeat(indent),
            theme.fg_active.fg(),
            line,
        ));
    }

    // Tool calls inline (compact: single merged line)
    if msg.role == MessageRole::Assistant {
        if let Some(name) = &msg.tool_name {
            let status = msg.tool_status.as_deref().unwrap_or("running");
            let status_colored = format_tool_status(status, theme);
            lines.push(format!(
                "{}{}\u{2699}{} {}{} {}{}",
                " ".repeat(indent),
                theme.accent_assistant.fg(), RESET,
                theme.fg_dim.fg(), name,
                status_colored,
                RESET,
            ));
        }
    }
}

fn render_tools_only(msg: &AgentMessage, theme: &ThemeTokens, width: usize, _indent: usize, lines: &mut Vec<String>) {
    // Only show tool-related messages
    if msg.role != MessageRole::Tool && msg.tool_name.is_none() {
        return;
    }

    if let Some(name) = &msg.tool_name {
        let status = msg.tool_status.as_deref().unwrap_or("running");
        let status_colored = format_tool_status(status, theme);
        let args_preview = msg.tool_arguments.as_deref().unwrap_or("");
        let max_args = width.saturating_sub(30);
        let args_short = if args_preview.len() > max_args {
            &args_preview[..max_args]
        } else {
            args_preview
        };

        lines.push(format!(
            "  {}\u{2699}{} {}{:<16}{} {}{}{}",
            theme.accent_assistant.fg(), RESET,
            theme.fg_active.fg(), name, RESET,
            status_colored,
            if !args_short.is_empty() { format!("  {}{}{}", theme.fg_dim.fg(), args_short, RESET) } else { String::new() },
            RESET,
        ));
    }
}

fn render_full(msg: &AgentMessage, theme: &ThemeTokens, width: usize, indent: usize, lines: &mut Vec<String>) {
    // Full mode: render everything including reasoning
    render_compact(msg, theme, width, indent, lines);

    // Show reasoning if present
    if let Some(reasoning) = &msg.reasoning {
        if !reasoning.is_empty() {
            lines.push(format!(
                "{}{}\u{25be} [-] Reasoning{}",
                " ".repeat(indent),
                theme.fg_dim.fg(),
                RESET,
            ));
            let reasoning_width = width.saturating_sub(indent + 2);
            for line in wrap_text(reasoning, reasoning_width) {
                lines.push(format!(
                    "{}{}\u{2502}{} {}{}{}",
                    " ".repeat(indent),
                    "\x1b[38;5;24m", // dark blue border
                    RESET,
                    theme.fg_dim.fg(),
                    line,
                    RESET,
                ));
            }
        }
    }
}

fn role_badge(role: MessageRole, theme: &ThemeTokens) -> (&'static str, crate::theme::Color) {
    match role {
        MessageRole::User => ("USER", theme.accent_primary),
        MessageRole::Assistant => ("ASST", theme.accent_assistant),
        MessageRole::System => ("SYS ", theme.fg_dim),
        MessageRole::Tool => ("TOOL", theme.accent_assistant),
        MessageRole::Unknown => ("??? ", theme.fg_dim),
    }
}

fn format_tool_status(status: &str, theme: &ThemeTokens) -> String {
    match status {
        "completed" | "done" | "success" => format!("{}\u{2713} done{}", theme.accent_success.fg(), RESET),
        "error" | "failed" => format!("{}\u{2717} error{}", theme.accent_danger.fg(), RESET),
        _ => format!("{}\u{28cb} running{}", theme.accent_secondary.fg(), RESET),
    }
}

/// Word-wrap text to fit within a given width
fn wrap_text(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![text.to_string()];
    }
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        if paragraph.is_empty() {
            lines.push(String::new());
            continue;
        }
        let mut current_line = String::new();
        for word in paragraph.split_whitespace() {
            if current_line.is_empty() {
                current_line = word.to_string();
            } else if current_line.len() + 1 + word.len() <= width {
                current_line.push(' ');
                current_line.push_str(word);
            } else {
                lines.push(current_line);
                current_line = word.to_string();
            }
        }
        if !current_line.is_empty() {
            lines.push(current_line);
        }
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_text_basic() {
        let lines = wrap_text("hello world foo bar", 12);
        assert_eq!(lines, vec!["hello world", "foo bar"]);
    }

    #[test]
    fn wrap_text_preserves_newlines() {
        let lines = wrap_text("line1\nline2", 80);
        assert_eq!(lines, vec!["line1", "line2"]);
    }

    #[test]
    fn user_message_has_badge() {
        let msg = AgentMessage {
            role: MessageRole::User,
            content: "Hello".into(),
            ..Default::default()
        };
        let lines = message_widget(&msg, TranscriptMode::Compact, &ThemeTokens::default(), 80);
        assert!(!lines.is_empty());
        // Should contain USER badge (visible in the ANSI output)
        let joined = lines.join("");
        assert!(joined.contains("USER"));
    }

    #[test]
    fn tool_message_shows_gear_icon() {
        let msg = AgentMessage {
            role: MessageRole::Tool,
            tool_name: Some("bash_command".into()),
            tool_status: Some("done".into()),
            ..Default::default()
        };
        let lines = message_widget(&msg, TranscriptMode::Compact, &ThemeTokens::default(), 80);
        let joined = lines.join("");
        assert!(joined.contains('\u{2699}'));
        assert!(joined.contains("bash_command"));
    }

    #[test]
    fn tools_mode_skips_non_tool_messages() {
        let msg = AgentMessage {
            role: MessageRole::User,
            content: "Hello".into(),
            ..Default::default()
        };
        let lines = message_widget(&msg, TranscriptMode::Tools, &ThemeTokens::default(), 80);
        assert!(lines.is_empty());
    }

    #[test]
    fn assistant_message_renders_in_compact() {
        let msg = AgentMessage {
            role: MessageRole::Assistant,
            content: "I can help with that.".into(),
            ..Default::default()
        };
        let lines = message_widget(&msg, TranscriptMode::Compact, &ThemeTokens::default(), 80);
        assert!(!lines.is_empty());
        let joined = lines.join("");
        assert!(joined.contains("ASST"));
    }

    #[test]
    fn full_mode_shows_reasoning() {
        let msg = AgentMessage {
            role: MessageRole::Assistant,
            content: "Result".into(),
            reasoning: Some("Step by step thinking".into()),
            ..Default::default()
        };
        let lines = message_widget(&msg, TranscriptMode::Full, &ThemeTokens::default(), 80);
        let joined = lines.join("");
        assert!(joined.contains("Reasoning"));
        assert!(joined.contains("Step by step"));
    }

    #[test]
    fn wrap_text_empty_string() {
        let lines = wrap_text("", 80);
        assert_eq!(lines, vec![""]);
    }

    #[test]
    fn wrap_text_zero_width() {
        let lines = wrap_text("hello", 0);
        assert_eq!(lines, vec!["hello"]);
    }
}
