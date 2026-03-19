// Temporary local copies until wire.rs rename (Task 9)
// These mirror the types in state.rs
#![allow(dead_code)]

#[derive(Debug, Clone, Default)]
pub struct AgentThread {
    pub id: String,
    pub title: String,
    pub created_at: u64,
    pub updated_at: u64,
    pub messages: Vec<AgentMessage>,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
}

#[derive(Debug, Clone, Default)]
pub struct AgentMessage {
    pub role: MessageRole,
    pub content: String,
    pub reasoning: Option<String>,
    pub tool_name: Option<String>,
    pub tool_arguments: Option<String>,
    pub tool_call_id: Option<String>,
    pub tool_status: Option<String>,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub tps: Option<f64>,
    pub generation_ms: Option<u64>,
    pub cost: Option<f64>,
    pub is_streaming: bool,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
    #[default]
    Unknown,
}

// ── TranscriptMode ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranscriptMode {
    Compact,
    Tools,
    Full,
}

// ── ToolCallStatus ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolCallStatus {
    Running,
    Done,
    Error,
}

// ── ToolCallVm ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ToolCallVm {
    pub call_id: String,
    pub name: String,
    pub arguments: String,
    pub status: ToolCallStatus,
    pub result: Option<String>,
    pub is_error: bool,
    pub started_at: u64,
}

// ── ChatAction ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum ChatAction {
    Delta { thread_id: String, content: String },
    Reasoning { thread_id: String, content: String },
    ToolCall { thread_id: String, call_id: String, name: String, args: String },
    ToolResult { thread_id: String, call_id: String, name: String, content: String, is_error: bool },
    TurnDone {
        thread_id: String,
        input_tokens: u64,
        output_tokens: u64,
        cost: Option<f64>,
        provider: Option<String>,
        model: Option<String>,
        tps: Option<f64>,
        generation_ms: Option<u64>,
    },
    ThreadListReceived(Vec<AgentThread>),
    ThreadDetailReceived(AgentThread),
    ThreadCreated { thread_id: String, title: String },
    SelectThread(String),
    ScrollChat(i32),
    NewThread,
    SetTranscriptMode(TranscriptMode),
}

// ── ChatState ─────────────────────────────────────────────────────────────────

pub struct ChatState {
    threads: Vec<AgentThread>,
    active_thread_id: Option<String>,
    streaming_content: String,
    streaming_reasoning: String,
    active_tool_calls: Vec<ToolCallVm>,
    scroll_offset: usize,
    scroll_locked: bool,
    transcript_mode: TranscriptMode,
}

impl ChatState {
    pub fn new() -> Self {
        Self {
            threads: Vec::new(),
            active_thread_id: None,
            streaming_content: String::new(),
            streaming_reasoning: String::new(),
            active_tool_calls: Vec::new(),
            scroll_offset: 0,
            scroll_locked: false,
            transcript_mode: TranscriptMode::Compact,
        }
    }

    pub fn threads(&self) -> &[AgentThread] {
        &self.threads
    }

    pub fn active_thread_id(&self) -> Option<&str> {
        self.active_thread_id.as_deref()
    }

    pub fn active_thread(&self) -> Option<&AgentThread> {
        let id = self.active_thread_id.as_deref()?;
        self.threads.iter().find(|t| t.id == id)
    }

    pub fn active_thread_mut(&mut self) -> Option<&mut AgentThread> {
        let id = self.active_thread_id.as_deref()?.to_owned();
        self.threads.iter_mut().find(|t| t.id == id)
    }

    pub fn streaming_content(&self) -> &str {
        &self.streaming_content
    }

    pub fn streaming_reasoning(&self) -> &str {
        &self.streaming_reasoning
    }

    pub fn active_tool_calls(&self) -> &[ToolCallVm] {
        &self.active_tool_calls
    }

    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    pub fn scroll_locked(&self) -> bool {
        self.scroll_locked
    }

    pub fn transcript_mode(&self) -> TranscriptMode {
        self.transcript_mode
    }

    pub fn is_streaming(&self) -> bool {
        !self.streaming_content.is_empty() || !self.streaming_reasoning.is_empty()
    }

