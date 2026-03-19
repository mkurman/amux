use ratatui::prelude::*;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::state::sidebar::SidebarState;
use crate::state::task::{GoalRunStatus, TaskState, TaskStatus};
use crate::theme::ThemeTokens;

pub fn render(
    frame: &mut Frame,
    area: Rect,
    tasks: &TaskState,
    sidebar: &SidebarState,
    theme: &ThemeTokens,
) {
    let mut lines: Vec<Line> = Vec::new();

    let all_tasks = tasks.tasks();
    let goal_runs = tasks.goal_runs();

    // Goal-run groups
    for run in goal_runs {
        let expanded = sidebar.is_expanded(&run.id);
        let arrow = if expanded { "\u{25be}" } else { "\u{25b8}" };
        let dot = goal_run_dot(run.status, theme);
        let status_span = match run.status {
            Some(GoalRunStatus::Running) => Span::styled("running", theme.accent_secondary),
            Some(GoalRunStatus::Completed) => Span::styled("done", theme.accent_success),
            Some(GoalRunStatus::Failed) => Span::styled("failed", theme.accent_danger),
            _ => Span::styled("idle", theme.fg_dim),
        };

        lines.push(Line::from(vec![
            Span::styled(arrow, theme.fg_active),
            Span::raw(" "),
            Span::raw(&run.title),
            Span::raw(" "),
            dot,
            Span::raw(" "),
            status_span,
        ]));

        if expanded {
            let child_tasks: Vec<_> = all_tasks
                .iter()
                .filter(|t| t.goal_run_id.as_deref() == Some(&run.id))
                .collect();

            if child_tasks.is_empty() {
                lines.push(Line::from(Span::styled("  (no tasks)", theme.fg_dim)));
            } else {
                for task in child_tasks {
                    let dot = status_dot_for_task(task.status, theme);
                    let status_lbl = status_label_for_task(task.status, theme);
                    lines.push(Line::from(vec![
                        Span::raw("  "),
                        dot,
                        Span::raw(" "),
                        Span::raw(&task.title),
                        Span::raw(" "),
                        status_lbl,
                    ]));

                    if let Some(ref session_id) = task.session_id {
                        lines.push(Line::from(vec![
                            Span::raw("    "),
                            Span::styled(
                                format!("\u{2514} thread: {}", session_id),
                                theme.fg_dim,
                            ),
                        ]));
                    }
                }
            }
        }
    }

    // Daemon group (tasks without goal_run_id)
    let daemon_tasks: Vec<_> = all_tasks
        .iter()
        .filter(|t| t.goal_run_id.is_none())
        .collect();

    if !daemon_tasks.is_empty() {
        let expanded = sidebar.is_expanded("__daemon__");
        let arrow = if expanded { "\u{25be}" } else { "\u{25b8}" };
        lines.push(Line::from(vec![
            Span::styled(arrow, theme.fg_dim),
            Span::raw(" "),
            Span::styled("daemon", theme.fg_active),
        ]));

        if expanded {
            for task in daemon_tasks {
                let dot = status_dot_for_task(task.status, theme);
                let status_lbl = status_label_for_task(task.status, theme);
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    dot,
                    Span::raw(" "),
                    Span::raw(&task.title),
                    Span::raw(" "),
                    status_lbl,
                ]));
            }
        }
    }

    // Empty state
    if lines.is_empty() {
        lines.push(Line::from(Span::styled(" No agents", theme.fg_dim)));
    }

    // Footer aggregate counts
    if !goal_runs.is_empty() || !all_tasks.is_empty() {
        let running_count = all_tasks
            .iter()
            .filter(|t| t.status == Some(TaskStatus::InProgress))
            .count();
        let total_count = all_tasks.len();
        let group_count = goal_runs.len();

        lines.push(Line::from(Span::styled(
            format!(
                " {} groups \u{00b7} {}/{} running",
                group_count, running_count, total_count
            ),
            theme.fg_dim,
        )));
    }

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, area);
}

fn goal_run_dot<'a>(status: Option<GoalRunStatus>, theme: &ThemeTokens) -> Span<'a> {
    match status {
        Some(GoalRunStatus::Running) => Span::styled("\u{25cf}", theme.accent_secondary),
        Some(GoalRunStatus::Completed) => Span::styled("\u{25cf}", theme.accent_success),
        Some(GoalRunStatus::Failed) => Span::styled("\u{25cf}", theme.accent_danger),
        _ => Span::styled("\u{25cf}", theme.fg_dim),
    }
}

fn status_dot_for_task<'a>(status: Option<TaskStatus>, theme: &ThemeTokens) -> Span<'a> {
    match status {
        Some(TaskStatus::InProgress) => Span::styled("\u{25cf}", theme.accent_secondary),
        Some(TaskStatus::Completed) => Span::styled("\u{25cf}", theme.accent_success),
        Some(TaskStatus::Failed) | Some(TaskStatus::FailedAnalyzing) => {
            Span::styled("\u{25cf}", theme.accent_danger)
        }
        Some(TaskStatus::Blocked) | Some(TaskStatus::AwaitingApproval) => {
            Span::styled("\u{25cf}", theme.accent_primary)
        }
        _ => Span::styled("\u{25cf}", theme.fg_dim),
    }
}

fn status_label_for_task<'a>(status: Option<TaskStatus>, theme: &ThemeTokens) -> Span<'a> {
    match status {
        Some(TaskStatus::InProgress) => Span::styled("running", theme.accent_secondary),
        Some(TaskStatus::Completed) => Span::styled("done", theme.accent_success),
        Some(TaskStatus::Failed) | Some(TaskStatus::FailedAnalyzing) => {
            Span::styled("failed", theme.accent_danger)
        }
        Some(TaskStatus::Blocked) => Span::styled("blocked", theme.fg_dim),
        Some(TaskStatus::AwaitingApproval) => Span::styled("awaiting", theme.accent_primary),
        Some(TaskStatus::Queued) | None => Span::styled("idle", theme.fg_dim),
        Some(TaskStatus::Cancelled) => Span::styled("cancelled", theme.fg_dim),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::sidebar::SidebarState;
    use crate::state::task::TaskState;

    #[test]
    fn subagents_handles_empty_state() {
        let tasks = TaskState::new();
        let _sidebar = SidebarState::new();
        let _theme = ThemeTokens::default();
        assert!(tasks.goal_runs().is_empty());
        assert!(tasks.tasks().is_empty());
    }
}
