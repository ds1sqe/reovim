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

    // ========================================================================
    // KeyHandler trait tests
    // ========================================================================

    /// Test handler that uses default `is_active` implementation.
    struct TestHandler {
        id: &'static str,
    }

    impl KeyHandler for TestHandler {
        fn id(&self) -> &str {
            self.id
        }

        fn priority(&self) -> HandlerPriority {
            priority::NORMAL
        }

        fn handle(&mut self, _event: &KeyEvent) -> KeyHandlerResult {
            KeyHandlerResult::Ignored
        }
    }

    #[test]
    fn test_key_handler_default_is_active() {
        let handler = TestHandler { id: "test" };
        // Default implementation returns true
        assert!(handler.is_active());
    }

    #[test]
    fn test_key_handler_id() {
        let handler = TestHandler { id: "my-handler" };
        assert_eq!(handler.id(), "my-handler");
    }

    #[test]
    fn test_key_handler_priority() {
        let handler = TestHandler { id: "test" };
        assert_eq!(handler.priority(), priority::NORMAL);
    }

    #[test]
    fn test_key_handler_handle() {
        let mut handler = TestHandler { id: "test" };
        let key = KeyEvent::new(crate::KeyCode::Char('x'));
        let result = handler.handle(&key);
        assert_eq!(result, KeyHandlerResult::Ignored);
    }

    /// Test handler with custom `is_active` returning false.
    struct InactiveHandler;

    impl KeyHandler for InactiveHandler {
        fn id(&self) -> &'static str {
            "inactive"
        }

        fn priority(&self) -> HandlerPriority {
            priority::NORMAL
        }

        fn handle(&mut self, _event: &KeyEvent) -> KeyHandlerResult {
            KeyHandlerResult::Consumed
        }

        fn is_active(&self) -> bool {
            false
        }
    }

    #[test]
    fn test_key_handler_custom_is_active() {
        let handler = InactiveHandler;
        assert!(!handler.is_active());
    }

    #[test]
    fn test_inactive_handler_all_methods() {
        let mut handler = InactiveHandler;
        assert_eq!(handler.id(), "inactive");
        assert_eq!(handler.priority(), priority::NORMAL);
        let key = KeyEvent::new(crate::KeyCode::Char('a'));
        assert_eq!(handler.handle(&key), KeyHandlerResult::Consumed);
        assert!(!handler.is_active());
    }

    // ========================================================================
    // Trait object safety tests
    // ========================================================================

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_input_driver_is_object_safe() {
        fn _accepts_ref(_: &dyn InputDriver) {}
        fn _accepts_box(_: Box<dyn InputDriver>) {}
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_key_handler_is_object_safe() {
        fn _accepts_ref(_: &dyn KeyHandler) {}
        fn _accepts_box(_: Box<dyn KeyHandler>) {}
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_clipboard_provider_is_object_safe() {
        fn _accepts_ref(_: &dyn ClipboardProvider) {}
        fn _accepts_box(_: Box<dyn ClipboardProvider>) {}
    }

    // ========================================================================
    // KeyHandlerResult additional tests
    // ========================================================================

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_key_handler_result_debug() {
        let consumed = format!("{:?}", KeyHandlerResult::Consumed);
        assert_eq!(consumed, "Consumed");
        let pending = format!("{:?}", KeyHandlerResult::Pending);
        assert_eq!(pending, "Pending");
        let ignored = format!("{:?}", KeyHandlerResult::Ignored);
        assert_eq!(ignored, "Ignored");
    }

    #[test]
    fn test_key_handler_result_clone() {
        let result = KeyHandlerResult::Consumed;
        let cloned = result;
        assert_eq!(result, cloned);
    }

    // ========================================================================
    // KeymapRegistry trait tests
    // ========================================================================

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_keymap_registry_is_object_safe() {
        fn _accepts_ref(_: &dyn KeymapRegistry<String>) {}
        fn _accepts_box(_: Box<dyn KeymapRegistry<String>>) {}
    }

    // ========================================================================
    // ClipboardProvider implementation test
    // ========================================================================

    struct MockClipboard {
        content: std::sync::Mutex<String>,
    }

    impl MockClipboard {
        fn new() -> Self {
            Self {
                content: std::sync::Mutex::new(String::new()),
            }
        }
    }

    impl ClipboardProvider for MockClipboard {
        fn read(&self) -> Result<String, crate::error::ClipboardError> {
            Ok(self.content.lock().unwrap().clone())
        }

        fn write(&mut self, text: &str) -> Result<(), crate::error::ClipboardError> {
            *self.content.lock().unwrap() = text.to_string();
            Ok(())
        }

        fn is_available(&self) -> bool {
            true
        }

        fn name(&self) -> &'static str {
            "mock"
        }
    }

    #[test]
    fn test_clipboard_provider_impl() {
        let mut clipboard = MockClipboard::new();
        assert!(clipboard.is_available());
        assert_eq!(clipboard.name(), "mock");

        clipboard.write("hello").unwrap();
        assert_eq!(clipboard.read().unwrap(), "hello");
    }

    #[test]
    fn test_clipboard_provider_as_trait_object() {
        let clipboard: Box<dyn ClipboardProvider> = Box::new(MockClipboard::new());
        assert!(clipboard.is_available());
        assert_eq!(clipboard.name(), "mock");
    }

    #[test]
    fn test_clipboard_provider_write_and_read_multiple() {
        let mut clipboard = MockClipboard::new();
        clipboard.write("first").unwrap();
        assert_eq!(clipboard.read().unwrap(), "first");
        clipboard.write("second").unwrap();
        assert_eq!(clipboard.read().unwrap(), "second");
    }

    #[test]
    fn test_clipboard_provider_empty_read() {
        let clipboard = MockClipboard::new();
        assert_eq!(clipboard.read().unwrap(), "");
    }

    // ========================================================================
    // InputDriver trait test (mock implementation)
    // ========================================================================

    struct MockInputDriver {
        running: bool,
    }

    impl MockInputDriver {
        fn new() -> Self {
            Self { running: false }
        }
    }

    impl InputDriver for MockInputDriver {
        fn init(&mut self) -> Result<(), InputError> {
            Ok(())
        }
        fn start(&mut self) -> Result<(), InputError> {
            self.running = true;
            Ok(())
        }
        fn stop(&mut self) -> Result<(), InputError> {
            self.running = false;
            Ok(())
        }
        fn is_running(&self) -> bool {
            self.running
        }
        fn inject_key(&mut self, _event: KeyEvent) -> Result<(), InputError> {
            if !self.running {
                return Err(InputError::NotRunning);
            }
            Ok(())
        }
        fn inject_mouse(&mut self, _event: MouseEvent) -> Result<(), InputError> {
            if !self.running {
                return Err(InputError::NotRunning);
            }
            Ok(())
        }
        fn register_handler(&mut self, _handler: Box<dyn KeyHandler>) -> Result<(), InputError> {
            Ok(())
        }
        fn unregister_handler(&mut self, _handler_id: &str) -> Result<(), InputError> {
            Ok(())
        }
    }

    #[test]
    fn test_input_driver_lifecycle() {
        let mut driver = MockInputDriver::new();
        assert!(!driver.is_running());

        driver.init().unwrap();
        assert!(!driver.is_running());

        driver.start().unwrap();
        assert!(driver.is_running());

        driver.stop().unwrap();
        assert!(!driver.is_running());
    }

    #[test]
    fn test_input_driver_inject_key_when_running() {
        let mut driver = MockInputDriver::new();
        driver.start().unwrap();
        let key = KeyEvent::new(crate::KeyCode::Char('a'));
        assert!(driver.inject_key(key).is_ok());
    }

    #[test]
    fn test_input_driver_inject_key_when_not_running() {
        let mut driver = MockInputDriver::new();
        let key = KeyEvent::new(crate::KeyCode::Char('a'));
        assert!(driver.inject_key(key).is_err());
    }

    #[test]
    fn test_input_driver_inject_mouse_when_running() {
        let mut driver = MockInputDriver::new();
        driver.start().unwrap();
        let mouse = MouseEvent::new(crate::mouse::MouseEventKind::Moved, 0, 0);
        assert!(driver.inject_mouse(mouse).is_ok());
    }

    #[test]
    fn test_input_driver_inject_mouse_when_not_running() {
        let mut driver = MockInputDriver::new();
        let mouse = MouseEvent::new(crate::mouse::MouseEventKind::Moved, 0, 0);
        assert!(driver.inject_mouse(mouse).is_err());
    }

    #[test]
    fn test_input_driver_register_handler() {
        let mut driver = MockInputDriver::new();
        let handler = TestHandler { id: "test-handler" };
        assert!(driver.register_handler(Box::new(handler)).is_ok());
    }

    #[test]
    fn test_input_driver_unregister_handler() {
        let mut driver = MockInputDriver::new();
        assert!(driver.unregister_handler("test-handler").is_ok());
    }

    #[test]
    fn test_input_driver_as_trait_object() {
        let driver = MockInputDriver::new();
        let _: &dyn InputDriver = &driver;
    }

    // ========================================================================
    // KeymapRegistry trait test (mock implementation)
    // ========================================================================

    struct MockKeymapRegistry {
        scopes: Vec<String>,
    }

    impl MockKeymapRegistry {
        fn new() -> Self {
            Self { scopes: Vec::new() }
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl KeymapRegistry<String> for MockKeymapRegistry {
        fn bind(
            &mut self,
            scope: &str,
            _keys: &[KeyEvent],
            _action: String,
        ) -> Result<(), InputError> {
            if !self.scopes.contains(&scope.to_string()) {
                self.scopes.push(scope.to_string());
            }
            Ok(())
        }

        fn unbind(&mut self, _scope: &str, _keys: &[KeyEvent]) -> Result<(), InputError> {
            Ok(())
        }

        fn lookup(&self, _scope: &str, _keys: &[KeyEvent]) -> KeymapResult<&String> {
            KeymapResult::None
        }

        fn is_prefix(&self, _scope: &str, _keys: &[KeyEvent]) -> bool {
            false
        }

        fn scopes(&self) -> Vec<&str> {
            self.scopes.iter().map(String::as_str).collect()
        }

        fn clear_scope(&mut self, scope: &str) {
            self.scopes.retain(|s| s != scope);
        }
    }

    #[test]
    fn test_keymap_registry_bind_and_scopes() {
        let mut registry = MockKeymapRegistry::new();
        let key = KeyEvent::new(crate::KeyCode::Char('j'));
        registry.bind("normal", &[key], "down".to_string()).unwrap();
        let scopes = registry.scopes();
        assert_eq!(scopes.len(), 1);
        assert_eq!(scopes[0], "normal");
    }

    #[test]
    fn test_keymap_registry_unbind() {
        let mut registry = MockKeymapRegistry::new();
        let key = KeyEvent::new(crate::KeyCode::Char('j'));
        assert!(registry.unbind("normal", &[key]).is_ok());
    }

    #[test]
    fn test_keymap_registry_lookup_not_found() {
        let registry = MockKeymapRegistry::new();
        let key = KeyEvent::new(crate::KeyCode::Char('j'));
        assert!(matches!(registry.lookup("normal", &[key]), KeymapResult::None));
    }

    #[test]
    fn test_keymap_registry_is_prefix() {
        let registry = MockKeymapRegistry::new();
        let key = KeyEvent::new(crate::KeyCode::Char('g'));
        assert!(!registry.is_prefix("normal", &[key]));
    }

    #[test]
    fn test_keymap_registry_clear_scope() {
        let mut registry = MockKeymapRegistry::new();
        let key = KeyEvent::new(crate::KeyCode::Char('j'));
        registry.bind("normal", &[key], "down".to_string()).unwrap();
        assert_eq!(registry.scopes().len(), 1);

        registry.clear_scope("normal");
        assert!(registry.scopes().is_empty());
    }

    #[test]
    fn test_keymap_registry_as_trait_object() {
        let registry = MockKeymapRegistry::new();
        let _: &dyn KeymapRegistry<String> = &registry;
    }

    // ========================================================================
    // Additional KeyHandler tests
    // ========================================================================

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_key_handler_consumed_result() {
        struct ConsumingHandler;

        impl KeyHandler for ConsumingHandler {
            fn id(&self) -> &'static str {
                "consumer"
            }
            fn priority(&self) -> HandlerPriority {
                priority::INTERCEPT
            }
            fn handle(&mut self, _event: &KeyEvent) -> KeyHandlerResult {
                KeyHandlerResult::Consumed
            }
        }

        let mut handler = ConsumingHandler;
        let key = KeyEvent::new(crate::KeyCode::Char('a'));
        assert_eq!(handler.handle(&key), KeyHandlerResult::Consumed);
        assert_eq!(handler.priority(), priority::INTERCEPT);
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_key_handler_pending_result() {
        struct PendingHandler;

        impl KeyHandler for PendingHandler {
            fn id(&self) -> &'static str {
                "pending"
            }
            fn priority(&self) -> HandlerPriority {
                priority::MODAL
            }
            fn handle(&mut self, _event: &KeyEvent) -> KeyHandlerResult {
                KeyHandlerResult::Pending
            }
        }

        let mut handler = PendingHandler;
        let key = KeyEvent::new(crate::KeyCode::Char('d'));
        assert_eq!(handler.handle(&key), KeyHandlerResult::Pending);
    }
}
