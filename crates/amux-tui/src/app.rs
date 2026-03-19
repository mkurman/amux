//! TuiModel compositor — delegates to decomposed state modules.
//!
//! This replaces the old monolithic 3,500-line app.rs with a clean
//! compositor that owns the 8 state sub-modules and bridges between
//! the daemon client events and the UI state.

use std::sync::mpsc::Receiver;

use ftui_core::event::{Event, KeyCode, KeyEventKind, Modifiers};
use ftui_runtime::program::Cmd;
use ftui_runtime::string_model::StringModel;
use tokio::sync::mpsc::UnboundedSender;
use web_time::Duration;

use crate::client::ClientEvent;
use crate::state::*;
use crate::theme::ThemeTokens;

// ── Message ──────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum Msg {
    Event(Event),
}

impl From<Event> for Msg {
    fn from(value: Event) -> Self {
        Self::Event(value)
    }
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
    #[allow(dead_code)]
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

    // Vim motion state
    pending_g: bool,

    // Responsive layout override: when Some, overrides breakpoint-based sidebar visibility
    show_sidebar_override: Option<bool>,

    // Set by /quit command; checked after modal enter to issue Cmd::quit()
    pending_quit: bool,
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

            pending_g: false,
            show_sidebar_override: None,
            pending_quit: false,
        }
    }

    fn send_daemon_command(&self, command: DaemonCommand) {
        let _ = self.daemon_cmd_tx.send(command);
    }

    // ── Daemon event pump ────────────────────────────────────────────────────

    fn pump_daemon_events(&mut self) {
        while let Ok(event) = self.daemon_events_rx.try_recv() {
            self.handle_client_event(event);
        }
    }

    fn handle_client_event(&mut self, event: ClientEvent) {
        match event {
            ClientEvent::Connected => {
                self.connected = true;
                self.status_line = "Connected to daemon".to_string();
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
            }
            ClientEvent::Error(message) => {
                self.status_line = format!("Error: {}", message);
            }
        }
    }

    // ── Key handling ─────────────────────────────────────────────────────────

    fn handle_key(&mut self, code: KeyCode, modifiers: Modifiers) -> Cmd<Msg> {
        // Modal takes priority
        if let Some(modal_kind) = self.modal.top() {
            return self.handle_key_modal(code, modifiers, modal_kind);
        }

        match self.input.mode() {
            input::InputMode::Normal => self.handle_key_normal(code, modifiers),
            input::InputMode::Insert => self.handle_key_insert(code, modifiers),
        }
    }

    fn handle_key_normal(&mut self, code: KeyCode, modifiers: Modifiers) -> Cmd<Msg> {
        let ctrl = modifiers.contains(Modifiers::CTRL);

        // Clear pending_g on any key that isn't 'g'
        if code != KeyCode::Char('g') {
            self.pending_g = false;
        }

        match code {
            KeyCode::Char('q') if !ctrl => return Cmd::quit(),
            KeyCode::Char('p') if ctrl => {
                self.modal
                    .reduce(modal::ModalAction::Push(modal::ModalKind::CommandPalette));
            }
            KeyCode::Char('t') if ctrl => {
                self.modal
                    .reduce(modal::ModalAction::Push(modal::ModalKind::ThreadPicker));
            }
            // Ctrl+B: toggle sidebar visibility override (useful in narrow terminals)
            KeyCode::Char('b') if ctrl => {
                let current = self.show_sidebar_override.unwrap_or(self.width >= 80);
                self.show_sidebar_override = Some(!current);
            }
            KeyCode::Tab => self.focus_next(),
            KeyCode::BackTab => self.focus_prev(),
            KeyCode::Char('i') | KeyCode::Enter => {
                self.focus = FocusArea::Input;
                self.input.set_mode(input::InputMode::Insert);
            }
            KeyCode::Char('/') => {
                self.input.reduce(input::InputAction::Clear);
                self.input.reduce(input::InputAction::InsertChar('/'));
                self.input.set_mode(input::InputMode::Insert);
                self.focus = FocusArea::Input;
                self.modal
                    .reduce(modal::ModalAction::Push(modal::ModalKind::CommandPalette));
            }
            // Vim motions
            KeyCode::Char('G') if !ctrl => {
                // Jump to bottom (most recent)
                self.chat.reduce(chat::ChatAction::ScrollChat(-(self.chat.scroll_offset() as i32)));
            }
            KeyCode::Char('g') if !ctrl => {
                if self.pending_g {
                    // gg = scroll to top
                    self.chat.reduce(chat::ChatAction::ScrollChat(i32::MAX / 2));
                    self.pending_g = false;
                } else {
                    self.pending_g = true;
                }
                return Cmd::none();
            }
            KeyCode::Char('d') if ctrl => {
                let half_page = (self.height / 2) as i32;
                self.chat.reduce(chat::ChatAction::ScrollChat(-half_page));
            }
            KeyCode::Char('u') if ctrl => {
                let half_page = (self.height / 2) as i32;
                self.chat.reduce(chat::ChatAction::ScrollChat(half_page));
            }
            KeyCode::Char('j') | KeyCode::Down => match self.focus {
                FocusArea::Chat => self.chat.reduce(chat::ChatAction::ScrollChat(-1)),
                FocusArea::Sidebar => self
                    .sidebar
                    .reduce(sidebar::SidebarAction::Navigate(1)),
                _ => {}
            },
            KeyCode::Char('k') | KeyCode::Up => match self.focus {
                FocusArea::Chat => self.chat.reduce(chat::ChatAction::ScrollChat(1)),
                FocusArea::Sidebar => self
                    .sidebar
                    .reduce(sidebar::SidebarAction::Navigate(-1)),
                _ => {}
            },
            KeyCode::Char('[') => self
                .sidebar
                .reduce(sidebar::SidebarAction::SwitchTab(sidebar::SidebarTab::Tasks)),
            KeyCode::Char(']') => self.sidebar.reduce(sidebar::SidebarAction::SwitchTab(
                sidebar::SidebarTab::Subagents,
            )),
            KeyCode::Escape => {
                // Already in normal mode, no modal -- do nothing
            }
            _ => {}
        }

        Cmd::none()
    }

    fn handle_key_insert(&mut self, code: KeyCode, modifiers: Modifiers) -> Cmd<Msg> {
        let ctrl = modifiers.contains(Modifiers::CTRL);

        // Global shortcuts even in insert mode
        if ctrl {
            match code {
                KeyCode::Char('p') => {
                    self.modal
                        .reduce(modal::ModalAction::Push(modal::ModalKind::CommandPalette));
                    return Cmd::none();
                }
                KeyCode::Char('t') => {
                    self.modal
                        .reduce(modal::ModalAction::Push(modal::ModalKind::ThreadPicker));
                    return Cmd::none();
                }
                _ => {}
            }
        }

        match code {
            KeyCode::Escape => {
                self.input.set_mode(input::InputMode::Normal);
            }
            KeyCode::Enter => {
                self.input.reduce(input::InputAction::Submit);
                if let Some(prompt) = self.input.take_submitted() {
                    self.submit_prompt(prompt);
                }
            }
            KeyCode::Backspace => {
                self.input.reduce(input::InputAction::Backspace);
                // Update command palette filter if open
                if self.modal.top() == Some(modal::ModalKind::CommandPalette) {
                    self.modal.reduce(modal::ModalAction::SetQuery(
                        self.input.buffer().to_string(),
                    ));
                }
            }
            KeyCode::Tab => {
                self.input.set_mode(input::InputMode::Normal);
                self.focus_next();
            }
            KeyCode::Char(c) => {
                self.input.reduce(input::InputAction::InsertChar(c));
                // Auto-open command palette on first '/'
                if c == '/'
                    && self.input.buffer() == "/"
                    && self.modal.top() != Some(modal::ModalKind::CommandPalette)
                {
                    self.modal
                        .reduce(modal::ModalAction::Push(modal::ModalKind::CommandPalette));
                }
                // Update command palette filter
                if self.modal.top() == Some(modal::ModalKind::CommandPalette) {
                    self.modal.reduce(modal::ModalAction::SetQuery(
                        self.input.buffer().to_string(),
                    ));
                }
            }
            _ => {}
        }

        Cmd::none()
    }

    fn handle_key_modal(
        &mut self,
        code: KeyCode,
        _modifiers: Modifiers,
        kind: modal::ModalKind,
    ) -> Cmd<Msg> {
        // Settings modal: Tab cycles tabs, Esc closes
        if kind == modal::ModalKind::Settings {
            match code {
                KeyCode::Tab => {
                    let all = SettingsTab::all();
                    let current = self.settings.active_tab();
                    let next_idx = all.iter().position(|&t| t == current)
                        .map(|i| (i + 1) % all.len())
                        .unwrap_or(0);
                    self.settings.reduce(SettingsAction::SwitchTab(all[next_idx]));
                    return Cmd::none();
                }
                KeyCode::BackTab => {
                    let all = SettingsTab::all();
                    let current = self.settings.active_tab();
                    let prev_idx = all.iter().position(|&t| t == current)
                        .map(|i| if i == 0 { all.len() - 1 } else { i - 1 })
                        .unwrap_or(0);
                    self.settings.reduce(SettingsAction::SwitchTab(all[prev_idx]));
                    return Cmd::none();
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
            return Cmd::none();
        }

        // Command palette and thread picker allow typing for search
        let is_searchable = matches!(
            kind,
            modal::ModalKind::CommandPalette | modal::ModalKind::ThreadPicker
        );

        match code {
            KeyCode::Escape => {
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
                    return Cmd::quit();
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
            // Non-searchable modals: j/k navigate
            KeyCode::Char('j') if !is_searchable => {
                self.modal.reduce(modal::ModalAction::Navigate(1));
            }
            KeyCode::Char('k') if !is_searchable => {
                self.modal.reduce(modal::ModalAction::Navigate(-1));
            }
            _ => {}
        }

        Cmd::none()
    }

    fn handle_modal_enter(&mut self, kind: modal::ModalKind) {
        match kind {
            modal::ModalKind::CommandPalette => {
                let cmd_name = self.modal.selected_command().map(|c| c.command.clone());
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
                        self.chat.reduce(chat::ChatAction::SelectThread(tid.clone()));
                        self.send_daemon_command(DaemonCommand::RequestThread(tid));
                        self.status_line = format!("Thread: {}", title);
                    }
                }
            }
            modal::ModalKind::ProviderPicker => {
                // Provider list is hardcoded in the widget; map cursor to provider name
                let providers = [
                    "openai", "anthropic", "groq", "ollama", "together",
                    "deepinfra", "cerebras", "zai", "kimi", "qwen",
                    "minimax", "openrouter", "custom",
                ];
                let cursor = self.modal.picker_cursor();
                if let Some(&provider) = providers.get(cursor) {
                    self.config.reduce(config::ConfigAction::SetProvider(provider.to_string()));
                    self.status_line = format!("Provider: {}", provider);
                    // Sync to daemon
                    if let Ok(json) = serde_json::to_string(&serde_json::json!({
                        "provider": provider,
                    })) {
                        self.send_daemon_command(DaemonCommand::SetConfigJson(json));
                    }
                }
                self.modal.reduce(modal::ModalAction::Pop);
            }
            modal::ModalKind::ModelPicker => {
                let models = self.config.fetched_models();
                if models.is_empty() {
                    // Still waiting for models — don't pop, re-fetch
                    self.send_daemon_command(DaemonCommand::FetchModels {
                        provider_id: self.config.provider.clone(),
                        base_url: self.config.base_url.clone(),
                        api_key: self.config.api_key.clone(),
                    });
                    self.status_line = "Waiting for models...".to_string();
                    return; // don't pop
                }
                let cursor = self.modal.picker_cursor();
                if let Some(model) = models.get(cursor) {
                    let model_id = model.id.clone();
                    self.config.reduce(config::ConfigAction::SetModel(model_id.clone()));
                    self.status_line = format!("Model: {}", model_id);
                    if let Ok(json) = serde_json::to_string(&serde_json::json!({
                        "model": model_id,
                    })) {
                        self.send_daemon_command(DaemonCommand::SetConfigJson(json));
                    }
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
        match command {
            "provider" => self
                .modal
                .reduce(modal::ModalAction::Push(modal::ModalKind::ProviderPicker)),
            "model" => {
                // Auto-fetch models when opening picker
                self.send_daemon_command(DaemonCommand::FetchModels {
                    provider_id: self.config.provider.clone(),
                    base_url: self.config.base_url.clone(),
                    api_key: self.config.api_key.clone(),
                });
                self.modal
                    .reduce(modal::ModalAction::Push(modal::ModalKind::ModelPicker));
            }
            "tools" => {
                self.status_line = "Tools config: use /settings → Tools tab".to_string();
            }
            "effort" => {
                // Cycle: "" → "low" → "medium" → "high" → ""
                let next = match self.config.reasoning_effort() {
                    "" => "low",
                    "low" => "medium",
                    "medium" => "high",
                    _ => "",
                };
                self.config.reduce(config::ConfigAction::SetReasoningEffort(next.to_string()));
                if let Ok(json) = serde_json::to_string(&serde_json::json!({
                    "reasoning_effort": next,
                })) {
                    self.send_daemon_command(DaemonCommand::SetConfigJson(json));
                }
                self.status_line = if next.is_empty() {
                    "Effort: default".to_string()
                } else {
                    format!("Effort: {}", next)
                };
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
                self.chat.reduce(chat::ChatAction::SetTranscriptMode(next));
                self.status_line = format!("View: {:?}", next);
            }
            "quit" => {
                self.pending_quit = true;
            }
            "prompt" => {
                self.status_line = "System prompt: use /settings → Agent tab".to_string();
            }
            "goal" => {
                self.status_line = "Goal runs: type your goal as a message".to_string();
            }
            _ => self.status_line = format!("Unknown command: {}", command),
        }
    }

    fn submit_prompt(&mut self, prompt: String) {
        if !self.connected {
            self.status_line = "Not connected to daemon".to_string();
            return;
        }

        // Send user message to daemon
        let thread_id = self.chat.active_thread_id().map(String::from);

        self.send_daemon_command(DaemonCommand::SendMessage {
            thread_id,
            content: prompt,
            session_id: self.default_session_id.clone(),
        });

        self.focus = FocusArea::Chat;
        self.input.set_mode(input::InputMode::Normal);
        self.status_line = "Prompt sent".to_string();
    }

    fn focus_next(&mut self) {
        self.focus = match self.focus {
            FocusArea::Chat => FocusArea::Sidebar,
            FocusArea::Sidebar => FocusArea::Input,
            FocusArea::Input => FocusArea::Chat,
        };
        if self.focus == FocusArea::Input {
            self.input.set_mode(input::InputMode::Insert);
        } else {
            self.input.set_mode(input::InputMode::Normal);
        }
    }

    fn focus_prev(&mut self) {
        self.focus = match self.focus {
            FocusArea::Chat => FocusArea::Input,
            FocusArea::Sidebar => FocusArea::Chat,
            FocusArea::Input => FocusArea::Sidebar,
        };
        if self.focus == FocusArea::Input {
            self.input.set_mode(input::InputMode::Insert);
        } else {
            self.input.set_mode(input::InputMode::Normal);
        }
    }
}

// ── StringModel implementation ───────────────────────────────────────────────

impl StringModel for TuiModel {
    type Message = Msg;

    fn init(&mut self) -> Cmd<Msg> {
        // Schedule a 50ms tick for polling daemon events
        Cmd::Tick(Duration::from_millis(50))
    }

    fn update(&mut self, msg: Msg) -> Cmd<Msg> {
        match msg {
            Msg::Event(Event::Tick) => {
                self.pump_daemon_events();
                self.tick_counter = self.tick_counter.wrapping_add(1);
                Cmd::Tick(Duration::from_millis(50))
            }
            Msg::Event(Event::Key(key)) => {
                if key.kind != KeyEventKind::Press {
                    return Cmd::none();
                }
                self.handle_key(key.code, key.modifiers)
            }
            Msg::Event(Event::Resize { width, height }) => {
                self.width = width;
                self.height = height;
                // Clear sidebar override on resize so layout recalculates from breakpoints
                self.show_sidebar_override = None;
                Cmd::none()
            }
            Msg::Event(Event::Mouse(mouse)) => {
                match mouse.kind {
                    ftui_core::event::MouseEventKind::ScrollUp => {
                        match self.focus {
                            FocusArea::Chat => self.chat.reduce(chat::ChatAction::ScrollChat(3)),
                            FocusArea::Sidebar => self.sidebar.reduce(sidebar::SidebarAction::Scroll(3)),
                            _ => {}
                        }
                    }
                    ftui_core::event::MouseEventKind::ScrollDown => {
                        match self.focus {
                            FocusArea::Chat => self.chat.reduce(chat::ChatAction::ScrollChat(-3)),
                            FocusArea::Sidebar => self.sidebar.reduce(sidebar::SidebarAction::Scroll(-3)),
                            _ => {}
                        }
                    }
                    ftui_core::event::MouseEventKind::Down(ftui_core::event::MouseButton::Left) => {
                        // Click-to-focus: determine which pane was clicked
                        let sidebar_start = if self.width >= 80 {
                            (self.width as usize * 65 / 100) as u16
                        } else {
                            self.width // no sidebar
                        };
                        if mouse.y >= 3 && mouse.y < self.height.saturating_sub(4) {
                            if mouse.x < sidebar_start {
                                self.focus = FocusArea::Chat;
                            } else {
                                self.focus = FocusArea::Sidebar;
                            }
                        } else if mouse.y >= self.height.saturating_sub(4) {
                            self.focus = FocusArea::Input;
                            self.input.set_mode(input::InputMode::Insert);
                        }
                    }
                    _ => {}
                }
                Cmd::none()
            }
            _ => Cmd::none(),
        }
    }

    fn view_string(&self) -> String {
        let mut lines = Vec::new();
        // Use actual terminal size, not stored (which may be stale/default)
        let (term_w, term_h) = crossterm::terminal::size().unwrap_or((self.width, self.height));
        let w = term_w as usize;
        let h = term_h as usize;

        // Header (3 lines)
        let header_lines = crate::widgets::header::header_widget(
            &self.config, &self.chat, &self.theme, false, w,
        );
        lines.extend(header_lines.iter().cloned());

        // Footer (4 lines)
        let footer_lines = crate::widgets::footer::footer_widget(
            &self.input, &self.theme, self.focus.clone(), self.focus == FocusArea::Input, w,
        );

        // Body height
        let body_h = (h).saturating_sub(lines.len() + footer_lines.len());
        if body_h == 0 {
            lines.extend(footer_lines);
            return lines.join("\n");
        }

        // Two-pane layout calculation
        // Responsive breakpoints:
        //   >= 120: full two-pane (65/35)
        //   100-119: compressed (70/30)
        //   80-99: single pane + sidebar toggle (Ctrl+B)
        //   < 80: single pane only
        let default_show_sidebar = w >= 80;
        let show_sidebar = self.show_sidebar_override.unwrap_or(default_show_sidebar);

        if show_sidebar {
            let gap = 1; // 1 col gap between panes
            let sidebar_w = if w >= 120 {
                (w * 33) / 100  // 33% for wide terminals
            } else {
                (w * 28) / 100  // 28% for medium/narrow
            };
            // Ensure total fits exactly — chat gets the remainder
            let chat_w = w.saturating_sub(sidebar_w + gap);
            // Sanity: if rounding causes total > w, shrink sidebar
            let sidebar_w = w.saturating_sub(chat_w + gap);

            let chat_lines = crate::widgets::chat::chat_widget(
                &self.chat, &self.theme, self.focus == FocusArea::Chat, chat_w, body_h,
            );
            let sidebar_lines = crate::widgets::sidebar::sidebar_widget(
                &self.sidebar, &self.tasks, &self.theme,
                self.focus == FocusArea::Sidebar, sidebar_w, body_h,
            );

            // Merge side-by-side — both panes fitted to exact width
            for i in 0..body_h {
                let left = chat_lines.get(i).cloned().unwrap_or_default();
                let right = sidebar_lines.get(i).cloned().unwrap_or_default();
                let left_fitted = crate::widgets::fit_to_width(&left, chat_w);
                let right_fitted = crate::widgets::fit_to_width(&right, sidebar_w);
                lines.push(format!("{}{}{}", left_fitted, " ".repeat(gap), right_fitted));
            }
        } else {
            // Single pane: full-width chat
            let chat_lines = crate::widgets::chat::chat_widget(
                &self.chat, &self.theme, self.focus == FocusArea::Chat, w, body_h,
            );
            lines.extend(chat_lines);
        }

        // Footer
        lines.extend(footer_lines);

        // Modal overlay — replaces the entire screen when active
        if let Some(modal_kind) = self.modal.top() {
            match modal_kind {
                crate::state::modal::ModalKind::CommandPalette => {
                    let overlay = crate::widgets::command_palette::command_palette_widget(
                        &self.modal, &self.theme, w, h,
                    );
                    // Replace lines with overlay
                    lines = overlay;
                }
                crate::state::modal::ModalKind::ThreadPicker => {
                    lines = crate::widgets::thread_picker::thread_picker_widget(
                        &self.chat, &self.modal, &self.theme, w, h,
                    );
                }
                crate::state::modal::ModalKind::ApprovalOverlay => {
                    lines = crate::widgets::approval::approval_widget(
                        &self.approval, &self.theme, w, h,
                    );
                }
                crate::state::modal::ModalKind::Settings => {
                    lines = crate::widgets::settings::settings_widget(
                        &self.settings, &self.config, &self.theme, w, h,
                    );
                }
                crate::state::modal::ModalKind::ProviderPicker => {
                    lines = crate::widgets::provider_picker::provider_picker_widget(
                        &self.modal, &self.config, &self.theme, w, h,
                    );
                }
                crate::state::modal::ModalKind::ModelPicker => {
                    lines = crate::widgets::model_picker::model_picker_widget(
                        &self.modal, &self.config, &self.theme, w, h,
                    );
                }
                // Other modals will be added in later tasks
                _ => {}
            }
        }

        // Safety: truncate every line to screen width to prevent overflow
        let final_lines: Vec<String> = lines
            .into_iter()
            .map(|line| crate::widgets::truncate_to_width(&line, w))
            .collect();
        final_lines.join("\n")
    }
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

