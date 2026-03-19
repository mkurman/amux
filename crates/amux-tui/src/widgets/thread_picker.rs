use crate::theme::{ThemeTokens, SHARP_BORDER, RESET};
use crate::state::chat::ChatState;
use crate::state::modal::ModalState;

/// Render the thread picker as an overlay.
/// Returns a full-screen Vec<String> (one entry per row) centered over the terminal.
pub fn thread_picker_widget(
    chat: &ChatState,
    modal: &ModalState,
    theme: &ThemeTokens,
    screen_width: usize,
    screen_height: usize,
) -> Vec<String> {
    let bc = theme.accent_secondary.fg(); // amber border
    let b = &SHARP_BORDER;

    // Size: ~60% width, ~50% height, centered
    let picker_w = (screen_width * 60 / 100).max(50).min(screen_width);
    let picker_h = (screen_height * 50 / 100).max(10).min(screen_height);
    let inner_w = picker_w.saturating_sub(2);
    let inner_h = picker_h.saturating_sub(2);

    let mut result = Vec::new();

    // Calculate centering offsets
    let x_pad = (screen_width.saturating_sub(picker_w)) / 2;
    let y_pad = (screen_height.saturating_sub(picker_h)) / 2;

    // Top padding
    for _ in 0..y_pad {
        result.push(" ".repeat(screen_width));
    }

    // Top border with title
    let title = " THREADS ";
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
        " ".repeat(screen_width.saturating_sub(x_pad + picker_w)),
    ));

    // Search input line
    let query = modal.command_query();
    let input_line = format!(
        " {}{}{}{}",
        theme.fg_active.fg(),
        if query.is_empty() { "Search threads..." } else { query },
        if query.is_empty() { "" } else { "█" },
        RESET,
    );
    let padded_input = super::pad_to_width(&input_line, inner_w);
    result.push(format!(
        "{}{}{}{}{}{}{}",
        " ".repeat(x_pad),
        bc, b.vertical,
        padded_input,
        b.vertical,
        RESET,
        " ".repeat(screen_width.saturating_sub(x_pad + picker_w)),
    ));

    // Separator
    result.push(format!(
        "{}{}{}{}{}{}{}",
        " ".repeat(x_pad),
        bc, b.vertical,
        super::repeat_char('─', inner_w),
        b.vertical,
        RESET,
        " ".repeat(screen_width.saturating_sub(x_pad + picker_w)),
    ));

    // Build thread list: "New conversation" first, then matching threads
    let threads = chat.threads();
    let active_id = chat.active_thread_id();
    let search = query.to_lowercase();

    // Filter threads by search query
    let filtered_threads: Vec<_> = threads
        .iter()
        .filter(|t| {
            search.is_empty() || t.title.to_lowercase().contains(&search)
        })
        .collect();

    let cursor = modal.picker_cursor();

    // list_h = inner_h minus: input row, separator, hints row
    let list_h = inner_h.saturating_sub(3);

    for i in 0..list_h {
        // Item 0 = "New conversation", items 1..N = threads
        let line = if i == 0 {
            // "New conversation" item
            let is_selected = cursor == 0;
            if is_selected {
                format!(
                    " {}{}  + New conversation{}{}",
                    theme.accent_secondary.bg(),
                    "\x1b[38;5;0m",
                    RESET,
                    " ".repeat(inner_w.saturating_sub(20)),
                )
            } else {
                format!(
                    "  {}{} + New conversation{}",
                    theme.fg_dim.fg(),
                    RESET,
                    String::new(),
                )
            }
        } else {
            let thread_idx = i - 1;
            if thread_idx < filtered_threads.len() {
                let thread = filtered_threads[thread_idx];
                let is_selected = cursor == i;
                let is_active = active_id == Some(thread.id.as_str());

                // Status dot: green for active/streaming, grey otherwise
                let dot = if is_active {
                    format!("{}●{}", theme.accent_success.fg(), RESET)
                } else {
                    format!("{}●{}", theme.fg_dim.fg(), RESET)
                };

                // Time ago
                let time_str = format_time_ago(thread.updated_at);

                // Token count
                let tokens = thread.total_input_tokens + thread.total_output_tokens;
                let token_str = format_tokens(tokens);

                // Title truncated
                let max_title = inner_w.saturating_sub(25);
                let title = if thread.title.len() > max_title && max_title > 3 {
                    format!("{}...", &thread.title[..max_title - 3])
                } else {
                    thread.title.clone()
                };

                if is_selected {
                    format!(
                        " {}{}{} {}{}  {}  {}  {}{}",
                        theme.accent_secondary.bg(),
                        "\x1b[38;5;0m",
                        dot,
                        title,
                        RESET,
                        theme.accent_secondary.bg(),
                        time_str,
                        token_str,
                        RESET,
                    )
                } else {
                    format!(
                        "  {} {}{}{}  {}{}  {}{}",
                        dot,
                        theme.fg_active.fg(),
                        title,
                        RESET,
                        theme.fg_dim.fg(),
                        time_str,
                        token_str,
                        RESET,
                    )
                }
            } else {
                String::new()
            }
        };

        let padded = super::pad_to_width(&line, inner_w);
        result.push(format!(
            "{}{}{}{}{}{}{}",
            " ".repeat(x_pad),
            bc, b.vertical,
            padded,
            b.vertical,
            RESET,
            " ".repeat(screen_width.saturating_sub(x_pad + picker_w)),
        ));
    }

    // Hints line
    let hints = format!(
        " {}j/k{} navigate  {}Enter{} select  {}Esc{} close",
        theme.fg_active.fg(), theme.fg_dim.fg(),
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
        " ".repeat(screen_width.saturating_sub(x_pad + picker_w)),
    ));

    // Bottom border
    result.push(format!(
        "{}{}{}{}{}{}{}",
        " ".repeat(x_pad),
        bc, b.bottom_left,
        super::repeat_char(b.horizontal, inner_w),
        b.bottom_right,
        RESET,
        " ".repeat(screen_width.saturating_sub(x_pad + picker_w)),
    ));

    // Bottom padding
    while result.len() < screen_height {
        result.push(" ".repeat(screen_width));
    }
    result.truncate(screen_height);

    result
}

