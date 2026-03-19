use ratatui::prelude::*;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, BorderType, Paragraph, Tabs};

use crate::state::config::ConfigState;
use crate::state::settings::{SettingsState, SettingsTab};
use crate::theme::ThemeTokens;

pub fn render(
    frame: &mut Frame,
    area: Rect,
    settings: &SettingsState,
    config: &ConfigState,
    theme: &ThemeTokens,
) {
    let block = Block::default()
        .title(" SETTINGS ")
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(theme.accent_secondary);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height < 5 {
        return;
    }

    // Split: tab bar (1) + separator (1) + content (flex) + hints (1)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // tab bar
            Constraint::Length(1), // separator
            Constraint::Min(1),   // content
            Constraint::Length(1), // hints
        ])
        .split(inner);

    // Tab bar
    let active = settings.active_tab();
    let tab_labels = vec!["Provider", "Model", "Tools", "Reasoning", "Gateway", "Agent"];
    let tab_index = match active {
        SettingsTab::Provider => 0,
        SettingsTab::Model => 1,
        SettingsTab::Tools => 2,
        SettingsTab::Reasoning => 3,
        SettingsTab::Gateway => 4,
        SettingsTab::Agent => 5,
    };
    let tabs = Tabs::new(tab_labels)
        .select(tab_index)
        .style(theme.fg_dim)
        .highlight_style(theme.fg_active)
        .divider(Span::styled(" | ", theme.fg_dim));
    frame.render_widget(tabs, chunks[0]);

    // Separator
    let sep = Line::from(Span::styled(
        "\u{2500}".repeat(chunks[1].width as usize),
        theme.fg_dim,
    ));
    frame.render_widget(Paragraph::new(sep), chunks[1]);

    // Content
    let content_lines = render_tab_content(settings, config, theme);
    let paragraph = Paragraph::new(content_lines);
    frame.render_widget(paragraph, chunks[2]);

    // Hints
    let hints = Line::from(vec![
        Span::raw(" "),
        Span::styled("Tab", theme.fg_active),
        Span::styled(" switch tab  ", theme.fg_dim),
        Span::styled("Esc", theme.fg_active),
        Span::styled(" close", theme.fg_dim),
    ]);
    frame.render_widget(Paragraph::new(hints), chunks[3]);
}

fn render_tab_content<'a>(
    settings: &SettingsState,
    config: &'a ConfigState,
    theme: &ThemeTokens,
) -> Vec<Line<'a>> {
    match settings.active_tab() {
        SettingsTab::Provider => render_provider_tab(config, theme),
        SettingsTab::Model => render_model_tab(config, theme),
        SettingsTab::Tools => render_tools_tab(theme),
        SettingsTab::Reasoning => render_reasoning_tab(config, theme),
        SettingsTab::Gateway => render_gateway_tab(theme),
        SettingsTab::Agent => render_agent_tab(config, theme),
    }
}

fn render_provider_tab<'a>(config: &'a ConfigState, theme: &ThemeTokens) -> Vec<Line<'a>> {
    let mut lines = Vec::new();

    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled("  Provider", theme.fg_active)));
    lines.push(Line::from(Span::styled(
        "  Select your LLM provider and credentials",
        theme.fg_dim,
    )));
    lines.push(Line::raw(""));

    let provider = if config.provider().is_empty() {
        "(not set)"
    } else {
        config.provider()
    };
    let base_url = if config.base_url().is_empty() {
        "(not set)"
    } else {
        config.base_url()
    };
    let model = if config.model().is_empty() {
        "(not set)"
    } else {
        config.model()
    };
    let api_key_display = mask_api_key(config.api_key());

    lines.push(Line::from(vec![
        Span::styled("  Active Provider: ", theme.fg_dim),
        Span::styled(provider, theme.fg_active),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  Base URL:        ", theme.fg_dim),
        Span::styled(base_url, theme.fg_active),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  API Key:         ", theme.fg_dim),
        Span::styled(api_key_display, theme.fg_active),
        Span::styled(" [show]", theme.fg_dim),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  Model:           ", theme.fg_dim),
        Span::styled(model, theme.fg_active),
    ]));

    lines
}

fn render_model_tab<'a>(config: &'a ConfigState, theme: &ThemeTokens) -> Vec<Line<'a>> {
    let mut lines = Vec::new();

    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled("  Model", theme.fg_active)));
    lines.push(Line::from(Span::styled(
        "  Select model for current provider",
        theme.fg_dim,
    )));
    lines.push(Line::raw(""));

    let current = if config.model().is_empty() {
        "(not set)"
    } else {
        config.model()
    };
    lines.push(Line::from(vec![
        Span::styled("  Current:   ", theme.fg_dim),
        Span::styled(current, theme.fg_active),
    ]));

    let models = config.fetched_models();
    if models.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("  Available:  ", theme.fg_dim),
            Span::styled("(press Enter to fetch)", theme.fg_dim),
        ]));
    } else {
        lines.push(Line::from(Span::styled("  Available:", theme.fg_dim)));
        for m in models {
            let display_name = m.name.as_deref().unwrap_or(&m.id);
            let is_active = m.id == config.model() || display_name == config.model();
            if is_active {
                lines.push(Line::from(vec![
                    Span::raw("    "),
                    Span::styled(format!("> {}", display_name), theme.accent_secondary),
                ]));
            } else {
                lines.push(Line::from(vec![
                    Span::raw("    "),
                    Span::styled(format!("  {}", display_name), theme.fg_dim),
                ]));
            }
        }
    }

    lines
}

