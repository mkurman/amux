# Tamux TUI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rewrite the tamux-tui crate from a monolithic StringModel line-painter into a decomposed widget-tree architecture with two-pane layout, streaming chat, modals, task sidebar, and settings panel.

**Architecture:** Elm-Delegated + Projection Layer. Root `TuiModel` composes 8 state modules (ChatState, TaskState, SidebarState, InputState, ModalState, ConfigState, ApprovalState, SettingsState). `DaemonProjection` transforms wire-format `ClientEvent` into typed `AppAction`. Pure widget functions build the view from state slices. Existing `client.rs` and `state.rs` reused unchanged.

**Tech Stack:** Rust, `ftui-runtime` 0.2.1 (with crossterm-compat), `ftui-core` 0.2.1, `amux-protocol`, tokio, serde_json

**Spec:** `docs/superpowers/specs/2026-03-19-tamux-tui-design.md`

---

## File Map

### Preserved (logic unchanged, minor path edits)
- `crates/amux-tui/Cargo.toml` — already has correct dependencies
- `crates/amux-tui/src/client.rs` — daemon IPC client, logic unchanged. Imports updated from `crate::state::*` to `crate::wire::*` after rename.
- `crates/amux-tui/src/state.rs` → **renamed to** `crates/amux-tui/src/wire.rs` — wire-format types (AgentThread, AgentMessage, etc.). Renamed to avoid collision with `state/` directory. All internal imports updated.

### Rewritten
- `crates/amux-tui/src/main.rs` — entry point, daemon bridge thread, ftui App bootstrap
- `crates/amux-tui/src/app.rs` — TuiModel compositor, Msg enum, StringModel impl

### Deleted (replaced by new modules)
- `crates/amux-tui/src/panes.rs`
- `crates/amux-tui/src/layout.rs`
- `crates/amux-tui/src/modal.rs`
- `crates/amux-tui/src/actions.rs`
- `crates/amux-tui/src/theme.rs`
- `crates/amux-tui/examples/frankentui_superintelligent_blueprint.rs`

### New Files — State Modules
- `crates/amux-tui/src/state/mod.rs` — AppAction enum, DaemonCommand enum, re-exports
- `crates/amux-tui/src/state/chat.rs` — ChatState, ChatAction, ToolCallVm, TranscriptMode
- `crates/amux-tui/src/state/input.rs` — InputState, InputAction, InputMode
- `crates/amux-tui/src/state/modal.rs` — ModalState, ModalAction, ModalKind, CommandItem
- `crates/amux-tui/src/state/sidebar.rs` — SidebarState, SidebarAction, SidebarTab
- `crates/amux-tui/src/state/task.rs` — TaskState, TaskAction
- `crates/amux-tui/src/state/config.rs` — ConfigState, ConfigAction
- `crates/amux-tui/src/state/approval.rs` — ApprovalState, ApprovalAction, RiskLevel, PendingApproval
- `crates/amux-tui/src/state/settings.rs` — SettingsState, SettingsAction, SettingsTab

### New Files — Projection
- `crates/amux-tui/src/projection.rs` — DaemonProjection::project(ClientEvent) → Vec<AppAction>

### New Files — Theme
- `crates/amux-tui/src/theme.rs` — ThemeTokens with ANSI-256 color palette, border sets

### New Files — Widgets
- `crates/amux-tui/src/widgets/mod.rs` — re-exports
- `crates/amux-tui/src/widgets/header.rs` — header_widget()
- `crates/amux-tui/src/widgets/footer.rs` — footer_widget()
- `crates/amux-tui/src/widgets/splash.rs` — splash_widget()
- `crates/amux-tui/src/widgets/chat.rs` — chat_widget()
- `crates/amux-tui/src/widgets/message.rs` — message_widget()
- `crates/amux-tui/src/widgets/reasoning.rs` — reasoning_widget()
- `crates/amux-tui/src/widgets/sidebar.rs` — sidebar_widget()
- `crates/amux-tui/src/widgets/task_tree.rs` — task_tree_widget()
- `crates/amux-tui/src/widgets/subagents.rs` — subagents_widget()
- `crates/amux-tui/src/widgets/command_palette.rs` — command_palette_widget()
- `crates/amux-tui/src/widgets/approval.rs` — approval_widget()
- `crates/amux-tui/src/widgets/thread_picker.rs` — thread_picker_widget()
- `crates/amux-tui/src/widgets/settings.rs` — settings_widget()
- `crates/amux-tui/src/widgets/provider_picker.rs` — provider_picker_widget()
- `crates/amux-tui/src/widgets/model_picker.rs` — model_picker_widget()

---

## Phase 0: Architectural Migration

### Task 1: Delete old modules and set up directory structure

**Files:**
- Delete: `crates/amux-tui/src/panes.rs`
- Delete: `crates/amux-tui/src/layout.rs`
- Delete: `crates/amux-tui/src/modal.rs`
- Delete: `crates/amux-tui/src/actions.rs`
- Delete: `crates/amux-tui/src/theme.rs`
- Delete: `crates/amux-tui/examples/frankentui_superintelligent_blueprint.rs`
- Create: `crates/amux-tui/src/state/mod.rs`
- Create: `crates/amux-tui/src/widgets/mod.rs`

