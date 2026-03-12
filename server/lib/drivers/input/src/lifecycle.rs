//! Mode lifecycle callbacks.
//!
//! This module provides the `ModeLifecycleHandler` trait for hooking into
//! mode transitions.
//!
//! # Architecture
//!
//! Policy modules implement `ModeLifecycleHandler` to perform actions when
//! modes change. For example, the vim module might:
//!
//! - Create an undo group when entering insert mode
//! - Commit the undo group when exiting insert mode
//! - Update the statusline on any mode change
//!
//! # Example
//!
//! ```ignore
//! use reovim_driver_input::ModeLifecycleHandler;
//! use reovim_kernel::api::v1::ModeId;
//!
//! struct VimLifecycleHandler;
//!
//! impl ModeLifecycleHandler for VimLifecycleHandler {
//!     fn on_input_mode_enter(&mut self, mode: &ModeId) {
//!         // Create undo group for text input
//!     }
//!
//!     fn on_input_mode_exit(&mut self, mode: &ModeId) {
//!         // Commit undo group
//!     }
//! }
//! ```

use reovim_kernel::api::v1::ModeId;

/// Mode lifecycle handler trait.
///
/// Policy modules implement this to hook into mode transitions.
/// The runner calls these methods at the appropriate times.
pub trait ModeLifecycleHandler: Send + Sync {
    /// Called when entering an input-accepting mode.
    ///
    /// For example, when transitioning from Normal to Insert mode.
    /// Use this to set up state for text input (e.g., start undo group).
    fn on_input_mode_enter(&mut self, mode: &ModeId);

    /// Called when exiting an input-accepting mode.
    ///
    /// For example, when transitioning from Insert to Normal mode.
    /// Use this to finalize state (e.g., commit undo group).
    fn on_input_mode_exit(&mut self, mode: &ModeId);

    /// Called on any mode transition.
    ///
    /// Called after `on_input_mode_enter` or `on_input_mode_exit` if applicable.
    /// Default implementation does nothing.
    fn on_mode_change(&mut self, from: &ModeId, to: &ModeId) {
        let _ = (from, to); // Default: no-op
    }

    /// Called when exiting a mode (before pop/set).
    ///
    /// Policy modules can use this to clean up mode-specific state.
    /// For example, the vim module clears pending operator state when
    /// exiting operator-pending mode.
    ///
    /// Default implementation does nothing.
    fn on_mode_exit(&mut self, mode: &ModeId) {
        let _ = mode; // Default: no-op
    }
}

/// No-op lifecycle handler for testing.
///
/// Implements all methods as no-ops.
#[derive(Debug, Default, Clone, Copy)]
pub struct NopLifecycleHandler;

impl ModeLifecycleHandler for NopLifecycleHandler {
    fn on_input_mode_enter(&mut self, _mode: &ModeId) {}
    fn on_input_mode_exit(&mut self, _mode: &ModeId) {}
}

// ============================================================================
// Tests
// ============================================================================
