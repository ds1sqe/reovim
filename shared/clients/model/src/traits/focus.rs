//! Focus management traits.
//!
//! These types and traits manage which UI element (panel or overlay)
//! has keyboard focus.

/// Current focus target.
///
/// Focus can be on either a panel (editor window) or an overlay
/// (popup like completion menu).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Focus {
    /// Focus is on a panel (viewport).
    Panel(u64),
    /// Focus is on an overlay.
    Overlay(String),
}

impl Focus {
    /// Create a panel focus.
    #[must_use]
    pub const fn panel(viewport_id: u64) -> Self {
        Self::Panel(viewport_id)
    }

    /// Create an overlay focus.
    #[must_use]
    pub fn overlay(overlay_id: impl Into<String>) -> Self {
        Self::Overlay(overlay_id.into())
    }

    /// Check if focus is on a panel.
    #[must_use]
    pub const fn is_panel(&self) -> bool {
        matches!(self, Self::Panel(_))
    }

    /// Check if focus is on an overlay.
    #[must_use]
    pub const fn is_overlay(&self) -> bool {
        matches!(self, Self::Overlay(_))
    }

    /// Get the focused viewport ID, if any.
    #[must_use]
    pub const fn viewport_id(&self) -> Option<u64> {
        match self {
            Self::Panel(id) => Some(*id),
            Self::Overlay(_) => None,
        }
    }

    /// Get the focused overlay ID, if any.
    #[must_use]
    pub fn overlay_id(&self) -> Option<&str> {
        match self {
            Self::Panel(_) => None,
            Self::Overlay(id) => Some(id),
        }
    }
}

/// Trait for managing focus between panels and overlays.
///
/// The focus manager tracks what has keyboard focus and handles
/// transitions between panels and overlays.
pub trait FocusManager {
    /// Get the current focus.
    fn current(&self) -> &Focus;

    /// Focus a specific panel (viewport).
    fn focus_panel(&mut self, viewport_id: u64);

    /// Focus a specific overlay.
    fn focus_overlay(&mut self, overlay_id: &str);

    /// Return focus to the last focused panel.
    ///
    /// Called when an overlay is dismissed.
    fn return_to_panel(&mut self);

    /// Check if the given panel has focus.
    fn is_panel_focused(&self, viewport_id: u64) -> bool {
        matches!(self.current(), Focus::Panel(id) if *id == viewport_id)
    }

    /// Check if the given overlay has focus.
    fn is_overlay_focused(&self, overlay_id: &str) -> bool {
        matches!(self.current(), Focus::Overlay(id) if id == overlay_id)
    }

    /// Check if any overlay has focus.
    fn has_overlay_focus(&self) -> bool {
        self.current().is_overlay()
    }
}

#[cfg(test)]
mod tests {
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
}
