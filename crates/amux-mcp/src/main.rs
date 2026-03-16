//! tamux-mcp: An MCP (Model Context Protocol) server that exposes the tamux
//! daemon's capabilities as tools over JSON-RPC stdio transport.
//!
//! Register this binary as an MCP server in Claude Code or other MCP clients:
//!
//! ```json
//! {
//!   "mcpServers": {
//!     "tamux": {
//!       "command": "tamux-mcp"
//!     }
//!   }
//! }
//! ```

use std::io::Write;

use amux_protocol::{
    AmuxCodec, ClientMessage, DaemonMessage, ManagedCommandRequest, ManagedCommandSource,
    SecurityLevel,
};
use anyhow::{Context, Result};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::time::{timeout, Duration};
use tokio_util::codec::Framed;
use tracing::{debug, error, info, warn};

// ---------------------------------------------------------------------------
// JSON-RPC types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Option<Value>,
}

#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i64,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
}

impl JsonRpcResponse {
    fn success(id: Option<Value>, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: Some(result),
            error: None,
        }
    }

    fn error(id: Option<Value>, code: i64, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.into(),
                data: None,
            }),
        }
    }
}

// JSON-RPC error codes
const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;
#[allow(dead_code)]
const INTERNAL_ERROR: i64 = -32603;

// ---------------------------------------------------------------------------
// MCP tool definitions
// ---------------------------------------------------------------------------

fn tool_definitions() -> Value {
    serde_json::json!([
        {
            "name": "execute_command",
            "description": "Execute a managed command inside a tamux terminal session. The command runs in a sandboxed lane with approval gating, automatic snapshots, and telemetry.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "session_id": {
                        "type": "string",
                        "description": "UUID of the target terminal session"
                    },
                    "command": {
                        "type": "string",
                        "description": "The shell command to execute"
                    },
                    "rationale": {
                        "type": "string",
                        "description": "Human-readable explanation of why this command is being run"
                    }
                },
                "required": ["session_id", "command", "rationale"]
            }
        },
        {
            "name": "search_history",
            "description": "Search the daemon's command and transcript history using full-text search.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Full-text search query"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of results to return"
                    }
                },
                "required": ["query"]
            }
        },
        {
            "name": "find_symbol",
            "description": "Search for code symbols (functions, types, variables) using the daemon's semantic indexing.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "workspace_root": {
                        "type": "string",
                        "description": "Absolute path to the workspace root directory"
                    },
                    "symbol": {
                        "type": "string",
                        "description": "Symbol name or pattern to search for"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of results to return"
                    }
                },
                "required": ["workspace_root", "symbol"]
            }
        },
        {
            "name": "list_snapshots",
            "description": "List recorded workspace snapshots and checkpoints. Snapshots are created automatically before managed commands.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "workspace_id": {
                        "type": "string",
                        "description": "Optional workspace ID to filter snapshots"
                    }
                },
                "required": []
            }
        },
        {
            "name": "restore_snapshot",
            "description": "Restore a previously recorded workspace snapshot, reverting the workspace to that point in time.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "snapshot_id": {
                        "type": "string",
                        "description": "ID of the snapshot to restore"
                    }
                },
                "required": ["snapshot_id"]
            }
        },
        {
            "name": "scrub_sensitive",
            "description": "Scrub sensitive data (secrets, tokens, passwords) from a text string using the daemon's detection engine.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "text": {
                        "type": "string",
                        "description": "Text to scrub for sensitive data"
                    }
                },
                "required": ["text"]
            }
        },
        {
            "name": "list_sessions",
            "description": "List all active terminal sessions managed by the tamux daemon.",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "required": []
            }
        },
        {
            "name": "verify_integrity",
            "description": "Verify the integrity of WORM (Write-Once-Read-Many) telemetry ledgers to detect tampering.",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "required": []
            }
        },
        {
            "name": "get_terminal_content",
            "description": "Read the scrollback buffer (visible content) of a terminal session. Use this to see what is currently displayed in a user's terminal pane.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "session_id": {
                        "type": "string",
                        "description": "UUID of the terminal session to read"
                    },
                    "max_lines": {
                        "type": "integer",
                        "description": "Maximum number of lines to return from the tail (default: 100)"
                    }
                },
                "required": ["session_id"]
            }
        },
        {
            "name": "type_in_terminal",
            "description": "Send keystrokes/input to a terminal session. Use this to type commands or interact with running programs in the user's terminal.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "session_id": {
                        "type": "string",
                        "description": "UUID of the terminal session"
                    },
                    "input": {
                        "type": "string",
                        "description": "Text to type into the terminal. Use \\n for Enter key."
                    }
                },
                "required": ["session_id", "input"]
            }
        },
        {
            "name": "get_git_status",
            "description": "Get git status for a working directory, showing modified, staged, and untracked files.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Absolute path to the git repository"
                    }
                },
                "required": ["path"]
            }
        }
    ])
}

