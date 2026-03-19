use crate::theme::{ThemeTokens, SHARP_BORDER, RESET};
use crate::state::settings::{SettingsState, SettingsTab};
use crate::state::config::ConfigState;

/// Render the settings overlay.
/// Returns a full-screen Vec<String> (one entry per row) centered over the terminal.
pub fn settings_widget(
    settings: &SettingsState,
    config: &ConfigState,
    theme: &ThemeTokens,
    screen_width: usize,
    screen_height: usize,
) -> Vec<String> {
    let bc = theme.accent_secondary.fg(); // amber border
    let b = &SHARP_BORDER;

    // Size: ~75% width, ~80% height, centered
    let panel_w = (screen_width * 75 / 100).max(60).min(screen_width);
    let panel_h = (screen_height * 80 / 100).max(20).min(screen_height);
    let inner_w = panel_w.saturating_sub(2);
    let inner_h = panel_h.saturating_sub(2);

    let x_pad = (screen_width.saturating_sub(panel_w)) / 2;
    let y_pad = (screen_height.saturating_sub(panel_h)) / 2;

    let mut result = Vec::new();

    // Top padding
    for _ in 0..y_pad {
        result.push(" ".repeat(screen_width));
    }

    // Top border with title
    let title = " SETTINGS ";
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
        " ".repeat(screen_width.saturating_sub(x_pad + panel_w)),
    ));

    // Tab bar line
    let tab_line = render_tab_bar(settings, theme, inner_w);
    let padded_tab = super::pad_to_width(&tab_line, inner_w);
    result.push(format!(
        "{}{}{}{}{}{}{}",
        " ".repeat(x_pad),
        bc, b.vertical,
        padded_tab,
        b.vertical,
        RESET,
        " ".repeat(screen_width.saturating_sub(x_pad + panel_w)),
    ));

    // Separator below tabs
    result.push(format!(
        "{}{}{}{}{}{}{}",
        " ".repeat(x_pad),
        bc, b.vertical,
        super::repeat_char('─', inner_w),
        b.vertical,
        RESET,
        " ".repeat(screen_width.saturating_sub(x_pad + panel_w)),
    ));

    // Content area: inner_h minus tab bar, separator, hints, bottom border
    let content_h = inner_h.saturating_sub(3); // tab bar + separator + hints
    let content_lines = render_tab_content(settings, config, theme, inner_w, content_h);

    for i in 0..content_h {
        let line = content_lines.get(i).cloned().unwrap_or_default();
        let padded = super::pad_to_width(&line, inner_w);
        result.push(format!(
            "{}{}{}{}{}{}{}",
            " ".repeat(x_pad),
            bc, b.vertical,
            padded,
            b.vertical,
            RESET,
            " ".repeat(screen_width.saturating_sub(x_pad + panel_w)),
        ));
    }

    // Hints line
    let hints = format!(
        " {}Tab{} switch tab  {}Esc{} close",
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
        " ".repeat(screen_width.saturating_sub(x_pad + panel_w)),
    ));

    // Bottom border
    result.push(format!(
        "{}{}{}{}{}{}{}",
        " ".repeat(x_pad),
        bc, b.bottom_left,
        super::repeat_char(b.horizontal, inner_w),
        b.bottom_right,
        RESET,
        " ".repeat(screen_width.saturating_sub(x_pad + panel_w)),
    ));

    // Bottom padding
    while result.len() < screen_height {
        result.push(" ".repeat(screen_width));
    }
    result.truncate(screen_height);

    result
}

fn render_tab_bar(settings: &SettingsState, theme: &ThemeTokens, _inner_w: usize) -> String {
    let active = settings.active_tab();
    let tabs = [
        (SettingsTab::Provider, "Provider"),
        (SettingsTab::Model, "Model"),
        (SettingsTab::Tools, "Tools"),
        (SettingsTab::Reasoning, "Reasoning"),
        (SettingsTab::Gateway, "Gateway"),
        (SettingsTab::Agent, "Agent"),
    ];

    let mut parts = Vec::new();
    for (tab, label) in &tabs {
        if *tab == active {
            // Active: bright white with brackets
            parts.push(format!(
                "{}[{}]{}",
                theme.fg_active.fg(),
                label,
                RESET,
            ));
        } else {
            // Inactive: dim
            parts.push(format!(
                "{} {} {}",
                theme.fg_dim.fg(),
                label,
                RESET,
            ));
        }
    }

    format!(" {}", parts.join(" "))
}

