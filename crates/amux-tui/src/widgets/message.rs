use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

use crate::state::chat::{AgentMessage, MessageRole, TranscriptMode};
use crate::theme::ThemeTokens;

/// Set of message indices whose reasoning blocks are expanded
pub type ExpandedReasoning = std::collections::HashSet<usize>;

/// Convert a message into ratatui Lines (all owned/static)
pub fn message_to_lines(
    msg: &AgentMessage,
    msg_index: usize,
    mode: TranscriptMode,
    theme: &ThemeTokens,
    width: usize,
    expanded: &ExpandedReasoning,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    match mode {
        TranscriptMode::Compact => render_compact(msg, msg_index, theme, width, expanded, &mut lines),
        TranscriptMode::Tools => render_tools_only(msg, theme, width, &mut lines),
        TranscriptMode::Full => render_full(msg, msg_index, theme, width, expanded, &mut lines),
    }

    lines
}

fn render_compact(
    msg: &AgentMessage,
    msg_index: usize,
    theme: &ThemeTokens,
    width: usize,
    expanded: &ExpandedReasoning,
    lines: &mut Vec<Line<'static>>,
) {
    let indent = 7;
    let content_width = width.saturating_sub(indent + 1);

    // TOOL messages always render as compact one-liner with gear icon
    if msg.role == MessageRole::Tool {
        if let Some(name) = &msg.tool_name {
            let status = msg.tool_status.as_deref().unwrap_or("done");
            let (status_text, status_style) = format_tool_status(status, theme);
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled("\u{2699}", theme.accent_assistant),
                Span::raw(" "),
                Span::styled(name.clone(), theme.fg_dim),
                Span::raw(" "),
                Span::styled(status_text, status_style),
            ]));
        }
        return;
    }

    let content = &msg.content;
    if content.is_empty() {
        return;
    }

    let (badge, badge_style) = role_badge(msg.role);

    // For assistant messages: show reasoning BEFORE content
    if msg.role == MessageRole::Assistant {
        if let Some(reasoning) = &msg.reasoning {
            if !reasoning.is_empty() {
                let is_expanded = expanded.contains(&msg_index);
                if is_expanded {
                    // Expanded reasoning
                    lines.push(Line::from(vec![
                        Span::raw(" ".repeat(indent)),
                        Span::styled("\u{25be} [-] Reasoning", theme.fg_dim),
                    ]));
                    let reasoning_width = width.saturating_sub(indent + 2);
                    let dark_blue = Style::default().fg(Color::Indexed(24));
                    for line in wrap_text(reasoning, reasoning_width) {
                        lines.push(Line::from(vec![
                            Span::raw(" ".repeat(indent)),
                            Span::styled("\u{2502}", dark_blue),
                            Span::raw(" "),
                            Span::styled(line, theme.fg_dim),
                        ]));
                    }
                } else {
                    // Collapsed reasoning hint
                    lines.push(Line::from(vec![
                        Span::raw(" ".repeat(indent)),
                        Span::styled("\u{25b6} [+] Reasoning", theme.fg_dim),
                    ]));
                }
            }
        }
    }

    // Badge + first line of content
    let content_lines = wrap_text(content, content_width);

    if let Some(first) = content_lines.first() {
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(
                badge,
                Style::default()
                    .bg(badge_style.fg.unwrap_or(Color::Indexed(245)))
                    .fg(Color::Black),
            ),
            Span::raw(" "),
            Span::styled(first.clone(), theme.fg_active),
        ]));
    }

    // Continuation lines
    for line in content_lines.iter().skip(1) {
        lines.push(Line::from(vec![
            Span::raw(" ".repeat(indent)),
            Span::styled(line.clone(), theme.fg_active),
        ]));
    }
}

fn render_tools_only(
    msg: &AgentMessage,
    theme: &ThemeTokens,
    width: usize,
    lines: &mut Vec<Line<'static>>,
) {
    if msg.role != MessageRole::Tool && msg.tool_name.is_none() {
        return;
    }

    if let Some(name) = &msg.tool_name {
        let status = msg.tool_status.as_deref().unwrap_or("done");
        let (status_text, status_style) = format_tool_status(status, theme);
        let args_preview = msg.tool_arguments.as_deref().unwrap_or("");
        let max_args = width.saturating_sub(30);
        let args_short = if args_preview.len() > max_args {
            &args_preview[..max_args]
        } else {
            args_preview
        };

        let mut spans = vec![
            Span::raw("  "),
            Span::styled("\u{2699}", theme.accent_assistant),
            Span::raw(" "),
            Span::styled(name.clone(), theme.fg_active),
            Span::raw(" "),
            Span::styled(status_text, status_style),
        ];

        if !args_short.is_empty() {
            spans.push(Span::raw("  "));
            spans.push(Span::styled(args_short.to_string(), theme.fg_dim));
        }

        lines.push(Line::from(spans));
    }
}

