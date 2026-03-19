use crate::theme::{ThemeTokens, SHARP_BORDER, FG_CLOSE, BG_CLOSE};
use crate::state::modal::ModalState;
use crate::state::config::ConfigState;

/// Black text color for highlighted items
const BLACK_FG: &str = "[fg=rgb(0,0,0)]";

/// Render the model picker overlay.
/// Returns a full-screen Vec<String> (one entry per row) centered over the terminal.
pub fn model_picker_widget(
    modal: &ModalState,
    config: &ConfigState,
    theme: &ThemeTokens,
    screen_width: usize,
    screen_height: usize,
) -> Vec<String> {
    let bc = theme.accent_secondary.fg(); // amber border
    let b = &SHARP_BORDER;

    let models = config.fetched_models();
    let has_models = !models.is_empty();

    // Size: ~45% width, dynamic height
    let list_h = if has_models { models.len().min(15) } else { 1 };
    let picker_w = (screen_width * 45 / 100).max(40).min(screen_width);
    let picker_h = (list_h + 4).max(8).min(screen_height);
    let inner_w = picker_w.saturating_sub(2);

    let x_pad = (screen_width.saturating_sub(picker_w)) / 2;
    let y_pad = (screen_height.saturating_sub(picker_h)) / 2;

    let mut result = Vec::new();

    // Top padding
    for _ in 0..y_pad {
        result.push(" ".repeat(screen_width));
    }

    // Top border with title
    let title = " MODEL ";
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
        FG_CLOSE,
        " ".repeat(screen_width.saturating_sub(x_pad + picker_w)),
    ));

    let cursor = modal.picker_cursor();
    let active_model = config.model();

    if has_models {
        let visible_models: Vec<_> = models.iter().take(list_h).collect();

        for (i, model) in visible_models.iter().enumerate() {
            let display_name = model.name.as_deref().unwrap_or(&model.id);
            let is_selected = i == cursor;
            let is_active = model.id == active_model || display_name == active_model;

            let ctx_str = model
                .context_window
                .map(|c| format!(" {}({}k ctx){}", theme.fg_dim.fg(), c / 1000, FG_CLOSE))
                .unwrap_or_default();

            let line = if is_selected {
                format!(
                    " {}{}> {}{}{}{}",
                    theme.accent_secondary.bg(),
                    BLACK_FG,
                    display_name,
                    FG_CLOSE,
                    BG_CLOSE,
                    ctx_str,
                )
            } else if is_active && !active_model.is_empty() {
                format!(
                    "  {}• {}{}{}",
                    theme.accent_secondary.fg(),
                    display_name,
                    FG_CLOSE,
                    ctx_str,
                )
            } else {
                format!(
                    "   {}{}{}{}",
                    theme.fg_active.fg(),
                    display_name,
                    FG_CLOSE,
                    ctx_str,
                )
            };

            let padded = super::pad_to_width(&line, inner_w);
            result.push(format!(
                "{}{}{}{}{}{}{}",
                " ".repeat(x_pad),
                bc, b.vertical,
                padded,
                b.vertical,
                FG_CLOSE,
                " ".repeat(screen_width.saturating_sub(x_pad + picker_w)),
            ));
        }

        if models.len() > list_h {
            let more_line = format!(
                "  {}... {} more models{}",
                theme.fg_dim.fg(),
                models.len() - list_h,
                FG_CLOSE,
            );
            let padded = super::pad_to_width(&more_line, inner_w);
            result.push(format!(
                "{}{}{}{}{}{}{}",
                " ".repeat(x_pad),
                bc, b.vertical,
                padded,
                b.vertical,
                FG_CLOSE,
                " ".repeat(screen_width.saturating_sub(x_pad + picker_w)),
            ));
        }
    } else {
        let empty_line = format!(
            "  {}Press Enter to fetch models{}",
            theme.fg_dim.fg(), FG_CLOSE,
        );
        let padded = super::pad_to_width(&empty_line, inner_w);
        result.push(format!(
            "{}{}{}{}{}{}{}",
            " ".repeat(x_pad),
            bc, b.vertical,
            padded,
            b.vertical,
            FG_CLOSE,
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
    let padded_hints = super::pad_to_width(&format!("{}{}", hints, FG_CLOSE), inner_w);
    result.push(format!(
        "{}{}{}{}{}{}{}",
        " ".repeat(x_pad),
        bc, b.vertical,
        padded_hints,
        b.vertical,
        FG_CLOSE,
        " ".repeat(screen_width.saturating_sub(x_pad + picker_w)),
    ));

    // Bottom border
    result.push(format!(
        "{}{}{}{}{}{}{}",
        " ".repeat(x_pad),
        bc, b.bottom_left,
        super::repeat_char(b.horizontal, inner_w),
        b.bottom_right,
        FG_CLOSE,
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
    use crate::state::config::{ConfigState, ConfigAction, FetchedModel};
    use crate::theme::ThemeTokens;

    fn make_models() -> Vec<FetchedModel> {
        vec![
            FetchedModel { id: "gpt-4o".into(), name: Some("GPT-4o".into()), context_window: Some(128_000) },
            FetchedModel { id: "gpt-4o-mini".into(), name: Some("GPT-4o Mini".into()), context_window: Some(128_000) },
            FetchedModel { id: "gpt-3.5-turbo".into(), name: Some("GPT-3.5 Turbo".into()), context_window: Some(16_385) },
        ]
    }

    #[test]
    fn model_picker_returns_correct_height() {
        let modal = ModalState::new();
        let config = ConfigState::new();
        let theme = ThemeTokens::default();
        let lines = model_picker_widget(&modal, &config, &theme, 120, 40);
        assert_eq!(lines.len(), 40);
    }

    #[test]
    fn model_picker_contains_title() {
        let modal = ModalState::new();
        let config = ConfigState::new();
        let theme = ThemeTokens::default();
        let lines = model_picker_widget(&modal, &config, &theme, 120, 40);
        let joined = lines.join("");
        assert!(joined.contains("MODEL"));
    }

    #[test]
    fn model_picker_shows_fetch_prompt_when_empty() {
        let modal = ModalState::new();
        let config = ConfigState::new();
        let theme = ThemeTokens::default();
        let lines = model_picker_widget(&modal, &config, &theme, 120, 40);
        let joined = lines.join("");
        assert!(joined.contains("Press Enter to fetch models"));
    }

    #[test]
    fn model_picker_shows_models_when_fetched() {
        let modal = ModalState::new();
        let mut config = ConfigState::new();
        config.reduce(ConfigAction::ModelsFetched(make_models()));
        let theme = ThemeTokens::default();
        let lines = model_picker_widget(&modal, &config, &theme, 120, 40);
        let joined = lines.join("");
        assert!(joined.contains("GPT-4o"));
        assert!(joined.contains("GPT-4o Mini"));
        assert!(joined.contains("GPT-3.5 Turbo"));
    }

    #[test]
    fn model_picker_does_not_show_fetch_prompt_when_models_loaded() {
        let modal = ModalState::new();
        let mut config = ConfigState::new();
        config.reduce(ConfigAction::ModelsFetched(make_models()));
        let theme = ThemeTokens::default();
        let lines = model_picker_widget(&modal, &config, &theme, 120, 40);
        let joined = lines.join("");
        assert!(!joined.contains("Press Enter to fetch models"));
    }

    #[test]
    fn model_picker_highlights_active_model() {
        let modal = ModalState::new();
        let mut config = ConfigState::new();
        config.reduce(ConfigAction::ModelsFetched(make_models()));
        config.reduce(ConfigAction::SetModel("gpt-4o".into()));
        let theme = ThemeTokens::default();
        let lines = model_picker_widget(&modal, &config, &theme, 120, 40);
        let joined = lines.join("");
        assert!(joined.contains("GPT-4o"));
    }

    #[test]
    fn model_picker_navigation_moves_cursor() {
        let mut modal = ModalState::new();
        modal.reduce(ModalAction::Push(ModalKind::ModelPicker));
        modal.reduce(ModalAction::Navigate(1));
        let mut config = ConfigState::new();
        config.reduce(ConfigAction::ModelsFetched(make_models()));
        let theme = ThemeTokens::default();
        let lines = model_picker_widget(&modal, &config, &theme, 120, 40);
        let joined = lines.join("");
        assert!(joined.contains("GPT-4o Mini"));
    }

    #[test]
    fn model_picker_shows_context_window_annotation() {
        let modal = ModalState::new();
        let mut config = ConfigState::new();
        config.reduce(ConfigAction::ModelsFetched(make_models()));
        let theme = ThemeTokens::default();
        let lines = model_picker_widget(&modal, &config, &theme, 120, 40);
        let joined = lines.join("");
        assert!(joined.contains("128k ctx"));
    }

    #[test]
    fn model_picker_hints_displayed() {
        let modal = ModalState::new();
        let config = ConfigState::new();
        let theme = ThemeTokens::default();
        let lines = model_picker_widget(&modal, &config, &theme, 120, 40);
        let joined = lines.join("");
        assert!(joined.contains("navigate"));
        assert!(joined.contains("select"));
        assert!(joined.contains("close"));
    }
}
