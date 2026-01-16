//! Input fallback handler types for unmatched key events.
//!
//! This module defines the interface for handling keys that don't match
//! any keymap binding. The event loop (mechanism) delegates to a trait;
//! modules (policy) implement the trait to define behavior.
//!
//! # Design Philosophy
//!
//! Following the Linux kernel's "mechanism vs policy" principle:
//! - **Mechanism** (this trait): Defines WHAT can happen when keys don't match
//! - **Policy** (implementations): Decides HOW to handle unmatched keys
//!
//! # Architecture
//!
//! The `FallbackContext` trait abstracts the application state needed by
//! fallback handlers, allowing modules to implement handlers without
//! depending on runner types. This breaks the circular dependency where
//! modules would otherwise need to import from runner.
//!
//! ```text
//! lib/drivers/input/          <-- FallbackContext trait (this crate)
//!        ^
//!        |  (implements trait)
//!        |
//! runner/                     <-- AppState implements FallbackContext
//!        ^
//!        |  (uses trait)
//!        |
//! modules/editor/             <-- EditorFallbackHandler<C: FallbackContext>
//! ```

use std::sync::Arc;

use {
    reovim_arch::sync::RwLock,
    reovim_kernel::api::v1::{Buffer, BufferId, Edit, ModeId, Position},
};

use crate::KeyEvent;

/// Result of fallback key handling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FallbackResult {
    /// Key was processed successfully (e.g., character inserted).
    Handled,
    /// Key was ignored (no action taken).
    Ignored,
    /// Key was invalid for current context (show warning/beep).
    Beep,
}

/// Context provided to fallback handlers.
///
/// This trait abstracts the application state needed by fallback handlers,
/// allowing modules to implement handlers without depending on runner types.
///
/// # Design Philosophy
///
/// The context provides read/write access to the minimal state needed for
/// fallback handling:
/// - Current mode (to decide behavior based on mode)
/// - Active buffer (to insert characters)
/// - Edit recording (for undo tracking)
///
/// # Example
///
/// ```ignore
/// use reovim_driver_input::{FallbackContext, FallbackResult, InputFallbackHandler, KeyEvent};
///
/// struct MyFallback;
///
/// impl<C: FallbackContext> InputFallbackHandler<C> for MyFallback {
///     fn handle_unmatched(&self, key: KeyEvent, ctx: &mut C) -> FallbackResult {
///         if ctx.current_mode().name() == "insert" {
///             // Insert character...
///             FallbackResult::Handled
///         } else {
///             FallbackResult::Beep
///         }
///     }
/// }
/// ```
pub trait FallbackContext: Send {
    /// Get the current mode ID.
    fn current_mode(&self) -> &ModeId;

    /// Get the active buffer ID, if any.
    fn active_buffer(&self) -> Option<BufferId>;

    /// Get a buffer by ID for reading/writing.
    ///
    /// Returns `None` if the buffer doesn't exist.
    fn get_buffer(&self, id: BufferId) -> Option<Arc<RwLock<Buffer>>>;

    /// Record an edit for undo tracking.
    ///
    /// Called by fallback handlers when they modify a buffer.
    /// This allows the runner to track edits for undo/redo.
    fn record_edit(
        &mut self,
        buffer_id: BufferId,
        edits: Vec<Edit>,
        cursor_before: Position,
        cursor_after: Position,
    );
}

/// Trait for handling unmatched key events.
///
/// Implementors decide what to do when a key sequence doesn't match any
/// binding. This allows different modules to provide different policies
/// without changing the event loop.
///
/// # Type Parameter
///
/// * `C` - The context type implementing [`FallbackContext`]
///
/// # Returns
///
/// A [`FallbackResult`] indicating how the key was handled.
///
/// # Example
///
/// ```ignore
/// use reovim_driver_input::{FallbackContext, FallbackResult, InputFallbackHandler, KeyEvent};
///
/// struct BeepOnUnmatched;
///
/// impl<C: FallbackContext> InputFallbackHandler<C> for BeepOnUnmatched {
///     fn handle_unmatched(&self, _key: KeyEvent, _ctx: &mut C) -> FallbackResult {
///         FallbackResult::Beep
///     }
/// }
/// ```
pub trait InputFallbackHandler<C: FallbackContext>: Send + Sync {
    /// Handle a key event that didn't match any binding.
    ///
    /// Called by the event loop when a key sequence results in `NotFound`
    /// from the keymap registry.
    ///
    /// # Arguments
    ///
    /// * `key` - The key event that didn't match
    /// * `ctx` - Mutable reference to the fallback context
    ///
    /// # Returns
    ///
    /// A [`FallbackResult`] indicating how the key was handled.
    fn handle_unmatched(&self, key: KeyEvent, ctx: &mut C) -> FallbackResult;
}

/// No-op fallback handler that ignores all unmatched keys.
///
/// Useful for testing or when no special handling is needed.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoOpFallback;

impl<C: FallbackContext> InputFallbackHandler<C> for NoOpFallback {
    fn handle_unmatched(&self, _key: KeyEvent, _ctx: &mut C) -> FallbackResult {
        FallbackResult::Ignored
    }
}

/// Fallback handler that beeps on all unmatched keys.
///
/// Useful for strict mode where any unbound key is an error.
#[derive(Debug, Clone, Copy, Default)]
pub struct BeepFallback;

impl<C: FallbackContext> InputFallbackHandler<C> for BeepFallback {
    fn handle_unmatched(&self, _key: KeyEvent, _ctx: &mut C) -> FallbackResult {
        FallbackResult::Beep
    }
}

