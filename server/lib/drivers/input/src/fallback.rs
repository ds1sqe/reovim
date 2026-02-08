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
//! depending on server types. This breaks the circular dependency where
//! modules would otherwise need to import from the server.
//!
//! ```text
//! server/lib/drivers/input/  <-- FallbackContext trait (this crate)
//!        ^
//!        |  (implements trait)
//!        |
//! server/lib/server/         <-- EditingState implements FallbackContext
//!        ^
//!        |  (uses trait)
//!        |
//! server/modules/editor/     <-- EditorFallbackHandler<C: FallbackContext>
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

    /// Get the current cursor position for the active window.
    ///
    /// Returns `None` if no active window or buffer.
    fn cursor_position(&self) -> Option<Position>;

    /// Set the cursor position for the active window.
    ///
    /// No-op if no active window.
    fn set_cursor_position(&mut self, pos: Position);

    /// Get a buffer by ID for reading/writing.
    ///
    /// Returns `None` if the buffer doesn't exist.
    fn get_buffer(&self, id: BufferId) -> Option<Arc<RwLock<Buffer>>>;

    /// Record an edit for undo tracking.
    ///
    /// Called by fallback handlers when they modify a buffer.
    /// This allows the runner to track edits for undo/redo.
    ///
    /// Use `accumulate_edit` instead for batched operations like character
    /// insertion in insert mode.
    fn record_edit(
        &mut self,
        buffer_id: BufferId,
        edits: Vec<Edit>,
        cursor_before: Position,
        cursor_after: Position,
    );

    /// Accumulate an edit for batched undo.
    ///
    /// Unlike `record_edit` which immediately creates an undo node,
    /// this method accumulates edits for later flushing as a single
    /// transaction. Use for consecutive operations like character
    /// insertion in insert mode.
    ///
    /// Call `flush_pending_edits` to commit the batch.
    ///
    /// # Arguments
    ///
    /// * `buffer_id` - The buffer being edited
    /// * `edit` - The edit to accumulate
    /// * `cursor_before` - Cursor position before this edit
    /// * `cursor_after` - Cursor position after this edit
    ///
    /// # Example
    ///
    /// ```ignore
    /// // In insert mode fallback handler
    /// ctx.accumulate_edit(buffer_id, edit, cursor_before, cursor_after);
    /// // Later, on mode change or command execution:
    /// ctx.flush_pending_edits();
    /// ```
    fn accumulate_edit(
        &mut self,
        buffer_id: BufferId,
        edit: Edit,
        cursor_before: Position,
        cursor_after: Position,
    );

    /// Flush accumulated edits as a single undo transaction.
    ///
    /// Commits all edits accumulated via `accumulate_edit` as a single
    /// undo node. Safe to call when empty (no-op).
    ///
    /// Called automatically on:
    /// - Mode change (e.g., exiting insert mode)
    /// - Before command execution
    /// - Buffer change
    fn flush_pending_edits(&mut self);
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

        fn cursor_position(&self) -> Option<Position> {
            Some(Position::origin())
        }

        fn set_cursor_position(&mut self, _pos: Position) {
            // No-op in mock
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

        fn accumulate_edit(
            &mut self,
            buffer_id: BufferId,
            edit: Edit,
            cursor_before: Position,
            cursor_after: Position,
        ) {
            // For testing, just record immediately (no actual batching in mock)
            self.recorded_edits
                .push((buffer_id, vec![edit], cursor_before, cursor_after));
        }

        fn flush_pending_edits(&mut self) {
            // No-op in mock - edits recorded immediately in accumulate_edit
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
            fn cursor_position(&self) -> Option<Position> {
                None
            }
            fn set_cursor_position(&mut self, _pos: Position) {}
            fn get_buffer(&self, _id: BufferId) -> Option<Arc<RwLock<Buffer>>> {
                None
            }
            fn record_edit(&mut self, _: BufferId, _: Vec<Edit>, _: Position, _: Position) {}
            fn accumulate_edit(&mut self, _: BufferId, _: Edit, _: Position, _: Position) {}
            fn flush_pending_edits(&mut self) {}
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

    // ========================================================================
    // FallbackResult additional tests
    // ========================================================================

    #[test]
    fn test_fallback_result_debug() {
        assert_eq!(format!("{:?}", FallbackResult::Handled), "Handled");
        assert_eq!(format!("{:?}", FallbackResult::Ignored), "Ignored");
        assert_eq!(format!("{:?}", FallbackResult::Beep), "Beep");
    }

    #[test]
    fn test_fallback_result_clone_copy() {
        let result = FallbackResult::Handled;
        let copied = result;
        assert_eq!(result, copied);
    }

    // ========================================================================
    // NoOpFallback/BeepFallback derive tests
    // ========================================================================

    #[test]
    fn test_noop_fallback_debug() {
        let handler = NoOpFallback;
        let debug = format!("{handler:?}");
        assert!(debug.contains("NoOpFallback"));
    }

    #[test]
    fn test_noop_fallback_clone_copy_default() {
        let handler = NoOpFallback;
        let cloned = handler;
        // Both are valid since Copy
        assert_eq!(format!("{handler:?}"), format!("{cloned:?}"));
        // Default constructor
        let default_handler = NoOpFallback;
        assert_eq!(format!("{default_handler:?}"), "NoOpFallback");
    }

    #[test]
    fn test_beep_fallback_debug() {
        let handler = BeepFallback;
        let debug = format!("{handler:?}");
        assert!(debug.contains("BeepFallback"));
    }

    #[test]
    fn test_beep_fallback_clone_copy_default() {
        let handler = BeepFallback;
        let cloned = handler;
        assert_eq!(format!("{handler:?}"), format!("{cloned:?}"));
        // Default constructor
        let default_handler = BeepFallback;
        assert_eq!(format!("{default_handler:?}"), "BeepFallback");
    }

    // ========================================================================
    // FallbackContext trait method tests
    // ========================================================================

    #[test]
    fn test_mock_context_current_mode() {
        let ctx = MockContext::normal();
        assert_eq!(ctx.current_mode().name(), "normal");
    }

    #[test]
    fn test_mock_context_active_buffer_none() {
        let ctx = MockContext::normal();
        assert!(ctx.active_buffer().is_none());
    }

    #[test]
    fn test_mock_context_cursor_position() {
        let ctx = MockContext::normal();
        assert_eq!(ctx.cursor_position(), Some(Position::origin()));
    }

    #[test]
    fn test_mock_context_set_cursor_position() {
        let mut ctx = MockContext::normal();
        ctx.set_cursor_position(Position::new(5, 10));
        // No-op in mock, should not panic
    }

    #[test]
    fn test_mock_context_get_buffer_none() {
        let ctx = MockContext::normal();
        let fake_id = BufferId::from_raw(99);
        assert!(ctx.get_buffer(fake_id).is_none());
    }

    #[test]
    fn test_mock_context_accumulate_edit() {
        let mut ctx = MockContext::normal();
        let buffer_id = BufferId::from_raw(1);
        let edit = Edit::insert(Position::new(0, 0), "x");
        ctx.accumulate_edit(buffer_id, edit, Position::new(0, 0), Position::new(0, 1));
        assert_eq!(ctx.recorded_edits.len(), 1);
    }

    #[test]
    fn test_mock_context_flush_pending_edits() {
        let mut ctx = MockContext::normal();
        ctx.flush_pending_edits();
        // No-op in mock, should not panic
    }

    #[test]
    fn test_mock_context_with_active_buffer() {
        let mut ctx = MockContext::normal();
        let bid = BufferId::from_raw(42);
        ctx.active_buffer = Some(bid);
        assert_eq!(ctx.active_buffer(), Some(bid));
    }

    // ========================================================================
    // NoOpFallback/BeepFallback Default trait tests
    // ========================================================================

    #[test]
    fn test_noop_fallback_default_trait() {
        let handler = NoOpFallback::default();
        let mut ctx = MockContext::normal();
        let key = KeyEvent::new(KeyCode::Char('z'));
        assert_eq!(handler.handle_unmatched(key, &mut ctx), FallbackResult::Ignored);
    }

    #[test]
    fn test_beep_fallback_default_trait() {
        let handler = BeepFallback::default();
        let mut ctx = MockContext::normal();
        let key = KeyEvent::new(KeyCode::Char('z'));
        assert_eq!(handler.handle_unmatched(key, &mut ctx), FallbackResult::Beep);
    }

    // ========================================================================
    // FallbackResult exhaustive variant tests
    // ========================================================================

    #[test]
    fn test_fallback_result_handled_ne_beep() {
        assert_ne!(FallbackResult::Handled, FallbackResult::Beep);
    }

    #[test]
    fn test_fallback_result_copy_is_identical() {
        let r1 = FallbackResult::Beep;
        let r2 = r1;
        assert_eq!(r1, r2);
    }

    // ========================================================================
    // FallbackContext mock - record_edit with real data
    // ========================================================================

    #[test]
    fn test_mock_context_record_edit_multiple() {
        let mut ctx = MockContext::normal();
        let bid = BufferId::from_raw(1);

        let edit1 = Edit::insert(Position::new(0, 0), "a");
        ctx.record_edit(bid, vec![edit1], Position::new(0, 0), Position::new(0, 1));

        let edit2 = Edit::insert(Position::new(0, 1), "b");
        ctx.record_edit(bid, vec![edit2], Position::new(0, 1), Position::new(0, 2));

        assert_eq!(ctx.recorded_edits.len(), 2);
        assert_eq!(ctx.recorded_edits[0].0, bid);
        assert_eq!(ctx.recorded_edits[1].0, bid);
    }

    #[test]
    fn test_mock_context_accumulate_edit_multiple() {
        let mut ctx = MockContext::normal();
        let bid = BufferId::from_raw(1);

        for i in 0..5 {
            let edit = Edit::insert(Position::new(0, i), "x");
            ctx.accumulate_edit(bid, edit, Position::new(0, i), Position::new(0, i + 1));
        }

        assert_eq!(ctx.recorded_edits.len(), 5);
    }

    // ========================================================================
    // Handler with special keys
    // ========================================================================

    #[test]
    fn test_noop_fallback_with_special_keys() {
        let handler = NoOpFallback;
        let mut ctx = MockContext::normal();

        // Test with Escape
        assert_eq!(
            handler.handle_unmatched(KeyEvent::new(KeyCode::Escape), &mut ctx),
            FallbackResult::Ignored
        );

        // Test with Enter
        assert_eq!(
            handler.handle_unmatched(KeyEvent::new(KeyCode::Enter), &mut ctx),
            FallbackResult::Ignored
        );

        // Test with Backspace
        assert_eq!(
            handler.handle_unmatched(KeyEvent::new(KeyCode::Backspace), &mut ctx),
            FallbackResult::Ignored
        );
    }

    #[test]
    fn test_beep_fallback_with_special_keys() {
        let handler = BeepFallback;
        let mut ctx = MockContext::normal();

        assert_eq!(
            handler.handle_unmatched(KeyEvent::new(KeyCode::Escape), &mut ctx),
            FallbackResult::Beep
        );
        assert_eq!(
            handler.handle_unmatched(KeyEvent::new(KeyCode::Tab), &mut ctx),
            FallbackResult::Beep
        );
    }

    // ========================================================================
    // Handler as trait object
    // ========================================================================

    #[test]
    fn test_fallback_handler_as_trait_object() {
        let noop: Box<dyn InputFallbackHandler<MockContext>> = Box::new(NoOpFallback);
        let mut ctx = MockContext::normal();
        let key = KeyEvent::new(KeyCode::Char('x'));
        assert_eq!(noop.handle_unmatched(key, &mut ctx), FallbackResult::Ignored);
    }

    #[test]
    fn test_beep_handler_as_trait_object() {
        let beep: Box<dyn InputFallbackHandler<MockContext>> = Box::new(BeepFallback);
        let mut ctx = MockContext::normal();
        let key = KeyEvent::new(KeyCode::Char('x'));
        assert_eq!(beep.handle_unmatched(key, &mut ctx), FallbackResult::Beep);
    }
}
