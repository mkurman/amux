use ratatui::prelude::*;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::state::sidebar::SidebarState;
use crate::state::task::{GoalRunStatus, HeartbeatOutcome, TaskState, TaskStatus};
use crate::theme::ThemeTokens;

pub fn render(
    frame: &mut Frame,
    area: Rect,
    tasks: &TaskState,
    sidebar: &SidebarState,
    theme: &ThemeTokens,
) {
    let lines = build_lines(tasks, sidebar, theme, area.width as usize);
    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, area);
}

fn build_lines(
    tasks: &TaskState,
    sidebar: &SidebarState,
    theme: &ThemeTokens,
    width: usize,
) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();
    let selected = sidebar.selected_item();
    // item_index tracks selectable items (goal runs, steps, standalone tasks)
    let mut item_index: usize = 0;
    let sel_style = Style::default().bg(Color::Indexed(236));

    // Zone 1: Goal Runs
    let goal_runs = tasks.goal_runs();

    for run in goal_runs {
        let expanded = sidebar.is_expanded(&run.id);
        let arrow = if expanded { "\u{25be}" } else { "\u{25b8}" };
        let dot = goal_run_status_dot(run.status, theme);
        let status_span = goal_run_status_label(run.status, theme);

        let is_selected = item_index == selected;
        let spans = vec![
            if is_selected {
                Span::styled("> ", Style::default().fg(Color::Indexed(178)))
            } else {
                Span::raw("  ")
            },
            Span::styled(arrow.to_string(), theme.fg_active),
            Span::raw(" "),
            Span::raw(run.title.clone()),
            Span::raw(" "),
            dot,
            status_span,
        ];
        let line = if is_selected {
            Line::from(spans).style(sel_style)
        } else {
            Line::from(spans)
        };
        lines.push(line);
        item_index += 1;

        if expanded {
            let mut steps = run.steps.clone();
            steps.sort_by_key(|s| s.order);

            for step in &steps {
                let chip = step_status_chip(step.status, theme);
                let is_sel = item_index == selected;
                let line = if is_sel {
                    Line::from(vec![
                        Span::styled("> ", Style::default().fg(Color::Indexed(178))),
                        Span::raw("  "),
                        chip,
                        Span::raw(" "),
                        Span::raw(step.title.clone()),
                    ]).style(sel_style)
                } else {
                    Line::from(vec![
                        Span::raw("    "),
                        chip,
                        Span::raw(" "),
                        Span::raw(step.title.clone()),
                    ])
                };
                lines.push(line);
                item_index += 1;
            }
        }
    }

    // Zone 2: Standalone Tasks
    let standalone: Vec<_> = tasks
        .tasks()
        .iter()
        .filter(|t| t.goal_run_id.is_none())
        .collect();

    if !standalone.is_empty() {
        if !goal_runs.is_empty() {
            lines.push(Line::from(Span::styled(
                "\u{2500}".repeat(width.min(40)),
                theme.fg_dim,
            )));
        }
        lines.push(Line::from(Span::styled(
            "Standalone Tasks".to_string(),
            theme.fg_dim,
        )));

        for task in standalone {
            let chip = task_status_chip(task.status, theme);
            let is_sel = item_index == selected;
            let line = if is_sel {
                Line::from(vec![
                    Span::styled("> ", Style::default().fg(Color::Indexed(178))),
                    chip,
                    Span::raw(" "),
                    Span::raw(task.title.clone()),
                ]).style(sel_style)
            } else {
                Line::from(vec![
                    Span::raw("  "),
                    chip,
                    Span::raw(" "),
                    Span::raw(task.title.clone()),
                ])
            };
            lines.push(line);
            item_index += 1;
        }
    }

    // Zone 3: Heartbeat
    let heartbeat_items = tasks.heartbeat_items();

    if !heartbeat_items.is_empty() {
        lines.push(Line::from(Span::styled(
            "\u{2500}".repeat(width.min(40)),
            theme.fg_dim,
        )));
        lines.push(Line::from(Span::styled(
            "\u{2665} Heartbeat".to_string(),
            theme.accent_danger,
        )));

        for item in heartbeat_items {
            let dot = heartbeat_dot(item.outcome, theme);
            let msg = item.message.as_deref().unwrap_or("OK");
            let mut spans = vec![
                Span::raw("  "),
                dot,
                Span::raw(" "),
                Span::raw(item.label.clone()),
                Span::raw("  "),
                Span::styled(msg.to_string(), theme.fg_dim),
            ];
            if matches!(item.outcome, Some(HeartbeatOutcome::Error)) {
                spans.push(Span::styled(" !", theme.accent_danger));
            }
            lines.push(Line::from(spans));
        }
    }

    // Empty state
    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            " No tasks".to_string(),
            theme.fg_dim,
        )));
    }

    lines
}