fn render_tab_content(
    settings: &SettingsState,
    config: &ConfigState,
    theme: &ThemeTokens,
    inner_w: usize,
    content_h: usize,
) -> Vec<String> {
    match settings.active_tab() {
        SettingsTab::Provider => render_provider_tab(config, theme, inner_w, content_h),
        SettingsTab::Model => render_model_tab(config, theme, inner_w, content_h),
        SettingsTab::Tools => render_tools_tab(theme, inner_w, content_h),
        SettingsTab::Reasoning => render_reasoning_tab(config, theme, inner_w, content_h),
        SettingsTab::Gateway => render_gateway_tab(theme, inner_w, content_h),
        SettingsTab::Agent => render_agent_tab(config, theme, inner_w, content_h),
    }
}

fn render_provider_tab(
    config: &ConfigState,
    theme: &ThemeTokens,
    _inner_w: usize,
    content_h: usize,
) -> Vec<String> {
    let mut lines = Vec::new();

    // Blank line
    lines.push(String::new());

    // Section header
    lines.push(format!(
        "  {}Provider{}",
        theme.fg_active.fg(), RESET,
    ));
    lines.push(format!(
        "  {}Select your LLM provider and credentials{}",
        theme.fg_dim.fg(), RESET,
    ));

    // Blank line
    lines.push(String::new());

    // Fields
    let provider = if config.provider().is_empty() { "(not set)" } else { config.provider() };
    let base_url = if config.base_url().is_empty() { "(not set)" } else { config.base_url() };
    let model = if config.model().is_empty() { "(not set)" } else { config.model() };

    // Masked API key
    let api_key_display = mask_api_key(config.api_key());

    lines.push(format!(
        "  {}Active Provider:{}  {} ▾ {}{}",
        theme.fg_dim.fg(), RESET,
        theme.fg_active.fg(), provider, RESET,
    ));
    lines.push(format!(
        "  {}Base URL:        {}  {}{}{}",
        theme.fg_dim.fg(), RESET,
        theme.fg_active.fg(), base_url, RESET,
    ));
    lines.push(format!(
        "  {}API Key:         {}  {}{}{} {}[show]{}",
        theme.fg_dim.fg(), RESET,
        theme.fg_active.fg(), api_key_display, RESET,
        theme.fg_dim.fg(), RESET,
    ));
    lines.push(format!(
        "  {}Model:           {}  {}{}{}",
        theme.fg_dim.fg(), RESET,
        theme.fg_active.fg(), model, RESET,
    ));

    // Pad remaining rows
    while lines.len() < content_h {
        lines.push(String::new());
    }
    lines.truncate(content_h);
    lines
}

fn render_model_tab(
    config: &ConfigState,
    theme: &ThemeTokens,
    _inner_w: usize,
    content_h: usize,
) -> Vec<String> {
    let mut lines = Vec::new();

    lines.push(String::new());
    lines.push(format!("  {}Model{}", theme.fg_active.fg(), RESET));
    lines.push(format!("  {}Select model for current provider{}", theme.fg_dim.fg(), RESET));
    lines.push(String::new());

    let current = if config.model().is_empty() { "(not set)" } else { config.model() };
    lines.push(format!(
        "  {}Current:{}   {}{}{}",
        theme.fg_dim.fg(), RESET,
        theme.fg_active.fg(), current, RESET,
    ));

    let models = config.fetched_models();
    if models.is_empty() {
        lines.push(format!(
            "  {}Available:{}  {}(press Enter to fetch){}",
            theme.fg_dim.fg(), RESET,
            theme.fg_dim.fg(), RESET,
        ));
    } else {
        lines.push(format!(
            "  {}Available:{}",
            theme.fg_dim.fg(), RESET,
        ));
        for m in models {
            let display_name = m.name.as_deref().unwrap_or(&m.id);
            let is_active = m.id == config.model() || display_name == config.model();
            if is_active {
                lines.push(format!(
                    "    {}> {}{}",
                    theme.accent_secondary.fg(), display_name, RESET,
                ));
            } else {
                lines.push(format!(
                    "    {}  {}{}",
                    theme.fg_dim.fg(), display_name, RESET,
                ));
            }
        }
    }

    while lines.len() < content_h {
        lines.push(String::new());
    }
    lines.truncate(content_h);
    lines
}

