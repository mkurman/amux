use crate::theme::{ThemeTokens, Color, FG_CLOSE};

/// Render the splash screen — centered logo + motto + hints.
/// Returns lines that should be placed in the chat area when no thread is active.
pub fn splash_widget(theme: &ThemeTokens, width: usize, height: usize) -> Vec<String> {
    let mut lines = Vec::new();

    // Vertical centering: put logo ~1/3 from top
    let pad_top = height / 3;
    for _ in 0..pad_top {
        lines.push(" ".repeat(width));
    }

    // Logo with gradient: ░▒▓█ T A M U X █▓▒░
    // Colors: dark blue → cyan → bright cyan across the gradient characters
    let gradient_colors = [
        Color(24),   // ░ dark blue
        Color(31),   // ▒ medium blue
        Color(38),   // ▓ blue-cyan
        Color(75),   // █ cyan (accent_primary)
    ];

    let logo = format!(
        "{}░{}▒{}▓{}█{} T A M U X {}█{}▓{}▒{}░{}",
        gradient_colors[0].fg(),
        gradient_colors[1].fg(),
        gradient_colors[2].fg(),
        gradient_colors[3].fg(),
        theme.accent_primary.fg(),
        gradient_colors[3].fg(),
        gradient_colors[2].fg(),
        gradient_colors[1].fg(),
        gradient_colors[0].fg(),
        FG_CLOSE,
    );
    lines.push(center_markup(&logo, width));

    // Motto
    let motto = format!(
        "{}plan · solve · ship{}",
        theme.fg_dim.fg(),
        FG_CLOSE,
    );
    lines.push(center_markup(&motto, width));

    // Empty line
    lines.push(" ".repeat(width));

    // Hints
    let hint1 = format!(
        "{}Type a prompt to begin, or{}",
        theme.fg_dim.fg(),
        FG_CLOSE,
    );
    lines.push(center_markup(&hint1, width));

    let hint2 = format!(
        "{}Ctrl+P{} {}to open command palette{}",
        theme.accent_primary.fg(),
        FG_CLOSE,
        theme.fg_dim.fg(),
        FG_CLOSE,
    );
    lines.push(center_markup(&hint2, width));

    let hint3 = format!(
        "{}Ctrl+T{} {}to pick a thread{}",
        theme.accent_primary.fg(),
        FG_CLOSE,
        theme.fg_dim.fg(),
        FG_CLOSE,
    );
    lines.push(center_markup(&hint3, width));

    // Pad remaining height
    while lines.len() < height {
        lines.push(" ".repeat(width));
    }

    // Truncate if too many lines
    lines.truncate(height);
    lines
}

/// Center a string containing markup tags within a given width
fn center_markup(s: &str, width: usize) -> String {
    let visible = super::strip_markup_len(s);
    if visible >= width {
        return s.to_string();
    }
    let pad_left = (width - visible) / 2;
    let pad_right = width - visible - pad_left;
    format!("{}{}{}", " ".repeat(pad_left), s, " ".repeat(pad_right))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splash_widget_returns_correct_height() {
        let theme = ThemeTokens::default();
        let lines = splash_widget(&theme, 80, 20);
        assert_eq!(lines.len(), 20);
    }

    #[test]
    fn splash_widget_lines_have_correct_width() {
        let theme = ThemeTokens::default();
        let lines = splash_widget(&theme, 80, 20);
        for line in &lines {
            let visible = super::super::strip_markup_len(line);
            assert_eq!(visible, 80, "Line visible width mismatch: {:?}", line);
        }
    }

    #[test]
    fn splash_widget_zero_height() {
        let theme = ThemeTokens::default();
        let lines = splash_widget(&theme, 80, 0);
        assert_eq!(lines.len(), 0);
    }

    #[test]
    fn splash_widget_small_dimensions() {
        let theme = ThemeTokens::default();
        let lines = splash_widget(&theme, 10, 5);
        assert_eq!(lines.len(), 5);
    }

    #[test]
    fn center_markup_centers_plain_string() {
        // "hello" is 5 chars, centering in 11 → 3 left, 3 right
        let result = center_markup("hello", 11);
        assert_eq!(result, "   hello   ");
    }

    #[test]
    fn center_markup_returns_as_is_when_too_wide() {
        let result = center_markup("hello world", 5);
        assert_eq!(result, "hello world");
    }
}
