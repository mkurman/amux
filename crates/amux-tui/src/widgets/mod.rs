// Widget modules — uncomment as implemented
pub mod header;
pub mod footer;
pub mod splash;
pub mod chat;
pub mod message;
pub mod reasoning;
pub mod sidebar;
pub mod task_tree;
pub mod subagents;
pub mod command_palette;
pub mod approval;
pub mod thread_picker;
pub mod settings;
pub mod provider_picker;
pub mod model_picker;

// Shared utilities

pub fn repeat_char(c: char, n: usize) -> String {
    std::iter::repeat(c).take(n).collect()
}

/// Approximate visible length by stripping ANSI escape sequences
pub fn strip_ansi_len(s: &str) -> usize {
    let mut len = 0;
    let mut in_escape = false;
    for c in s.chars() {
        if c == '\x1b' {
            in_escape = true;
        } else if in_escape {
            if c == 'm' {
                in_escape = false;
            }
        } else {
            len += 1;
        }
    }
    len
}

/// Pad a string (containing ANSI escapes) to a visible width
pub fn pad_to_width(s: &str, width: usize) -> String {
    let visible = strip_ansi_len(s);
    if visible < width {
        format!("{}{}", s, " ".repeat(width - visible))
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_ansi_len_plain_string() {
        assert_eq!(strip_ansi_len("hello"), 5);
    }

    #[test]
    fn strip_ansi_len_with_escapes() {
        // "\x1b[38;5;75m" + "hello" + "\x1b[0m" — visible len = 5
        let s = "\x1b[38;5;75mhello\x1b[0m";
        assert_eq!(strip_ansi_len(s), 5);
    }

    #[test]
    fn repeat_char_produces_correct_count() {
        assert_eq!(repeat_char('─', 5), "─────");
        assert_eq!(repeat_char('─', 0), "");
    }

    #[test]
    fn pad_to_width_pads_short_string() {
        let padded = pad_to_width("hi", 5);
        assert_eq!(padded, "hi   ");
    }

    #[test]
    fn pad_to_width_does_not_truncate_long_string() {
        let padded = pad_to_width("hello world", 5);
        assert_eq!(padded, "hello world");
    }
}
