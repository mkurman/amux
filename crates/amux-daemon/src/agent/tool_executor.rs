//! Tool execution for the agent engine.
//!
//! Maps tool calls to daemon infrastructure. Tools that require frontend
//! state (workspace/pane/browser) are not available in daemon mode — only
//! tools that can execute headlessly are included here.

use std::sync::Arc;

use amux_protocol::SessionId;
use anyhow::Result;
use tokio::sync::broadcast;

use crate::session_manager::SessionManager;

use super::types::{
    AgentConfig, AgentEvent, NotificationSeverity, ToolCall, ToolDefinition, ToolFunctionDef,
    ToolResult,
};

const ONECONTEXT_TOOL_QUERY_MAX_CHARS: usize = 300;
const ONECONTEXT_TOOL_OUTPUT_MAX_CHARS: usize = 12_000;
const ONECONTEXT_TOOL_TIMEOUT_SECS: u64 = 8;

// ---------------------------------------------------------------------------
// Tool definitions (OpenAI function calling schema)
// ---------------------------------------------------------------------------

pub fn get_available_tools(config: &AgentConfig) -> Vec<ToolDefinition> {
    let mut tools = Vec::new();

    if config.tools.bash {
        tools.push(tool_def(
            "bash_command",
            "Execute a non-interactive shell command and return stdout, stderr, and exit code. Runs headless (no TTY). For interactive/TUI programs, use type_in_terminal instead.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "command": { "type": "string", "description": "The shell command to execute" },
                    "timeout_seconds": { "type": "integer", "description": "Max execution time in seconds (default: 30)" },
                    "cwd": { "type": "string", "description": "Working directory (optional)" }
                },
                "required": ["command"]
            }),
        ));
    }

    if config.tools.file_operations {
        tools.push(tool_def(
            "list_files",
            "List files and directories at a given path.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Directory path to list" }
                },
                "required": ["path"]
            }),
        ));

        tools.push(tool_def(
            "read_file",
            "Read the contents of a file.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File path to read" },
                    "max_lines": { "type": "integer", "description": "Max lines to read (default: 200)" }
                },
                "required": ["path"]
            }),
        ));

        tools.push(tool_def(
            "write_file",
            "Write content to a file, creating it if it doesn't exist.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File path to write" },
                    "content": { "type": "string", "description": "File content to write" }
                },
                "required": ["path", "content"]
            }),
        ));

        tools.push(tool_def(
            "search_files",
            "Search for a pattern in files using grep. Returns matching lines with file paths and line numbers.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "pattern": { "type": "string", "description": "Regex pattern to search for" },
                    "path": { "type": "string", "description": "Directory to search in (default: current directory)" },
                    "file_pattern": { "type": "string", "description": "Glob pattern to filter files (e.g. '*.rs', '*.ts')" },
                    "max_results": { "type": "integer", "description": "Max results to return (default: 50)" }
                },
                "required": ["pattern"]
            }),
        ));
    }

    if config.tools.system_info {
        tools.push(tool_def(
            "get_system_info",
            "Get system information: CPU, memory, disk, load average, hostname.",
            serde_json::json!({ "type": "object", "properties": {} }),
        ));

        tools.push(tool_def(
            "list_processes",
            "List running processes sorted by CPU usage.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "limit": { "type": "integer", "description": "Max processes to show (default: 20)" }
                }
            }),
        ));
    }

    // History search (daemon has SQLite FTS)
    tools.push(tool_def(
        "search_history",
        "Search command execution history. Returns matching commands with timestamps and exit codes.",
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Search query" },
                "limit": { "type": "integer", "description": "Max results (default: 20)" }
            },
            "required": ["query"]
        }),
    ));
    tools.push(tool_def(
        "onecontext_search",
        "Search Aline OneContext history for related prior sessions/events/turns.",
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Search query" },
                "scope": { "type": "string", "enum": ["session", "event", "turn"], "description": "Search scope (default: session)" },
                "no_regex": { "type": "boolean", "description": "Treat query as plain text (default: true)" }
            },
            "required": ["query"]
        }),
    ));

    // Daemon session management
    tools.push(tool_def(
        "list_sessions",
        "List all active terminal sessions managed by the daemon.",
        serde_json::json!({ "type": "object", "properties": {} }),
    ));

    // Always available: notify and memory tools
    tools.push(tool_def(
        "notify_user",
        "Send a proactive notification to the user via configured channels.",
        serde_json::json!({
            "type": "object",
            "properties": {
                "title": { "type": "string", "description": "Notification title" },
                "message": { "type": "string", "description": "Notification body" },
                "severity": { "type": "string", "enum": ["info", "warning", "alert", "error"] }
            },
            "required": ["title", "message"]
        }),
    ));

    tools.push(tool_def(
        "update_memory",
        "Update your persistent memory with new knowledge. Persists across sessions and restarts.",
        serde_json::json!({
            "type": "object",
            "properties": {
                "content": { "type": "string", "description": "Updated memory content (markdown)" }
            },
            "required": ["content"]
        }),
    ));

    if config.tools.web_search {
        tools.push(tool_def(
            "web_search",
            "Search the web and return results. Requires a search API key in config.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Search query" },
                    "max_results": { "type": "integer", "description": "Max results (default: 5)" }
                },
                "required": ["query"]
            }),
        ));
    }

    if config.tools.web_browse {
        tools.push(tool_def(
            "fetch_url",
            "Fetch a URL and return its text content (HTML stripped to text).",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "URL to fetch" },
                    "max_length": { "type": "integer", "description": "Max characters to return (default: 10000)" }
                },
                "required": ["url"]
            }),
        ));
    }

    if config.tools.gateway_messaging {
        for (name, desc, params) in [
            ("send_slack_message", "Send a message to a Slack channel. If channel is omitted, sends to the default channel from settings (slackChannelFilter).", serde_json::json!({
                "type": "object",
                "properties": {
                    "channel": { "type": "string", "description": "Slack channel name or ID. Optional — uses default from config if omitted." },
                    "message": { "type": "string", "description": "Message text" }
                },
                "required": ["message"]
            })),
            ("send_discord_message", "Send a message to a Discord channel or user. If channel_id and user_id are both omitted, sends to the default channel (discordChannelFilter) or default user DM (discordAllowedUsers) from settings.", serde_json::json!({
                "type": "object",
                "properties": {
                    "channel_id": { "type": "string", "description": "Discord channel ID. Optional — uses default from config if omitted." },
                    "user_id": { "type": "string", "description": "Discord user ID for DM. Optional." },
                    "message": { "type": "string", "description": "Message text" }
                },
                "required": ["message"]
            })),
            ("send_telegram_message", "Send a message to a Telegram chat. If chat_id is omitted, sends to the default chat from settings (telegramAllowedChats).", serde_json::json!({
                "type": "object",
                "properties": {
                    "chat_id": { "type": "string", "description": "Telegram chat ID. Optional — uses default from config if omitted." },
                    "message": { "type": "string", "description": "Message text" }
                },
                "required": ["message"]
            })),
            ("send_whatsapp_message", "Send a message to a WhatsApp contact. If phone is omitted, sends to the default contact from settings (whatsappAllowedContacts).", serde_json::json!({
                "type": "object",
                "properties": {
                    "phone": { "type": "string", "description": "Phone in E.164 format or WhatsApp JID. Optional — uses default from config if omitted." },
                    "message": { "type": "string", "description": "Message text" }
                },
                "required": ["message"]
            })),
        ] {
            tools.push(tool_def(name, desc, params));
        }
    }

    // Terminal pane tools
    tools.push(tool_def("list_terminals", "List all open terminal panes with their IDs and names.", serde_json::json!({"type":"object","properties":{}})));
    tools.push(tool_def("read_active_terminal_content", "Read the current terminal buffer content from a pane.", serde_json::json!({
        "type": "object",
        "properties": { "pane": { "type": "string", "description": "Pane ID or name (optional, defaults to active)" } }
    })));
    tools.push(tool_def("run_terminal_command", "Execute a non-interactive shell command and return stdout/stderr. Runs in a headless subprocess — NO TTY. Use ONLY for commands that produce text output (ls, cat, grep, git, curl, etc). Do NOT use for interactive/TUI programs (vim, htop, codex, claude, etc) — use type_in_terminal instead.", serde_json::json!({
        "type": "object",
        "properties": {
            "command": { "type": "string", "description": "Non-interactive shell command to execute" },
            "cwd": { "type": "string", "description": "Working directory (optional)" },
            "timeout_seconds": { "type": "integer", "description": "Max execution time in seconds (default: 30)" }
        },
        "required": ["command"]
    })));
    tools.push(tool_def("type_in_terminal", "Type text into an existing terminal session as raw keyboard input. Use this for: interactive TUI programs (codex, vim, htop), REPLs (python, node), typing commands in running shells, or any program that needs a real TTY. Text and Enter are sent with a small delay between them so TUIs process correctly. You can also send special keys like ctrl+c, escape, tab, arrow keys, etc.", serde_json::json!({
        "type": "object",
        "properties": {
            "text": { "type": "string", "description": "Text to type into the terminal" },
            "press_enter": { "type": "boolean", "description": "Whether to press Enter after typing (default: true)" },
            "key": { "type": "string", "description": "Send a special key instead of text. Options: enter, ctrl+c, ctrl+d, ctrl+z, ctrl+l, ctrl+a, ctrl+e, ctrl+u, ctrl+k, escape, tab, up, down, left, right, backspace, delete, home, end, page_up, page_down. When 'key' is set, 'text' is ignored." },
            "pane": { "type": "string", "description": "Pane ID or name (optional, defaults to first active session)" }
        },
        "required": []
    })));

    // Workspace tools — executed via WorkspaceCommand event on the frontend
    tools.push(tool_def("list_workspaces", "List workspaces, surfaces, and panes (with names and IDs).", serde_json::json!({"type":"object","properties":{}})));
    tools.push(tool_def("create_workspace", "Create a new workspace and make it active.", serde_json::json!({
        "type": "object",
        "properties": { "name": { "type": "string", "description": "Optional workspace name" } }
    })));
    tools.push(tool_def("set_active_workspace", "Set the active workspace by ID or name.", serde_json::json!({
        "type": "object",
        "properties": { "workspace": { "type": "string", "description": "Workspace ID or name" } },
        "required": ["workspace"]
    })));
    tools.push(tool_def("create_surface", "Create a new surface (tab) in a workspace.", serde_json::json!({
        "type": "object",
        "properties": {
            "workspace": { "type": "string", "description": "Optional workspace ID or name" },
            "name": { "type": "string", "description": "Optional surface name" }
        }
    })));
    tools.push(tool_def("set_active_surface", "Set active surface by ID or name.", serde_json::json!({
        "type": "object",
        "properties": {
            "surface": { "type": "string", "description": "Surface ID or name" },
            "workspace": { "type": "string", "description": "Optional workspace scope" }
        },
        "required": ["surface"]
    })));
    tools.push(tool_def("split_pane", "Split a pane horizontally or vertically. Works in BSP layout mode. In canvas mode, creates a new panel instead.", serde_json::json!({
        "type": "object",
        "properties": {
            "direction": { "type": "string", "enum": ["horizontal", "vertical"] },
            "pane": { "type": "string", "description": "Optional pane ID or name" },
            "new_pane_name": { "type": "string", "description": "Optional name for new pane" }
        },
        "required": ["direction"]
    })));
    tools.push(tool_def("rename_pane", "Rename a pane.", serde_json::json!({
        "type": "object",
        "properties": {
            "pane": { "type": "string", "description": "Optional pane ID or name" },
            "name": { "type": "string", "description": "New pane name" }
        },
        "required": ["name"]
    })));
    tools.push(tool_def("set_layout_preset", "Apply a layout preset to a surface.", serde_json::json!({
        "type": "object",
        "properties": {
            "preset": { "type": "string", "enum": ["single", "2-columns", "3-columns", "grid-2x2", "main-stack"] },
            "surface": { "type": "string", "description": "Optional surface ID or name" },
            "workspace": { "type": "string", "description": "Optional workspace scope" }
        },
        "required": ["preset"]
    })));
    tools.push(tool_def("equalize_layout", "Equalize all split ratios in a surface.", serde_json::json!({
        "type": "object",
        "properties": {
            "surface": { "type": "string", "description": "Optional surface ID or name" },
            "workspace": { "type": "string", "description": "Optional workspace scope" }
        }
    })));
    tools.push(tool_def("list_snippets", "List saved snippets with names and content previews.", serde_json::json!({
        "type": "object",
        "properties": { "owner": { "type": "string", "enum": ["user", "assistant", "both"] } }
    })));
    tools.push(tool_def("create_snippet", "Create a new snippet.", serde_json::json!({
        "type": "object",
        "properties": {
            "name": { "type": "string" },
            "content": { "type": "string" },
            "category": { "type": "string" },
            "description": { "type": "string" },
            "tags": { "type": "array", "items": { "type": "string" } }
        },
        "required": ["name", "content"]
    })));
    tools.push(tool_def("run_snippet", "Execute a snippet by ID or name in a pane.", serde_json::json!({
        "type": "object",
        "properties": {
            "snippet": { "type": "string", "description": "Snippet ID or name" },
            "pane": { "type": "string", "description": "Optional pane ID or name" },
            "params": { "type": "object", "additionalProperties": { "type": "string" } },
            "execute": { "type": "boolean", "description": "Append Enter after inserting (default: true)" }
        },
        "required": ["snippet"]
    })));

    tools
}

