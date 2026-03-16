//! Manages external agent processes (OpenClaw, Hermes) as subprocesses.
//!
//! When `agent_backend` is set to "openclaw" or "hermes", the daemon routes
//! messages through the external agent's CLI rather than using the built-in
//! LLM client. The agent uses its own provider, tools, memory, and gateway
//! connections.

use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Context, Result};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

use super::types::AgentEvent;

/// Maximum time (seconds) to wait for a one-shot agent response.
const ONE_SHOT_TIMEOUT_SECS: u64 = 300;

#[derive(Debug)]
struct StreamCancelledError;

impl std::fmt::Display for StreamCancelledError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("stream cancelled")
    }
}

impl std::error::Error for StreamCancelledError {}

pub fn is_stream_cancelled(error: &anyhow::Error) -> bool {
    error.downcast_ref::<StreamCancelledError>().is_some()
}

// ---------------------------------------------------------------------------
// External agent runner
// ---------------------------------------------------------------------------

pub struct ExternalAgentRunner {
    agent_type: String,
    executable: Option<String>,
    event_tx: broadcast::Sender<AgentEvent>,
    gateway_process: tokio::sync::Mutex<Option<tokio::process::Child>>,
    /// Whether tamux-mcp is configured in this agent's MCP config.
    has_tamux_mcp: bool,
}

/// Information about an external agent's availability.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExternalAgentStatus {
    pub agent_type: String,
    pub available: bool,
    pub executable: Option<String>,
    pub version: Option<String>,
    pub has_tamux_mcp: bool,
}

impl ExternalAgentRunner {
    pub fn new(agent_type: &str, event_tx: broadcast::Sender<AgentEvent>) -> Self {
        let executable = find_executable(agent_type);
        let has_tamux_mcp = check_tamux_mcp_configured(agent_type);
        Self {
            agent_type: agent_type.to_string(),
            executable,
            event_tx,
            gateway_process: tokio::sync::Mutex::new(None),
            has_tamux_mcp,
        }
    }

    /// Whether tamux-mcp is configured in this agent's MCP settings.
    pub fn has_tamux_mcp(&self) -> bool {
        self.has_tamux_mcp
    }

    /// Whether the external agent executable was found on PATH.
    pub fn is_available(&self) -> bool {
        self.executable.is_some()
    }

    /// Get status information about this external agent.
    pub fn status(&self) -> ExternalAgentStatus {
        ExternalAgentStatus {
            agent_type: self.agent_type.clone(),
            available: self.executable.is_some(),
            executable: self.executable.clone(),
            version: None,
            has_tamux_mcp: self.has_tamux_mcp,
        }
    }

