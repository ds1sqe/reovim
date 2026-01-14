//! Input fallback handler trait for unmatched key events.
//!
//! This module defines the mechanism for handling keys that don't match
//! any keymap binding. The event loop (mechanism) delegates to a trait;
//! modules (policy) implement the trait to define behavior.
//!
//! # Design Philosophy
//!
//! Following the Linux kernel's "mechanism vs policy" principle:
//! - **Mechanism** (this trait): Defines WHAT can happen when keys don't match
//! - **Policy** (implementations): Decides HOW to handle unmatched keys
//!
//! For example, the editor module's `EditorFallbackHandler` inserts
//! characters when in Insert mode, but the event loop doesn't know about
//! Insert mode - it just delegates to the handler.

use reovim_driver_input::KeyEvent;

use crate::AppState;

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

/// Trait for handling unmatched key events.
///
/// Implementors decide what to do when a key sequence doesn't match any
/// binding. This allows different modules to provide different policies
/// without changing the event loop.
///
/// # Example
///
/// ```ignore
/// use runner_new::{AppState, InputFallbackHandler, FallbackResult};
/// use reovim_driver_input::KeyEvent;
///
/// struct MyFallback;
///
/// impl InputFallbackHandler for MyFallback {
///     fn handle_unmatched(&self, key: KeyEvent, app: &mut AppState) -> FallbackResult {
///         // Insert character if in insert mode
///         if app.current_mode().name() == "insert" {
///             // ... insert logic ...
///             return FallbackResult::Handled;
///         }
///         FallbackResult::Beep
///     }
/// }
/// ```
pub trait InputFallbackHandler: Send + Sync {
    /// Handle a key event that didn't match any binding.
    ///
    /// Called by the event loop when a key sequence results in `NotFound`
    /// from the keymap registry.
    ///
    /// # Arguments
    ///
    /// * `key` - The key event that didn't match
    /// * `app` - Mutable reference to application state
    ///
    /// # Returns
    ///
    /// A [`FallbackResult`] indicating how the key was handled.
    fn handle_unmatched(&self, key: KeyEvent, app: &mut AppState) -> FallbackResult;
}

/// No-op fallback handler that ignores all unmatched keys.
///
/// Useful for testing or when no special handling is needed.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoOpFallback;

impl InputFallbackHandler for NoOpFallback {
    fn handle_unmatched(&self, _key: KeyEvent, _app: &mut AppState) -> FallbackResult {
        FallbackResult::Ignored
    }
}

/// Fallback handler that beeps on all unmatched keys.
///
/// Useful for strict mode where any unbound key is an error.
#[derive(Debug, Clone, Copy, Default)]
pub struct BeepFallback;

impl InputFallbackHandler for BeepFallback {
    fn handle_unmatched(&self, _key: KeyEvent, _app: &mut AppState) -> FallbackResult {
        FallbackResult::Beep
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        reovim_driver_input::KeyCode,
        reovim_kernel::api::v1::{KernelContext, ModeId, ModuleId},
    };

    fn test_app() -> AppState {
        let kernel = KernelContext::default();
        let mode = ModeId::new(ModuleId::new("test"), "normal");
        AppState::new(kernel, mode)
    }

    #[test]
    fn test_noop_fallback() {
        let handler = NoOpFallback;
        let mut app = test_app();
        let key = KeyEvent::new(KeyCode::Char('x'));

        assert_eq!(handler.handle_unmatched(key, &mut app), FallbackResult::Ignored);
    }

    #[test]
    fn test_beep_fallback() {
        let handler = BeepFallback;
        let mut app = test_app();
        let key = KeyEvent::new(KeyCode::Char('x'));

        assert_eq!(handler.handle_unmatched(key, &mut app), FallbackResult::Beep);
    }

    #[test]
    fn test_fallback_result_equality() {
        assert_eq!(FallbackResult::Handled, FallbackResult::Handled);
        assert_ne!(FallbackResult::Handled, FallbackResult::Ignored);
        assert_ne!(FallbackResult::Ignored, FallbackResult::Beep);
    }
}