/// Format millisecond timestamp as "Xm ago" or "Xh ago" etc.
/// Returns empty string if timestamp is 0 (unknown).
fn format_time_ago(updated_at: u64) -> String {
    if updated_at == 0 {
        return String::new();
    }
    let now = now_millis();
    if now < updated_at {
        return "just now".to_string();
    }
    let diff_secs = (now - updated_at) / 1000;
    if diff_secs < 60 {
        format!("{}s ago", diff_secs)
    } else if diff_secs < 3600 {
        format!("{}m ago", diff_secs / 60)
    } else if diff_secs < 86400 {
        format!("{}h ago", diff_secs / 3600)
    } else {
        format!("{}d ago", diff_secs / 86400)
    }
}

/// Format token count compactly: "1.2k" for >= 1000, else plain number.
fn format_tokens(tokens: u64) -> String {
    if tokens == 0 {
        return String::new();
    }
    if tokens >= 1_000_000 {
        format!("{:.1}M tok", tokens as f64 / 1_000_000.0)
    } else if tokens >= 1000 {
        format!("{:.1}k tok", tokens as f64 / 1000.0)
    } else {
        format!("{} tok", tokens)
    }
}

/// Get current time in milliseconds (wall clock).
fn now_millis() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::chat::{ChatState, ChatAction, AgentThread};
    use crate::state::modal::{ModalState, ModalAction, ModalKind};
    use crate::theme::ThemeTokens;

    #[test]
    fn thread_picker_returns_correct_dimensions() {
        let chat = ChatState::new();
        let modal = ModalState::new();
        let theme = ThemeTokens::default();
        let lines = thread_picker_widget(&chat, &modal, &theme, 120, 40);
        assert_eq!(lines.len(), 40);
    }

    #[test]
    fn thread_picker_contains_threads_title() {
        let chat = ChatState::new();
        let modal = ModalState::new();
        let theme = ThemeTokens::default();
        let lines = thread_picker_widget(&chat, &modal, &theme, 120, 40);
        let joined = lines.join("");
        assert!(joined.contains("THREADS"));
    }

    #[test]
    fn thread_picker_shows_new_conversation_first() {
        let chat = ChatState::new();
        let modal = ModalState::new();
        let theme = ThemeTokens::default();
        let lines = thread_picker_widget(&chat, &modal, &theme, 120, 40);
        let joined = lines.join("");
        assert!(joined.contains("New conversation"));
    }

    #[test]
    fn thread_picker_shows_thread_titles() {
        let mut chat = ChatState::new();
        chat.reduce(ChatAction::ThreadListReceived(vec![
            AgentThread {
                id: "t1".into(),
                title: "My First Thread".into(),
                updated_at: 0,
                ..Default::default()
            },
        ]));
        let modal = ModalState::new();
        let theme = ThemeTokens::default();
        let lines = thread_picker_widget(&chat, &modal, &theme, 120, 40);
        let joined = lines.join("");
        assert!(joined.contains("My First Thread"));
    }

    #[test]
    fn thread_picker_filters_by_query() {
        let mut chat = ChatState::new();
        chat.reduce(ChatAction::ThreadListReceived(vec![
            AgentThread { id: "t1".into(), title: "Rust project".into(), ..Default::default() },
            AgentThread { id: "t2".into(), title: "Python scripts".into(), ..Default::default() },
        ]));
        let mut modal = ModalState::new();
        modal.reduce(ModalAction::Push(ModalKind::ThreadPicker));
        modal.reduce(ModalAction::SetQuery("rust".into()));
        let theme = ThemeTokens::default();
        let lines = thread_picker_widget(&chat, &modal, &theme, 120, 40);
        let joined = lines.join("");
        assert!(joined.contains("Rust project"));
        assert!(!joined.contains("Python scripts"));
    }

    #[test]
    fn thread_picker_highlights_active_thread_with_green_dot() {
        let mut chat = ChatState::new();
        chat.reduce(ChatAction::ThreadListReceived(vec![
            AgentThread { id: "t1".into(), title: "Active Thread".into(), ..Default::default() },
        ]));
        chat.reduce(ChatAction::SelectThread("t1".into()));
        let modal = ModalState::new();
        let theme = ThemeTokens::default();
        let lines = thread_picker_widget(&chat, &modal, &theme, 120, 40);
        let joined = lines.join("");
        // Active thread has green dot (accent_success color)
        assert!(joined.contains(&theme.accent_success.fg()));
        assert!(joined.contains("Active Thread"));
    }

    #[test]
    fn format_time_ago_zero_returns_empty() {
        assert_eq!(format_time_ago(0), "");
    }

    #[test]
    fn format_time_ago_seconds() {
        let now = now_millis();
        let ts = now - 30_000; // 30 seconds ago
        let s = format_time_ago(ts);
        assert!(s.contains("s ago") || s.contains("m ago"), "got: {}", s);
    }

    #[test]
    fn format_tokens_zero_returns_empty() {
        assert_eq!(format_tokens(0), "");
    }

    #[test]
    fn format_tokens_thousands() {
        let s = format_tokens(1500);
        assert!(s.contains("k tok"), "got: {}", s);
    }

    #[test]
    fn format_tokens_small() {
        let s = format_tokens(500);
        assert_eq!(s, "500 tok");
    }
}
