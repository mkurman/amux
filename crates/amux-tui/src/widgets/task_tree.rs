use crate::theme::{ThemeTokens, FG_CLOSE};
use crate::state::task::{TaskState, TaskStatus, GoalRunStatus, HeartbeatOutcome};
use crate::state::sidebar::SidebarState;
use super::pad_to_width;

// ── helpers ───────────────────────────────────────────────────────────────────

fn goal_run_status_dot(status: Option<GoalRunStatus>, theme: &ThemeTokens) -> String {
    match status {
        Some(GoalRunStatus::Running) => format!("{}●{}", theme.accent_secondary.fg(), FG_CLOSE),
        Some(GoalRunStatus::Completed) => format!("{}●{}", theme.accent_success.fg(), FG_CLOSE),
        Some(GoalRunStatus::Failed) => format!("{}●{}", theme.accent_danger.fg(), FG_CLOSE),
        Some(GoalRunStatus::Cancelled) | Some(GoalRunStatus::Pending) | None => {
            format!("{}●{}", theme.fg_dim.fg(), FG_CLOSE)
        }
    }
}

fn task_status_chip(status: Option<TaskStatus>, theme: &ThemeTokens) -> String {
    match status {
        None | Some(TaskStatus::Queued) => format!("{}\\[ ]{}", theme.fg_dim.fg(), FG_CLOSE),
        Some(TaskStatus::InProgress) => format!("{}\\[~]{}", theme.accent_secondary.fg(), FG_CLOSE),
        Some(TaskStatus::Completed) => format!("{}\\[x]{}", theme.accent_success.fg(), FG_CLOSE),
        Some(TaskStatus::Failed) | Some(TaskStatus::FailedAnalyzing) => {
            format!("{}\\[!]{}", theme.accent_danger.fg(), FG_CLOSE)
        }
        Some(TaskStatus::Blocked) => format!("{}\\[B]{}", theme.fg_dim.fg(), FG_CLOSE),
        Some(TaskStatus::AwaitingApproval) => {
            format!("{}\\[?]{}", theme.accent_primary.fg(), FG_CLOSE)
        }
        Some(TaskStatus::Cancelled) => format!("{}\\[C]{}", theme.fg_dim.fg(), FG_CLOSE),
    }
}

fn heartbeat_dot(outcome: Option<HeartbeatOutcome>, theme: &ThemeTokens) -> String {
    match outcome {
        Some(HeartbeatOutcome::Ok) | None => {
            format!("{}●{}", theme.accent_success.fg(), FG_CLOSE)
        }
        Some(HeartbeatOutcome::Warn) => format!("{}●{}", theme.accent_secondary.fg(), FG_CLOSE),
        Some(HeartbeatOutcome::Error) => format!("{}●{}", theme.accent_danger.fg(), FG_CLOSE),
    }
}

fn separator_line(width: usize, theme: &ThemeTokens) -> String {
    format!("{}{}{}", theme.fg_dim.fg(), "─".repeat(width), FG_CLOSE)
}

// ── public widget ─────────────────────────────────────────────────────────────