// ---------------------------------------------------------------------------
// Daemon IPC connection
// ---------------------------------------------------------------------------

/// Connect to the tamux daemon and return a framed codec stream.
async fn connect_daemon(
) -> Result<Framed<impl tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin, AmuxCodec>> {
    #[cfg(unix)]
    {
        let runtime_dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_string());
        let path = std::path::PathBuf::from(runtime_dir).join("tamux-daemon.sock");
        let stream = tokio::net::UnixStream::connect(&path)
            .await
            .with_context(|| format!("cannot connect to tamux daemon at {}", path.display()))?;
        Ok(Framed::new(stream, AmuxCodec))
    }

    #[cfg(windows)]
    {
        let addr = amux_protocol::default_tcp_addr();
        let stream = tokio::net::TcpStream::connect(&addr)
            .await
            .with_context(|| format!("cannot connect to tamux daemon on {addr}"))?;
        Ok(Framed::new(stream, AmuxCodec))
    }
}

/// Send a single message to the daemon and read back one response.
async fn daemon_roundtrip(msg: ClientMessage) -> Result<DaemonMessage> {
    let mut framed = connect_daemon().await?;
    framed.send(msg).await.context("failed to send to daemon")?;
    let resp = framed
        .next()
        .await
        .ok_or_else(|| anyhow::anyhow!("daemon closed connection before responding"))??;
    Ok(resp)
}

// ---------------------------------------------------------------------------
// Tool dispatch
// ---------------------------------------------------------------------------

/// Execute an MCP tool call against the daemon and return the result as JSON.
async fn handle_tool_call(name: &str, args: &Value) -> Value {
    match dispatch_tool(name, args).await {
        Ok(result) => serde_json::json!({
            "content": [{
                "type": "text",
                "text": serde_json::to_string_pretty(&result).unwrap_or_else(|_| result.to_string())
            }]
        }),
        Err(err) => serde_json::json!({
            "content": [{
                "type": "text",
                "text": format!("Error: {err:#}")
            }],
            "isError": true
        }),
    }
}

async fn dispatch_tool(name: &str, args: &Value) -> Result<Value> {
    match name {
        "execute_command" => tool_execute_command(args).await,
        "search_history" => tool_search_history(args).await,
        "find_symbol" => tool_find_symbol(args).await,
        "list_snapshots" => tool_list_snapshots(args).await,
        "restore_snapshot" => tool_restore_snapshot(args).await,
        "scrub_sensitive" => tool_scrub_sensitive(args).await,
        "list_sessions" => tool_list_sessions(args).await,
        "verify_integrity" => tool_verify_integrity(args).await,
        "get_terminal_content" => tool_get_terminal_content(args).await,
        "type_in_terminal" => tool_type_in_terminal(args).await,
        "get_git_status" => tool_get_git_status(args).await,
        _ => anyhow::bail!("unknown tool: {name}"),
    }
}

// ---------------------------------------------------------------------------
// Tool implementations
// ---------------------------------------------------------------------------

