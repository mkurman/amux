use crate::theme::{ThemeTokens, ROUNDED_BORDER, RESET};
use crate::state::config::ConfigState;
use crate::state::chat::ChatState;

pub fn header_widget(
    config: &ConfigState,
    chat: &ChatState,
    theme: &ThemeTokens,
    focused: bool,
    width: usize,
) -> Vec<String> {
    let border_color = if focused { theme.accent_primary } else { theme.fg_dim };
    let bc = border_color.fg();
    let b = &ROUNDED_BORDER;

    // Build content
    let logo = format!(
        "{}░▒▓{}TAMUX{}▓▒░{}",
        theme.fg_dim.fg(),
        theme.accent_primary.fg(),
        theme.fg_dim.fg(),
        RESET,
    );

    let model = if config.model.is_empty() {
        "no model".to_string()
    } else {
        format!("{}{}{}", theme.fg_active.fg(), config.model, RESET)
    };

    // Token usage from active thread
    let (in_tok, out_tok) = if let Some(thread) = chat.active_thread() {
        (thread.total_input_tokens, thread.total_output_tokens)
    } else {
        (0, 0)
    };
    let total_tok = in_tok + out_tok;
    let usage = format!(
        "{}{}k tok{}",
        theme.fg_dim.fg(),
        if total_tok > 0 {
            format!("{:.1}", total_tok as f64 / 1000.0)
        } else {
            "0".to_string()
        },
        RESET,
    );

    let inner_width = width.saturating_sub(2); // border chars

    let content = format!(
        " {} {} {} {}",
        logo,
        if config.provider.is_empty() { "" } else { &config.provider },
        model,
        usage,
    );

    // Pad content to inner_width
    let visible_len = super::strip_ansi_len(&content);
    let padded = if visible_len < inner_width {
        format!("{}{}", content, " ".repeat(inner_width - visible_len))
    } else {
        content
    };

    vec![
        format!(
            "{}{}{}{}{}",
            bc,
            b.top_left,
            super::repeat_char(b.horizontal, inner_width),
            b.top_right,
            RESET
        ),
        format!("{}{}{}{}{}", bc, b.vertical, padded, b.vertical, RESET),
        format!(
            "{}{}{}{}{}",
            bc,
            b.bottom_left,
            super::repeat_char(b.horizontal, inner_width),
            b.bottom_right,
            RESET
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::ThemeTokens;
    use crate::state::config::ConfigState;
    use crate::state::chat::ChatState;

    #[test]
    fn header_widget_returns_three_lines() {
        let config = ConfigState::new();
        let chat = ChatState::new();
        let theme = ThemeTokens::default();
        let lines = header_widget(&config, &chat, &theme, false, 80);
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn header_widget_focused_vs_unfocused() {
        let config = ConfigState::new();
        let chat = ChatState::new();
        let theme = ThemeTokens::default();
        let unfocused = header_widget(&config, &chat, &theme, false, 80);
        let focused = header_widget(&config, &chat, &theme, true, 80);
        // They should differ (different border color)
        assert_ne!(unfocused[0], focused[0]);
    }
}
