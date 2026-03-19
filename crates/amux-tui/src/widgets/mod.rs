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

/// Escape `[` characters in user-provided text so they are not
/// misinterpreted as markup tag openers by the renderer.
pub fn escape_markup(s: &str) -> String {
    s.replace('[', "\\[")
}

/// Visible column width by stripping ftui markup tags and using Unicode width.
///
/// Markup tags are `[tagname]`, `[tagname=value]`, `[/tagname]`.
/// Escaped brackets `\[` count as one visible character.
/// Uses `unicode_width::UnicodeWidthChar` for proper CJK/emoji/box-drawing widths.
pub fn strip_markup_len(s: &str) -> usize {
    use unicode_width::UnicodeWidthChar;
    let mut len = 0;
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(&next) = chars.peek() {
                if next == '[' || next == '\\' {
                    let escaped = chars.next().unwrap();
                    len += UnicodeWidthChar::width(escaped).unwrap_or(1);
                    continue;
                }
            }
            len += UnicodeWidthChar::width(c).unwrap_or(1);
        } else if c == '[' {
            // Potential markup tag — consume until ']'
            let mut is_tag = false;
            let mut tag_chars = Vec::new();
            tag_chars.push(c);
            for tc in chars.by_ref() {
                tag_chars.push(tc);
                if tc == ']' {
                    is_tag = true;
                    break;
                }
                if tc == '\n' {
                    break;
                }
            }
            if !is_tag {
                for tc in &tag_chars {
                    len += UnicodeWidthChar::width(*tc).unwrap_or(1);
                }
            }
        } else {
            len += UnicodeWidthChar::width(c).unwrap_or(1);
        }
    }
    len
}

/// Legacy alias — delegates to strip_markup_len
#[allow(dead_code)]
pub fn strip_ansi_len(s: &str) -> usize {
    strip_markup_len(s)
}

/// Pad a string (containing markup tags) to a visible width.
/// If the string is shorter, pads with spaces. If longer, returns as-is
/// (use `fit_to_width` to both truncate and pad).
pub fn pad_to_width(s: &str, width: usize) -> String {
    let visible = strip_markup_len(s);
    if visible < width {
        format!("{}{}", s, " ".repeat(width - visible))
    } else {
        s.to_string()
    }
}

/// Truncate AND pad a string to exactly `width` visible characters.
/// This ensures the string takes exactly `width` columns — no more, no less.
pub fn fit_to_width(s: &str, width: usize) -> String {
    let visible = strip_markup_len(s);
    if visible <= width {
        // Pad short strings
        format!("{}{}", s, " ".repeat(width - visible))
    } else {
        // Truncate long strings
        truncate_to_width(s, width)
    }
}

/// Truncate a string (containing markup tags) to a maximum visible column width.
/// Preserves markup tags but removes visible characters beyond the limit.
/// Uses Unicode width for proper column counting.
pub fn truncate_to_width(s: &str, width: usize) -> String {
    use unicode_width::UnicodeWidthChar;
    let mut result = String::new();
    let mut visible = 0;
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if visible >= width {
            break;
        }

        if c == '\\' {
            if let Some(&next) = chars.peek() {
                if next == '[' || next == '\\' {
                    let escaped = chars.next().unwrap();
                    let w = UnicodeWidthChar::width(escaped).unwrap_or(1);
                    if visible + w <= width {
                        result.push(c);
                        result.push(escaped);
                        visible += w;
                    }
                    continue;
                }
            }
            let w = UnicodeWidthChar::width(c).unwrap_or(1);
            if visible + w <= width {
                result.push(c);
                visible += w;
            }
        } else if c == '[' {
            // Consume the entire tag (zero visible width)
            result.push(c);
            for tc in chars.by_ref() {
                result.push(tc);
                if tc == ']' {
                    break;
                }
            }
        } else {
            let w = UnicodeWidthChar::width(c).unwrap_or(1);
            if visible + w <= width {
                result.push(c);
                visible += w;
            } else {
                break; // char would exceed width
            }
        }
    }

    result
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
    fn escape_markup_escapes_brackets() {
        assert_eq!(escape_markup("hello [world]"), "hello \\[world]");
        assert_eq!(escape_markup("no brackets"), "no brackets");
        assert_eq!(escape_markup("[a][b]"), "\\[a]\\[b]");
    }

    #[test]
    fn pad_to_width_with_markup() {
        let s = "[fg=rgb(1,2,3)]hi[/fg]";
        let padded = pad_to_width(s, 5);
        // visible "hi" = 2 chars, need 3 more spaces
        assert!(padded.ends_with("   "));
    }
}