async fn tool_execute_command(args: &Value) -> Result<Value> {
    let session_id: uuid::Uuid = args
        .get("session_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing required parameter: session_id"))?
        .parse()
        .context("invalid session_id UUID")?;

    let command = args
        .get("command")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing required parameter: command"))?
        .to_string();

    let rationale = args
        .get("rationale")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing required parameter: rationale"))?
        .to_string();

    let msg = ClientMessage::ExecuteManagedCommand {
        id: session_id,
        request: ManagedCommandRequest {
            command: command.clone(),
            rationale,
            allow_network: false,
            sandbox_enabled: true,
            security_level: SecurityLevel::Moderate,
            cwd: None,
            language_hint: None,
            source: ManagedCommandSource::Agent,
        },
    };

    // For managed commands we may receive multiple messages (queued, started,
    // finished). We need to consume the stream until we get a terminal state.
    let mut framed = connect_daemon().await?;
    framed.send(msg).await.context("failed to send to daemon")?;

    let mut events: Vec<Value> = Vec::new();

    while let Some(resp) = framed.next().await {
        let resp = resp.context("error reading from daemon")?;
        match resp {
            DaemonMessage::ManagedCommandQueued {
                execution_id,
                position,
                snapshot,
                ..
            } => {
                events.push(serde_json::json!({
                    "event": "queued",
                    "execution_id": execution_id,
                    "position": position,
                    "snapshot": snapshot,
                }));
            }
            DaemonMessage::ManagedCommandStarted {
                execution_id,
                command: cmd,
                ..
            } => {
                events.push(serde_json::json!({
                    "event": "started",
                    "execution_id": execution_id,
                    "command": cmd,
                }));
            }
            DaemonMessage::ManagedCommandFinished {
                execution_id,
                command: cmd,
                exit_code,
                duration_ms,
                snapshot,
                ..
            } => {
                events.push(serde_json::json!({
                    "event": "finished",
                    "execution_id": execution_id,
                    "command": cmd,
                    "exit_code": exit_code,
                    "duration_ms": duration_ms,
                    "snapshot": snapshot,
                }));
                // Terminal state reached.
                break;
            }
            DaemonMessage::ManagedCommandRejected {
                execution_id,
                message,
                ..
            } => {
                events.push(serde_json::json!({
                    "event": "rejected",
                    "execution_id": execution_id,
                    "message": message,
                }));
                break;
            }
            DaemonMessage::ApprovalRequired { approval, .. } => {
                events.push(serde_json::json!({
                    "event": "approval_required",
                    "approval_id": approval.approval_id,
                    "risk_level": approval.risk_level,
                    "blast_radius": approval.blast_radius,
                    "reasons": approval.reasons,
                }));
                // Approval blocks execution; report back to caller.
                break;
            }
            DaemonMessage::Error { message } => {
                anyhow::bail!("daemon error: {message}");
            }
            _ => {
                // Ignore other messages (output bytes, etc.)
            }
        }
    }

    Ok(serde_json::json!({
        "command": command,
        "session_id": session_id.to_string(),
        "events": events,
    }))
}

async fn tool_search_history(args: &Value) -> Result<Value> {
    let query = args
        .get("query")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing required parameter: query"))?
        .to_string();

    let limit = args
        .get("limit")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize);

    let resp = daemon_roundtrip(ClientMessage::SearchHistory { query, limit }).await?;

    match resp {
        DaemonMessage::HistorySearchResult {
            query,
            summary,
            hits,
        } => Ok(serde_json::json!({
            "query": query,
            "summary": summary,
            "hits": hits,
        })),
        DaemonMessage::Error { message } => anyhow::bail!("daemon error: {message}"),
        other => anyhow::bail!("unexpected daemon response: {other:?}"),
    }
}

async fn tool_find_symbol(args: &Value) -> Result<Value> {
    let workspace_root = args
        .get("workspace_root")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing required parameter: workspace_root"))?
        .to_string();

    let symbol = args
        .get("symbol")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing required parameter: symbol"))?
        .to_string();

    let limit = args
        .get("limit")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize);

    let resp = daemon_roundtrip(ClientMessage::FindSymbol {
        workspace_root,
        symbol,
        limit,
    })
    .await?;

    match resp {
        DaemonMessage::SymbolSearchResult { symbol, matches } => Ok(serde_json::json!({
            "symbol": symbol,
            "matches": matches,
        })),
        DaemonMessage::Error { message } => anyhow::bail!("daemon error: {message}"),
        other => anyhow::bail!("unexpected daemon response: {other:?}"),
    }
}