    pub fn reduce(&mut self, action: ChatAction) {
        match action {
            ChatAction::Delta { thread_id, content } => {
                // Set active thread if not set, or if it matches the incoming thread
                if self.active_thread_id.is_none()
                    || self.active_thread_id.as_deref() == Some(&thread_id)
                {
                    self.active_thread_id = Some(thread_id);
                }
                self.streaming_content.push_str(&content);
            }

            ChatAction::Reasoning { thread_id: _, content } => {
                self.streaming_reasoning.push_str(&content);
            }

            ChatAction::ToolCall { thread_id: _, call_id, name, args } => {
                self.active_tool_calls.push(ToolCallVm {
                    call_id,
                    name,
                    arguments: args,
                    status: ToolCallStatus::Running,
                    result: None,
                    is_error: false,
                    started_at: 0,
                });
            }

            ChatAction::ToolResult { thread_id: _, call_id, name: _, content, is_error } => {
                if let Some(tc) = self.active_tool_calls.iter_mut().find(|tc| tc.call_id == call_id) {
                    tc.status = if is_error { ToolCallStatus::Error } else { ToolCallStatus::Done };
                    tc.result = Some(content);
                    tc.is_error = is_error;
                }
            }

            ChatAction::TurnDone {
                thread_id,
                input_tokens,
                output_tokens,
                cost,
                provider: _,
                model: _,
                tps,
                generation_ms,
            } => {
                // Only finalize if this is for the active thread
                if self.active_thread_id.as_deref() == Some(&thread_id) {
                    let content = std::mem::take(&mut self.streaming_content);
                    let reasoning = std::mem::take(&mut self.streaming_reasoning);

                    if !content.is_empty() || !reasoning.is_empty() {
                        let msg = AgentMessage {
                            role: MessageRole::Assistant,
                            content,
                            reasoning: if reasoning.is_empty() { None } else { Some(reasoning) },
                            input_tokens,
                            output_tokens,
                            tps,
                            generation_ms,
                            cost,
                            ..Default::default()
                        };

                        if let Some(thread) = self.threads.iter_mut().find(|t| t.id == thread_id) {
                            thread.messages.push(msg);
                            thread.total_input_tokens += input_tokens;
                            thread.total_output_tokens += output_tokens;
                        }
                    }

                    // Clear active tool calls on turn completion
                    self.active_tool_calls.clear();
                }
            }

            ChatAction::ThreadListReceived(new_threads) => {
                // Preserve active selection if the thread still exists
                if let Some(active_id) = &self.active_thread_id {
                    if !new_threads.iter().any(|t| &t.id == active_id) {
                        self.active_thread_id = None;
                    }
                }
                self.threads = new_threads;
            }

            ChatAction::ThreadDetailReceived(incoming) => {
                if let Some(existing) = self.threads.iter_mut().find(|t| t.id == incoming.id) {
                    // Merge: use incoming if it has more messages
                    if incoming.messages.len() >= existing.messages.len() {
                        *existing = incoming;
                    }
                } else {
                    self.threads.push(incoming);
                }
            }

            ChatAction::ThreadCreated { thread_id, title } => {
                let thread = AgentThread {
                    id: thread_id.clone(),
                    title,
                    ..Default::default()
                };
                self.threads.push(thread);
                self.active_thread_id = Some(thread_id);
            }

            ChatAction::SelectThread(thread_id) => {
                self.active_thread_id = Some(thread_id);
            }

            ChatAction::ScrollChat(delta) => {
                if delta > 0 {
                    self.scroll_offset = self.scroll_offset.saturating_add(delta as usize);
                    self.scroll_locked = true;
                } else if delta < 0 {
                    let decrease = (-delta) as usize;
                    self.scroll_offset = self.scroll_offset.saturating_sub(decrease);
                    if self.scroll_offset == 0 {
                        self.scroll_locked = false;
                    }
                }
            }

            ChatAction::NewThread => {
                self.active_thread_id = None;
            }

            ChatAction::SetTranscriptMode(mode) => {
                self.transcript_mode = mode;
            }
        }
    }
}