fn tool_def(name: &str, description: &str, parameters: serde_json::Value) -> ToolDefinition {
    ToolDefinition {
        tool_type: "function".into(),
        function: ToolFunctionDef {
            name: name.into(),
            description: description.into(),
            parameters,
        },
    }
}

// ---------------------------------------------------------------------------
// Tool execution
// ---------------------------------------------------------------------------

pub async fn execute_tool(
    tool_call: &ToolCall,
    session_manager: &Arc<SessionManager>,
    _session_id: Option<SessionId>,
    event_tx: &broadcast::Sender<AgentEvent>,
    agent_data_dir: &std::path::Path,
    http_client: &reqwest::Client,
) -> ToolResult {
    let args: serde_json::Value = serde_json::from_str(&tool_call.function.arguments)
        .unwrap_or(serde_json::json!({}));

    tracing::info!(
        tool = %tool_call.function.name,
        args = %tool_call.function.arguments,
        "agent tool call"
    );

    let result = match tool_call.function.name.as_str() {
        // Terminal/session tools (daemon owns sessions directly)
        "list_terminals" | "list_sessions" => execute_list_sessions(session_manager).await,
        "read_active_terminal_content" => execute_read_terminal(&args, session_manager).await,
        "run_terminal_command" => execute_run_terminal_command(&args, session_manager).await,
        "type_in_terminal" => execute_type_in_terminal(&args, session_manager).await,
        // Gateway messaging (execute via CLI)
        "send_slack_message" | "send_discord_message" | "send_telegram_message" | "send_whatsapp_message" =>
            execute_gateway_message(tool_call.function.name.as_str(), &args, http_client, agent_data_dir).await,
        // Workspace/snippet tools (read/write persistence files directly)
        "list_workspaces" | "create_workspace" | "set_active_workspace" |
        "create_surface" | "set_active_surface" | "split_pane" | "rename_pane" |
        "set_layout_preset" | "equalize_layout" |
        "list_snippets" | "create_snippet" | "run_snippet" =>
            execute_workspace_tool(tool_call.function.name.as_str(), &args, event_tx).await,
        // Daemon-native tools
        "bash_command" => execute_bash(&args).await,
        "list_files" => execute_list_files(&args).await,
        "read_file" => execute_read_file(&args).await,
        "write_file" => execute_write_file(&args).await,
        "search_files" => execute_search_files(&args).await,
        "get_system_info" => execute_system_info().await,
        "list_processes" => execute_list_processes(&args).await,
        "search_history" => execute_search_history(&args, session_manager).await,
        "onecontext_search" => execute_onecontext_search(&args).await,
        "notify_user" => execute_notify(&args, event_tx).await,
        "update_memory" => execute_update_memory(&args, agent_data_dir).await,
        "web_search" => execute_web_search(&args, http_client).await,
        "fetch_url" => execute_fetch_url(&args, http_client).await,
        other => Err(anyhow::anyhow!("Unknown tool: {other}")),
    };

    match result {
        Ok(content) => {
            tracing::info!(tool = %tool_call.function.name, result_len = content.len(), "agent tool result: ok");
            ToolResult {
                tool_call_id: tool_call.id.clone(),
                name: tool_call.function.name.clone(),
                content,
                is_error: false,
            }
        }
        Err(e) => {
            tracing::warn!(tool = %tool_call.function.name, error = %e, "agent tool result: error");
            ToolResult {
                tool_call_id: tool_call.id.clone(),
                name: tool_call.function.name.clone(),
                content: format!("Error: {e}"),
                is_error: true,
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tool implementations
// ---------------------------------------------------------------------------

async fn execute_bash(args: &serde_json::Value) -> Result<String> {
    let command = args
        .get("command")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing 'command' argument"))?;

    let timeout_secs = args
        .get("timeout_seconds")
        .and_then(|v| v.as_u64())
        .unwrap_or(30);

    let cwd = args.get("cwd").and_then(|v| v.as_str());

    let mut cmd = tokio::process::Command::new("sh");
    cmd.arg("-c").arg(command);
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }

    let output = tokio::time::timeout(
        std::time::Duration::from_secs(timeout_secs),
        cmd.output(),
    )
    .await
    .map_err(|_| anyhow::anyhow!("command timed out after {timeout_secs}s"))??;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let exit_code = output.status.code().unwrap_or(-1);

    // Truncate large output
    let max_chars = 50_000;
    let stdout_str = if stdout.len() > max_chars {
        format!("{}...\n(truncated, {} chars total)", &stdout[..max_chars], stdout.len())
    } else {
        stdout.to_string()
    };

    let mut result = format!("Exit code: {exit_code}");
    if !stdout_str.is_empty() {
        result.push_str(&format!("\n\nStdout:\n{stdout_str}"));
    }
    if !stderr.is_empty() {
        let stderr_str = if stderr.len() > 5000 {
            format!("{}...(truncated)", &stderr[..5000])
        } else {
            stderr.to_string()
        };
        result.push_str(&format!("\n\nStderr:\n{stderr_str}"));
    }

    Ok(result)
}

async fn execute_list_files(args: &serde_json::Value) -> Result<String> {
    let path = args
        .get("path")
        .and_then(|v| v.as_str())
        .unwrap_or(".");

    let mut entries = tokio::fs::read_dir(path).await?;
    let mut items = Vec::new();

    while let Some(entry) = entries.next_entry().await? {
        let metadata = entry.metadata().await?;
        let kind = if metadata.is_dir() { "dir" } else { "file" };
        let size = metadata.len();
        let name = entry.file_name().to_string_lossy().to_string();
        items.push(format!("{kind}\t{size}\t{name}"));
    }

    items.sort();
    if items.is_empty() {
        Ok("(empty directory)".into())
    } else {
        Ok(items.join("\n"))
    }
}

async fn execute_read_file(args: &serde_json::Value) -> Result<String> {
    let path = args
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing 'path' argument"))?;

    let max_lines = args
        .get("max_lines")
        .and_then(|v| v.as_u64())
        .unwrap_or(200) as usize;

    let content = tokio::fs::read_to_string(path).await?;
    let total_lines = content.lines().count();
    let lines: Vec<&str> = content.lines().take(max_lines).collect();

    let mut result = lines.join("\n");
    if total_lines > max_lines {
        result.push_str(&format!(
            "\n\n... (truncated, showing {max_lines} of {total_lines} lines)"
        ));
    }

    Ok(result)
}

async fn execute_write_file(args: &serde_json::Value) -> Result<String> {
    let path = args
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing 'path' argument"))?;

    let content = args
        .get("content")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing 'content' argument"))?;

    // Ensure parent directory exists
    if let Some(parent) = std::path::Path::new(path).parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    tokio::fs::write(path, content).await?;
    Ok(format!("Written {} bytes to {path}", content.len()))
}

async fn execute_search_files(args: &serde_json::Value) -> Result<String> {
    let pattern = args
        .get("pattern")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing 'pattern' argument"))?;

    let path = args
        .get("path")
        .and_then(|v| v.as_str())
        .unwrap_or(".");

    let max_results = args
        .get("max_results")
        .and_then(|v| v.as_u64())
        .unwrap_or(50);

    let file_pattern = args.get("file_pattern").and_then(|v| v.as_str());

    // Use grep for search
    let mut cmd_args = vec!["-rn".to_string(), "--color=never".to_string()];
    if let Some(fp) = file_pattern {
        cmd_args.push(format!("--include={fp}"));
    }
    cmd_args.push(pattern.to_string());
    cmd_args.push(path.to_string());

    let output = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        tokio::process::Command::new("grep")
            .args(&cmd_args)
            .output(),
    )
    .await
    .map_err(|_| anyhow::anyhow!("search timed out"))??;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.lines().take(max_results as usize).collect();
    let total = stdout.lines().count();

    if lines.is_empty() {
        Ok("No matches found.".into())
    } else {
        let mut result = lines.join("\n");
        if total > lines.len() {
            result.push_str(&format!("\n\n... ({} more matches)", total - lines.len()));
        }
        Ok(result)
    }
}

async fn execute_system_info() -> Result<String> {
    use sysinfo::System;

    let mut sys = System::new_all();
    sys.refresh_all();

    let total_mem = sys.total_memory();
    let used_mem = sys.used_memory();
    let cpu_count = sys.cpus().len();
    let load_avg = System::load_average();

    Ok(format!(
        "CPU cores: {cpu_count}\n\
         Load average: {:.2} {:.2} {:.2}\n\
         Memory: {:.1} GB / {:.1} GB ({:.0}% used)\n\
         OS: {} {}\n\
         Hostname: {}",
        load_avg.one,
        load_avg.five,
        load_avg.fifteen,
        used_mem as f64 / 1_073_741_824.0,
        total_mem as f64 / 1_073_741_824.0,
        (used_mem as f64 / total_mem as f64) * 100.0,
        System::name().unwrap_or_default(),
        System::os_version().unwrap_or_default(),
        System::host_name().unwrap_or_default(),
    ))
}

async fn execute_list_processes(args: &serde_json::Value) -> Result<String> {
    let limit = args
        .get("limit")
        .and_then(|v| v.as_u64())
        .unwrap_or(20) as usize;

    use sysinfo::System;
    let mut sys = System::new_all();
    sys.refresh_all();

    let mut procs: Vec<(u32, String, f32, u64)> = sys
        .processes()
        .values()
        .map(|p| {
            (
                p.pid().as_u32(),
                p.name().to_string(),
                p.cpu_usage(),
                p.memory(),
            )
        })
        .collect();

    procs.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

    let header = format!("{:<8} {:<30} {:>8} {:>12}", "PID", "NAME", "CPU%", "MEM(MB)");
    let rows: Vec<String> = procs
        .iter()
        .take(limit)
        .map(|(pid, name, cpu, mem)| {
            format!(
                "{:<8} {:<30} {:>7.1}% {:>12.1}",
                pid,
                if name.len() > 30 { &name[..30] } else { name },
                cpu,
                *mem as f64 / 1_048_576.0
            )
        })
        .collect();

    Ok(format!("{header}\n{}", rows.join("\n")))
}

async fn execute_search_history(
    args: &serde_json::Value,
    session_manager: &Arc<SessionManager>,
) -> Result<String> {
    let query = args
        .get("query")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing 'query' argument"))?;

    let limit = args
        .get("limit")
        .and_then(|v| v.as_u64())
        .unwrap_or(20) as usize;

    let (summary, hits) = session_manager.search_history(query, limit)?;

    if hits.is_empty() {
        Ok("No matching history entries.".into())
    } else {
        let lines: Vec<String> = hits
            .iter()
            .map(|h| {
                format!(
                    "[{:.1}] {} — {}",
                    h.score,
                    h.title,
                    h.excerpt.chars().take(120).collect::<String>(),
                )
            })
            .collect();
        Ok(format!("{summary}\n\n{}", lines.join("\n")))
    }
}

async fn execute_onecontext_search(args: &serde_json::Value) -> Result<String> {
    let query = args
        .get("query")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing 'query' argument"))?
        .trim();

    if query.is_empty() {
        return Err(anyhow::anyhow!("'query' must not be empty"));
    }

    let scope = args
        .get("scope")
        .and_then(|v| v.as_str())
        .unwrap_or("session");
    if !matches!(scope, "session" | "event" | "turn") {
        return Err(anyhow::anyhow!(
            "invalid 'scope': {scope} (expected session, event, or turn)"
        ));
    }

    if which::which("aline").is_err() {
        return Ok("OneContext search unavailable: `aline` CLI not found on PATH.".into());
    }

    let no_regex = args
        .get("no_regex")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let bounded_query = query
        .chars()
        .take(ONECONTEXT_TOOL_QUERY_MAX_CHARS)
        .collect::<String>();

    let mut cmd = tokio::process::Command::new("aline");
    cmd.arg("search")
        .arg(&bounded_query)
        .arg("-t")
        .arg(scope)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .stdin(std::process::Stdio::null());
    if no_regex {
        cmd.arg("--no-regex");
    }

    let output = tokio::time::timeout(
        std::time::Duration::from_secs(ONECONTEXT_TOOL_TIMEOUT_SECS),
        cmd.output(),
    )
    .await
    .map_err(|_| anyhow::anyhow!("onecontext search timed out"))??;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if stderr.is_empty() {
            return Err(anyhow::anyhow!("onecontext search failed"));
        }
        return Err(anyhow::anyhow!("onecontext search failed: {stderr}"));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return Ok(format!(
            "No OneContext matches for \"{bounded_query}\" in {scope} scope."
        ));
    }

    let trimmed_chars = trimmed.chars().count();
    let output_text = if trimmed_chars > ONECONTEXT_TOOL_OUTPUT_MAX_CHARS {
        let shortened = trimmed
            .chars()
            .take(ONECONTEXT_TOOL_OUTPUT_MAX_CHARS)
            .collect::<String>();
        format!(
            "{}\n\n(truncated, {} chars total)",
            shortened,
            trimmed_chars
        )
    } else {
        trimmed.to_string()
    };

    Ok(format!(
        "OneContext results for \"{bounded_query}\" ({scope} scope):\n\n{output_text}"
    ))
}

async fn execute_list_sessions(session_manager: &Arc<SessionManager>) -> Result<String> {
    let sessions = session_manager.list().await;

    if sessions.is_empty() {
        Ok("No active sessions.".into())
    } else {
        let lines: Vec<String> = sessions
            .iter()
            .map(|s| {
                format!(
                    "{} cols={} rows={} alive={} cwd={}",
                    s.id,
                    s.cols,
                    s.rows,
                    s.is_alive,
                    s.cwd.as_deref().unwrap_or("?"),
                )
            })
            .collect();
        Ok(lines.join("\n"))
    }
}

async fn execute_notify(
    args: &serde_json::Value,
    event_tx: &broadcast::Sender<AgentEvent>,
) -> Result<String> {
    let title = args
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("Notification");
    let message = args
        .get("message")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let severity = match args.get("severity").and_then(|v| v.as_str()) {
        Some("warning") => NotificationSeverity::Warning,
        Some("alert") => NotificationSeverity::Alert,
        Some("error") => NotificationSeverity::Error,
        _ => NotificationSeverity::Info,
    };

    let _ = event_tx.send(AgentEvent::Notification {
        title: title.into(),
        body: message.into(),
        severity,
        channels: vec!["in-app".into()],
    });

    Ok(format!("Notification sent: {title}"))
}

async fn execute_update_memory(
    args: &serde_json::Value,
    agent_data_dir: &std::path::Path,
) -> Result<String> {
    let content = args
        .get("content")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing 'content' argument"))?;

    let memory_dir = super::active_memory_dir(agent_data_dir);
    let memory_path = memory_dir.join("MEMORY.md");
    tokio::fs::create_dir_all(&memory_dir).await?;
    tokio::fs::write(&memory_path, content).await?;

    Ok("Memory updated successfully.".into())
}

async fn execute_web_search(
    args: &serde_json::Value,
    http_client: &reqwest::Client,
) -> Result<String> {
    let query = args
        .get("query")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing 'query' argument"))?;

    let max_results = args
        .get("max_results")
        .and_then(|v| v.as_u64())
        .unwrap_or(5);

    // Use DuckDuckGo lite as a zero-config fallback
    let url = format!(
        "https://lite.duckduckgo.com/lite/?q={}&kl=us-en",
        urlencoding::encode(query)
    );

    let resp = http_client
        .get(&url)
        .header("User-Agent", "tamux-agent/0.1")
        .send()
        .await?;

    let text = resp.text().await?;

    // Extract result snippets from DDG lite HTML
    let mut results = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("<a rel=\"nofollow\"") {
            // Extract URL and text
            if let (Some(href_start), Some(href_end)) = (trimmed.find("href=\""), trimmed.find("\">")) {
                let url = &trimmed[href_start + 6..href_end];
                let text_start = href_end + 2;
                if let Some(text_end) = trimmed[text_start..].find("</a>") {
                    let title = &trimmed[text_start..text_start + text_end];
                    results.push(format!("- {title}\n  {url}"));
                }
            }
        }
        if results.len() >= max_results as usize {
            break;
        }
    }

    if results.is_empty() {
        Ok(format!("No web results found for: {query}"))
    } else {
        Ok(format!("Web results for \"{query}\":\n\n{}", results.join("\n\n")))
    }
}