fn goal_run_status_dot(status: Option<GoalRunStatus>, theme: &ThemeTokens) -> Span<'static> {
    match status {
        Some(GoalRunStatus::Running) => Span::styled("\u{25cf}", theme.accent_secondary),
        Some(GoalRunStatus::Completed) => Span::styled("\u{25cf}", theme.accent_success),
        Some(GoalRunStatus::Failed) => Span::styled("\u{25cf}", theme.accent_danger),
        _ => Span::styled("\u{25cf}", theme.fg_dim),
    }
}

fn goal_run_status_label(status: Option<GoalRunStatus>, theme: &ThemeTokens) -> Span<'static> {
    match status {
        Some(GoalRunStatus::Running) => Span::styled(" running", theme.accent_secondary),
        Some(GoalRunStatus::Completed) => Span::styled(" done", theme.accent_success),
        Some(GoalRunStatus::Failed) => Span::styled(" failed", theme.accent_danger),
        Some(GoalRunStatus::Cancelled) => Span::styled(" cancelled", theme.fg_dim),
        _ => Span::styled(" pending", theme.fg_dim),
    }
}

fn step_status_chip(status: Option<GoalRunStatus>, theme: &ThemeTokens) -> Span<'static> {
    match status {
        None | Some(GoalRunStatus::Pending) => Span::styled("[ ]", theme.fg_dim),
        Some(GoalRunStatus::Running) => Span::styled("[~]", theme.accent_secondary),
        Some(GoalRunStatus::Completed) => Span::styled("[x]", theme.accent_success),
        Some(GoalRunStatus::Failed) => Span::styled("[!]", theme.accent_danger),
        Some(GoalRunStatus::Cancelled) => Span::styled("[C]", theme.fg_dim),
    }
}

fn task_status_chip(status: Option<TaskStatus>, theme: &ThemeTokens) -> Span<'static> {
    match status {
        None | Some(TaskStatus::Queued) => Span::styled("[ ]", theme.fg_dim),
        Some(TaskStatus::InProgress) => Span::styled("[~]", theme.accent_secondary),
        Some(TaskStatus::Completed) => Span::styled("[x]", theme.accent_success),
        Some(TaskStatus::Failed) | Some(TaskStatus::FailedAnalyzing) => {
            Span::styled("[!]", theme.accent_danger)
        }
        Some(TaskStatus::Blocked) => Span::styled("[B]", theme.fg_dim),
        Some(TaskStatus::AwaitingApproval) => Span::styled("[?]", theme.accent_primary),
        Some(TaskStatus::Cancelled) => Span::styled("[C]", theme.fg_dim),
    }
}

fn heartbeat_dot(outcome: Option<HeartbeatOutcome>, theme: &ThemeTokens) -> Span<'static> {
    match outcome {
        Some(HeartbeatOutcome::Ok) | None => Span::styled("\u{25cf}", theme.accent_success),
        Some(HeartbeatOutcome::Warn) => Span::styled("\u{25cf}", theme.accent_secondary),
        Some(HeartbeatOutcome::Error) => Span::styled("\u{25cf}", theme.accent_danger),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::sidebar::SidebarState;
    use crate::state::task::TaskState;

    #[test]
    fn task_tree_handles_empty_state() {
        let tasks = TaskState::new();
        let _sidebar = SidebarState::new();
        let _theme = ThemeTokens::default();
        assert!(tasks.goal_runs().is_empty());
        assert!(tasks.tasks().is_empty());
    }

    #[test]
    fn task_status_chips_all_variants() {
        let theme = ThemeTokens::default();
        let _ = task_status_chip(None, &theme);
        let _ = task_status_chip(Some(TaskStatus::Queued), &theme);
        let _ = task_status_chip(Some(TaskStatus::InProgress), &theme);
        let _ = task_status_chip(Some(TaskStatus::Completed), &theme);
        let _ = task_status_chip(Some(TaskStatus::Failed), &theme);
        let _ = task_status_chip(Some(TaskStatus::Blocked), &theme);
        let _ = task_status_chip(Some(TaskStatus::AwaitingApproval), &theme);
    }
}