fn render_tools_tab(theme: &ThemeTokens, _inner_w: usize, content_h: usize) -> Vec<String> {
    let mut lines = Vec::new();

    lines.push(String::new());
    lines.push(format!("  {}Tools{}", theme.fg_active.fg(), RESET));
    lines.push(format!("  {}Enable or disable tool categories{}", theme.fg_dim.fg(), RESET));
    lines.push(String::new());

    // Default tool categories (all enabled by default except Web Browse and Messaging Gateway)
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
            format!("{}[x]{}", theme.accent_success.fg(), RESET)
        } else {
            format!("{}[ ]{}", theme.fg_dim.fg(), RESET)
        };
        lines.push(format!(
            "  {} {}{}{}",
            checkbox,
            theme.fg_active.fg(), name, RESET,
        ));
    }

    while lines.len() < content_h {
        lines.push(String::new());
    }
    lines.truncate(content_h);
    lines
}

fn render_reasoning_tab(
    config: &ConfigState,
    theme: &ThemeTokens,
    _inner_w: usize,
    content_h: usize,
) -> Vec<String> {
    let mut lines = Vec::new();

    lines.push(String::new());
    lines.push(format!("  {}Reasoning{}", theme.fg_active.fg(), RESET));
    lines.push(format!("  {}Configure extended thinking{}", theme.fg_dim.fg(), RESET));
    lines.push(String::new());

    let current_effort = config.reasoning_effort();
    let effort_display = if current_effort.is_empty() { "Medium" } else { current_effort };

    lines.push(format!(
        "  {}Effort:{}  {}(●) {}{}  {}← current{}",
        theme.fg_dim.fg(), RESET,
        theme.accent_secondary.fg(), effort_display, RESET,
        theme.fg_dim.fg(), RESET,
    ));
    lines.push(String::new());
    lines.push(format!(
        "  {}Options:{}  {}Off / Minimal / Low / Medium / High / Extra High{}",
        theme.fg_dim.fg(), RESET,
        theme.fg_dim.fg(), RESET,
    ));

    while lines.len() < content_h {
        lines.push(String::new());
    }
    lines.truncate(content_h);
    lines
}

fn render_gateway_tab(theme: &ThemeTokens, _inner_w: usize, content_h: usize) -> Vec<String> {
    let mut lines = Vec::new();

    lines.push(String::new());
    lines.push(format!("  {}Gateway{}", theme.fg_active.fg(), RESET));
    lines.push(format!("  {}Messaging platform connections{}", theme.fg_dim.fg(), RESET));
    lines.push(String::new());

    lines.push(format!(
        "  {}Gateway Enabled:{}  {}[x] Yes{}",
        theme.fg_dim.fg(), RESET,
        theme.accent_success.fg(), RESET,
    ));

    while lines.len() < content_h {
        lines.push(String::new());
    }
    lines.truncate(content_h);
    lines
}

