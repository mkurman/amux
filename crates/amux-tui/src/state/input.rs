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
    DeleteWord,   // delete word before cursor (Ctrl+Backspace / Ctrl+W)
    ClearLine,    // clear entire input line (Ctrl+U)
    Submit,
    ToggleMode,
    Clear,
    InsertNewline,
    MoveCursorLeft,
    MoveCursorRight,
    MoveCursorUp,
    MoveCursorDown,
    /// Move cursor up one visual line (accounting for text wrapping at given width)
    MoveCursorUpVisual(usize),
    /// Move cursor down one visual line
    MoveCursorDownVisual(usize),
    MoveCursorHome,
    MoveCursorEnd,
    MoveCursorToPos(usize),
    Undo,
    Redo,
}

/// A pasted multi-line block stored out-of-band; a placeholder token is kept
/// in the buffer and expanded to the full content on submit.
#[derive(Debug, Clone)]
pub struct PasteBlock {
    pub id: usize,
    pub content: String,
    pub line_count: usize,
}

pub struct InputState {
    buffer: String,
    cursor: usize,           // byte offset in buffer
    mode: InputMode,
    submitted: Option<String>,
    undo_stack: Vec<String>,  // previous buffer states
    redo_stack: Vec<String>,  // for redo after undo
    paste_blocks: Vec<PasteBlock>,
    next_paste_id: usize,
}

impl InputState {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            cursor: 0,
            mode: InputMode::Insert, // Start in Insert mode
            submitted: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            paste_blocks: Vec::new(),
            next_paste_id: 0,
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

    /// Get cursor byte offset
    pub fn cursor_pos(&self) -> usize {
        self.cursor
    }

    /// Get cursor (line, col) for rendering
    pub fn cursor_line_col_public(&self) -> (usize, usize) {
        self.cursor_line_col()
    }

    /// Convert (line, col) to byte offset (public, for mouse click positioning)
    pub fn line_col_to_offset_public(&self, line: usize, col: usize) -> usize {
        self.line_col_to_offset(line, col)
    }

    /// Return the stored paste blocks (for rendering).
    pub fn paste_blocks(&self) -> &[PasteBlock] {
        &self.paste_blocks
    }

    /// Insert a paste block at the current cursor position.
    ///
    /// Single-line pastes are inserted character-by-character as normal text.
    /// Multi-line pastes store the full content in `paste_blocks` and insert
    /// a NUL-delimited placeholder token (`\x00PASTE:N\x00`) into the buffer
    /// so the display layer can render a collapsed amber preview.
    pub fn insert_paste_block(&mut self, content: String) {
        self.save_undo();
        let line_count = content.lines().count();
        if line_count <= 1 {
            // Single-line paste: just insert normally
            for c in content.chars() {
                self.buffer.insert(self.cursor, c);
                self.cursor += c.len_utf8();
            }
            return;
        }
        let id = self.next_paste_id;
        self.next_paste_id += 1;
        let placeholder = format!("\x00PASTE:{}\x00", id);
        for c in placeholder.chars() {
            self.buffer.insert(self.cursor, c);
            self.cursor += c.len_utf8();
        }
        self.paste_blocks.push(PasteBlock { id, content, line_count });
    }

    /// Expand all paste-block placeholder tokens in `text`, replacing them
    /// with their full content.  Called on submit so the message sent to the
    /// daemon contains the real text.
    pub fn expand_paste_blocks(&self, text: &str) -> String {
        let mut result = text.to_string();
        for block in &self.paste_blocks {
            let placeholder = format!("\x00PASTE:{}\x00", block.id);
            result = result.replace(&placeholder, &block.content);
        }
        result
    }