async fn execute_fetch_url(
    args: &serde_json::Value,
    http_client: &reqwest::Client,
) -> Result<String> {
    let url = args
        .get("url")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing 'url' argument"))?;

    let max_length = args
        .get("max_length")
        .and_then(|v| v.as_u64())
        .unwrap_or(10_000) as usize;

    let resp = http_client
        .get(url)
        .header("User-Agent", "tamux-agent/0.1")
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await?;

    let status = resp.status();
    let text = resp.text().await?;

    // Basic HTML tag stripping
    let stripped = strip_html_tags(&text);
    let truncated = if stripped.len() > max_length {
        format!(
            "{}...\n\n(truncated, {} chars total)",
            &stripped[..max_length],
            stripped.len()
        )
    } else {
        stripped
    };

    Ok(format!("HTTP {status}\n\n{truncated}"))
}

// ---------------------------------------------------------------------------
// Terminal/session tools — daemon owns sessions directly
// ---------------------------------------------------------------------------

async fn execute_read_terminal(
    args: &serde_json::Value,
    session_manager: &Arc<SessionManager>,
) -> Result<String> {
    let sessions = session_manager.list().await;
    if sessions.is_empty() {
        return Ok("No active terminal sessions.".into());
    }

    let target_id = if let Some(pane) = args.get("pane").and_then(|v| v.as_str()) {
        sessions.iter().find(|s| s.id.to_string().contains(pane)).map(|s| s.id)
    } else {
        None
    };

    let sid = target_id.unwrap_or(sessions[0].id);

    // Read full scrollback, no line limit — get everything the session has
    match session_manager.get_scrollback(sid, None).await {
        Ok(data) => {
            if data.is_empty() {
                return Ok("(terminal buffer is empty)".into());
            }

            // Strip ANSI escapes using the strip-ansi-escapes crate (already in deps)
            let stripped = strip_ansi_escapes::strip(&data);
            let text = String::from_utf8_lossy(&stripped);

            // Take last 200 lines to keep output manageable
            let lines: Vec<&str> = text.lines().collect();
            let start = if lines.len() > 200 { lines.len() - 200 } else { 0 };
            let visible: Vec<&str> = lines[start..].iter()
                .filter(|l| !l.trim().is_empty())
                .copied()
                .collect();

            if visible.is_empty() {
                Ok("(terminal buffer contains only whitespace/control characters)".into())
            } else {
                let mut result = visible.join("\n");
                if start > 0 {
                    result = format!("... ({} earlier lines omitted)\n\n{result}", start);
                }
                Ok(result)
            }
        }
        Err(e) => Ok(format!("Failed to read terminal: {e}")),
    }
}

