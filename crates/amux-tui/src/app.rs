//! TuiModel compositor -- delegates to decomposed state modules.
//!
//! This replaces the old monolithic 3,500-line app.rs with a clean
//! compositor that owns the 8 state sub-modules and bridges between
//! the daemon client events and the UI state.

use std::sync::mpsc::Receiver;

use crossterm::event::{KeyCode, KeyModifiers, MouseEvent, MouseEventKind, MouseButton};
use ratatui::prelude::*;
use ratatui::widgets::Clear;
use tokio::sync::mpsc::UnboundedSender;

use crate::client::ClientEvent;
use crate::providers;
use crate::state::*;
use crate::theme::ThemeTokens;
use crate::widgets;

// ── Public types ─────────────────────────────────────────────────────────────

/// A file attached to the next outgoing message.
#[derive(Debug, Clone)]
pub struct Attachment {
    pub filename: String,
    pub content: String,
    pub size_bytes: usize,
}

// ── Helper types ─────────────────────────────────────────────────────────────

/// Flat representation of a sidebar item for matching selected index to data.
struct SidebarFlatItem {
    #[allow(dead_code)]
    thread_id: Option<String>,
    goal_run_id: Option<String>,
    title: String,
}

// ── TuiModel ─────────────────────────────────────────────────────────────────

pub struct TuiModel {
    // State modules
    chat: chat::ChatState,
    input: input::InputState,
    modal: modal::ModalState,
    sidebar: sidebar::SidebarState,
    tasks: task::TaskState,
    config: config::ConfigState,
    approval: approval::ApprovalState,
    settings: settings::SettingsState,

    // UI chrome
    focus: FocusArea,
    theme: ThemeTokens,
    width: u16,
    height: u16,

    // Infrastructure
    daemon_cmd_tx: UnboundedSender<DaemonCommand>,
    daemon_events_rx: Receiver<ClientEvent>,

    // Status
    connected: bool,
    status_line: String,
    default_session_id: Option<String>,
    tick_counter: u64,

    // Error state
    last_error: Option<String>,     // stored for viewing via '!' key
    error_active: bool,             // active indicator (pulsing dot, clears on next action)
    error_tick: u64,                // tick when error occurred (for pulse animation)

    // Vim motion state
    pending_g: bool,

    // Responsive layout override: when Some, overrides breakpoint-based sidebar visibility
    show_sidebar_override: Option<bool>,

    // Set by /quit command; checked after modal enter to issue quit
    pending_quit: bool,

    // Double-Esc stream stop state
    pending_stop: bool,
    pending_stop_tick: u64,

    // Pending file attachments (prepended to next submitted message)
    attachments: Vec<Attachment>,

    // Queue of prompts submitted while streaming (auto-sent after TurnDone)
    queued_prompts: Vec<String>,

    // Thread ID whose stream was cancelled via double-Esc (ignore further events)
    cancelled_thread_id: Option<String>,
}

impl TuiModel {
    pub fn new(
        daemon_events_rx: Receiver<ClientEvent>,
        daemon_cmd_tx: UnboundedSender<DaemonCommand>,
    ) -> Self {
        Self {
            chat: chat::ChatState::new(),
            input: input::InputState::new(),
            modal: modal::ModalState::new(),
            sidebar: sidebar::SidebarState::new(),
            tasks: task::TaskState::new(),
            config: config::ConfigState::new(),
            approval: approval::ApprovalState::new(),
            settings: settings::SettingsState::new(),

            focus: FocusArea::Input,
            theme: ThemeTokens::default(),
            width: 120,
            height: 40,

            daemon_cmd_tx,
            daemon_events_rx,

            connected: false,
            status_line: "Starting...".to_string(),
            default_session_id: None,
            tick_counter: 0,
            last_error: None,
            error_active: false,
            error_tick: 0,

            pending_g: false,
            show_sidebar_override: None,
            pending_quit: false,
            pending_stop: false,
            pending_stop_tick: 0,

            attachments: Vec::new(),
            queued_prompts: Vec::new(),
            cancelled_thread_id: None,
        }
    }

    fn send_daemon_command(&self, command: DaemonCommand) {
        let _ = self.daemon_cmd_tx.send(command);
    }

    /// Calculate the dynamic input area height based on buffer content and attachments.
    /// Grows from 3 (border + 1 line + border) to a max of 12 rows.
    fn input_height(&self) -> u16 {
        use unicode_width::UnicodeWidthStr;
        // The Paragraph with .wrap() wraps at the inner area width.
        // Inner area = block inner = total_width - 2 (borders).
        // But all our Line spans start with " ▶ " (first line) or "   " (continuations)
        // which take display columns. Ratatui counts the full span widths.
        // The key insight: Paragraph::wrap wraps at `area.width` columns total,
        // INCLUDING the prompt spans. So effective text width = area.width - prompt_width.
        //
        // Block::inner with Borders::ALL subtracts 2 (left+right border).
        // So inner.width = self.width - 2.
        // Prompt " ▶ " = 1 + 2 + 1 = 4 display columns (▶ is 2-wide).
        let inner_w = self.width.saturating_sub(2) as usize;
        let prompt_w = 4; // " ▶ " display width
        let text_w = inner_w.saturating_sub(prompt_w);
        if text_w <= 2 {
            return 3;
        }
        let visual_lines: usize = self.input.buffer().split('\n')
            .map(|line| {
                let display_width = UnicodeWidthStr::width(line) + 1; // +1 for cursor
                if text_w == 0 { 1 } else { (display_width + text_w - 1) / text_w }
            })
            .sum();
        let attach_count = self.attachments.len();
        (visual_lines + attach_count + 2).clamp(3, 12) as u16
    }

    /// Handle pasted text (from bracketed paste). Inserts all characters
    /// including newlines into the input buffer without triggering submit.
    /// If the pasted text looks like a single file path that exists on disk,
    /// auto-attaches the file instead of inserting the path as text.
    pub fn handle_paste(&mut self, text: String) {
        // Ensure focus is on input when pasting
        if self.focus != FocusArea::Input {
            self.focus = FocusArea::Input;
            self.input.set_mode(input::InputMode::Insert);
        }

        let trimmed = text.trim();

        // Check if pasted text is a single file path
        if !trimmed.contains('\n') && (
            trimmed.starts_with('/') ||
            trimmed.starts_with('~') ||
            trimmed.starts_with("C:\\") ||
            trimmed.starts_with("D:\\")
        ) {
            let expanded = if trimmed.starts_with('~') {
                let home = std::env::var("HOME")
                    .or_else(|_| std::env::var("USERPROFILE"))
                    .unwrap_or_default();
                trimmed.replacen('~', &home, 1)
            } else {
                trimmed.to_string()
            };

            if std::path::Path::new(&expanded).is_file() {
                // It's a real file path — auto-attach it
                self.attach_file(trimmed);
                return;
            }
        }

        // Multi-line paste: create a collapsed paste block preview
        if text.contains('\n') {
            self.input.insert_paste_block(text);
        } else {
            // Single-line: insert normally
            for c in text.chars() {
                self.input.reduce(input::InputAction::InsertChar(c));
            }
        }
    }

