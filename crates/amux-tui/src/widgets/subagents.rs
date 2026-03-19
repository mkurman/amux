use crate::theme::{ThemeTokens, FG_CLOSE};
use crate::state::task::{TaskState, TaskStatus, GoalRunStatus};
use crate::state::sidebar::SidebarState;
use super::pad_to_width;

// ── helpers ───────────────────────────────────────────────────────────────────

fn status_dot_for_task(status: Option<TaskStatus>, theme: &ThemeTokens) -> String {
    match status {
        Some(TaskStatus::InProgress) => format!("{}●{}", theme.accent_secondary.fg(), FG_CLOSE),
        Some(TaskStatus::Completed) => format!("{}●{}", theme.accent_success.fg(), FG_CLOSE),
        Some(TaskStatus::Failed) | Some(TaskStatus::FailedAnalyzing) => {
            format!("{}●{}", theme.accent_danger.fg(), FG_CLOSE)
        }
        Some(TaskStatus::Blocked) | Some(TaskStatus::AwaitingApproval) => {
            format!("{}●{}", theme.accent_primary.fg(), FG_CLOSE)
        }
        _ => format!("{}●{}", theme.fg_dim.fg(), FG_CLOSE),
    }
}

fn status_label_for_task(status: Option<TaskStatus>, theme: &ThemeTokens) -> String {
    match status {
        Some(TaskStatus::InProgress) => {
            format!("{}running{}", theme.accent_secondary.fg(), FG_CLOSE)
        }
        Some(TaskStatus::Completed) => format!("{}done{}", theme.accent_success.fg(), FG_CLOSE),
        Some(TaskStatus::Failed) | Some(TaskStatus::FailedAnalyzing) => {
            format!("{}failed{}", theme.accent_danger.fg(), FG_CLOSE)
        }
        Some(TaskStatus::Blocked) => format!("{}blocked{}", theme.fg_dim.fg(), FG_CLOSE),
        Some(TaskStatus::AwaitingApproval) => {
            format!("{}awaiting{}", theme.accent_primary.fg(), FG_CLOSE)
        }
        Some(TaskStatus::Queued) | None => format!("{}idle{}", theme.fg_dim.fg(), FG_CLOSE),
        Some(TaskStatus::Cancelled) => format!("{}cancelled{}", theme.fg_dim.fg(), FG_CLOSE),
    }
}

fn goal_run_dot(status: Option<GoalRunStatus>, theme: &ThemeTokens) -> String {
    match status {
        Some(GoalRunStatus::Running) => format!("{}●{}", theme.accent_secondary.fg(), FG_CLOSE),
        Some(GoalRunStatus::Completed) => format!("{}●{}", theme.accent_success.fg(), FG_CLOSE),
        Some(GoalRunStatus::Failed) => format!("{}●{}", theme.accent_danger.fg(), FG_CLOSE),
        _ => format!("{}●{}", theme.fg_dim.fg(), FG_CLOSE),
    }
}

// ── public widget ─────────────────────────────────────────────────────────────