async fn execute_run_terminal_command(
    args: &serde_json::Value,
    _session_manager: &Arc<SessionManager>,
) -> Result<String> {
    // Agent uses direct process execution — bypasses the managed lane
    // and approval system. The daemon agent is a trusted process.
    execute_bash(args).await
}

async fn execute_type_in_terminal(
    args: &serde_json::Value,
    session_manager: &Arc<SessionManager>,
) -> Result<String> {
    let sessions = session_manager.list().await;
    if sessions.is_empty() {
        return Err(anyhow::anyhow!("No active terminal sessions to type into"));
    }

    let target_id = if let Some(pane) = args.get("pane").and_then(|v| v.as_str()) {
        sessions.iter().find(|s| s.id.to_string().contains(pane)).map(|s| s.id)
    } else {
        sessions.first().map(|s| s.id)
    };

    let sid = target_id.ok_or_else(|| anyhow::anyhow!("Target session not found"))?;

    // Check if sending a special key
    let description;
    let input: Vec<u8> = if let Some(key) = args.get("key").and_then(|v| v.as_str()) {
        description = format!("key:{key}");
        resolve_key_sequence(key)
    } else {
        let text = args.get("text").and_then(|v| v.as_str()).unwrap_or("");
        let press_enter = args.get("press_enter").and_then(|v| v.as_bool()).unwrap_or(true);

        description = if press_enter {
            format!("{text} + Enter")
        } else {
            text.to_string()
        };

        // Send text first
        if !text.is_empty() {
            session_manager.write_input(sid, text.as_bytes()).await?;
        }
        if press_enter {
            // Small delay so the TUI processes the text before Enter
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            session_manager.write_input(sid, b"\r").await?;
        }

        // Signal that we already sent — skip the write_input below
        Vec::new()
    };

    if !input.is_empty() {
        session_manager.write_input(sid, &input).await?;
    }

    // Wait for the terminal to process input
    tokio::time::sleep(std::time::Duration::from_millis(800)).await;

    // Read back terminal content to see the result
    match session_manager.get_scrollback(sid, None).await {
        Ok(data) => {
            let stripped = strip_ansi_escapes::strip(&data);
            let text_out = String::from_utf8_lossy(&stripped);
            let lines: Vec<&str> = text_out.lines().collect();
            let start = if lines.len() > 30 { lines.len() - 30 } else { 0 };
            let visible: Vec<&str> = lines[start..].iter()
                .filter(|l| !l.trim().is_empty())
                .copied()
                .collect();

            Ok(format!(
                "Sent '{description}' to session {sid}\n\nTerminal output (last 30 lines):\n{}",
                visible.join("\n"),
            ))
        }
        Err(_) => {
            Ok(format!("Sent '{description}' to session {sid}"))
        }
    }
}

