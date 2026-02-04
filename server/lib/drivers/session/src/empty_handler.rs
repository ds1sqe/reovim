//! Empty session handler mechanism.
//!
//! Provides the trait for handling empty sessions (no buffers).
//! Modules implement this trait to define policy for what happens
//! when a session starts with no buffers.
//!
//! # Linux Kernel Parallel
//!
//! Like a driver's `probe()` function that initializes a device,
//! `EmptySessionHandler::handle()` initializes an empty session.

use std::path::Path;

/// Action to take when session is empty.
///
/// Returned by [`EmptySessionHandler::handle()`] to indicate what
/// action the runner should take.
#[derive(Debug, Clone)]
pub enum EmptySessionAction {
    /// Create a buffer with the given name and content.
    ///
    /// - `name`: Buffer name (`None` = unnamed scratch buffer)
    /// - `content`: Initial content (empty string for blank buffer)
    CreateBuffer {
        /// Buffer name. `None` creates an unnamed scratch buffer.
        name: Option<String>,
        /// Initial buffer content.
        content: String,
    },

    /// Do nothing - defer to next handler.
    ///
    /// Return this to let a lower-priority handler decide.
    None,
}

/// Context provided to empty session handlers.
///
/// Contains read-only information about the session state.
/// Handlers cannot mutate state directly; they return an action
/// that the runner executes.
#[derive(Debug)]
pub struct EmptySessionContext<'a> {
    /// Session ID.
    pub session_id: u64,

    /// Command-line file arguments (files to open).
    /// If non-empty, handler should likely return `None`.
    pub file_args: &'a [String],

    /// Current working directory.
    pub cwd: &'a Path,
}

/// Handler for empty session state.
///
/// Modules implement this trait to define what happens when a session
/// has no buffers. The runner calls handlers in priority order until
/// one returns an action other than [`EmptySessionAction::None`].
///
/// # Priority Convention
///
/// - 0-50: Core handlers (system-level)
/// - 100: Default module priority
/// - 200+: Late/fallback handlers
///
/// # Example
///
/// ```ignore
/// use reovim_driver_session::{
///     EmptySessionHandler, EmptySessionContext, EmptySessionAction
/// };
///
/// pub struct ScratchBufferHandler;
///
/// impl EmptySessionHandler for ScratchBufferHandler {
///     fn handle(&self, _ctx: &EmptySessionContext) -> EmptySessionAction {
///         EmptySessionAction::CreateBuffer {
///             name: None,
///             content: String::new(),
///         }
///     }
///
///     fn priority(&self) -> u32 { 100 }
///
///     fn id(&self) -> &'static str { "defaults:scratch-buffer" }
///
///     fn description(&self) -> &'static str {
///         "Create empty scratch buffer on startup"
///     }
/// }
/// ```
pub trait EmptySessionHandler: Send + Sync + 'static {
    /// Handle empty session state.
    ///
    /// Called when session has no buffers. Return an action describing
    /// what should happen. Return [`EmptySessionAction::None`] to defer
    /// to next handler.
    fn handle(&self, ctx: &EmptySessionContext) -> EmptySessionAction;

    /// Handler priority (lower = called first).
    ///
    /// Default is 100 (standard module priority).
    fn priority(&self) -> u32 {
        100
    }

    /// Unique handler identifier.
    ///
    /// Format: `"module-name:handler-name"` (e.g., `"scratch-buffer:handler"`).
    fn id(&self) -> &'static str;

    /// Human-readable description for debugging/introspection.
    fn description(&self) -> &'static str;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify the trait is object-safe (can use `dyn EmptySessionHandler`).
    #[test]
    fn trait_is_object_safe() {
        struct TestHandler;

        impl EmptySessionHandler for TestHandler {
            fn handle(&self, _ctx: &EmptySessionContext) -> EmptySessionAction {
                EmptySessionAction::None
            }
            fn id(&self) -> &'static str {
                "test:handler"
            }
            fn description(&self) -> &'static str {
                "Test handler"
            }
        }

        // This compiles if trait is object-safe
        let _: Box<dyn EmptySessionHandler> = Box::new(TestHandler);
    }

    #[test]
    fn action_variants_constructible() {
        let create = EmptySessionAction::CreateBuffer {
            name: Some("test".to_string()),
            content: "hello".to_string(),
        };
        assert!(matches!(create, EmptySessionAction::CreateBuffer { .. }));

        let none = EmptySessionAction::None;
        assert!(matches!(none, EmptySessionAction::None));
    }

    #[test]
    fn default_priority_is_100() {
        struct TestHandler;

        impl EmptySessionHandler for TestHandler {
            fn handle(&self, _ctx: &EmptySessionContext) -> EmptySessionAction {
                EmptySessionAction::None
            }
            fn id(&self) -> &'static str {
                "test:handler"
            }
            fn description(&self) -> &'static str {
                "Test handler"
            }
        }

        let handler = TestHandler;
        assert_eq!(handler.priority(), 100);
    }
}
