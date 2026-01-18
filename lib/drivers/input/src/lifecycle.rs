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
//! use reovim_driver_input::{ModeLifecycleHandler, ModeRegistry};
//! use reovim_kernel::api::v1::ModeId;
//!
//! struct VimLifecycleHandler {
//!     registry: ModeRegistry,
//! }
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

#[cfg(test)]
mod tests {
    use {super::*, reovim_kernel::api::v1::ModuleId};

    #[test]
    fn test_nop_lifecycle_handler() {
        let mut handler = NopLifecycleHandler;
        let mode = ModeId::with_discriminant(ModuleId::new("test"), "INSERT", 1);

        // Should not panic
        handler.on_input_mode_enter(&mode);
        handler.on_input_mode_exit(&mode);
        handler.on_mode_change(&mode, &mode);
        handler.on_mode_exit(&mode);
    }

    #[test]
    fn test_lifecycle_handler_object_safe() {
        // Verify the trait is object-safe
        fn _accepts_ref(_: &dyn ModeLifecycleHandler) {}
        fn _accepts_box(_: Box<dyn ModeLifecycleHandler>) {}
    }

    // Test implementation to verify trait works
    #[allow(clippy::struct_field_names)]
    struct TestLifecycleHandler {
        enter_count: usize,
        exit_count: usize,
        change_count: usize,
        mode_exit_count: usize,
    }

    impl TestLifecycleHandler {
        fn new() -> Self {
            Self {
                enter_count: 0,
                exit_count: 0,
                change_count: 0,
                mode_exit_count: 0,
            }
        }
    }

    impl ModeLifecycleHandler for TestLifecycleHandler {
        fn on_input_mode_enter(&mut self, _mode: &ModeId) {
            self.enter_count += 1;
        }

        fn on_input_mode_exit(&mut self, _mode: &ModeId) {
            self.exit_count += 1;
        }

        fn on_mode_change(&mut self, _from: &ModeId, _to: &ModeId) {
            self.change_count += 1;
        }

        fn on_mode_exit(&mut self, _mode: &ModeId) {
            self.mode_exit_count += 1;
        }
    }

    #[test]
    fn test_lifecycle_handler_implementation() {
        let mut handler = TestLifecycleHandler::new();
        let mode = ModeId::with_discriminant(ModuleId::new("test"), "INSERT", 1);

        assert_eq!(handler.enter_count, 0);
        handler.on_input_mode_enter(&mode);
        assert_eq!(handler.enter_count, 1);

        assert_eq!(handler.exit_count, 0);
        handler.on_input_mode_exit(&mode);
        assert_eq!(handler.exit_count, 1);

        assert_eq!(handler.change_count, 0);
        handler.on_mode_change(&mode, &mode);
        assert_eq!(handler.change_count, 1);

        assert_eq!(handler.mode_exit_count, 0);
        handler.on_mode_exit(&mode);
        assert_eq!(handler.mode_exit_count, 1);
    }
}