/// Render the task-tree sidebar body as a vector of lines (no border, no padding).
///
/// Three zones:
///   1. Goal Runs  — collapsible tree with step children
///   2. Standalone Tasks  — tasks without a goal_run_id
///   3. Heartbeat  — service health dots
pub fn task_tree_widget(
    tasks: &TaskState,
    sidebar: &SidebarState,
    theme: &ThemeTokens,
    width: usize,
    height: usize,
) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();

    // ── Zone 1: Goal Runs ─────────────────────────────────────────────────────
    let goal_runs = tasks.goal_runs();

    for run in goal_runs {
        let expanded = sidebar.is_expanded(&run.id);
        let arrow = if expanded { "▾" } else { "▸" };
        let dot = goal_run_status_dot(run.status, theme);
        let status_label = match run.status {
            Some(GoalRunStatus::Running) => {
                format!(" {}running{}", theme.accent_secondary.fg(), FG_CLOSE)
            }
            Some(GoalRunStatus::Completed) => {
                format!(" {}done{}", theme.accent_success.fg(), FG_CLOSE)
            }
            Some(GoalRunStatus::Failed) => {
                format!(" {}failed{}", theme.accent_danger.fg(), FG_CLOSE)
            }
            Some(GoalRunStatus::Cancelled) => {
                format!(" {}cancelled{}", theme.fg_dim.fg(), FG_CLOSE)
            }
            Some(GoalRunStatus::Pending) | None => {
                format!(" {}pending{}", theme.fg_dim.fg(), FG_CLOSE)
            }
        };

        let header = format!(
            "{}{}{} {} {} {}",
            theme.fg_active.fg(),
            arrow,
            FG_CLOSE,
            run.title,
            dot,
            status_label,
        );
        lines.push(pad_to_width(&header, width));

        if expanded {
            // Sort steps by order
            let mut steps = run.steps.clone();
            steps.sort_by_key(|s| s.order);

            for step in &steps {
                let chip = match step.status {
                    None | Some(GoalRunStatus::Pending) => {
                        format!("{}\\[ ]{}", theme.fg_dim.fg(), FG_CLOSE)
                    }
                    Some(GoalRunStatus::Running) => {
                        format!("{}\\[~]{}", theme.accent_secondary.fg(), FG_CLOSE)
                    }
                    Some(GoalRunStatus::Completed) => {
                        format!("{}\\[x]{}", theme.accent_success.fg(), FG_CLOSE)
                    }
                    Some(GoalRunStatus::Failed) => {
                        format!("{}\\[!]{}", theme.accent_danger.fg(), FG_CLOSE)
                    }
                    Some(GoalRunStatus::Cancelled) => {
                        format!("{}\\[C]{}", theme.fg_dim.fg(), FG_CLOSE)
                    }
                };
                let step_line =
                    format!("  {} {}", chip, step.title);
                lines.push(pad_to_width(&step_line, width));
            }
        }
    }

    // ── Zone 2: Standalone Tasks ──────────────────────────────────────────────
    let standalone: Vec<_> = tasks
        .tasks()
        .iter()
        .filter(|t| t.goal_run_id.is_none())
        .collect();

    if !standalone.is_empty() {
        if !goal_runs.is_empty() {
            // separator
            lines.push(pad_to_width(&separator_line(width, theme), width));
        }
        let section_header = format!(
            "{}Standalone Tasks{}",
            theme.fg_dim.fg(),
            FG_CLOSE
        );
        lines.push(pad_to_width(&section_header, width));

        for task in standalone {
            let chip = task_status_chip(task.status, theme);
            let task_line = format!("  {} {}", chip, task.title);
            lines.push(pad_to_width(&task_line, width));
        }
    }

    // ── Zone 3: Heartbeat ─────────────────────────────────────────────────────
    let heartbeat_items = tasks.heartbeat_items();

    if !heartbeat_items.is_empty() {
        lines.push(pad_to_width(&separator_line(width, theme), width));
        let hb_header = format!("{}♥ Heartbeat{}", theme.accent_danger.fg(), FG_CLOSE);
        lines.push(pad_to_width(&hb_header, width));

        for item in heartbeat_items {
            let dot = heartbeat_dot(item.outcome, theme);
            let msg = item.message.as_deref().unwrap_or("OK");
            let warn = if matches!(item.outcome, Some(HeartbeatOutcome::Error)) {
                format!(" {}!{}", theme.accent_danger.fg(), FG_CLOSE)
            } else {
                String::new()
            };
            let hb_line = format!(
                "  {} {}  {}{}{}",
                dot,
                item.label,
                theme.fg_dim.fg(),
                msg,
                FG_CLOSE,
            );
            let hb_line = format!("{}{}", hb_line, warn);
            lines.push(pad_to_width(&hb_line, width));
        }
    }

    // ── Empty state ───────────────────────────────────────────────────────────
    if lines.is_empty() {
        let empty = format!(" {}No tasks{}", theme.fg_dim.fg(), FG_CLOSE);
        lines.push(pad_to_width(&empty, width));
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
    use crate::state::task::{AgentTask, GoalRun, GoalRunStep, HeartbeatItem, TaskState, TaskAction};
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

    // ─── basic rendering ───────────────────────────────────────────────────────

    #[test]
    fn task_tree_returns_exact_height() {
        let tasks = TaskState::new();
        let sidebar = make_sidebar();
        let theme = make_theme();
        let lines = task_tree_widget(&tasks, &sidebar, &theme, 40, 20);
        assert_eq!(lines.len(), 20);
    }

    #[test]
    fn task_tree_empty_state_shows_no_tasks() {
        let tasks = TaskState::new();
        let sidebar = make_sidebar();
        let theme = make_theme();
        let lines = task_tree_widget(&tasks, &sidebar, &theme, 40, 5);
        let combined = lines.join("\n");
        assert!(combined.contains("No tasks"), "expected 'No tasks' in output");
    }

    // ─── goal runs ─────────────────────────────────────────────────────────────

    #[test]
    fn task_tree_shows_goal_run_title() {
        let mut tasks = TaskState::new();
        tasks.reduce(TaskAction::GoalRunListReceived(vec![GoalRun {
            id: "g1".into(),
            title: "Fix auth tests".into(),
            status: Some(GoalRunStatus::Running),
            ..Default::default()
        }]));
        let sidebar = make_sidebar();
        let theme = make_theme();
        let lines = task_tree_widget(&tasks, &sidebar, &theme, 60, 10);
        let combined = lines.join("\n");
        assert!(combined.contains("Fix auth tests"), "goal run title missing");
    }

    #[test]
    fn task_tree_collapsed_goal_run_shows_arrow_right() {
        let mut tasks = TaskState::new();
        tasks.reduce(TaskAction::GoalRunListReceived(vec![GoalRun {
            id: "g1".into(),
            title: "Deploy staging".into(),
            status: Some(GoalRunStatus::Failed),
            ..Default::default()
        }]));
        let sidebar = make_sidebar(); // not expanded
        let theme = make_theme();
        let lines = task_tree_widget(&tasks, &sidebar, &theme, 60, 10);
        // Should show ▸ (collapsed)
        assert!(lines[0].contains("▸"), "collapsed goal run should show ▸");
    }

    #[test]
    fn task_tree_expanded_goal_run_shows_steps() {
        let mut tasks = TaskState::new();
        tasks.reduce(TaskAction::GoalRunListReceived(vec![GoalRun {
            id: "g1".into(),
            title: "Fix auth tests".into(),
            status: Some(GoalRunStatus::Running),
            steps: vec![
                GoalRunStep {
                    id: "s1".into(),
                    title: "Investigate failures".into(),
                    status: Some(GoalRunStatus::Completed),
                    order: 0,
                },
                GoalRunStep {
                    id: "s2".into(),
                    title: "Fix root cause".into(),
                    status: Some(GoalRunStatus::Running),
                    order: 1,
                },
            ],
            ..Default::default()
        }]));
        let sidebar = make_sidebar_with_expanded("g1");
        let theme = make_theme();
        let lines = task_tree_widget(&tasks, &sidebar, &theme, 60, 15);
        let combined = lines.join("\n");
        assert!(combined.contains("▾"), "expanded goal run should show ▾");
        assert!(combined.contains("Investigate failures"), "step 1 title missing");
        assert!(combined.contains("Fix root cause"), "step 2 title missing");
    }

    #[test]
    fn task_tree_step_status_chips() {
        let mut tasks = TaskState::new();
        tasks.reduce(TaskAction::GoalRunListReceived(vec![GoalRun {
            id: "g1".into(),
            title: "Goal".into(),
            status: Some(GoalRunStatus::Running),
            steps: vec![
                GoalRunStep {
                    id: "s1".into(),
                    title: "Pending step".into(),
                    status: None,
                    order: 0,
                },
                GoalRunStep {
                    id: "s2".into(),
                    title: "Done step".into(),
                    status: Some(GoalRunStatus::Completed),
                    order: 1,
                },
                GoalRunStep {
                    id: "s3".into(),
                    title: "Failed step".into(),
                    status: Some(GoalRunStatus::Failed),
                    order: 2,
                },
            ],
            ..Default::default()
        }]));
        let sidebar = make_sidebar_with_expanded("g1");
        let theme = make_theme();
        let lines = task_tree_widget(&tasks, &sidebar, &theme, 60, 20);
        let combined = lines.join("\n");
        // Escaped brackets: \[ ] \[x] \[!]
        assert!(combined.contains("\\[ ]"), "pending chip missing");
        assert!(combined.contains("\\[x]"), "done chip missing");
        assert!(combined.contains("\\[!]"), "failed chip missing");
    }

    // ─── standalone tasks ──────────────────────────────────────────────────────

    #[test]
    fn task_tree_shows_standalone_tasks_section() {
        let mut tasks = TaskState::new();
        tasks.reduce(TaskAction::TaskListReceived(vec![
            AgentTask {
                id: "t1".into(),
                title: "Analyze logs".into(),
                status: Some(TaskStatus::InProgress),
                goal_run_id: None,
                ..Default::default()
            },
        ]));
        let sidebar = make_sidebar();
        let theme = make_theme();
        let lines = task_tree_widget(&tasks, &sidebar, &theme, 60, 15);
        let combined = lines.join("\n");
        assert!(combined.contains("Standalone Tasks"), "section header missing");
        assert!(combined.contains("Analyze logs"), "task title missing");
    }

    #[test]
    fn task_tree_tasks_with_goal_run_id_are_not_standalone() {
        let mut tasks = TaskState::new();
        tasks.reduce(TaskAction::TaskListReceived(vec![
            AgentTask {
                id: "t1".into(),
                title: "Linked task".into(),
                status: Some(TaskStatus::InProgress),
                goal_run_id: Some("g1".into()),
                ..Default::default()
            },
            AgentTask {
                id: "t2".into(),
                title: "Standalone task".into(),
                status: Some(TaskStatus::Queued),
                goal_run_id: None,
                ..Default::default()
            },
        ]));
        let sidebar = make_sidebar();
        let theme = make_theme();
        let lines = task_tree_widget(&tasks, &sidebar, &theme, 60, 20);
        let combined = lines.join("\n");
        assert!(combined.contains("Standalone Tasks"));
        assert!(combined.contains("Standalone task"));
        assert!(!combined.contains("Linked task"), "linked task should not appear in standalone zone");
    }

    // ─── heartbeat ─────────────────────────────────────────────────────────────

    #[test]
    fn task_tree_shows_heartbeat_zone() {
        let mut tasks = TaskState::new();
        tasks.reduce(TaskAction::HeartbeatItemsReceived(vec![
            HeartbeatItem {
                id: "h1".into(),
                label: "daemon".into(),
                outcome: Some(HeartbeatOutcome::Ok),
                message: Some("OK".into()),
                timestamp: 0,
            },
            HeartbeatItem {
                id: "h2".into(),
                label: "memory".into(),
                outcome: Some(HeartbeatOutcome::Error),
                message: Some("92%".into()),
                timestamp: 0,
            },
        ]));
        let sidebar = make_sidebar();
        let theme = make_theme();
        let lines = task_tree_widget(&tasks, &sidebar, &theme, 60, 20);
        let combined = lines.join("\n");
        assert!(combined.contains("♥ Heartbeat"), "heartbeat section missing");
        assert!(combined.contains("daemon"), "daemon label missing");
        assert!(combined.contains("memory"), "memory label missing");
    }

    #[test]
    fn task_tree_heartbeat_error_shows_warning_marker() {
        let mut tasks = TaskState::new();
        tasks.reduce(TaskAction::HeartbeatItemsReceived(vec![
            HeartbeatItem {
                id: "h1".into(),
                label: "disk".into(),
                outcome: Some(HeartbeatOutcome::Error),
                message: Some("99%".into()),
                timestamp: 0,
            },
        ]));
        let sidebar = make_sidebar();
        let theme = make_theme();
        let lines = task_tree_widget(&tasks, &sidebar, &theme, 60, 20);
        let combined = lines.join("\n");
        assert!(combined.contains("!"), "error heartbeat should show ! marker");
    }

    // ─── task status chips ─────────────────────────────────────────────────────

    #[test]
    fn task_status_chips_all_variants() {
        let theme = make_theme();
        assert!(task_status_chip(None, &theme).contains("\\[ ]"));
        assert!(task_status_chip(Some(TaskStatus::Queued), &theme).contains("\\[ ]"));
        assert!(task_status_chip(Some(TaskStatus::InProgress), &theme).contains("\\[~]"));
        assert!(task_status_chip(Some(TaskStatus::Completed), &theme).contains("\\[x]"));
        assert!(task_status_chip(Some(TaskStatus::Failed), &theme).contains("\\[!]"));
        assert!(task_status_chip(Some(TaskStatus::Blocked), &theme).contains("\\[B]"));
        assert!(task_status_chip(Some(TaskStatus::AwaitingApproval), &theme).contains("\\[?]"));
    }

    // ─── height truncation / padding ──────────────────────────────────────────

    #[test]
    fn task_tree_truncates_to_height() {
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
        let lines = task_tree_widget(&tasks, &sidebar, &theme, 40, 8);
        assert_eq!(lines.len(), 8);
    }
}