- [ ] **Step 1: Delete old module files**

```bash
cd crates/amux-tui
rm -f src/panes.rs src/layout.rs src/modal.rs src/actions.rs src/theme.rs
rm -rf examples/
```

- [ ] **Step 2: Create state/mod.rs with AppAction and DaemonCommand enums**

Create `crates/amux-tui/src/state/mod.rs` with:
- `AppAction` enum wrapping sub-module actions (Chat, Task, Sidebar, Input, Modal, Config, Approval, Settings, plus Status, Focus, Resize, Tick variants)
- `DaemonCommand` enum (Refresh, RefreshServices, RequestThread, SendMessage, FetchModels, SetConfigJson, ControlGoalRun, ResolveTaskApproval, SpawnSession)
- `FocusArea` enum (Chat, Sidebar, Input)
- Re-exports for all sub-module types

The DaemonCommand enum matches the existing one in app.rs (lines 80-110).

- [ ] **Step 3: Create widgets/mod.rs stub**

Create `crates/amux-tui/src/widgets/mod.rs` as an empty module with placeholder comments for each widget file to be added.

- [ ] **Step 4: Verify directory structure exists**

```bash
ls -la crates/amux-tui/src/state/
ls -la crates/amux-tui/src/widgets/
```

Expected: Both directories exist with mod.rs files.

- [ ] **Step 5: Commit**

```bash
git add -A crates/amux-tui/
git commit -m "refactor: remove old TUI modules, scaffold state/ and widgets/ directories"
```

### Task 2: Create theme.rs with retro-hacker color palette

**Files:**
- Create: `crates/amux-tui/src/theme.rs`

- [ ] **Step 1: Write theme test**

Add to bottom of `crates/amux-tui/src/theme.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_theme_has_all_tokens() {
        let theme = ThemeTokens::default();
        // Verify all 8 color tokens are set (non-zero indexed values for non-reset)
        assert_ne!(theme.fg_dim, theme.accent_primary);
        assert_ne!(theme.accent_danger, theme.accent_success);
    }

    #[test]
    fn border_sets_have_correct_char_counts() {
        assert_eq!(ROUNDED_BORDER.top_left, '╭');
        assert_eq!(ROUNDED_BORDER.top_right, '╮');
        assert_eq!(SHARP_BORDER.top_left, '╔');
        assert_eq!(SHARP_BORDER.bottom_right, '╝');
    }
}
```

- [ ] **Step 2: Write ThemeTokens struct and border sets**

```rust
#[derive(Debug, Clone, Copy)]
pub struct Color(pub u8); // ANSI-256 index

impl Color {
    pub const RESET: Self = Self(0);
}

#[derive(Debug, Clone, Copy)]
pub struct ThemeTokens {
    pub bg_main: Color,           // terminal default
    pub fg_dim: Color,            // Indexed(245)
    pub fg_active: Color,         // Indexed(255)
    pub accent_primary: Color,    // Indexed(75) — cyan
    pub accent_assistant: Color,  // Indexed(183) — lavender
    pub accent_secondary: Color,  // Indexed(178) — amber
    pub accent_success: Color,    // Indexed(78) — green
    pub accent_danger: Color,     // Indexed(203) — red
}

impl Default for ThemeTokens {
    fn default() -> Self {
        Self {
            bg_main: Color::RESET,
            fg_dim: Color(245),
            fg_active: Color(255),
            accent_primary: Color(75),
            accent_assistant: Color(183),
            accent_secondary: Color(178),
            accent_success: Color(78),
            accent_danger: Color(203),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct BorderSet {
    pub top_left: char,
    pub top_right: char,
    pub bottom_left: char,
    pub bottom_right: char,
    pub horizontal: char,
    pub vertical: char,
}

pub const ROUNDED_BORDER: BorderSet = BorderSet {
    top_left: '╭', top_right: '╮',
    bottom_left: '╰', bottom_right: '╯',
    horizontal: '─', vertical: '│',
};

pub const SHARP_BORDER: BorderSet = BorderSet {
    top_left: '╔', top_right: '╗',
    bottom_left: '╚', bottom_right: '╝',
    horizontal: '═', vertical: '║',
};
```

- [ ] **Step 3: Run tests**

```bash
cd crates/amux-tui && cargo test --lib theme
```

Expected: 2 tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/amux-tui/src/theme.rs
git commit -m "feat(tui): add retro-hacker color palette and border sets"
```

### Task 3: Create InputState module

**Files:**
- Create: `crates/amux-tui/src/state/input.rs`

- [ ] **Step 1: Write InputState tests**

```rust
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
        assert_eq!(submitted, Some("hi".to_string()));
        assert_eq!(state.buffer(), "");
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
}
```

- [ ] **Step 2: Implement InputState**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Insert,
}

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
        Self { buffer: String::new(), mode: InputMode::Insert, submitted: None }
    }

    pub fn buffer(&self) -> &str { &self.buffer }
    pub fn mode(&self) -> InputMode { self.mode }
    pub fn multiline(&self) -> bool { self.buffer.contains('\n') }

    pub fn take_submitted(&mut self) -> Option<String> {
        self.submitted.take()
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
```

