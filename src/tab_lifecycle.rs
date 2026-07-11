#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseOutcome {
    Ignored,
    Activate(u64),
    KeepActive(u64),
    CreateBlank,
}

/// Removes a tab and determines the next active tab. The browser/window teardown
/// is deliberately left to the caller so a tab close cannot become app quit.
pub fn close_tab(tabs: &mut Vec<u64>, active: u64, tab_id: u64) -> CloseOutcome {
    let Some(index) = tabs.iter().position(|id| *id == tab_id) else {
        return CloseOutcome::Ignored;
    };
    tabs.remove(index);
    if tabs.is_empty() {
        return CloseOutcome::CreateBlank;
    }
    if active != tab_id {
        return CloseOutcome::KeepActive(active);
    }
    CloseOutcome::Activate(tabs[index.min(tabs.len() - 1)])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closes_one_of_multiple_tabs() {
        let mut tabs = vec![1, 2, 3];
        assert_eq!(close_tab(&mut tabs, 1, 2), CloseOutcome::KeepActive(1));
        assert_eq!(tabs, vec![1, 3]);
    }

    #[test]
    fn closing_active_tab_activates_right_neighbor() {
        let mut tabs = vec![1, 2, 3];
        assert_eq!(close_tab(&mut tabs, 2, 2), CloseOutcome::Activate(3));
    }

    #[test]
    fn closing_active_last_position_activates_left_neighbor() {
        let mut tabs = vec![1, 2, 3];
        assert_eq!(close_tab(&mut tabs, 3, 3), CloseOutcome::Activate(2));
    }

    #[test]
    fn closing_background_tab_preserves_active_tab() {
        let mut tabs = vec![1, 2, 3];
        assert_eq!(close_tab(&mut tabs, 2, 1), CloseOutcome::KeepActive(2));
    }

    #[test]
    fn closing_last_tab_requests_blank_tab_not_app_exit() {
        let mut tabs = vec![7];
        assert_eq!(close_tab(&mut tabs, 7, 7), CloseOutcome::CreateBlank);
        assert!(tabs.is_empty());
    }

    #[test]
    fn unknown_tab_is_ignored() {
        let mut tabs = vec![1];
        assert_eq!(close_tab(&mut tabs, 1, 99), CloseOutcome::Ignored);
    }
}
