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
    let tab_labels = vec!["Provider", "Tools", "Web Search", "Reasoning", "Gateway", "Agent"];
    let tab_index = match active {
        SettingsTab::Provider => 0,
        SettingsTab::Tools => 1,
        SettingsTab::WebSearch => 2,
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

    // Hints — context-sensitive
    let hints = if settings.is_editing() {
        Line::from(vec![
            Span::raw(" "),
            Span::styled("Enter", theme.fg_active),
            Span::styled(" confirm  ", theme.fg_dim),
            Span::styled("Esc", theme.fg_active),
            Span::styled(" cancel", theme.fg_dim),
        ])
    } else {
        Line::from(vec![
            Span::raw(" "),
            Span::styled("↑↓", theme.fg_active),
            Span::styled(" navigate  ", theme.fg_dim),
            Span::styled("Enter", theme.fg_active),
            Span::styled(" edit/select  ", theme.fg_dim),
            Span::styled("Space", theme.fg_active),
            Span::styled(" toggle  ", theme.fg_dim),
            Span::styled("Tab", theme.fg_active),
            Span::styled(" switch tab  ", theme.fg_dim),
            Span::styled("Esc", theme.fg_active),
            Span::styled(" close", theme.fg_dim),
        ])
    };
    frame.render_widget(Paragraph::new(hints), chunks[3]);
}

fn render_tab_content<'a>(
    settings: &'a SettingsState,
    config: &'a ConfigState,
    theme: &ThemeTokens,
) -> Vec<Line<'a>> {
    match settings.active_tab() {
        SettingsTab::Provider => render_provider_tab(settings, config, theme),
        SettingsTab::Tools => render_tools_tab(settings, config, theme),
        SettingsTab::WebSearch => render_websearch_tab(settings, config, theme),
        SettingsTab::Reasoning => render_reasoning_tab(settings, config, theme),
        SettingsTab::Gateway => render_gateway_tab(settings, config, theme),
        SettingsTab::Agent => render_agent_tab(settings, config, theme),
    }
}

fn render_provider_tab<'a>(
    settings: &'a SettingsState,
    config: &'a ConfigState,
    theme: &ThemeTokens,
) -> Vec<Line<'a>> {
    let mut lines = Vec::new();

    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled("  Provider", theme.fg_active)));
    lines.push(Line::from(Span::styled(
        "  Select your LLM provider and credentials",
        theme.fg_dim,
    )));
    lines.push(Line::raw(""));

    let provider_val = if config.provider().is_empty() {
        "(not set)".to_string()
    } else {
        config.provider().to_string()
    };
    let base_url_val = if config.base_url().is_empty() {
        "(not set)".to_string()
    } else {
        config.base_url().to_string()
    };
    let model_val = if config.model().is_empty() {
        "(not set)".to_string()
    } else {
        config.model().to_string()
    };
    let api_key_val = mask_api_key(config.api_key());
    let effort_val = if config.reasoning_effort().is_empty() {
        "off".to_string()
    } else {
        config.reasoning_effort().to_string()
    };

    // Field definitions: (index, label, value, field_name, hint)
    let fields: [(usize, &str, String, &str, &str); 5] = [
        (0, "Provider", provider_val, "provider",         " [Enter: pick]"),
        (1, "Base URL", base_url_val, "base_url",         " [Enter: edit]"),
        (2, "API Key",  api_key_val,  "api_key",          " [Enter: edit]"),
        (3, "Model",    model_val,    "model",             " [Enter: pick]"),
        (4, "Effort",   effort_val,   "reasoning_effort",  " [Enter: pick]"),
    ];

    for (idx, label, value, field_name, hint) in &fields {
        let is_selected = settings.field_cursor() == *idx;
        let is_editing =
            settings.is_editing() && settings.editing_field() == Some(*field_name);

        let marker = if is_selected { ">" } else { " " };

        let display_value: String = if is_editing {
            // Show edit buffer with cursor block
            if *field_name == "api_key" {
                // Show raw characters while editing API key
                format!("{}\u{2588}", settings.edit_buffer())
            } else {
                format!("{}\u{2588}", settings.edit_buffer())
            }
        } else {
            value.clone()
        };

        let marker_style = if is_selected {
            theme.accent_primary
        } else {
            theme.fg_dim
        };
        let value_style = if is_editing {
            theme.fg_active
        } else if is_selected {
            theme.accent_primary
        } else {
            theme.fg_active
        };

        let mut spans = vec![
            Span::styled(format!(" {} ", marker), marker_style),
            Span::styled(format!("{:<15} ", label), theme.fg_dim),
            Span::styled(display_value, value_style),
        ];

        // Show hint on selected but not editing
        if is_selected && !is_editing {
            spans.push(Span::styled(*hint, theme.fg_dim));
        }

        lines.push(Line::from(spans));
    }

    lines
}

