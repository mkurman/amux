#![allow(dead_code)]

use anyhow::Result;
use futures::{SinkExt, StreamExt};
use serde_json::Value;
use std::sync::Mutex;
use tokio::sync::mpsc;
use tokio_util::codec::Framed;
use tracing::{debug, error, info, warn};

use amux_protocol::{default_tcp_addr, AmuxCodec, ClientMessage, DaemonMessage};

use crate::wire::{
    AgentConfigSnapshot, AgentTask, AgentThread, FetchedModel, GoalRun, GoalRunStatus,
    HeartbeatItem, TaskStatus,
};

#[cfg(unix)]
use tokio::net::UnixStream;

#[derive(Debug, Clone)]
pub enum ClientEvent {
    Connected,
    Disconnected,
    SessionSpawned { session_id: String },

    ThreadList(Vec<AgentThread>),
    ThreadDetail(Option<AgentThread>),
    ThreadCreated { thread_id: String, title: String },
    TaskList(Vec<AgentTask>),
    TaskUpdate(AgentTask),
    GoalRunList(Vec<GoalRun>),
    GoalRunDetail(Option<GoalRun>),
    GoalRunUpdate(GoalRun),
    AgentConfig(AgentConfigSnapshot),
    AgentConfigRaw(Value),
    ModelsFetched(Vec<FetchedModel>),
    HeartbeatItems(Vec<HeartbeatItem>),

    Delta { thread_id: String, content: String },
    Reasoning { thread_id: String, content: String },
    ToolCall {
        thread_id: String,
        call_id: String,
        name: String,
        arguments: String,
    },
    ToolResult {
        thread_id: String,
        call_id: String,
        name: String,
        content: String,
        is_error: bool,
    },
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

pub struct DaemonClient {
    event_tx: mpsc::Sender<ClientEvent>,
    request_tx: mpsc::UnboundedSender<ClientMessage>,
    request_rx: Mutex<Option<mpsc::UnboundedReceiver<ClientMessage>>>,
}

impl DaemonClient {
    pub fn new(event_tx: mpsc::Sender<ClientEvent>) -> Self {
        let (request_tx, request_rx) = mpsc::unbounded_channel();
        Self {
            event_tx,
            request_tx,
            request_rx: Mutex::new(Some(request_rx)),
        }
    }

    pub async fn connect(&self) -> Result<()> {
        let event_tx = self.event_tx.clone();
        let Some(request_rx) = self.request_rx.lock().expect("request mutex poisoned").take() else {
            return Ok(());
        };

        tokio::spawn(async move {
            #[cfg(unix)]
            {
                let runtime_dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_string());
                let socket_path = std::path::PathBuf::from(runtime_dir).join("tamux-daemon.sock");

                if socket_path.exists() {
                    match UnixStream::connect(&socket_path).await {
                        Ok(stream) => {
                            info!("Connected to daemon via unix socket");
                            let _ = event_tx.send(ClientEvent::Connected).await;
                            let framed = Framed::new(stream, AmuxCodec);
                            Self::handle_connection(framed, event_tx, request_rx).await;
                            return;
                        }
                        Err(err) => warn!("Unix socket exists but connection failed: {}", err),
                    }
                }
            }

            let addr = Self::resolve_daemon_addr(&default_tcp_addr());
            match tokio::net::TcpStream::connect(&addr).await {
                Ok(stream) => {
                    info!("Connected to daemon via tcp {}", addr);
                    let _ = event_tx.send(ClientEvent::Connected).await;
                    let framed = Framed::new(stream, AmuxCodec);
                    Self::handle_connection(framed, event_tx, request_rx).await;
                }
                Err(err) => {
                    let _ = event_tx
                        .send(ClientEvent::Error(format!(
                            "Cannot connect to daemon at {} ({})",
                            addr, err
                        )))
                        .await;
                    let _ = event_tx.send(ClientEvent::Disconnected).await;
                }
            }
        });

        Ok(())
    }

    fn resolve_daemon_addr(default_addr: &str) -> String {
        #[cfg(target_os = "linux")]
        {
            if std::path::Path::new("/proc/sys/fs/binfmt_misc/WSLInterop").exists()
                || std::path::Path::new("/run/WSL").exists()
            {
                if let Ok(contents) = std::fs::read_to_string("/etc/resolv.conf") {
                    for line in contents.lines() {
                        if line.starts_with("nameserver") {
                            if let Some(host_ip) = line.split_whitespace().nth(1) {
                                let port = default_addr.split(':').nth(1).unwrap_or("17563");
                                return format!("{}:{}", host_ip, port);
                            }
                        }
                    }
                }
            }
        }

        default_addr.to_string()
    }