async fn tool_list_snapshots(args: &Value) -> Result<Value> {
    let workspace_id = args
        .get("workspace_id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let resp = daemon_roundtrip(ClientMessage::ListSnapshots { workspace_id }).await?;

    match resp {
        DaemonMessage::SnapshotList { snapshots } => Ok(serde_json::json!({
            "snapshots": snapshots,
        })),
        DaemonMessage::Error { message } => anyhow::bail!("daemon error: {message}"),
        other => anyhow::bail!("unexpected daemon response: {other:?}"),
    }
}

async fn tool_restore_snapshot(args: &Value) -> Result<Value> {
    let snapshot_id = args
        .get("snapshot_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing required parameter: snapshot_id"))?
        .to_string();

    let resp = daemon_roundtrip(ClientMessage::RestoreSnapshot { snapshot_id }).await?;

    match resp {
        DaemonMessage::SnapshotRestored {
            snapshot_id,
            ok,
            message,
        } => Ok(serde_json::json!({
            "snapshot_id": snapshot_id,
            "ok": ok,
            "message": message,
        })),
        DaemonMessage::Error { message } => anyhow::bail!("daemon error: {message}"),
        other => anyhow::bail!("unexpected daemon response: {other:?}"),
    }
}

async fn tool_scrub_sensitive(args: &Value) -> Result<Value> {
    let text = args
        .get("text")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing required parameter: text"))?
        .to_string();

    let resp = daemon_roundtrip(ClientMessage::ScrubSensitive { text }).await?;

    match resp {
        DaemonMessage::ScrubResult { text } => Ok(serde_json::json!({
            "scrubbed_text": text,
        })),
        DaemonMessage::Error { message } => anyhow::bail!("daemon error: {message}"),
        other => anyhow::bail!("unexpected daemon response: {other:?}"),
    }
}

async fn tool_list_sessions(_args: &Value) -> Result<Value> {
    let resp = daemon_roundtrip(ClientMessage::ListSessions).await?;

    match resp {
        DaemonMessage::SessionList { sessions } => Ok(serde_json::json!({
            "sessions": sessions,
        })),
        DaemonMessage::Error { message } => anyhow::bail!("daemon error: {message}"),
        other => anyhow::bail!("unexpected daemon response: {other:?}"),
    }
}

async fn tool_verify_integrity(_args: &Value) -> Result<Value> {
    let resp = daemon_roundtrip(ClientMessage::VerifyTelemetryIntegrity).await?;

    match resp {
        DaemonMessage::TelemetryIntegrityResult { results } => Ok(serde_json::json!({
            "results": results,
        })),
        DaemonMessage::Error { message } => anyhow::bail!("daemon error: {message}"),
        other => anyhow::bail!("unexpected daemon response: {other:?}"),
    }
}

async fn tool_get_terminal_content(args: &Value) -> Result<Value> {
    let session_id: uuid::Uuid = args
        .get("session_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing required parameter: session_id"))?
        .parse()
        .context("invalid session_id UUID")?;

    let max_lines = args
        .get("max_lines")
        .and_then(|v| v.as_u64())
        .map(|v| v as usize);

    let resp = daemon_roundtrip(ClientMessage::GetScrollback {
        id: session_id,
        max_lines: Some(max_lines.unwrap_or(100)),
    })
    .await?;

    match resp {
        DaemonMessage::Scrollback { id, data } => {
            let text = String::from_utf8_lossy(&data);
            // Strip ANSI escape sequences for cleaner output
            let clean = strip_ansi_basic(&text);
            Ok(serde_json::json!({
                "session_id": id.to_string(),
                "content": clean,
                "lines": clean.lines().count(),
            }))
        }
        DaemonMessage::Error { message } => anyhow::bail!("daemon error: {message}"),
        other => anyhow::bail!("unexpected daemon response: {other:?}"),
    }
}

async fn tool_type_in_terminal(args: &Value) -> Result<Value> {
    let session_id: uuid::Uuid = args
        .get("session_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing required parameter: session_id"))?
        .parse()
        .context("invalid session_id UUID")?;

    let input = args
        .get("input")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing required parameter: input"))?;

    // Convert escape sequences
    let data = input
        .replace("\\n", "\n")
        .replace("\\r", "\r")
        .replace("\\t", "\t")
        .into_bytes();

    let mut framed = connect_daemon().await?;
    framed
        .send(ClientMessage::Input {
            id: session_id,
            data: data.clone(),
        })
        .await
        .context("failed to send input to daemon")?;

    // Input is fire-and-forget on success, but invalid/exited sessions yield
    // an immediate daemon Error. Wait briefly to propagate that outcome.
    match timeout(Duration::from_millis(250), framed.next()).await {
        Ok(Some(Ok(DaemonMessage::Error { message }))) => {
            anyhow::bail!("daemon error: {message}");
        }
        Ok(Some(Err(e))) => {
            return Err(e.into());
        }
        Ok(None) => {
            anyhow::bail!("daemon connection closed while sending input");
        }
        Ok(Some(Ok(_))) | Err(_) => {
            // No immediate error => treat as accepted.
        }
    }

    Ok(serde_json::json!({
        "session_id": session_id.to_string(),
        "bytes_sent": data.len(),
        "status": "ok",
    }))
}

