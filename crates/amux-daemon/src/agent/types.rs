use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Agent configuration (persisted to ~/.tamux/agent/config.json)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_provider")]
    pub provider: String,
    #[serde(default)]
    pub base_url: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default = "default_system_prompt")]
    pub system_prompt: String,
    #[serde(default = "default_max_tool_loops")]
    pub max_tool_loops: u32,
    #[serde(default = "default_task_poll_secs")]
    pub task_poll_interval_secs: u64,
    #[serde(default = "default_heartbeat_mins")]
    pub heartbeat_interval_mins: u64,
    #[serde(default)]
    pub tools: ToolsConfig,
    /// Additional provider configurations keyed by provider name.
    #[serde(default)]
    pub providers: HashMap<String, ProviderConfig>,
    /// Gateway configuration for chat platform connections.
    #[serde(default)]
    pub gateway: GatewayConfig,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GatewayConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub slack_token: String,
    #[serde(default)]
    pub telegram_token: String,
    #[serde(default)]
    pub discord_token: String,
    #[serde(default)]
    pub command_prefix: String,
}

fn default_provider() -> String {
    "openai".into()
}
fn default_system_prompt() -> String {
    "You are tamux, an always-on agentic terminal multiplexer assistant. You can execute terminal commands, monitor systems, and send messages to connected chat platforms. Use your tools proactively. Be concise and direct.".into()
}
fn default_max_tool_loops() -> u32 {
    25
}
fn default_task_poll_secs() -> u64 {
    10
}
fn default_heartbeat_mins() -> u64 {
    30
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: default_provider(),
            base_url: String::new(),
            model: String::new(),
            api_key: String::new(),
            system_prompt: default_system_prompt(),
            max_tool_loops: default_max_tool_loops(),
            task_poll_interval_secs: default_task_poll_secs(),
            heartbeat_interval_mins: default_heartbeat_mins(),
            tools: ToolsConfig::default(),
            providers: HashMap::new(),
            gateway: GatewayConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub base_url: String,
    pub model: String,
    pub api_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolsConfig {
    #[serde(default = "default_true")]
    pub bash: bool,
    #[serde(default)]
    pub web_search: bool,
    #[serde(default)]
    pub web_browse: bool,
    #[serde(default)]
    pub vision: bool,
    #[serde(default = "default_true")]
    pub gateway_messaging: bool,
    #[serde(default = "default_true")]
    pub file_operations: bool,
    #[serde(default = "default_true")]
    pub system_info: bool,
}

fn default_true() -> bool {
    true
}

impl Default for ToolsConfig {
    fn default() -> Self {
        Self {
            bash: true,
            web_search: false,
            web_browse: false,
            vision: false,
            gateway_messaging: true,
            file_operations: true,
            system_info: true,
        }
    }
}

// ---------------------------------------------------------------------------
// Agent events (broadcast to frontend subscribers)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentEvent {
    Delta {
        thread_id: String,
        content: String,
    },
    Reasoning {
        thread_id: String,
        content: String,
    },
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
    },
    Error {
        thread_id: String,
        message: String,
    },
    ThreadCreated {
        thread_id: String,
        title: String,
    },
    TaskUpdate {
        task_id: String,
        status: TaskStatus,
        progress: u8,
        message: Option<String>,
    },
    HeartbeatResult {
        item_id: String,
        result: HeartbeatOutcome,
        message: String,
    },
    Notification {
        title: String,
        body: String,
        severity: NotificationSeverity,
        channels: Vec<String>,
    },
    /// Request to send a message via a gateway platform (Slack/Discord/Telegram/WhatsApp).
    GatewaySend {
        platform: String,
        target: String,
        message: String,
    },
    /// Execute a workspace UI command on the frontend.
    WorkspaceCommand {
        command: String,
        args: serde_json::Value,
    },
    /// Incoming message from a gateway platform (for frontend display).
    GatewayIncoming {
        platform: String,
        sender: String,
        content: String,
        channel: String,
    },
}

// ---------------------------------------------------------------------------
// Threads & messages
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentThread {
    pub id: String,
    pub title: String,
    pub messages: Vec<AgentMessage>,
    pub created_at: u64,
    pub updated_at: u64,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMessage {
    pub role: MessageRole,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<String>,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}

// ---------------------------------------------------------------------------
// Tool calls
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub function: ToolFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolFunction {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub tool_call_id: String,
    pub name: String,
    pub content: String,
    pub is_error: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: ToolFunctionDef,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolFunctionDef {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

// ---------------------------------------------------------------------------
// Task queue
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum TaskPriority {
    Low,
    Normal,
    High,
    Urgent,
}

impl Default for TaskPriority {
    fn default() -> Self {
        Self::Normal
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTask {
    pub id: String,
    pub title: String,
    pub description: String,
    pub status: TaskStatus,
    #[serde(default)]
    pub priority: TaskPriority,
    #[serde(default)]
    pub progress: u8,
    pub created_at: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    #[serde(default = "default_source")]
    pub source: String,
    #[serde(default)]
    pub notify_on_complete: bool,
    #[serde(default)]
    pub notify_channels: Vec<String>,
}

fn default_source() -> String {
    "user".into()
}

// ---------------------------------------------------------------------------
// Heartbeat
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum HeartbeatOutcome {
    Ok,
    Alert,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatItem {
    pub id: String,
    pub label: String,
    pub prompt: String,
    #[serde(default = "default_zero")]
    pub interval_minutes: u64,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_run_at: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_result: Option<HeartbeatOutcome>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_message: Option<String>,
    #[serde(default)]
    pub notify_on_alert: bool,
    #[serde(default)]
    pub notify_channels: Vec<String>,
}

fn default_zero() -> u64 {
    0
}

// ---------------------------------------------------------------------------
// Notifications
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum NotificationSeverity {
    Info,
    Warning,
    Alert,
    Error,
}

// ---------------------------------------------------------------------------
// Persistent memory (SOUL.md, MEMORY.md, USER.md)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentMemory {
    pub soul: String,
    pub memory: String,
    pub user_profile: String,
}

// ---------------------------------------------------------------------------
// LLM completion types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum CompletionChunk {
    Delta {
        content: String,
        reasoning: Option<String>,
    },
    ToolCalls {
        tool_calls: Vec<ToolCall>,
        content: Option<String>,
        reasoning: Option<String>,
        input_tokens: Option<u64>,
        output_tokens: Option<u64>,
    },
    Done {
        content: String,
        reasoning: Option<String>,
        input_tokens: u64,
        output_tokens: u64,
    },
    Error {
        message: String,
    },
}