- [ ] **Step 3: Register module in state/mod.rs**

Add `pub mod input;` and re-export `InputState`, `InputAction`, `InputMode`.

- [ ] **Step 4: Run tests**

```bash
cd crates/amux-tui && cargo test --lib state::input
```

Expected: 6 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/amux-tui/src/state/input.rs crates/amux-tui/src/state/mod.rs
git commit -m "feat(tui): add InputState with Normal/Insert mode switching"
```

### Task 4: Create ChatState module

**Files:**
- Create: `crates/amux-tui/src/state/chat.rs`

- [ ] **Step 1: Write ChatState tests**

Focus on core reduce logic: delta appending, thread selection, scroll behavior, turn finalization.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delta_appends_to_streaming_content() {
        let mut state = ChatState::new();
        state.reduce(ChatAction::ThreadCreated {
            thread_id: "t1".into(), title: "Test".into(),
        });
        state.reduce(ChatAction::Delta { thread_id: "t1".into(), content: "Hello".into() });
        state.reduce(ChatAction::Delta { thread_id: "t1".into(), content: " world".into() });
        assert_eq!(state.streaming_content(), "Hello world");
    }

    #[test]
    fn turn_done_finalizes_streaming_into_message() {
        let mut state = ChatState::new();
        state.reduce(ChatAction::ThreadCreated {
            thread_id: "t1".into(), title: "Test".into(),
        });
        state.reduce(ChatAction::Delta { thread_id: "t1".into(), content: "Hi".into() });
        state.reduce(ChatAction::TurnDone {
            thread_id: "t1".into(),
            input_tokens: 100, output_tokens: 50,
            cost: Some(0.01), provider: Some("openai".into()),
            model: Some("gpt-4o".into()), tps: Some(45.0),
            generation_ms: Some(1200),
        });
        assert_eq!(state.streaming_content(), "");
        let thread = state.active_thread().unwrap();
        let last = thread.messages.last().unwrap();
        assert_eq!(last.content, "Hi");
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
}
```

- [ ] **Step 2: Implement ChatState**

Implement the struct with fields from spec Section 3.1. Include `ToolCallVm` for tracking active tool calls and `TranscriptMode` enum. The `reduce()` handles all `ChatAction` variants. The `effects()` method returns `DaemonCommand::RequestThread` when selecting a thread.

Key types needed:
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranscriptMode { Compact, Tools, Full }

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolCallStatus { Running, Done, Error }
```

- [ ] **Step 3: Register in state/mod.rs, run tests**

```bash
cd crates/amux-tui && cargo test --lib state::chat
```

Expected: 5 tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/amux-tui/src/state/chat.rs crates/amux-tui/src/state/mod.rs
git commit -m "feat(tui): add ChatState with streaming, scroll lock, thread management"
```

### Task 5: Create ModalState module

**Files:**
- Create: `crates/amux-tui/src/state/modal.rs`

- [ ] **Step 1: Write ModalState tests**

Test stack push/pop, fuzzy filtering, navigation clamping.

```rust
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
        state.reduce(ModalAction::FuzzyFilter);
        // "provider" and "prompt" should match "pro"
        assert!(state.filtered_items().len() >= 2);
        // "model" should not match
        let filtered_names: Vec<_> = state.filtered_items().iter()
            .map(|&idx| &state.command_items()[idx].command).collect();
        assert!(!filtered_names.contains(&&"model".to_string()));
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
}
```

- [ ] **Step 2: Implement ModalState**

Include `ModalKind` enum (CommandPalette, ThreadPicker, ProviderPicker, ModelPicker, ApprovalOverlay, Settings, EffortPicker, ToolsPicker, ViewPicker), `CommandItem` struct, default command registry (from spec Section 6.7), and simple substring fuzzy match.

- [ ] **Step 3: Run tests**

```bash
cd crates/amux-tui && cargo test --lib state::modal
```