/// Render a subagents view grouped by goal_run_id.
///
/// Goal runs act as "parent agents". Tasks without a goal_run_id go under "daemon".
/// Each group is collapsible via sidebar.is_expanded().
pub fn subagents_widget(
    tasks: &TaskState,
    sidebar: &SidebarState,
    theme: &ThemeTokens,
    width: usize,
    height: usize,
) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();

    let all_tasks = tasks.tasks();
    let goal_runs = tasks.goal_runs();

    // ── Goal-run groups ───────────────────────────────────────────────────────
    for run in goal_runs {
        let expanded = sidebar.is_expanded(&run.id);
        let arrow = if expanded { "▾" } else { "▸" };
        let dot = goal_run_dot(run.status, theme);

        // Parent "agent" line  — title doubles as agent label
        let parent_line = format!(
            "{}{}{} {} {} {}",
            theme.fg_active.fg(),
            arrow,
            FG_CLOSE,
            run.title,
            dot,
            match run.status {
                Some(GoalRunStatus::Running) => {
                    format!("{}running{}", theme.accent_secondary.fg(), FG_CLOSE)
                }
                Some(GoalRunStatus::Completed) => {
                    format!("{}done{}", theme.accent_success.fg(), FG_CLOSE)
                }
                Some(GoalRunStatus::Failed) => {
                    format!("{}failed{}", theme.accent_danger.fg(), FG_CLOSE)
                }
                _ => format!("{}idle{}", theme.fg_dim.fg(), FG_CLOSE),
            },
        );
        lines.push(pad_to_width(&parent_line, width));

        if expanded {
            // Tasks that belong to this goal run
            let child_tasks: Vec<_> = all_tasks
                .iter()
                .filter(|t| t.goal_run_id.as_deref() == Some(&run.id))
                .collect();

            if child_tasks.is_empty() {
                let none_line = format!(
                    "  {}(no tasks){}", theme.fg_dim.fg(), FG_CLOSE
                );
                lines.push(pad_to_width(&none_line, width));
            } else {
                for task in child_tasks {
                    let dot = status_dot_for_task(task.status, theme);
                    let status_lbl = status_label_for_task(task.status, theme);
                    let task_line = format!("  {} {} {}", dot, task.title, status_lbl);
                    lines.push(pad_to_width(&task_line, width));

                    // If there's a session thread, show it as └ thread: <id>
                    if let Some(ref session_id) = task.session_id {
                        let thread_line = format!(
                            "    {}└ thread: {}{}",
                            theme.fg_dim.fg(),
                            session_id,
                            FG_CLOSE
                        );
                        lines.push(pad_to_width(&thread_line, width));
                    }
                }
            }
        }
    }

    // ── Daemon group (tasks without goal_run_id) ──────────────────────────────
    let daemon_tasks: Vec<_> = all_tasks
        .iter()
        .filter(|t| t.goal_run_id.is_none())
        .collect();

    if !daemon_tasks.is_empty() {
        let expanded = sidebar.is_expanded("__daemon__");
        let arrow = if expanded { "▾" } else { "▸" };
        let daemon_header = format!(
            "{}{}{} {}daemon{}",
            theme.fg_dim.fg(),
            arrow,
            FG_CLOSE,
            theme.fg_active.fg(),
            FG_CLOSE,
        );
        lines.push(pad_to_width(&daemon_header, width));

        if expanded {
            for task in daemon_tasks {
                let dot = status_dot_for_task(task.status, theme);
                let status_lbl = status_label_for_task(task.status, theme);
                let task_line = format!("  {} {} {}", dot, task.title, status_lbl);
                lines.push(pad_to_width(&task_line, width));
            }
        }
    }

    // ── Empty state ───────────────────────────────────────────────────────────
    if lines.is_empty() {
        let empty = format!(" {}No agents{}", theme.fg_dim.fg(), FG_CLOSE);
        lines.push(pad_to_width(&empty, width));
    }

    // ── Footer aggregate counts ───────────────────────────────────────────────
    if !goal_runs.is_empty() || !all_tasks.is_empty() {
        let running_count = all_tasks
            .iter()
            .filter(|t| t.status == Some(TaskStatus::InProgress))
            .count();
        let total_count = all_tasks.len();
        let group_count = goal_runs.len();

        let footer = format!(
            " {}{} groups · {}/{} running{}",
            theme.fg_dim.fg(),
            group_count,
            running_count,
            total_count,
            FG_CLOSE,
        );
        lines.push(pad_to_width(&footer, width));
    }

    // ── Truncate / pad to height ──────────────────────────────────────────────
    while lines.len() < height {
        lines.push(" ".repeat(width));
    }
    lines.truncate(height);

    lines
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::task::{AgentTask, GoalRun, TaskState, TaskAction};
    use crate::state::sidebar::{SidebarState, SidebarAction};

    fn make_theme() -> ThemeTokens {
        ThemeTokens::default()
    }

    fn make_sidebar() -> SidebarState {
        SidebarState::new()
    }

    fn make_sidebar_with_expanded(id: &str) -> SidebarState {
        let mut s = SidebarState::new();
        s.reduce(SidebarAction::ToggleExpand(id.to_string()));
        s
    }

    // ─── basic ────────────────────────────────────────────────────────────────

    #[test]
    fn subagents_widget_returns_exact_height() {
        let tasks = TaskState::new();
        let sidebar = make_sidebar();
        let theme = make_theme();
        let lines = subagents_widget(&tasks, &sidebar, &theme, 40, 15);
        assert_eq!(lines.len(), 15);
    }

    #[test]
    fn subagents_widget_empty_shows_no_agents() {
        let tasks = TaskState::new();
        let sidebar = make_sidebar();
        let theme = make_theme();
        let lines = subagents_widget(&tasks, &sidebar, &theme, 40, 5);
        let combined = lines.join("\n");
        assert!(combined.contains("No agents"), "expected 'No agents' in empty state");
    }

    // ─── goal run groups ───────────────────────────────────────────────────────

    #[test]
    fn subagents_shows_goal_run_as_parent_agent() {
        let mut tasks = TaskState::new();
        tasks.reduce(TaskAction::GoalRunListReceived(vec![GoalRun {
            id: "g1".into(),
            title: "hermes · coding".into(),
            status: Some(GoalRunStatus::Running),
            ..Default::default()
        }]));
        let sidebar = make_sidebar();
        let theme = make_theme();
        let lines = subagents_widget(&tasks, &sidebar, &theme, 60, 10);
        let combined = lines.join("\n");
        assert!(combined.contains("hermes · coding"), "goal run title missing");
    }

    #[test]
    fn subagents_collapsed_shows_arrow_right() {
        let mut tasks = TaskState::new();
        tasks.reduce(TaskAction::GoalRunListReceived(vec![GoalRun {
            id: "g1".into(),
            title: "openclaw · research".into(),
            status: Some(GoalRunStatus::Pending),
            ..Default::default()
        }]));
        let sidebar = make_sidebar(); // collapsed
        let theme = make_theme();
        let lines = subagents_widget(&tasks, &sidebar, &theme, 60, 10);
        assert!(lines[0].contains("▸"), "collapsed group should show ▸");
    }

    #[test]
    fn subagents_expanded_shows_child_tasks() {
        let mut tasks = TaskState::new();
        tasks.reduce(TaskAction::GoalRunListReceived(vec![GoalRun {
            id: "g1".into(),
            title: "hermes · coding".into(),
            status: Some(GoalRunStatus::Running),
            ..Default::default()
        }]));
        tasks.reduce(TaskAction::TaskListReceived(vec![
            AgentTask {
                id: "t1".into(),
                title: "fix-auth-mock".into(),
                status: Some(TaskStatus::InProgress),
                goal_run_id: Some("g1".into()),
                session_id: Some("fix-tests".into()),
                ..Default::default()
            },
        ]));
        let sidebar = make_sidebar_with_expanded("g1");
        let theme = make_theme();
        let lines = subagents_widget(&tasks, &sidebar, &theme, 60, 15);
        let combined = lines.join("\n");
        assert!(combined.contains("fix-auth-mock"), "task title missing");
        assert!(combined.contains("fix-tests"), "session thread id missing");
    }

    // ─── daemon group ─────────────────────────────────────────────────────────

    #[test]
    fn subagents_daemon_group_appears_for_orphan_tasks() {
        let mut tasks = TaskState::new();
        tasks.reduce(TaskAction::TaskListReceived(vec![
            AgentTask {
                id: "t1".into(),
                title: "Background scan".into(),
                status: Some(TaskStatus::Queued),
                goal_run_id: None,
                ..Default::default()
            },
        ]));
        let sidebar = make_sidebar();
        let theme = make_theme();
        let lines = subagents_widget(&tasks, &sidebar, &theme, 60, 10);
        let combined = lines.join("\n");
        assert!(combined.contains("daemon"), "daemon group missing");
    }

    #[test]
    fn subagents_daemon_group_tasks_visible_when_expanded() {
        let mut tasks = TaskState::new();
        tasks.reduce(TaskAction::TaskListReceived(vec![
            AgentTask {
                id: "t1".into(),
                title: "Background scan".into(),
                status: Some(TaskStatus::Queued),
                goal_run_id: None,
                ..Default::default()
            },
        ]));
        let sidebar = make_sidebar_with_expanded("__daemon__");
        let theme = make_theme();
        let lines = subagents_widget(&tasks, &sidebar, &theme, 60, 10);
        let combined = lines.join("\n");
        assert!(combined.contains("Background scan"), "daemon task should be visible when expanded");
    }

    // ─── footer aggregate ─────────────────────────────────────────────────────

    #[test]
    fn subagents_footer_shows_aggregate_counts() {
        let mut tasks = TaskState::new();
        tasks.reduce(TaskAction::GoalRunListReceived(vec![
            GoalRun { id: "g1".into(), title: "Goal One".into(), ..Default::default() },
            GoalRun { id: "g2".into(), title: "Goal Two".into(), ..Default::default() },
        ]));
        tasks.reduce(TaskAction::TaskListReceived(vec![
            AgentTask {
                id: "t1".into(),
                title: "Task 1".into(),
                status: Some(TaskStatus::InProgress),
                goal_run_id: Some("g1".into()),
                ..Default::default()
            },
            AgentTask {
                id: "t2".into(),
                title: "Task 2".into(),
                status: Some(TaskStatus::Queued),
                goal_run_id: Some("g2".into()),
                ..Default::default()
            },
        ]));
        let sidebar = make_sidebar();
        let theme = make_theme();
        let lines = subagents_widget(&tasks, &sidebar, &theme, 60, 20);
        let combined = lines.join("\n");
        assert!(combined.contains("groups"), "footer should mention groups");
        assert!(combined.contains("running"), "footer should mention running");
    }

    // ─── height behavior ──────────────────────────────────────────────────────

    #[test]
    fn subagents_widget_truncates_to_height() {
        let mut tasks = TaskState::new();
        tasks.reduce(TaskAction::GoalRunListReceived(
            (0..20)
                .map(|i| GoalRun {
                    id: format!("g{}", i),
                    title: format!("Goal {}", i),
                    ..Default::default()
                })
                .collect(),
        ));
        let sidebar = make_sidebar();
        let theme = make_theme();
        let lines = subagents_widget(&tasks, &sidebar, &theme, 40, 6);
        assert_eq!(lines.len(), 6);
    }
}
