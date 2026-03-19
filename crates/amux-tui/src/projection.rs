#![allow(dead_code)]
//! DaemonProjection — maps wire-format ClientEvent into typed AppAction.
//!
//! This is a pure function with no side effects. Each ClientEvent variant maps
//! to zero or more AppAction values that get dispatched to state sub-modules.
//!
//! NOTE: Uses local type aliases until Task 9 resolves the state.rs → wire.rs rename.

use crate::state::{
    AppAction,
    chat::ChatAction,
    task::TaskAction,
    config::ConfigAction,
};

// Forward reference — will be `use crate::client::ClientEvent` after Task 9
// For now, re-export from here so app.rs can use it
#[derive(Debug, Clone)]
pub enum ClientEvent {
    Connected,
    Disconnected,
    SessionSpawned { session_id: String },

    ThreadList(Vec<crate::state::chat::AgentThread>),
    ThreadDetail(Option<crate::state::chat::AgentThread>),
    ThreadCreated { thread_id: String, title: String },

    TaskList(Vec<crate::state::task::AgentTask>),
    TaskUpdate(crate::state::task::AgentTask),

    GoalRunList(Vec<crate::state::task::GoalRun>),
    GoalRunDetail(Option<crate::state::task::GoalRun>),
    GoalRunUpdate(crate::state::task::GoalRun),

    AgentConfig(crate::state::config::AgentConfigSnapshot),
    AgentConfigRaw(serde_json::Value),
    ModelsFetched(Vec<crate::state::config::FetchedModel>),

    HeartbeatItems(Vec<crate::state::task::HeartbeatItem>),

    Delta { thread_id: String, content: String },
    Reasoning { thread_id: String, content: String },
    ToolCall { thread_id: String, call_id: String, name: String, arguments: String },
    ToolResult { thread_id: String, call_id: String, name: String, content: String, is_error: bool },
    Done {
        thread_id: String,
        input_tokens: u64,
        output_tokens: u64,
        cost: Option<f64>,
        provider: Option<String>,
        model: Option<String>,
        tps: Option<f64>,
        generation_ms: Option<u64>,
    },

    Error(String),
}

pub struct DaemonProjection;

