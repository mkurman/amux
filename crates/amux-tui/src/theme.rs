#![allow(dead_code)]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color(pub u8); // ANSI-256 index

/// Closing tag for foreground color markup
pub const FG_CLOSE: &str = "[/fg]";

/// Closing tag for background color markup
pub const BG_CLOSE: &str = "[/bg]";

/// Empty string replacing the old ANSI RESET constant.
/// Use FG_CLOSE / BG_CLOSE to close specific tags instead.
pub const RESET: &str = "";

impl Color {
    pub const RESET: Self = Self(0);

    /// Emit ftui markup opening tag for foreground color
    pub fn fg(self) -> String {
        if self.0 == 0 {
            FG_CLOSE.to_string()
        } else {
            let (r, g, b) = ansi256_to_rgb(self.0);
            format!("[fg=rgb({},{},{})]", r, g, b)
        }
    }

    /// Emit ftui markup opening tag for background color
    pub fn bg(self) -> String {
        if self.0 == 0 {
            BG_CLOSE.to_string()
        } else {
            let (r, g, b) = ansi256_to_rgb(self.0);
            format!("[bg=rgb({},{},{})]", r, g, b)
        }
    }
}

/// Convert ANSI-256 color index to RGB values
fn ansi256_to_rgb(idx: u8) -> (u8, u8, u8) {
    match idx {
        // Standard colors (0-15) - approximate common terminal colors
        0 => (0, 0, 0),
        1 => (170, 0, 0),
        2 => (0, 170, 0),
        3 => (170, 170, 0),
        4 => (0, 0, 170),
        5 => (170, 0, 170),
        6 => (0, 170, 170),
        7 => (170, 170, 170),
        8 => (85, 85, 85),
        9 => (255, 85, 85),
        10 => (85, 255, 85),
        11 => (255, 255, 85),
        12 => (85, 85, 255),
        13 => (255, 85, 255),
        14 => (85, 255, 255),
        15 => (255, 255, 255),
        // 216-color cube (16-231)
        16..=231 => {
            let idx = idx - 16;
            let r = (idx / 36) % 6;
            let g = (idx / 6) % 6;
            let b = idx % 6;
            let to_val = |v: u8| if v == 0 { 0 } else { 55 + 40 * v };
            (to_val(r), to_val(g), to_val(b))
        }
        // Grayscale (232-255)
        232..=255 => {
            let v = 8 + 10 * (idx - 232);
            (v, v, v)
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

    #[test]
    fn color_fg_emits_markup_tag() {
        let c = Color(75);
        let tag = c.fg();
        assert!(tag.starts_with("[fg=rgb("));
        assert!(tag.ends_with(")]"));
    }

    #[test]
    fn color_bg_emits_markup_tag() {
        let c = Color(75);
        let tag = c.bg();
        assert!(tag.starts_with("[bg=rgb("));
        assert!(tag.ends_with(")]"));
    }

    #[test]
    fn color_reset_fg_emits_close_tag() {
        let c = Color::RESET;
        assert_eq!(c.fg(), "[/fg]");
    }

    #[test]
    fn color_reset_bg_emits_close_tag() {
        let c = Color::RESET;
        assert_eq!(c.bg(), "[/bg]");
    }

    #[test]
    fn ansi256_standard_colors() {
        assert_eq!(ansi256_to_rgb(0), (0, 0, 0));
        assert_eq!(ansi256_to_rgb(15), (255, 255, 255));
    }

    #[test]
    fn ansi256_cube_colors() {
        // Color 16 = first cube color (0,0,0)
        assert_eq!(ansi256_to_rgb(16), (0, 0, 0));
        // Color 231 = last cube color (5,5,5) -> (255,255,255)
        assert_eq!(ansi256_to_rgb(231), (255, 255, 255));
    }

    #[test]
    fn ansi256_grayscale() {
        assert_eq!(ansi256_to_rgb(232), (8, 8, 8));
        assert_eq!(ansi256_to_rgb(255), (238, 238, 238));
    }
}
