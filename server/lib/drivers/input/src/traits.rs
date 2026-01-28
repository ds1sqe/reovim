//! Input driver traits.
//!
//! Linux equivalent: `include/linux/input.h` (input subsystem)
//!
//! # Architecture
//!
//! ```text
//! +-----------------+
//! | InputDriver     |  <-- Lifecycle: init, start, stop
//! +-----------------+
//!         |
//!         v
//! +-----------------+
//! | KeyHandler      |  <-- Receives and processes key events
//! +-----------------+
//!         |
//!         v
//! +-----------------+
//! | KeymapRegistry  |  <-- Maps key sequences to actions
//! +-----------------+
//!
//! +-----------------+
//! | ClipboardProvider |  <-- System clipboard access
//! +-----------------+
//! ```

#![allow(clippy::missing_errors_doc)]

use crate::{
    error::{ClipboardError, InputError},
    key::{KeyEvent, KeymapResult},
    mouse::MouseEvent,
};

/// Input driver lifecycle management.
///
/// This trait manages the input driver lifecycle and provides
/// key/mouse injection for testing and RPC.
pub trait InputDriver: Send + Sync {
    /// Initialize the input driver.
    ///
    /// Called once before `start()`. Setup resources here.
    fn init(&mut self) -> Result<(), InputError>;

    /// Start the input driver.
    ///
    /// Begin reading input events and dispatching to handlers.
    fn start(&mut self) -> Result<(), InputError>;

    /// Stop the input driver.
    ///
    /// Stop reading events. Resources may be kept for restart.
    fn stop(&mut self) -> Result<(), InputError>;

    /// Check if the driver is currently running.
    fn is_running(&self) -> bool;

    /// Inject a key event (for testing/RPC).
    ///
    /// The event is processed as if it came from the terminal.
    fn inject_key(&mut self, event: KeyEvent) -> Result<(), InputError>;

    /// Inject a mouse event (for testing/RPC).
    ///
    /// The event is processed as if it came from the terminal.
    fn inject_mouse(&mut self, event: MouseEvent) -> Result<(), InputError>;

    /// Register a key handler.
    ///
    /// Handlers are called in priority order (higher priority first).
    fn register_handler(&mut self, handler: Box<dyn KeyHandler>) -> Result<(), InputError>;

    /// Unregister a key handler by ID.
    fn unregister_handler(&mut self, handler_id: &str) -> Result<(), InputError>;
}

/// Handler priority type.
///
/// Higher values are processed first.
pub type HandlerPriority = i32;

/// Priority levels for common handler types.
pub mod priority {
    use super::HandlerPriority;

    /// Highest priority - intercepts all keys (e.g., escape sequences).
    pub const INTERCEPT: HandlerPriority = 1000;

    /// High priority - modal handlers (e.g., command mode).
    pub const MODAL: HandlerPriority = 500;

    /// Normal priority - standard key bindings.
    pub const NORMAL: HandlerPriority = 0;

    /// Low priority - fallback handlers.
    pub const FALLBACK: HandlerPriority = -500;
}

/// Key event handler.
///
/// Handlers receive key events in priority order. If a handler
/// consumes an event, lower-priority handlers are not called.
pub trait KeyHandler: Send {
    /// Unique identifier for this handler.
    fn id(&self) -> &str;

    /// Priority for event dispatch (higher = earlier).
    fn priority(&self) -> HandlerPriority;

    /// Handle a key event.
    ///
    /// Returns how the event was processed.
    fn handle(&mut self, event: &KeyEvent) -> KeyHandlerResult;

    /// Check if this handler is currently active.
    ///
    /// Inactive handlers are skipped during dispatch.
    fn is_active(&self) -> bool {
        true
    }
}

/// Result of handling a key event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyHandlerResult {
    /// Event was consumed - do not dispatch to lower-priority handlers.
    Consumed,

    /// Event is part of a multi-key sequence - wait for more keys.
    Pending,

    /// Event was not handled - dispatch to next handler.
    Ignored,
}

/// Keymap registry for binding key sequences to actions.
///
/// This trait provides the interface for managing keymaps.
/// Implementations handle the trie/lookup structure.
///
/// The generic type `A` represents the action type (command ID, callback, etc.).
pub trait KeymapRegistry<A>: Send + Sync {
    /// Bind a key sequence to an action in a scope.
    ///
    /// # Arguments
    ///
    /// * `scope` - The keymap scope (e.g., "normal", "insert", "command")
    /// * `keys` - The key sequence to bind
    /// * `action` - The action to execute when keys are pressed
    fn bind(&mut self, scope: &str, keys: &[KeyEvent], action: A) -> Result<(), InputError>;

    /// Unbind a key sequence from a scope.
    fn unbind(&mut self, scope: &str, keys: &[KeyEvent]) -> Result<(), InputError>;

    /// Look up a key sequence in a scope.
    fn lookup(&self, scope: &str, keys: &[KeyEvent]) -> KeymapResult<&A>;

    /// Check if a key sequence is a valid prefix in a scope.
    fn is_prefix(&self, scope: &str, keys: &[KeyEvent]) -> bool;

    /// List all scopes.
    fn scopes(&self) -> Vec<&str>;

    /// Clear all bindings in a scope.
    fn clear_scope(&mut self, scope: &str);
}

/// System clipboard provider.
///
/// Provides read/write access to the system clipboard.
/// Implementations handle platform-specific clipboard APIs.
pub trait ClipboardProvider: Send + Sync {
    /// Read text from the system clipboard.
    fn read(&self) -> Result<String, ClipboardError>;

    /// Write text to the system clipboard.
    fn write(&mut self, text: &str) -> Result<(), ClipboardError>;

    /// Check if clipboard is available.
    fn is_available(&self) -> bool;

    /// Get the name of this provider (for debugging).
    fn name(&self) -> &str;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_handler_result_variants() {
        assert_eq!(KeyHandlerResult::Consumed, KeyHandlerResult::Consumed);
        assert_eq!(KeyHandlerResult::Pending, KeyHandlerResult::Pending);
        assert_eq!(KeyHandlerResult::Ignored, KeyHandlerResult::Ignored);
        assert_ne!(KeyHandlerResult::Consumed, KeyHandlerResult::Ignored);
        assert_ne!(KeyHandlerResult::Consumed, KeyHandlerResult::Pending);
        assert_ne!(KeyHandlerResult::Pending, KeyHandlerResult::Ignored);
    }

    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn test_priority_ordering() {
        assert!(priority::INTERCEPT > priority::MODAL);
        assert!(priority::MODAL > priority::NORMAL);
        assert!(priority::NORMAL > priority::FALLBACK);
    }

    #[test]
    fn test_priority_values() {
        assert_eq!(priority::INTERCEPT, 1000);
        assert_eq!(priority::MODAL, 500);
        assert_eq!(priority::NORMAL, 0);
        assert_eq!(priority::FALLBACK, -500);
    }
}
