#![allow(dead_code)]

// ── SettingsTab ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsTab {
    Provider,
    Tools,
    WebSearch,
    Reasoning,
    Gateway,
    Agent,
}

impl SettingsTab {
    const ALL: &'static [SettingsTab] = &[
        SettingsTab::Provider,
        SettingsTab::Tools,
        SettingsTab::WebSearch,
        SettingsTab::Reasoning,
        SettingsTab::Gateway,
        SettingsTab::Agent,
    ];

    pub fn all() -> &'static [SettingsTab] {
        Self::ALL
    }
}

// ── SettingsAction ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum SettingsAction {
    Open,
    Close,
    SwitchTab(SettingsTab),
    NavigateField(i32),
    EditField,
    ConfirmEdit,
    CancelEdit,
    InsertChar(char),
    Backspace,
    ToggleCheckbox,
    SelectRadio,
    OpenDropdown,
    NavigateDropdown(i32),
    SelectDropdown,
    Save,
}

// ── SettingsState ─────────────────────────────────────────────────────────────

pub struct SettingsState {
    active_tab: SettingsTab,
    field_cursor: usize,
    editing_field: Option<String>,
    edit_buffer: String,
    dropdown_open: bool,
    dropdown_cursor: usize,
    dirty: bool,
}

impl SettingsState {
    pub fn new() -> Self {
        Self {
            active_tab: SettingsTab::Provider,
            field_cursor: 0,
            editing_field: None,
            edit_buffer: String::new(),
            dropdown_open: false,
            dropdown_cursor: 0,
            dirty: false,
        }
    }

    pub fn active_tab(&self) -> SettingsTab {
        self.active_tab
    }

    pub fn field_cursor(&self) -> usize {
        self.field_cursor
    }

    pub fn editing_field(&self) -> Option<&str> {
        self.editing_field.as_deref()
    }

    pub fn is_editing(&self) -> bool {
        self.editing_field.is_some()
    }

    pub fn edit_buffer(&self) -> &str {
        &self.edit_buffer
    }

    pub fn is_dropdown_open(&self) -> bool {
        self.dropdown_open
    }