impl Default for ChatState {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delta_appends_to_streaming_content() {
        let mut state = ChatState::new();
        state.reduce(ChatAction::ThreadCreated { thread_id: "t1".into(), title: "Test".into() });
        state.reduce(ChatAction::Delta { thread_id: "t1".into(), content: "Hello".into() });
        state.reduce(ChatAction::Delta { thread_id: "t1".into(), content: " world".into() });
        assert_eq!(state.streaming_content(), "Hello world");
    }

    #[test]
    fn turn_done_finalizes_streaming_into_message() {
        let mut state = ChatState::new();
        state.reduce(ChatAction::ThreadCreated { thread_id: "t1".into(), title: "Test".into() });
        state.reduce(ChatAction::Delta { thread_id: "t1".into(), content: "Hi".into() });
        state.reduce(ChatAction::TurnDone {
            thread_id: "t1".into(),
            input_tokens: 100,
            output_tokens: 50,
            cost: Some(0.01),
            provider: Some("openai".into()),
            model: Some("gpt-4o".into()),
            tps: Some(45.0),
            generation_ms: Some(1200),
        });
        assert_eq!(state.streaming_content(), "");
        let thread = state.active_thread().unwrap();
        let last = thread.messages.last().unwrap();
        assert_eq!(last.content, "Hi");
        assert_eq!(last.role, MessageRole::Assistant);
    }

    #[test]
    fn scroll_up_locks_scroll() {
        let mut state = ChatState::new();
        state.reduce(ChatAction::ScrollChat(5));
        assert!(state.scroll_locked());
        assert_eq!(state.scroll_offset(), 5);
    }

    #[test]
    fn scroll_to_zero_unlocks() {
        let mut state = ChatState::new();
        state.reduce(ChatAction::ScrollChat(5));
        state.reduce(ChatAction::ScrollChat(-5));
        assert!(!state.scroll_locked());
        assert_eq!(state.scroll_offset(), 0);
    }

    #[test]
    fn thread_list_received_replaces_threads() {
        let mut state = ChatState::new();
        let threads = vec![
            AgentThread { id: "t1".into(), title: "First".into(), ..Default::default() },
            AgentThread { id: "t2".into(), title: "Second".into(), ..Default::default() },
        ];
        state.reduce(ChatAction::ThreadListReceived(threads));
        assert_eq!(state.threads().len(), 2);
    }

    #[test]
    fn tool_call_tracks_running_tool() {
        let mut state = ChatState::new();
        state.reduce(ChatAction::ThreadCreated { thread_id: "t1".into(), title: "Test".into() });
        state.reduce(ChatAction::ToolCall {
            thread_id: "t1".into(),
            call_id: "c1".into(),
            name: "bash_command".into(),
            args: "ls".into(),
        });
        assert_eq!(state.active_tool_calls().len(), 1);
        assert_eq!(state.active_tool_calls()[0].status, ToolCallStatus::Running);
    }

    #[test]
    fn tool_result_updates_status() {
        let mut state = ChatState::new();
        state.reduce(ChatAction::ThreadCreated { thread_id: "t1".into(), title: "Test".into() });
        state.reduce(ChatAction::ToolCall {
            thread_id: "t1".into(),
            call_id: "c1".into(),
            name: "bash_command".into(),
            args: "ls".into(),
        });
        state.reduce(ChatAction::ToolResult {
            thread_id: "t1".into(),
            call_id: "c1".into(),
            name: "bash_command".into(),
            content: "file.txt".into(),
            is_error: false,
        });
        assert_eq!(state.active_tool_calls()[0].status, ToolCallStatus::Done);
    }

    #[test]
    fn new_thread_clears_active() {
        let mut state = ChatState::new();
        state.reduce(ChatAction::ThreadCreated { thread_id: "t1".into(), title: "Test".into() });
        assert!(state.active_thread_id().is_some());
        state.reduce(ChatAction::NewThread);
        assert!(state.active_thread_id().is_none());
    }

    #[test]
    fn select_thread_changes_active() {
        let mut state = ChatState::new();
        state.reduce(ChatAction::ThreadListReceived(vec![
            AgentThread { id: "t1".into(), title: "First".into(), ..Default::default() },
            AgentThread { id: "t2".into(), title: "Second".into(), ..Default::default() },
        ]));
        state.reduce(ChatAction::SelectThread("t2".into()));
        assert_eq!(state.active_thread_id(), Some("t2"));
    }
}
