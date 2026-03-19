use crate::theme::{ThemeTokens, ROUNDED_BORDER, RESET};
use crate::state::input::{InputState, InputMode};
use crate::state::FocusArea;

pub fn footer_widget(
    input: &InputState,
    theme: &ThemeTokens,
    _focus: FocusArea,
    focused: bool,
    width: usize,
) -> Vec<String> {
    let border_color = if focused { theme.accent_primary } else { theme.fg_dim };
    let bc = border_color.fg();
    let b = &ROUNDED_BORDER;
    let inner_width = width.saturating_sub(2);

    // Line 1: mode + input
    let mode_label = match input.mode() {
        InputMode::Normal => format!("{}NORMAL{}", theme.fg_dim.fg(), RESET),
        InputMode::Insert => format!("{}INSERT{}", theme.accent_primary.fg(), RESET),
    };
    let cursor = if input.mode() == InputMode::Insert { "█" } else { "" };
    let input_line = format!(
        " {} {}▶{} {}{}",
        mode_label,
        theme.accent_primary.fg(),
        RESET,
        input.buffer(),
        cursor,
    );
    let padded_input = pad_to_width(&input_line, inner_width);

    // Line 2: context-sensitive hints
    let hints = format!(
        " {}tab{}:focus  {}ctrl+p{}:commands  {}ctrl+t{}:threads  {}/{}:slash  {}q{}:quit",
        theme.fg_active.fg(),
        theme.fg_dim.fg(),
        theme.fg_active.fg(),
        theme.fg_dim.fg(),
        theme.fg_active.fg(),
        theme.fg_dim.fg(),
        theme.fg_active.fg(),
        theme.fg_dim.fg(),
        theme.fg_active.fg(),
        theme.fg_dim.fg(),
    );
    let padded_hints_raw = format!("{}{}", hints, RESET);
    let padded_hints = pad_to_width(&padded_hints_raw, inner_width);

    vec![
        format!(
            "{}{}{}{}{}",
            bc,
            b.top_left,
            super::repeat_char(b.horizontal, inner_width),
            b.top_right,
            RESET
        ),
        format!("{}{}{}{}{}", bc, b.vertical, padded_input, b.vertical, RESET),
        format!("{}{}{}{}{}", bc, b.vertical, padded_hints, b.vertical, RESET),
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

fn pad_to_width(s: &str, width: usize) -> String {
    let visible = super::strip_ansi_len(s);
    if visible < width {
        format!("{}{}", s, " ".repeat(width - visible))
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::ThemeTokens;
    use crate::state::input::InputState;
    use crate::state::FocusArea;

    #[test]
    fn footer_widget_returns_four_lines() {
        let input = InputState::new();
        let theme = ThemeTokens::default();
        let lines = footer_widget(&input, &theme, FocusArea::Input, true, 80);
        assert_eq!(lines.len(), 4);
    }

    #[test]
    fn footer_widget_focused_vs_unfocused() {
        let input = InputState::new();
        let theme = ThemeTokens::default();
        let focused = footer_widget(&input, &theme, FocusArea::Input, true, 80);
        let unfocused = footer_widget(&input, &theme, FocusArea::Chat, false, 80);
        assert_ne!(focused[0], unfocused[0]);
    }

    #[test]
    fn footer_widget_normal_mode_shows_normal_label() {
        let mut input = InputState::new();
        input.set_mode(InputMode::Normal);
        let theme = ThemeTokens::default();
        let lines = footer_widget(&input, &theme, FocusArea::Chat, false, 80);
        assert!(lines[1].contains("NORMAL"));
    }

    #[test]
    fn footer_widget_insert_mode_shows_insert_label() {
        let mut input = InputState::new();
        input.set_mode(InputMode::Insert);
        let theme = ThemeTokens::default();
        let lines = footer_widget(&input, &theme, FocusArea::Input, true, 80);
        assert!(lines[1].contains("INSERT"));
    }
}
