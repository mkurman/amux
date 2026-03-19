#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModalKind {
    CommandPalette,
    ThreadPicker,
    ProviderPicker,
    ModelPicker,
    ApprovalOverlay,
    Settings,
    EffortPicker,
    ToolsPicker,
    ViewPicker,
}

#[derive(Debug, Clone)]
pub struct CommandItem {
    pub command: String,
    pub description: String,
}

#[derive(Debug, Clone)]
pub enum ModalAction {
    Push(ModalKind),
    Pop,
    SetQuery(String),
    Navigate(i32),    // +1 = down, -1 = up
    Execute,
    FuzzyFilter,
}

pub struct ModalState {
    stack: Vec<ModalKind>,
    command_query: String,
    command_items: Vec<CommandItem>,
    filtered_indices: Vec<usize>,
    picker_cursor: usize,
}

impl ModalState {
    pub fn new() -> Self {
        let items = default_command_items();
        let filtered = (0..items.len()).collect();
        Self {
            stack: Vec::new(),
            command_query: String::new(),
            command_items: items,
            filtered_indices: filtered,
            picker_cursor: 0,
        }
    }

    // Accessors
    pub fn top(&self) -> Option<ModalKind> { self.stack.last().copied() }
    pub fn is_empty(&self) -> bool { self.stack.is_empty() }
    pub fn command_query(&self) -> &str { &self.command_query }
    pub fn command_items(&self) -> &[CommandItem] { &self.command_items }
    pub fn filtered_items(&self) -> &[usize] { &self.filtered_indices }
    pub fn picker_cursor(&self) -> usize { self.picker_cursor }

    pub fn reduce(&mut self, action: ModalAction) {
        match action {
            ModalAction::Push(kind) => {
                self.stack.push(kind);
                self.command_query.clear();
                self.picker_cursor = 0;
                self.refilter();
            }
            ModalAction::Pop => {
                self.stack.pop();
                self.command_query.clear();
                self.picker_cursor = 0;
                self.refilter();
            }
            ModalAction::SetQuery(query) => {
                self.command_query = query;
                self.refilter();
                self.picker_cursor = 0;
            }
            ModalAction::Navigate(delta) => {
                let len = self.filtered_indices.len();
                if len == 0 { return; }
                if delta > 0 {
                    self.picker_cursor = (self.picker_cursor + delta as usize).min(len - 1);
                } else {
                    self.picker_cursor = self.picker_cursor.saturating_sub((-delta) as usize);
                }
            }
            ModalAction::Execute => {
                // Execution is handled by the app layer — this just marks intent
            }
            ModalAction::FuzzyFilter => {
                self.refilter();
            }
        }
    }

    /// Get the currently selected command (if any)
    pub fn selected_command(&self) -> Option<&CommandItem> {
        self.filtered_indices.get(self.picker_cursor)
            .and_then(|&idx| self.command_items.get(idx))
    }

    fn refilter(&mut self) {
        let query = self.command_query.to_lowercase();
        if query.is_empty() {
            self.filtered_indices = (0..self.command_items.len()).collect();
        } else {
            // Strip leading '/' for matching
            let q = query.strip_prefix('/').unwrap_or(&query);
            self.filtered_indices = self.command_items.iter()
                .enumerate()
                .filter(|(_, item)| {
                    item.command.to_lowercase().contains(q)
                        || item.description.to_lowercase().contains(q)
                })
                .map(|(idx, _)| idx)
                .collect();
        }
    }
}

fn default_command_items() -> Vec<CommandItem> {
    vec![
        CommandItem { command: "provider".into(), description: "Switch LLM backend".into() },
        CommandItem { command: "model".into(), description: "Switch active model".into() },
        CommandItem { command: "tools".into(), description: "Toggle tool categories".into() },
        CommandItem { command: "effort".into(), description: "Set reasoning effort".into() },
        CommandItem { command: "thread".into(), description: "Pick conversation thread".into() },
        CommandItem { command: "new".into(), description: "New conversation".into() },
        CommandItem { command: "goal".into(), description: "Start a goal run".into() },
        CommandItem { command: "view".into(), description: "Switch transcript mode".into() },
        CommandItem { command: "settings".into(), description: "Open settings panel".into() },
        CommandItem { command: "prompt".into(), description: "Edit system prompt".into() },
        CommandItem { command: "quit".into(), description: "Exit TUI".into() },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_and_pop_modal() {
        let mut state = ModalState::new();
        assert!(state.top().is_none());
        state.reduce(ModalAction::Push(ModalKind::CommandPalette));
        assert_eq!(state.top(), Some(ModalKind::CommandPalette));
        state.reduce(ModalAction::Pop);
        assert!(state.top().is_none());
    }

    #[test]
    fn stacked_modals_pop_in_order() {
        let mut state = ModalState::new();
        state.reduce(ModalAction::Push(ModalKind::CommandPalette));
        state.reduce(ModalAction::Push(ModalKind::ProviderPicker));
        assert_eq!(state.top(), Some(ModalKind::ProviderPicker));
        state.reduce(ModalAction::Pop);
        assert_eq!(state.top(), Some(ModalKind::CommandPalette));
    }

    #[test]
    fn fuzzy_filter_narrows_items() {
        let mut state = ModalState::new();
        state.reduce(ModalAction::SetQuery("pro".into()));
        // "provider" and "prompt" should match "pro"
        let filtered_commands: Vec<&str> = state.filtered_items().iter()
            .map(|&idx| state.command_items()[idx].command.as_str())
            .collect();
        assert!(filtered_commands.contains(&"provider"));
        assert!(filtered_commands.contains(&"prompt"));
        assert!(!filtered_commands.contains(&"model"));
    }

    #[test]
    fn slash_prefix_stripped_for_matching() {
        let mut state = ModalState::new();
        state.reduce(ModalAction::SetQuery("/mod".into()));
        let filtered_commands: Vec<&str> = state.filtered_items().iter()
            .map(|&idx| state.command_items()[idx].command.as_str())
            .collect();
        assert!(filtered_commands.contains(&"model"));
    }

    #[test]
    fn navigation_clamps_to_bounds() {
        let mut state = ModalState::new();
        state.reduce(ModalAction::Navigate(-1));
        assert_eq!(state.picker_cursor(), 0);
        for _ in 0..100 {
            state.reduce(ModalAction::Navigate(1));
        }
        assert!(state.picker_cursor() < state.command_items().len());
    }

    #[test]
    fn selected_command_returns_correct_item() {
        let mut state = ModalState::new();
        state.reduce(ModalAction::Navigate(2));
        let selected = state.selected_command().unwrap();
        assert_eq!(selected.command, "tools");
    }

    #[test]
    fn push_resets_query_and_cursor() {
        let mut state = ModalState::new();
        state.reduce(ModalAction::SetQuery("test".into()));
        state.reduce(ModalAction::Navigate(3));
        state.reduce(ModalAction::Push(ModalKind::CommandPalette));
        assert_eq!(state.command_query(), "");
        assert_eq!(state.picker_cursor(), 0);
    }

    #[test]
    fn empty_filter_shows_all_items() {
        let state = ModalState::new();
        assert_eq!(state.filtered_items().len(), state.command_items().len());
    }
}
