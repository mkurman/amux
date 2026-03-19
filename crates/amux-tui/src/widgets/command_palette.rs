use crate::theme::{ThemeTokens, SHARP_BORDER, RESET};
use crate::state::modal::ModalState;

/// Render the command palette as an overlay.
/// Returns a full-screen Vec<String> (one entry per row) centered over the terminal.
pub fn command_palette_widget(
    modal: &ModalState,
    theme: &ThemeTokens,
    screen_width: usize,
    screen_height: usize,
) -> Vec<String> {
    let bc = theme.accent_secondary.fg(); // amber border
    let b = &SHARP_BORDER;

    // Size: ~50% width, ~40% height, centered
    let palette_w = (screen_width * 50 / 100).max(40).min(screen_width);
    let palette_h = (screen_height * 40 / 100).max(8).min(screen_height);
    let inner_w = palette_w.saturating_sub(2);
    let inner_h = palette_h.saturating_sub(2);

    let mut result = Vec::new();

    // Calculate centering offsets
    let x_pad = (screen_width.saturating_sub(palette_w)) / 2;
    let y_pad = (screen_height.saturating_sub(palette_h)) / 2;

    // Top padding (dimmed backdrop rows)
    for _ in 0..y_pad {
        result.push(" ".repeat(screen_width));
    }

    // Top border
    let title = " COMMANDS ";
    let title_len = title.len();
    let border_remaining = inner_w.saturating_sub(title_len);
    result.push(format!(
        "{}{}{}{}{}{}{}{}{}",
        " ".repeat(x_pad),
        bc, b.top_left,
        super::repeat_char(b.horizontal, 2),
        title,
        super::repeat_char(b.horizontal, border_remaining.saturating_sub(2)),
        b.top_right,
        RESET,
        " ".repeat(screen_width.saturating_sub(x_pad + palette_w)),
    ));

    // Search input line
    let query = modal.command_query();
    let input_line = format!(
        " {}{}{}{}",
        theme.fg_active.fg(),
        if query.is_empty() { "/" } else { query },
        "█",
        RESET,
    );
    let padded_input = super::pad_to_width(&input_line, inner_w);
    result.push(format!(
        "{}{}{}{}{}{}{}",
        " ".repeat(x_pad),
        bc, b.vertical,
        padded_input,
        b.vertical,
        RESET,
        " ".repeat(screen_width.saturating_sub(x_pad + palette_w)),
    ));

    // Separator
    result.push(format!(
        "{}{}{}{}{}{}{}",
        " ".repeat(x_pad),
        bc, b.vertical,
        super::repeat_char('─', inner_w),
        b.vertical,
        RESET,
        " ".repeat(screen_width.saturating_sub(x_pad + palette_w)),
    ));

    // Command list (inner_h - 3: input, separator, hints)
    let list_h = inner_h.saturating_sub(3);
    let filtered = modal.filtered_items();
    let items = modal.command_items();
    let cursor = modal.picker_cursor();

    for i in 0..list_h {
        let line = if i < filtered.len() {
            let idx = filtered[i];
            let item = &items[idx];
            let is_selected = i == cursor;

            if is_selected {
                // Amber bg, black text
                format!(
                    " {}{}> /{:<12} {}{}",
                    theme.accent_secondary.bg(),
                    "\x1b[38;5;0m", // black text
                    item.command,
                    item.description,
                    RESET,
                )
            } else {
                format!(
                    "   {}/{}{} {}{}{}",
                    theme.fg_active.fg(),
                    item.command,
                    RESET,
                    theme.fg_dim.fg(),
                    item.description,
                    RESET,
                )
            }
        } else {
            String::new()
        };

        let padded = super::pad_to_width(&line, inner_w);
        result.push(format!(
            "{}{}{}{}{}{}{}",
            " ".repeat(x_pad),
            bc, b.vertical,
            padded,
            b.vertical,
            RESET,
            " ".repeat(screen_width.saturating_sub(x_pad + palette_w)),
        ));
    }

    // Hints line
    let hints = format!(
        " {}j/k{} navigate  {}Enter{} select  {}Esc{} close",
        theme.fg_active.fg(), theme.fg_dim.fg(),
        theme.fg_active.fg(), theme.fg_dim.fg(),
        theme.fg_active.fg(), theme.fg_dim.fg(),
    );
    let padded_hints = super::pad_to_width(&format!("{}{}", hints, RESET), inner_w);
    result.push(format!(
        "{}{}{}{}{}{}{}",
        " ".repeat(x_pad),
        bc, b.vertical,
        padded_hints,
        b.vertical,
        RESET,
        " ".repeat(screen_width.saturating_sub(x_pad + palette_w)),
    ));

    // Bottom border
    result.push(format!(
        "{}{}{}{}{}{}{}",
        " ".repeat(x_pad),
        bc, b.bottom_left,
        super::repeat_char(b.horizontal, inner_w),
        b.bottom_right,
        RESET,
        " ".repeat(screen_width.saturating_sub(x_pad + palette_w)),
    ));

    // Bottom padding
    while result.len() < screen_height {
        result.push(" ".repeat(screen_width));
    }
    result.truncate(screen_height);

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_returns_correct_dimensions() {
        let modal = ModalState::new();
        let theme = ThemeTokens::default();
        let lines = command_palette_widget(&modal, &theme, 120, 40);
        assert_eq!(lines.len(), 40);
    }

    #[test]
    fn palette_contains_commands_title() {
        let modal = ModalState::new();
        let theme = ThemeTokens::default();
        let lines = command_palette_widget(&modal, &theme, 120, 40);
        let joined = lines.join("");
        assert!(joined.contains("COMMANDS"));
    }

    #[test]
    fn palette_shows_filtered_commands() {
        let mut modal = ModalState::new();
        modal.reduce(crate::state::modal::ModalAction::SetQuery("/pro".into()));
        let theme = ThemeTokens::default();
        let lines = command_palette_widget(&modal, &theme, 120, 40);
        let joined = lines.join("");
        assert!(joined.contains("provider"));
        assert!(joined.contains("prompt"));
    }
}