    /// Send a one-shot message to the external agent and collect the response.
    ///
    /// Spawns the agent CLI in one-shot mode, reads stdout line-by-line to
    /// stream deltas, strips TUI noise, and enforces a timeout.
    pub async fn send_message(
        &self,
        thread_id: &str,
        prompt: &str,
        cancel_token: Option<CancellationToken>,
    ) -> Result<String> {
        let request_started_at = Instant::now();
        let exe = self
            .executable
            .as_deref()
            .context(format!("{} executable not found on PATH", self.agent_type))?;

        let args = build_one_shot_args(&self.agent_type, prompt);

        tracing::info!(
            agent = %self.agent_type,
            exe = %exe,
            "external agent: sending message"
        );

        let mut child = tokio::process::Command::new(exe)
            .args(&args)
            // Suppress TUI decorations as much as possible
            .env("TERM", "dumb")
            .env("NO_COLOR", "1")
            .env("CI", "1")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .stdin(std::process::Stdio::null())
            .spawn()
            .context(format!("failed to spawn {} process", self.agent_type))?;

        let stdout = child.stdout.take().context("failed to capture stdout")?;
        let stderr = child.stderr.take().context("failed to capture stderr")?;

        // Read stdout line-by-line with timeout
        let tid = thread_id.to_string();
        let agent_type = self.agent_type.clone();
        let event_tx = self.event_tx.clone();
        // OpenClaw with --json outputs structured JSON — collect raw without noise filtering
        let is_json_mode = agent_type == "openclaw";

        let read_future = async {
            let mut reader = BufReader::new(stdout).lines();
            let mut collected = Vec::<String>::new();
            let mut first_output_at: Option<Instant> = None;

            while let Some(line) = reader.next_line().await? {
                if is_json_mode {
                    // Collect raw JSON lines without filtering
                    if first_output_at.is_none() && !line.trim().is_empty() {
                        first_output_at = Some(Instant::now());
                    }
                    collected.push(line);
                } else {
                    let clean = strip_ansi(&line);
                    let clean = clean.trim();

                    if clean.is_empty() || is_tui_noise(clean) {
                        continue;
                    }

                    if first_output_at.is_none() {
                        first_output_at = Some(Instant::now());
                    }
                    collected.push(clean.to_string());

                    // Stream each line as a delta event (for Hermes)
                    let content = if collected.len() > 1 {
                        format!("\n{clean}")
                    } else {
                        clean.to_string()
                    };
                    let _ = event_tx.send(AgentEvent::Delta {
                        thread_id: tid.clone(),
                        content,
                    });
                }
            }

            // Also capture stderr for error context
            let mut stderr_reader = BufReader::new(stderr).lines();
            let mut stderr_lines = Vec::new();
            while let Some(line) = stderr_reader.next_line().await? {
                let clean = strip_ansi(&line);
                let trimmed = clean.trim();
                if !trimmed.is_empty() {
                    stderr_lines.push(trimmed.to_string());
                }
            }

            Ok::<(Vec<String>, Vec<String>, Option<Instant>), anyhow::Error>((
                collected,
                stderr_lines,
                first_output_at,
            ))
        };

        let result = if let Some(token) = cancel_token.as_ref() {
            tokio::select! {
                _ = token.cancelled() => {
                    let _ = child.kill().await;
                    return Err(StreamCancelledError.into());
                }
                timed = tokio::time::timeout(
                    std::time::Duration::from_secs(ONE_SHOT_TIMEOUT_SECS),
                    read_future
                ) => timed,
            }
        } else {
            tokio::time::timeout(
                std::time::Duration::from_secs(ONE_SHOT_TIMEOUT_SECS),
                read_future,
            )
            .await
        };

        // Handle timeout
        let (collected, stderr_lines, first_output_at) = match result {
            Ok(Ok(data)) => data,
            Ok(Err(e)) => {
                // Kill the process on IO error
                let _ = child.kill().await;
                let _ = self.event_tx.send(AgentEvent::Error {
                    thread_id: thread_id.to_string(),
                    message: format!("{}: {e}", self.agent_type),
                });
                return Err(e);
            }
            Err(_elapsed) => {
                let _ = child.kill().await;
                let msg = format!(
                    "{} timed out after {}s",
                    self.agent_type, ONE_SHOT_TIMEOUT_SECS
                );
                let _ = self.event_tx.send(AgentEvent::Error {
                    thread_id: thread_id.to_string(),
                    message: msg.clone(),
                });
                anyhow::bail!(msg);
            }
        };

        // Wait for process exit
        let status = if let Some(token) = cancel_token.as_ref() {
            tokio::select! {
                _ = token.cancelled() => {
                    let _ = child.kill().await;
                    return Err(StreamCancelledError.into());
                }
                status = child.wait() => status?,
            }
        } else {
            child.wait().await?
        };

        if !status.success() && collected.is_empty() {
            let error_msg = if stderr_lines.is_empty() {
                format!("{} exited with status {}", self.agent_type, status)
            } else {
                format!("{}: {}", self.agent_type, stderr_lines.join("\n"))
            };
            let _ = self.event_tx.send(AgentEvent::Error {
                thread_id: thread_id.to_string(),
                message: error_msg.clone(),
            });
            anyhow::bail!(error_msg);
        }

        let raw_output = if collected.is_empty() {
            if !stderr_lines.is_empty() {
                stderr_lines.join("\n")
            } else {
                String::new()
            }
        } else {
            collected.join("\n")
        };

        // Parse structured JSON output (OpenClaw --json) or use raw text
        let parsed = parse_structured_response(&self.agent_type, &raw_output);
        let generation_secs = first_output_at
            .unwrap_or(request_started_at)
            .elapsed()
            .as_secs_f64();
        let generation_ms = Some((generation_secs * 1000.0).round() as u64);
        let tps = if parsed.output_tokens > 0 && generation_secs > 0.0 {
            Some(parsed.output_tokens as f64 / generation_secs)
        } else {
            None
        };

        // For JSON-mode agents, emit the parsed text as a delta now
        if is_json_mode && !parsed.text.is_empty() {
            let _ = self.event_tx.send(AgentEvent::Delta {
                thread_id: thread_id.to_string(),
                content: parsed.text.clone(),
            });
        }

        // Emit done event with real token counts if available
        let _ = self.event_tx.send(AgentEvent::Done {
            thread_id: thread_id.to_string(),
            input_tokens: parsed.input_tokens,
            output_tokens: parsed.output_tokens,
            cost: None,
            provider: parsed.provider.or_else(|| Some(self.agent_type.clone())),
            model: parsed.model,
            tps,
            generation_ms,
        });

        tracing::info!(
            agent = %self.agent_type,
            response_len = parsed.text.len(),
            "external agent: message complete"
        );

        Ok(parsed.text)
    }

