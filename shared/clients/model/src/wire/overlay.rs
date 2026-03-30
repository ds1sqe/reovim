//! Overlay types for popups, menus, and floating UI elements.
//!
//! Overlays are logical UI elements sent from the server. The client
//! is responsible for rendering them according to platform capabilities.

use serde::{Deserialize, Serialize};

use super::origin::SemanticOrigin;

/// State of an interactive overlay (e.g., completion menu).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
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
/// rendered state appropriate for its platform.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct LogicalOverlay {
    /// Unique identifier for this overlay instance.
    pub id: String,
    /// Semantic origin — WHAT this data relates to (informational, not directive).
    /// Clients decide positioning independently based on this metadata.
    pub origin: Option<SemanticOrigin>,
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
    pub fn new(id: impl Into<String>, kind: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            origin: None,
            kind: kind.into(),
            data: serde_json::Value::Null,
            state: OverlayState::new(),
            priority: 0,
        }
    }

    /// Set the semantic origin.
    #[must_use]
    pub const fn with_origin(mut self, origin: SemanticOrigin) -> Self {
        self.origin = Some(origin);
        self
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
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn with_state(mut self, state: OverlayState) -> Self {
        self.state = state;
        self
    }
}

#[cfg(test)]
#[path = "overlay_tests.rs"]
mod tests;
