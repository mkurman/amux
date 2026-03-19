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
    std::iter::repeat_n(c, n).collect()
}

/// Approximate visible length by stripping ftui markup tags.
///
/// Markup tags are `[tagname]`, `[tagname=value]`, `[/tagname]`.
/// Escaped brackets `\[` count as one visible character.
pub fn strip_markup_len(s: &str) -> usize {
    let mut len = 0;
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            // Escaped character: `\[` or `\\` — the next char is visible
            if let Some(&next) = chars.peek() {
                if next == '[' || next == '\\' {
                    chars.next();
                    len += 1;
                    continue;
                }
            }
            // Lone backslash — count it
            len += 1;
        } else if c == '[' {
            // Potential markup tag — consume until ']'
            let mut is_tag = false;
            let mut depth = 0;
            // Save position to restore if not a valid tag
            let mut tag_chars = Vec::new();
            tag_chars.push(c);
            for tc in chars.by_ref() {
                tag_chars.push(tc);
                if tc == ']' {
                    is_tag = true;
                    break;
                }
                if tc == '[' {
                    depth += 1;
                    if depth > 2 {
                        break; // not a tag
                    }
                }
                if tc == '\n' {
                    break; // tags don't span lines
                }
            }
            if !is_tag {
                // Not a tag — count all consumed chars as visible
                len += tag_chars.len();
            }
            // If is_tag, the entire [...] is markup and not visible
        } else {
            len += 1;
        }
    }
    len
}

/// Legacy alias — delegates to strip_markup_len
#[allow(dead_code)]
pub fn strip_ansi_len(s: &str) -> usize {
    strip_markup_len(s)
}

/// Pad a string (containing markup tags) to a visible width
pub fn pad_to_width(s: &str, width: usize) -> String {
    let visible = strip_markup_len(s);
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
    fn strip_markup_len_plain_string() {
        assert_eq!(strip_markup_len("hello"), 5);
    }

    #[test]
    fn strip_markup_len_with_fg_tags() {
        // "[fg=rgb(95,135,255)]" + "hello" + "[/fg]" — visible len = 5
        let s = "[fg=rgb(95,135,255)]hello[/fg]";
        assert_eq!(strip_markup_len(s), 5);
    }

    #[test]
    fn strip_markup_len_with_bg_tags() {
        let s = "[bg=rgb(255,0,0)]text[/bg]";
        assert_eq!(strip_markup_len(s), 4);
    }

    #[test]
    fn strip_markup_len_escaped_bracket() {
        // "\[x]" should be 3 visible chars: [, x, ]
        let s = "\\[x]";
        assert_eq!(strip_markup_len(s), 3);
    }

    #[test]
    fn strip_markup_len_nested_tags() {
        let s = "[fg=rgb(1,2,3)][bold]hi[/bold][/fg]";
        assert_eq!(strip_markup_len(s), 2);
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

    #[test]
    fn pad_to_width_with_markup() {
        let s = "[fg=rgb(1,2,3)]hi[/fg]";
        let padded = pad_to_width(s, 5);
        // visible "hi" = 2 chars, need 3 more spaces
        assert!(padded.ends_with("   "));
    }
}