Expected: 4 tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/amux-tui/src/state/modal.rs crates/amux-tui/src/state/mod.rs
git commit -m "feat(tui): add ModalState with stack, fuzzy filter, command registry"
```

### Task 6: Create remaining state modules (TaskState, SidebarState, ConfigState, ApprovalState, SettingsState)

**Files:**
- Create: `crates/amux-tui/src/state/task.rs`
- Create: `crates/amux-tui/src/state/sidebar.rs`
- Create: `crates/amux-tui/src/state/config.rs`
- Create: `crates/amux-tui/src/state/approval.rs`
- Create: `crates/amux-tui/src/state/settings.rs`

- [ ] **Step 1: Implement TaskState with tests**

Follow spec Section 3.2. Tests: task list upsert, goal run upsert, heartbeat update.

- [ ] **Step 2: Implement SidebarState with tests**

Follow spec Section 3.3. Tests: tab switching, navigation clamping, expand/collapse toggle.

- [ ] **Step 3: Implement ConfigState with tests**

Follow spec Section 3.6. Tests: config received populates fields, models fetched populates list.

- [ ] **Step 4: Implement ApprovalState with tests**

Follow spec Section 3.7. Tests: approval required adds to pending, resolve removes, risk level parsing from string.

**Important:** The wire-format `AgentTask` does NOT contain `command`, `risk_level`, or `blast_radius` fields. `PendingApproval` must be heuristically constructed: extract command text from `AgentTask.blocked_reason`, classify risk level by pattern-matching the command string (e.g., `rm -rf` → Critical, `git push --force` → High, `cargo publish` → Medium), and infer blast radius from the command pattern. See spec Section 6.9 for the full heuristic approach.

- [ ] **Step 5: Implement SettingsState with tests**

Follow spec Section 3.8. Tests: tab cycling, field navigation, dirty flag on edit.

- [ ] **Step 6: Register all modules in state/mod.rs, run full test suite**

```bash
cd crates/amux-tui && cargo test --lib state
```

Expected: All state module tests pass.

- [ ] **Step 7: Commit**

```bash
git add crates/amux-tui/src/state/
git commit -m "feat(tui): add TaskState, SidebarState, ConfigState, ApprovalState, SettingsState"
```

### Task 7: Create DaemonProjection

**Files:**
- Create: `crates/amux-tui/src/projection.rs`

- [ ] **Step 1: Write projection tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::ClientEvent;

    #[test]
    fn delta_event_maps_to_chat_action() {
        let actions = DaemonProjection::project(ClientEvent::Delta {
            thread_id: "t1".into(),
            content: "hello".into(),
        });
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            AppAction::Chat(ChatAction::Delta { thread_id, content }) => {
                assert_eq!(thread_id, "t1");
                assert_eq!(content, "hello");
            }
            _ => panic!("expected Chat Delta action"),
        }
    }

    #[test]
    fn connected_event_maps_to_status_and_refresh() {
        let actions = DaemonProjection::project(ClientEvent::Connected);
        assert!(actions.len() >= 2); // Status + trigger refresh
    }

    #[test]
    fn task_list_maps_to_task_action() {
        let actions = DaemonProjection::project(ClientEvent::TaskList(vec![]));
        assert_eq!(actions.len(), 1);
        matches!(&actions[0], AppAction::Task(TaskAction::TaskListReceived(_)));
    }

    #[test]
    fn done_event_maps_to_chat_turn_done() {
        let actions = DaemonProjection::project(ClientEvent::Done {
            thread_id: "t1".into(),
            input_tokens: 100,
            output_tokens: 50,
            cost: Some(0.01),
            provider: Some("openai".into()),
            model: Some("gpt-4o".into()),
            tps: Some(45.0),
            generation_ms: Some(1200),
        });
        assert!(actions.iter().any(|a| matches!(a, AppAction::Chat(ChatAction::TurnDone { .. }))));
    }
}
```

- [ ] **Step 2: Implement DaemonProjection::project()**

Pure function mapping every `ClientEvent` variant to `Vec<AppAction>`. One match arm per variant. No side effects.

- [ ] **Step 3: Run tests**

```bash
cd crates/amux-tui && cargo test --lib projection
```