async fn tool_get_git_status(args: &Value) -> Result<Value> {
    let path = args
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing required parameter: path"))?
        .to_string();

    let resp = daemon_roundtrip(ClientMessage::GetGitStatus { path }).await?;

    match resp {
        DaemonMessage::GitStatus {
            path: repo_path,
            info,
        } => Ok(serde_json::json!({
            "path": repo_path,
            "branch": info.branch,
            "is_dirty": info.is_dirty,
            "ahead": info.ahead,
            "behind": info.behind,
            "untracked": info.untracked,
            "modified": info.modified,
            "staged": info.staged,
        })),
        DaemonMessage::Error { message } => anyhow::bail!("daemon error: {message}"),
        other => anyhow::bail!("unexpected daemon response: {other:?}"),
    }
}

/// Basic ANSI escape sequence stripping for terminal scrollback output.
fn strip_ansi_basic(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\x1b' {
            if chars.peek() == Some(&'[') {
                chars.next();
                while let Some(&next) = chars.peek() {
                    chars.next();
                    if ('\x40'..='\x7e').contains(&next) {
                        break;
                    }
                }
            } else if chars.peek() == Some(&']') {
                chars.next();
                while let Some(&next) = chars.peek() {
                    chars.next();
                    if next == '\x07' {
                        break;
                    }
                    if next == '\x1b' && chars.peek() == Some(&'\\') {
                        chars.next();
                        break;
                    }
                }
            } else {
                chars.next();
            }
        } else {
            result.push(c);
        }
    }

    result
}

// ---------------------------------------------------------------------------
// MCP protocol method handlers
// ---------------------------------------------------------------------------

fn handle_initialize(id: Option<Value>) -> JsonRpcResponse {
    JsonRpcResponse::success(
        id,
        serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {}
            },
            "serverInfo": {
                "name": "tamux-mcp",
                "version": env!("CARGO_PKG_VERSION")
            }
        }),
    )
}

fn handle_tools_list(id: Option<Value>) -> JsonRpcResponse {
    JsonRpcResponse::success(
        id,
        serde_json::json!({
            "tools": tool_definitions()
        }),
    )
}

async fn handle_tools_call(id: Option<Value>, params: Option<Value>) -> JsonRpcResponse {
    let params = match params {
        Some(p) => p,
        None => {
            return JsonRpcResponse::error(id, INVALID_PARAMS, "missing params for tools/call");
        }
    };

    let tool_name = match params.get("name").and_then(|v| v.as_str()) {
        Some(n) => n.to_string(),
        None => {
            return JsonRpcResponse::error(id, INVALID_PARAMS, "missing tool name in params");
        }
    };

    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or(serde_json::json!({}));

    let result = handle_tool_call(&tool_name, &arguments).await;
    JsonRpcResponse::success(id, result)
}

// ---------------------------------------------------------------------------
// Stdio transport — content-length framing
// ---------------------------------------------------------------------------