fn render_tools_tab<'a>(
    settings: &'a SettingsState,
    config: &'a ConfigState,
    theme: &ThemeTokens,
) -> Vec<Line<'a>> {
    let mut lines = Vec::new();

    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled("  Tools", theme.fg_active)));
    lines.push(Line::from(Span::styled(
        "  Enable or disable tool categories",
        theme.fg_dim,
    )));
    lines.push(Line::raw(""));

    let tools: [(bool, &str); 7] = [
        (config.tool_bash, "Terminal / Bash"),
        (config.tool_file_ops, "File Operations"),
        (config.tool_web_search, "Web Search"),
        (config.tool_web_browse, "Web Browse"),
        (config.tool_vision, "Vision"),
        (config.tool_system_info, "System Info"),
        (config.tool_gateway, "Gateway Messaging"),
    ];

    for (i, (enabled, name)) in tools.iter().enumerate() {
        let is_selected = settings.field_cursor() == i;
        let check = if *enabled { "[x]" } else { "[ ]" };
        let marker = if is_selected { "> " } else { "  " };

        let marker_style = if is_selected {
            theme.accent_primary
        } else {
            theme.fg_dim
        };
        let check_style = if *enabled {
            theme.accent_success
        } else {
            theme.fg_dim
        };
        let label_style = if is_selected {
            theme.accent_primary
        } else {
            theme.fg_active
        };

        let mut spans = vec![
            Span::styled(marker, marker_style),
            Span::styled(check, check_style),
            Span::raw(" "),
            Span::styled(*name, label_style),
        ];
        if is_selected {
            spans.push(Span::styled("  [Space: toggle]", theme.fg_dim));
        }
        lines.push(Line::from(spans));
    }

    lines
}

