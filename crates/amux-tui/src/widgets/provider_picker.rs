use crate::theme::{ThemeTokens, SHARP_BORDER, RESET};
use crate::state::modal::ModalState;
use crate::state::config::ConfigState;

/// All supported LLM providers.
pub const PROVIDERS: &[&str] = &[
    "OpenAI",
    "Anthropic",
    "Groq",
    "Ollama",
    "Together",
    "DeepInfra",
    "Cerebras",
    "Z.AI (GLM)",
    "Kimi/Moonshot",
    "Qwen (Alibaba)",
    "MiniMax",
    "OpenRouter",
    "Custom",
];

/// Render the provider picker overlay.
/// Returns a full-screen Vec<String> (one entry per row) centered over the terminal.
pub fn provider_picker_widget(
    modal: &ModalState,
    config: &ConfigState,
    theme: &ThemeTokens,
    screen_width: usize,
    screen_height: usize,
) -> Vec<String> {
    let bc = theme.accent_secondary.fg(); // amber border
    let b = &SHARP_BORDER;

    // Size: ~35% width, fits provider list + header + footer
    let list_len = PROVIDERS.len();
    let picker_w = (screen_width * 35 / 100).max(35).min(screen_width);
    let picker_h = (list_len + 4).max(10).min(screen_height); // header + list + hints + border
    let inner_w = picker_w.saturating_sub(2);

    let x_pad = (screen_width.saturating_sub(picker_w)) / 2;
    let y_pad = (screen_height.saturating_sub(picker_h)) / 2;

    let mut result = Vec::new();

    // Top padding
    for _ in 0..y_pad {
        result.push(" ".repeat(screen_width));
    }

    // Top border with title
    let title = " PROVIDER ";
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
        " ".repeat(screen_width.saturating_sub(x_pad + picker_w)),
    ));

    let cursor = modal.picker_cursor();
    let active_provider = config.provider();

    // Provider list
    for (i, &provider) in PROVIDERS.iter().enumerate() {
        let is_selected = i == cursor;
        let is_active = provider.eq_ignore_ascii_case(active_provider)
            || active_provider.to_lowercase().contains(&provider.to_lowercase())
            || provider.to_lowercase().contains(&active_provider.to_lowercase());

        let line = if is_selected {
            // Selected: amber highlight
            format!(
                " {}{}> {}{}{}",
                theme.accent_secondary.bg(),
                "\x1b[38;5;0m", // black text on amber
                provider,
                RESET,
                " ".repeat(inner_w.saturating_sub(provider.len() + 3)),
            )
        } else if is_active && !active_provider.is_empty() {
            // Active but not selected: dim accent
            format!(
                "  {}• {}{}",
                theme.accent_secondary.fg(),
                provider,
                RESET,
            )
        } else {
            format!(
                "   {}{}{}",
                theme.fg_active.fg(),
                provider,
                RESET,
            )
        };

        let padded = super::pad_to_width(&line, inner_w);
        result.push(format!(
            "{}{}{}{}{}{}{}",
            " ".repeat(x_pad),
            bc, b.vertical,
            padded,
            b.vertical,
            RESET,
            " ".repeat(screen_width.saturating_sub(x_pad + picker_w)),
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
        " ".repeat(screen_width.saturating_sub(x_pad + picker_w)),
    ));

    // Bottom border
    result.push(format!(
        "{}{}{}{}{}{}{}",
        " ".repeat(x_pad),
        bc, b.bottom_left,
        super::repeat_char(b.horizontal, inner_w),
        b.bottom_right,
        RESET,
        " ".repeat(screen_width.saturating_sub(x_pad + picker_w)),
    ));

    // Bottom padding
    while result.len() < screen_height {
        result.push(" ".repeat(screen_width));
    }
    result.truncate(screen_height);

    result
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::modal::{ModalState, ModalAction, ModalKind};
    use crate::state::config::{ConfigState, ConfigAction};
    use crate::theme::ThemeTokens;

    #[test]
    fn provider_picker_returns_correct_height() {
        let modal = ModalState::new();
        let config = ConfigState::new();
        let theme = ThemeTokens::default();
        let lines = provider_picker_widget(&modal, &config, &theme, 120, 40);
        assert_eq!(lines.len(), 40);
    }

    #[test]
    fn provider_picker_contains_title() {
        let modal = ModalState::new();
        let config = ConfigState::new();
        let theme = ThemeTokens::default();
        let lines = provider_picker_widget(&modal, &config, &theme, 120, 40);
        let joined = lines.join("");
        assert!(joined.contains("PROVIDER"));
    }

    #[test]
    fn provider_picker_shows_all_providers() {
        let modal = ModalState::new();
        let config = ConfigState::new();
        let theme = ThemeTokens::default();
        let lines = provider_picker_widget(&modal, &config, &theme, 120, 40);
        let joined = lines.join("");
        for provider in PROVIDERS {
            assert!(joined.contains(provider), "missing provider: {}", provider);
        }
    }

    #[test]
    fn provider_picker_13_providers() {
        assert_eq!(PROVIDERS.len(), 13);
    }

    #[test]
    fn provider_picker_first_item_selected_by_default() {
        let modal = ModalState::new();
        let config = ConfigState::new();
        let theme = ThemeTokens::default();
        let lines = provider_picker_widget(&modal, &config, &theme, 120, 40);
        let joined = lines.join("");
        // First provider "OpenAI" should be highlighted (amber bg used)
        assert!(joined.contains("OpenAI"));
        // cursor == 0 so first item is selected, amber bg escape used
        assert!(joined.contains(&theme.accent_secondary.bg()));
    }

    #[test]
    fn provider_picker_navigation_moves_cursor() {
        let mut modal = ModalState::new();
        modal.reduce(ModalAction::Push(ModalKind::ProviderPicker));
        modal.reduce(ModalAction::Navigate(2)); // cursor = 2 → Groq
        let config = ConfigState::new();
        let theme = ThemeTokens::default();
        let lines = provider_picker_widget(&modal, &config, &theme, 120, 40);
        let joined = lines.join("");
        // "Groq" should appear with the selection marker
        assert!(joined.contains("Groq"));
    }

    #[test]
    fn provider_picker_shows_active_provider_marker() {
        let modal = ModalState::new();
        let mut config = ConfigState::new();
        config.reduce(ConfigAction::SetProvider("Anthropic".into()));
        let theme = ThemeTokens::default();
        let lines = provider_picker_widget(&modal, &config, &theme, 120, 40);
        let joined = lines.join("");
        assert!(joined.contains("Anthropic"));
    }

    #[test]
    fn provider_picker_hints_displayed() {
        let modal = ModalState::new();
        let config = ConfigState::new();
        let theme = ThemeTokens::default();
        let lines = provider_picker_widget(&modal, &config, &theme, 120, 40);
        let joined = lines.join("");
        assert!(joined.contains("navigate"));
        assert!(joined.contains("select"));
        assert!(joined.contains("close"));
    }
}