/// Read a single JSON-RPC message from stdin using content-length framing.
///
/// The MCP specification uses HTTP-style headers:
/// ```text
/// Content-Length: <n>\r\n
/// \r\n
/// <n bytes of JSON>
/// ```
/// Read a JSON-RPC message from stdin.
///
/// Auto-detects framing format:
/// - If a line starts with `{`, it's newline-delimited JSON (Python `mcp` SDK style)
/// - If a line starts with `Content-Length:`, it's LSP-style content-length framing
async fn read_message(reader: &mut BufReader<tokio::io::Stdin>) -> Result<Option<String>> {
    loop {
        let mut line = String::new();
        let n = reader
            .read_line(&mut line)
            .await
            .context("failed to read from stdin")?;

        if n == 0 {
            return Ok(None); // EOF
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue; // Skip blank lines
        }

        // Newline-delimited JSON: line starts with '{' — it's a complete message
        if trimmed.starts_with('{') {
            return Ok(Some(trimmed.to_string()));
        }

        // Content-Length framing: read headers, then body
        if let Some(value) = trimmed.strip_prefix("Content-Length:") {
            let length: usize = value
                .trim()
                .parse()
                .context("invalid Content-Length value")?;

            // Read remaining headers until blank line
            loop {
                let mut header_line = String::new();
                let hn = reader
                    .read_line(&mut header_line)
                    .await
                    .context("failed to read header")?;
                if hn == 0 || header_line.trim().is_empty() {
                    break;
                }
            }

            let mut body = vec![0u8; length];
            tokio::io::AsyncReadExt::read_exact(reader, &mut body)
                .await
                .context("failed to read message body")?;

            let text = String::from_utf8(body).context("message body is not valid UTF-8")?;
            return Ok(Some(text));
        }

        // Skip unknown header lines
    }
}

/// Write a JSON-RPC response to stdout.
///
/// Supports two framing modes based on an environment variable:
/// - `TAMUX_MCP_FRAMING=content-length` — Content-Length header framing (LSP-style)
/// - Default — newline-delimited JSON (one JSON object per line, as used by the
///   Python `mcp` SDK's stdio transport)
fn write_message(msg: &JsonRpcResponse) -> Result<()> {
    let body = serde_json::to_string(msg).context("failed to serialize response")?;

    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    if std::env::var("TAMUX_MCP_FRAMING").as_deref() == Ok("content-length") {
        let header = format!("Content-Length: {}\r\n\r\n", body.len());
        out.write_all(header.as_bytes())?;
        out.write_all(body.as_bytes())?;
    } else {
        // Newline-delimited JSON — compatible with Python mcp SDK
        out.write_all(body.as_bytes())?;
        out.write_all(b"\n")?;
    }

    out.flush()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Main loop
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<()> {
    // Log to stderr so stdout stays clean for JSON-RPC.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .with_writer(std::io::stderr)
        .init();

    info!("tamux-mcp server starting");

    let mut reader = BufReader::new(tokio::io::stdin());

    loop {
        let message = match read_message(&mut reader).await {
            Ok(Some(msg)) => msg,
            Ok(None) => {
                debug!("stdin closed, shutting down");
                break;
            }
            Err(err) => {
                error!("failed to read message: {err:#}");
                let resp = JsonRpcResponse::error(None, PARSE_ERROR, format!("{err:#}"));
                if let Err(e) = write_message(&resp) {
                    error!("failed to write error response: {e:#}");
                }
                continue;
            }
        };

        debug!("received: {message}");

        let request: JsonRpcRequest = match serde_json::from_str(&message) {
            Ok(req) => req,
            Err(err) => {
                warn!("invalid JSON-RPC message: {err}");
                let resp =
                    JsonRpcResponse::error(None, PARSE_ERROR, format!("invalid JSON: {err}"));
                write_message(&resp)?;
                continue;
            }
        };

        if request.jsonrpc != "2.0" {
            let resp = JsonRpcResponse::error(
                request.id.clone(),
                INVALID_REQUEST,
                "only JSON-RPC 2.0 is supported",
            );
            write_message(&resp)?;
            continue;
        }

        let response = match request.method.as_str() {
            "initialize" => Some(handle_initialize(request.id)),
            "initialized" | "notifications/initialized" => {
                // Notification — no response needed.
                debug!("received initialized notification");
                None
            }
            "tools/list" => Some(handle_tools_list(request.id)),
            "tools/call" => Some(handle_tools_call(request.id, request.params).await),
            "notifications/cancelled" => {
                debug!("received cancellation notification");
                None
            }
            "ping" => Some(JsonRpcResponse::success(request.id, serde_json::json!({}))),
            method => {
                warn!("unknown method: {method}");
                // Notifications (no id) should not receive error responses.
                if request.id.is_some() {
                    Some(JsonRpcResponse::error(
                        request.id,
                        METHOD_NOT_FOUND,
                        format!("unknown method: {method}"),
                    ))
                } else {
                    None
                }
            }
        };

        if let Some(resp) = response {
            if let Err(e) = write_message(&resp) {
                error!("failed to write response: {e:#}");
            }
        }
    }

    info!("tamux-mcp server stopped");
    Ok(())
}
