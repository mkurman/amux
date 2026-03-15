use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Identifiers
// ---------------------------------------------------------------------------

/// Unique identifier for a terminal session.
pub type SessionId = Uuid;

/// Unique identifier for a workspace (passed as string).
pub type WorkspaceId = String;

// ---------------------------------------------------------------------------
// Client -> Daemon requests
// ---------------------------------------------------------------------------

/// Messages sent from a client to the daemon.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClientMessage {
    /// Spawn a new terminal session.
    SpawnSession {
        /// Optional shell override (e.g. "pwsh", "bash"). Daemon picks the
        /// default shell when `None`.
        shell: Option<String>,
        /// Initial working directory. Falls back to user home if `None`.
        cwd: Option<String>,
        /// Optional environment variable overrides.
        env: Option<Vec<(String, String)>>,
        /// Workspace ID for grouping.
        workspace_id: Option<WorkspaceId>,
        /// Initial terminal width in columns.
        cols: u16,
        /// Initial terminal height in rows.
        rows: u16,
    },

    /// Attach to an existing session (start receiving output).
    AttachSession { id: SessionId },

    /// Clone an existing session into a new independent PTY session.
    CloneSession {
        /// Source session to clone metadata/scrollback from.
        source_id: SessionId,
        /// Optional workspace override for the cloned session.
        workspace_id: Option<WorkspaceId>,
        /// Optional terminal width override (defaults to source cols).
        cols: Option<u16>,
        /// Optional terminal height override (defaults to source rows).
        rows: Option<u16>,
        /// Whether to preload source scrollback into the clone.
        replay_scrollback: bool,
        /// Fallback CWD when the source session's CWD cannot be resolved
        /// (e.g. on Windows where /proc is unavailable).
        cwd: Option<String>,
    },

    /// Detach from a session (stop receiving output; session keeps running).
    DetachSession { id: SessionId },

    /// Kill / close a session.
    KillSession { id: SessionId },

    /// Send raw terminal input bytes to a session.
    Input { id: SessionId, data: Vec<u8> },

    /// Execute a daemon-managed command inside a session lane.
    ExecuteManagedCommand {
        id: SessionId,
        request: ManagedCommandRequest,
    },

    /// Resolve a previously issued approval request.
    ResolveApproval {
        id: SessionId,
        approval_id: String,
        decision: ApprovalDecision,
    },

    /// Notify the daemon about a terminal resize.
    Resize { id: SessionId, cols: u16, rows: u16 },

    /// List all active sessions.
    ListSessions,

    /// List sessions belonging to a specific workspace.
    ListWorkspaceSessions { workspace_id: WorkspaceId },

    /// Request the scrollback buffer for a session.
    GetScrollback {
        id: SessionId,
        /// Maximum number of lines to return (from the tail).
        max_lines: Option<usize>,
    },

    /// Ask the daemon to analyze a session's recent output (AI sidecar).
    AnalyzeSession {
        id: SessionId,
        max_lines: Option<usize>,
    },

    /// Search daemon-managed command/transcript history.
    SearchHistory { query: String, limit: Option<usize> },

    /// Generate a reusable skill document from historical executions.
    GenerateSkill {
        query: Option<String>,
        title: Option<String>,
    },

    /// Search for symbols using daemon-side semantic indexing.
    FindSymbol {
        workspace_root: String,
        symbol: String,
        limit: Option<usize>,
    },

    /// List recorded workspace snapshots/checkpoints.
    ListSnapshots { workspace_id: Option<WorkspaceId> },

    /// Restore a previously recorded snapshot.
    RestoreSnapshot { snapshot_id: String },

    /// Request git status for a working directory.
    GetGitStatus { path: String },

    /// Subscribe to OSC notifications for sessions (the daemon will push
    /// `OscNotification` messages for any attached session).
    SubscribeNotifications,

    /// Request the daemon to scrub sensitive data from a string.
    ScrubSensitive { text: String },

    /// Verify WORM telemetry ledger integrity.
    VerifyTelemetryIntegrity,

    /// Checkpoint a session's process state using CRIU.
    CheckpointSession { id: SessionId },

    /// Ping / health-check.
    Ping,
}

// ---------------------------------------------------------------------------
// Daemon -> Client responses
// ---------------------------------------------------------------------------