    pub fn dropdown_cursor(&self) -> usize {
        self.dropdown_cursor
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Start inline editing for a field, pre-populated with its current value.
    pub fn start_editing(&mut self, field: &str, current_value: &str) {
        self.editing_field = Some(field.to_string());
        self.edit_buffer = current_value.to_string();
    }

    /// Map `field_cursor` to the field name for the active tab.
    pub fn current_field_name(&self) -> &str {
        match self.active_tab {
            SettingsTab::Provider => match self.field_cursor {
                0 => "provider",
                1 => "base_url",
                2 => "api_key",
                3 => "model",
                4 => "reasoning_effort",
                _ => "",
            },
            SettingsTab::Tools => match self.field_cursor {
                0 => "tool_bash",
                1 => "tool_file_ops",
                2 => "tool_web_search",
                3 => "tool_web_browse",
                4 => "tool_vision",
                5 => "tool_system_info",
                6 => "tool_gateway",
                _ => "",
            },
            SettingsTab::WebSearch => match self.field_cursor {
                0 => "web_search_enabled",
                1 => "search_provider",
                2 => "firecrawl_api_key",
                3 => "exa_api_key",
                4 => "tavily_api_key",
                5 => "search_max_results",
                6 => "search_timeout",
                _ => "",
            },
            SettingsTab::Reasoning => match self.field_cursor {
                0 => "reasoning_effort",
                _ => "",
            },
            SettingsTab::Gateway => match self.field_cursor {
                0 => "gateway_enabled",
                1 => "slack_token",
                2 => "telegram_token",
                3 => "discord_token",
                4 => "gateway_prefix",
                _ => "",
            },
            SettingsTab::Agent => match self.field_cursor {
                0 => "agent_name",
                1 => "system_prompt",
                2 => "backend",
                _ => "",
            },
        }
    }

    /// Number of navigable fields in the current tab (for cursor clamping).
    pub fn field_count(&self) -> usize {
        match self.active_tab {
            SettingsTab::Provider => 5,
            SettingsTab::Tools => 7,
            SettingsTab::WebSearch => 7,
            SettingsTab::Reasoning => 1,
            SettingsTab::Gateway => 5,
            SettingsTab::Agent => 3,
        }
    }

    /// Navigate fields within `field_count` items; clamps at both ends.
    pub fn navigate_field(&mut self, delta: i32, field_count: usize) {
        if field_count == 0 {
            self.field_cursor = 0;
            return;
        }
        if delta > 0 {
            self.field_cursor =
                (self.field_cursor + delta as usize).min(field_count.saturating_sub(1));
        } else {
            self.field_cursor = self.field_cursor.saturating_sub((-delta) as usize);
        }
    }

    pub fn reduce(&mut self, action: SettingsAction) {
        match action {
            SettingsAction::Open => {
                self.active_tab = SettingsTab::Provider;
                self.field_cursor = 0;
                self.editing_field = None;
                self.edit_buffer.clear();
                self.dropdown_open = false;
                self.dropdown_cursor = 0;
                self.dirty = false;
            }

            SettingsAction::Close => {
                self.editing_field = None;
                self.edit_buffer.clear();
                self.dropdown_open = false;
            }

            SettingsAction::SwitchTab(tab) => {
                self.active_tab = tab;
                self.field_cursor = 0;
                self.editing_field = None;
                self.edit_buffer.clear();
                self.dropdown_open = false;
                self.dropdown_cursor = 0;
            }

            SettingsAction::NavigateField(delta) => {
                let count = self.field_count();
                if delta > 0 {
                    self.field_cursor =
                        (self.field_cursor + delta as usize).min(count.saturating_sub(1));
                } else {
                    self.field_cursor =
                        self.field_cursor.saturating_sub((-delta) as usize);
                }
            }

            SettingsAction::EditField => {
                let field_name = self.current_field_name().to_string();
                self.editing_field = Some(field_name);
                self.dirty = true;
            }

            SettingsAction::InsertChar(c) => {
                if self.editing_field.is_some() {
                    self.edit_buffer.push(c);
                }
            }

            SettingsAction::Backspace => {
                if self.editing_field.is_some() {
                    self.edit_buffer.pop();
                }
            }

            SettingsAction::ConfirmEdit => {
                self.editing_field = None;
                // edit_buffer is left intact so the caller can read the final value
                // before this action; it will be cleared on next start_editing/open.
            }

            SettingsAction::CancelEdit => {
                self.editing_field = None;
                self.edit_buffer.clear();
            }

            SettingsAction::ToggleCheckbox => {
                self.dirty = true;
            }

            SettingsAction::SelectRadio => {
                self.dirty = true;
            }

            SettingsAction::OpenDropdown => {
                self.dropdown_open = true;
                self.dropdown_cursor = 0;
            }

            SettingsAction::NavigateDropdown(delta) => {
                if self.dropdown_open {
                    if delta > 0 {
                        self.dropdown_cursor =
                            self.dropdown_cursor.saturating_add(delta as usize);
                    } else {
                        self.dropdown_cursor =
                            self.dropdown_cursor.saturating_sub((-delta) as usize);
                    }
                }
            }

            SettingsAction::SelectDropdown => {
                self.dropdown_open = false;
                self.dirty = true;
            }

            SettingsAction::Save => {
                self.dirty = false;
                self.editing_field = None;
                self.edit_buffer.clear();
            }
        }
    }
}

impl Default for SettingsState {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_resets_to_provider_tab() {
        let mut state = SettingsState::new();
        state.reduce(SettingsAction::SwitchTab(SettingsTab::Agent));
        state.reduce(SettingsAction::NavigateField(3));
        state.reduce(SettingsAction::EditField);
        assert_eq!(state.active_tab(), SettingsTab::Agent);
        assert!(state.is_dirty());