    /// Start the external agent in gateway mode (long-running process).
    ///
    /// The agent manages its own platform connections (Slack, Discord, Telegram)
    /// via its own gateway infrastructure.
    pub async fn start_gateway(&self) -> Result<()> {
        let exe = self
            .executable
            .as_deref()
            .context(format!("{} executable not found on PATH", self.agent_type))?;

        // Stop existing gateway process if any
        self.stop_gateway().await;

        let args = build_gateway_args(&self.agent_type);

        tracing::info!(
            agent = %self.agent_type,
            exe = %exe,
            "external agent: starting gateway mode"
        );

        let child = tokio::process::Command::new(exe)
            .args(&args)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .stdin(std::process::Stdio::null())
            .spawn()
            .context(format!(
                "failed to spawn {} gateway process",
                self.agent_type
            ))?;

        tracing::info!(
            agent = %self.agent_type,
            pid = ?child.id(),
            "external agent: gateway process started"
        );

        *self.gateway_process.lock().await = Some(child);
        Ok(())
    }

    /// Stop the external agent's gateway process.
    pub async fn stop_gateway(&self) {
        let mut proc = self.gateway_process.lock().await;
        if let Some(ref mut child) = *proc {
            tracing::info!(
                agent = %self.agent_type,
                "external agent: stopping gateway process"
            );
            let _ = child.kill().await;
        }
        *proc = None;
    }

    /// Stop all processes managed by this runner.
    pub async fn stop(&self) {
        self.stop_gateway().await;
    }
}

// ---------------------------------------------------------------------------
// Executable discovery
// ---------------------------------------------------------------------------

/// Find an agent executable on PATH.
fn find_executable(agent_type: &str) -> Option<String> {
    let name = match agent_type {
        "hermes" => "hermes",
        "openclaw" => "openclaw",
        _ => agent_type,
    };

    match which::which(name) {
        Ok(path) => {
            let path_str = path.to_string_lossy().to_string();
            tracing::info!(
                agent = %agent_type,
                path = %path_str,
                "external agent: executable found"
            );
            Some(path_str)
        }
        Err(_) => {
            tracing::debug!(
                agent = %agent_type,
                name = %name,
                "external agent: executable not found on PATH"
            );
            None
        }
    }
}

// ---------------------------------------------------------------------------
// Command construction
// ---------------------------------------------------------------------------