impl DaemonProjection {
    /// Pure function: project a wire-format event into typed UI actions.
    pub fn project(event: ClientEvent) -> Vec<AppAction> {
        match event {
            ClientEvent::Connected => vec![
                AppAction::Connected,
                AppAction::Status("Connected to daemon".into()),
            ],
            ClientEvent::Disconnected => vec![
                AppAction::Disconnected,
                AppAction::Status("Disconnected from daemon".into()),
            ],
            ClientEvent::SessionSpawned { session_id } => vec![
                AppAction::Status(format!("Session bound: {}", session_id)),
            ],

            // Thread events → ChatAction
            ClientEvent::ThreadList(threads) => vec![
                AppAction::Chat(ChatAction::ThreadListReceived(threads)),
            ],
            ClientEvent::ThreadDetail(Some(thread)) => vec![
                AppAction::Chat(ChatAction::ThreadDetailReceived(thread)),
            ],
            ClientEvent::ThreadDetail(None) => vec![],
            ClientEvent::ThreadCreated { thread_id, title } => vec![
                AppAction::Chat(ChatAction::ThreadCreated { thread_id, title }),
            ],

            // Task events → TaskAction
            ClientEvent::TaskList(tasks) => vec![
                AppAction::Task(TaskAction::TaskListReceived(tasks)),
            ],
            ClientEvent::TaskUpdate(task) => vec![
                AppAction::Task(TaskAction::TaskUpdate(task)),
            ],

            // Goal run events → TaskAction
            ClientEvent::GoalRunList(runs) => vec![
                AppAction::Task(TaskAction::GoalRunListReceived(runs)),
            ],
            ClientEvent::GoalRunDetail(Some(run)) => vec![
                AppAction::Task(TaskAction::GoalRunDetailReceived(run)),
            ],
            ClientEvent::GoalRunDetail(None) => vec![],
            ClientEvent::GoalRunUpdate(run) => vec![
                AppAction::Task(TaskAction::GoalRunUpdate(run)),
            ],

            // Config events → ConfigAction
            ClientEvent::AgentConfig(config) => vec![
                AppAction::Config(ConfigAction::ConfigReceived(config)),
            ],
            ClientEvent::AgentConfigRaw(raw) => vec![
                AppAction::Config(ConfigAction::ConfigRawReceived(raw)),
            ],
            ClientEvent::ModelsFetched(models) => vec![
                AppAction::Config(ConfigAction::ModelsFetched(models)),
            ],

            // Heartbeat → TaskAction
            ClientEvent::HeartbeatItems(items) => vec![
                AppAction::Task(TaskAction::HeartbeatItemsReceived(items)),
            ],

            // Streaming events → ChatAction
            ClientEvent::Delta { thread_id, content } => vec![
                AppAction::Chat(ChatAction::Delta { thread_id, content }),
            ],
            ClientEvent::Reasoning { thread_id, content } => vec![
                AppAction::Chat(ChatAction::Reasoning { thread_id, content }),
            ],
            ClientEvent::ToolCall { thread_id, call_id, name, arguments } => vec![
                AppAction::Chat(ChatAction::ToolCall {
                    thread_id, call_id, name, args: arguments,
                }),
            ],
            ClientEvent::ToolResult { thread_id, call_id, name, content, is_error } => vec![
                AppAction::Chat(ChatAction::ToolResult {
                    thread_id, call_id, name, content, is_error,
                }),
            ],
            ClientEvent::Done {
                thread_id, input_tokens, output_tokens,
                cost, provider, model, tps, generation_ms,
            } => vec![
                AppAction::Chat(ChatAction::TurnDone {
                    thread_id, input_tokens, output_tokens,
                    cost, provider, model, tps, generation_ms,
                }),
            ],

            // Error → Status
            ClientEvent::Error(message) => vec![
                AppAction::Status(format!("Error: {}", message)),
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connected_produces_connected_and_status() {
        let actions = DaemonProjection::project(ClientEvent::Connected);
        assert!(actions.len() >= 2);
        assert!(actions.iter().any(|a| matches!(a, AppAction::Connected)));
    }

    #[test]
    fn delta_maps_to_chat_action() {
        let actions = DaemonProjection::project(ClientEvent::Delta {
            thread_id: "t1".into(),
            content: "hello".into(),
        });
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            AppAction::Chat(ChatAction::Delta { thread_id, content }) => {
                assert_eq!(thread_id, "t1");
                assert_eq!(content, "hello");
            }
            other => panic!("expected Chat Delta, got {:?}", other),
        }
    }

    #[test]
    fn task_list_maps_to_task_action() {
        let actions = DaemonProjection::project(ClientEvent::TaskList(vec![]));
        assert_eq!(actions.len(), 1);
        assert!(matches!(&actions[0], AppAction::Task(TaskAction::TaskListReceived(_))));
    }

    #[test]
    fn done_maps_to_turn_done() {
        let actions = DaemonProjection::project(ClientEvent::Done {
            thread_id: "t1".into(),
            input_tokens: 100,
            output_tokens: 50,
            cost: Some(0.01),
            provider: Some("openai".into()),
            model: Some("gpt-4o".into()),
            tps: Some(45.0),
            generation_ms: Some(1200),
        });
        assert!(actions.iter().any(|a| matches!(a, AppAction::Chat(ChatAction::TurnDone { .. }))));
    }

    #[test]
    fn error_maps_to_status() {
        let actions = DaemonProjection::project(ClientEvent::Error("test error".into()));
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            AppAction::Status(msg) => assert!(msg.contains("test error")),
            other => panic!("expected Status, got {:?}", other),
        }
    }

    #[test]
    fn thread_detail_none_produces_empty() {
        let actions = DaemonProjection::project(ClientEvent::ThreadDetail(None));
        assert!(actions.is_empty());
    }

    #[test]
    fn tool_call_maps_args_to_args_field() {
        let actions = DaemonProjection::project(ClientEvent::ToolCall {
            thread_id: "t1".into(),
            call_id: "c1".into(),
            name: "bash".into(),
            arguments: "ls -la".into(),
        });
        match &actions[0] {
            AppAction::Chat(ChatAction::ToolCall { args, .. }) => {
                assert_eq!(args, "ls -la");
            }
            other => panic!("expected ToolCall, got {:?}", other),
        }
    }
}
