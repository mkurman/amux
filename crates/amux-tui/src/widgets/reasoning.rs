#![allow(dead_code)]

use crate::theme::{ThemeTokens, FG_CLOSE};

/// Render a reasoning block — collapsed (1 line) or expanded (header + content with dark blue border).
///
/// Collapsed: `▸ \[+] Reasoning (12s · 847 tok)` in fg_dim
/// Expanded: `▾ \[-] Reasoning (12s · 847 tok)` then reasoning text with dark blue `│` left border
pub fn reasoning_widget(
    reasoning: &str,
    expanded: bool,
    elapsed_secs: Option<u64>,
    token_count: Option<u64>,
    theme: &ThemeTokens,
    width: usize,
) -> Vec<String> {
    let mut lines = Vec::new();

    // Build the stats suffix: "(12s · 847 tok)"
    let stats = build_stats(elapsed_secs, token_count);

    // Dark blue color for reasoning border
    let dark_blue = crate::theme::Color(24);

    if expanded {
        // Header line: ▾ \[-] Reasoning (12s · 847 tok)
        let header = format!(
            "{}▾ \\[-] Reasoning {}{}",
            theme.fg_dim.fg(),
            stats,
            FG_CLOSE,
        );
        lines.push(header);

        // Reasoning content with dark blue left border
        let content_width = width.saturating_sub(2); // 2 chars for "│ "
        for paragraph in reasoning.split('\n') {
            if paragraph.is_empty() {
                lines.push(format!(
                    "{}\u{2502}{}",
                    dark_blue.fg(),
                    FG_CLOSE,
                ));
                continue;
            }
            // Word-wrap within content_width
            let wrapped = wrap_text(paragraph, content_width);
            for line in wrapped {
                lines.push(format!(
                    "{}\u{2502}{} {}{}{}",
                    dark_blue.fg(),
                    FG_CLOSE,
                    theme.fg_dim.fg(),
                    line,
                    FG_CLOSE,
                ));
            }
        }
    } else {
        // Collapsed single line: ▸ \[+] Reasoning (12s · 847 tok)
        let line = format!(
            "{}▸ \\[+] Reasoning {}{}",
            theme.fg_dim.fg(),
            stats,
            FG_CLOSE,
        );
        lines.push(line);
    }

    lines
}

/// Build stats string like "(12s · 847 tok)" from optional elapsed/token values.
fn build_stats(elapsed_secs: Option<u64>, token_count: Option<u64>) -> String {
    match (elapsed_secs, token_count) {
        (Some(s), Some(t)) => format!("({}s · {} tok)", s, t),
        (Some(s), None) => format!("({}s)", s),
        (None, Some(t)) => format!("({} tok)", t),
        (None, None) => String::new(),
    }
}

/// Word-wrap text to fit within a given width.
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
    use crate::theme::ThemeTokens;

    #[test]
    fn reasoning_widget_collapsed_is_one_line() {
        let theme = ThemeTokens::default();
        let lines = reasoning_widget("Some deep thoughts", false, Some(12), Some(847), &theme, 80);
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn reasoning_widget_collapsed_shows_plus_icon() {
        let theme = ThemeTokens::default();
        let lines = reasoning_widget("Some deep thoughts", false, Some(12), Some(847), &theme, 80);
        let line = &lines[0];
        // In markup the literal [+] is escaped as \[+]
        assert!(line.contains("\\[+]"), "expected \\[+] in: {}", line);
        assert!(line.contains("Reasoning"), "expected Reasoning in: {}", line);
    }

    #[test]
    fn reasoning_widget_collapsed_shows_stats() {
        let theme = ThemeTokens::default();
        let lines = reasoning_widget("Some deep thoughts", false, Some(12), Some(847), &theme, 80);
        let line = &lines[0];
        assert!(line.contains("12s"), "expected 12s in: {}", line);
        assert!(line.contains("847 tok"), "expected 847 tok in: {}", line);
    }

    #[test]
    fn reasoning_widget_expanded_has_multiple_lines() {
        let theme = ThemeTokens::default();
        let lines = reasoning_widget("Step one\nStep two", true, Some(5), Some(200), &theme, 80);
        assert!(lines.len() > 1, "expected multiple lines, got {}", lines.len());
    }

    #[test]
    fn reasoning_widget_expanded_shows_minus_icon() {
        let theme = ThemeTokens::default();
        let lines = reasoning_widget("Thinking...", true, Some(5), None, &theme, 80);
        assert!(lines[0].contains("\\[-]"), "expected \\[-] in header: {}", lines[0]);
        assert!(lines[0].contains("Reasoning"), "expected Reasoning in header: {}", lines[0]);
    }

    #[test]
    fn reasoning_widget_expanded_shows_dark_blue_border() {
        let theme = ThemeTokens::default();
        let lines = reasoning_widget("First line", true, None, None, &theme, 80);
        // Content lines after header should contain the dark blue color markup tag
        let content_joined = lines[1..].join("");
        assert!(content_joined.contains("[fg=rgb("), "expected markup fg tag");
        assert!(content_joined.contains('\u{2502}'), "expected │ border char");
    }

    #[test]
    fn reasoning_widget_expanded_contains_reasoning_text() {
        let theme = ThemeTokens::default();
        let lines = reasoning_widget("Step by step thinking", true, None, None, &theme, 80);
        let joined = lines.join("");
        assert!(joined.contains("Step by step thinking"));
    }

    #[test]
    fn reasoning_widget_no_stats_shows_no_parens() {
        let theme = ThemeTokens::default();
        let lines = reasoning_widget("text", false, None, None, &theme, 80);
        let line = &lines[0];
        // Should not contain "()" since no stats
        assert!(!line.contains("()"), "should not have empty parens: {}", line);
    }

    #[test]
    fn build_stats_both_some() {
        assert_eq!(build_stats(Some(10), Some(500)), "(10s · 500 tok)");
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