/// Resolve a key name to its terminal escape sequence bytes.
fn resolve_key_sequence(key: &str) -> Vec<u8> {
    match key.to_lowercase().as_str() {
        "enter" | "return" => vec![b'\r'],
        "ctrl+c" => vec![0x03],
        "ctrl+d" => vec![0x04],
        "ctrl+z" => vec![0x1a],
        "ctrl+l" => vec![0x0c],
        "ctrl+a" => vec![0x01],
        "ctrl+e" => vec![0x05],
        "ctrl+u" => vec![0x15],
        "ctrl+k" => vec![0x0b],
        "ctrl+w" => vec![0x17],
        "ctrl+r" => vec![0x12],
        "ctrl+p" => vec![0x10],
        "ctrl+n" => vec![0x0e],
        "escape" | "esc" => vec![0x1b],
        "tab" => vec![b'\t'],
        "backspace" => vec![0x7f],
        "delete" => vec![0x1b, b'[', b'3', b'~'],
        "up" => vec![0x1b, b'[', b'A'],
        "down" => vec![0x1b, b'[', b'B'],
        "right" => vec![0x1b, b'[', b'C'],
        "left" => vec![0x1b, b'[', b'D'],
        "home" => vec![0x1b, b'[', b'H'],
        "end" => vec![0x1b, b'[', b'F'],
        "page_up" => vec![0x1b, b'[', b'5', b'~'],
        "page_down" => vec![0x1b, b'[', b'6', b'~'],
        // F-keys
        "f1" => vec![0x1b, b'O', b'P'],
        "f2" => vec![0x1b, b'O', b'Q'],
        "f3" => vec![0x1b, b'O', b'R'],
        "f4" => vec![0x1b, b'O', b'S'],
        // Default: send as raw text
        other => other.as_bytes().to_vec(),
    }
}

