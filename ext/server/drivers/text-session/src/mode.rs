//! Mode trait with lifecycle hooks.
//!
//! This module provides the [`SessionMode`] trait that modules implement
//! for their modes. Unlike the kernel's `Mode` trait (which is for compile-time
//! mode enums), this trait is object-safe and supports runtime lifecycle hooks.
//!
//! # Design
//!
//! - **Kernel `Mode` trait**: Compile-time mode enums with type safety
//! - **Session `SessionMode` trait**: Runtime mode instances with lifecycle
//!
//! The kernel's `ModeStack` manages `ModeId` values. The session driver's
//! `SessionMode` trait adds runtime behavior (`on_enter`, `on_exit`, resolver).
//!
//! # Lifecycle
//!
//! ```text
//! push_mode(mode_id)
//!   └─> SessionMode::on_enter() ─┐
//!       ├─> Ok(()) ─────────────>│ Mode pushed to stack
//!       └─> Err(e) ─────────────>│ Transition aborted, error logged
//!
//! pop_mode()
//!   └─> SessionMode::on_exit() ──> Mode popped from stack
//!       (always succeeds - best-effort cleanup)
//! ```

use std::fmt;

use reovim_kernel::api::v1::ModeId;

/// Error from mode lifecycle hooks.
///
/// Returned by [`SessionMode::on_enter()`] when mode transition should be aborted.
#[derive(Debug, Clone)]
pub struct ModeError {
    /// Human-readable error message.
    pub message: String,
}

impl ModeError {
    /// Create a new mode error.
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for ModeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "mode error: {}", self.message)
    }
}

impl std::error::Error for ModeError {}

/// Mode with lifecycle hooks.
///
/// Modules implement this trait for their modes. The runner calls lifecycle
/// hooks when modes are pushed/popped, allowing modes to initialize and
/// cleanup state.
///
/// # Object Safety
///
/// This trait is intentionally object-safe (`dyn SessionMode`) to support
/// runtime mode registration. Use the kernel's `Mode` trait for compile-time
/// mode enums.
///
/// # Example
///
/// ```ignore
/// use reovim_driver_text_session::{SessionMode, ModeError};
/// use reovim_kernel::api::v1::ModeId;
///
/// pub struct VimInsertMode;
///
/// impl SessionMode for VimInsertMode {
///     fn id(&self) -> ModeId {
///         ModeId::new(ModuleId::new("vim"), "insert")
///     }
///
///     fn on_enter(&self) -> Result<(), ModeError> {
///         // Initialize insert mode state
///         Ok(())
///     }
///
///     fn on_exit(&self) {
///         // Cleanup insert mode state
///     }
/// }
/// ```
pub trait SessionMode: Send + Sync + 'static {
    /// Mode identifier.
    ///
    /// Must match the `ModeId` used in the kernel's `ModeStack`.
    fn id(&self) -> ModeId;

    /// Whether this is an entry point mode (can be home mode).
    ///
    /// Entry modes are candidates for the session's home mode.
    /// Only one mode per module should return `true`.
    fn is_entry(&self) -> bool {
        false
    }

    /// Called when this mode is pushed onto the stack.
    ///
    /// Return `Err` to abort the mode transition. The error is logged
    /// and the mode is not pushed.
    ///
    /// # Errors
    ///
    /// Return `Err(ModeError)` if the mode cannot be entered.
    fn on_enter(&self) -> Result<(), ModeError> {
        Ok(())
    }

    /// Called when this mode is popped from the stack.
    ///
    /// This method is infallible (like a destructor). Errors are logged
    /// but cannot prevent the pop. Use for best-effort cleanup.
    fn on_exit(&self) {}
}
#[cfg(test)]
#[path = "mode_tests.rs"]
mod tests;
