#![allow(dead_code)]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Insert,
}

#[derive(Debug, Clone)]
pub enum InputAction {
    InsertChar(char),
    Backspace,
    Submit,
    ToggleMode,
    Clear,
    InsertNewline,
}

pub struct InputState {
    buffer: String,
    mode: InputMode,
    submitted: Option<String>,
}

impl InputState {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            mode: InputMode::Insert, // Start in Insert mode
            submitted: None,
        }
    }

    pub fn buffer(&self) -> &str {
        &self.buffer
    }

    pub fn mode(&self) -> InputMode {
        self.mode
    }

    pub fn multiline(&self) -> bool {
        self.buffer.contains('\n')
    }

    pub fn take_submitted(&mut self) -> Option<String> {
        self.submitted.take()
    }

    pub fn set_mode(&mut self, mode: InputMode) {
        self.mode = mode;
    }

    pub fn reduce(&mut self, action: InputAction) {
        match action {
            InputAction::InsertChar(c) => self.buffer.push(c),
            InputAction::Backspace => { self.buffer.pop(); }
            InputAction::Submit => {
                if !self.buffer.trim().is_empty() {
                    self.submitted = Some(self.buffer.clone());
                    self.buffer.clear();
                }
            }
            InputAction::ToggleMode => {
                self.mode = match self.mode {
                    InputMode::Normal => InputMode::Insert,
                    InputMode::Insert => InputMode::Normal,
                };
            }
            InputAction::Clear => self.buffer.clear(),
            InputAction::InsertNewline => self.buffer.push('\n'),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_char_appends_to_buffer() {
        let mut state = InputState::new();
        state.reduce(InputAction::InsertChar('h'));
        state.reduce(InputAction::InsertChar('i'));
        assert_eq!(state.buffer(), "hi");
    }

    #[test]
    fn backspace_removes_last_char() {
        let mut state = InputState::new();
        state.reduce(InputAction::InsertChar('a'));
        state.reduce(InputAction::InsertChar('b'));
        state.reduce(InputAction::Backspace);
        assert_eq!(state.buffer(), "a");
    }

    #[test]
    fn backspace_on_empty_is_noop() {
        let mut state = InputState::new();
        state.reduce(InputAction::Backspace);
        assert_eq!(state.buffer(), "");
    }

    #[test]
    fn submit_returns_buffer_and_clears() {
        let mut state = InputState::new();
        state.reduce(InputAction::InsertChar('h'));
        state.reduce(InputAction::InsertChar('i'));
        let submitted = state.take_submitted();
        // submit hasn't been called yet
        assert!(submitted.is_none());

        state.reduce(InputAction::Submit);
        let submitted = state.take_submitted();
        assert_eq!(submitted, Some("hi".to_string()));
        assert_eq!(state.buffer(), "");
    }

    #[test]
    fn submit_empty_is_noop() {
        let mut state = InputState::new();
        state.reduce(InputAction::Submit);
        assert!(state.take_submitted().is_none());
    }

    #[test]
    fn submit_whitespace_only_is_noop() {
        let mut state = InputState::new();
        state.reduce(InputAction::InsertChar(' '));
        state.reduce(InputAction::InsertChar(' '));
        state.reduce(InputAction::Submit);
        assert!(state.take_submitted().is_none());
    }

    #[test]
    fn toggle_mode_switches_between_normal_and_insert() {
        let mut state = InputState::new();
        assert_eq!(state.mode(), InputMode::Insert);
        state.reduce(InputAction::ToggleMode);
        assert_eq!(state.mode(), InputMode::Normal);
        state.reduce(InputAction::ToggleMode);
        assert_eq!(state.mode(), InputMode::Insert);
    }

    #[test]
    fn newline_inserts_newline_char() {
        let mut state = InputState::new();
        state.reduce(InputAction::InsertChar('a'));
        state.reduce(InputAction::InsertNewline);
        state.reduce(InputAction::InsertChar('b'));
        assert_eq!(state.buffer(), "a\nb");
        assert!(state.multiline());
    }

    #[test]
    fn clear_empties_buffer() {
        let mut state = InputState::new();
        state.reduce(InputAction::InsertChar('x'));
        state.reduce(InputAction::Clear);
        assert_eq!(state.buffer(), "");
        assert!(!state.multiline());
    }
}