// ---------------------------------------------------------------------------
// Gateway messaging — execute via CLI subprocess
// ---------------------------------------------------------------------------

async fn execute_gateway_message(
    tool_name: &str,
    args: &serde_json::Value,
    http_client: &reqwest::Client,
    agent_data_dir: &std::path::Path,
) -> Result<String> {
    let message = args.get("message").and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing 'message' argument"))?;

    // Read gateway tokens from agent config or settings.json
    let config_path = agent_data_dir.join("config.json");
    let settings_path = agent_data_dir
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .join("settings.json");

    let (slack_token, telegram_token, discord_token) = {
        // Try agent config first
        let mut st = String::new();
        let mut tt = String::new();
        let mut dt = String::new();

        if let Ok(raw) = tokio::fs::read_to_string(&config_path).await {
            if let Ok(cfg) = serde_json::from_str::<serde_json::Value>(&raw) {
                st = cfg.pointer("/gateway/slack_token").and_then(|v| v.as_str()).unwrap_or("").to_string();
                tt = cfg.pointer("/gateway/telegram_token").and_then(|v| v.as_str()).unwrap_or("").to_string();
                dt = cfg.pointer("/gateway/discord_token").and_then(|v| v.as_str()).unwrap_or("").to_string();
            }
        }

        // Fall back to settings.json
        if st.is_empty() && tt.is_empty() && dt.is_empty() {
            if let Ok(raw) = tokio::fs::read_to_string(&settings_path).await {
                if let Ok(s) = serde_json::from_str::<serde_json::Value>(&raw) {
                    st = s.pointer("/settings/slackToken").or_else(|| s.get("slackToken")).and_then(|v| v.as_str()).unwrap_or("").to_string();
                    tt = s.pointer("/settings/telegramToken").or_else(|| s.get("telegramToken")).and_then(|v| v.as_str()).unwrap_or("").to_string();
                    dt = s.pointer("/settings/discordToken").or_else(|| s.get("discordToken")).and_then(|v| v.as_str()).unwrap_or("").to_string();
                }
            }
        }

        (st, tt, dt)
    };

    // Read default targets from settings for when user doesn't specify
    let settings: serde_json::Value = if let Ok(raw) = tokio::fs::read_to_string(&settings_path).await {
        serde_json::from_str(&raw).unwrap_or_default()
    } else {
        serde_json::Value::Null
    };

    let setting = |key: &str| -> String {
        settings.pointer(&format!("/settings/{key}"))
            .or_else(|| settings.get(key))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    };

    let first_csv = |val: &str| -> String {
        val.split(',').next().unwrap_or("").trim().to_string()
    };

    match tool_name {
        "send_slack_message" => {
            let channel = args.get("channel").and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| first_csv(&setting("slackChannelFilter")));
            if channel.is_empty() {
                return Err(anyhow::anyhow!("No channel specified and no default slackChannelFilter in settings"));
            }
            let channel = channel.as_str();
            if slack_token.is_empty() {
                return Err(anyhow::anyhow!("Slack token not configured in settings"));
            }
            tracing::info!(platform = "slack", channel = %channel, message = %message, "gateway: sending message");
            let resp = http_client
                .post("https://slack.com/api/chat.postMessage")
                .bearer_auth(&slack_token)
                .json(&serde_json::json!({ "channel": channel, "text": message }))
                .send().await?;
            let body: serde_json::Value = resp.json().await?;
            if body.get("ok").and_then(|v| v.as_bool()) == Some(true) {
                Ok(format!("Slack message sent to #{channel}"))
            } else {
                let err = body.get("error").and_then(|v| v.as_str()).unwrap_or("unknown error");
                Err(anyhow::anyhow!("Slack API error: {err}"))
            }
        }
        "send_discord_message" => {
            let mut channel_id = args.get("channel_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let mut user_id = args.get("user_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
            // Fall back to defaults from settings
            if channel_id.is_empty() && user_id.is_empty() {
                let default_channel = first_csv(&setting("discordChannelFilter"));
                if !default_channel.is_empty() {
                    channel_id = default_channel;
                } else {
                    let default_user = first_csv(&setting("discordAllowedUsers"));
                    if !default_user.is_empty() {
                        user_id = default_user;
                    }
                }
            }
            let channel_id = channel_id.as_str();
            let user_id = user_id.as_str();
            if discord_token.is_empty() {
                return Err(anyhow::anyhow!("Discord token not configured in settings"));
            }

            let target_channel = if !channel_id.is_empty() {
                channel_id.to_string()
            } else if !user_id.is_empty() {
                // Create DM channel first
                let resp = http_client
                    .post("https://discord.com/api/v10/users/@me/channels")
                    .header("Authorization", format!("Bot {discord_token}"))
                    .json(&serde_json::json!({ "recipient_id": user_id }))
                    .send().await?;
                let body: serde_json::Value = resp.json().await?;
                body.get("id").and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Failed to create DM channel: {body}"))?
                    .to_string()
            } else {
                return Err(anyhow::anyhow!("Either channel_id or user_id is required"));
            };

            tracing::info!(platform = "discord", channel = %target_channel, message = %message, "gateway: sending message");
            let resp = http_client
                .post(format!("https://discord.com/api/v10/channels/{target_channel}/messages"))
                .header("Authorization", format!("Bot {discord_token}"))
                .json(&serde_json::json!({ "content": message }))
                .send().await?;

            if resp.status().is_success() {
                Ok(format!("Discord message sent to {target_channel}"))
            } else {
                let body = resp.text().await.unwrap_or_default();
                Err(anyhow::anyhow!("Discord API error: {body}"))
            }
        }
        "send_telegram_message" => {
            let chat_id = args.get("chat_id").and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| first_csv(&setting("telegramAllowedChats")));
            if chat_id.is_empty() {
                return Err(anyhow::anyhow!("No chat_id specified and no default telegramAllowedChats in settings"));
            }
            let chat_id = chat_id.as_str();
            if telegram_token.is_empty() {
                return Err(anyhow::anyhow!("Telegram token not configured in settings"));
            }
            tracing::info!(platform = "telegram", chat_id = %chat_id, message = %message, "gateway: sending message");
            let url = format!("https://api.telegram.org/bot{telegram_token}/sendMessage");
            let resp = http_client
                .post(&url)
                .json(&serde_json::json!({ "chat_id": chat_id, "text": message }))
                .send().await?;
            let body: serde_json::Value = resp.json().await?;
            if body.get("ok").and_then(|v| v.as_bool()) == Some(true) {
                Ok(format!("Telegram message sent to {chat_id}"))
            } else {
                let desc = body.get("description").and_then(|v| v.as_str()).unwrap_or("unknown error");
                Err(anyhow::anyhow!("Telegram API error: {desc}"))
            }
        }
        "send_whatsapp_message" => {
            let phone = args.get("phone").and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| first_csv(&setting("whatsappAllowedContacts")));
            if phone.is_empty() {
                return Err(anyhow::anyhow!("No phone specified and no default whatsappAllowedContacts in settings"));
            }
            let phone = phone.as_str();
            // WhatsApp requires separate phone_number_id and token — read from settings
            let settings: serde_json::Value = if let Ok(raw) = tokio::fs::read_to_string(&settings_path).await {
                serde_json::from_str(&raw).unwrap_or_default()
            } else {
                serde_json::Value::Null
            };
            let wa_token = settings.pointer("/settings/whatsappToken").or_else(|| settings.get("whatsappToken")).and_then(|v| v.as_str()).unwrap_or("");
            let phone_id = settings.pointer("/settings/whatsappPhoneNumberId").or_else(|| settings.get("whatsappPhoneNumberId")).and_then(|v| v.as_str()).unwrap_or("");
            if wa_token.is_empty() || phone_id.is_empty() {
                return Err(anyhow::anyhow!("WhatsApp token/phoneNumberId not configured"));
            }
            tracing::info!(platform = "whatsapp", phone = %phone, "gateway: sending message");
            let url = format!("https://graph.facebook.com/v18.0/{phone_id}/messages");
            let resp = http_client
                .post(&url)
                .bearer_auth(wa_token)
                .json(&serde_json::json!({
                    "messaging_product": "whatsapp",
                    "to": phone,
                    "type": "text",
                    "text": { "body": message }
                }))
                .send().await?;
            if resp.status().is_success() {
                Ok(format!("WhatsApp message sent to {phone}"))
            } else {
                let body = resp.text().await.unwrap_or_default();
                Err(anyhow::anyhow!("WhatsApp API error: {body}"))
            }
        }
        _ => Err(anyhow::anyhow!("unknown gateway tool")),
    }
}