/// Messages sent from the daemon back to a client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DaemonMessage {
    /// Confirmation that a session was spawned.
    SessionSpawned { id: SessionId },

    /// Confirmation that a session was cloned.
    SessionCloned {
        source_id: SessionId,
        id: SessionId,
        /// The command that was actively running in the source session
        /// (detected via shell integration), if any.
        active_command: Option<String>,
    },

    /// Confirmation that the client is now attached.
    SessionAttached { id: SessionId },

    /// Confirmation of detach.
    SessionDetached { id: SessionId },

    /// Session was killed.
    SessionKilled { id: SessionId },

    /// Session exited on its own (process exited).
    SessionExited {
        id: SessionId,
        exit_code: Option<i32>,
    },

    /// Terminal output bytes from a session.
    Output { id: SessionId, data: Vec<u8> },

    /// Shell lifecycle marker emitted by the daemon.
    CommandStarted { id: SessionId, command: String },

    /// Shell lifecycle marker emitted by the daemon.
    CommandFinished {
        id: SessionId,
        exit_code: Option<i32>,
    },

    /// Managed command has been queued for serial execution.
    ManagedCommandQueued {
        id: SessionId,
        execution_id: String,
        position: usize,
        snapshot: Option<SnapshotInfo>,
    },

    /// Managed command is blocked pending approval.
    ApprovalRequired {
        id: SessionId,
        approval: ApprovalPayload,
    },

    /// Approval decision has been recorded.
    ApprovalResolved {
        id: SessionId,
        approval_id: String,
        decision: ApprovalDecision,
    },

    /// Managed command has started executing.
    ManagedCommandStarted {
        id: SessionId,
        execution_id: String,
        command: String,
        source: ManagedCommandSource,
    },

    /// Managed command completed.
    ManagedCommandFinished {
        id: SessionId,
        execution_id: String,
        command: String,
        exit_code: Option<i32>,
        duration_ms: Option<u64>,
        snapshot: Option<SnapshotInfo>,
    },

    /// Managed command was rejected by validation or policy.
    ManagedCommandRejected {
        id: SessionId,
        execution_id: Option<String>,
        message: String,
    },

    /// Reply to `ListSessions`.
    SessionList { sessions: Vec<SessionInfo> },

    /// Scrollback data reply.
    Scrollback { id: SessionId, data: Vec<u8> },

    /// AI analysis result.
    AnalysisResult { id: SessionId, result: String },

    /// Search results from SQLite/FTS-backed history.
    HistorySearchResult {
        query: String,
        summary: String,
        hits: Vec<HistorySearchHit>,
    },

    /// Generated procedural skill document.
    SkillGenerated { title: String, path: String },

    /// Semantic symbol search results.
    SymbolSearchResult {
        symbol: String,
        matches: Vec<SymbolMatch>,
    },

    /// Recorded snapshots/checkpoints.
    SnapshotList { snapshots: Vec<SnapshotInfo> },

    /// Snapshot restore result.
    SnapshotRestored {
        snapshot_id: String,
        ok: bool,
        message: String,
    },

    /// OSC notification parsed from terminal output.
    OscNotification {
        id: SessionId,
        notification: OscNotificationPayload,
    },

    /// Git status reply.
    GitStatus { path: String, info: GitInfo },

    /// Scrubbed text reply.
    ScrubResult { text: String },

    /// CWD change detected from a session.
    CwdChanged { id: SessionId, cwd: String },

    /// Telemetry integrity verification result.
    TelemetryIntegrityResult { results: Vec<TelemetryLedgerStatus> },

    /// Result of a CRIU checkpoint operation.
    SessionCheckpointed {
        id: SessionId,
        ok: bool,
        path: Option<String>,
        message: String,
    },

    /// Pong.
    Pong,

    /// Generic error.
    Error { message: String },
}

// ---------------------------------------------------------------------------
// Supporting types
// ---------------------------------------------------------------------------

/// Metadata about a running session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: SessionId,
    pub title: Option<String>,
    pub cwd: Option<String>,
    pub cols: u16,
    pub rows: u16,
    pub created_at: u64, // Unix timestamp
    pub workspace_id: Option<WorkspaceId>,
    pub exit_code: Option<i32>,
    pub is_alive: bool,
}

/// OSC notification payload (parsed from OSC 9, 99, 777).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OscNotificationPayload {
    pub source: OscSource,
    pub title: String,
    pub body: String,
    pub subtitle: Option<String>,
    pub icon: Option<String>,
    pub progress: Option<u8>, // 0..100
}

/// OSC notification source type.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum OscSource {
    /// OSC 9 — simple notification (iTerm2 Growl).
    Osc9,
    /// OSC 99 — structured notification (kitty).
    Osc99,
    /// OSC 777 — notification (rxvt-unicode).
    Osc777,
}

/// Git repository information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitInfo {
    pub branch: Option<String>,
    pub is_dirty: bool,
    pub ahead: u32,
    pub behind: u32,
    pub untracked: u32,
    pub modified: u32,
    pub staged: u32,
}

/// Source of a managed command request.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ManagedCommandSource {
    Human,
    Agent,
    Replay,
    Gateway,
}

/// Approval decision for daemon-managed commands.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ApprovalDecision {
    ApproveOnce,
    ApproveSession,
    Deny,
}

/// Security policy level controlling approval strictness.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum SecurityLevel {
    Highest,
    Moderate,
    Lowest,
    Yolo,
}

impl Default for SecurityLevel {
    fn default() -> Self {
        Self::Moderate
    }
}

/// Request describing a managed command execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagedCommandRequest {
    pub command: String,
    pub rationale: String,
    pub allow_network: bool,
    #[serde(default)]
    pub sandbox_enabled: bool,
    #[serde(default)]
    pub security_level: SecurityLevel,
    pub cwd: Option<String>,
    pub language_hint: Option<String>,
    pub source: ManagedCommandSource,
}

/// Structured approval payload rendered by the UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalPayload {
    pub approval_id: String,
    pub execution_id: String,
    pub command: String,
    pub rationale: String,
    pub risk_level: String,
    pub blast_radius: String,
    pub reasons: Vec<String>,
    pub workspace_id: Option<WorkspaceId>,
    pub allow_network: bool,
}

/// Recorded snapshot/checkpoint metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotInfo {
    pub snapshot_id: String,
    pub workspace_id: Option<WorkspaceId>,
    pub session_id: Option<SessionId>,
    pub command: Option<String>,
    pub kind: String,
    pub label: String,
    pub path: String,
    pub created_at: u64,
    pub status: String,
    pub details: String,
}

/// Search hit from historical command/transcript recall.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistorySearchHit {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub excerpt: String,
    pub path: Option<String>,
    pub timestamp: u64,
    pub score: f64,
}

/// Symbol search result emitted by the daemon.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolMatch {
    pub path: String,
    pub line: usize,
    pub kind: String,
    pub snippet: String,
}

/// Status of a single WORM telemetry ledger.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryLedgerStatus {
    pub kind: String,
    pub total_entries: usize,
    pub valid: bool,
    pub first_invalid_seq: Option<usize>,
    pub message: String,
}
