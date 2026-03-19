#![allow(dead_code)]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color(pub u8); // ANSI-256 index

impl Color {
    pub const RESET: Self = Self(0);

    /// Emit ANSI foreground escape sequence
    pub fn fg(self) -> String {
        if self.0 == 0 {
            "\x1b[0m".to_string()
        } else {
            format!("\x1b[38;5;{}m", self.0)
        }
    }

    /// Emit ANSI background escape sequence
    pub fn bg(self) -> String {
        if self.0 == 0 {
            "\x1b[0m".to_string()
        } else {
            format!("\x1b[48;5;{}m", self.0)
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ThemeTokens {
    pub bg_main: Color,           // terminal default
    pub fg_dim: Color,            // Indexed(245) — inactive text, borders
    pub fg_active: Color,         // Indexed(255) — bright active text
    pub accent_primary: Color,    // Indexed(75) — cyan, focus ring, user msgs
    pub accent_assistant: Color,  // Indexed(183) — lavender, assistant msgs
    pub accent_secondary: Color,  // Indexed(178) — amber, warnings, menu highlights
    pub accent_success: Color,    // Indexed(78) — green, completed, OK
    pub accent_danger: Color,     // Indexed(203) — red, errors, critical risk
}

impl Default for ThemeTokens {
    fn default() -> Self {
        Self {
            bg_main: Color::RESET,
            fg_dim: Color(245),
            fg_active: Color(255),
            accent_primary: Color(75),
            accent_assistant: Color(183),
            accent_secondary: Color(178),
            accent_success: Color(78),
            accent_danger: Color(203),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct BorderSet {
    pub top_left: char,
    pub top_right: char,
    pub bottom_left: char,
    pub bottom_right: char,
    pub horizontal: char,
    pub vertical: char,
}

pub const ROUNDED_BORDER: BorderSet = BorderSet {
    top_left: '╭',
    top_right: '╮',
    bottom_left: '╰',
    bottom_right: '╯',
    horizontal: '─',
    vertical: '│',
};

pub const SHARP_BORDER: BorderSet = BorderSet {
    top_left: '╔',
    top_right: '╗',
    bottom_left: '╚',
    bottom_right: '╝',
    horizontal: '═',
    vertical: '║',
};

/// Reset all ANSI attributes
pub const RESET: &str = "\x1b[0m";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_theme_has_all_tokens() {
        let theme = ThemeTokens::default();
        assert_ne!(theme.fg_dim.0, theme.accent_primary.0);
        assert_ne!(theme.accent_danger.0, theme.accent_success.0);
    }

    #[test]
    fn border_sets_have_correct_chars() {
        assert_eq!(ROUNDED_BORDER.top_left, '╭');
        assert_eq!(ROUNDED_BORDER.top_right, '╮');
        assert_eq!(SHARP_BORDER.top_left, '╔');
        assert_eq!(SHARP_BORDER.bottom_right, '╝');
    }
}