        state.reduce(SettingsAction::Open);
        assert_eq!(state.active_tab(), SettingsTab::Provider);
        assert_eq!(state.field_cursor(), 0);
        assert!(state.editing_field().is_none());
        assert!(!state.is_dirty());
    }

    #[test]
    fn switch_tab_resets_cursor_and_editing() {
        let mut state = SettingsState::new();
        state.reduce(SettingsAction::NavigateField(4));
        state.reduce(SettingsAction::EditField);
        assert!(state.editing_field().is_some());

        state.reduce(SettingsAction::SwitchTab(SettingsTab::Tools));
        assert_eq!(state.active_tab(), SettingsTab::Tools);
        assert_eq!(state.field_cursor(), 0);
        assert!(state.editing_field().is_none());
    }

    #[test]
    fn navigate_field_increases_cursor() {
        let mut state = SettingsState::new();
        state.reduce(SettingsAction::NavigateField(2));
        assert_eq!(state.field_cursor(), 2);
        state.reduce(SettingsAction::NavigateField(1));
        assert_eq!(state.field_cursor(), 3);
    }

    #[test]
    fn navigate_field_clamps_at_zero() {
        let mut state = SettingsState::new();
        state.reduce(SettingsAction::NavigateField(-10));
        assert_eq!(state.field_cursor(), 0);
    }

    #[test]
    fn navigate_field_clamps_at_max() {
        let mut state = SettingsState::new();
        // Provider tab has 5 fields (0..4)
        state.reduce(SettingsAction::NavigateField(100));
        assert_eq!(state.field_cursor(), 4);
    }

    #[test]
    fn navigate_field_method_clamps_at_max() {
        let mut state = SettingsState::new();
        state.navigate_field(100, 5);
        assert_eq!(state.field_cursor(), 4);
    }

    #[test]
    fn edit_field_sets_dirty() {
        let mut state = SettingsState::new();
        assert!(!state.is_dirty());
        state.reduce(SettingsAction::EditField);
        assert!(state.is_dirty());
        assert!(state.editing_field().is_some());
    }

    #[test]
    fn confirm_edit_clears_editing_field() {
        let mut state = SettingsState::new();
        state.reduce(SettingsAction::EditField);
        assert!(state.editing_field().is_some());
        state.reduce(SettingsAction::ConfirmEdit);
        assert!(state.editing_field().is_none());
        // dirty remains true until saved
        assert!(state.is_dirty());
    }

    #[test]
    fn cancel_edit_clears_editing_field() {
        let mut state = SettingsState::new();
        state.reduce(SettingsAction::EditField);
        state.reduce(SettingsAction::CancelEdit);
        assert!(state.editing_field().is_none());
    }

    #[test]
    fn save_clears_dirty_flag() {
        let mut state = SettingsState::new();
        state.reduce(SettingsAction::EditField);
        assert!(state.is_dirty());
        state.reduce(SettingsAction::Save);
        assert!(!state.is_dirty());
    }

    #[test]
    fn dropdown_open_and_navigate() {
        let mut state = SettingsState::new();
        assert!(!state.is_dropdown_open());
        state.reduce(SettingsAction::OpenDropdown);
        assert!(state.is_dropdown_open());
        assert_eq!(state.dropdown_cursor(), 0);

        state.reduce(SettingsAction::NavigateDropdown(2));
        assert_eq!(state.dropdown_cursor(), 2);
        state.reduce(SettingsAction::NavigateDropdown(-1));
        assert_eq!(state.dropdown_cursor(), 1);
    }

    #[test]
    fn select_dropdown_closes_and_sets_dirty() {
        let mut state = SettingsState::new();
        state.reduce(SettingsAction::OpenDropdown);
        state.reduce(SettingsAction::SelectDropdown);
        assert!(!state.is_dropdown_open());
        assert!(state.is_dirty());
    }

    #[test]
    fn close_clears_editing_and_dropdown() {
        let mut state = SettingsState::new();
        state.reduce(SettingsAction::EditField);
        state.reduce(SettingsAction::OpenDropdown);
        state.reduce(SettingsAction::Close);
        assert!(state.editing_field().is_none());
        assert!(!state.is_dropdown_open());
    }

