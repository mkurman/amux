#![allow(dead_code)]

use std::collections::HashSet;

// ── SidebarTab ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarTab {
    Tasks,
    Subagents,
}

// ── SidebarAction ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum SidebarAction {
    SwitchTab(SidebarTab),
    Navigate(i32),
    ToggleExpand(String),
    Scroll(i32),
}

// ── SidebarState ──────────────────────────────────────────────────────────────

pub struct SidebarState {
    active_tab: SidebarTab,
    selected_item: usize,
    scroll_offset: usize,
    expanded_nodes: HashSet<String>,
}

impl SidebarState {
    pub fn new() -> Self {
        Self {
            active_tab: SidebarTab::Tasks,
            selected_item: 0,
            scroll_offset: 0,
            expanded_nodes: HashSet::new(),
        }
    }

    pub fn active_tab(&self) -> SidebarTab {
        self.active_tab
    }

    pub fn selected_item(&self) -> usize {
        self.selected_item
    }

    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    pub fn is_expanded(&self, node_id: &str) -> bool {
        self.expanded_nodes.contains(node_id)
    }

    /// Navigate within `item_count` visible items; clamps at both ends.
    pub fn navigate(&mut self, delta: i32, item_count: usize) {
        if item_count == 0 {
            self.selected_item = 0;
            return;
        }
        if delta > 0 {
            self.selected_item =
                (self.selected_item + delta as usize).min(item_count.saturating_sub(1));
        } else {
            self.selected_item = self.selected_item.saturating_sub((-delta) as usize);
        }
    }

    pub fn reduce(&mut self, action: SidebarAction) {
        match action {
            SidebarAction::SwitchTab(tab) => {
                self.active_tab = tab;
                self.selected_item = 0;
                self.scroll_offset = 0;
            }

            SidebarAction::Navigate(delta) => {
                // Without a known item_count at reduce time we apply a generous
                // upper bound and let the render layer re-clamp if needed.
                if delta > 0 {
                    self.selected_item =
                        self.selected_item.saturating_add(delta as usize);
                } else {
                    self.selected_item =
                        self.selected_item.saturating_sub((-delta) as usize);
                }
            }

            SidebarAction::ToggleExpand(node_id) => {
                if self.expanded_nodes.contains(&node_id) {
                    self.expanded_nodes.remove(&node_id);
                } else {
                    self.expanded_nodes.insert(node_id);
                }
            }

            SidebarAction::Scroll(delta) => {
                if delta > 0 {
                    self.scroll_offset =
                        self.scroll_offset.saturating_add(delta as usize);
                } else {
                    self.scroll_offset =
                        self.scroll_offset.saturating_sub((-delta) as usize);
                }
            }
        }
    }
}

impl Default for SidebarState {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_switch_resets_selection_and_scroll() {
        let mut state = SidebarState::new();
        state.reduce(SidebarAction::Navigate(5));
        state.reduce(SidebarAction::Scroll(3));
        assert_eq!(state.selected_item(), 5);
        assert_eq!(state.scroll_offset(), 3);

        state.reduce(SidebarAction::SwitchTab(SidebarTab::Subagents));
        assert_eq!(state.active_tab(), SidebarTab::Subagents);
        assert_eq!(state.selected_item(), 0);
        assert_eq!(state.scroll_offset(), 0);
    }

    #[test]
    fn navigate_down_increases_selection() {
        let mut state = SidebarState::new();
        state.reduce(SidebarAction::Navigate(3));
        assert_eq!(state.selected_item(), 3);
    }

    #[test]
    fn navigate_up_clamps_at_zero() {
        let mut state = SidebarState::new();
        state.reduce(SidebarAction::Navigate(-10));
        assert_eq!(state.selected_item(), 0);
    }

    #[test]
    fn navigate_clamped_with_item_count() {
        let mut state = SidebarState::new();
        let item_count = 5;
        state.navigate(100, item_count);
        assert_eq!(state.selected_item(), 4); // max index = item_count - 1
    }

    #[test]
    fn toggle_expand_toggles_set_membership() {
        let mut state = SidebarState::new();
        assert!(!state.is_expanded("node1"));

        state.reduce(SidebarAction::ToggleExpand("node1".into()));
        assert!(state.is_expanded("node1"));

        state.reduce(SidebarAction::ToggleExpand("node1".into()));
        assert!(!state.is_expanded("node1"));
    }

    #[test]
    fn toggle_expand_independent_nodes() {
        let mut state = SidebarState::new();
        state.reduce(SidebarAction::ToggleExpand("a".into()));
        state.reduce(SidebarAction::ToggleExpand("b".into()));
        assert!(state.is_expanded("a"));
        assert!(state.is_expanded("b"));

        state.reduce(SidebarAction::ToggleExpand("a".into()));
        assert!(!state.is_expanded("a"));
        assert!(state.is_expanded("b"));
    }

    #[test]
    fn scroll_adjusts_offset() {
        let mut state = SidebarState::new();
        state.reduce(SidebarAction::Scroll(4));
        assert_eq!(state.scroll_offset(), 4);
        state.reduce(SidebarAction::Scroll(-2));
        assert_eq!(state.scroll_offset(), 2);
        state.reduce(SidebarAction::Scroll(-10));
        assert_eq!(state.scroll_offset(), 0);
    }

    #[test]
    fn default_tab_is_tasks() {
        let state = SidebarState::new();
        assert_eq!(state.active_tab(), SidebarTab::Tasks);
    }
}
