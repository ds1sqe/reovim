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
    reovim_domain_text::{Edit, Position},
    reovim_kernel::api::v1::{BufferId, ModeId},
    reovim_provider_text::BufferOps,
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
    fn get_buffer(&self, id: BufferId) -> Option<Arc<RwLock<dyn BufferOps>>>;

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
