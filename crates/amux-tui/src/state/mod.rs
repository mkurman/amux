// Sub-module declarations — uncomment as modules are implemented
pub mod chat;
pub mod input;
pub mod modal;
pub mod sidebar;
pub mod task;
pub mod config;
pub mod approval;
pub mod settings;

// ── Focus ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FocusArea {
    Chat,
    Sidebar,
    Input,
}

// ── Daemon commands ───────────────────────────────────────────────────────────

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum DaemonCommand {
    Refresh,
    RefreshServices,
    RequestThread(String),
    RequestGoalRunDetail(String),
    SendMessage {
        thread_id: Option<String>,
        content: String,
        session_id: Option<String>,
    },
    FetchModels {
        provider_id: String,
        base_url: String,
        api_key: String,
    },
    SetConfigJson(String),
    ControlGoalRun {
        goal_run_id: String,
        action: String,
    },
    ResolveTaskApproval {
        approval_id: String,
        decision: String,
    },
    SpawnSession {
        shell: Option<String>,
        cwd: Option<String>,
        cols: u16,
        rows: u16,
    },
}

// ── Placeholder sub-action enums ──────────────────────────────────────────────
// These will be filled in by later tasks.

#[allow(unused_imports)]
pub use chat::{ChatState, ChatAction, TranscriptMode, ToolCallVm, ToolCallStatus};
#[allow(unused_imports)]
pub use task::{TaskState, TaskAction};
#[allow(unused_imports)]
pub use sidebar::{SidebarState, SidebarAction, SidebarTab};
#[allow(unused_imports)]
pub use input::{InputState, InputAction, InputMode};
#[allow(unused_imports)]
pub use modal::{ModalState, ModalAction, ModalKind, CommandItem};
#[allow(unused_imports)]
pub use config::{ConfigState, ConfigAction};
#[allow(unused_imports)]
pub use approval::{ApprovalState, ApprovalAction, RiskLevel, PendingApproval};
#[allow(unused_imports)]
pub use settings::{SettingsState, SettingsAction, SettingsTab};

// ── Top-level app action ──────────────────────────────────────────────────────

#[allow(dead_code)]
#[derive(Debug)]
pub enum AppAction {
    Chat(ChatAction),
    Task(TaskAction),
    Sidebar(SidebarAction),
    Input(InputAction),
    Modal(ModalAction),
    Config(ConfigAction),
    Approval(ApprovalAction),
    Settings(SettingsAction),
    Status(String),
    Focus(FocusArea),
    Resize { width: u16, height: u16 },
    Tick,
    Connected,
    Disconnected,
    Quit,
}

// ── Re-exports ────────────────────────────────────────────────────────────────

#[allow(unused_imports)]
pub use AppAction as Action;