Expected: 4 tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/amux-tui/src/projection.rs
git commit -m "feat(tui): add DaemonProjection for ClientEvent → AppAction mapping"
```

### Task 8: Rewrite app.rs as TuiModel compositor

**Files:**
- Rewrite: `crates/amux-tui/src/app.rs`

- [ ] **Step 1: Write new TuiModel struct**

Compose all 8 state modules + infrastructure fields (daemon channels, theme, focus, width/height, connected, status_line, output_log). Implement `StringModel` trait: `update()` pumps daemon events through projection, maps crossterm events to AppAction, dispatches to sub-modules, collects effects. `view()` returns a placeholder string for now (will be replaced by widget rendering in Phase 1).

- [ ] **Step 2: Implement update() — daemon event pump**

On `Event::Tick`: drain `daemon_events_rx` via `try_recv()`, project each through `DaemonProjection::project()`, dispatch resulting `AppAction` to sub-modules.

- [ ] **Step 3: Implement update() — key event routing**

Map `(input_mode, focus, modal_stack.top())` → `AppAction`. Implement the keyboard map from spec Section 7.3: Tab/Shift+Tab cycle focus, Ctrl+P opens command palette, Esc closes modal, Normal mode j/k scroll, Insert mode char input, etc.

- [ ] **Step 4: Implement effects collection**

After dispatching actions, collect `DaemonCommand` from sub-module `.effects()` and send via `daemon_cmd_tx`.

- [ ] **Step 5: Implement view() stub**

Return a minimal string: header line with "TAMUX" + connected status, body with "Chat area", footer with input buffer. Enough to verify the app runs.

- [ ] **Step 6: Verify compilation and basic run**

```bash
cd crates/amux-tui && cargo build
```

Expected: Compiles without errors.

- [ ] **Step 7: Commit**

```bash
git add crates/amux-tui/src/app.rs
git commit -m "feat(tui): rewrite TuiModel as compositor with decomposed state dispatch"
```

### Task 9: Rewrite main.rs entry point

**Files:**
- Rewrite: `crates/amux-tui/src/main.rs`

- [ ] **Step 1: Update mod declarations**

Replace old module declarations with new ones: `mod app; mod client; mod projection; mod state; mod theme; mod widgets;`. Keep `state` as the old wire-types file — rename it or nest appropriately. The wire types file (`state.rs` with AgentThread, AgentMessage, etc.) should be accessible as `crate::wire` or similar to avoid collision with the `state/` directory.

Strategy: rename `crates/amux-tui/src/state.rs` → `crates/amux-tui/src/wire.rs` and update all imports. Then `state/` becomes the new state module directory.

- [ ] **Step 2: Update daemon bridge match arms**

The bridge thread translates `DaemonCommand` → `ClientMessage`. Update the match arms to use the new `DaemonCommand` enum from `state/mod.rs` (which should be identical to the old one).

- [ ] **Step 3: Verify full compilation and startup**

```bash
cd crates/amux-tui && cargo build && cargo run -- 2>/dev/null || true
```

Expected: Compiles. If daemon is running, connects and shows minimal view. If not, shows "Disconnected" status.

- [ ] **Step 4: Commit**

```bash
git add crates/amux-tui/src/
git commit -m "feat(tui): rewire main.rs entry point with new module structure"
```

---

## Phase 1: Shell & Chat

### Task 10: Implement header_widget and footer_widget

**Files:**
- Create: `crates/amux-tui/src/widgets/header.rs`
- Create: `crates/amux-tui/src/widgets/footer.rs`

- [ ] **Step 1: Implement header_widget()**

Renders a single row inside rounded borders: `░▒▓TAMUX▓▒░ [agent_name] | model_name | tokens · $cost`. Uses `ThemeTokens` for colors. Takes `&ConfigState`, `&ChatState`, `&ThemeTokens` as input.

The header is built as a `StringModel` view string: one bordered line using ANSI escape codes for colors. Reference the `ThemeTokens` Color(u8) values to emit `\x1b[38;5;{n}m` sequences.

- [ ] **Step 2: Implement footer_widget()**

Two lines in rounded border:
1. `▶ {input_buffer}` with cursor indicator when in Insert mode.
2. Context-sensitive shortcut hints based on current mode and focus.

Takes `&InputState`, `&ThemeTokens`, `FocusArea` as input.

- [ ] **Step 3: Wire into app.rs view()**

Replace the stub view with header + footer rendering.

- [ ] **Step 4: Verify visual output**

```bash
cd crates/amux-tui && cargo run
```

Expected: Header and footer visible with correct colors.

- [ ] **Step 5: Commit**

```bash
git add crates/amux-tui/src/widgets/header.rs crates/amux-tui/src/widgets/footer.rs crates/amux-tui/src/widgets/mod.rs crates/amux-tui/src/app.rs
git commit -m "feat(tui): add header and footer widgets with retro-hacker theming"
```

### Task 11: Implement splash_widget

**Files:**
- Create: `crates/amux-tui/src/widgets/splash.rs`

- [ ] **Step 1: Implement splash_widget()**

Centered ASCII art:
```
░▒▓█ T A M U X █▓▒░
     plan · solve · ship

Type a prompt to begin, or
Ctrl+P to open command palette
Ctrl+T to pick a thread
```

Uses gradient colors: `accent_primary` spectrum from dark to light across the `░▒▓█` characters. Hints in `fg_dim`. Center-aligns based on available width.

- [ ] **Step 2: Wire into chat_widget when no active thread**

When `ChatState.active_thread()` is `None` and no messages, render splash instead of message list.

- [ ] **Step 3: Verify**

```bash
cd crates/amux-tui && cargo run
```

Expected: Logo visible on empty startup.

- [ ] **Step 4: Commit**

```bash
git add crates/amux-tui/src/widgets/splash.rs crates/amux-tui/src/widgets/chat.rs crates/amux-tui/src/widgets/mod.rs
git commit -m "feat(tui): add splash screen with TAMUX gradient logo"
```

### Task 12: Implement message_widget and chat_widget

**Files:**
- Create: `crates/amux-tui/src/widgets/message.rs`
- Create: `crates/amux-tui/src/widgets/chat.rs`

- [ ] **Step 1: Implement message_widget()**

Renders a single `AgentMessage` as lines:
- Role badge: `USER` (cyan bg), `ASST` (lavender bg), `SYS` (grey bg), `TOOL` (gear icon).
- Content with 7-char indent for line wrapping.
- Tool call indicators (compact mode): `⚙ name  ✓ done 1.2s`.
- Streaming content with `█` cursor appended.

Takes `&AgentMessage`, `TranscriptMode`, `&ThemeTokens`, `width: usize`.

- [ ] **Step 2: Implement chat_widget()**

Scrollable message list inside rounded border. Title: "Conversation". Focus-aware border color.

Logic:
1. If no active thread → render splash.
2. Else, render messages from active thread.
3. If streaming, append streaming_content as a partial assistant message.
4. Apply scroll_offset (0 = tail, positive = lines from bottom).

Takes `&ChatState`, `&ThemeTokens`, `focused: bool`, `width: usize`, `height: usize`.

- [ ] **Step 3: Wire into app.rs view()**

Replace body area with `chat_widget()` output.

- [ ] **Step 4: Test with daemon connection**

Start the daemon, send a message, verify streaming renders correctly with role badges and auto-scroll.

- [ ] **Step 5: Commit**

```bash
git add crates/amux-tui/src/widgets/message.rs crates/amux-tui/src/widgets/chat.rs crates/amux-tui/src/app.rs
git commit -m "feat(tui): add chat and message widgets with streaming support"
```

### Task 13: Implement two-pane layout in view()

**Files:**
- Modify: `crates/amux-tui/src/app.rs`

- [ ] **Step 1: Implement layout calculation**

Compute pane dimensions from terminal width/height:
- Header: 3 rows (border + content + border)
- Footer: 4 rows (border + input + hints + border)
- Body: remaining rows
- Chat pane: flex 7 (~65% of width)
- Sidebar pane: flex 3 (~35% of width)
- 1 col gap between panes

Handle responsive breakpoints: below 100 cols, sidebar hidden.

- [ ] **Step 2: Implement sidebar stub**

Render a bordered "Context" panel with placeholder text for sidebar area.

- [ ] **Step 3: Compose full layout in view()**

```
header rows
chat rows (left) | sidebar rows (right)  ← side by side
footer rows
```

Merge left and right pane rows side-by-side with proper column offsets.

- [ ] **Step 4: Verify two-pane layout**

```bash
cd crates/amux-tui && cargo run
```

Expected: Two bordered panes side-by-side with correct proportions.

- [ ] **Step 5: Commit**

```bash
git add crates/amux-tui/src/app.rs
git commit -m "feat(tui): implement two-pane layout with responsive sidebar"
```

---

## Phase 2: Modals & Interaction

### Task 14: Implement command_palette_widget

**Files:**
- Create: `crates/amux-tui/src/widgets/command_palette.rs`

- [ ] **Step 1: Implement command_palette_widget()**

Sharp amber border. Search input at top showing `/{query}`. Filtered command list below. Selected item highlighted in amber bg + black fg. Footer with hint text. Centered overlay at ~50% width, ~40% height.

Takes `&ModalState`, `&ThemeTokens`.

- [ ] **Step 2: Wire Ctrl+P and / triggers**

In `app.rs` key routing: Ctrl+P pushes CommandPalette modal. `/` in Insert mode (when first char) also opens it.

- [ ] **Step 3: Wire j/k navigation and Enter execution**

Modal mode: j/k navigate picker_cursor, Enter executes selected command, Esc pops modal.

- [ ] **Step 4: Implement command execution dispatch**

When a command is selected: match command string → push appropriate sub-modal or dispatch action (e.g., "/new" creates new thread, "/quit" quits).

- [ ] **Step 5: Verify**

Open palette with Ctrl+P, type "pro", verify fuzzy filter shows "provider" and "prompt", navigate with j/k, press Enter.

- [ ] **Step 6: Commit**

```bash
git add crates/amux-tui/src/widgets/command_palette.rs crates/amux-tui/src/app.rs crates/amux-tui/src/widgets/mod.rs
git commit -m "feat(tui): add command palette with fuzzy search and command dispatch"
```

### Task 15: Implement thread_picker_widget

**Files:**
- Create: `crates/amux-tui/src/widgets/thread_picker.rs`

- [ ] **Step 1: Implement thread_picker_widget()**

Sharp amber border. Search input. First item: "+ New conversation". Thread list with green/grey dots, title, time ago, token count. Selected item highlighted.

Takes `&ChatState`, `&ModalState`, `&ThemeTokens`.

- [ ] **Step 2: Wire Ctrl+T trigger and selection**

Ctrl+T pushes ThreadPicker modal. Enter on thread → `ChatAction::SelectThread`, sends `DaemonCommand::RequestThread`. Enter on "+ New" → `ChatAction::NewThread`.

- [ ] **Step 3: Verify**

Open picker with Ctrl+T, verify thread list loads from daemon, select a thread.

- [ ] **Step 4: Commit**

```bash
git add crates/amux-tui/src/widgets/thread_picker.rs crates/amux-tui/src/app.rs
git commit -m "feat(tui): add thread picker modal with search and daemon loading"
```

### Task 16: Implement approval_widget

**Files:**
- Create: `crates/amux-tui/src/widgets/approval.rs`

- [ ] **Step 1: Implement approval_widget()**

Sharp border — red for HIGH/CRITICAL risk, amber for MEDIUM. Shows risk badge, command text, blast radius, source task. Action row: `[Y] Allow once  [A] Allow for session  [N] Reject`. Dimmed backdrop.

Takes `&ApprovalState`, `&ThemeTokens`.

- [ ] **Step 2: Wire Y/A/N key handling**

When ApprovalOverlay is top modal: Y sends `DaemonCommand::ResolveTaskApproval { decision: "allow_once" }`, A sends "allow_session", N sends "reject". All pop the modal.

- [ ] **Step 3: Wire approval detection from TaskState**

In projection or app.rs tick: scan tasks for `awaiting_approval_id.is_some()`, construct `PendingApproval` with heuristic risk classification from `blocked_reason`, push ApprovalOverlay modal.

- [ ] **Step 4: Commit**

```bash
git add crates/amux-tui/src/widgets/approval.rs crates/amux-tui/src/app.rs
git commit -m "feat(tui): add approval modal with risk classification and Y/A/N handling"
```

### Task 17: Implement reasoning_widget and tool call indicators

**Files:**
- Create: `crates/amux-tui/src/widgets/reasoning.rs`
- Modify: `crates/amux-tui/src/widgets/message.rs`

- [ ] **Step 1: Implement reasoning_widget()**

Collapsed: `▸ [+] Reasoning (12s · 847 tok)` in fg_dim.
Expanded: `▾ [-] Reasoning (...)` + dark blue left border `│` + dim reasoning text.

Takes `reasoning: &str`, `expanded: bool`, `elapsed_label: &str`, `&ThemeTokens`.

- [ ] **Step 2: Add tool call rendering to message_widget**

Compact mode: `⚙ {name}  {status_badge} {elapsed}` — one line per tool. Status badges: `✓ done` (green), `⠋ running` (amber), `✗ error` (red).

Expanded mode (when selected): show args + truncated result below tool line.

Braille spinner: cycle through `⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏` based on a tick counter.

- [ ] **Step 3: Add transcript mode support**

In chat_widget: respect `ChatState.transcript_mode`:
- Compact: merged tool row, concise content.
- Tools: tool calls only.
- Full: everything expanded.

- [ ] **Step 4: Wire `r` key to toggle reasoning**

In Normal mode with Chat focused: `r` toggles reasoning expansion on the selected/latest message.

- [ ] **Step 5: Commit**

```bash
git add crates/amux-tui/src/widgets/reasoning.rs crates/amux-tui/src/widgets/message.rs crates/amux-tui/src/widgets/chat.rs crates/amux-tui/src/app.rs
git commit -m "feat(tui): add collapsible reasoning, tool call indicators, transcript modes"
```

---

## Phase 3: Sidebar & Tasks

### Task 18: Implement task_tree_widget and sidebar

**Files:**
- Create: `crates/amux-tui/src/widgets/task_tree.rs`
- Create: `crates/amux-tui/src/widgets/sidebar.rs`

- [ ] **Step 1: Implement task_tree_widget()**

Three zones:
1. Goal runs with collapsible steps: `▾ Fix auth tests ● running` → indented step children with status chips `[x]` `[~]` `[ ]` `[!]`.
2. Standalone tasks (no goal_run_id): flat list with priority badges.
3. Heartbeat items with colored dots.

Takes `&TaskState`, `&SidebarState`, `&ThemeTokens`, `width: usize`.

- [ ] **Step 2: Implement sidebar_widget()**

Rounded border with title "Context". Tab row: `[Tasks] Subagents` or `Tasks [Subagents]`. Focus-aware border color.

Routes to task_tree_widget or subagents_widget based on active tab.

Takes `&SidebarState`, `&TaskState`, `&ThemeTokens`, `focused: bool`, `width: usize`, `height: usize`.

- [ ] **Step 3: Wire into app.rs layout**

Replace sidebar stub with real sidebar_widget output.

- [ ] **Step 4: Wire `[` / `]` keys for tab switching**

In Normal mode: `[` switches to Tasks tab, `]` switches to Subagents tab.

- [ ] **Step 5: Verify with daemon**

Connect to daemon with active tasks/goal runs, verify tree renders correctly.

- [ ] **Step 6: Commit**

```bash
git add crates/amux-tui/src/widgets/task_tree.rs crates/amux-tui/src/widgets/sidebar.rs crates/amux-tui/src/widgets/mod.rs crates/amux-tui/src/app.rs
git commit -m "feat(tui): add task tree and sidebar with goal run nesting"
```

### Task 19: Implement subagents_widget

**Files:**
- Create: `crates/amux-tui/src/widgets/subagents.rs`

- [ ] **Step 1: Implement subagents_widget()**

Group by runtime (hermes, openclaw, daemon). Each group: collapsible header with runtime name + classification. Child items: status dot + name + state. Footer: aggregate counts.

Note: The current `TaskState` may not have dedicated subagent data yet. Use tasks with `source: "subagent"` marker or separate the rendering once the daemon provides subagent-specific data. For now, render tasks grouped by goal_run_id as a hierarchy.

Takes `&TaskState`, `&SidebarState`, `&ThemeTokens`, `width: usize`.

- [ ] **Step 2: Wire into sidebar_widget**

When Subagents tab is active, render subagents_widget.

- [ ] **Step 3: Commit**

```bash
git add crates/amux-tui/src/widgets/subagents.rs crates/amux-tui/src/widgets/sidebar.rs
git commit -m "feat(tui): add subagents view with runtime grouping"
```

---

## Phase 4: Settings & Polish

### Task 20: Implement settings_widget

**Files:**
- Create: `crates/amux-tui/src/widgets/settings.rs`

- [ ] **Step 1: Implement settings tab rendering**

Six tabs rendered as `[Provider] Model Tools Reasoning Gateway Agent`. Active tab highlighted. Below tabs: fields for current tab.

Field renderers:
- Text input: `label:  value█` (editable)
- Secret: `label:  ••••abcd [show]`
- Dropdown: `label:  ▾ Selected Item`
- Checkbox: `[x] Label` or `[ ] Label`
- Radio: `(●) Selected` or `( ) Unselected`
- Number: `label:  123`

Takes `&SettingsState`, `&ConfigState`, `&ThemeTokens`.

- [ ] **Step 2: Wire /settings and Ctrl+, triggers**

Push Settings modal. Wire Tab/Shift+Tab for tab cycling within settings. Wire j/k for field navigation, Enter for edit, Space for toggle, Esc for close.

- [ ] **Step 3: Wire save → DaemonCommand::SetConfigJson**

On close or Ctrl+S: serialize current ConfigState to JSON, send to daemon.

- [ ] **Step 4: Verify end-to-end**

Open settings, change provider, change model, verify daemon receives config update.

- [ ] **Step 5: Commit**

```bash
git add crates/amux-tui/src/widgets/settings.rs crates/amux-tui/src/app.rs
git commit -m "feat(tui): add 6-tab settings panel with daemon config sync"
```

### Task 21: Implement provider_picker and model_picker sub-modals

**Files:**
- Create: `crates/amux-tui/src/widgets/provider_picker.rs`
- Create: `crates/amux-tui/src/widgets/model_picker.rs`

- [ ] **Step 1: Implement provider_picker_widget()**

List of 20+ providers (OpenAI, Anthropic, Groq, Ollama, etc.). Selected item in amber. Enter selects and updates ConfigState. Pops modal on selection.

- [ ] **Step 2: Implement model_picker_widget()**

If `ConfigState.fetched_models` is empty, show "Press Enter to fetch models". Otherwise, list models with context window. Selected in amber.

On open: trigger `DaemonCommand::FetchModels` if list is empty.

- [ ] **Step 3: Wire from command palette /provider and /model**

Command execution dispatches Push(ProviderPicker) or Push(ModelPicker).

- [ ] **Step 4: Commit**

```bash
git add crates/amux-tui/src/widgets/provider_picker.rs crates/amux-tui/src/widgets/model_picker.rs crates/amux-tui/src/app.rs
git commit -m "feat(tui): add provider and model picker sub-modals"
```

### Task 22: Implement responsive breakpoints and vim motions

**Files:**
- Modify: `crates/amux-tui/src/app.rs`

- [ ] **Step 1: Implement responsive sidebar collapse**

In layout calculation: if width < 100, hide sidebar. If width < 80, single-pane only. Add Ctrl+B toggle for sidebar overlay in narrow mode.

- [ ] **Step 2: Implement G/gg vim motions**

In Normal mode with Chat focused: `G` jumps to bottom (scroll_offset = 0), `g` followed by `g` jumps to top (scroll_offset = max).

Track `pending_g` state for the `gg` two-key sequence.

- [ ] **Step 3: Implement Ctrl+D/Ctrl+U page jumps**

Half-page scroll: `Ctrl+D` scrolls down by height/2, `Ctrl+U` scrolls up by height/2.

- [ ] **Step 4: Implement mouse support**

Handle `MouseEventKind::Down(Left)` for pane focus. Handle `MouseEventKind::ScrollUp/Down` for pane scrolling. Click targets use layout rectangles.

- [ ] **Step 5: Commit**

```bash
git add crates/amux-tui/src/app.rs
git commit -m "feat(tui): add responsive breakpoints, vim motions, mouse support"
```

### Task 23: Final integration test and cleanup

**Files:**
- Modify: Various

- [ ] **Step 1: Run full test suite**

```bash
cd crates/amux-tui && cargo test
```

Expected: All tests pass.

- [ ] **Step 2: Run clippy**

```bash
cd crates/amux-tui && cargo clippy -- -D warnings
```

Fix any warnings.

- [ ] **Step 3: Manual smoke test with daemon**

Start daemon, run TUI, verify:
1. Splash screen shows on startup.
2. Send a message, see streaming response.
3. Ctrl+P opens command palette with fuzzy search.
4. Ctrl+T opens thread picker.
5. Sidebar shows tasks and heartbeat.
6. /settings opens settings panel.
7. Approval modal appears for risky commands.
8. j/k, Ctrl+D/U, Tab navigation all work.

- [ ] **Step 4: Commit**

```bash
git add -A crates/amux-tui/
git commit -m "feat(tui): final cleanup and integration verification"
```