fn render_websearch_tab<'a>(
    settings: &'a SettingsState,
    config: &'a ConfigState,
    theme: &ThemeTokens,
) -> Vec<Line<'a>> {
    let mut lines = Vec::new();

    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled("  Web Search", theme.fg_active)));
    lines.push(Line::from(Span::styled(
        "  Configure web search tool and providers",
        theme.fg_dim,
    )));
    lines.push(Line::raw(""));

    // Field 0: web_search_enabled (checkbox — mirrors tool_web_search)
    {
        let is_selected = settings.field_cursor() == 0;
        let marker = if is_selected { "> " } else { "  " };
        let marker_style = if is_selected { theme.accent_primary } else { theme.fg_dim };
        let check = if config.tool_web_search { "[x]" } else { "[ ]" };
        let check_style = if config.tool_web_search { theme.accent_success } else { theme.fg_dim };
        let label_style = if is_selected { theme.accent_primary } else { theme.fg_active };
        let mut spans = vec![
            Span::styled(marker, marker_style),
            Span::styled(check, check_style),
            Span::raw(" "),
            Span::styled("Enable Web Search", label_style),
        ];
        if is_selected {
            spans.push(Span::styled("  [Space: toggle]", theme.fg_dim));
        }
        lines.push(Line::from(spans));
    }

    // Field 1: search_provider (cycle on Enter)
    {
        let is_selected = settings.field_cursor() == 1;
        let marker = if is_selected { "> " } else { "  " };
        let marker_style = if is_selected { theme.accent_primary } else { theme.fg_dim };
        let provider_val = if config.search_provider.is_empty() || config.search_provider == "none" {
            "none".to_string()
        } else {
            config.search_provider.clone()
        };
        let value_style = if is_selected { theme.accent_primary } else { theme.fg_active };
        let mut spans = vec![
            Span::styled(marker, marker_style),
            Span::styled(format!("{:<16} ", "Provider:"), theme.fg_dim),
            Span::styled(provider_val, value_style),
        ];
        if is_selected {
            spans.push(Span::styled("  [Enter: cycle]", theme.fg_dim));
        }
        lines.push(Line::from(spans));
    }

    // Fields 2–4: API keys (masked, inline edit)
    let api_key_fields: [(usize, &str, &str, &str); 3] = [
        (2, "Firecrawl Key:  ", config.firecrawl_api_key.as_str(), "firecrawl_api_key"),
        (3, "Exa Key:        ", config.exa_api_key.as_str(),       "exa_api_key"),
        (4, "Tavily Key:     ", config.tavily_api_key.as_str(),    "tavily_api_key"),
    ];

    for (idx, label, value, field_name) in &api_key_fields {
        let is_selected = settings.field_cursor() == *idx;
        let is_editing = settings.is_editing() && settings.editing_field() == Some(field_name);
        let marker = if is_selected { "> " } else { "  " };
        let marker_style = if is_selected { theme.accent_primary } else { theme.fg_dim };

        let display_value: String = if is_editing {
            format!("{}\u{2588}", settings.edit_buffer())
        } else if value.is_empty() {
            "(not set)".to_string()
        } else {
            mask_api_key(value)
        };

        let value_style = if is_editing {
            theme.fg_active
        } else if is_selected {
            theme.accent_primary
        } else {
            theme.fg_active
        };

        let mut spans = vec![
            Span::styled(marker, marker_style),
            Span::styled(format!("{:<16} ", label), theme.fg_dim),
            Span::styled(display_value, value_style),
        ];
        if is_selected && !is_editing {
            spans.push(Span::styled("  [Enter: edit]", theme.fg_dim));
        }
        lines.push(Line::from(spans));
    }

    // Field 5: search_max_results (numeric inline edit)
    {
        let is_selected = settings.field_cursor() == 5;
        let is_editing = settings.is_editing() && settings.editing_field() == Some("search_max_results");
        let marker = if is_selected { "> " } else { "  " };
        let marker_style = if is_selected { theme.accent_primary } else { theme.fg_dim };

        let display_value: String = if is_editing {
            format!("{}\u{2588}", settings.edit_buffer())
        } else {
            config.search_max_results.to_string()
        };

        let value_style = if is_editing {
            theme.fg_active
        } else if is_selected {
            theme.accent_primary
        } else {
            theme.fg_active
        };

        let mut spans = vec![
            Span::styled(marker, marker_style),
            Span::styled(format!("{:<16} ", "Max Results:"), theme.fg_dim),
            Span::styled(display_value, value_style),
        ];
        if is_selected && !is_editing {
            spans.push(Span::styled("  [Enter: edit]", theme.fg_dim));
        }
        lines.push(Line::from(spans));
    }

    // Field 6: search_timeout_secs (numeric inline edit)
    {
        let is_selected = settings.field_cursor() == 6;
        let is_editing = settings.is_editing() && settings.editing_field() == Some("search_timeout");
        let marker = if is_selected { "> " } else { "  " };
        let marker_style = if is_selected { theme.accent_primary } else { theme.fg_dim };

        let display_value: String = if is_editing {
            format!("{}\u{2588}", settings.edit_buffer())
        } else {
            format!("{}s", config.search_timeout_secs)
        };

        let value_style = if is_editing {
            theme.fg_active
        } else if is_selected {
            theme.accent_primary
        } else {
            theme.fg_active
        };

        let mut spans = vec![
            Span::styled(marker, marker_style),
            Span::styled(format!("{:<16} ", "Timeout:"), theme.fg_dim),
            Span::styled(display_value, value_style),
        ];
        if is_selected && !is_editing {
            spans.push(Span::styled("  [Enter: edit]", theme.fg_dim));
        }
        lines.push(Line::from(spans));
    }

    lines
}

fn render_reasoning_tab<'a>(
    settings: &'a SettingsState,
    config: &'a ConfigState,
    theme: &ThemeTokens,
) -> Vec<Line<'a>> {
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
        "off"
    } else {
        current_effort
    };

    // Only one field: effort
    let is_selected = settings.field_cursor() == 0;
    let marker = if is_selected { "> " } else { "  " };
    let marker_style = if is_selected { theme.accent_primary } else { theme.fg_dim };
    let value_style = if is_selected { theme.accent_primary } else { theme.accent_secondary };

    let mut spans = vec![
        Span::styled(marker, marker_style),
        Span::styled("Effort:  ", theme.fg_dim),
        Span::styled(effort_display.to_string(), value_style),
    ];
    if is_selected {
        spans.push(Span::styled("  [Enter: pick]", theme.fg_dim));
    }
    lines.push(Line::from(spans));

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