/// Build CLI args for one-shot agent execution.
fn build_one_shot_args(agent_type: &str, prompt: &str) -> Vec<String> {
    match agent_type {
        // `hermes chat -q "prompt" -Q` — `-Q` suppresses TUI output,
        // giving clean text-only responses.
        "hermes" => vec![
            "chat".to_string(),
            "-q".to_string(),
            prompt.to_string(),
            "-Q".to_string(),
        ],
        // `openclaw agent --agent main -m "prompt" --json`
        "openclaw" => vec![
            "agent".to_string(),
            "--agent".to_string(),
            "main".to_string(),
            "-m".to_string(),
            prompt.to_string(),
            "--json".to_string(),
        ],
        _ => vec!["--".to_string(), prompt.to_string()],
    }
}

/// Build CLI args for gateway (long-running) mode.
fn build_gateway_args(agent_type: &str) -> Vec<String> {
    match agent_type {
        "hermes" => vec!["gateway".to_string()],
        "openclaw" => vec!["gateway".to_string()],
        _ => vec!["gateway".to_string()],
    }
}

// ---------------------------------------------------------------------------
// Output cleaning
// ---------------------------------------------------------------------------

/// Strip ANSI escape sequences from a string.
fn strip_ansi(input: &str) -> String {
    // Matches: ESC[ ... final_byte, ESC] ... ST, and other common escapes
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // CSI sequence: ESC [ ... (ends at 0x40-0x7E)
            if chars.peek() == Some(&'[') {
                chars.next();
                while let Some(&next) = chars.peek() {
                    chars.next();
                    if ('\x40'..='\x7e').contains(&next) {
                        break;
                    }
                }
            // OSC sequence: ESC ] ... (ends at BEL or ST)
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
                // Other escape — skip the next char
                chars.next();
            }
        } else if c == '\r' {
            // Carriage return — spinners use \r to overwrite the line.
            // Discard everything accumulated so far so only the text
            // after the last \r survives (mimics terminal behavior).
            result.clear();
        } else {
            result.push(c);
        }
    }

    result
}

/// Detect TUI noise lines (banners, box borders, spinners, progress indicators).
fn is_tui_noise(line: &str) -> bool {
    let s = line.trim();

    // Empty after stripping
    if s.is_empty() {
        return true;
    }

    // Box-drawing characters (╭╮╰╯│─┌┐└┘├┤┬┴┼)
    if s.starts_with('╭')
        || s.starts_with('╰')
        || s.starts_with('│')
        || s.starts_with('┌')
        || s.starts_with('└')
    {
        return true;
    }
    // Lines that end with box chars
    if s.ends_with('╮')
        || s.ends_with('╯')
        || s.ends_with('│')
        || s.ends_with('┐')
        || s.ends_with('┘')
    {
        return true;
    }
    // Horizontal rules made of box-drawing
    if s.chars().all(|c| "─═╌╍┈┉━".contains(c) || c == ' ') && s.len() > 3 {
        return true;
    }

    // ASCII art banner (block characters ██╗)
    if s.contains("██") || s.contains("╗") || s.contains("╚") || s.contains("╔") {
        return true;
    }

    // Braille art (used for logos)
    if s.chars().any(|c| ('\u{2800}'..='\u{28FF}').contains(&c)) {
        return true;
    }

    // Spinner frames — Hermes uses many random verbs with "..." suffix
    // and emoticon faces like ( ͡° ͜ʖ ͡°), (⌐■_■), ( •_•), (¬_¬), (˘⌣˘)
    if s.contains("...)") && (s.contains("(") && s.contains("s)")) {
        // Matches patterns like: ✶ ( ͡° ͜ʖ ͡°) computing... (1.3s)
        return true;
    }
    // Explicit spinner verb matches as fallback
    if s.contains("processing...")
        || s.contains("mulling...")
        || s.contains("thinking...")
        || s.contains("pondering...")
        || s.contains("reasoning...")
        || s.contains("working...")
        || s.contains("computing...")
        || s.contains("deliberating...")
        || s.contains("analyzing...")
        || s.contains("formulating...")
    {
        return true;
    }

    // Hermes-specific decorations
    if s.starts_with("Query:")
        || s.contains("Hermes Agent v")
        || s.contains("Available Tools")
        || s.contains("Available Skills")
    {
        return true;
    }
    if s.contains("Session:") && s.len() < 60 {
        return true;
    }
    // Lines that are mostly tool/skill listings inside the banner box
    if (s.contains("browser_") || s.contains("file_tools:") || s.contains("code_execution:"))
        && !s.starts_with('{')
    {
        return true;
    }
    // "(and N more toolsets...)" or "N tools · N skills"
    if s.contains("more toolsets") || s.contains("tools ·") || s.contains("/help for") {
        return true;
    }
    // Provider line like "Qwen3.5... · Nous Research"
    if s.contains("· Nous") || s.contains("· OpenRouter") || s.contains("· OpenAI") {
        return true;
    }

    // Retry / error noise from the agent
    if s.contains("error, retrying") || s.contains("API retry") {
        return true;
    }
    // API error lines with emoji prefixes
    if s.starts_with("⚠️") || s.starts_with("❌") || s.starts_with("⏳") {
        return true;
    }
    // Indented error detail lines (⏱️, 📝, 📊)
    if s.starts_with("⏱️") || s.starts_with("📝") || s.starts_with("📊") {
        return true;
    }

    // Hermes role markers before response text — these are Hermes TUI
    // decorations, not the actual content. The format is exactly "> assistant"
    // or "> user" on a line by itself.
    if s == "> assistant" || s == "> user" {
        return true;
    }
    if s.starts_with("Resume this session with:") || s.starts_with("hermes --resume") {
        return true;
    }
    if s.starts_with("Duration:")
        || s.starts_with("Messages:")
        || s.starts_with("Session:")
        || s.starts_with("session_id:")
    {
        return true;
    }

    false
}