fn render_agent_tab(
    config: &ConfigState,
    theme: &ThemeTokens,
    _inner_w: usize,
    content_h: usize,
) -> Vec<String> {
    let mut lines = Vec::new();

    lines.push(String::new());
    lines.push(format!("  {}Agent{}", theme.fg_active.fg(), RESET));
    lines.push(format!("  {}Agent identity and behavior{}", theme.fg_dim.fg(), RESET));
    lines.push(String::new());

    // Derive agent name from raw config, or fall back to "Sisyphus"
    let agent_name = if let Some(raw) = config.agent_config_raw() {
        raw.get("agent_name")
            .and_then(|v| v.as_str())
            .unwrap_or("Sisyphus")
            .to_string()
    } else {
        "Sisyphus".to_string()
    };

    lines.push(format!(
        "  {}Agent Name:{}  {}{}{}",
        theme.fg_dim.fg(), RESET,
        theme.fg_active.fg(), agent_name, RESET,
    ));
    lines.push(format!(
        "  {}Backend:{}     {}daemon{}",
        theme.fg_dim.fg(), RESET,
        theme.fg_active.fg(), RESET,
    ));

    while lines.len() < content_h {
        lines.push(String::new());
    }
    lines.truncate(content_h);
    lines
}

/// Mask an API key: show first 3 chars, dots, last 4 chars.
/// If key is short, show dots only.
fn mask_api_key(key: &str) -> String {
    if key.is_empty() {
        return "(not set)".to_string();
    }
    let chars: Vec<char> = key.chars().collect();
    let len = chars.len();
    if len <= 7 {
        return "••••••••".to_string();
    }
    let prefix: String = chars[..3].iter().collect();
    let suffix: String = chars[len - 4..].iter().collect();
    format!("{}••••••••{}", prefix, suffix)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::settings::{SettingsState, SettingsAction};
    use crate::state::config::{ConfigState, ConfigAction, AgentConfigSnapshot};
    use crate::theme::ThemeTokens;

    fn make_config() -> ConfigState {
        let mut cfg = ConfigState::new();
        cfg.reduce(ConfigAction::ConfigReceived(AgentConfigSnapshot {
            provider: "OpenAI".into(),
            base_url: "https://api.openai.com/v1".into(),
            model: "gpt-4o".into(),
            api_key: "sk-abcdefgh12345678abcd".into(),
            reasoning_effort: "medium".into(),
        }));
        cfg
    }

    #[test]
    fn settings_widget_returns_correct_height() {
        let settings = SettingsState::new();
        let config = ConfigState::new();
        let theme = ThemeTokens::default();
        let lines = settings_widget(&settings, &config, &theme, 120, 40);
        assert_eq!(lines.len(), 40);
    }

    #[test]
    fn settings_widget_contains_title() {
        let settings = SettingsState::new();
        let config = ConfigState::new();
        let theme = ThemeTokens::default();
        let lines = settings_widget(&settings, &config, &theme, 120, 40);
        let joined = lines.join("");
        assert!(joined.contains("SETTINGS"));
    }

    #[test]
    fn settings_widget_shows_tab_bar() {
        let settings = SettingsState::new();
        let config = ConfigState::new();
        let theme = ThemeTokens::default();
        let lines = settings_widget(&settings, &config, &theme, 120, 40);
        let joined = lines.join("");
        assert!(joined.contains("Provider"));
        assert!(joined.contains("Model"));
        assert!(joined.contains("Tools"));
        assert!(joined.contains("Reasoning"));
        assert!(joined.contains("Gateway"));
        assert!(joined.contains("Agent"));
    }

    #[test]
    fn settings_widget_provider_tab_shows_config() {
        let settings = SettingsState::new();
        let config = make_config();
        let theme = ThemeTokens::default();
        let lines = settings_widget(&settings, &config, &theme, 120, 40);
        let joined = lines.join("");
        assert!(joined.contains("OpenAI"));
        assert!(joined.contains("gpt-4o"));
        assert!(joined.contains("api.openai.com"));
    }

    #[test]
    fn settings_widget_switches_to_model_tab() {
        let mut settings = SettingsState::new();
        settings.reduce(SettingsAction::SwitchTab(SettingsTab::Model));
        let config = make_config();
        let theme = ThemeTokens::default();
        let lines = settings_widget(&settings, &config, &theme, 120, 40);
        let joined = lines.join("");
        assert!(joined.contains("Select model for current provider"));
        assert!(joined.contains("Current:"));
    }

    #[test]
    fn settings_widget_tools_tab_shows_checkboxes() {
        let mut settings = SettingsState::new();
        settings.reduce(SettingsAction::SwitchTab(SettingsTab::Tools));
        let config = ConfigState::new();
        let theme = ThemeTokens::default();
        let lines = settings_widget(&settings, &config, &theme, 120, 40);
        let joined = lines.join("");
        assert!(joined.contains("Terminal / Bash"));
        assert!(joined.contains("File Operations"));
        assert!(joined.contains("Web Search"));
    }

    #[test]
    fn settings_widget_reasoning_tab() {
        let mut settings = SettingsState::new();
        settings.reduce(SettingsAction::SwitchTab(SettingsTab::Reasoning));
        let config = make_config();
        let theme = ThemeTokens::default();
        let lines = settings_widget(&settings, &config, &theme, 120, 40);
        let joined = lines.join("");
        assert!(joined.contains("Configure extended thinking"));
        assert!(joined.contains("Off / Minimal / Low / Medium / High / Extra High"));
    }

    #[test]
    fn settings_widget_gateway_tab() {
        let mut settings = SettingsState::new();
        settings.reduce(SettingsAction::SwitchTab(SettingsTab::Gateway));
        let config = ConfigState::new();
        let theme = ThemeTokens::default();
        let lines = settings_widget(&settings, &config, &theme, 120, 40);
        let joined = lines.join("");
        assert!(joined.contains("Messaging platform connections"));
        assert!(joined.contains("Gateway Enabled"));
    }

    #[test]
    fn settings_widget_agent_tab() {
        let mut settings = SettingsState::new();
        settings.reduce(SettingsAction::SwitchTab(SettingsTab::Agent));
        let config = ConfigState::new();
        let theme = ThemeTokens::default();
        let lines = settings_widget(&settings, &config, &theme, 120, 40);
        let joined = lines.join("");
        assert!(joined.contains("Agent identity and behavior"));
        assert!(joined.contains("Sisyphus"));
        assert!(joined.contains("daemon"));
    }

    #[test]
    fn settings_widget_api_key_is_masked() {
        let settings = SettingsState::new();
        let config = make_config();
        let theme = ThemeTokens::default();
        let lines = settings_widget(&settings, &config, &theme, 120, 40);
        let joined = lines.join("");
        // Should NOT contain the raw key
        assert!(!joined.contains("sk-abcdefgh12345678abcd"));
        // Should contain masked dots
        assert!(joined.contains("••••••••"));
    }

    #[test]
    fn mask_api_key_short_returns_dots() {
        assert_eq!(mask_api_key("short"), "••••••••");
    }

    #[test]
    fn mask_api_key_long_shows_prefix_and_suffix() {
        let masked = mask_api_key("sk-abcdefghijklmnopabcd");
        assert!(masked.starts_with("sk-"));
        assert!(masked.ends_with("abcd"));
        assert!(masked.contains("••••••••"));
    }

    #[test]
    fn mask_api_key_empty_returns_not_set() {
        assert_eq!(mask_api_key(""), "(not set)");
    }

    #[test]
    fn settings_widget_model_tab_with_fetched_models() {
        let mut settings = SettingsState::new();
        settings.reduce(SettingsAction::SwitchTab(SettingsTab::Model));
        let mut config = make_config();
        config.reduce(ConfigAction::ModelsFetched(vec![
            crate::state::config::FetchedModel {
                id: "gpt-4o".into(),
                name: Some("GPT-4o".into()),
                context_window: Some(128_000),
            },
            crate::state::config::FetchedModel {
                id: "gpt-4o-mini".into(),
                name: Some("GPT-4o Mini".into()),
                context_window: Some(128_000),
            },
        ]));
        let theme = ThemeTokens::default();
        let lines = settings_widget(&settings, &config, &theme, 120, 40);
        let joined = lines.join("");
        assert!(joined.contains("GPT-4o"));
        assert!(joined.contains("GPT-4o Mini"));
    }
}
