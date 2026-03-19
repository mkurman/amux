use crate::theme::{ThemeTokens, ROUNDED_BORDER, FG_CLOSE};
use crate::state::sidebar::SidebarState;
use crate::state::task::TaskState;

/// Render the context sidebar (right pane in two-pane layout)
pub fn sidebar_widget(
    sidebar: &SidebarState,
    tasks: &TaskState,
    theme: &ThemeTokens,
    focused: bool,
    width: usize,
    height: usize,
) -> Vec<String> {
    let border_color = if focused { theme.accent_primary } else { theme.fg_dim };
    let bc = border_color.fg();
    let b = &ROUNDED_BORDER;
    let inner_width = width.saturating_sub(2);
    let inner_height = height.saturating_sub(2);

    let mut result = Vec::new();

    // Title bar with tabs — escape literal brackets around tab labels
    let title = format!(
        " {} {} ",
        if sidebar.active_tab() == crate::state::sidebar::SidebarTab::Tasks {
            format!("{}\\[Tasks]{}", theme.fg_active.fg(), bc)
        } else {
            format!("{}Tasks{}", theme.fg_dim.fg(), bc)
        },
        if sidebar.active_tab() == crate::state::sidebar::SidebarTab::Subagents {
            format!("{}\\[Subagents]{}", theme.fg_active.fg(), bc)
        } else {
            format!("{}Subagents{}", theme.fg_dim.fg(), bc)
        },
    );
    let title_visible_len = crate::widgets::strip_markup_len(&title);
    let remaining = inner_width.saturating_sub(title_visible_len);

    result.push(format!(
        "{}{}{}{}{}{}{}",
        bc, b.top_left,
        super::repeat_char(b.horizontal, 1),
        title,
        super::repeat_char(b.horizontal, remaining.saturating_sub(1).min(inner_width)),
        b.top_right,
        FG_CLOSE,
    ));

    // Body — routed to real widgets based on active tab
    let body_lines = match sidebar.active_tab() {
        crate::state::sidebar::SidebarTab::Tasks => {
            super::task_tree::task_tree_widget(tasks, sidebar, theme, inner_width, inner_height)
        }
        crate::state::sidebar::SidebarTab::Subagents => {
            super::subagents::subagents_widget(tasks, sidebar, theme, inner_width, inner_height)
        }
    };

    for line in &body_lines {
        // Fit content to inner_width so border chars don't get pushed past sidebar edge
        let fitted = super::fit_to_width(line, inner_width);
        result.push(format!("{}{}{}{}{}", bc, b.vertical, fitted, b.vertical, FG_CLOSE));
    }

    // Bottom border
    result.push(format!(
        "{}{}{}{}{}",
        bc, b.bottom_left,
        super::repeat_char(b.horizontal, inner_width),
        b.bottom_right,
        FG_CLOSE,
    ));

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::sidebar::SidebarState;
    use crate::state::task::TaskState;

    #[test]
    fn sidebar_widget_returns_correct_height() {
        let sidebar = SidebarState::new();
        let tasks = TaskState::new();
        let theme = ThemeTokens::default();
        let lines = sidebar_widget(&sidebar, &tasks, &theme, false, 40, 20);
        assert_eq!(lines.len(), 20);
    }

    #[test]
    fn sidebar_widget_min_height() {
        let sidebar = SidebarState::new();
        let tasks = TaskState::new();
        let theme = ThemeTokens::default();
        // height=2: top border + bottom border, no body rows
        let lines = sidebar_widget(&sidebar, &tasks, &theme, false, 40, 2);
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn sidebar_widget_focused_changes_border_color() {
        let sidebar = SidebarState::new();
        let tasks = TaskState::new();
        let theme = ThemeTokens::default();
        let unfocused = sidebar_widget(&sidebar, &tasks, &theme, false, 40, 10);
        let focused = sidebar_widget(&sidebar, &tasks, &theme, true, 40, 10);
        // Focused should use accent_primary color, unfocused fg_dim — they should differ
        assert_ne!(unfocused[0], focused[0]);
    }
}
