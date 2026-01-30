//! Overlay types for popups, menus, and floating UI elements.
//!
//! Overlays are logical UI elements sent from the server. The client
//! is responsible for rendering them according to platform capabilities.

use super::Anchor;

/// State of an interactive overlay (e.g., completion menu).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OverlayState {
    /// Currently selected index (for lists).
    pub selected_index: Option<u32>,
    /// Current filter text (for fuzzy matching).
    pub filter: String,
    /// Scroll offset for long lists.
    pub scroll_offset: u32,
    /// Whether the overlay is loading data.
    pub loading: bool,
}

impl OverlayState {
    /// Create a new overlay state with default values.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            selected_index: None,
            filter: String::new(),
            scroll_offset: 0,
            loading: false,
        }
    }

    /// Create a state with a selected index.
    #[must_use]
    pub const fn with_selection(index: u32) -> Self {
        Self {
            selected_index: Some(index),
            filter: String::new(),
            scroll_offset: 0,
            loading: false,
        }
    }
}

/// A logical overlay from the server.
///
/// Contains all information needed to render an overlay, but no
/// platform-specific details. The client interprets this into
/// rendered state using its `OverlayRenderer` implementation.
#[derive(Debug, Clone, PartialEq)]
pub struct LogicalOverlay {
    /// Unique identifier for this overlay instance.
    pub id: String,
    /// Anchor point for positioning.
    pub anchor: Anchor,
    /// Overlay type (e.g., "completion", "hover", "signature").
    pub kind: String,
    /// Payload data (contents depend on kind).
    pub data: serde_json::Value,
    /// Interactive state.
    pub state: OverlayState,
    /// Priority for z-ordering (higher = on top).
    pub priority: u32,
}

impl LogicalOverlay {
    /// Create a new logical overlay.
    #[must_use]
    pub fn new(id: impl Into<String>, kind: impl Into<String>, anchor: Anchor) -> Self {
        Self {
            id: id.into(),
            anchor,
            kind: kind.into(),
            data: serde_json::Value::Null,
            state: OverlayState::new(),
            priority: 0,
        }
    }

    /// Set the data payload.
    #[must_use]
    pub fn with_data(mut self, data: serde_json::Value) -> Self {
        self.data = data;
        self
    }

    /// Set the priority.
    #[must_use]
    pub const fn with_priority(mut self, priority: u32) -> Self {
        self.priority = priority;
        self
    }

    /// Set the initial state.
    #[must_use]
    pub fn with_state(mut self, state: OverlayState) -> Self {
        self.state = state;
        self
    }

    /// Check if this is a completion overlay.
    #[must_use]
    pub fn is_completion(&self) -> bool {
        self.kind == "completion"
    }

    /// Check if this is a hover overlay.
    #[must_use]
    pub fn is_hover(&self) -> bool {
        self.kind == "hover"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_overlay_state_default() {
        let state = OverlayState::new();
        assert_eq!(state.selected_index, None);
        assert!(state.filter.is_empty());
        assert_eq!(state.scroll_offset, 0);
        assert!(!state.loading);
    }

    #[test]
    fn test_overlay_state_with_selection() {
        let state = OverlayState::with_selection(5);
        assert_eq!(state.selected_index, Some(5));
    }

    #[test]
    fn test_logical_overlay_new() {
        let overlay = LogicalOverlay::new("comp-1", "completion", Anchor::Cursor);
        assert_eq!(overlay.id, "comp-1");
        assert_eq!(overlay.kind, "completion");
        assert!(overlay.is_completion());
        assert!(!overlay.is_hover());
    }

    #[test]
    fn test_logical_overlay_builder() {
        let overlay = LogicalOverlay::new("hover-1", "hover", Anchor::Center)
            .with_data(serde_json::json!({"text": "Hello"}))
            .with_priority(10);

        assert_eq!(overlay.id, "hover-1");
        assert!(overlay.is_hover());
        assert_eq!(overlay.priority, 10);
        assert_eq!(overlay.data["text"], "Hello");
    }

    #[test]
    fn test_overlay_serialization_roundtrip() {
        let data = serde_json::json!({
            "items": ["foo", "bar", "baz"],
            "count": 3
        });
        let overlay =
            LogicalOverlay::new("test", "completion", Anchor::Cursor).with_data(data.clone());

        // Verify data is preserved
        assert_eq!(overlay.data, data);
        assert_eq!(overlay.data["items"][0], "foo");
        assert_eq!(overlay.data["count"], 3);
    }
}
