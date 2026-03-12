//! Mode stack operations.
//!
//! This module provides the [`ModeApi`] trait for mode stack manipulation.
//! Resolvers and commands use this to push, pop, and query modes.
//!
//! # Design
//!
//! Following Unix philosophy: this trait does ONE thing well - mode management.
//!
//! # Example
//!
//! ```ignore
//! use reovim_driver_session::api::ModeApi;
//!
//! fn handle_escape<S: ModeApi>(session: &mut S) {
//!     // Pop back to previous mode (or stay at home mode)
//!     let _ = session.pop_mode(None);
//! }
//! ```

use reovim_kernel::api::v1::ModeId;

// Re-export transition types for convenience
pub use crate::transition::{PopResult, TransitionContext};

/// Mode stack operations.
///
/// Provides access to the editing mode stack for resolvers and commands.
/// The mode stack follows vim-style semantics: modes can be pushed and
/// popped, with a "home" mode that cannot be removed.
pub trait ModeApi: Send {
    /// Get the current mode (top of stack).
    fn current_mode(&self) -> &ModeId;

    /// Get the home mode (bottom of stack, cannot be popped).
    fn home_mode(&self) -> &ModeId;

    /// Get the mode stack depth.
    fn mode_depth(&self) -> usize;

    /// Check if a mode is anywhere on the stack.
    fn is_mode_active(&self, mode: &ModeId) -> bool;

    /// Get the full mode stack as a snapshot (bottom to top).
    fn mode_stack(&self) -> Vec<ModeId>;

    /// Push a mode onto the stack.
    ///
    /// The new mode becomes the current mode. The previous mode
    /// remains on the stack and can be returned to via `pop_mode`.
    fn push_mode(&mut self, mode: ModeId, ctx: TransitionContext);

    /// Pop the current mode from the stack.
    ///
    /// Returns `Ok(())` if successful, or `Err(ModeError::CannotPopHomeMode)`
    /// if trying to pop the home mode.
    ///
    /// # Arguments
    ///
    /// * `result` - Optional result to pass to the parent mode
    ///
    /// # Errors
    ///
    /// Returns [`ModeError::CannotPopHomeMode`] if only the home mode remains.
    fn pop_mode(&mut self, result: Option<PopResult>) -> Result<(), ModeError>;

    /// Replace the current mode (pop + push atomically).
    ///
    /// This is equivalent to popping and pushing, but works even when
    /// only the home mode exists.
    fn set_mode(&mut self, mode: ModeId, ctx: TransitionContext);
}

/// Errors from mode operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModeError {
    /// Cannot pop the home mode.
    CannotPopHomeMode,
}

impl std::fmt::Display for ModeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CannotPopHomeMode => write!(f, "cannot pop home mode"),
        }
    }
}

impl std::error::Error for ModeError {}
#[cfg(test)]
#[path = "tests/mode.rs"]
mod tests;
