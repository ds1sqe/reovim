use super::*;

struct MockFocusManager {
    current: Focus,
    last_panel: u64,
}

impl MockFocusManager {
    fn new(initial_viewport: u64) -> Self {
        Self {
            current: Focus::Panel(initial_viewport),
            last_panel: initial_viewport,
        }
    }
}

impl FocusManager for MockFocusManager {
    fn current(&self) -> &Focus {
        &self.current
    }

    fn focus_panel(&mut self, viewport_id: u64) {
        self.last_panel = viewport_id;
        self.current = Focus::Panel(viewport_id);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn focus_overlay(&mut self, overlay_id: &str) {
        // Remember the last panel before switching to overlay
        if let Focus::Panel(id) = self.current {
            self.last_panel = id;
        }
        self.current = Focus::Overlay(overlay_id.to_string());
    }

    fn return_to_panel(&mut self) {
        self.current = Focus::Panel(self.last_panel);
    }
}

// Focus enum tests
#[test]
fn test_focus_panel() {
    let focus = Focus::panel(42);
    assert!(focus.is_panel());
    assert!(!focus.is_overlay());
    assert_eq!(focus.viewport_id(), Some(42));
    assert_eq!(focus.overlay_id(), None);
}

#[test]
fn test_focus_overlay() {
    let focus = Focus::overlay("completion");
    assert!(!focus.is_panel());
    assert!(focus.is_overlay());
    assert_eq!(focus.viewport_id(), None);
    assert_eq!(focus.overlay_id(), Some("completion"));
}

// FocusManager tests
#[test]
fn test_focus_manager_initial() {
    let manager = MockFocusManager::new(1);
    assert!(manager.current().is_panel());
    assert!(manager.is_panel_focused(1));
}

#[test]
fn test_focus_manager_focus_panel() {
    let mut manager = MockFocusManager::new(1);
    manager.focus_panel(2);
    assert!(manager.is_panel_focused(2));
    assert!(!manager.is_panel_focused(1));
}

#[test]
fn test_focus_manager_focus_overlay() {
    let mut manager = MockFocusManager::new(1);
    manager.focus_overlay("completion");
    assert!(manager.is_overlay_focused("completion"));
    assert!(manager.has_overlay_focus());
    assert!(!manager.is_panel_focused(1));
}

#[test]
fn test_focus_manager_return_to_panel() {
    let mut manager = MockFocusManager::new(1);
    manager.focus_overlay("completion");
    assert!(manager.has_overlay_focus());

    manager.return_to_panel();
    assert!(manager.is_panel_focused(1));
    assert!(!manager.has_overlay_focus());
}

#[test]
fn test_focus_manager_remembers_last_panel() {
    let mut manager = MockFocusManager::new(1);
    manager.focus_panel(2);
    manager.focus_overlay("completion");
    manager.return_to_panel();

    // Should return to panel 2, not 1
    assert!(manager.is_panel_focused(2));
}