// ---------------------------------------------------------------------------
// Workspace/snippet tools — read/write persistence files
// ---------------------------------------------------------------------------

async fn execute_workspace_tool(
    tool_name: &str,
    args: &serde_json::Value,
    event_tx: &broadcast::Sender<AgentEvent>,
) -> Result<String> {
    let data_dir = super::agent_data_dir().parent().unwrap_or(std::path::Path::new(".")).to_path_buf();

    match tool_name {
        "list_workspaces" => {
            let session_path = data_dir.join("session.json");
            match tokio::fs::read_to_string(&session_path).await {
                Ok(raw) => {
                    let parsed: serde_json::Value = serde_json::from_str(&raw)?;
                    let workspaces = parsed.get("workspaces").and_then(|w| w.as_array());
                    match workspaces {
                        Some(ws) => {
                            let mut lines = Vec::new();
                            for w in ws {
                                let name = w.get("name").and_then(|v| v.as_str()).unwrap_or("?");
                                let id = w.get("id").and_then(|v| v.as_str()).unwrap_or("?");
                                let surfaces = w.get("surfaces").and_then(|v| v.as_array()).map(|s| s.len()).unwrap_or(0);
                                lines.push(format!("- {name} (id: {id}, {surfaces} surfaces)"));
                            }
                            Ok(lines.join("\n"))
                        }
                        None => Ok("No workspaces found.".into()),
                    }
                }
                Err(_) => Ok("No session file found (app may not have saved state yet).".into()),
            }
        }
        "list_snippets" => {
            let snippets_path = data_dir.join("snippets.json");
            match tokio::fs::read_to_string(&snippets_path).await {
                Ok(raw) => {
                    let parsed: serde_json::Value = serde_json::from_str(&raw)?;
                    let snippets = parsed.as_array();
                    match snippets {
                        Some(ss) => {
                            let mut lines = Vec::new();
                            for s in ss {
                                let name = s.get("name").and_then(|v| v.as_str()).unwrap_or("?");
                                let content = s.get("content").and_then(|v| v.as_str()).unwrap_or("");
                                let preview: String = content.chars().take(60).collect();
                                lines.push(format!("- {name}: {preview}"));
                            }
                            Ok(lines.join("\n"))
                        }
                        None => Ok("No snippets found.".into()),
                    }
                }
                Err(_) => Ok("No snippets file found.".into()),
            }
        }
        // Mutation tools — emit WorkspaceCommand event for frontend execution
        other => {
            let _ = event_tx.send(AgentEvent::WorkspaceCommand {
                command: other.to_string(),
                args: args.clone(),
            });
            Ok(format!("Executed {other}"))
        }
    }
}

