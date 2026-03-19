#![allow(dead_code)]

use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

use crate::theme::ThemeTokens;

/// Render a reasoning block as Lines -- collapsed (1 line) or expanded (header + content).
pub fn reasoning_lines<'a>(
    reasoning: &'a str,
    expanded: bool,
    elapsed_secs: Option<u64>,
    token_count: Option<u64>,
    theme: &ThemeTokens,
    width: usize,
) -> Vec<Line<'a>> {
    let mut lines = Vec::new();
    let stats = build_stats(elapsed_secs, token_count);
    let dark_blue_style = Style::default().fg(Color::Indexed(24));

    if expanded {
        // Header line
        lines.push(Line::from(vec![
            Span::styled(
                format!("\u{25be} [-] Reasoning {}", stats),
                theme.fg_dim,
            ),
        ]));

        // Reasoning content with dark blue left border
        let content_width = width.saturating_sub(2);
        for paragraph in reasoning.split('\n') {
            if paragraph.is_empty() {
                lines.push(Line::from(Span::styled("\u{2502}", dark_blue_style)));
                continue;
            }
            let wrapped = wrap_text(paragraph, content_width);
            for line in wrapped {
                lines.push(Line::from(vec![
                    Span::styled("\u{2502}", dark_blue_style),
                    Span::raw(" "),
                    Span::styled(line, theme.fg_dim),
                ]));
            }
        }
    } else {
        // Collapsed single line
        lines.push(Line::from(Span::styled(
            format!("\u{25b8} [+] Reasoning {}", stats),
            theme.fg_dim,
        )));
    }

    lines
}

fn build_stats(elapsed_secs: Option<u64>, token_count: Option<u64>) -> String {
    match (elapsed_secs, token_count) {
        (Some(s), Some(t)) => format!("({}s \u{00b7} {} tok)", s, t),
        (Some(s), None) => format!("({}s)", s),
        (None, Some(t)) => format!("({} tok)", t),
        (None, None) => String::new(),
    }
}

fn wrap_text(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![text.to_string()];
    }
    let mut lines = Vec::new();
    let mut current_line = String::new();
    for word in text.split_whitespace() {
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
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reasoning_collapsed_is_one_line() {
        let theme = ThemeTokens::default();
        let lines = reasoning_lines("Some deep thoughts", false, Some(12), Some(847), &theme, 80);
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn reasoning_expanded_has_multiple_lines() {
        let theme = ThemeTokens::default();
        let lines = reasoning_lines("Step one\nStep two", true, Some(5), Some(200), &theme, 80);
        assert!(lines.len() > 1);
    }

    #[test]
    fn build_stats_both_some() {
        assert_eq!(
            build_stats(Some(10), Some(500)),
            "(10s \u{00b7} 500 tok)"
        );
    }

    #[test]
    fn build_stats_only_elapsed() {
        assert_eq!(build_stats(Some(5), None), "(5s)");
    }

    #[test]
    fn build_stats_only_tokens() {
        assert_eq!(build_stats(None, Some(300)), "(300 tok)");
    }

    #[test]
    fn build_stats_neither() {
        assert_eq!(build_stats(None, None), "");
    }

    #[test]
    fn wrap_text_long_line_wraps() {
        let lines = wrap_text("one two three four five six seven eight", 20);
        assert!(lines.len() > 1);
    }

    #[test]
    fn wrap_text_zero_width_returns_input() {
        let lines = wrap_text("hello", 0);
        assert_eq!(lines, vec!["hello"]);
    }
}
