use ratatui::prelude::*;
use ratatui::text::Span;
use ratatui::widgets::{Block, Borders, BorderType, Tabs};

use crate::state::sidebar::SidebarState;
use crate::state::task::TaskState;
use crate::theme::ThemeTokens;

pub fn render(
    frame: &mut Frame,
    area: Rect,
    sidebar: &SidebarState,
    tasks: &TaskState,
    theme: &ThemeTokens,
    focused: bool,
) {
    let border_style = if focused {
        theme.accent_primary
    } else {
        theme.fg_dim
    };

    // Tab titles
    let active_tab = sidebar.active_tab();
    let tab_index = match active_tab {
        crate::state::sidebar::SidebarTab::Tasks => 0,
        crate::state::sidebar::SidebarTab::Subagents => 1,
    };

    let tabs = Tabs::new(vec!["Tasks", "Subagents"])
        .select(tab_index)
        .style(theme.fg_dim)
        .highlight_style(theme.fg_active)
        .divider(Span::styled(" | ", theme.fg_dim));

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_style);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height < 2 {
        return;
    }

    // Split inner into tabs line + body
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(inner);

    frame.render_widget(tabs, chunks[0]);

    // Body -- routed to real widgets based on active tab
    match active_tab {
        crate::state::sidebar::SidebarTab::Tasks => {
            super::task_tree::render(frame, chunks[1], tasks, sidebar, theme);
        }
        crate::state::sidebar::SidebarTab::Subagents => {
            super::subagents::render(frame, chunks[1], tasks, sidebar, theme);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::sidebar::SidebarState;
    use crate::state::task::TaskState;

    #[test]
    fn sidebar_handles_empty_state() {
        let sidebar = SidebarState::new();
        let tasks = TaskState::new();
        let _theme = ThemeTokens::default();
        assert_eq!(
            sidebar.active_tab(),
            crate::state::sidebar::SidebarTab::Tasks
        );
        assert!(tasks.tasks().is_empty());
    }
}
