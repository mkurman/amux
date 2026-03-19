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

        // Generic modal handling
        match code {
            KeyCode::Escape => {
                self.modal.reduce(modal::ModalAction::Pop);
                self.input.reduce(input::InputAction::Clear);
            }
            KeyCode::Char('j') | KeyCode::Down => {
                self.modal.reduce(modal::ModalAction::Navigate(1));
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.modal.reduce(modal::ModalAction::Navigate(-1));
            }
            KeyCode::Enter => {
                // Execute selected command
                if let Some(cmd) = self.modal.selected_command() {
                    let command = cmd.command.clone();
                    self.execute_command(&command);
                }
                self.modal.reduce(modal::ModalAction::Pop);
                self.input.reduce(input::InputAction::Clear);
            }
            _ => {}
        }

        Cmd::none()
    }

    fn execute_command(&mut self, command: &str) {
        match command {
            "provider" => self
                .modal
                .reduce(modal::ModalAction::Push(modal::ModalKind::ProviderPicker)),
            "model" => self
                .modal
                .reduce(modal::ModalAction::Push(modal::ModalKind::ModelPicker)),
            "tools" => self
                .modal
                .reduce(modal::ModalAction::Push(modal::ModalKind::ToolsPicker)),
            "effort" => self
                .modal
                .reduce(modal::ModalAction::Push(modal::ModalKind::EffortPicker)),
            "thread" => self
                .modal
                .reduce(modal::ModalAction::Push(modal::ModalKind::ThreadPicker)),
            "new" => self.chat.reduce(chat::ChatAction::NewThread),
            "settings" => self
                .modal
                .reduce(modal::ModalAction::Push(modal::ModalKind::Settings)),
            "view" => self
                .modal
                .reduce(modal::ModalAction::Push(modal::ModalKind::ViewPicker)),
            "quit" => {} // Will be handled as Cmd::quit() via normal mode 'q'
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
                Cmd::none()
            }
            Msg::Event(Event::Mouse(_mouse)) => {
                // Mouse handling -- placeholder for Phase 4
                Cmd::none()
            }
            _ => Cmd::none(),
        }
    }

    fn view_string(&self) -> String {
        let mut lines = Vec::new();
        let w = self.width as usize;

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
        let body_h = (self.height as usize).saturating_sub(lines.len() + footer_lines.len());
        if body_h == 0 {
            lines.extend(footer_lines);
            return lines.join("\n");
        }

        // Two-pane layout calculation
        let show_sidebar = w >= 80; // Hide sidebar below 80 cols

        if show_sidebar {
            let gap = 1; // 1 col gap between panes
            let sidebar_w = if w >= 120 {
                (w * 35) / 100  // 35% for wide terminals
            } else if w >= 100 {
                (w * 30) / 100  // 30% for medium
            } else {
                (w * 30) / 100  // 30% for narrow (80-99)
            };
            let chat_w = w.saturating_sub(sidebar_w + gap);

            let chat_lines = crate::widgets::chat::chat_widget(
                &self.chat, &self.theme, self.focus == FocusArea::Chat, chat_w, body_h,
            );
            let sidebar_lines = crate::widgets::sidebar::sidebar_widget(
                &self.sidebar, &self.tasks, &self.theme,
                self.focus == FocusArea::Sidebar, sidebar_w, body_h,
            );

            // Merge side-by-side
            for i in 0..body_h {
                let left = chat_lines.get(i).cloned().unwrap_or_default();
                let right = sidebar_lines.get(i).cloned().unwrap_or_default();
                // Pad left to chat_w visible chars, add gap, then right
                let left_padded = crate::widgets::pad_to_width(&left, chat_w);
                lines.push(format!("{}{}{}", left_padded, " ".repeat(gap), right));
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
                        &self.modal, &self.theme, w, self.height as usize,
                    );
                    // Replace lines with overlay
                    lines = overlay;
                }
                // Other modals will be added in later tasks
                _ => {}
            }
        }

        lines.join("\n")
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

// ── Helpers ──────────────────────────────────────────────────────────────────

fn truncate_or_pad(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{}{}", s, " ".repeat(width - s.len()))
    }
}

fn center_str(s: &str, width: usize) -> String {
    if s.len() >= width {
        return s[..width].to_string();
    }
    let pad = (width - s.len()) / 2;
    format!(
        "{}{}{}",
        " ".repeat(pad),
        s,
        " ".repeat(width - pad - s.len())
    )
}
