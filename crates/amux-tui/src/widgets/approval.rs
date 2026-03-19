use crate::theme::{ThemeTokens, SHARP_BORDER, FG_CLOSE};
use crate::state::approval::{ApprovalState, RiskLevel};

/// Render the approval modal as an overlay.
/// Border color depends on risk level: red for HIGH/CRITICAL, amber for MEDIUM, dim for LOW.
/// Returns a full-screen Vec<String> (one entry per row) centered over the terminal.
pub fn approval_widget(
    approval: &ApprovalState,
    theme: &ThemeTokens,
    screen_width: usize,
    screen_height: usize,
) -> Vec<String> {
    let b = &SHARP_BORDER;

    // Size: ~60% width, ~40% height, centered
    let modal_w = (screen_width * 60 / 100).max(50).min(screen_width);
    let modal_h = (screen_height * 40 / 100).max(10).min(screen_height);
    let inner_w = modal_w.saturating_sub(2);
    let inner_h = modal_h.saturating_sub(2);

    let x_pad = (screen_width.saturating_sub(modal_w)) / 2;
    let y_pad = (screen_height.saturating_sub(modal_h)) / 2;

    let mut result = Vec::new();

    // Top padding
    for _ in 0..y_pad {
        result.push(" ".repeat(screen_width));
    }

    // Get the current pending approval (if any)
    let ap = approval.current_approval();

    // Determine border color from risk level
    let (bc, risk_label, risk_color) = match ap.map(|a| a.risk_level) {
        Some(RiskLevel::Critical) => (
            theme.accent_danger.fg(),
            "█ CRITICAL RISK █",
            theme.accent_danger.fg(),
        ),
        Some(RiskLevel::High) => (
            theme.accent_danger.fg(),
            "█ HIGH RISK █",
            theme.accent_danger.fg(),
        ),
        Some(RiskLevel::Medium) => (
            theme.accent_secondary.fg(),
            "▲ MEDIUM RISK ▲",
            theme.accent_secondary.fg(),
        ),
        Some(RiskLevel::Low) | None => (
            theme.fg_dim.fg(),
            "◆ LOW RISK ◆",
            theme.fg_dim.fg(),
        ),
    };

    // Top border with APPROVAL title
    let title = " APPROVAL REQUIRED ";
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
        FG_CLOSE,
        " ".repeat(screen_width.saturating_sub(x_pad + modal_w)),
    ));

    // Helper: push a bordered content line
    let push_line = |result: &mut Vec<String>, content: &str| {
        let padded = super::pad_to_width(content, inner_w);
        result.push(format!(
            "{}{}{}{}{}{}{}",
            " ".repeat(x_pad),
            bc, b.vertical,
            padded,
            b.vertical,
            FG_CLOSE,
            " ".repeat(screen_width.saturating_sub(x_pad + modal_w)),
        ));
    };

    // Empty line
    push_line(&mut result, "");

    // Risk badge
    let badge_line = format!(
        "  {}{}{}",
        risk_color,
        risk_label,
        FG_CLOSE,
    );
    push_line(&mut result, &badge_line);

    // Empty line
    push_line(&mut result, "");

    if let Some(ap) = ap {
        // Command text
        let cmd_label = format!(
            "  {}Command:{} {}{}{}",
            theme.fg_dim.fg(), FG_CLOSE,
            theme.fg_active.fg(), ap.command, FG_CLOSE,
        );
        push_line(&mut result, &cmd_label);

        // Blast radius
        if !ap.blast_radius.is_empty() {
            let br_line = format!(
                "  {}Blast radius:{} {}{}{}",
                theme.fg_dim.fg(), FG_CLOSE,
                theme.fg_active.fg(), ap.blast_radius, FG_CLOSE,
            );
            push_line(&mut result, &br_line);
        }

        // Task title
        if let Some(task_title) = &ap.task_title {
            let task_line = format!(
                "  {}Task:{} {}{}{}",
                theme.fg_dim.fg(), FG_CLOSE,
                theme.fg_dim.fg(), task_title, FG_CLOSE,
            );
            push_line(&mut result, &task_line);
        }
    } else {
        push_line(&mut result, &format!("  {}No pending approvals.{}", theme.fg_dim.fg(), FG_CLOSE));
    }

    // Fill remaining inner lines up to the action row
    let _content_rows_used = result.len() - y_pad - 1;
    let rows_before_action = inner_h.saturating_sub(2);
    while result.len() - y_pad - 1 < rows_before_action {
        push_line(&mut result, "");
    }

    // Separator
    let sep_line = format!(
        "{}{}{}{}{}{}{}",
        " ".repeat(x_pad),
        bc, b.vertical,
        super::repeat_char('─', inner_w),
        b.vertical,
        FG_CLOSE,
        " ".repeat(screen_width.saturating_sub(x_pad + modal_w)),
    );
    if result.len() - y_pad - 1 < inner_h {
        result.push(sep_line);
    }

    // Action row — escape literal brackets around key labels
    let actions = format!(
        "  {}\\[Y]{} Allow once  {}\\[A]{} Allow for session  {}\\[N]{} Reject",
        theme.accent_success.fg(), theme.fg_active.fg(),
        theme.accent_secondary.fg(), theme.fg_active.fg(),
        theme.accent_danger.fg(), theme.fg_active.fg(),
    );
    let action_line = format!("{}{}", actions, FG_CLOSE);
    if result.len() - y_pad - 1 < inner_h {
        push_line(&mut result, &action_line);
    }

    // Fill remaining rows up to inner_h
    while result.len() - y_pad - 1 < inner_h {
        push_line(&mut result, "");
    }

    // Bottom border
    result.push(format!(
        "{}{}{}{}{}{}{}",
        " ".repeat(x_pad),
        bc, b.bottom_left,
        super::repeat_char(b.horizontal, inner_w),
        b.bottom_right,
        FG_CLOSE,
        " ".repeat(screen_width.saturating_sub(x_pad + modal_w)),
    ));

    // Bottom padding
    while result.len() < screen_height {
        result.push(" ".repeat(screen_width));
    }
    result.truncate(screen_height);

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::approval::{ApprovalState, ApprovalAction, PendingApproval, RiskLevel};
    use crate::theme::ThemeTokens;

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
    fn approval_widget_returns_correct_dimensions() {
        let approval = ApprovalState::new();
        let theme = ThemeTokens::default();
        let lines = approval_widget(&approval, &theme, 120, 40);
        assert_eq!(lines.len(), 40);
    }

    #[test]
    fn approval_widget_shows_title() {
        let approval = ApprovalState::new();
        let theme = ThemeTokens::default();
        let lines = approval_widget(&approval, &theme, 120, 40);
        let joined = lines.join("");
        assert!(joined.contains("APPROVAL REQUIRED"));
    }

    #[test]
    fn approval_widget_shows_command() {
        let mut approval = ApprovalState::new();
        approval.reduce(ApprovalAction::ApprovalRequired(make_approval(
            RiskLevel::High,
            "kubectl delete pod mypod",
        )));
        let theme = ThemeTokens::default();
        let lines = approval_widget(&approval, &theme, 120, 40);
        let joined = lines.join("");
        assert!(joined.contains("kubectl delete pod mypod"));
    }

    #[test]
    fn approval_widget_shows_blast_radius() {
        let mut approval = ApprovalState::new();
        approval.reduce(ApprovalAction::ApprovalRequired(make_approval(
            RiskLevel::Medium,
            "rm -rf ./build",
        )));
        let theme = ThemeTokens::default();
        let lines = approval_widget(&approval, &theme, 120, 40);
        let joined = lines.join("");
        assert!(joined.contains("production cluster"));
    }

    #[test]
    fn approval_widget_shows_task_title() {
        let mut approval = ApprovalState::new();
        approval.reduce(ApprovalAction::ApprovalRequired(make_approval(
            RiskLevel::Low,
            "ls -la",
        )));
        let theme = ThemeTokens::default();
        let lines = approval_widget(&approval, &theme, 120, 40);
        let joined = lines.join("");
        assert!(joined.contains("Deploy to production"));
    }

    #[test]
    fn approval_widget_shows_action_row() {
        let mut approval = ApprovalState::new();
        approval.reduce(ApprovalAction::ApprovalRequired(make_approval(
            RiskLevel::High,
            "docker system prune",
        )));
        let theme = ThemeTokens::default();
        let lines = approval_widget(&approval, &theme, 120, 40);
        let joined = lines.join("");
        // Escaped brackets
        assert!(joined.contains("\\[Y]"));
        assert!(joined.contains("\\[A]"));
        assert!(joined.contains("\\[N]"));
    }

    #[test]
    fn approval_widget_critical_shows_danger_risk_badge() {
        let mut approval = ApprovalState::new();
        approval.reduce(ApprovalAction::ApprovalRequired(make_approval(
            RiskLevel::Critical,
            "rm -rf /",
        )));
        let theme = ThemeTokens::default();
        let lines = approval_widget(&approval, &theme, 120, 40);
        let joined = lines.join("");
        assert!(joined.contains("CRITICAL RISK"));
        assert!(joined.contains(&theme.accent_danger.fg()));
    }

    #[test]
    fn approval_widget_medium_shows_amber_risk_badge() {
        let mut approval = ApprovalState::new();
        approval.reduce(ApprovalAction::ApprovalRequired(make_approval(
            RiskLevel::Medium,
            "rm -rf ./temp",
        )));
        let theme = ThemeTokens::default();
        let lines = approval_widget(&approval, &theme, 120, 40);
        let joined = lines.join("");
        assert!(joined.contains("MEDIUM RISK"));
        assert!(joined.contains(&theme.accent_secondary.fg()));
    }

    #[test]
    fn approval_widget_no_pending_shows_empty_message() {
        let approval = ApprovalState::new();
        let theme = ThemeTokens::default();
        let lines = approval_widget(&approval, &theme, 120, 40);
        let joined = lines.join("");
        assert!(joined.contains("No pending approvals"));
    }
}
