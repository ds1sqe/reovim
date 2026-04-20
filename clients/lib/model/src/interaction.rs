//! Interaction types for overlay input handling.
//!
//! These types represent user interactions with overlays (like completion
//! menus) and the results of those interactions.

use serde::{Deserialize, Serialize};

/// User interaction with an overlay.
///
/// Represents actions the user can take when interacting with
/// an overlay like a completion menu or command palette.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub enum Interaction {
    /// Select the next item in a list.
    SelectNext,

    /// Select the previous item in a list.
    SelectPrev,

    /// Select a specific item by index.
    SelectIndex(u32),

    /// Filter the list by the given text.
    Filter(String),

    /// Confirm the current selection.
    Confirm,

    /// Cancel/dismiss the overlay.
    Cancel,

    /// Scroll down by the given number of items.
    ScrollDown(u32),

    /// Scroll up by the given number of items.
    ScrollUp(u32),

    /// Page down (scroll by visible height).
    PageDown,

    /// Page up (scroll by visible height).
    PageUp,

    /// Custom interaction (renderer-specific).
    Custom(String),
}

impl Interaction {
    /// Create a filter interaction.
    #[must_use]
    pub fn filter(text: impl Into<String>) -> Self {
        Self::Filter(text.into())
    }

    /// Create a custom interaction.
    #[must_use]
    pub fn custom(action: impl Into<String>) -> Self {
        Self::Custom(action.into())
    }

    /// Check if this is a navigation interaction.
    #[must_use]
    pub const fn is_navigation(&self) -> bool {
        matches!(
            self,
            Self::SelectNext
                | Self::SelectPrev
                | Self::SelectIndex(_)
                | Self::ScrollDown(_)
                | Self::ScrollUp(_)
                | Self::PageDown
                | Self::PageUp
        )
    }

    /// Check if this is a terminal interaction (ends the overlay).
    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        matches!(self, Self::Confirm | Self::Cancel)
    }
}

/// Result of handling an interaction.
///
/// Tells the client what to do after an overlay processes an interaction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub enum InteractionResult {
    /// The overlay's state was updated.
    ///
    /// The client should re-render the overlay with the new state.
    StateUpdate(crate::wire::OverlayState),

    /// The user confirmed a selection.
    ///
    /// Contains the selected value (type depends on overlay kind).
    Confirm(serde_json::Value),

    /// The user cancelled the overlay.
    Cancel,

    /// The interaction should be passed to the next handler.
    ///
    /// The overlay didn't handle this interaction; try the next
    /// handler in the chain (e.g., the editor).
    PassThrough,

    /// The interaction was consumed but no state change.
    ///
    /// The overlay handled the interaction but nothing visible changed.
    Consumed,
}

impl InteractionResult {
    /// Create a confirmation result with the given value.
    #[must_use]
    pub fn confirm(value: impl Into<serde_json::Value>) -> Self {
        Self::Confirm(value.into())
    }

    /// Create a state update result.
    #[must_use]
    pub const fn state_update(state: crate::wire::OverlayState) -> Self {
        Self::StateUpdate(state)
    }

    /// Check if this result closes the overlay.
    #[must_use]
    pub const fn closes_overlay(&self) -> bool {
        matches!(self, Self::Confirm(_) | Self::Cancel)
    }

    /// Check if this result was handled (not passed through).
    #[must_use]
    pub const fn was_handled(&self) -> bool {
        !matches!(self, Self::PassThrough)
    }
}

#[cfg(test)]
#[path = "interaction_tests.rs"]
mod tests;