    /// Read a file at the given path (with ~ expansion) and add it to the pending attachments.
    fn attach_file(&mut self, path: &str) {
        let expanded = if path.starts_with('~') {
            let home = std::env::var("HOME")
                .or_else(|_| std::env::var("USERPROFILE"))
                .unwrap_or_default();
            path.replacen('~', &home, 1)
        } else {
            path.to_string()
        };

        match std::fs::read_to_string(&expanded) {
            Ok(content) => {
                let size = content.len();
                let filename = std::path::Path::new(&expanded)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| expanded.clone());
                self.attachments.push(Attachment {
                    filename: filename.clone(),
                    content,
                    size_bytes: size,
                });
                self.status_line = format!("Attached: {} ({} bytes)", filename, size);
            }
            Err(e) => {
                self.status_line = format!("Failed to attach '{}': {}", path, e);
                self.last_error = Some(format!("Attach failed: {}", e));
                self.error_active = true;
                self.error_tick = self.tick_counter;
            }
        }
    }

    /// Push the current config state to the daemon via SetConfigJson.
    /// TUI explicitly disables frontend-only tools (workspace, terminal management,
    /// managed commands) since those require the Electron GUI.
    fn sync_config_to_daemon(&self) {
        if let Ok(json) = serde_json::to_string(&serde_json::json!({
            "provider": &self.config.provider,
            "base_url": &self.config.base_url,
            "api_key": &self.config.api_key,
            "model": &self.config.model,
            "reasoning_effort": &self.config.reasoning_effort,
            "tools": {
                "bash": self.config.tool_bash,
                "file_operations": self.config.tool_file_ops,
                "web_search": self.config.tool_web_search,
                "web_browse": self.config.tool_web_browse,
                "vision": self.config.tool_vision,
                "system_info": self.config.tool_system_info,
                "gateway_messaging": self.config.tool_gateway,
                // TUI-specific: always disable frontend-only tools
                "workspace": false,
                "terminal_management": false,
                "managed_commands": false,
            },
            "search_provider": &self.config.search_provider,
            "firecrawl_api_key": &self.config.firecrawl_api_key,
            "exa_api_key": &self.config.exa_api_key,
            "tavily_api_key": &self.config.tavily_api_key,
            "search_max_results": self.config.search_max_results,
            "search_timeout_secs": self.config.search_timeout_secs,
            "enable_streaming": self.config.enable_streaming,
            "max_tool_loops": self.config.max_tool_loops,
            "max_retries": self.config.max_retries,
            "retry_delay_ms": self.config.retry_delay_ms,
            "context_budget_tokens": self.config.context_budget_tokens,
            "gateway": {
                "enabled": self.config.gateway_enabled,
                "command_prefix": &self.config.gateway_prefix,
                "slack_token": &self.config.slack_token,
                "slack_channel_filter": &self.config.slack_channel_filter,
                "telegram_token": &self.config.telegram_token,
                "telegram_allowed_chats": &self.config.telegram_allowed_chats,
                "discord_token": &self.config.discord_token,
                "discord_channel_filter": &self.config.discord_channel_filter,
                "discord_allowed_users": &self.config.discord_allowed_users,
                "whatsapp_allowed_contacts": &self.config.whatsapp_allowed_contacts,
                "whatsapp_token": &self.config.whatsapp_token,
                "whatsapp_phone_id": &self.config.whatsapp_phone_id,
            },
        })) {
            self.send_daemon_command(DaemonCommand::SetConfigJson(json));
        }
        self.save_settings();
    }

    /// Load saved settings from ~/.tamux/agent-settings.json on startup.
    pub fn load_saved_settings(&mut self) {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .unwrap_or_default();
        let path = format!("{}/.tamux/agent-settings.json", home);
        tracing::info!("Loading settings from: {}", path);
        let Ok(data) = std::fs::read_to_string(&path) else {
            return;
        };
        let Ok(json): Result<serde_json::Value, _> = serde_json::from_str(&data) else {
            return;
        };

        // Get active provider
        let provider_id = json
            .get("activeProvider")
            .and_then(|v| v.as_str())
            .unwrap_or("openai");

        // Get provider-specific config
        if let Some(provider_config) = json.get(provider_id) {
            let base_url = provider_config
                .get("baseUrl")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let model = provider_config
                .get("model")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let api_key = provider_config
                .get("apiKey")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            self.config.provider = provider_id.to_string();
            if !base_url.is_empty() {
                self.config.base_url = base_url.to_string();
            } else if let Some(def) = providers::find_by_id(provider_id) {
                self.config.base_url = def.default_base_url.to_string();
            }
            if !model.is_empty() {
                self.config.model = model.to_string();
            }
            if !api_key.is_empty() {
                self.config.api_key = api_key.to_string();
            }
        }

        // Load tool toggles from agent settings
        let get_bool = |key: &str| json.get(key).and_then(|v| v.as_bool()).unwrap_or(false);
        let get_str = |key: &str| json.get(key).and_then(|v| v.as_str()).unwrap_or("").to_string();

        self.config.tool_bash = json.get("enableBashTool").and_then(|v| v.as_bool()).unwrap_or(true);
        self.config.tool_web_search = get_bool("enableWebSearchTool");
        self.config.tool_web_browse = get_bool("enableWebBrowsingTool");
        self.config.tool_vision = get_bool("enableVisionTool");

        // Reasoning effort
        if let Some(effort) = json.get("reasoningEffort").and_then(|v| v.as_str()) {
            self.config.reasoning_effort = effort.to_string();
        }

        // Web search provider + keys
        self.config.search_provider = get_str("searchToolProvider");
        self.config.firecrawl_api_key = get_str("firecrawlApiKey");
        self.config.exa_api_key = get_str("exaApiKey");
        self.config.tavily_api_key = get_str("tavilyApiKey");
        self.config.search_max_results = json.get("searchMaxResults").and_then(|v| v.as_u64()).unwrap_or(8) as u32;
        self.config.search_timeout_secs = json.get("searchTimeoutSeconds").and_then(|v| v.as_u64()).unwrap_or(20) as u32;

        // Chat settings
        self.config.enable_streaming = json.get("enableStreaming").and_then(|v| v.as_bool()).unwrap_or(true);
        self.config.enable_conversation_memory = json.get("enableConversationMemory").and_then(|v| v.as_bool()).unwrap_or(true);
        self.config.enable_honcho_memory = get_bool("enableHonchoMemory");
        self.config.honcho_api_key = get_str("honchoApiKey");
        self.config.honcho_base_url = get_str("honchoBaseUrl");
        self.config.honcho_workspace_id = {
            let ws = get_str("honchoWorkspaceId");
            if ws.is_empty() { "tamux".to_string() } else { ws }
        };

        // Advanced settings
        self.config.auto_compact_context = json.get("autoCompactContext").and_then(|v| v.as_bool()).unwrap_or(true);
        self.config.max_context_messages = json.get("maxContextMessages").and_then(|v| v.as_u64()).unwrap_or(100) as u32;
        self.config.max_tool_loops = json.get("maxToolLoops").and_then(|v| v.as_u64()).unwrap_or(25) as u32;
        self.config.max_retries = json.get("maxRetries").and_then(|v| v.as_u64()).unwrap_or(3) as u32;
        self.config.retry_delay_ms = json.get("retryDelayMs").and_then(|v| v.as_u64()).unwrap_or(2000) as u32;
        self.config.context_budget_tokens = json.get("contextBudgetTokens").and_then(|v| v.as_u64()).unwrap_or(100000) as u32;
        self.config.compact_threshold_pct = json.get("compactThresholdPercent").and_then(|v| v.as_u64()).unwrap_or(80) as u32;
        self.config.keep_recent_on_compact = json.get("keepRecentOnCompaction").and_then(|v| v.as_u64()).unwrap_or(10) as u32;
        self.config.bash_timeout_secs = json.get("bashTimeoutSeconds").and_then(|v| v.as_u64()).unwrap_or(30) as u32;

        // Gateway config (from settings.json — different file)
        let settings_path = format!("{}/.tamux/settings.json", home);
        if let Ok(settings_data) = std::fs::read_to_string(&settings_path) {
            if let Ok(settings_json) = serde_json::from_str::<serde_json::Value>(&settings_data) {
                self.config.gateway_enabled = settings_json.get("gatewayEnabled").and_then(|v| v.as_bool()).unwrap_or(false);
                self.config.gateway_prefix = settings_json.get("gatewayCommandPrefix").and_then(|v| v.as_str()).unwrap_or("!tamux").to_string();
                self.config.slack_token = settings_json.get("slackToken").and_then(|v| v.as_str()).unwrap_or("").to_string();
                self.config.slack_channel_filter = settings_json.get("slackChannelFilter").and_then(|v| v.as_str()).unwrap_or("").to_string();
                self.config.telegram_token = settings_json.get("telegramToken").and_then(|v| v.as_str()).unwrap_or("").to_string();
                self.config.telegram_allowed_chats = settings_json.get("telegramAllowedChats").and_then(|v| v.as_str()).unwrap_or("").to_string();
                self.config.discord_token = settings_json.get("discordToken").and_then(|v| v.as_str()).unwrap_or("").to_string();
                self.config.discord_channel_filter = settings_json.get("discordChannelFilter").and_then(|v| v.as_str()).unwrap_or("").to_string();
                self.config.discord_allowed_users = settings_json.get("discordAllowedUsers").and_then(|v| v.as_str()).unwrap_or("").to_string();
                self.config.whatsapp_allowed_contacts = settings_json.get("whatsappAllowedContacts").and_then(|v| v.as_str()).unwrap_or("").to_string();
                self.config.whatsapp_token = settings_json.get("whatsappToken").and_then(|v| v.as_str()).unwrap_or("").to_string();
                self.config.whatsapp_phone_id = settings_json.get("whatsappPhoneNumberId").and_then(|v| v.as_str()).unwrap_or("").to_string();
                if self.config.gateway_enabled {
                    self.config.tool_gateway = true;
                }
            }
        }

        // Store the full JSON for per-provider API keys
        self.config.agent_config_raw = Some(json);

        self.status_line = format!(
            "Loaded settings: {} / {}",
            self.config.provider, self.config.model
        );
    }

    /// Persist current settings to ~/.tamux/agent-settings.json (and gateway to settings.json).
    /// Reads the existing file first to preserve fields we don't manage.
    fn save_settings(&self) {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .unwrap_or_default();
        if home.is_empty() {
            return;
        }

        // Ensure ~/.tamux/ directory exists
        let dir = format!("{}/.tamux", home);
        let _ = std::fs::create_dir_all(&dir);

        let path = format!("{}/.tamux/agent-settings.json", home);

        // Read existing file to preserve fields we don't manage
        let mut json: serde_json::Value = if let Ok(data) = std::fs::read_to_string(&path) {
            serde_json::from_str(&data).unwrap_or_else(|_| serde_json::json!({}))
        } else {
            serde_json::json!({})
        };

        // Update our managed top-level fields
        json["activeProvider"] = serde_json::Value::String(self.config.provider.clone());
        json["reasoningEffort"] = serde_json::Value::String(self.config.reasoning_effort.clone());
        json["enableBashTool"] = serde_json::Value::Bool(self.config.tool_bash);
        json["enableWebSearchTool"] = serde_json::Value::Bool(self.config.tool_web_search);
        json["enableWebBrowsingTool"] = serde_json::Value::Bool(self.config.tool_web_browse);
        json["enableVisionTool"] = serde_json::Value::Bool(self.config.tool_vision);
        json["searchToolProvider"] = serde_json::Value::String(self.config.search_provider.clone());
        json["firecrawlApiKey"] = serde_json::Value::String(self.config.firecrawl_api_key.clone());
        json["exaApiKey"] = serde_json::Value::String(self.config.exa_api_key.clone());
        json["tavilyApiKey"] = serde_json::Value::String(self.config.tavily_api_key.clone());
        json["searchMaxResults"] = serde_json::Value::Number(self.config.search_max_results.into());
        json["searchTimeoutSeconds"] = serde_json::Value::Number(self.config.search_timeout_secs.into());

        // Chat settings
        json["enableStreaming"] = serde_json::Value::Bool(self.config.enable_streaming);
        json["enableConversationMemory"] = serde_json::Value::Bool(self.config.enable_conversation_memory);
        json["enableHonchoMemory"] = serde_json::Value::Bool(self.config.enable_honcho_memory);
        json["honchoApiKey"] = serde_json::Value::String(self.config.honcho_api_key.clone());
        json["honchoBaseUrl"] = serde_json::Value::String(self.config.honcho_base_url.clone());
        json["honchoWorkspaceId"] = serde_json::Value::String(self.config.honcho_workspace_id.clone());

        // Advanced settings
        json["autoCompactContext"] = serde_json::Value::Bool(self.config.auto_compact_context);
        json["maxContextMessages"] = serde_json::Value::Number(self.config.max_context_messages.into());
        json["maxToolLoops"] = serde_json::Value::Number(self.config.max_tool_loops.into());
        json["maxRetries"] = serde_json::Value::Number(self.config.max_retries.into());
        json["retryDelayMs"] = serde_json::Value::Number(self.config.retry_delay_ms.into());
        json["contextBudgetTokens"] = serde_json::Value::Number(self.config.context_budget_tokens.into());
        json["compactThresholdPercent"] = serde_json::Value::Number(self.config.compact_threshold_pct.into());
        json["keepRecentOnCompaction"] = serde_json::Value::Number(self.config.keep_recent_on_compact.into());
        json["bashTimeoutSeconds"] = serde_json::Value::Number(self.config.bash_timeout_secs.into());

        // Update per-provider config block for the active provider
        let provider_config = serde_json::json!({
            "baseUrl": &self.config.base_url,
            "model": &self.config.model,
            "apiKey": &self.config.api_key,
        });
        json[&self.config.provider] = provider_config;

        if let Ok(data) = serde_json::to_string_pretty(&json) {
            if let Err(e) = std::fs::write(&path, data) {
                tracing::warn!("Failed to write agent-settings.json: {}", e);
            }
        }

        // Save gateway config to settings.json
        let settings_path = format!("{}/.tamux/settings.json", home);
        let mut settings_json: serde_json::Value =
            if let Ok(data) = std::fs::read_to_string(&settings_path) {
                serde_json::from_str(&data).unwrap_or_else(|_| serde_json::json!({}))
            } else {
                serde_json::json!({})
            };
        settings_json["gatewayEnabled"] = serde_json::Value::Bool(self.config.gateway_enabled);
        settings_json["gatewayCommandPrefix"] = serde_json::Value::String(self.config.gateway_prefix.clone());
        settings_json["slackToken"] = serde_json::Value::String(self.config.slack_token.clone());
        settings_json["slackChannelFilter"] = serde_json::Value::String(self.config.slack_channel_filter.clone());
        settings_json["telegramToken"] = serde_json::Value::String(self.config.telegram_token.clone());
        settings_json["telegramAllowedChats"] = serde_json::Value::String(self.config.telegram_allowed_chats.clone());
        settings_json["discordToken"] = serde_json::Value::String(self.config.discord_token.clone());
        settings_json["discordChannelFilter"] = serde_json::Value::String(self.config.discord_channel_filter.clone());
        settings_json["discordAllowedUsers"] = serde_json::Value::String(self.config.discord_allowed_users.clone());
        settings_json["whatsappAllowedContacts"] = serde_json::Value::String(self.config.whatsapp_allowed_contacts.clone());
        settings_json["whatsappToken"] = serde_json::Value::String(self.config.whatsapp_token.clone());
        settings_json["whatsappPhoneNumberId"] = serde_json::Value::String(self.config.whatsapp_phone_id.clone());
        if let Ok(data) = serde_json::to_string_pretty(&settings_json) {
            if let Err(e) = std::fs::write(&settings_path, data) {
                tracing::warn!("Failed to write settings.json: {}", e);
            }
        }
    }

    // ── Rendering ─────────────────────────────────────────────────────────────

    pub fn render(&self, frame: &mut Frame) {
        let area = frame.area();
        let w = area.width;

        // Layout: header (3) + body (flex) + input (dynamic 3-12) + status bar (1)
        // Use the actual frame width for accurate height calculation
        let saved_width = self.width;
        // SAFETY: we're in render (immutable), but input_height reads self.width
        // Just use frame width directly via a local calculation
        let input_height = {
            use unicode_width::UnicodeWidthStr;
            let inner_w = w.saturating_sub(2) as usize;
            let prompt_w = 4;
            let text_w = inner_w.saturating_sub(prompt_w);
            if text_w <= 2 {
                3u16
            } else {
                let visual_lines: usize = self.input.buffer().split('\n')
                    .map(|line| {
                        let dw = UnicodeWidthStr::width(line) + 1;
                        if text_w == 0 { 1 } else { (dw + text_w - 1) / text_w }
                    })
                    .sum();
                let attach_count = self.attachments.len();
                (visual_lines + attach_count + 2).clamp(3, 12) as u16
            }
        };
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),            // header
                Constraint::Min(1),              // body
                Constraint::Length(input_height), // input (bordered, grows with content)
                Constraint::Length(1),            // status bar (bare, below input)
            ])
            .split(area);

        // Render header
        widgets::header::render(frame, chunks[0], &self.config, &self.chat, &self.theme);

        // Render body (two-pane or single)
        let default_show_sidebar = w >= 80;
        let show_sidebar = self.show_sidebar_override.unwrap_or(default_show_sidebar);

        if show_sidebar {
            let sidebar_pct = if w >= 120 { 33 } else { 28 };
            let body_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(100 - sidebar_pct),
                    Constraint::Percentage(sidebar_pct),
                ])
                .split(chunks[1]);
            widgets::chat::render(
                frame,
                body_chunks[0],
                &self.chat,
                &self.theme,
                self.focus == FocusArea::Chat,
            );
            widgets::sidebar::render(
                frame,
                body_chunks[1],
                &self.sidebar,
                &self.tasks,
                &self.theme,
                self.focus == FocusArea::Sidebar,
            );
        } else {
            widgets::chat::render(
                frame,
                chunks[1],
                &self.chat,
                &self.theme,
                self.focus == FocusArea::Chat,
            );
        }

        // Render input box (bordered) — dim when modal is open
        widgets::footer::render_input(
            frame,
            chunks[2],
            &self.input,
            &self.theme,
            self.focus == FocusArea::Input,
            self.modal.top().is_some(),
            &self.attachments,
            self.tick_counter,
        );

        // Render status bar (bare, below input)
        widgets::footer::render_status_bar(
            frame,
            chunks[3],
            &self.theme,
            self.connected,
            self.error_active,
            self.tick_counter,
            self.error_tick,
            self.queued_prompts.len(),
        );

        // Modal overlay
        if let Some(modal_kind) = self.modal.top() {
            let overlay_area = match modal_kind {
                modal::ModalKind::Settings => centered_rect(75, 80, area),
                modal::ModalKind::ApprovalOverlay => centered_rect(60, 40, area),
                modal::ModalKind::CommandPalette => centered_rect(50, 40, area),
                modal::ModalKind::ThreadPicker => centered_rect(60, 50, area),
                modal::ModalKind::ProviderPicker => centered_rect(35, 65, area),
                modal::ModalKind::ModelPicker => centered_rect(45, 50, area),
                modal::ModalKind::EffortPicker => centered_rect(35, 30, area),
                modal::ModalKind::ToolsPicker | modal::ModalKind::ViewPicker => {
                    centered_rect(40, 35, area)
                }
                modal::ModalKind::Help => centered_rect(70, 80, area),
            };
            frame.render_widget(Clear, overlay_area);

            match modal_kind {
                modal::ModalKind::CommandPalette => {
                    widgets::command_palette::render(
                        frame,
                        overlay_area,
                        &self.modal,
                        &self.theme,
                    );
                }
                modal::ModalKind::ThreadPicker => {
                    widgets::thread_picker::render(
                        frame,
                        overlay_area,
                        &self.chat,
                        &self.modal,
                        &self.theme,
                    );
                }
                modal::ModalKind::ApprovalOverlay => {
                    widgets::approval::render(
                        frame,
                        overlay_area,
                        &self.approval,
                        &self.theme,
                    );
                }
                modal::ModalKind::Settings => {
                    widgets::settings::render(
                        frame,
                        overlay_area,
                        &self.settings,
                        &self.config,
                        &self.theme,
                    );
                }
                modal::ModalKind::ProviderPicker => {
                    widgets::provider_picker::render(
                        frame,
                        overlay_area,
                        &self.modal,
                        &self.config,
                        &self.theme,
                    );
                }
                modal::ModalKind::ModelPicker => {
                    widgets::model_picker::render(
                        frame,
                        overlay_area,
                        &self.modal,
                        &self.config,
                        &self.theme,
                    );
                }
                modal::ModalKind::EffortPicker => {
                    render_effort_picker(
                        frame,
                        overlay_area,
                        &self.modal,
                        &self.config,
                        &self.theme,
                    );
                }
                modal::ModalKind::ToolsPicker | modal::ModalKind::ViewPicker => {
                    // Not yet implemented -- just render the area as empty
                }
                modal::ModalKind::Help => {
                    render_help_modal(frame, overlay_area, &self.theme);
                }
            }
        }
    }

    // ── Daemon event pump ────────────────────────────────────────────────────

    pub fn pump_daemon_events(&mut self) {
        while let Ok(event) = self.daemon_events_rx.try_recv() {
            self.handle_client_event(event);
        }
    }

    fn handle_client_event(&mut self, event: ClientEvent) {
        // Skip streaming events for a cancelled thread (double-Esc stop)
        if let Some(ref cancelled_id) = self.cancelled_thread_id.clone() {
            let skip = match &event {
                ClientEvent::Delta { thread_id, .. }
                | ClientEvent::Reasoning { thread_id, .. }
                | ClientEvent::ToolCall { thread_id, .. }
                | ClientEvent::ToolResult { thread_id, .. } => thread_id == cancelled_id,
                ClientEvent::Done { thread_id, .. } => {
                    if thread_id == cancelled_id {
                        self.cancelled_thread_id = None;
                        true
                    } else {
                        false
                    }
                }
                _ => false,
            };
            if skip {
                return;
            }
        }

        match event {
            ClientEvent::Connected => {
                self.connected = true;
                self.status_line = "Connected to daemon".to_string();
                // Sync our config (provider, model, api_key, reasoning_effort) to daemon
                self.sync_config_to_daemon();
                self.send_daemon_command(DaemonCommand::Refresh);
                self.send_daemon_command(DaemonCommand::RefreshServices);
                // Auto-spawn terminal session
                let cwd = std::env::current_dir()
                    .ok()
                    .map(|p| p.to_string_lossy().to_string());
                let shell = std::env::var("SHELL").ok();
                self.send_daemon_command(DaemonCommand::SpawnSession {
                    shell,
                    cwd,
                    cols: self.width.max(80),
                    rows: self.height.max(24),
                });
            }
            ClientEvent::Disconnected => {
                self.connected = false;
                self.default_session_id = None;
                self.status_line = "Disconnected from daemon".to_string();
            }
            ClientEvent::SessionSpawned { session_id } => {
                self.default_session_id = Some(session_id.clone());
                self.status_line = format!("Session: {}", session_id);
            }
            ClientEvent::ThreadList(threads) => {
                let threads = threads.into_iter().map(convert_thread).collect();
                self.chat
                    .reduce(chat::ChatAction::ThreadListReceived(threads));
            }
            ClientEvent::ThreadDetail(Some(thread)) => {
                self.chat
                    .reduce(chat::ChatAction::ThreadDetailReceived(convert_thread(
                        thread,
                    )));
            }
            ClientEvent::ThreadDetail(None) => {}
            ClientEvent::ThreadCreated { thread_id, title } => {
                self.chat
                    .reduce(chat::ChatAction::ThreadCreated { thread_id, title });
            }
            ClientEvent::TaskList(tasks) => {
                let tasks = tasks.into_iter().map(convert_task).collect();
                self.tasks.reduce(task::TaskAction::TaskListReceived(tasks));
            }
            ClientEvent::TaskUpdate(t) => {
                self.tasks
                    .reduce(task::TaskAction::TaskUpdate(convert_task(t)));
            }
            ClientEvent::GoalRunList(runs) => {
                let runs = runs.into_iter().map(convert_goal_run).collect();
                self.tasks
                    .reduce(task::TaskAction::GoalRunListReceived(runs));
            }
            ClientEvent::GoalRunDetail(Some(run)) => {
                self.tasks
                    .reduce(task::TaskAction::GoalRunDetailReceived(convert_goal_run(
                        run,
                    )));
            }
            ClientEvent::GoalRunDetail(None) => {}
            ClientEvent::GoalRunUpdate(run) => {
                self.tasks
                    .reduce(task::TaskAction::GoalRunUpdate(convert_goal_run(run)));
            }
            ClientEvent::AgentConfig(cfg) => {
                self.config
                    .reduce(config::ConfigAction::ConfigReceived(
                        config::AgentConfigSnapshot {
                            provider: cfg.provider,
                            base_url: cfg.base_url,
                            model: cfg.model,
                            api_key: cfg.api_key,
                            reasoning_effort: cfg.reasoning_effort,
                        },
                    ));
            }
            ClientEvent::AgentConfigRaw(raw) => {
                self.config
                    .reduce(config::ConfigAction::ConfigRawReceived(raw));
            }
            ClientEvent::ModelsFetched(models) => {
                let models = models
                    .into_iter()
                    .map(|m| config::FetchedModel {
                        id: m.id,
                        name: m.name,
                        context_window: m.context_window,
                    })
                    .collect();
                self.config
                    .reduce(config::ConfigAction::ModelsFetched(models));
            }
            ClientEvent::HeartbeatItems(items) => {
                let items = items.into_iter().map(convert_heartbeat).collect();
                self.tasks
                    .reduce(task::TaskAction::HeartbeatItemsReceived(items));
            }
            ClientEvent::Delta {
                thread_id,
                content,
            } => {
                self.chat
                    .reduce(chat::ChatAction::Delta { thread_id, content });
            }
            ClientEvent::Reasoning {
                thread_id,
                content,
            } => {
                self.chat
                    .reduce(chat::ChatAction::Reasoning { thread_id, content });
            }
            ClientEvent::ToolCall {
                thread_id,
                call_id,
                name,
                arguments,
            } => {
                self.chat.reduce(chat::ChatAction::ToolCall {
                    thread_id,
                    call_id,
                    name,
                    args: arguments,
                });
            }
            ClientEvent::ToolResult {
                thread_id,
                call_id,
                name,
                content,
                is_error,
            } => {
                self.chat.reduce(chat::ChatAction::ToolResult {
                    thread_id,
                    call_id,
                    name,
                    content,
                    is_error,
                });
            }
            ClientEvent::Done {
                thread_id,
                input_tokens,
                output_tokens,
                cost,
                provider,
                model,
                tps,
                generation_ms,
            } => {
                self.chat.reduce(chat::ChatAction::TurnDone {
                    thread_id,
                    input_tokens,
                    output_tokens,
                    cost,
                    provider,
                    model,
                    tps,
                    generation_ms,
                });

                // Send next queued prompt if any
                if !self.queued_prompts.is_empty() {
                    let next_prompt = self.queued_prompts.remove(0);
                    self.submit_prompt(next_prompt);
                }
            }
            ClientEvent::Error(message) => {
                self.last_error = Some(message.clone());
                self.error_active = true;
                self.error_tick = self.tick_counter;
                self.status_line = format!("Error: {}", message);
                // Also show error in chat as a system message
                if let Some(thread) = self.chat.active_thread_mut() {
                    thread.messages.push(chat::AgentMessage {
                        role: chat::MessageRole::System,
                        content: format!("Error: {}", message),
                        ..Default::default()
                    });
                }
            }
        }
    }

    // ── Key handling ─────────────────────────────────────────────────────────

    /// Returns true if the app should quit.
    /// The input is always in "insert" mode -- there is no Normal/Insert concept exposed to
    /// the user.  Navigation keys (j/k/G/gg/Ctrl-D/U) only apply when focus is Chat or
    /// Sidebar; when focus is Input all printable characters go to the input buffer.
    pub fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers) -> bool {
        // Modal takes priority
        if let Some(modal_kind) = self.modal.top() {
            return self.handle_key_modal(code, modifiers, modal_kind);
        }

        let ctrl = modifiers.contains(KeyModifiers::CONTROL);


        // Clear pending_stop on any non-Esc key
        if code != KeyCode::Esc {
            self.pending_stop = false;
        }

        match code {
            // ── Global Ctrl shortcuts (work regardless of focus) ──────────────
            KeyCode::Char('p') if ctrl => {
                self.modal
                    .reduce(modal::ModalAction::Push(modal::ModalKind::CommandPalette));
            }
            KeyCode::Char('t') if ctrl => {
                self.modal
                    .reduce(modal::ModalAction::Push(modal::ModalKind::ThreadPicker));
            }
            KeyCode::Char('b') if ctrl => {
                let current = self.show_sidebar_override.unwrap_or(self.width >= 80);
                self.show_sidebar_override = Some(!current);
            }
            KeyCode::Char('d') if ctrl => {
                let half_page = (self.height / 2) as i32;
                self.chat.reduce(chat::ChatAction::ScrollChat(-half_page));
            }
            KeyCode::Char('u') if ctrl => {
                if self.focus == FocusArea::Input {
                    // Clear entire input line
                    self.input.reduce(input::InputAction::ClearLine);
                } else {
                    let half_page = (self.height / 2) as i32;
                    self.chat.reduce(chat::ChatAction::ScrollChat(half_page));
                }
            }
            KeyCode::PageDown if self.focus == FocusArea::Chat => {
                let half_page = (self.height / 2) as i32;
                self.chat.reduce(chat::ChatAction::ScrollChat(-half_page));
            }
            KeyCode::PageUp if self.focus == FocusArea::Chat => {
                let half_page = (self.height / 2) as i32;
                self.chat.reduce(chat::ChatAction::ScrollChat(half_page));
            }

            // ── Esc: close modal > stop stream (double) > move focus to Chat ──
            KeyCode::Esc => {
                if self.chat.is_streaming() {
                    if self.pending_stop
                        && (self.tick_counter.saturating_sub(self.pending_stop_tick)) < 40
                    {
                        // Double-Esc within ~2 s: force stop + ignore further daemon events
                        self.cancelled_thread_id = self.chat.active_thread_id().map(String::from);
                        self.chat.reduce(chat::ChatAction::ForceStopStreaming);
                        self.status_line = "Stream stopped".to_string();
                        self.pending_stop = false;
                    } else {
                        self.pending_stop = true;
                        self.pending_stop_tick = self.tick_counter;
                        self.status_line = "Press Esc again to stop stream".to_string();
                    }
                } else {
                    // Not streaming: clear message selection or move focus to Chat
                    self.pending_stop = false;
                    if self.focus == FocusArea::Chat && self.chat.selected_message().is_some() {
                        self.chat.select_message(None);
                        // Return to tail-following: reset scroll to 0
                        let current_scroll = self.chat.scroll_offset() as i32;
                        if current_scroll > 0 {
                            self.chat.reduce(chat::ChatAction::ScrollChat(-current_scroll));
                        }
                    } else if self.focus == FocusArea::Input {
                        self.focus = FocusArea::Chat;
                    }
                }
            }

            // ── Tab/BackTab: cycle focus ──────────────────────────────────────
            KeyCode::Tab => self.focus_next(),
            KeyCode::BackTab => self.focus_prev(),

            // ── Error shortcut ────────────────────────────────────────────────
            KeyCode::Char('!') if !ctrl && self.focus != FocusArea::Input => {
                if let Some(err) = &self.last_error {
                    self.status_line = err.clone();
                }
                self.error_active = false;
                self.last_error = None;
            }

            // Quit only via /quit command (no single-key quit to avoid accidents)

            // ── Arrow keys when focus is Input — move cursor in textarea ─────
            KeyCode::Left if self.focus == FocusArea::Input => {
                self.input.reduce(input::InputAction::MoveCursorLeft);
            }
            KeyCode::Right if self.focus == FocusArea::Input => {
                self.input.reduce(input::InputAction::MoveCursorRight);
            }
            KeyCode::Up if self.focus == FocusArea::Input => {
                self.input.reduce(input::InputAction::MoveCursorUp);
            }
            KeyCode::Down if self.focus == FocusArea::Input => {
                self.input.reduce(input::InputAction::MoveCursorDown);
            }
            KeyCode::Home if self.focus == FocusArea::Input => {
                self.input.reduce(input::InputAction::MoveCursorHome);
            }
            KeyCode::End if self.focus == FocusArea::Input => {
                self.input.reduce(input::InputAction::MoveCursorEnd);
            }

            // ── Undo / Redo ──────────────────────────────────────────────────
            KeyCode::Char('z') if ctrl && self.focus == FocusArea::Input => {
                self.input.reduce(input::InputAction::Undo);
            }
            KeyCode::Char('y') if ctrl && self.focus == FocusArea::Input => {
                self.input.reduce(input::InputAction::Redo);
            }

            // ── Scroll to top/bottom ─────────────────────────────────────────
            KeyCode::Home if self.focus == FocusArea::Chat => {
                self.chat.reduce(chat::ChatAction::ScrollChat(i32::MAX / 2));
                self.chat.select_message(Some(0));
            }
            KeyCode::End if self.focus == FocusArea::Chat => {
                let offset = self.chat.scroll_offset() as i32;
                self.chat.reduce(chat::ChatAction::ScrollChat(-offset));
                self.chat.select_message(None);
            }
            KeyCode::Down if self.focus != FocusArea::Input => match self.focus {
                FocusArea::Chat => self.chat.select_next_message(),
                FocusArea::Sidebar => self.sidebar.reduce(sidebar::SidebarAction::Navigate(1)),
                _ => {}
            },
            KeyCode::Up if self.focus != FocusArea::Input => match self.focus {
                FocusArea::Chat => self.chat.select_prev_message(),
                FocusArea::Sidebar => self.sidebar.reduce(sidebar::SidebarAction::Navigate(-1)),
                _ => {}
            },
            // Toggle reasoning on selected message (or last assistant if none selected)
            KeyCode::Char('r') if self.focus == FocusArea::Chat => {
                if let Some(sel) = self.chat.selected_message() {
                    self.chat.toggle_reasoning(sel);
                } else {
                    self.chat.toggle_last_reasoning();
                }
            }
            // Toggle tool expansion on selected message
            KeyCode::Char('e') if self.focus == FocusArea::Chat => {
                if let Some(sel) = self.chat.selected_message() {
                    // Only toggle if the selected message is a Tool message
                    let is_tool = self.chat.active_thread()
                        .and_then(|t| t.messages.get(sel))
                        .map(|m| m.role == chat::MessageRole::Tool)
                        .unwrap_or(false);
                    if is_tool {
                        self.chat.toggle_tool_expansion(sel);
                    }
                }
            }
            // Sidebar tab switching via Tab when sidebar is focused
            // (removed [/] keys — confusing)

            // ── Ctrl+J / Ctrl+Enter: insert newline in input ─────────────────
            KeyCode::Char('j') if ctrl && self.focus == FocusArea::Input => {
                self.input.reduce(input::InputAction::InsertNewline);
            }

            // ── Input-only: Enter submits, Backspace deletes, chars type ──────
            KeyCode::Enter => {
                // Shift+Enter, Alt+Enter, or Ctrl+Enter inserts newline
                let shift = modifiers.contains(KeyModifiers::SHIFT);
                let alt = modifiers.contains(KeyModifiers::ALT);
                let ctrl_enter = modifiers.contains(KeyModifiers::CONTROL);
                if shift || alt || ctrl_enter {
                    if self.focus != FocusArea::Input {
                        self.focus = FocusArea::Input;
                        self.input.set_mode(input::InputMode::Insert);
                    }
                    self.input.reduce(input::InputAction::InsertNewline);
                    return false;
                }
                // When Chat is focused and a message is selected: toggle tool expansion
                if self.focus == FocusArea::Chat {
                    if let Some(sel) = self.chat.selected_message() {
                        let is_tool = self.chat.active_thread()
                            .and_then(|t| t.messages.get(sel))
                            .map(|m| m.role == chat::MessageRole::Tool)
                            .unwrap_or(false);
                        if is_tool {
                            self.chat.toggle_tool_expansion(sel);
                        }
                        // Also toggle reasoning if it's an assistant message with reasoning
                        let has_reasoning = self.chat.active_thread()
                            .and_then(|t| t.messages.get(sel))
                            .map(|m| m.role == chat::MessageRole::Assistant && m.reasoning.is_some())
                            .unwrap_or(false);
                        if has_reasoning {
                            self.chat.toggle_reasoning(sel);
                        }
                        return false;
                    }
                }
                // When Sidebar is focused: open selected task thread
                if self.focus == FocusArea::Sidebar {
                    self.handle_sidebar_enter();
                    return false;
                }
                // Activate input focus on Enter if not already there
                if self.focus != FocusArea::Input {
                    self.focus = FocusArea::Input;
                    self.input.set_mode(input::InputMode::Insert);
                    return false;
                }
                self.input.reduce(input::InputAction::Submit);
                if let Some(prompt) = self.input.take_submitted() {
                    if prompt.starts_with('/') {
                        let trimmed = prompt.trim_start_matches('/');
                        let cmd = trimmed.split_whitespace().next().unwrap_or("");
                        let args = trimmed[cmd.len()..].trim();
                        if cmd == "apikey" && !args.is_empty() {
                            self.config.api_key = args.to_string();
                            self.status_line = format!("API key set ({}...)", &args[..args.len().min(8)]);
                            if let Ok(json) = serde_json::to_string(&serde_json::json!({
                                "api_key": args,
                            })) {
                                self.send_daemon_command(DaemonCommand::SetConfigJson(json));
                            }
                        } else if cmd == "attach" && !args.is_empty() {
                            self.attach_file(args);
                        } else {
                            self.execute_command(cmd);
                        }
                    } else {
                        self.submit_prompt(prompt);
                    }
                }
            }
            KeyCode::Backspace if ctrl => {
                // Ctrl+Backspace: delete word backwards
                if self.focus == FocusArea::Input {
                    self.input.reduce(input::InputAction::DeleteWord);
                }
            }
            KeyCode::Backspace => {
                if self.focus == FocusArea::Input {
                    self.input.reduce(input::InputAction::Backspace);
                    if self.modal.top() == Some(modal::ModalKind::CommandPalette) {
                        self.modal.reduce(modal::ModalAction::SetQuery(
                            self.input.buffer().to_string(),
                        ));
                    }
                }
            }

            // ── Slash in any focus: jump to input + open command palette ──────
            KeyCode::Char('/') if self.focus != FocusArea::Input => {
                self.input.reduce(input::InputAction::Clear);
                self.input.reduce(input::InputAction::InsertChar('/'));
                self.input.set_mode(input::InputMode::Insert);
                self.focus = FocusArea::Input;
                self.modal
                    .reduce(modal::ModalAction::Push(modal::ModalKind::CommandPalette));
            }

            // ── Ctrl+W: delete word backwards (alternative to Ctrl+Backspace) ──
            KeyCode::Char('w') if ctrl && self.focus == FocusArea::Input => {
                self.input.reduce(input::InputAction::DeleteWord);
            }

            // ── Ctrl+V: paste from system clipboard ─────────────────────────
            KeyCode::Char('v') if ctrl => {
                match arboard::Clipboard::new().and_then(|mut cb| cb.get_text()) {
                    Ok(text) if !text.is_empty() => {
                        self.handle_paste(text);
                    }
                    _ => {
                        // Clipboard unavailable or empty — ignore silently
                    }
                }
            }

            // ── Copy selected message to clipboard ──────────────────────────
            KeyCode::Char('c') if self.focus == FocusArea::Chat && self.chat.selected_message().is_some() => {
                if let Some(sel) = self.chat.selected_message() {
                    if let Some(thread) = self.chat.active_thread() {
                        if let Some(msg) = thread.messages.get(sel) {
                            copy_to_clipboard(&msg.content);
                            self.status_line = "Copied to clipboard".to_string();
                        }
                    }
                }
            }

            // ── All printable chars when focus is Input ───────────────────────
            KeyCode::Char(c) => {
                if self.focus == FocusArea::Input {
                    self.input.reduce(input::InputAction::InsertChar(c));
                    if c == '/'
                        && self.input.buffer() == "/"
                        && self.modal.top() != Some(modal::ModalKind::CommandPalette)
                    {
                        self.modal
                            .reduce(modal::ModalAction::Push(modal::ModalKind::CommandPalette));
                    }
                    if self.modal.top() == Some(modal::ModalKind::CommandPalette) {
                        self.modal.reduce(modal::ModalAction::SetQuery(
                            self.input.buffer().to_string(),
                        ));
                    }
                } else {
                    // When not in input, typing shifts focus to input
                    self.focus = FocusArea::Input;
                    self.input.set_mode(input::InputMode::Insert);
                    self.input.reduce(input::InputAction::InsertChar(c));
                }
            }

            _ => {}
        }

        false
    }

    fn handle_key_modal(
        &mut self,
        code: KeyCode,
        _modifiers: KeyModifiers,
        kind: modal::ModalKind,
    ) -> bool {
        // Settings modal: inline editing + field navigation
        if kind == modal::ModalKind::Settings {
            // When actively editing a text field, route all keys to the edit buffer
            if self.settings.is_editing() {
                // Textarea mode: Enter inserts newline, Ctrl+Enter confirms
                if self.settings.is_textarea() {
                    match code {
                        KeyCode::Enter if _modifiers.contains(KeyModifiers::CONTROL) => {
                            // Ctrl+Enter: confirm textarea edit (fall through to apply logic below)
                        }
                        KeyCode::Enter => {
                            // Plain Enter: insert newline in textarea
                            self.settings.reduce(SettingsAction::InsertChar('\n'));
                            return false;
                        }
                        KeyCode::Esc => {
                            self.settings.reduce(SettingsAction::CancelEdit);
                            return false;
                        }
                        KeyCode::Backspace => {
                            self.settings.reduce(SettingsAction::Backspace);
                            return false;
                        }
                        KeyCode::Char(c) => {
                            self.settings.reduce(SettingsAction::InsertChar(c));
                            return false;
                        }
                        _ => return false,
                    }
                }

                match code {
                    KeyCode::Enter => {
                        // Apply the edit buffer value to config
                        let field = self.settings.editing_field().unwrap_or("").to_string();
                        let value = self.settings.edit_buffer().to_string();
                        match field.as_str() {
                            "base_url" => self.config.base_url = value,
                            "api_key" => self.config.api_key = value,
                            "gateway_prefix" => self.config.gateway_prefix = value,
                            "slack_token" => self.config.slack_token = value,
                            "slack_channel_filter" => self.config.slack_channel_filter = value,
                            "telegram_token" => self.config.telegram_token = value,
                            "telegram_allowed_chats" => self.config.telegram_allowed_chats = value,
                            "discord_token" => self.config.discord_token = value,
                            "discord_channel_filter" => self.config.discord_channel_filter = value,
                            "discord_allowed_users" => self.config.discord_allowed_users = value,
                            "whatsapp_allowed_contacts" => self.config.whatsapp_allowed_contacts = value,
                            "whatsapp_token" => self.config.whatsapp_token = value,
                            "whatsapp_phone_id" => self.config.whatsapp_phone_id = value,
                            "firecrawl_api_key" => self.config.firecrawl_api_key = value,
                            "exa_api_key" => self.config.exa_api_key = value,
                            "tavily_api_key" => self.config.tavily_api_key = value,
                            "search_max_results" => {
                                if let Ok(n) = value.parse::<u32>() {
                                    self.config.search_max_results = n.clamp(1, 20);
                                }
                            }
                            "search_timeout" => {
                                if let Ok(n) = value.parse::<u32>() {
                                    self.config.search_timeout_secs = n.clamp(3, 120);
                                }
                            }
                            "honcho_api_key" => self.config.honcho_api_key = value,
                            "honcho_base_url" => self.config.honcho_base_url = value,
                            "honcho_workspace_id" => self.config.honcho_workspace_id = value,
                            "max_context_messages" => {
                                if let Ok(n) = value.parse::<u32>() {
                                    self.config.max_context_messages = n.clamp(10, 500);
                                }
                            }
                            "max_tool_loops" => {
                                if let Ok(n) = value.parse::<u32>() {
                                    self.config.max_tool_loops = n.clamp(0, 1000);
                                }
                            }
                            "max_retries" => {
                                if let Ok(n) = value.parse::<u32>() {
                                    self.config.max_retries = n.clamp(0, 10);
                                }
                            }
                            "retry_delay_ms" => {
                                if let Ok(n) = value.parse::<u32>() {
                                    self.config.retry_delay_ms = n.clamp(100, 60000);
                                }
                            }
                            "context_budget_tokens" => {
                                if let Ok(n) = value.parse::<u32>() {
                                    self.config.context_budget_tokens = n.clamp(10000, 500000);
                                }
                            }
                            "compact_threshold_pct" => {
                                if let Ok(n) = value.parse::<u32>() {
                                    self.config.compact_threshold_pct = n.clamp(50, 95);
                                }
                            }
                            "keep_recent_on_compact" => {
                                if let Ok(n) = value.parse::<u32>() {
                                    self.config.keep_recent_on_compact = n.clamp(1, 50);
                                }
                            }
                            "bash_timeout_secs" => {
                                if let Ok(n) = value.parse::<u32>() {
                                    self.config.bash_timeout_secs = n.clamp(5, 300);
                                }
                            }
                            "agent_name" => {
                                if let Some(ref mut raw) = self.config.agent_config_raw {
                                    raw["agent_name"] =
                                        serde_json::Value::String(value);
                                }
                            }
                            "system_prompt" => {
                                if let Some(ref mut raw) = self.config.agent_config_raw {
                                    raw["system_prompt"] =
                                        serde_json::Value::String(value);
                                } else {
                                    let mut raw = serde_json::json!({});
                                    raw["system_prompt"] =
                                        serde_json::Value::String(value);
                                    self.config.agent_config_raw = Some(raw);
                                }
                            }
                            _ => {}
                        }
                        self.settings.reduce(SettingsAction::ConfirmEdit);
                        self.sync_config_to_daemon();
                    }
                    KeyCode::Esc => {
                        self.settings.reduce(SettingsAction::CancelEdit);
                    }
                    KeyCode::Backspace => {
                        self.settings.reduce(SettingsAction::Backspace);
                    }
                    KeyCode::Char(c) => {
                        self.settings.reduce(SettingsAction::InsertChar(c));
                    }
                    _ => {}
                }
                return false;
            }

            // Not editing — normal navigation
            match code {
                KeyCode::Tab => {
                    let all = SettingsTab::all();
                    let current = self.settings.active_tab();
                    let next_idx = all
                        .iter()
                        .position(|&t| t == current)
                        .map(|i| (i + 1) % all.len())
                        .unwrap_or(0);
                    self.settings
                        .reduce(SettingsAction::SwitchTab(all[next_idx]));
                    return false;
                }
                KeyCode::BackTab => {
                    let all = SettingsTab::all();
                    let current = self.settings.active_tab();
                    let prev_idx = all
                        .iter()
                        .position(|&t| t == current)
                        .map(|i| if i == 0 { all.len() - 1 } else { i - 1 })
                        .unwrap_or(0);
                    self.settings
                        .reduce(SettingsAction::SwitchTab(all[prev_idx]));
                    return false;
                }
                KeyCode::Down => {
                    self.settings.reduce(SettingsAction::NavigateField(1));
                    return false;
                }
                KeyCode::Up => {
                    self.settings.reduce(SettingsAction::NavigateField(-1));
                    return false;
                }
                KeyCode::Enter => {
                    let field = self.settings.current_field_name().to_string();
                    match field.as_str() {
                        // Provider tab pickers
                        "provider" => {
                            self.execute_command("provider");
                        }
                        "model" => {
                            self.execute_command("model");
                        }
                        "reasoning_effort" => {
                            self.execute_command("effort");
                        }
                        // Provider tab inline text edits
                        "base_url" => {
                            let current = self.config.base_url.clone();
                            self.settings.start_editing("base_url", &current);
                        }
                        "api_key" => {
                            let current = self.config.api_key.clone();
                            self.settings.start_editing("api_key", &current);
                        }
                        // Gateway tab inline text edits
                        "gateway_prefix" => {
                            let current = self.config.gateway_prefix.clone();
                            self.settings.start_editing("gateway_prefix", &current);
                        }
                        "slack_token" => {
                            let current = self.config.slack_token.clone();
                            self.settings.start_editing("slack_token", &current);
                        }
                        "slack_channel_filter" => {
                            let current = self.config.slack_channel_filter.clone();
                            self.settings.start_editing("slack_channel_filter", &current);
                        }
                        "telegram_token" => {
                            let current = self.config.telegram_token.clone();
                            self.settings.start_editing("telegram_token", &current);
                        }
                        "telegram_allowed_chats" => {
                            let current = self.config.telegram_allowed_chats.clone();
                            self.settings.start_editing("telegram_allowed_chats", &current);
                        }
                        "discord_token" => {
                            let current = self.config.discord_token.clone();
                            self.settings.start_editing("discord_token", &current);
                        }
                        "discord_channel_filter" => {
                            let current = self.config.discord_channel_filter.clone();
                            self.settings.start_editing("discord_channel_filter", &current);
                        }
                        "discord_allowed_users" => {
                            let current = self.config.discord_allowed_users.clone();
                            self.settings.start_editing("discord_allowed_users", &current);
                        }
                        "whatsapp_allowed_contacts" => {
                            let current = self.config.whatsapp_allowed_contacts.clone();
                            self.settings.start_editing("whatsapp_allowed_contacts", &current);
                        }
                        "whatsapp_token" => {
                            let current = self.config.whatsapp_token.clone();
                            self.settings.start_editing("whatsapp_token", &current);
                        }
                        "whatsapp_phone_id" => {
                            let current = self.config.whatsapp_phone_id.clone();
                            self.settings.start_editing("whatsapp_phone_id", &current);
                        }
                        // Web Search tab
                        "search_provider" => {
                            // Cycle: none -> firecrawl -> exa -> tavily -> none
                            let next = match self.config.search_provider.as_str() {
                                "none" | "" => "firecrawl",
                                "firecrawl"  => "exa",
                                "exa"        => "tavily",
                                _            => "none",
                            };
                            self.config.search_provider = next.to_string();
                            self.sync_config_to_daemon();
                        }
                        "firecrawl_api_key" => {
                            let current = self.config.firecrawl_api_key.clone();
                            self.settings.start_editing("firecrawl_api_key", &current);
                        }
                        "exa_api_key" => {
                            let current = self.config.exa_api_key.clone();
                            self.settings.start_editing("exa_api_key", &current);
                        }
                        "tavily_api_key" => {
                            let current = self.config.tavily_api_key.clone();
                            self.settings.start_editing("tavily_api_key", &current);
                        }
                        "search_max_results" => {
                            let current = self.config.search_max_results.to_string();
                            self.settings.start_editing("search_max_results", &current);
                        }
                        "search_timeout" => {
                            let current = self.config.search_timeout_secs.to_string();
                            self.settings.start_editing("search_timeout", &current);
                        }
                        // Chat tab inline text edits
                        "honcho_api_key" => {
                            let current = self.config.honcho_api_key.clone();
                            self.settings.start_editing("honcho_api_key", &current);
                        }
                        "honcho_base_url" => {
                            let current = self.config.honcho_base_url.clone();
                            self.settings.start_editing("honcho_base_url", &current);
                        }
                        "honcho_workspace_id" => {
                            let current = self.config.honcho_workspace_id.clone();
                            self.settings.start_editing("honcho_workspace_id", &current);
                        }
                        // Advanced tab numeric edits
                        "max_context_messages" => {
                            let current = self.config.max_context_messages.to_string();
                            self.settings.start_editing("max_context_messages", &current);
                        }
                        "max_tool_loops" => {
                            let current = self.config.max_tool_loops.to_string();
                            self.settings.start_editing("max_tool_loops", &current);
                        }
                        "max_retries" => {
                            let current = self.config.max_retries.to_string();
                            self.settings.start_editing("max_retries", &current);
                        }
                        "retry_delay_ms" => {
                            let current = self.config.retry_delay_ms.to_string();
                            self.settings.start_editing("retry_delay_ms", &current);
                        }
                        "context_budget_tokens" => {
                            let current = self.config.context_budget_tokens.to_string();
                            self.settings.start_editing("context_budget_tokens", &current);
                        }
                        "compact_threshold_pct" => {
                            let current = self.config.compact_threshold_pct.to_string();
                            self.settings.start_editing("compact_threshold_pct", &current);
                        }
                        "keep_recent_on_compact" => {
                            let current = self.config.keep_recent_on_compact.to_string();
                            self.settings.start_editing("keep_recent_on_compact", &current);
                        }
                        "bash_timeout_secs" => {
                            let current = self.config.bash_timeout_secs.to_string();
                            self.settings.start_editing("bash_timeout_secs", &current);
                        }
                        // Agent tab inline text edits
                        "agent_name" => {
                            let current = if let Some(raw) = self.config.agent_config_raw.as_ref() {
                                raw.get("agent_name")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("Sisyphus")
                                    .to_string()
                            } else {
                                "Sisyphus".to_string()
                            };
                            self.settings.start_editing("agent_name", &current);
                        }
                        "system_prompt" => {
                            let current = if let Some(raw) = self.config.agent_config_raw.as_ref() {
                                raw.get("system_prompt")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string()
                            } else {
                                String::new()
                            };
                            self.settings.start_editing("system_prompt", &current);
                        }
                        _ => {}
                    }
                    return false;
                }
                KeyCode::Char(' ') => {
                    let field = self.settings.current_field_name().to_string();
                    match field.as_str() {
                        "gateway_enabled" => {
                            self.config.gateway_enabled = !self.config.gateway_enabled;
                            self.sync_config_to_daemon();
                        }
                        "web_search_enabled" => {
                            self.config.tool_web_search = !self.config.tool_web_search;
                            self.sync_config_to_daemon();
                        }
                        "enable_streaming" => {
                            self.config.enable_streaming = !self.config.enable_streaming;
                            self.sync_config_to_daemon();
                        }
                        "enable_conversation_memory" => {
                            self.config.enable_conversation_memory = !self.config.enable_conversation_memory;
                            self.sync_config_to_daemon();
                        }
                        "enable_honcho_memory" => {
                            self.config.enable_honcho_memory = !self.config.enable_honcho_memory;
                            self.sync_config_to_daemon();
                        }
                        "auto_compact_context" => {
                            self.config.auto_compact_context = !self.config.auto_compact_context;
                            self.sync_config_to_daemon();
                        }
                        f if f.starts_with("tool_") => {
                            let tool_name = f.strip_prefix("tool_").unwrap_or(f).to_string();
                            self.config.reduce(config::ConfigAction::ToggleTool(tool_name));
                            self.sync_config_to_daemon();
                        }
                        _ => {
                            self.settings.reduce(SettingsAction::ToggleCheckbox);
                        }
                    }
                    return false;
                }
                _ => {
                    // fall through to generic Esc handling below
                }
            }
        }

        // Approval modal has special single-key handling
        if kind == modal::ModalKind::ApprovalOverlay {
            match code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    if let Some(ap) = self.approval.current_approval() {
                        let id = ap.approval_id.clone();
                        self.send_daemon_command(DaemonCommand::ResolveTaskApproval {
                            approval_id: id,
                            decision: "allow_once".to_string(),
                        });
                    }
                    self.modal.reduce(modal::ModalAction::Pop);
                }
                KeyCode::Char('a') | KeyCode::Char('A') => {
                    if let Some(ap) = self.approval.current_approval() {
                        let id = ap.approval_id.clone();
                        self.send_daemon_command(DaemonCommand::ResolveTaskApproval {
                            approval_id: id,
                            decision: "allow_session".to_string(),
                        });
                    }
                    self.modal.reduce(modal::ModalAction::Pop);
                }
                KeyCode::Char('n') | KeyCode::Char('N') => {
                    if let Some(ap) = self.approval.current_approval() {
                        let id = ap.approval_id.clone();
                        self.send_daemon_command(DaemonCommand::ResolveTaskApproval {
                            approval_id: id,
                            decision: "reject".to_string(),
                        });
                    }
                    self.modal.reduce(modal::ModalAction::Pop);
                }
                _ => {}
            }
            return false;
        }

        // Command palette and thread picker allow typing for search
        let is_searchable = matches!(
            kind,
            modal::ModalKind::CommandPalette | modal::ModalKind::ThreadPicker
        );

        match code {
            KeyCode::Esc => {
                self.modal.reduce(modal::ModalAction::Pop);
                self.input.reduce(input::InputAction::Clear);
            }
            // Arrow keys navigate in ALL modals
            KeyCode::Down => {
                self.modal.reduce(modal::ModalAction::Navigate(1));
            }
            KeyCode::Up => {
                self.modal.reduce(modal::ModalAction::Navigate(-1));
            }
            KeyCode::Enter => {
                self.handle_modal_enter(kind);
                if self.pending_quit {
                    self.pending_quit = false;
                    return true;
                }
            }
            KeyCode::Backspace if is_searchable => {
                self.input.reduce(input::InputAction::Backspace);
                self.modal.reduce(modal::ModalAction::SetQuery(
                    self.input.buffer().to_string(),
                ));
            }
            KeyCode::Char(c) if is_searchable => {
                // Searchable modals: chars go to search input
                self.input.reduce(input::InputAction::InsertChar(c));
                self.modal.reduce(modal::ModalAction::SetQuery(
                    self.input.buffer().to_string(),
                ));
            }
            _ => {}
        }

        false
    }

    fn handle_modal_enter(&mut self, kind: modal::ModalKind) {
        tracing::info!("handle_modal_enter: {:?}", kind);
        match kind {
            modal::ModalKind::CommandPalette => {
                let cmd_name = self.modal.selected_command().map(|c| c.command.clone());
                tracing::info!(
                    "selected_command: {:?}, cursor: {}, filtered: {:?}",
                    cmd_name,
                    self.modal.picker_cursor(),
                    self.modal.filtered_items()
                );
                self.modal.reduce(modal::ModalAction::Pop);
                self.input.reduce(input::InputAction::Clear);
                if let Some(command) = cmd_name {
                    self.execute_command(&command);
                }
            }
            modal::ModalKind::ThreadPicker => {
                let cursor = self.modal.picker_cursor();
                self.modal.reduce(modal::ModalAction::Pop);
                self.input.reduce(input::InputAction::Clear);
                if cursor == 0 {
                    self.chat.reduce(chat::ChatAction::NewThread);
                    self.status_line = "New conversation".to_string();
                } else {
                    let threads = self.chat.threads();
                    if let Some(thread) = threads.get(cursor - 1) {
                        let tid = thread.id.clone();
                        let title = thread.title.clone();
                        self.chat
                            .reduce(chat::ChatAction::SelectThread(tid.clone()));
                        self.send_daemon_command(DaemonCommand::RequestThread(tid));
                        self.status_line = format!("Thread: {}", title);
                    }
                }
            }
            modal::ModalKind::ProviderPicker => {
                let cursor = self.modal.picker_cursor();
                if let Some(def) = providers::PROVIDERS.get(cursor) {
                    let old_base_url = self.config.base_url.clone();
                    self.config.provider = def.id.to_string();

                    // Auto-set base_url if it was empty or matched a previous provider's default
                    if old_base_url.is_empty() || providers::is_known_default_url(&old_base_url) {
                        self.config.base_url = def.default_base_url.to_string();
                    }

                    // Auto-set default model
                    self.config.model = def.default_model.to_string();

                    // Restore saved API key for this provider from agent_config_raw
                    if let Some(raw) = &self.config.agent_config_raw {
                        if let Some(provider_config) = raw.get(def.id) {
                            if let Some(key) = provider_config.get("apiKey").and_then(|v| v.as_str()) {
                                if !key.is_empty() {
                                    self.config.api_key = key.to_string();
                                }
                            }
                            // Also restore saved model if available
                            if let Some(saved_model) = provider_config.get("model").and_then(|v| v.as_str()) {
                                if !saved_model.is_empty() {
                                    self.config.model = saved_model.to_string();
                                }
                            }
                        }
                    }

                    // Repopulate hardcoded models for new provider
                    let models = providers::known_models_for_provider(def.id);
                    self.config
                        .reduce(config::ConfigAction::ModelsFetched(models));

                    self.status_line = format!("Provider: {}", def.name);
                    self.sync_config_to_daemon();
                }
                self.modal.reduce(modal::ModalAction::Pop);
            }
            modal::ModalKind::ModelPicker => {
                let models = self.config.fetched_models();
                if models.is_empty() {
                    // No models available -- close picker
                    self.status_line =
                        "No models available. Set model in /settings".to_string();
                } else {
                    let cursor = self.modal.picker_cursor();
                    if let Some(model) = models.get(cursor) {
                        let model_id = model.id.clone();
                        self.config
                            .reduce(config::ConfigAction::SetModel(model_id.clone()));
                        self.status_line = format!("Model: {}", model_id);
                        if let Ok(json) = serde_json::to_string(&serde_json::json!({
                            "model": model_id,
                        })) {
                            self.send_daemon_command(DaemonCommand::SetConfigJson(json));
                        }
                        self.save_settings();
                    }
                }
                self.modal.reduce(modal::ModalAction::Pop);
            }
            modal::ModalKind::EffortPicker => {
                let efforts = ["", "low", "medium", "high", "xhigh"];
                let cursor = self.modal.picker_cursor();
                if let Some(&effort) = efforts.get(cursor) {
                    self.config
                        .reduce(config::ConfigAction::SetReasoningEffort(
                            effort.to_string(),
                        ));
                    if let Ok(json) = serde_json::to_string(&serde_json::json!({
                        "reasoning_effort": effort,
                    })) {
                        self.send_daemon_command(DaemonCommand::SetConfigJson(json));
                    }
                    self.status_line = if effort.is_empty() {
                        "Effort: off".to_string()
                    } else {
                        format!("Effort: {}", effort)
                    };
                    self.save_settings();
                }
                self.modal.reduce(modal::ModalAction::Pop);
            }
            _ => {
                // Generic: just pop
                self.modal.reduce(modal::ModalAction::Pop);
                self.input.reduce(input::InputAction::Clear);
            }
        }
    }

    fn execute_command(&mut self, command: &str) {
        tracing::info!("execute_command: {:?}", command);
        match command {
            "provider" => {
                self.modal.reduce(modal::ModalAction::Push(modal::ModalKind::ProviderPicker));
                self.modal.set_picker_item_count(providers::PROVIDERS.len());
            }
            "model" => {
                // Show hardcoded models immediately as fallback
                let models = providers::known_models_for_provider(&self.config.provider);
                if !models.is_empty() {
                    self.config
                        .reduce(config::ConfigAction::ModelsFetched(models));
                }
                // Also trigger async fetch from provider API (will update list when done)
                self.send_daemon_command(DaemonCommand::FetchModels {
                    provider_id: self.config.provider.clone(),
                    base_url: self.config.base_url.clone(),
                    api_key: self.config.api_key.clone(),
                });
                let count = self.config.fetched_models().len().max(1);
                self.modal
                    .reduce(modal::ModalAction::Push(modal::ModalKind::ModelPicker));
                self.modal.set_picker_item_count(count);
            }
            "tools" => {
                self.status_line = "Tools config: use /settings -> Tools tab".to_string();
            }
            "effort" => {
                self.modal.reduce(modal::ModalAction::Push(modal::ModalKind::EffortPicker));
                self.modal.set_picker_item_count(5);
            }
            "thread" => self
                .modal
                .reduce(modal::ModalAction::Push(modal::ModalKind::ThreadPicker)),
            "new" => self.chat.reduce(chat::ChatAction::NewThread),
            "settings" => self
                .modal
                .reduce(modal::ModalAction::Push(modal::ModalKind::Settings)),
            "view" => {
                // Cycle transcript mode
                let next = match self.chat.transcript_mode() {
                    chat::TranscriptMode::Compact => chat::TranscriptMode::Tools,
                    chat::TranscriptMode::Tools => chat::TranscriptMode::Full,
                    chat::TranscriptMode::Full => chat::TranscriptMode::Compact,
                };
                self.chat
                    .reduce(chat::ChatAction::SetTranscriptMode(next));
                self.status_line = format!("View: {:?}", next);
            }
            "quit" => {
                self.pending_quit = true;
            }
            "prompt" => {
                self.status_line =
                    "System prompt: use /settings -> Agent tab".to_string();
            }
            "goal" => {
                self.status_line = "Goal runs: type your goal as a message".to_string();
            }
            "attach" => {
                self.status_line = "Usage: /attach <path>  — attach a file to the next message".to_string();
            }
            "help" => {
                self.modal.reduce(modal::ModalAction::Push(modal::ModalKind::Help));
                self.modal.set_picker_item_count(100);
            }
            _ => self.status_line = format!("Unknown command: {}", command),
        }
    }

    fn submit_prompt(&mut self, prompt: String) {
        if !self.connected {
            self.status_line = "Not connected to daemon".to_string();
            return;
        }

        // Queue the message if the assistant is currently streaming
        if self.chat.is_streaming() {
            self.queued_prompts.push(prompt);
            self.status_line = format!("QUEUED ({})", self.queued_prompts.len());
            return;
        }

        // Build final message content, prepending any pending attachments
        let final_content = if self.attachments.is_empty() {
            prompt.clone()
        } else {
            let mut parts: Vec<String> = self.attachments.drain(..)
                .map(|att| format!("<attached_file name=\"{}\">\n{}\n</attached_file>", att.filename, att.content))
                .collect();
            parts.push(prompt.clone());
            parts.join("\n\n")
        };

        let thread_id = self.chat.active_thread_id().map(String::from);

        // Add user message to local thread so it's visible immediately
        if thread_id.is_none() {
            // New thread — create one locally
            self.chat.reduce(chat::ChatAction::ThreadCreated {
                thread_id: format!("local-{}", self.tick_counter),
                title: if prompt.len() > 40 { format!("{}...", &prompt[..40]) } else { prompt.clone() },
            });
        }

        // Add user message to the active thread
        if let Some(thread) = self.chat.active_thread_mut() {
            thread.messages.push(chat::AgentMessage {
                role: chat::MessageRole::User,
                content: final_content.clone(),
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0),
                ..Default::default()
            });
        }

        // Send to daemon
        self.send_daemon_command(DaemonCommand::SendMessage {
            thread_id,
            content: final_content,
            session_id: self.default_session_id.clone(),
        });

        self.focus = FocusArea::Chat;
        // Keep insert mode so the user can immediately type the next message
        self.input.set_mode(input::InputMode::Insert);
        self.status_line = "Prompt sent".to_string();
        self.error_active = false; // Clear error on new message
    }

    fn focus_next(&mut self) {
        self.focus = match self.focus {
            FocusArea::Chat => FocusArea::Sidebar,
            FocusArea::Sidebar => FocusArea::Input,
            FocusArea::Input => FocusArea::Chat,
        };
        // Always keep input mode as Insert -- the UI has no Normal mode concept
        self.input.set_mode(input::InputMode::Insert);
    }

    fn focus_prev(&mut self) {
        self.focus = match self.focus {
            FocusArea::Chat => FocusArea::Input,
            FocusArea::Sidebar => FocusArea::Chat,
            FocusArea::Input => FocusArea::Sidebar,
        };
        // Always keep input mode as Insert -- the UI has no Normal mode concept
        self.input.set_mode(input::InputMode::Insert);
    }

    fn handle_sidebar_enter(&mut self) {
        let selected = self.sidebar.selected_item();
        // Build a flat list of items matching the sidebar task_tree rendering order
        let mut flat_items: Vec<SidebarFlatItem> = Vec::new();

        for run in self.tasks.goal_runs() {
            flat_items.push(SidebarFlatItem {
                thread_id: None, // Goal runs don't directly have thread_ids
                goal_run_id: Some(run.id.clone()),
                title: run.title.clone(),
            });
            if self.sidebar.is_expanded(&run.id) {
                for step in &run.steps {
                    flat_items.push(SidebarFlatItem {
                        thread_id: None,
                        goal_run_id: Some(run.id.clone()),
                        title: step.title.clone(),
                    });
                }
            }
        }

        // Standalone tasks
        for t in self.tasks.tasks() {
            if t.goal_run_id.is_none() {
                flat_items.push(SidebarFlatItem {
                    thread_id: None,
                    goal_run_id: None,
                    title: t.title.clone(),
                });
            }
        }

        if let Some(item) = flat_items.get(selected) {
            if let Some(goal_run_id) = &item.goal_run_id {
                // Request goal run detail
                self.send_daemon_command(DaemonCommand::RequestGoalRunDetail(goal_run_id.clone()));
                self.status_line = format!("Goal: {}", item.title);
            } else {
                self.status_line = format!("Task: {}", item.title);
            }
        }
    }

    pub fn handle_resize(&mut self, w: u16, h: u16) {
        self.width = w;
        self.height = h;
        // Clear sidebar override on resize so layout recalculates from breakpoints
        self.show_sidebar_override = None;
    }

    pub fn handle_mouse(&mut self, mouse: MouseEvent) {
        // Calculate layout boundaries to match the actual render() layout.
        // Vertical: header=3 rows, body=flex, input=3 rows, status=1 row.
        // Body starts at row 3, ends at height-4 (exclusive).
        // Input occupies rows [height-4, height-1), status is the last row.
        let body_start_row: u16 = 3;
        let actual_input_height = self.input_height();
        let input_start_row: u16 = self.height.saturating_sub(actual_input_height + 1); // +1 for status bar

        // Horizontal: sidebar is shown when width >= 80 (or overridden).
        // Chat takes (100 - sidebar_pct)% of the body width.
        let default_show_sidebar = self.width >= 80;
        let show_sidebar = self.show_sidebar_override.unwrap_or(default_show_sidebar);
        let sidebar_pct: u16 = if self.width >= 120 { 33 } else { 28 };
        // sidebar_start_col is where the sidebar begins (column index).
        // chat occupies [0, sidebar_start_col), sidebar occupies [sidebar_start_col, width).
        let sidebar_start_col: u16 = if show_sidebar {
            self.width * (100 - sidebar_pct) / 100
        } else {
            self.width // no sidebar visible
        };

        // Determine which pane the mouse cursor is in (for position-based scroll).
        let cursor_in_body = mouse.row >= body_start_row && mouse.row < input_start_row;
        let cursor_in_sidebar = show_sidebar && cursor_in_body && mouse.column >= sidebar_start_col;
        let cursor_in_chat = cursor_in_body && mouse.column < sidebar_start_col;
        let cursor_in_input = mouse.row >= input_start_row && mouse.row < self.height.saturating_sub(1);

        match mouse.kind {
            // Scroll based on where the cursor IS, not just the current focus.
            MouseEventKind::ScrollUp => {
                if cursor_in_chat {
                    self.chat.reduce(chat::ChatAction::ScrollChat(3));
                } else if cursor_in_sidebar {
                    self.sidebar.reduce(sidebar::SidebarAction::Scroll(3));
                } else if cursor_in_input {
                    // Scroll input by moving cursor up
                    for _ in 0..3 {
                        self.input.reduce(input::InputAction::MoveCursorUp);
                    }
                }
            }
            MouseEventKind::ScrollDown => {
                if cursor_in_chat {
                    self.chat.reduce(chat::ChatAction::ScrollChat(-3));
                } else if cursor_in_sidebar {
                    self.sidebar.reduce(sidebar::SidebarAction::Scroll(-3));
                } else if cursor_in_input {
                    for _ in 0..3 {
                        self.input.reduce(input::InputAction::MoveCursorDown);
                    }
                }
            }
            MouseEventKind::Down(MouseButton::Left) => {
                // Click-to-focus: set focus based on which pane was clicked.
                if cursor_in_chat {
                    self.focus = FocusArea::Chat;
                    // Click in chat area: select the last message as starting point.
                    // The user can then navigate with j/k.
                    let msg_count = self.chat.active_thread()
                        .map(|t| t.messages.len())
                        .unwrap_or(0);
                    if msg_count > 0 {
                        let last_idx = msg_count.saturating_sub(1);
                        self.chat.select_message(Some(last_idx));
                    }
                } else if cursor_in_sidebar {
                    self.focus = FocusArea::Sidebar;
                    // Click in sidebar: select item near click position
                    let click_row = mouse.row.saturating_sub(body_start_row + 2) as usize; // +2 for border + tabs
                    let scroll = self.sidebar.scroll_offset();
                    let item_idx = click_row + scroll;
                    self.sidebar.reduce(sidebar::SidebarAction::Navigate(
                        item_idx as i32 - self.sidebar.selected_item() as i32,
                    ));
                } else if cursor_in_input {
                    self.focus = FocusArea::Input;
                    // Calculate buffer position from click coordinates
                    let input_inner_x = mouse.column.saturating_sub(4) as usize; // border + prompt " ▶ "
                    let input_inner_y = mouse.row.saturating_sub(input_start_row + 1) as usize; // border
                    let offset = self.input.line_col_to_offset_public(input_inner_y, input_inner_x);
                    self.input.reduce(input::InputAction::MoveCursorToPos(offset));
                }
                // Always keep Insert mode active
                self.input.set_mode(input::InputMode::Insert);
            }
            // Right-click: paste from clipboard (like Ctrl+V)
            MouseEventKind::Down(MouseButton::Right) => {
                match arboard::Clipboard::new().and_then(|mut cb| cb.get_text()) {
                    Ok(text) if !text.is_empty() => {
                        self.handle_paste(text);
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

// ── Inline effort picker ─────────────────────────────────────────────────────

fn render_effort_picker(
    frame: &mut Frame,
    area: Rect,
    modal: &modal::ModalState,
    config: &config::ConfigState,
    theme: &ThemeTokens,
) {
    use ratatui::style::{Color, Style};
    use ratatui::text::{Line, Span};
    use ratatui::widgets::{Block, Borders, BorderType, List, ListItem, Paragraph};

    let block = Block::default()
        .title(" EFFORT ")
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(theme.accent_secondary);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let efforts = [
        ("", "Off"),
        ("low", "Low"),
        ("medium", "Medium"),
        ("high", "High"),
        ("xhigh", "Extra High"),
    ];

    let cursor = modal.picker_cursor();
    let current = config.reasoning_effort();

    let items: Vec<ListItem> = efforts
        .iter()
        .enumerate()
        .map(|(i, (value, label))| {
            let is_current = *value == current;
            let marker = if is_current { "\u{25cf} " } else { "  " };
            let is_selected = i == cursor;

            if is_selected {
                ListItem::new(Line::from(vec![
                    Span::raw("> "),
                    Span::raw(marker),
                    Span::raw(*label),
                ]))
                .style(
                    Style::default()
                        .bg(Color::Indexed(178))
                        .fg(Color::Black),
                )
            } else {
                let style = if is_current {
                    theme.accent_primary
                } else {
                    theme.fg_dim
                };
                ListItem::new(Line::from(vec![
                    Span::raw("  "),
                    Span::raw(marker),
                    Span::styled(*label, style),
                ]))
            }
        })
        .collect();

    // Split inner into list area and hints area
    let inner_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner);

    let list = List::new(items);
    frame.render_widget(list, inner_chunks[0]);

    // Hints
    let hints = Line::from(vec![
        Span::styled("↑↓", theme.fg_active),
        Span::styled(" nav  ", theme.fg_dim),
        Span::styled("Enter", theme.fg_active),
        Span::styled(" sel  ", theme.fg_dim),
        Span::styled("Esc", theme.fg_active),
        Span::styled(" close", theme.fg_dim),
    ]);
    frame.render_widget(Paragraph::new(hints), inner_chunks[1]);
}

// ── Help modal ───────────────────────────────────────────────────────────────

fn render_help_modal(frame: &mut Frame, area: Rect, theme: &ThemeTokens) {
    use ratatui::text::{Line, Span};
    use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Wrap};

    let block = Block::default()
        .title(" KEYBOARD SHORTCUTS ")
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(theme.accent_secondary);

    let inner = block.inner(area);
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);

    let lines = vec![
        Line::raw(""),
        Line::from(Span::styled("  Navigation", theme.accent_primary)),
        Line::from(vec![Span::styled("  Tab / Shift+Tab  ", theme.fg_active), Span::styled("Cycle focus: Chat → Sidebar → Input", theme.fg_dim)]),
        Line::from(vec![Span::styled("  Ctrl+P           ", theme.fg_active), Span::styled("Open command palette", theme.fg_dim)]),
        Line::from(vec![Span::styled("  Ctrl+T           ", theme.fg_active), Span::styled("Open thread picker", theme.fg_dim)]),
        Line::from(vec![Span::styled("  Ctrl+B           ", theme.fg_active), Span::styled("Toggle sidebar", theme.fg_dim)]),
        Line::from(vec![Span::styled("  /                ", theme.fg_active), Span::styled("Open command palette (from any focus)", theme.fg_dim)]),
        Line::raw(""),
        Line::from(Span::styled("  Chat (when focused)", theme.accent_primary)),
        Line::from(vec![Span::styled("  ↑ / ↓            ", theme.fg_active), Span::styled("Select message", theme.fg_dim)]),
        Line::from(vec![Span::styled("  PgUp / PgDn      ", theme.fg_active), Span::styled("Scroll chat", theme.fg_dim)]),
        Line::from(vec![Span::styled("  Ctrl+D / Ctrl+U  ", theme.fg_active), Span::styled("Half-page scroll", theme.fg_dim)]),
        Line::from(vec![Span::styled("  Home / End       ", theme.fg_active), Span::styled("Scroll to top / bottom", theme.fg_dim)]),
        Line::from(vec![Span::styled("  r                ", theme.fg_active), Span::styled("Toggle reasoning on selected message", theme.fg_dim)]),
        Line::from(vec![Span::styled("  e / Enter        ", theme.fg_active), Span::styled("Toggle tool call expansion", theme.fg_dim)]),
        Line::from(vec![Span::styled("  c                ", theme.fg_active), Span::styled("Copy selected message to clipboard", theme.fg_dim)]),
        Line::from(vec![Span::styled("  Esc              ", theme.fg_active), Span::styled("Clear selection", theme.fg_dim)]),
        Line::raw(""),
        Line::from(Span::styled("  Input", theme.accent_primary)),
        Line::from(vec![Span::styled("  Enter            ", theme.fg_active), Span::styled("Send message", theme.fg_dim)]),
        Line::from(vec![Span::styled("  Shift+Enter      ", theme.fg_active), Span::styled("Insert newline", theme.fg_dim)]),
        Line::from(vec![Span::styled("  ← → ↑ ↓         ", theme.fg_active), Span::styled("Move cursor in textarea", theme.fg_dim)]),
        Line::from(vec![Span::styled("  Ctrl+Backspace   ", theme.fg_active), Span::styled("Delete word backwards", theme.fg_dim)]),
        Line::from(vec![Span::styled("  Ctrl+W           ", theme.fg_active), Span::styled("Delete word backwards", theme.fg_dim)]),
        Line::from(vec![Span::styled("  Ctrl+U           ", theme.fg_active), Span::styled("Clear input line", theme.fg_dim)]),
        Line::from(vec![Span::styled("  Ctrl+Z           ", theme.fg_active), Span::styled("Undo", theme.fg_dim)]),
        Line::from(vec![Span::styled("  Ctrl+Y           ", theme.fg_active), Span::styled("Redo", theme.fg_dim)]),
        Line::raw(""),
        Line::from(Span::styled("  Streaming", theme.accent_primary)),
        Line::from(vec![Span::styled("  Esc              ", theme.fg_active), Span::styled("Show stop prompt (first press)", theme.fg_dim)]),
        Line::from(vec![Span::styled("  Esc Esc          ", theme.fg_active), Span::styled("Force stop stream (double press within 2s)", theme.fg_dim)]),
        Line::raw(""),
        Line::from(Span::styled("  Error", theme.accent_primary)),
        Line::from(vec![Span::styled("  !                ", theme.fg_active), Span::styled("Show last error, clear error dot", theme.fg_dim)]),
        Line::raw(""),
        Line::from(Span::styled("  Commands (/)", theme.accent_primary)),
        Line::from(vec![Span::styled("  /settings        ", theme.fg_active), Span::styled("Open settings panel", theme.fg_dim)]),
        Line::from(vec![Span::styled("  /provider        ", theme.fg_active), Span::styled("Switch LLM provider", theme.fg_dim)]),
        Line::from(vec![Span::styled("  /model           ", theme.fg_active), Span::styled("Switch model", theme.fg_dim)]),
        Line::from(vec![Span::styled("  /effort          ", theme.fg_active), Span::styled("Set reasoning effort", theme.fg_dim)]),
        Line::from(vec![Span::styled("  /thread          ", theme.fg_active), Span::styled("Pick conversation thread", theme.fg_dim)]),
        Line::from(vec![Span::styled("  /new             ", theme.fg_active), Span::styled("New conversation", theme.fg_dim)]),
        Line::from(vec![Span::styled("  /attach <path>   ", theme.fg_active), Span::styled("Attach file to message", theme.fg_dim)]),
        Line::from(vec![Span::styled("  /view            ", theme.fg_active), Span::styled("Cycle transcript mode", theme.fg_dim)]),
        Line::from(vec![Span::styled("  /help            ", theme.fg_active), Span::styled("This help screen", theme.fg_dim)]),
        Line::from(vec![Span::styled("  /quit            ", theme.fg_active), Span::styled("Exit TUI", theme.fg_dim)]),
        Line::raw(""),
        Line::from(Span::styled("  Press Esc to close", theme.fg_dim)),
    ];

    let paragraph = Paragraph::new(lines)
        .scroll((0, 0))
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);
}

// ── Helper ───────────────────────────────────────────────────────────────────

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

// ── Wire-to-state type conversions ───────────────────────────────────────────

fn convert_thread(t: crate::wire::AgentThread) -> chat::AgentThread {
    chat::AgentThread {
        id: t.id,
        title: t.title,
        created_at: t.created_at,
        updated_at: t.updated_at,
        messages: t.messages.into_iter().map(convert_message).collect(),
        total_input_tokens: t.total_input_tokens,
        total_output_tokens: t.total_output_tokens,
    }
}

fn convert_message(m: crate::wire::AgentMessage) -> chat::AgentMessage {
    chat::AgentMessage {
        role: match m.role {
            crate::wire::MessageRole::System => chat::MessageRole::System,
            crate::wire::MessageRole::User => chat::MessageRole::User,
            crate::wire::MessageRole::Assistant => chat::MessageRole::Assistant,
            crate::wire::MessageRole::Tool => chat::MessageRole::Tool,
            crate::wire::MessageRole::Unknown => chat::MessageRole::Unknown,
        },
        content: m.content,
        reasoning: m.reasoning,
        tool_name: m.tool_name,
        tool_arguments: m.tool_arguments,
        tool_call_id: m.tool_call_id,
        tool_status: m.tool_status,
        input_tokens: m.input_tokens,
        output_tokens: m.output_tokens,
        tps: m.tps,
        generation_ms: m.generation_ms,
        cost: m.cost,
        is_streaming: m.is_streaming,
        timestamp: m.timestamp,
    }
}

fn convert_task(t: crate::wire::AgentTask) -> task::AgentTask {
    task::AgentTask {
        id: t.id,
        title: t.title,
        status: t.status.map(|s| match s {
            crate::wire::TaskStatus::Queued => task::TaskStatus::Queued,
            crate::wire::TaskStatus::InProgress => task::TaskStatus::InProgress,
            crate::wire::TaskStatus::AwaitingApproval => task::TaskStatus::AwaitingApproval,
            crate::wire::TaskStatus::Blocked => task::TaskStatus::Blocked,
            crate::wire::TaskStatus::FailedAnalyzing => task::TaskStatus::FailedAnalyzing,
            crate::wire::TaskStatus::Completed => task::TaskStatus::Completed,
            crate::wire::TaskStatus::Failed => task::TaskStatus::Failed,
            crate::wire::TaskStatus::Cancelled => task::TaskStatus::Cancelled,
        }),
        progress: t.progress,
        session_id: t.session_id,
        goal_run_id: t.goal_run_id,
        goal_step_title: t.goal_step_title,
        awaiting_approval_id: t.awaiting_approval_id,
        blocked_reason: t.blocked_reason,
    }
}

fn convert_goal_run(r: crate::wire::GoalRun) -> task::GoalRun {
    task::GoalRun {
        id: r.id,
        title: r.title,
        status: r.status.map(|s| match s {
            crate::wire::GoalRunStatus::Queued => task::GoalRunStatus::Pending,
            crate::wire::GoalRunStatus::Planning => task::GoalRunStatus::Pending,
            crate::wire::GoalRunStatus::Running => task::GoalRunStatus::Running,
            crate::wire::GoalRunStatus::AwaitingApproval => task::GoalRunStatus::Pending,
            crate::wire::GoalRunStatus::Paused => task::GoalRunStatus::Pending,
            crate::wire::GoalRunStatus::Completed => task::GoalRunStatus::Completed,
            crate::wire::GoalRunStatus::Failed => task::GoalRunStatus::Failed,
            crate::wire::GoalRunStatus::Cancelled => task::GoalRunStatus::Cancelled,
        }),
        steps: r
            .steps
            .into_iter()
            .map(|step| task::GoalRunStep {
                id: step.id,
                title: step.title,
                status: step.status.map(|s| match s {
                    crate::wire::GoalRunStepStatus::Pending => task::GoalRunStatus::Pending,
                    crate::wire::GoalRunStepStatus::InProgress => task::GoalRunStatus::Running,
                    crate::wire::GoalRunStepStatus::Completed => task::GoalRunStatus::Completed,
                    crate::wire::GoalRunStepStatus::Failed => task::GoalRunStatus::Failed,
                    crate::wire::GoalRunStepStatus::Skipped => task::GoalRunStatus::Cancelled,
                }),
                order: step.position as u32,
            })
            .collect(),
        created_at: 0,
        updated_at: 0,
    }
}

fn convert_heartbeat(h: crate::wire::HeartbeatItem) -> task::HeartbeatItem {
    task::HeartbeatItem {
        id: h.id,
        label: h.label,
        outcome: h.last_result.map(|r| match r {
            crate::wire::HeartbeatOutcome::Ok => task::HeartbeatOutcome::Ok,
            crate::wire::HeartbeatOutcome::Alert => task::HeartbeatOutcome::Warn,
            crate::wire::HeartbeatOutcome::Error => task::HeartbeatOutcome::Error,
        }),
        message: h.last_message,
        timestamp: 0,
    }
}

/// Copy text to the system clipboard using the OSC 52 escape sequence.
/// This works in most modern terminals (iTerm2, kitty, alacritty, Windows Terminal, etc.)
fn copy_to_clipboard(text: &str) {
    use base64::Engine;
    let encoded = base64::engine::general_purpose::STANDARD.encode(text);
    // OSC 52: set clipboard content — \x1b]52;c;<base64>\x07
    print!("\x1b]52;c;{}\x07", encoded);
}

