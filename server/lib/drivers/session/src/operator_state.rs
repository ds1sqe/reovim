//! Operator-pending state for text object range communication.
//!
//! This module provides [`OperatorPendingState`], a session extension that
//! enables communication between text object commands and operator resolvers.
//!
//! # Design
//!
//! Text objects (like `iw`, `aw`, `i"`) calculate ranges directly instead of
//! moving the cursor. They need a way to communicate that range to the
//! operator resolver (like `VimDeleteResolver`).
//!
//! This is a **mechanism** (session driver) that enables **policy** (modules)
//! to communicate. The vim module's `VimSessionState` can delegate to this,
//! or modules can use it directly.
//!
//! # Example
//!
//! ```ignore
//! use reovim_driver_session::{OperatorPendingState, TextObjRange};
//! use reovim_kernel::api::v1::Position;
//!
//! // In a text object command:
//! let state = runtime.ext_mut::<OperatorPendingState>();
//! state.set_textobj_range(TextObjRange::characterwise(
//!     Position::new(0, 0),
//!     Position::new(0, 5),
//! ));
//!
//! // In an operator resolver's on_command_complete:
//! let state = extensions.get_mut::<OperatorPendingState>();
//! if let Some(range) = state.and_then(|s| s.take_textobj_range()) {
//!     // Use the range for the operator
//! }
//! ```

use crate::{SessionExtension, TextObjRange};

/// Session extension for operator-pending state.
///
/// This provides a communication channel between text object commands
/// and operator resolvers. Text objects SET the range, operators CONSUME it.
///
/// # Thread Safety
///
/// This implements `Send + Sync` as required by `SessionExtension`.
/// Access should be synchronized at the session level.
#[derive(Debug, Default)]
pub struct OperatorPendingState {
    /// Pending text object range.
    ///
    /// Set by text object commands, consumed by operator resolvers.
    pending_textobj_range: Option<TextObjRange>,
}

impl SessionExtension for OperatorPendingState {
    fn create() -> Self {
        Self::default()
    }
}

impl OperatorPendingState {
    /// Create a new empty operator-pending state.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            pending_textobj_range: None,
        }
    }

    /// Set the pending text object range.
    ///
    /// Called by text object commands after calculating the range.
    pub const fn set_textobj_range(&mut self, range: TextObjRange) {
        self.pending_textobj_range = Some(range);
    }

    /// Take the pending text object range, clearing it.
    ///
    /// Called by operator resolvers in `on_command_complete`.
    /// Uses `take()` semantics to ensure the range is consumed exactly once.
    pub const fn take_textobj_range(&mut self) -> Option<TextObjRange> {
        self.pending_textobj_range.take()
    }

    /// Check if there's a pending text object range.
    #[must_use]
    pub const fn has_textobj_range(&self) -> bool {
        self.pending_textobj_range.is_some()
    }

    /// Clear all pending state.
    ///
    /// Called when an operation is cancelled.
    pub const fn clear(&mut self) {
        self.pending_textobj_range = None;
    }
}

#[cfg(test)]
mod tests {
    use reovim_kernel::api::v1::Position;

    use super::*;

    #[test]
    fn test_default_state() {
        let state = OperatorPendingState::default();
        assert!(!state.has_textobj_range());
    }

    #[test]
    fn test_set_and_take_textobj_range() {
        let mut state = OperatorPendingState::new();

        // Set the range
        let range = TextObjRange::characterwise(Position::new(0, 0), Position::new(0, 5));
        state.set_textobj_range(range);
        assert!(state.has_textobj_range());

        // Take consumes the range
        let taken = state.take_textobj_range();
        assert!(taken.is_some());
        assert_eq!(taken.unwrap().start, Position::new(0, 0));
        assert_eq!(taken.unwrap().end, Position::new(0, 5));

        // Now it's gone
        assert!(!state.has_textobj_range());
        assert!(state.take_textobj_range().is_none());
    }

    #[test]
    fn test_clear() {
        let mut state = OperatorPendingState::new();

        // Set the range
        let range = TextObjRange::linewise(Position::new(1, 0), Position::new(3, 0));
        state.set_textobj_range(range);
        assert!(state.has_textobj_range());

        // Clear removes it
        state.clear();
        assert!(!state.has_textobj_range());
    }

    #[test]
    fn test_take_returns_none_when_empty() {
        let mut state = OperatorPendingState::new();
        assert!(state.take_textobj_range().is_none());
    }

    #[test]
    fn test_session_extension_create() {
        let state = OperatorPendingState::create();
        assert!(!state.has_textobj_range());
    }

    #[test]
    fn test_set_overwrites_previous() {
        let mut state = OperatorPendingState::new();

        let range1 = TextObjRange::characterwise(Position::new(0, 0), Position::new(0, 5));
        state.set_textobj_range(range1);

        let range2 = TextObjRange::linewise(Position::new(1, 0), Position::new(3, 0));
        state.set_textobj_range(range2);

        // Should get the second range
        let taken = state.take_textobj_range().unwrap();
        assert!(taken.is_linewise);
        assert_eq!(taken.start, Position::new(1, 0));
    }

    #[test]
    fn test_clear_on_empty_is_noop() {
        let mut state = OperatorPendingState::new();
        state.clear(); // Should not panic
        assert!(!state.has_textobj_range());
    }

    #[test]
    fn test_double_take_returns_none() {
        let mut state = OperatorPendingState::new();
        let range = TextObjRange::characterwise(Position::new(0, 0), Position::new(0, 5));
        state.set_textobj_range(range);

        assert!(state.take_textobj_range().is_some());
        assert!(state.take_textobj_range().is_none()); // Second take returns None
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_debug() {
        let state = OperatorPendingState::new();
        let debug = format!("{state:?}");
        assert!(debug.contains("OperatorPendingState"));
    }

    #[test]
    fn test_default_vs_new() {
        let default = OperatorPendingState::default();
        let new = OperatorPendingState::new();
        assert!(!default.has_textobj_range());
        assert!(!new.has_textobj_range());
    }
}