// ---------------------------------------------------------------------------
// Structured response parsing
// ---------------------------------------------------------------------------

struct ParsedResponse {
    text: String,
    input_tokens: u64,
    output_tokens: u64,
    provider: Option<String>,
    model: Option<String>,
}

/// Parse structured JSON output from external agents.
/// OpenClaw with `--json` returns a well-structured response with payloads and metadata.
/// Hermes returns plain text (already collected line by line).
fn parse_structured_response(agent_type: &str, raw: &str) -> ParsedResponse {
    let trimmed = raw.trim();

    // Try parsing as OpenClaw JSON response
    if agent_type == "openclaw" {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(trimmed) {
            // Extract text from result.payloads[].text
            let text = json
                .pointer("/result/payloads")
                .and_then(|p| p.as_array())
                .map(|payloads| {
                    payloads
                        .iter()
                        .filter_map(|p| p.get("text").and_then(|t| t.as_str()))
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .unwrap_or_default();

            // Extract token usage from result.meta.agentMeta.usage
            let usage = json.pointer("/result/meta/agentMeta/usage");
            let input_tokens = usage
                .and_then(|u| u.get("input"))
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let output_tokens = usage
                .and_then(|u| u.get("output"))
                .and_then(|v| v.as_u64())
                .unwrap_or(0);

            let provider = json
                .pointer("/result/meta/agentMeta/provider")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let model = json
                .pointer("/result/meta/agentMeta/model")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            if !text.is_empty() {
                return ParsedResponse {
                    text,
                    input_tokens,
                    output_tokens,
                    provider,
                    model,
                };
            }
        }
    }

    // Fallback: use raw text as-is
    ParsedResponse {
        text: trimmed.to_string(),
        input_tokens: 0,
        output_tokens: 0,
        provider: None,
        model: None,
    }
}

// ---------------------------------------------------------------------------
// MCP configuration detection & injection
// ---------------------------------------------------------------------------

/// Resolve the tamux-mcp binary path (next to daemon binary, or on PATH).
fn find_tamux_mcp_binary() -> Option<PathBuf> {
    // 1. Next to the current executable (co-located build)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join("tamux-mcp");
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }

    // 2. On PATH
    if let Ok(path) = which::which("tamux-mcp") {
        return Some(path);
    }

    None
}

/// Check if tamux-mcp is already configured in the agent's MCP settings.
fn check_tamux_mcp_configured(agent_type: &str) -> bool {
    match agent_type {
        "hermes" => check_hermes_mcp_config(),
        "openclaw" => check_openclaw_mcp_config(),
        _ => false,
    }
}

/// Check ~/.hermes/config.yaml for a tamux entry under mcp_servers.
fn check_hermes_mcp_config() -> bool {
    let config_path = dirs::home_dir()
        .map(|h| h.join(".hermes/config.yaml"))
        .unwrap_or_default();

    let content = match std::fs::read_to_string(&config_path) {
        Ok(c) => c,
        Err(_) => return false,
    };

    // Check if mcp_servers section has a tamux entry
    content.contains("mcp_servers:") && content.contains("tamux")
}

/// Check if tamux is configured in mcporter (used by OpenClaw).
fn check_openclaw_mcp_config() -> bool {
    // Check via mcporter CLI
    match std::process::Command::new("mcporter")
        .args(["config", "get", "tamux", "--json"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
    {
        Ok(status) => status.success(),
        Err(_) => false,
    }
}

/// Inject tamux-mcp into the agent's MCP config if not already present.
/// Returns true if injection was performed.
pub fn ensure_tamux_mcp_configured(agent_type: &str) -> bool {
    if check_tamux_mcp_configured(agent_type) {
        tracing::info!(agent = %agent_type, "tamux-mcp already configured");
        return false;
    }

    let mcp_binary = match find_tamux_mcp_binary() {
        Some(p) => p,
        None => {
            tracing::warn!(
                agent = %agent_type,
                "tamux-mcp binary not found — cannot auto-inject MCP config"
            );
            return false;
        }
    };

    let mcp_path = mcp_binary.to_string_lossy().to_string();

    match agent_type {
        "hermes" => inject_hermes_mcp_config(&mcp_path),
        "openclaw" => inject_openclaw_mcp_config(&mcp_path),
        _ => false,
    }
}

/// Inject tamux-mcp into ~/.hermes/config.yaml under mcp_servers.
fn inject_hermes_mcp_config(mcp_path: &str) -> bool {
    let config_path = match dirs::home_dir() {
        Some(h) => h.join(".hermes/config.yaml"),
        None => return false,
    };

    let content = match std::fs::read_to_string(&config_path) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(error = %e, "failed to read hermes config");
            return false;
        }
    };

    // Build the tamux MCP server entry
    let tamux_entry =
        format!("\nmcp_servers:\n  tamux:\n    command: \"{mcp_path}\"\n    args: []\n");

    let new_content = if content.contains("mcp_servers:") {
        // Insert tamux entry after the existing mcp_servers: line
        content.replacen(
            "mcp_servers:",
            &format!("mcp_servers:\n  tamux:\n    command: \"{mcp_path}\"\n    args: []"),
            1,
        )
    } else {
        // Append mcp_servers section at the end
        format!("{content}\n{tamux_entry}")
    };

    match std::fs::write(&config_path, &new_content) {
        Ok(()) => {
            tracing::info!(
                path = %config_path.display(),
                mcp_binary = %mcp_path,
                "injected tamux-mcp into hermes config"
            );
            true
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to write hermes config");
            false
        }
    }
}

/// Inject tamux-mcp into OpenClaw via `mcporter config add`.
fn inject_openclaw_mcp_config(mcp_path: &str) -> bool {
    match std::process::Command::new("mcporter")
        .args([
            "config",
            "add",
            "tamux",
            "--command",
            mcp_path,
            "--scope",
            "home",
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .status()
    {
        Ok(status) if status.success() => {
            tracing::info!(
                mcp_binary = %mcp_path,
                "injected tamux-mcp into mcporter config for openclaw"
            );
            true
        }
        Ok(status) => {
            tracing::warn!(
                exit = %status,
                "mcporter config add failed"
            );
            false
        }
        Err(e) => {
            tracing::warn!(
                error = %e,
                "mcporter not found — install mcporter for OpenClaw MCP integration"
            );
            false
        }
    }
}
