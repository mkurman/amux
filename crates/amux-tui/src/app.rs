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
use crate::state::*;
use crate::theme::ThemeTokens;
use crate::widgets;

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
    #[allow(dead_code)]
    tick_counter: u64,

    // Vim motion state
    pending_g: bool,

    // Responsive layout override: when Some, overrides breakpoint-based sidebar visibility
    show_sidebar_override: Option<bool>,

    // Set by /quit command; checked after modal enter to issue quit
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

    // ── Rendering ─────────────────────────────────────────────────────────────

    pub fn render(&self, frame: &mut Frame) {
        let area = frame.area();
        let w = area.width;

        // Layout: header (3) + body (flex) + footer (4)
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),  // header
                Constraint::Min(1),    // body
                Constraint::Length(4), // footer
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

        // Render footer
        widgets::footer::render(
            frame,
            chunks[2],
            &self.input,
            &self.theme,
            &self.status_line,
            self.focus == FocusArea::Input,
        );

        // Modal overlay
        if let Some(modal_kind) = self.modal.top() {
            let overlay_area = match modal_kind {
                modal::ModalKind::Settings => centered_rect(75, 80, area),
                modal::ModalKind::ApprovalOverlay => centered_rect(60, 40, area),
                modal::ModalKind::CommandPalette => centered_rect(50, 40, area),
                modal::ModalKind::ThreadPicker => centered_rect(60, 50, area),
                modal::ModalKind::ProviderPicker => centered_rect(35, 50, area),
                modal::ModalKind::ModelPicker => centered_rect(45, 50, area),
                modal::ModalKind::EffortPicker => centered_rect(35, 30, area),
                modal::ModalKind::ToolsPicker | modal::ModalKind::ViewPicker => {
                    centered_rect(40, 35, area)
                }
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

    /// Returns true if the app should quit
    pub fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers) -> bool {
        // Modal takes priority
        if let Some(modal_kind) = self.modal.top() {
            return self.handle_key_modal(code, modifiers, modal_kind);
        }

        match self.input.mode() {
            input::InputMode::Normal => self.handle_key_normal(code, modifiers),
            input::InputMode::Insert => self.handle_key_insert(code, modifiers),
        }
    }

    fn handle_key_normal(&mut self, code: KeyCode, modifiers: KeyModifiers) -> bool {
        let ctrl = modifiers.contains(KeyModifiers::CONTROL);

        // Clear pending_g on any key that isn't 'g'
        if code != KeyCode::Char('g') {
            self.pending_g = false;
        }

        match code {
            KeyCode::Char('q') if !ctrl => return true,
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
                self.chat
                    .reduce(chat::ChatAction::ScrollChat(-(self.chat.scroll_offset() as i32)));
            }
            KeyCode::Char('g') if !ctrl => {
                if self.pending_g {
                    // gg = scroll to top
                    self.chat
                        .reduce(chat::ChatAction::ScrollChat(i32::MAX / 2));
                    self.pending_g = false;
                } else {
                    self.pending_g = true;
                }
                return false;
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
            KeyCode::Esc => {
                // Already in normal mode, no modal -- do nothing
            }
            _ => {}
        }

        false
    }

    fn handle_key_insert(&mut self, code: KeyCode, modifiers: KeyModifiers) -> bool {
        let ctrl = modifiers.contains(KeyModifiers::CONTROL);

        // Global shortcuts even in insert mode
        if ctrl {
            match code {
                KeyCode::Char('p') => {
                    self.modal
                        .reduce(modal::ModalAction::Push(modal::ModalKind::CommandPalette));
                    return false;
                }
                KeyCode::Char('t') => {
                    self.modal
                        .reduce(modal::ModalAction::Push(modal::ModalKind::ThreadPicker));
                    return false;
                }
                _ => {}
            }
        }

        match code {
            KeyCode::Esc => {
                self.input.set_mode(input::InputMode::Normal);
            }
            KeyCode::Enter => {
                // If command palette is open, Enter selects from palette
                if self.modal.top() == Some(modal::ModalKind::CommandPalette) {
                    self.handle_modal_enter(modal::ModalKind::CommandPalette);
                    if self.pending_quit {
                        self.pending_quit = false;
                        return true;
                    }
                    return false;
                }
                self.input.reduce(input::InputAction::Submit);
                if let Some(prompt) = self.input.take_submitted() {
                    // Slash commands: /command args
                    if prompt.starts_with('/') {
                        let cmd = prompt
                            .trim_start_matches('/')
                            .split_whitespace()
                            .next()
                            .unwrap_or("");
                        self.execute_command(cmd);
                    } else {
                        self.submit_prompt(prompt);
                    }
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

        false
    }

    fn handle_key_modal(
        &mut self,
        code: KeyCode,
        _modifiers: KeyModifiers,
        kind: modal::ModalKind,
    ) -> bool {
        // Settings modal: Tab cycles tabs, Esc closes
        if kind == modal::ModalKind::Settings {
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
            // Non-searchable modals: j/k navigate
            KeyCode::Char('j') if !is_searchable => {
                self.modal.reduce(modal::ModalAction::Navigate(1));
            }
            KeyCode::Char('k') if !is_searchable => {
                self.modal.reduce(modal::ModalAction::Navigate(-1));
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
                // Provider list is hardcoded in the widget; map cursor to provider name
                let providers = [
                    "openai",
                    "anthropic",
                    "groq",
                    "ollama",
                    "together",
                    "deepinfra",
                    "cerebras",
                    "zai",
                    "kimi",
                    "qwen",
                    "minimax",
                    "openrouter",
                    "custom",
                ];
                let cursor = self.modal.picker_cursor();
                if let Some(&provider) = providers.get(cursor) {
                    self.config
                        .reduce(config::ConfigAction::SetProvider(provider.to_string()));
                    // Repopulate models for new provider
                    let models = known_models_for_provider(provider);
                    self.config
                        .reduce(config::ConfigAction::ModelsFetched(models));
                    // Auto-select first model
                    if let Some(first) = known_models_for_provider(provider).first() {
                        self.config
                            .reduce(config::ConfigAction::SetModel(first.id.clone()));
                    }
                    self.status_line = format!("Provider: {}", provider);
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
            "provider" => self
                .modal
                .reduce(modal::ModalAction::Push(modal::ModalKind::ProviderPicker)),
            "model" => {
                // Populate with known models for current provider (offline, no daemon needed)
                let models = known_models_for_provider(&self.config.provider);
                if !models.is_empty() {
                    self.config
                        .reduce(config::ConfigAction::ModelsFetched(models));
                }
                self.modal
                    .reduce(modal::ModalAction::Push(modal::ModalKind::ModelPicker));
            }
            "tools" => {
                self.status_line = "Tools config: use /settings -> Tools tab".to_string();
            }
            "effort" => {
                self.modal
                    .reduce(modal::ModalAction::Push(modal::ModalKind::EffortPicker));
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

    pub fn handle_resize(&mut self, w: u16, h: u16) {
        self.width = w;
        self.height = h;
        // Clear sidebar override on resize so layout recalculates from breakpoints
        self.show_sidebar_override = None;
    }

    pub fn handle_mouse(&mut self, mouse: MouseEvent) {
        match mouse.kind {
            MouseEventKind::ScrollUp => match self.focus {
                FocusArea::Chat => self.chat.reduce(chat::ChatAction::ScrollChat(3)),
                FocusArea::Sidebar => {
                    self.sidebar.reduce(sidebar::SidebarAction::Scroll(3))
                }
                _ => {}
            },
            MouseEventKind::ScrollDown => match self.focus {
                FocusArea::Chat => self.chat.reduce(chat::ChatAction::ScrollChat(-3)),
                FocusArea::Sidebar => {
                    self.sidebar.reduce(sidebar::SidebarAction::Scroll(-3))
                }
                _ => {}
            },
            MouseEventKind::Down(MouseButton::Left) => {
                // Click-to-focus: determine which pane was clicked
                let sidebar_start = if self.width >= 80 {
                    (self.width as usize * 65 / 100) as u16
                } else {
                    self.width // no sidebar
                };
                if mouse.row >= 3 && mouse.row < self.height.saturating_sub(4) {
                    if mouse.column < sidebar_start {
                        self.focus = FocusArea::Chat;
                    } else {
                        self.focus = FocusArea::Sidebar;
                    }
                } else if mouse.row >= self.height.saturating_sub(4) {
                    self.focus = FocusArea::Input;
                    self.input.set_mode(input::InputMode::Insert);
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
        Span::styled("j/k", theme.fg_active),
        Span::styled(" nav  ", theme.fg_dim),
        Span::styled("Enter", theme.fg_active),
        Span::styled(" sel  ", theme.fg_dim),
        Span::styled("Esc", theme.fg_active),
        Span::styled(" close", theme.fg_dim),
    ]);
    frame.render_widget(Paragraph::new(hints), inner_chunks[1]);
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

// ── Offline model catalogue ───────────────────────────────────────────────────

/// Return a hardcoded list of known models for the given provider so the model
/// picker works without a live daemon fetch.
fn known_models_for_provider(provider: &str) -> Vec<config::FetchedModel> {
    let models: &[(&str, &str, u32)] = match provider {
        "openai" => &[
            ("gpt-4.1", "GPT-4.1", 1_000_000),
            ("gpt-4.1-mini", "GPT-4.1 Mini", 1_000_000),
            ("gpt-4.1-nano", "GPT-4.1 Nano", 1_000_000),
            ("o3", "o3", 200_000),
            ("o3-mini", "o3 Mini", 200_000),
            ("o4-mini", "o4 Mini", 200_000),
            ("gpt-4o", "GPT-4o", 128_000),
            ("gpt-4o-mini", "GPT-4o Mini", 128_000),
        ],
        "anthropic" => &[
            ("claude-opus-4-6", "Claude Opus 4.6", 1_000_000),
            ("claude-sonnet-4-6", "Claude Sonnet 4.6", 200_000),
            ("claude-haiku-4-5-20251001", "Claude Haiku 4.5", 200_000),
        ],
        "groq" => &[
            ("llama-3.3-70b-versatile", "Llama 3.3 70B", 128_000),
            ("llama-3.1-8b-instant", "Llama 3.1 8B", 131_072),
            ("gemma2-9b-it", "Gemma 2 9B", 8_192),
        ],
        "ollama" => &[
            ("llama3.3", "Llama 3.3", 128_000),
            ("qwen2.5-coder", "Qwen 2.5 Coder", 32_768),
            ("deepseek-r1", "DeepSeek R1", 64_000),
            ("mistral", "Mistral", 32_768),
        ],
        "together" => &[
            ("meta-llama/Llama-3.3-70B-Instruct-Turbo", "Llama 3.3 70B", 128_000),
            ("deepseek-ai/DeepSeek-R1", "DeepSeek R1", 64_000),
            ("Qwen/Qwen2.5-72B-Instruct-Turbo", "Qwen 2.5 72B", 32_768),
        ],
        "deepinfra" => &[
            ("meta-llama/Llama-3.3-70B-Instruct", "Llama 3.3 70B", 128_000),
            ("Qwen/Qwen2.5-Coder-32B-Instruct", "Qwen 2.5 Coder 32B", 32_768),
        ],
        "zai" => &[
            ("glm-4-plus", "GLM-4 Plus", 128_000),
            ("glm-4-air", "GLM-4 Air", 128_000),
            ("glm-4-flash", "GLM-4 Flash", 128_000),
        ],
        "kimi" => &[
            ("moonshot-v1-128k", "Moonshot v1 128K", 128_000),
            ("moonshot-v1-32k", "Moonshot v1 32K", 32_768),
        ],
        "qwen" => &[
            ("qwen-max", "Qwen Max", 32_768),
            ("qwen-plus", "Qwen Plus", 131_072),
            ("qwen-turbo", "Qwen Turbo", 131_072),
        ],
        "openrouter" => &[
            ("anthropic/claude-opus-4-6", "Claude Opus 4.6", 1_000_000),
            ("openai/gpt-4.1", "GPT-4.1", 1_000_000),
            ("google/gemini-2.5-pro", "Gemini 2.5 Pro", 1_000_000),
            ("meta-llama/llama-3.3-70b-instruct", "Llama 3.3 70B", 128_000),
        ],
        _ => &[],
    };
    models
        .iter()
        .map(|(id, name, ctx)| config::FetchedModel {
            id: id.to_string(),
            name: Some(name.to_string()),
            context_window: Some(*ctx),
        })
        .collect()
}