fn strip_ansi_codes(text: &str) -> String {
    // Simple ANSI escape stripping
    let mut result = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            // Skip escape sequence
            if let Some(&next) = chars.peek() {
                if next == '[' {
                    chars.next();
                    // Skip until terminator (letter)
                    while let Some(&c) = chars.peek() {
                        chars.next();
                        if c.is_ascii_alphabetic() || c == '~' {
                            break;
                        }
                    }
                } else if next == ']' {
                    chars.next();
                    // Skip OSC until BEL or ST
                    while let Some(c) = chars.next() {
                        if c == '\x07' { break; }
                        if c == '\x1b' {
                            if chars.peek() == Some(&'\\') { chars.next(); break; }
                        }
                    }
                }
            }
        } else {
            result.push(ch);
        }
    }
    result
}

fn strip_html_tags(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let mut in_tag = false;
    let mut in_script = false;
    let mut in_style = false;

    let lower = html.to_lowercase();
    let chars: Vec<char> = html.chars().collect();
    let lower_chars: Vec<char> = lower.chars().collect();

    let mut i = 0;
    while i < chars.len() {
        if !in_tag && chars[i] == '<' {
            // Check for script/style
            let remaining: String = lower_chars[i..].iter().take(10).collect();
            if remaining.starts_with("<script") {
                in_script = true;
            } else if remaining.starts_with("<style") {
                in_style = true;
            } else if remaining.starts_with("</script") {
                in_script = false;
            } else if remaining.starts_with("</style") {
                in_style = false;
            }
            in_tag = true;
        } else if in_tag && chars[i] == '>' {
            in_tag = false;
        } else if !in_tag && !in_script && !in_style {
            result.push(chars[i]);
        }
        i += 1;
    }

    // Collapse whitespace
    let mut collapsed = String::new();
    let mut last_was_space = false;
    for ch in result.chars() {
        if ch.is_whitespace() {
            if !last_was_space {
                collapsed.push(if ch == '\n' { '\n' } else { ' ' });
                last_was_space = true;
            }
        } else {
            collapsed.push(ch);
            last_was_space = false;
        }
    }

    collapsed.trim().to_string()
}

// Minimal URL encoding (only used for web_search query)
mod urlencoding {
    pub fn encode(s: &str) -> String {
        let mut result = String::new();
        for byte in s.bytes() {
            match byte {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                    result.push(byte as char);
                }
                b' ' => result.push('+'),
                _ => {
                    result.push('%');
                    result.push_str(&format!("{:02X}", byte));
                }
            }
        }
        result
    }
}
