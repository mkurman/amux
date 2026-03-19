#![allow(dead_code)]

use ratatui::style::{Color, Style};
use ratatui::widgets::BorderType;

#[derive(Debug, Clone, Copy)]
pub struct ThemeTokens {
    pub fg_dim: Style,
    pub fg_active: Style,
    pub accent_primary: Style,     // cyan
    pub accent_assistant: Style,   // lavender
    pub accent_secondary: Style,   // amber
    pub accent_success: Style,     // green
    pub accent_danger: Style,      // red
}

impl Default for ThemeTokens {
    fn default() -> Self {
        Self {
            fg_dim: Style::default().fg(Color::Indexed(245)),
            fg_active: Style::default().fg(Color::Indexed(255)),
            accent_primary: Style::default().fg(Color::Indexed(75)),
            accent_assistant: Style::default().fg(Color::Indexed(183)),
            accent_secondary: Style::default().fg(Color::Indexed(178)),
            accent_success: Style::default().fg(Color::Indexed(78)),
            accent_danger: Style::default().fg(Color::Indexed(203)),
        }
    }
}

pub const ROUNDED_BORDER: BorderType = BorderType::Rounded;
pub const SHARP_BORDER: BorderType = BorderType::Double;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_theme_has_all_tokens() {
        let theme = ThemeTokens::default();
        assert_ne!(theme.fg_dim, theme.accent_primary);
        assert_ne!(theme.accent_danger, theme.accent_success);
    }
}
