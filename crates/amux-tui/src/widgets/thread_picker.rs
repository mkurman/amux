use ratatui::prelude::*;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, BorderType, List, ListItem, Paragraph};

use crate::state::chat::ChatState;
use crate::state::modal::ModalState;
use crate::theme::ThemeTokens;

pub fn render(
    frame: &mut Frame,
    area: Rect,
    chat: &ChatState,
    modal: &ModalState,
    theme: &ThemeTokens,
) {
    let block = Block::default()
        .title(" THREADS ")
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(theme.accent_secondary);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height < 4 {
        return;
    }

    // Split: search input (1) + separator (1) + list (flex) + hints (1)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // search
            Constraint::Length(1), // separator
            Constraint::Min(1),   // list
            Constraint::Length(1), // hints
        ])
        .split(inner);

    // Search input
    let query = modal.command_query();
    let input_line = Line::from(vec![
        Span::raw(" "),
        Span::styled(
            if query.is_empty() {
                "Search threads..."
            } else {
                query
            },
            theme.fg_active,
        ),
        if query.is_empty() {
            Span::raw("")
        } else {
            Span::raw("\u{2588}")
        },
    ]);
    frame.render_widget(Paragraph::new(input_line), chunks[0]);

    // Separator
    let sep = Line::from(Span::styled(
        "\u{2500}".repeat(chunks[1].width as usize),
        theme.fg_dim,
    ));
    frame.render_widget(Paragraph::new(sep), chunks[1]);

    // Build thread list
    let threads = chat.threads();
    let active_id = chat.active_thread_id();
    let search = query.to_lowercase();

    let filtered_threads: Vec<_> = threads
        .iter()
        .filter(|t| search.is_empty() || t.title.to_lowercase().contains(&search))
        .collect();

    let cursor = modal.picker_cursor();
    let list_h = chunks[2].height as usize;
    let inner_w = inner.width as usize;

    let list_items: Vec<ListItem> = (0..list_h)
        .map(|i| {
            if i == 0 {
                // "New conversation" item
                let is_selected = cursor == 0;
                if is_selected {
                    ListItem::new(Line::from(vec![
                        Span::raw("  + New conversation"),
                    ]))
                    .style(
                        Style::default()
                            .bg(Color::Indexed(178))
                            .fg(Color::Black),
                    )
                } else {
                    ListItem::new(Line::from(vec![
                        Span::raw("  "),
                        Span::styled("+ New conversation", theme.fg_dim),
                    ]))
                }
            } else {
                let thread_idx = i - 1;
                if thread_idx < filtered_threads.len() {
                    let thread = filtered_threads[thread_idx];
                    let is_selected = cursor == i;
                    let is_active = active_id == Some(thread.id.as_str());

                    let dot_style = if is_active {
                        theme.accent_success
                    } else {
                        theme.fg_dim
                    };

                    let time_str = format_time_ago(thread.updated_at);
                    let tokens = thread.total_input_tokens + thread.total_output_tokens;
                    let token_str = format_tokens(tokens);

                    let max_title = inner_w.saturating_sub(25);
                    let title = if thread.title.len() > max_title && max_title > 3 {
                        format!("{}...", &thread.title[..max_title - 3])
                    } else {
                        thread.title.clone()
                    };

                    if is_selected {
                        ListItem::new(Line::from(vec![
                            Span::styled("\u{25cf}", dot_style),
                            Span::raw(" "),
                            Span::raw(title),
                            Span::raw("  "),
                            Span::raw(time_str),
                            Span::raw("  "),
                            Span::raw(token_str),
                        ]))
                        .style(
                            Style::default()
                                .bg(Color::Indexed(178))
                                .fg(Color::Black),
                        )
                    } else {
                        ListItem::new(Line::from(vec![
                            Span::raw("  "),
                            Span::styled("\u{25cf}", dot_style),
                            Span::raw(" "),
                            Span::styled(title, theme.fg_active),
                            Span::raw("  "),
                            Span::styled(time_str, theme.fg_dim),
                            Span::raw("  "),
                            Span::styled(token_str, theme.fg_dim),
                        ]))
                    }
                } else {
                    ListItem::new(Line::raw(""))
                }
            }
        })
        .collect();

    let list = List::new(list_items);
    frame.render_widget(list, chunks[2]);

    // Hints
    let hints = Line::from(vec![
        Span::raw(" "),
        Span::styled("j/k", theme.fg_active),
        Span::styled(" navigate  ", theme.fg_dim),
        Span::styled("Enter", theme.fg_active),
        Span::styled(" select  ", theme.fg_dim),
        Span::styled("Esc", theme.fg_active),
        Span::styled(" close", theme.fg_dim),
    ]);
    frame.render_widget(Paragraph::new(hints), chunks[3]);
}

/// Format millisecond timestamp as "Xm ago" or "Xh ago" etc.
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

/// Format token count compactly
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

    #[test]
    fn format_time_ago_zero_returns_empty() {
        assert_eq!(format_time_ago(0), "");
    }

    #[test]
    fn format_tokens_zero_returns_empty() {
        assert_eq!(format_tokens(0), "");
    }

    #[test]
    fn format_tokens_thousands() {
        let s = format_tokens(1500);
        assert!(s.contains("k tok"));
    }

    #[test]
    fn format_tokens_small() {
        let s = format_tokens(500);
        assert_eq!(s, "500 tok");
    }
}