fn render_full(
    msg: &AgentMessage,
    msg_index: usize,
    theme: &ThemeTokens,
    width: usize,
    expanded: &ExpandedReasoning,
    lines: &mut Vec<Line<'static>>,
) {
    // Full mode: always expand reasoning
    let mut full_expanded = expanded.clone();
    full_expanded.insert(msg_index);
    render_compact(msg, msg_index, theme, width, &full_expanded, lines);
}

fn role_badge(role: MessageRole) -> (&'static str, Style) {
    match role {
        MessageRole::User => ("USER", Style::default().fg(Color::Indexed(75))),
        MessageRole::Assistant => ("ASST", Style::default().fg(Color::Indexed(183))),
        MessageRole::System => ("SYS ", Style::default().fg(Color::Indexed(245))),
        MessageRole::Tool => ("TOOL", Style::default().fg(Color::Indexed(183))),
        MessageRole::Unknown => ("??? ", Style::default().fg(Color::Indexed(245))),
    }
}

fn format_tool_status(status: &str, theme: &ThemeTokens) -> (&'static str, Style) {
    match status {
        "completed" | "done" | "success" => ("\u{2713} done", theme.accent_success),
        "error" | "failed" => ("\u{2717} error", theme.accent_danger),
        _ => ("\u{25cf} running", theme.accent_secondary),
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

    fn empty_expanded() -> ExpandedReasoning {
        ExpandedReasoning::new()
    }

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
        let lines = message_to_lines(&msg, 0, TranscriptMode::Compact, &ThemeTokens::default(), 80, &empty_expanded());
        assert!(!lines.is_empty());
    }

    #[test]
    fn tool_message_shows_gear_icon() {
        let msg = AgentMessage {
            role: MessageRole::Tool,
            tool_name: Some("bash_command".into()),
            tool_status: Some("done".into()),
            content: "some output here".into(), // has content but should still show compact
            ..Default::default()
        };
        let lines = message_to_lines(&msg, 0, TranscriptMode::Compact, &ThemeTokens::default(), 80, &empty_expanded());
        assert_eq!(lines.len(), 1); // single compact line
    }

    #[test]
    fn tool_message_with_content_renders_compact() {
        let msg = AgentMessage {
            role: MessageRole::Tool,
            tool_name: Some("list_workspaces".into()),
            tool_status: Some("done".into()),
            content: "Workspace Default:\n  Surface: Infinite Canvas".into(),
            ..Default::default()
        };
        let lines = message_to_lines(&msg, 0, TranscriptMode::Compact, &ThemeTokens::default(), 80, &empty_expanded());
        // Should be 1 compact line, not the full content
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn reasoning_before_content() {
        let msg = AgentMessage {
            role: MessageRole::Assistant,
            content: "Here is my answer".into(),
            reasoning: Some("Let me think...".into()),
            ..Default::default()
        };
        let lines = message_to_lines(&msg, 0, TranscriptMode::Compact, &ThemeTokens::default(), 80, &empty_expanded());
        // First line should be reasoning hint, then ASST badge
        assert!(lines.len() >= 2);
        let first_text: String = lines[0].spans.iter().map(|s| s.content.to_string()).collect();
        assert!(first_text.contains("Reasoning"), "First line should be reasoning hint, got: {}", first_text);
    }

    #[test]
    fn reasoning_expandable() {
        let msg = AgentMessage {
            role: MessageRole::Assistant,
            content: "Answer".into(),
            reasoning: Some("Thinking step by step".into()),
            ..Default::default()
        };
        let collapsed = message_to_lines(&msg, 0, TranscriptMode::Compact, &ThemeTokens::default(), 80, &empty_expanded());
        let mut exp = empty_expanded();
        exp.insert(0);
        let expanded = message_to_lines(&msg, 0, TranscriptMode::Compact, &ThemeTokens::default(), 80, &exp);
        assert!(expanded.len() > collapsed.len(), "Expanded should have more lines");
    }

    #[test]
    fn tools_mode_skips_non_tool_messages() {
        let msg = AgentMessage {
            role: MessageRole::User,
            content: "Hello".into(),
            ..Default::default()
        };
        let lines = message_to_lines(&msg, 0, TranscriptMode::Tools, &ThemeTokens::default(), 80, &empty_expanded());
        assert!(lines.is_empty());
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
