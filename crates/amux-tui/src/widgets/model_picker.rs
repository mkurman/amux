use ratatui::prelude::*;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, BorderType, List, ListItem, Paragraph};

use crate::state::config::ConfigState;
use crate::state::modal::ModalState;
use crate::theme::ThemeTokens;

pub fn render(
    frame: &mut Frame,
    area: Rect,
    modal: &ModalState,
    config: &ConfigState,
    theme: &ThemeTokens,
) {
    let block = Block::default()
        .title(" MODEL ")
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(theme.accent_secondary);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height < 3 {
        return;
    }

    // Split: list (flex) + hints (1)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner);

    let models = config.fetched_models();
    let cursor = modal.picker_cursor();
    let active_model = config.model();

    if models.is_empty() {
        let empty_line = Line::from(Span::styled(
            "  Press Enter to fetch models",
            theme.fg_dim,
        ));
        frame.render_widget(Paragraph::new(empty_line), chunks[0]);
    } else {
        let list_h = chunks[0].height as usize;
        let visible_models: Vec<_> = models.iter().take(list_h).collect();

        let mut list_items: Vec<ListItem> = visible_models
            .iter()
            .enumerate()
            .map(|(i, model)| {
                let display_name = model.name.as_deref().unwrap_or(&model.id);
                let is_selected = i == cursor;
                let is_active = model.id == active_model || display_name == active_model;

                let ctx_str = model
                    .context_window
                    .map(|c| format!(" ({}k ctx)", c / 1000))
                    .unwrap_or_default();

                if is_selected {
                    ListItem::new(Line::from(vec![
                        Span::raw(" > "),
                        Span::raw(display_name.to_string()),
                        Span::raw(ctx_str),
                    ]))
                    .style(
                        Style::default()
                            .bg(Color::Indexed(178))
                            .fg(Color::Black),
                    )
                } else if is_active && !active_model.is_empty() {
                    ListItem::new(Line::from(vec![
                        Span::raw("  "),
                        Span::styled(
                            format!("\u{2022} {}", display_name),
                            theme.accent_secondary,
                        ),
                        Span::styled(ctx_str, theme.fg_dim),
                    ]))
                } else {
                    ListItem::new(Line::from(vec![
                        Span::raw("   "),
                        Span::styled(display_name.to_string(), theme.fg_active),
                        Span::styled(ctx_str, theme.fg_dim),
                    ]))
                }
            })
            .collect();

        if models.len() > list_h {
            list_items.push(ListItem::new(Line::from(Span::styled(
                format!("  ... {} more models", models.len() - list_h),
                theme.fg_dim,
            ))));
        }

        let list = List::new(list_items);
        frame.render_widget(list, chunks[0]);
    }

    // Hints
    let hints = Line::from(vec![
        Span::raw(" "),
        Span::styled("j/k", theme.fg_active),
        Span::styled(" nav  ", theme.fg_dim),
        Span::styled("Enter", theme.fg_active),
        Span::styled(" sel  ", theme.fg_dim),
        Span::styled("Esc", theme.fg_active),
        Span::styled(" close", theme.fg_dim),
    ]);
    frame.render_widget(Paragraph::new(hints), chunks[1]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::config::{ConfigAction, ConfigState, FetchedModel};

    #[test]
    fn model_picker_handles_empty_state() {
        let _modal = ModalState::new();
        let config = ConfigState::new();
        let _theme = ThemeTokens::default();
        assert!(config.fetched_models().is_empty());
    }

    #[test]
    fn model_picker_handles_fetched_state() {
        let mut config = ConfigState::new();
        config.reduce(ConfigAction::ModelsFetched(vec![FetchedModel {
            id: "gpt-4o".into(),
            name: Some("GPT-4o".into()),
            context_window: Some(128_000),
        }]));
        assert_eq!(config.fetched_models().len(), 1);
    }
}