    #[test]
    fn all_tabs_covers_six_variants() {
        assert_eq!(SettingsTab::all().len(), 6);
    }

    #[test]
    fn tab_cycling_through_all() {
        let mut state = SettingsState::new();
        for &tab in SettingsTab::all() {
            state.reduce(SettingsAction::SwitchTab(tab));
            assert_eq!(state.active_tab(), tab);
        }
    }

    #[test]
    fn insert_char_appends_to_edit_buffer() {
        let mut state = SettingsState::new();
        state.start_editing("base_url", "https://");
        assert!(state.is_editing());
        assert_eq!(state.edit_buffer(), "https://");

        state.reduce(SettingsAction::InsertChar('a'));
        state.reduce(SettingsAction::InsertChar('p'));
        state.reduce(SettingsAction::InsertChar('i'));
        assert_eq!(state.edit_buffer(), "https://api");
    }

    #[test]
    fn backspace_removes_last_char() {
        let mut state = SettingsState::new();
        state.start_editing("api_key", "sk-abc");
        state.reduce(SettingsAction::Backspace);
        assert_eq!(state.edit_buffer(), "sk-ab");
        state.reduce(SettingsAction::Backspace);
        assert_eq!(state.edit_buffer(), "sk-a");
    }

    #[test]
    fn backspace_on_empty_buffer_is_noop() {
        let mut state = SettingsState::new();
        state.start_editing("api_key", "");
        state.reduce(SettingsAction::Backspace);
        assert_eq!(state.edit_buffer(), "");
    }

    #[test]
    fn cancel_edit_clears_buffer() {
        let mut state = SettingsState::new();
        state.start_editing("base_url", "https://example.com");
        state.reduce(SettingsAction::InsertChar('!'));
        state.reduce(SettingsAction::CancelEdit);
        assert!(!state.is_editing());
        assert_eq!(state.edit_buffer(), "");
    }

    #[test]
    fn confirm_edit_keeps_buffer_value() {
        let mut state = SettingsState::new();
        state.start_editing("base_url", "https://");
        state.reduce(SettingsAction::InsertChar('x'));
        state.reduce(SettingsAction::ConfirmEdit);
        assert!(!state.is_editing());
        // Buffer still has value so caller can read it
        assert_eq!(state.edit_buffer(), "https://x");
    }

    #[test]
    fn current_field_name_provider_tab() {
        let mut state = SettingsState::new();
        assert_eq!(state.current_field_name(), "provider");
        state.reduce(SettingsAction::NavigateField(1));
        assert_eq!(state.current_field_name(), "base_url");
        state.reduce(SettingsAction::NavigateField(1));
        assert_eq!(state.current_field_name(), "api_key");
        state.reduce(SettingsAction::NavigateField(1));
        assert_eq!(state.current_field_name(), "model");
        state.reduce(SettingsAction::NavigateField(1));
        assert_eq!(state.current_field_name(), "reasoning_effort");
    }

    #[test]
    fn current_field_name_tools_tab() {
        let mut state = SettingsState::new();
        state.reduce(SettingsAction::SwitchTab(SettingsTab::Tools));
        assert_eq!(state.current_field_name(), "tool_bash");
        state.reduce(SettingsAction::NavigateField(1));
        assert_eq!(state.current_field_name(), "tool_file_ops");
        state.reduce(SettingsAction::NavigateField(1));
        assert_eq!(state.current_field_name(), "tool_web_search");
        state.reduce(SettingsAction::NavigateField(1));
        assert_eq!(state.current_field_name(), "tool_web_browse");
        state.reduce(SettingsAction::NavigateField(1));
        assert_eq!(state.current_field_name(), "tool_vision");
        state.reduce(SettingsAction::NavigateField(1));
        assert_eq!(state.current_field_name(), "tool_system_info");
        state.reduce(SettingsAction::NavigateField(1));
        assert_eq!(state.current_field_name(), "tool_gateway");
    }

