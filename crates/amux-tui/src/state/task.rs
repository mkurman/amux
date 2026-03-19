// Local wire type copies (will be replaced by crate::wire imports in Task 9)
// These mirror the types in state.rs
#![allow(dead_code)]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    Queued,
    InProgress,
    AwaitingApproval,
    Blocked,
    FailedAnalyzing,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Default)]
pub struct AgentTask {
    pub id: String,
    pub title: String,
    pub status: Option<TaskStatus>,
    pub progress: u8,
    pub session_id: Option<String>,
    pub goal_run_id: Option<String>,
    pub goal_step_title: Option<String>,
    pub awaiting_approval_id: Option<String>,
    pub blocked_reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoalRunStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Default)]
pub struct GoalRunStep {
    pub id: String,
    pub title: String,
    pub status: Option<GoalRunStatus>,
    pub order: u32,
}

#[derive(Debug, Clone, Default)]
pub struct GoalRun {
    pub id: String,
    pub title: String,
    pub status: Option<GoalRunStatus>,
    pub steps: Vec<GoalRunStep>,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeartbeatOutcome {
    Ok,
    Warn,
    Error,
}

#[derive(Debug, Clone, Default)]
pub struct HeartbeatItem {
    pub id: String,
    pub label: String,
    pub outcome: Option<HeartbeatOutcome>,
    pub message: Option<String>,
    pub timestamp: u64,
}

// ── TaskAction ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum TaskAction {
    TaskListReceived(Vec<AgentTask>),
    TaskUpdate(AgentTask),
    GoalRunListReceived(Vec<GoalRun>),
    GoalRunDetailReceived(GoalRun),
    GoalRunUpdate(GoalRun),
    HeartbeatItemsReceived(Vec<HeartbeatItem>),
}

// ── TaskState ─────────────────────────────────────────────────────────────────

pub struct TaskState {
    tasks: Vec<AgentTask>,
    goal_runs: Vec<GoalRun>,
    heartbeat_items: Vec<HeartbeatItem>,
}

impl TaskState {
    pub fn new() -> Self {
        Self {
            tasks: Vec::new(),
            goal_runs: Vec::new(),
            heartbeat_items: Vec::new(),
        }
    }

    pub fn tasks(&self) -> &[AgentTask] {
        &self.tasks
    }

    pub fn goal_runs(&self) -> &[GoalRun] {
        &self.goal_runs
    }

    pub fn heartbeat_items(&self) -> &[HeartbeatItem] {
        &self.heartbeat_items
    }

    pub fn task_by_id(&self, id: &str) -> Option<&AgentTask> {
        self.tasks.iter().find(|t| t.id == id)
    }

    pub fn goal_run_by_id(&self, id: &str) -> Option<&GoalRun> {
        self.goal_runs.iter().find(|r| r.id == id)
    }

    pub fn reduce(&mut self, action: TaskAction) {
        match action {
            TaskAction::TaskListReceived(tasks) => {
                self.tasks = tasks;
            }

            TaskAction::TaskUpdate(updated) => {
                if let Some(existing) = self.tasks.iter_mut().find(|t| t.id == updated.id) {
                    *existing = updated;
                } else {
                    self.tasks.push(updated);
                }
            }

            TaskAction::GoalRunListReceived(runs) => {
                self.goal_runs = runs;
            }

            TaskAction::GoalRunDetailReceived(run) | TaskAction::GoalRunUpdate(run) => {
                if let Some(existing) = self.goal_runs.iter_mut().find(|r| r.id == run.id) {
                    *existing = run;
                } else {
                    self.goal_runs.push(run);
                }
            }

            TaskAction::HeartbeatItemsReceived(items) => {
                self.heartbeat_items = items;
            }
        }
    }
}

impl Default for TaskState {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_task(id: &str, title: &str) -> AgentTask {
        AgentTask { id: id.into(), title: title.into(), ..Default::default() }
    }

    fn make_goal_run(id: &str, title: &str) -> GoalRun {
        GoalRun { id: id.into(), title: title.into(), ..Default::default() }
    }

    #[test]
    fn task_list_received_replaces_tasks() {
        let mut state = TaskState::new();
        state.reduce(TaskAction::TaskListReceived(vec![
            make_task("t1", "First"),
            make_task("t2", "Second"),
        ]));
        assert_eq!(state.tasks().len(), 2);

        // Replace with a smaller list
        state.reduce(TaskAction::TaskListReceived(vec![make_task("t3", "Third")]));
        assert_eq!(state.tasks().len(), 1);
        assert_eq!(state.tasks()[0].id, "t3");
    }

    #[test]
    fn task_update_upserts_by_id() {
        let mut state = TaskState::new();
        state.reduce(TaskAction::TaskListReceived(vec![make_task("t1", "Original")]));

        // Update existing task
        state.reduce(TaskAction::TaskUpdate(AgentTask {
            id: "t1".into(),
            title: "Updated".into(),
            status: Some(TaskStatus::InProgress),
            ..Default::default()
        }));
        assert_eq!(state.tasks().len(), 1);
        assert_eq!(state.tasks()[0].title, "Updated");
        assert_eq!(state.tasks()[0].status, Some(TaskStatus::InProgress));

        // Insert new task
        state.reduce(TaskAction::TaskUpdate(make_task("t2", "New")));
        assert_eq!(state.tasks().len(), 2);
    }

    #[test]
    fn goal_run_list_received_replaces_goal_runs() {
        let mut state = TaskState::new();
        state.reduce(TaskAction::GoalRunListReceived(vec![
            make_goal_run("g1", "Goal One"),
            make_goal_run("g2", "Goal Two"),
        ]));
        assert_eq!(state.goal_runs().len(), 2);

        state.reduce(TaskAction::GoalRunListReceived(vec![]));
        assert_eq!(state.goal_runs().len(), 0);
    }

    #[test]
    fn goal_run_detail_received_upserts() {
        let mut state = TaskState::new();
        state.reduce(TaskAction::GoalRunListReceived(vec![make_goal_run("g1", "Original")]));

        // Update via detail
        state.reduce(TaskAction::GoalRunDetailReceived(GoalRun {
            id: "g1".into(),
            title: "Detailed".into(),
            ..Default::default()
        }));
        assert_eq!(state.goal_runs().len(), 1);
        assert_eq!(state.goal_runs()[0].title, "Detailed");

        // Insert new via update
        state.reduce(TaskAction::GoalRunUpdate(make_goal_run("g2", "New Goal")));
        assert_eq!(state.goal_runs().len(), 2);
    }

    #[test]
    fn heartbeat_items_received_replaces() {
        let mut state = TaskState::new();
        let items = vec![
            HeartbeatItem { id: "h1".into(), label: "Service A".into(), ..Default::default() },
            HeartbeatItem { id: "h2".into(), label: "Service B".into(), ..Default::default() },
        ];
        state.reduce(TaskAction::HeartbeatItemsReceived(items));
        assert_eq!(state.heartbeat_items().len(), 2);

        state.reduce(TaskAction::HeartbeatItemsReceived(vec![]));
        assert_eq!(state.heartbeat_items().len(), 0);
    }

    #[test]
    fn task_by_id_returns_correct_task() {
        let mut state = TaskState::new();
        state.reduce(TaskAction::TaskListReceived(vec![
            make_task("t1", "Alpha"),
            make_task("t2", "Beta"),
        ]));
        assert_eq!(state.task_by_id("t2").map(|t| t.title.as_str()), Some("Beta"));
        assert!(state.task_by_id("unknown").is_none());
    }
}
