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
#[path = "operator_state_tests.rs"]
mod tests;