    /// Build the amber display label for a paste-block placeholder.
    /// Returns `None` when no block with the given `id` exists.
    pub fn paste_block_display(id: usize, blocks: &[PasteBlock]) -> Option<String> {
        blocks.iter().find(|b| b.id == id).map(|b| {
            let first_line = b.content.lines().next().unwrap_or("");
            let truncated: String = first_line.chars().take(20).collect();
            let ellipsis = if first_line.chars().count() > 20 { "..." } else { "" };
            format!(
                "[Pasted text #{} +{} lines: {}{}]",
                b.id + 1,
                b.line_count,
                truncated,
                ellipsis
            )
        })
    }

    /// Get (line_index, col_index) for current cursor position
    fn cursor_line_col(&self) -> (usize, usize) {
        let before = &self.buffer[..self.cursor];
        let line = before.matches('\n').count();
        let last_newline = before.rfind('\n').map(|i| i + 1).unwrap_or(0);
        let col = self.cursor - last_newline;
        (line, col)
    }

    /// Convert (line, col) to byte offset, clamping col to line length
    fn line_col_to_offset(&self, line: usize, col: usize) -> usize {
        let mut offset = 0;
        for (i, line_str) in self.buffer.split('\n').enumerate() {
            if i == line {
                return offset + col.min(line_str.len());
            }
            offset += line_str.len() + 1; // +1 for \n
        }
        self.buffer.len() // past end
    }

    /// Save current buffer state for undo
    fn save_undo(&mut self) {
        if self.undo_stack.last().map(|s| s.as_str()) != Some(&self.buffer) {
            self.undo_stack.push(self.buffer.clone());
            if self.undo_stack.len() > 100 {
                self.undo_stack.remove(0);
            }
            self.redo_stack.clear();
        }
    }

    /// Get cursor position in visual (wrapped) line coordinates.
    /// Each logical line is split into visual lines of `wrap_width` chars.
    fn cursor_visual_line_col(&self, wrap_width: usize) -> (usize, usize) {
        let mut vis_line = 0;
        let mut offset = 0;
        for logical_line in self.buffer.split('\n') {
            let len = logical_line.chars().count();
            let vis_lines_in_this = if wrap_width == 0 || len == 0 { 1 } else { (len + wrap_width - 1) / wrap_width };

            let line_start = offset;
            let line_end = offset + logical_line.len();

            if self.cursor >= line_start && self.cursor <= line_end {
                // Cursor is in this logical line
                let chars_before = self.buffer[line_start..self.cursor].chars().count();
                let vis_line_in_this = chars_before / wrap_width.max(1);
                let vis_col = chars_before % wrap_width.max(1);
                return (vis_line + vis_line_in_this, vis_col);
            }

            vis_line += vis_lines_in_this;
            offset = line_end + 1; // +1 for \n
        }
        (vis_line, 0)
    }

    /// Total number of visual lines when wrapping at `wrap_width`.
    fn total_visual_lines(&self, wrap_width: usize) -> usize {
        self.buffer.split('\n')
            .map(|line| {
                let len = line.chars().count();
                if wrap_width == 0 || len == 0 { 1 } else { (len + wrap_width - 1) / wrap_width }
            })
            .sum()
    }

    /// Convert visual (wrapped) line + col to byte offset.
    fn visual_line_col_to_offset(&self, target_vis_line: usize, target_col: usize, wrap_width: usize) -> usize {
        let mut vis_line = 0;
        let mut offset = 0;
        for logical_line in self.buffer.split('\n') {
            let len = logical_line.chars().count();
            let vis_lines_in_this = if wrap_width == 0 || len == 0 { 1 } else { (len + wrap_width - 1) / wrap_width };

            if target_vis_line >= vis_line && target_vis_line < vis_line + vis_lines_in_this {
                // Target is in this logical line
                let vis_line_within = target_vis_line - vis_line;
                let char_offset = vis_line_within * wrap_width + target_col.min(wrap_width - 1);
                let clamped = char_offset.min(len);
                // Convert char offset to byte offset
                let byte_offset: usize = logical_line.chars().take(clamped).map(|c| c.len_utf8()).sum();
                return offset + byte_offset;
            }

            vis_line += vis_lines_in_this;
            offset += logical_line.len() + 1; // +1 for \n
        }
        self.buffer.len()
    }