    async fn handle_connection<S>(
        framed: Framed<S, AmuxCodec>,
        event_tx: mpsc::Sender<ClientEvent>,
        mut request_rx: mpsc::UnboundedReceiver<ClientMessage>,
    ) where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
    {
        let (mut sink, mut stream) = framed.split();

        for request in [ClientMessage::AgentSubscribe, ClientMessage::AgentListThreads] {
            if let Err(err) = sink.send(request).await {
                error!("Failed initial daemon request: {}", err);
                let _ = event_tx
                    .send(ClientEvent::Error(format!("Protocol error: {}", err)))
                    .await;
                let _ = event_tx.send(ClientEvent::Disconnected).await;
                return;
            }
        }

        loop {
            tokio::select! {
                inbound = stream.next() => {
                    match inbound {
                        Some(Ok(message)) => {
                            if !Self::handle_daemon_message(message, &event_tx).await {
                                break;
                            }
                        }
                        Some(Err(err)) => {
                            let _ = event_tx.send(ClientEvent::Error(format!("Connection error: {}", err))).await;
                            break;
                        }
                        None => break,
                    }
                }
                outbound = request_rx.recv() => {
                    match outbound {
                        Some(request) => {
                            if let Err(err) = sink.send(request).await {
                                let _ = event_tx.send(ClientEvent::Error(format!("Send error: {}", err))).await;
                                break;
                            }
                        }
                        None => break,
                    }
                }
            }
        }

        let _ = event_tx.send(ClientEvent::Disconnected).await;
    }

    async fn handle_daemon_message(message: DaemonMessage, event_tx: &mpsc::Sender<ClientEvent>) -> bool {
        match message {
            DaemonMessage::AgentEvent { event_json } => match serde_json::from_str::<Value>(&event_json) {
                Ok(event) => Self::dispatch_agent_event(event, event_tx).await,
                Err(err) => warn!("Failed to parse agent event: {}", err),
            },
            DaemonMessage::AgentThreadList { threads_json } => match serde_json::from_str::<Vec<AgentThread>>(&threads_json) {
                Ok(threads) => {
                    let _ = event_tx.send(ClientEvent::ThreadList(threads)).await;
                }
                Err(err) => warn!("Failed to parse thread list: {}", err),
            },
            DaemonMessage::AgentThreadDetail { thread_json } => match serde_json::from_str::<Option<AgentThread>>(&thread_json) {
                Ok(thread) => {
                    let _ = event_tx.send(ClientEvent::ThreadDetail(thread)).await;
                }
                Err(err) => warn!("Failed to parse thread detail: {}", err),
            },
            DaemonMessage::AgentTaskList { tasks_json } => match serde_json::from_str::<Vec<AgentTask>>(&tasks_json) {
                Ok(tasks) => {
                    let _ = event_tx.send(ClientEvent::TaskList(tasks)).await;
                }
                Err(err) => warn!("Failed to parse task list: {}", err),
            },
            DaemonMessage::AgentGoalRunList { goal_runs_json } => {
                match serde_json::from_str::<Vec<GoalRun>>(&goal_runs_json) {
                    Ok(goal_runs) => {
                        let _ = event_tx.send(ClientEvent::GoalRunList(goal_runs)).await;
                    }
                    Err(err) => warn!("Failed to parse goal run list: {}", err),
                }
            }
            DaemonMessage::AgentGoalRunDetail { goal_run_json } => {
                match serde_json::from_str::<Option<GoalRun>>(&goal_run_json) {
                    Ok(goal_run) => {
                        let _ = event_tx.send(ClientEvent::GoalRunDetail(goal_run)).await;
                    }
                    Err(err) => warn!("Failed to parse goal run detail: {}", err),
                }
            }
            DaemonMessage::AgentConfigResponse { config_json } => {
                match serde_json::from_str::<Value>(&config_json) {
                    Ok(raw) => {
                        if let Ok(config) = serde_json::from_value::<AgentConfigSnapshot>(raw.clone()) {
                            let _ = event_tx.send(ClientEvent::AgentConfig(config)).await;
                        }
                        let _ = event_tx.send(ClientEvent::AgentConfigRaw(raw)).await;
                    }
                    Err(err) => warn!("Failed to parse agent config response: {}", err),
                }
            }
            DaemonMessage::AgentHeartbeatItems { items_json } => {
                match serde_json::from_str::<Vec<HeartbeatItem>>(&items_json) {
                    Ok(items) => {
                        let _ = event_tx.send(ClientEvent::HeartbeatItems(items)).await;
                    }
                    Err(err) => warn!("Failed to parse heartbeat items: {}", err),
                }
            }
            DaemonMessage::SessionSpawned { id } => {
                let _ = event_tx
                    .send(ClientEvent::SessionSpawned {
                        session_id: id.to_string(),
                    })
                    .await;
            }
            DaemonMessage::Error { message } => {
                let _ = event_tx.send(ClientEvent::Error(message)).await;
            }
            other => {
                debug!("Ignoring daemon message: {:?}", other);
            }
        }

        true
    }