fn render_tools_tab(theme: &ThemeTokens) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled("  Tools", theme.fg_active)));
    lines.push(Line::from(Span::styled(
        "  Enable or disable tool categories",
        theme.fg_dim,
    )));
    lines.push(Line::raw(""));

    let tools = [
        (true, "Terminal / Bash"),
        (true, "File Operations"),
        (true, "Web Search"),
        (false, "Web Browse"),
        (true, "Workspace"),
        (false, "Messaging Gateway"),
    ];

    for (enabled, name) in &tools {
        let checkbox = if *enabled {
            Span::styled("[x]", theme.accent_success)
        } else {
            Span::styled("[ ]", theme.fg_dim)
        };
        lines.push(Line::from(vec![
            Span::raw("  "),
            checkbox,
            Span::raw(" "),
            Span::styled(*name, theme.fg_active),
        ]));
    }

    lines
}

fn render_reasoning_tab<'a>(config: &'a ConfigState, theme: &ThemeTokens) -> Vec<Line<'a>> {
    let mut lines = Vec::new();

    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled("  Reasoning", theme.fg_active)));
    lines.push(Line::from(Span::styled(
        "  Configure extended thinking",
        theme.fg_dim,
    )));
    lines.push(Line::raw(""));

    let current_effort = config.reasoning_effort();
    let effort_display = if current_effort.is_empty() {
        "Medium"
    } else {
        current_effort
    };

    lines.push(Line::from(vec![
        Span::styled("  Effort:  ", theme.fg_dim),
        Span::styled(format!("(\u{25cf}) {}", effort_display), theme.accent_secondary),
        Span::styled(" <- current", theme.fg_dim),
    ]));
    lines.push(Line::raw(""));
    lines.push(Line::from(vec![
        Span::styled("  Options:  ", theme.fg_dim),
        Span::styled(
            "Off / Minimal / Low / Medium / High / Extra High",
            theme.fg_dim,
        ),
    ]));

    lines
}

fn render_gateway_tab(theme: &ThemeTokens) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled("  Gateway", theme.fg_active)));
    lines.push(Line::from(Span::styled(
        "  Messaging platform connections",
        theme.fg_dim,
    )));
    lines.push(Line::raw(""));

    lines.push(Line::from(vec![
        Span::styled("  Gateway Enabled:  ", theme.fg_dim),
        Span::styled("[x] Yes", theme.accent_success),
    ]));

    lines
}

fn render_agent_tab<'a>(config: &'a ConfigState, theme: &ThemeTokens) -> Vec<Line<'a>> {
    let mut lines = Vec::new();

    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled("  Agent", theme.fg_active)));
    lines.push(Line::from(Span::styled(
        "  Agent identity and behavior",
        theme.fg_dim,
    )));
    lines.push(Line::raw(""));

    let agent_name = if let Some(raw) = config.agent_config_raw() {
        raw.get("agent_name")
            .and_then(|v| v.as_str())
            .unwrap_or("Sisyphus")
            .to_string()
    } else {
        "Sisyphus".to_string()
    };

    lines.push(Line::from(vec![
        Span::styled("  Agent Name:  ", theme.fg_dim),
        Span::styled(agent_name, theme.fg_active),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  Backend:     ", theme.fg_dim),
        Span::styled("daemon", theme.fg_active),
    ]));

    lines
}

fn mask_api_key(key: &str) -> String {
    if key.is_empty() {
        return "(not set)".to_string();
    }
    let chars: Vec<char> = key.chars().collect();
    let len = chars.len();
    if len <= 7 {
        return "\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}".to_string();
    }
    let prefix: String = chars[..3].iter().collect();
    let suffix: String = chars[len - 4..].iter().collect();
    format!(
        "{}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}{}",
        prefix, suffix
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::config::ConfigState;
    use crate::state::settings::SettingsState;

    #[test]
    fn settings_handles_empty_state() {
        let settings = SettingsState::new();
        let config = ConfigState::new();
        let _theme = ThemeTokens::default();
        assert_eq!(settings.active_tab(), SettingsTab::Provider);
        assert!(config.model().is_empty());
    }

    #[test]
    fn settings_api_key_is_masked() {
        let masked = mask_api_key("sk-abcdefgh12345678abcd");
        assert!(!masked.contains("abcdefgh"));
        assert!(masked.contains("\u{2022}"));
    }

    #[test]
    fn mask_api_key_short_returns_dots() {
        assert_eq!(
            mask_api_key("short"),
            "\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}"
        );
    }

    #[test]
    fn mask_api_key_empty_returns_not_set() {
        assert_eq!(mask_api_key(""), "(not set)");
    }
}