    #[test]
    fn current_field_name_gateway_tab() {
        let mut state = SettingsState::new();
        state.reduce(SettingsAction::SwitchTab(SettingsTab::Gateway));
        assert_eq!(state.current_field_name(), "gateway_enabled");
        state.reduce(SettingsAction::NavigateField(1));
        assert_eq!(state.current_field_name(), "slack_token");
        state.reduce(SettingsAction::NavigateField(1));
        assert_eq!(state.current_field_name(), "telegram_token");
        state.reduce(SettingsAction::NavigateField(1));
        assert_eq!(state.current_field_name(), "discord_token");
        state.reduce(SettingsAction::NavigateField(1));
        assert_eq!(state.current_field_name(), "gateway_prefix");
    }

    #[test]
    fn current_field_name_agent_tab() {
        let mut state = SettingsState::new();
        state.reduce(SettingsAction::SwitchTab(SettingsTab::Agent));
        assert_eq!(state.current_field_name(), "agent_name");
        state.reduce(SettingsAction::NavigateField(1));
        assert_eq!(state.current_field_name(), "system_prompt");
        state.reduce(SettingsAction::NavigateField(1));
        assert_eq!(state.current_field_name(), "backend");
    }

    #[test]
    fn current_field_name_reasoning_tab() {
        let mut state = SettingsState::new();
        state.reduce(SettingsAction::SwitchTab(SettingsTab::Reasoning));
        assert_eq!(state.current_field_name(), "reasoning_effort");
        // Only 1 field, can't navigate past it
        state.reduce(SettingsAction::NavigateField(5));
        assert_eq!(state.current_field_name(), "reasoning_effort");
        assert_eq!(state.field_cursor(), 0);
    }

    #[test]
    fn field_count_per_tab() {
        let mut state = SettingsState::new();
        assert_eq!(state.field_count(), 5); // Provider (provider, base_url, api_key, model, effort)
        state.reduce(SettingsAction::SwitchTab(SettingsTab::Tools));
        assert_eq!(state.field_count(), 7); // 7 tool checkboxes
        state.reduce(SettingsAction::SwitchTab(SettingsTab::WebSearch));
        assert_eq!(state.field_count(), 7); // enabled, provider, 3 keys, max_results, timeout
        state.reduce(SettingsAction::SwitchTab(SettingsTab::Reasoning));
        assert_eq!(state.field_count(), 1); // effort only
        state.reduce(SettingsAction::SwitchTab(SettingsTab::Gateway));
        assert_eq!(state.field_count(), 5); // enabled, slack, telegram, discord, prefix
        state.reduce(SettingsAction::SwitchTab(SettingsTab::Agent));
        assert_eq!(state.field_count(), 3); // name, prompt, backend
    }

    #[test]
    fn current_field_name_websearch_tab() {
        let mut state = SettingsState::new();
        state.reduce(SettingsAction::SwitchTab(SettingsTab::WebSearch));
        assert_eq!(state.current_field_name(), "web_search_enabled");
        state.reduce(SettingsAction::NavigateField(1));
        assert_eq!(state.current_field_name(), "search_provider");
        state.reduce(SettingsAction::NavigateField(1));
        assert_eq!(state.current_field_name(), "firecrawl_api_key");
        state.reduce(SettingsAction::NavigateField(1));
        assert_eq!(state.current_field_name(), "exa_api_key");
        state.reduce(SettingsAction::NavigateField(1));
        assert_eq!(state.current_field_name(), "tavily_api_key");
        state.reduce(SettingsAction::NavigateField(1));
        assert_eq!(state.current_field_name(), "search_max_results");
        state.reduce(SettingsAction::NavigateField(1));
        assert_eq!(state.current_field_name(), "search_timeout");
    }

    #[test]
    fn insert_char_ignored_when_not_editing() {
        let mut state = SettingsState::new();
        state.reduce(SettingsAction::InsertChar('x'));
        assert_eq!(state.edit_buffer(), "");
    }
}