    async fn dispatch_agent_event(event: Value, event_tx: &mpsc::Sender<ClientEvent>) {
        let Some(kind) = event.get("type").and_then(Value::as_str) else {
            return;
        };

        match kind {
            "thread_created" => {
                let _ = event_tx
                    .send(ClientEvent::ThreadCreated {
                        thread_id: get_string(&event, "thread_id").unwrap_or_default(),
                        title: get_string(&event, "title").unwrap_or_else(|| "New Conversation".to_string()),
                    })
                    .await;
            }
            "delta" => {
                let _ = event_tx
                    .send(ClientEvent::Delta {
                        thread_id: get_string(&event, "thread_id").unwrap_or_default(),
                        content: get_string(&event, "content").unwrap_or_default(),
                    })
                    .await;
            }
            "reasoning" => {
                let _ = event_tx
                    .send(ClientEvent::Reasoning {
                        thread_id: get_string(&event, "thread_id").unwrap_or_default(),
                        content: get_string(&event, "content").unwrap_or_default(),
                    })
                    .await;
            }
            "tool_call" => {
                let _ = event_tx
                    .send(ClientEvent::ToolCall {
                        thread_id: get_string(&event, "thread_id").unwrap_or_default(),
                        call_id: get_string(&event, "call_id").unwrap_or_default(),
                        name: get_string(&event, "name").unwrap_or_default(),
                        arguments: get_string_lossy(&event, "arguments"),
                    })
                    .await;
            }
            "tool_result" => {
                let _ = event_tx
                    .send(ClientEvent::ToolResult {
                        thread_id: get_string(&event, "thread_id").unwrap_or_default(),
                        call_id: get_string(&event, "call_id").unwrap_or_default(),
                        name: get_string(&event, "name").unwrap_or_default(),
                        content: get_string_lossy(&event, "content"),
                        is_error: event.get("is_error").and_then(Value::as_bool).unwrap_or(false),
                    })
                    .await;
            }
            "done" => {
                let _ = event_tx
                    .send(ClientEvent::Done {
                        thread_id: get_string(&event, "thread_id").unwrap_or_default(),
                        input_tokens: event.get("input_tokens").and_then(Value::as_u64).unwrap_or(0),
                        output_tokens: event.get("output_tokens").and_then(Value::as_u64).unwrap_or(0),
                        cost: event.get("cost").and_then(Value::as_f64),
                        provider: get_string(&event, "provider"),
                        model: get_string(&event, "model"),
                        tps: event.get("tps").and_then(Value::as_f64),
                        generation_ms: event.get("generation_ms").and_then(Value::as_u64),
                    })
                    .await;
            }
            "error" => {
                let _ = event_tx
                    .send(ClientEvent::Error(
                        get_string(&event, "message").unwrap_or_else(|| "Unknown agent error".to_string()),
                    ))
                    .await;
            }
            "task_update" => {
                let task = event
                    .get("task")
                    .cloned()
                    .and_then(|raw| serde_json::from_value::<AgentTask>(raw).ok())
                    .unwrap_or_else(|| AgentTask {
                        id: get_string(&event, "task_id").unwrap_or_default(),
                        title: get_string(&event, "message")
                            .unwrap_or_else(|| "Task update".to_string()),
                        status: event
                            .get("status")
                            .cloned()
                            .and_then(|raw| serde_json::from_value::<TaskStatus>(raw).ok()),
                        progress: event.get("progress").and_then(Value::as_u64).unwrap_or(0) as u8,
                        session_id: None,
                        goal_run_id: None,
                        goal_step_title: None,
                        awaiting_approval_id: None,
                        blocked_reason: get_string(&event, "message"),
                    });

                let _ = event_tx.send(ClientEvent::TaskUpdate(task)).await;
            }
            "goal_run_update" => {
                let goal_run = event
                    .get("goal_run")
                    .cloned()
                    .and_then(|raw| serde_json::from_value::<GoalRun>(raw).ok())
                    .unwrap_or_else(|| GoalRun {
                        id: get_string(&event, "goal_run_id").unwrap_or_default(),
                        title: get_string(&event, "message")
                            .unwrap_or_else(|| "Goal run update".to_string()),
                        status: event
                            .get("status")
                            .cloned()
                            .and_then(|raw| serde_json::from_value::<GoalRunStatus>(raw).ok()),
                        last_error: get_string(&event, "message"),
                        ..GoalRun::default()
                    });

                let _ = event_tx.send(ClientEvent::GoalRunUpdate(goal_run)).await;
            }
            _ => {}
        }
    }

