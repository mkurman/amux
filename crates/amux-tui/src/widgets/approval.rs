use ratatui::prelude::*;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, BorderType, Paragraph};

use crate::state::approval::{ApprovalState, RiskLevel};
use crate::theme::ThemeTokens;

pub fn render(
    frame: &mut Frame,
    area: Rect,
    approval: &ApprovalState,
    theme: &ThemeTokens,
) {
    let ap = approval.current_approval();

    // Determine border color from risk level
    let (border_style, risk_label, risk_style) = match ap.map(|a| a.risk_level) {
        Some(RiskLevel::Critical) => (
            theme.accent_danger,
            "\u{2588} CRITICAL RISK \u{2588}",
            theme.accent_danger,
        ),
        Some(RiskLevel::High) => (
            theme.accent_danger,
            "\u{2588} HIGH RISK \u{2588}",
            theme.accent_danger,
        ),
        Some(RiskLevel::Medium) => (
            theme.accent_secondary,
            "\u{25b2} MEDIUM RISK \u{25b2}",
            theme.accent_secondary,
        ),
        Some(RiskLevel::Low) | None => (
            theme.fg_dim,
            "\u{25c6} LOW RISK \u{25c6}",
            theme.fg_dim,
        ),
    };

    let block = Block::default()
        .title(" APPROVAL REQUIRED ")
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(border_style);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines: Vec<Line> = Vec::new();

    // Empty line
    lines.push(Line::raw(""));

    // Risk badge
    lines.push(Line::from(vec![
        Span::raw("  "),
        Span::styled(risk_label, risk_style),
    ]));

    // Empty line
    lines.push(Line::raw(""));

    if let Some(ap) = ap {
        // Command text
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled("Command: ", theme.fg_dim),
            Span::styled(&ap.command, theme.fg_active),
        ]));

        // Blast radius
        if !ap.blast_radius.is_empty() {
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled("Blast radius: ", theme.fg_dim),
                Span::styled(&ap.blast_radius, theme.fg_active),
            ]));
        }

        // Task title
        if let Some(task_title) = &ap.task_title {
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled("Task: ", theme.fg_dim),
                Span::styled(task_title.as_str(), theme.fg_dim),
            ]));
        }
    } else {
        lines.push(Line::from(Span::styled(
            "  No pending approvals.",
            theme.fg_dim,
        )));
    }

    // Pad to fill before action row
    let action_rows = 2; // separator + action line
    while lines.len() < inner.height.saturating_sub(action_rows as u16) as usize {
        lines.push(Line::raw(""));
    }

    // Separator
    lines.push(Line::from(Span::styled(
        "\u{2500}".repeat(inner.width as usize),
        theme.fg_dim,
    )));

    // Action row
    lines.push(Line::from(vec![
        Span::raw("  "),
        Span::styled("[Y]", theme.accent_success),
        Span::styled(" Allow once  ", theme.fg_active),
        Span::styled("[A]", theme.accent_secondary),
        Span::styled(" Allow for session  ", theme.fg_active),
        Span::styled("[N]", theme.accent_danger),
        Span::styled(" Reject", theme.fg_active),
    ]));

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner);
}

#[cfg(test)]
mod tests {
    use crate::state::approval::{ApprovalAction, ApprovalState, PendingApproval, RiskLevel};

    fn make_approval(risk: RiskLevel, command: &str) -> PendingApproval {
        PendingApproval {
            approval_id: "ap1".into(),
            task_id: "t1".into(),
            task_title: Some("Deploy to production".into()),
            command: command.into(),
            risk_level: risk,
            blast_radius: "production cluster".into(),
        }
    }

    #[test]
    fn approval_handles_empty_state() {
        let approval = ApprovalState::new();
        assert!(approval.current_approval().is_none());
    }

    #[test]
    fn approval_handles_pending_state() {
        let mut approval = ApprovalState::new();
        approval.reduce(ApprovalAction::ApprovalRequired(make_approval(
            RiskLevel::High,
            "kubectl delete pod",
        )));
        assert!(approval.current_approval().is_some());
    }
}