    pub fn reduce(&mut self, action: InputAction) {
        match action {
            InputAction::InsertChar(c) => {
                self.save_undo();
                self.buffer.insert(self.cursor, c);
                self.cursor += c.len_utf8();
            }
            InputAction::Backspace => {
                if self.cursor > 0 {
                    self.save_undo();
                    let prev = self.buffer[..self.cursor]
                        .char_indices()
                        .last()
                        .map(|(i, _)| i)
                        .unwrap_or(0);
                    self.buffer.drain(prev..self.cursor);
                    self.cursor = prev;
                }
            }
            InputAction::DeleteWord => {
                if self.cursor > 0 {
                    self.save_undo();
                    let before = &self.buffer[..self.cursor];
                    // Skip trailing whitespace before cursor
                    let trimmed_end = before.trim_end_matches(|c: char| c.is_whitespace());
                    if trimmed_end.is_empty() {
                        // All whitespace before cursor
                        self.buffer.drain(..self.cursor);
                        self.cursor = 0;
                    } else if let Some(last_space) =
                        trimmed_end.rfind(|c: char| c.is_whitespace())
                    {
                        let new_cursor = last_space + 1;
                        self.buffer.drain(new_cursor..self.cursor);
                        self.cursor = new_cursor;
                    } else {
                        // Single word: delete everything before cursor
                        self.buffer.drain(..self.cursor);
                        self.cursor = 0;
                    }
                }
            }
            InputAction::ClearLine => {
                self.save_undo();
                self.buffer.clear();
                self.cursor = 0;
                self.paste_blocks.clear();
            }
            InputAction::Submit => {
                if !self.buffer.trim().is_empty() {
                    let expanded = self.expand_paste_blocks(&self.buffer);
                    self.submitted = Some(expanded);
                    self.buffer.clear();
                    self.cursor = 0;
                    self.undo_stack.clear();
                    self.redo_stack.clear();
                    self.paste_blocks.clear();
                }
            }
            InputAction::ToggleMode => {
                self.mode = match self.mode {
                    InputMode::Normal => InputMode::Insert,
                    InputMode::Insert => InputMode::Normal,
                };
            }
            InputAction::Clear => {
                self.save_undo();
                self.buffer.clear();
                self.cursor = 0;
                self.paste_blocks.clear();
            }
            InputAction::InsertNewline => {
                self.save_undo();
                self.buffer.insert(self.cursor, '\n');
                self.cursor += 1;
            }
            InputAction::MoveCursorLeft => {
                if self.cursor > 0 {
                    let prev = self.buffer[..self.cursor]
                        .char_indices()
                        .last()
                        .map(|(i, _)| i)
                        .unwrap_or(0);
                    self.cursor = prev;
                }
            }
            InputAction::MoveCursorRight => {
                if self.cursor < self.buffer.len() {
                    let next = self.buffer[self.cursor..]
                        .char_indices()
                        .nth(1)
                        .map(|(i, _)| self.cursor + i)
                        .unwrap_or(self.buffer.len());
                    self.cursor = next;
                }
            }
            InputAction::MoveCursorUp => {
                let (line, col) = self.cursor_line_col();
                if line > 0 {
                    self.cursor = self.line_col_to_offset(line - 1, col);
                }
            }
            InputAction::MoveCursorDown => {
                let (line, col) = self.cursor_line_col();
                let line_count = self.buffer.matches('\n').count() + 1;
                if line + 1 < line_count {
                    self.cursor = self.line_col_to_offset(line + 1, col);
                }
            }
            InputAction::MoveCursorUpVisual(wrap_width) => {
                if wrap_width == 0 {
                    return;
                }
                // Find cursor's position in visual (wrapped) coordinates
                let (vis_line, vis_col) = self.cursor_visual_line_col(wrap_width);
                if vis_line > 0 {
                    self.cursor = self.visual_line_col_to_offset(vis_line - 1, vis_col, wrap_width);
                }
            }
            InputAction::MoveCursorDownVisual(wrap_width) => {
                if wrap_width == 0 {
                    return;
                }
                let (vis_line, vis_col) = self.cursor_visual_line_col(wrap_width);
                let total_vis_lines = self.total_visual_lines(wrap_width);
                if vis_line + 1 < total_vis_lines {
                    self.cursor = self.visual_line_col_to_offset(vis_line + 1, vis_col, wrap_width);
                }
            }
            InputAction::MoveCursorHome => {
                let before = &self.buffer[..self.cursor];
                self.cursor = before.rfind('\n').map(|i| i + 1).unwrap_or(0);
            }
            InputAction::MoveCursorEnd => {
                let after = &self.buffer[self.cursor..];
                if let Some(nl) = after.find('\n') {
                    self.cursor += nl;
                } else {
                    self.cursor = self.buffer.len();
                }
            }
            InputAction::MoveCursorToPos(pos) => {
                self.cursor = pos.min(self.buffer.len());
            }
            InputAction::Undo => {
                if let Some(prev) = self.undo_stack.pop() {
                    self.redo_stack.push(self.buffer.clone());
                    self.buffer = prev;
                    self.cursor = self.cursor.min(self.buffer.len());
                }
            }
            InputAction::Redo => {
                if let Some(next) = self.redo_stack.pop() {
                    self.undo_stack.push(self.buffer.clone());
                    self.buffer = next;
                    self.cursor = self.cursor.min(self.buffer.len());
                }
            }
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
        assert_eq!(state.cursor_pos(), 2);
    }

    #[test]
    fn backspace_removes_char_before_cursor() {
        let mut state = InputState::new();
        state.reduce(InputAction::InsertChar('a'));
        state.reduce(InputAction::InsertChar('b'));
        state.reduce(InputAction::Backspace);
        assert_eq!(state.buffer(), "a");
        assert_eq!(state.cursor_pos(), 1);
    }

    #[test]
    fn backspace_on_empty_is_noop() {
        let mut state = InputState::new();
        state.reduce(InputAction::Backspace);
        assert_eq!(state.buffer(), "");
        assert_eq!(state.cursor_pos(), 0);
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
        assert_eq!(state.cursor_pos(), 0);
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
        assert_eq!(state.cursor_pos(), 3);
    }

    #[test]
    fn clear_empties_buffer() {
        let mut state = InputState::new();
        state.reduce(InputAction::InsertChar('x'));
        state.reduce(InputAction::Clear);
        assert_eq!(state.buffer(), "");
        assert!(!state.multiline());
        assert_eq!(state.cursor_pos(), 0);
    }

    #[test]
    fn delete_word_removes_last_word() {
        let mut state = InputState::new();
        for c in "hello world".chars() {
            state.reduce(InputAction::InsertChar(c));
        }
        state.reduce(InputAction::DeleteWord);
        assert_eq!(state.buffer(), "hello ");
        assert_eq!(state.cursor_pos(), 6);
    }

    #[test]
    fn delete_word_on_single_word_clears() {
        let mut state = InputState::new();
        for c in "hello".chars() {
            state.reduce(InputAction::InsertChar(c));
        }
        state.reduce(InputAction::DeleteWord);
        assert_eq!(state.buffer(), "");
        assert_eq!(state.cursor_pos(), 0);
    }

    #[test]
    fn delete_word_on_empty_is_noop() {
        let mut state = InputState::new();
        state.reduce(InputAction::DeleteWord);
        assert_eq!(state.buffer(), "");
    }

    #[test]
    fn delete_word_with_trailing_spaces() {
        let mut state = InputState::new();
        for c in "hello world  ".chars() {
            state.reduce(InputAction::InsertChar(c));
        }
        state.reduce(InputAction::DeleteWord);
        assert_eq!(state.buffer(), "hello ");
        assert_eq!(state.cursor_pos(), 6);
    }

    #[test]
    fn clear_line_empties_buffer() {
        let mut state = InputState::new();
        for c in "some text here".chars() {
            state.reduce(InputAction::InsertChar(c));
        }
        state.reduce(InputAction::ClearLine);
        assert_eq!(state.buffer(), "");
        assert_eq!(state.cursor_pos(), 0);
    }

    // ── Cursor movement tests ────────────────────────────────────────────────

    #[test]
    fn cursor_left_right() {
        let mut state = InputState::new();
        for c in "abc".chars() {
            state.reduce(InputAction::InsertChar(c));
        }
        assert_eq!(state.cursor_pos(), 3);

        state.reduce(InputAction::MoveCursorLeft);
        assert_eq!(state.cursor_pos(), 2);

        state.reduce(InputAction::MoveCursorLeft);
        assert_eq!(state.cursor_pos(), 1);

        state.reduce(InputAction::MoveCursorRight);
        assert_eq!(state.cursor_pos(), 2);
    }

    #[test]
    fn cursor_left_at_start_is_noop() {
        let mut state = InputState::new();
        state.reduce(InputAction::InsertChar('a'));
        state.reduce(InputAction::MoveCursorLeft);
        state.reduce(InputAction::MoveCursorLeft); // already at 0
        assert_eq!(state.cursor_pos(), 0);
    }

    #[test]
    fn cursor_right_at_end_is_noop() {
        let mut state = InputState::new();
        state.reduce(InputAction::InsertChar('a'));
        state.reduce(InputAction::MoveCursorRight); // already at end
        assert_eq!(state.cursor_pos(), 1);
    }

    #[test]
    fn insert_at_middle_of_buffer() {
        let mut state = InputState::new();
        for c in "ac".chars() {
            state.reduce(InputAction::InsertChar(c));
        }
        // Cursor is at 2, move left once to position 1
        state.reduce(InputAction::MoveCursorLeft);
        assert_eq!(state.cursor_pos(), 1);

        // Insert 'b' at position 1
        state.reduce(InputAction::InsertChar('b'));
        assert_eq!(state.buffer(), "abc");
        assert_eq!(state.cursor_pos(), 2);
    }

    #[test]
    fn backspace_at_middle_of_buffer() {
        let mut state = InputState::new();
        for c in "abc".chars() {
            state.reduce(InputAction::InsertChar(c));
        }
        // Move to position 2 (between 'b' and 'c')
        state.reduce(InputAction::MoveCursorLeft);
        assert_eq!(state.cursor_pos(), 2);

        // Backspace deletes 'b'
        state.reduce(InputAction::Backspace);
        assert_eq!(state.buffer(), "ac");
        assert_eq!(state.cursor_pos(), 1);
    }

    #[test]
    fn cursor_up_down_multiline() {
        let mut state = InputState::new();
        // Build "abc\ndef\nghi"
        for c in "abc".chars() {
            state.reduce(InputAction::InsertChar(c));
        }
        state.reduce(InputAction::InsertNewline);
        for c in "def".chars() {
            state.reduce(InputAction::InsertChar(c));
        }
        state.reduce(InputAction::InsertNewline);
        for c in "ghi".chars() {
            state.reduce(InputAction::InsertChar(c));
        }
        // Cursor at end of "ghi" (line 2, col 3)
        assert_eq!(state.cursor_pos(), 11);

        // Move up to line 1 (same col 3 => end of "def")
        state.reduce(InputAction::MoveCursorUp);
        let (line, col) = state.cursor_line_col_public();
        assert_eq!(line, 1);
        assert_eq!(col, 3);

        // Move up to line 0
        state.reduce(InputAction::MoveCursorUp);
        let (line, col) = state.cursor_line_col_public();
        assert_eq!(line, 0);
        assert_eq!(col, 3);

        // Move up again — already at line 0, should stay
        state.reduce(InputAction::MoveCursorUp);
        let (line, _) = state.cursor_line_col_public();
        assert_eq!(line, 0);

        // Move down to line 1
        state.reduce(InputAction::MoveCursorDown);
        let (line, _) = state.cursor_line_col_public();
        assert_eq!(line, 1);

        // Move down to line 2
        state.reduce(InputAction::MoveCursorDown);
        let (line, _) = state.cursor_line_col_public();
        assert_eq!(line, 2);

        // Move down again — already at last line, should stay
        state.reduce(InputAction::MoveCursorDown);
        let (line, _) = state.cursor_line_col_public();
        assert_eq!(line, 2);
    }

    #[test]
    fn cursor_up_clamps_col_to_shorter_line() {
        let mut state = InputState::new();
        // Build "abcdef\nhi" — line 0 has 6 chars, line 1 has 2 chars
        for c in "abcdef".chars() {
            state.reduce(InputAction::InsertChar(c));
        }
        state.reduce(InputAction::InsertNewline);
        for c in "hi".chars() {
            state.reduce(InputAction::InsertChar(c));
        }
        // Cursor at end of line 1, col 2
        assert_eq!(state.cursor_pos(), 9);

        // Move to end of line 0 (col 6) via Home on line 1 then Up
        // Actually: go up from col 2 => line 0 col 2
        state.reduce(InputAction::MoveCursorUp);
        let (line, col) = state.cursor_line_col_public();
        assert_eq!(line, 0);
        assert_eq!(col, 2);

        // Move cursor to end of line 0 (col 6)
        state.reduce(InputAction::MoveCursorEnd);
        let (_, col) = state.cursor_line_col_public();
        assert_eq!(col, 6);

        // Move down to line 1 — col 6 should clamp to line 1 length (2)
        state.reduce(InputAction::MoveCursorDown);
        let (line, col) = state.cursor_line_col_public();
        assert_eq!(line, 1);
        assert_eq!(col, 2); // clamped
    }

    #[test]
    fn home_and_end() {
        let mut state = InputState::new();
        for c in "abc\ndef".chars() {
            state.reduce(InputAction::InsertChar(c));
        }
        // Cursor at end of "def" (line 1, col 3)
        assert_eq!(state.cursor_pos(), 7);

        // Home moves to start of line 1 (position 4)
        state.reduce(InputAction::MoveCursorHome);
        assert_eq!(state.cursor_pos(), 4);

        // End moves to end of line 1 (position 7)
        state.reduce(InputAction::MoveCursorEnd);
        assert_eq!(state.cursor_pos(), 7);

        // Move up to line 0
        state.reduce(InputAction::MoveCursorUp);
        // Home on line 0 moves to position 0
        state.reduce(InputAction::MoveCursorHome);
        assert_eq!(state.cursor_pos(), 0);

        // End on line 0 moves to position 3 (before newline)
        state.reduce(InputAction::MoveCursorEnd);
        assert_eq!(state.cursor_pos(), 3);
    }

    #[test]
    fn move_cursor_to_pos() {
        let mut state = InputState::new();
        for c in "hello".chars() {
            state.reduce(InputAction::InsertChar(c));
        }
        state.reduce(InputAction::MoveCursorToPos(2));
        assert_eq!(state.cursor_pos(), 2);

        // Clamp to buffer length
        state.reduce(InputAction::MoveCursorToPos(100));
        assert_eq!(state.cursor_pos(), 5);
    }

    // ── Undo / Redo tests ────────────────────────────────────────────────────

    #[test]
    fn undo_restores_previous_state() {
        let mut state = InputState::new();
        state.reduce(InputAction::InsertChar('a'));
        state.reduce(InputAction::InsertChar('b'));
        assert_eq!(state.buffer(), "ab");

        state.reduce(InputAction::Undo);
        assert_eq!(state.buffer(), "a");

        state.reduce(InputAction::Undo);
        assert_eq!(state.buffer(), "");
    }

    #[test]
    fn undo_on_empty_is_noop() {
        let mut state = InputState::new();
        state.reduce(InputAction::Undo);
        assert_eq!(state.buffer(), "");
    }

    #[test]
    fn redo_after_undo() {
        let mut state = InputState::new();
        state.reduce(InputAction::InsertChar('a'));
        state.reduce(InputAction::InsertChar('b'));
        state.reduce(InputAction::Undo);
        assert_eq!(state.buffer(), "a");

        state.reduce(InputAction::Redo);
        assert_eq!(state.buffer(), "ab");
    }

    #[test]
    fn redo_cleared_on_new_edit() {
        let mut state = InputState::new();
        state.reduce(InputAction::InsertChar('a'));
        state.reduce(InputAction::InsertChar('b'));
        state.reduce(InputAction::Undo);
        assert_eq!(state.buffer(), "a");

        // New edit clears redo stack
        state.reduce(InputAction::InsertChar('c'));
        assert_eq!(state.buffer(), "ac");

        // Redo should be empty now
        state.reduce(InputAction::Redo);
        assert_eq!(state.buffer(), "ac"); // no change
    }

    #[test]
    fn undo_clamps_cursor() {
        let mut state = InputState::new();
        for c in "hello".chars() {
            state.reduce(InputAction::InsertChar(c));
        }
        assert_eq!(state.cursor_pos(), 5);

        // Undo back to "hell"
        state.reduce(InputAction::Undo);
        // Cursor should be clamped to buffer length
        assert!(state.cursor_pos() <= state.buffer().len());
    }

    // ── UTF-8 cursor tests ───────────────────────────────────────────────────

    #[test]
    fn cursor_movement_with_multibyte_chars() {
        let mut state = InputState::new();
        // Insert some multi-byte characters
        state.reduce(InputAction::InsertChar('a'));
        state.reduce(InputAction::InsertChar('\u{00e9}')); // é (2 bytes)
        state.reduce(InputAction::InsertChar('b'));
        assert_eq!(state.buffer(), "a\u{00e9}b");
        assert_eq!(state.cursor_pos(), 4); // 1 + 2 + 1

        state.reduce(InputAction::MoveCursorLeft);
        assert_eq!(state.cursor_pos(), 3); // before 'b'

        state.reduce(InputAction::MoveCursorLeft);
        assert_eq!(state.cursor_pos(), 1); // before 'é'

        state.reduce(InputAction::MoveCursorRight);
        assert_eq!(state.cursor_pos(), 3); // after 'é'
    }

    #[test]
    fn delete_word_at_cursor_in_middle() {
        let mut state = InputState::new();
        for c in "hello world end".chars() {
            state.reduce(InputAction::InsertChar(c));
        }
        // Move cursor back to after "world " (before "end")
        state.reduce(InputAction::MoveCursorLeft);
        state.reduce(InputAction::MoveCursorLeft);
        state.reduce(InputAction::MoveCursorLeft);
        assert_eq!(state.cursor_pos(), 12);

        state.reduce(InputAction::DeleteWord);
        assert_eq!(state.buffer(), "hello end");
        assert_eq!(state.cursor_pos(), 6);
    }

    #[test]
    fn line_col_conversion_roundtrip() {
        let state = {
            let mut s = InputState::new();
            for c in "abc\nde\nfghij".chars() {
                s.reduce(InputAction::InsertChar(c));
            }
            s
        };
        // Line 0: "abc" (len 3)
        assert_eq!(state.line_col_to_offset_public(0, 0), 0);
        assert_eq!(state.line_col_to_offset_public(0, 3), 3);
        assert_eq!(state.line_col_to_offset_public(0, 10), 3); // clamped

        // Line 1: "de" (len 2), starts at offset 4
        assert_eq!(state.line_col_to_offset_public(1, 0), 4);
        assert_eq!(state.line_col_to_offset_public(1, 2), 6);

        // Line 2: "fghij" (len 5), starts at offset 7
        assert_eq!(state.line_col_to_offset_public(2, 0), 7);
        assert_eq!(state.line_col_to_offset_public(2, 5), 12);

        // Beyond last line
        assert_eq!(state.line_col_to_offset_public(5, 0), 12);
    }

    // ── Paste-block tests ────────────────────────────────────────────────────

    #[test]
    fn single_line_paste_inserts_normally() {
        let mut state = InputState::new();
        state.insert_paste_block("hello".to_string());
        assert_eq!(state.buffer(), "hello");
        assert!(state.paste_blocks().is_empty());
    }

    #[test]
    fn multiline_paste_stores_block_and_placeholder() {
        let mut state = InputState::new();
        state.insert_paste_block("line1\nline2\nline3".to_string());
        assert_eq!(state.paste_blocks().len(), 1);
        assert_eq!(state.paste_blocks()[0].line_count, 3);
        // Buffer should contain the placeholder token, not the raw text
        assert!(state.buffer().contains("\x00PASTE:0\x00"));
        assert!(!state.buffer().contains("line1"));
    }

    #[test]
    fn paste_block_ids_increment() {
        let mut state = InputState::new();
        state.insert_paste_block("a\nb".to_string());
        state.insert_paste_block("c\nd".to_string());
        assert_eq!(state.paste_blocks().len(), 2);
        assert_eq!(state.paste_blocks()[0].id, 0);
        assert_eq!(state.paste_blocks()[1].id, 1);
    }

    #[test]
    fn expand_paste_blocks_replaces_placeholders() {
        let mut state = InputState::new();
        state.reduce(InputAction::InsertChar('x'));
        state.reduce(InputAction::InsertChar(' '));
        state.insert_paste_block("foo\nbar".to_string());
        state.reduce(InputAction::InsertChar(' '));
        state.reduce(InputAction::InsertChar('y'));
        let expanded = state.expand_paste_blocks(state.buffer());
        assert!(expanded.contains("foo\nbar"));
        assert!(expanded.starts_with("x "));
        assert!(expanded.ends_with(" y"));
    }

    #[test]
    fn submit_expands_paste_blocks_in_submitted_text() {
        let mut state = InputState::new();
        state.insert_paste_block("first\nsecond".to_string());
        state.reduce(InputAction::Submit);
        let submitted = state.take_submitted().expect("should have submitted text");
        assert!(submitted.contains("first\nsecond"));
        assert!(state.paste_blocks().is_empty());
        assert_eq!(state.buffer(), "");
    }

    #[test]
    fn clear_removes_paste_blocks() {
        let mut state = InputState::new();
        state.insert_paste_block("a\nb".to_string());
        assert_eq!(state.paste_blocks().len(), 1);
        state.reduce(InputAction::Clear);
        assert!(state.paste_blocks().is_empty());
    }

    #[test]
    fn clear_line_removes_paste_blocks() {
        let mut state = InputState::new();
        state.insert_paste_block("a\nb".to_string());
        state.reduce(InputAction::ClearLine);
        assert!(state.paste_blocks().is_empty());
    }

    #[test]
    fn paste_block_display_formats_label() {
        let blocks = vec![PasteBlock {
            id: 0,
            content: "fix this bug\nand that one\nand another".to_string(),
            line_count: 3,
        }];
        let label = InputState::paste_block_display(0, &blocks).unwrap();
        assert!(label.contains("[Pasted text #1"));
        assert!(label.contains("+3 lines"));
        assert!(label.contains("fix this bug"));
    }

    #[test]
    fn paste_block_display_truncates_long_first_line() {
        let long_line = "a".repeat(30);
        let content = format!("{}\nmore", long_line);
        let blocks = vec![PasteBlock {
            id: 0,
            content,
            line_count: 2,
        }];
        let label = InputState::paste_block_display(0, &blocks).unwrap();
        assert!(label.contains("..."));
    }

    #[test]
    fn paste_block_display_returns_none_for_unknown_id() {
        let blocks: Vec<PasteBlock> = Vec::new();
        assert!(InputState::paste_block_display(42, &blocks).is_none());
    }
}