#[cfg(test)]
mod tests {
    use reovim_kernel::api::v1::ModuleId;

    use {super::*, crate::KeyCode};

    /// Mock context for testing.
    struct MockContext {
        mode: ModeId,
        active_buffer: Option<BufferId>,
        recorded_edits: Vec<(BufferId, Vec<Edit>, Position, Position)>,
    }

    impl MockContext {
        fn normal() -> Self {
            Self {
                mode: ModeId::new(ModuleId::new("test"), "normal"),
                active_buffer: None,
                recorded_edits: Vec::new(),
            }
        }
    }

    impl FallbackContext for MockContext {
        fn current_mode(&self) -> &ModeId {
            &self.mode
        }

        fn active_buffer(&self) -> Option<BufferId> {
            self.active_buffer
        }

        fn get_buffer(&self, _id: BufferId) -> Option<Arc<RwLock<Buffer>>> {
            None
        }

        fn record_edit(
            &mut self,
            buffer_id: BufferId,
            edits: Vec<Edit>,
            cursor_before: Position,
            cursor_after: Position,
        ) {
            self.recorded_edits
                .push((buffer_id, edits, cursor_before, cursor_after));
        }
    }

    #[test]
    fn test_noop_fallback() {
        let handler = NoOpFallback;
        let mut ctx = MockContext::normal();
        let key = KeyEvent::new(KeyCode::Char('x'));

        let result = handler.handle_unmatched(key, &mut ctx);
        assert_eq!(result, FallbackResult::Ignored);
    }

    #[test]
    fn test_beep_fallback() {
        let handler = BeepFallback;
        let mut ctx = MockContext::normal();
        let key = KeyEvent::new(KeyCode::Char('x'));

        let result = handler.handle_unmatched(key, &mut ctx);
        assert_eq!(result, FallbackResult::Beep);
    }

    #[test]
    fn test_fallback_result_equality() {
        assert_eq!(FallbackResult::Handled, FallbackResult::Handled);
        assert_ne!(FallbackResult::Handled, FallbackResult::Ignored);
        assert_ne!(FallbackResult::Ignored, FallbackResult::Beep);
    }

    /// Test that generic handlers work with different context types.
    ///
    /// This demonstrates the design's flexibility - the same handler
    /// implementation works with any type implementing `FallbackContext`.
    #[test]
    fn test_handler_works_with_different_contexts() {
        /// Alternative context with different internal structure.
        struct AltContext {
            mode: ModeId,
        }

        impl AltContext {
            fn new() -> Self {
                Self {
                    mode: ModeId::new(ModuleId::new("alt"), "custom"),
                }
            }
        }

        impl FallbackContext for AltContext {
            fn current_mode(&self) -> &ModeId {
                &self.mode
            }
            fn active_buffer(&self) -> Option<BufferId> {
                None
            }
            fn get_buffer(&self, _id: BufferId) -> Option<Arc<RwLock<Buffer>>> {
                None
            }
            fn record_edit(&mut self, _: BufferId, _: Vec<Edit>, _: Position, _: Position) {}
        }

        // Same handler works with both context types
        let handler = NoOpFallback;
        let key = KeyEvent::new(KeyCode::Char('x'));

        let mut mock = MockContext::normal();
        let mut alt = AltContext::new();

        assert_eq!(handler.handle_unmatched(key, &mut mock), FallbackResult::Ignored);
        assert_eq!(handler.handle_unmatched(key, &mut alt), FallbackResult::Ignored);
    }

    /// Test that custom handlers can be implemented for any context.
    ///
    /// This demonstrates the mechanism vs policy separation - handlers
    /// define policy while the trait defines mechanism.
    #[test]
    fn test_custom_handler_implementation() {
        /// Custom handler that handles 'y' specially, beeps on others.
        struct CustomHandler;

        impl<C: FallbackContext> InputFallbackHandler<C> for CustomHandler {
            fn handle_unmatched(&self, key: KeyEvent, _ctx: &mut C) -> FallbackResult {
                if key.code == KeyCode::Char('y') {
                    FallbackResult::Handled
                } else {
                    FallbackResult::Beep
                }
            }
        }

        let handler = CustomHandler;
        let mut ctx = MockContext::normal();

        assert_eq!(
            handler.handle_unmatched(KeyEvent::new(KeyCode::Char('y')), &mut ctx),
            FallbackResult::Handled
        );
        assert_eq!(
            handler.handle_unmatched(KeyEvent::new(KeyCode::Char('n')), &mut ctx),
            FallbackResult::Beep
        );
    }

    /// Test that `record_edit` is properly called by handlers.
    #[test]
    fn test_record_edit_tracking() {
        /// Handler that records an edit when handling a key.
        struct EditRecordingHandler;

        impl<C: FallbackContext> InputFallbackHandler<C> for EditRecordingHandler {
            fn handle_unmatched(&self, _key: KeyEvent, ctx: &mut C) -> FallbackResult {
                let buffer_id = BufferId::from_raw(1);
                let edit = Edit::insert(Position::new(0, 0), "test");
                ctx.record_edit(buffer_id, vec![edit], Position::new(0, 0), Position::new(0, 4));
                FallbackResult::Handled
            }
        }

        let handler = EditRecordingHandler;
        let mut ctx = MockContext::normal();
        let key = KeyEvent::new(KeyCode::Char('x'));

        let result = handler.handle_unmatched(key, &mut ctx);

        assert_eq!(result, FallbackResult::Handled);
        assert_eq!(ctx.recorded_edits.len(), 1);
        assert_eq!(ctx.recorded_edits[0].0, BufferId::from_raw(1));
    }
}