    fn send(&self, request: ClientMessage) -> Result<()> {
        self.request_tx.send(request)?;
        Ok(())
    }

    pub fn refresh(&self) -> Result<()> {
        self.send(ClientMessage::AgentListThreads)
    }

    pub fn refresh_services(&self) -> Result<()> {
        for request in [
            ClientMessage::AgentListTasks,
            ClientMessage::AgentListGoalRuns,
            ClientMessage::AgentGetConfig,
            ClientMessage::AgentHeartbeatGetItems,
        ] {
            self.send(request)?;
        }
        Ok(())
    }

    pub fn request_goal_run(&self, goal_run_id: impl Into<String>) -> Result<()> {
        self.send(ClientMessage::AgentGetGoalRun {
            goal_run_id: goal_run_id.into(),
        })
    }

    pub fn request_thread(&self, thread_id: impl Into<String>) -> Result<()> {
        self.send(ClientMessage::AgentGetThread {
            thread_id: thread_id.into(),
        })
    }

    pub fn send_message(
        &self,
        thread_id: Option<String>,
        content: String,
        session_id: Option<String>,
    ) -> Result<()> {
        let _ = session_id; // reserved for future use
        self.send(ClientMessage::AgentSendMessage {
            thread_id,
            content,
        })
    }

    pub fn spawn_session(
        &self,
        shell: Option<String>,
        cwd: Option<String>,
        cols: u16,
        rows: u16,
    ) -> Result<()> {
        self.send(ClientMessage::SpawnSession {
            shell,
            cwd,
            env: None,
            workspace_id: None,
            cols,
            rows,
        })
    }

    pub fn control_goal_run(&self, goal_run_id: String, action: String) -> Result<()> {
        self.send(ClientMessage::AgentControlGoalRun {
            goal_run_id,
            action,
            step_index: None,
        })
    }

    pub fn fetch_models(&self, _provider_id: String, _base_url: String, _api_key: String) -> Result<()> {
        // Models fetching is not yet supported in the protocol; no-op stub.
        warn!("fetch_models: not supported by current protocol");
        Ok(())
    }

    pub fn set_config_json(&self, config_json: String) -> Result<()> {
        self.send(ClientMessage::AgentSetConfig { config_json })
    }

    pub fn resolve_task_approval(&self, approval_id: String, decision: String) -> Result<()> {
        use amux_protocol::ApprovalDecision;
        let decision = match decision.as_str() {
            "allow_once" | "approve_once" => ApprovalDecision::ApproveOnce,
            "allow_session" | "approve_session" => ApprovalDecision::ApproveSession,
            _ => ApprovalDecision::Deny,
        };
        // ResolveApproval requires a SessionId; we use a nil UUID as placeholder
        // since the daemon routes by approval_id.
        self.send(ClientMessage::ResolveApproval {
            id: uuid::Uuid::nil(),
            approval_id,
            decision,
        })
    }
}

fn get_string(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(ToOwned::to_owned)
}

fn get_string_lossy(value: &Value, key: &str) -> String {
    match value.get(key) {
        Some(Value::String(inner)) => inner.clone(),
        Some(other) => other.to_string(),
        None => String::new(),
    }
}