fn render_gateway_tab<'a>(
    settings: &'a SettingsState,
    config: &'a ConfigState,
    theme: &ThemeTokens,
) -> Vec<Line<'a>> {
    let mut lines = Vec::new();

    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled("  Gateway", theme.fg_active)));
    lines.push(Line::from(Span::styled(
        "  Messaging platform connections",
        theme.fg_dim,
    )));
    lines.push(Line::raw(""));

    // Field 0: gateway_enabled (checkbox)
    {
        let is_selected = settings.field_cursor() == 0;
        let marker = if is_selected { "> " } else { "  " };
        let marker_style = if is_selected { theme.accent_primary } else { theme.fg_dim };
        let check = if config.gateway_enabled { "[x]" } else { "[ ]" };
        let check_style = if config.gateway_enabled { theme.accent_success } else { theme.fg_dim };
        let label_style = if is_selected { theme.accent_primary } else { theme.fg_active };
        let mut spans = vec![
            Span::styled(marker, marker_style),
            Span::styled(check, check_style),
            Span::raw(" "),
            Span::styled("Gateway Enabled", label_style),
        ];
        if is_selected {
            spans.push(Span::styled("  [Space: toggle]", theme.fg_dim));
        }
        lines.push(Line::from(spans));
    }

    // Text fields: index, label, value, field_name
    let text_fields: [(usize, &str, &str, &str); 4] = [
        (1, "Slack Token   ", config.slack_token.as_str(),    "slack_token"),
        (2, "Telegram Token", config.telegram_token.as_str(), "telegram_token"),
        (3, "Discord Token ", config.discord_token.as_str(),  "discord_token"),
        (4, "Command Prefix", config.gateway_prefix.as_str(), "gateway_prefix"),
    ];

    for (idx, label, value, field_name) in &text_fields {
        let is_selected = settings.field_cursor() == *idx;
        let is_editing = settings.is_editing() && settings.editing_field() == Some(field_name);
        let marker = if is_selected { "> " } else { "  " };
        let marker_style = if is_selected { theme.accent_primary } else { theme.fg_dim };

        let display_value: String = if is_editing {
            format!("{}\u{2588}", settings.edit_buffer())
        } else if value.is_empty() {
            "(not set)".to_string()
        } else {
            // Mask tokens; prefix visible
            let v = *value;
            let chars: Vec<char> = v.chars().collect();
            if chars.len() > 8 {
                let prefix: String = chars[..4].iter().collect();
                format!("{}\u{2022}\u{2022}\u{2022}\u{2022}", prefix)
            } else {
                v.to_string()
            }
        };

        let value_style = if is_editing {
            theme.fg_active
        } else if is_selected {
            theme.accent_primary
        } else {
            theme.fg_active
        };

        let mut spans = vec![
            Span::styled(marker, marker_style),
            Span::styled(format!("{:<16} ", label), theme.fg_dim),
            Span::styled(display_value, value_style),
        ];
        if is_selected && !is_editing {
            spans.push(Span::styled("  [Enter: edit]", theme.fg_dim));
        }
        lines.push(Line::from(spans));
    }

    lines
}

fn render_agent_tab<'a>(
    settings: &'a SettingsState,
    config: &'a ConfigState,
    theme: &ThemeTokens,
) -> Vec<Line<'a>> {
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

    let system_prompt = if let Some(raw) = config.agent_config_raw() {
        raw.get("system_prompt")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    } else {
        String::new()
    };

    // (field_index, label, value, field_name, hint)
    let editable_fields: [(usize, &str, String, &str, &str); 2] = [
        (0, "Agent Name    ", agent_name,      "agent_name",   " [Enter: edit]"),
        (1, "System Prompt ", system_prompt,   "system_prompt"," [Enter: edit]"),
    ];

    for (idx, label, value, field_name, hint) in &editable_fields {
        let is_selected = settings.field_cursor() == *idx;
        let is_editing = settings.is_editing() && settings.editing_field() == Some(field_name);
        let marker = if is_selected { "> " } else { "  " };
        let marker_style = if is_selected { theme.accent_primary } else { theme.fg_dim };

        let display_value: String = if is_editing {
            format!("{}\u{2588}", settings.edit_buffer())
        } else if value.is_empty() {
            "(not set)".to_string()
        } else {
            // Truncate long values for display
            let v = value.as_str();
            if v.chars().count() > 40 {
                let truncated: String = v.chars().take(37).collect();
                format!("{}...", truncated)
            } else {
                v.to_string()
            }
        };

        let value_style = if is_editing {
            theme.fg_active
        } else if is_selected {
            theme.accent_primary
        } else {
            theme.fg_active
        };

        let mut spans = vec![
            Span::styled(marker, marker_style),
            Span::styled(format!("{:<16} ", label), theme.fg_dim),
            Span::styled(display_value, value_style),
        ];
        if is_selected && !is_editing {
            spans.push(Span::styled(*hint, theme.fg_dim));
        }
        lines.push(Line::from(spans));
    }

    // Field 2: backend (read-only)
    {
        let is_selected = settings.field_cursor() == 2;
        let marker = if is_selected { "> " } else { "  " };
        let marker_style = if is_selected { theme.accent_primary } else { theme.fg_dim };
        let value_style = if is_selected { theme.accent_primary } else { theme.fg_dim };
        lines.push(Line::from(vec![
            Span::styled(marker, marker_style),
            Span::styled("Backend           ", theme.fg_dim),
            Span::styled("daemon", value_style),
        ]));
    }

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
        assert_eq!(config.model(), "gpt-5.4");
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
