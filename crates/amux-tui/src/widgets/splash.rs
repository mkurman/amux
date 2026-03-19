use ratatui::prelude::*;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::theme::ThemeTokens;

pub fn render(frame: &mut Frame, area: Rect, theme: &ThemeTokens) {
    let height = area.height as usize;
    let mut lines: Vec<Line> = Vec::new();

    // Vertical centering: put logo ~1/3 from top
    let pad_top = height / 3;
    for _ in 0..pad_top {
        lines.push(Line::raw(""));
    }

    // Logo with gradient
    let logo = Line::from(vec![
        Span::styled("\u{2591}", Style::default().fg(Color::Indexed(24))),
        Span::styled("\u{2592}", Style::default().fg(Color::Indexed(31))),
        Span::styled("\u{2593}", Style::default().fg(Color::Indexed(38))),
        Span::styled("\u{2588}", Style::default().fg(Color::Indexed(75))),
        Span::styled(" T A M U X ", theme.accent_primary),
        Span::styled("\u{2588}", Style::default().fg(Color::Indexed(75))),
        Span::styled("\u{2593}", Style::default().fg(Color::Indexed(38))),
        Span::styled("\u{2592}", Style::default().fg(Color::Indexed(31))),
        Span::styled("\u{2591}", Style::default().fg(Color::Indexed(24))),
    ]);
    lines.push(logo);

    // Motto
    lines.push(Line::from(Span::styled(
        "plan \u{00b7} solve \u{00b7} ship",
        theme.fg_dim,
    )));

    // Empty line
    lines.push(Line::raw(""));

    // Hints
    lines.push(Line::from(Span::styled(
        "Type a prompt to begin, or",
        theme.fg_dim,
    )));
    lines.push(Line::from(vec![
        Span::styled("Ctrl+P", theme.accent_primary),
        Span::styled(" to open command palette", theme.fg_dim),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Ctrl+T", theme.accent_primary),
        Span::styled(" to pick a thread", theme.fg_dim),
    ]));

    // Pad remaining height
    while lines.len() < height {
        lines.push(Line::raw(""));
    }
    lines.truncate(height);

    let paragraph = Paragraph::new(lines).alignment(Alignment::Center);
    frame.render_widget(paragraph, area);
}

#[cfg(test)]
mod tests {
    #[test]
    fn splash_module_exists() {
        // Just ensure the module compiles
        assert!(true);
    }
}
