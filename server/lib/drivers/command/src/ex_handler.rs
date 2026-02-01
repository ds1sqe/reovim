//! Ex-command handler trait and types.
//!
//! This module defines the mechanism for ex-commands (`:w`, `:q`, `:e`, etc.).
//! Modules implement `ExCommandHandler` to define command behavior.
//!
//! # Architecture
//!
//! - **Mechanism (this module)**: Defines WHAT an ex-command handler is
//! - **Policy (modules)**: Implements HOW specific commands behave
//!
//! # Migrated from `server/modules/commands/src/types.rs`
//!
//! These types were moved here to enable cross-module access. The commands
//! module implements this trait, and the vim module can access handlers via
//! `ExCommandDispatcher` in `ServiceRegistry`.

use std::sync::Arc;

use {
    reovim_driver_vfs::VfsDriver,
    reovim_kernel::api::v1::{BufferId, KernelContext, Position, WindowId},
};

// ============================================================================
// Range Type (for command ranges like :1,5d)
// ============================================================================

/// Text range for command execution (line-based).
///
/// Used for ex-commands that operate on line ranges (e.g., `:1,5d`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExCommandRange {
    /// Start position (inclusive).
    pub start: Position,
    /// End position (exclusive).
    pub end: Position,
}

impl ExCommandRange {
    /// Create a new range.
    #[must_use]
    pub const fn new(start: Position, end: Position) -> Self {
        Self { start, end }
    }
}

// ============================================================================
// ExCommandHandler Trait
// ============================================================================

/// Handles ex-commands (commands entered via :).
///
/// - **Mechanism (Driver)**: This trait definition
/// - **Policy (Module)**: What `:w`, `:q`, `:e` actually do
///
/// # Implementation
///
/// Modules implement this trait to define command behavior:
///
/// ```ignore
/// use reovim_driver_command::{ExCommandHandler, ExCommandContext, ExCommandError};
///
/// struct WriteCommand;
///
/// impl ExCommandHandler for WriteCommand {
///     fn id(&self) -> &'static str { "write" }
///     fn names(&self) -> &[&'static str] { &["w", "write"] }
///
///     fn execute(&self, ctx: &mut ExCommandContext<'_>, args: &[&str])
///         -> Result<(), ExCommandError>
///     {
///         let buffer_id = ctx.buffer_id
///             .ok_or(ExCommandError::NoBuffer)?;
///         // Write the buffer to disk...
///         Ok(())
///     }
///
///     fn help(&self) -> &'static str {
///         "Write the current buffer to disk"
///     }
/// }
/// ```
pub trait ExCommandHandler: Send + Sync {
    /// Command identifier.
    fn id(&self) -> &'static str;

    /// Command names (e.g., `["w", "write"]`).
    ///
    /// The first name is the canonical name.
    fn names(&self) -> &[&'static str];

    /// Execute the command.
    ///
    /// # Arguments
    ///
    /// * `ctx` - Execution context
    /// * `args` - Command arguments (e.g., `:w foo.txt` has args `["foo.txt"]`)
    ///
    /// # Errors
    ///
    /// Returns `ExCommandError` if the command fails.
    fn execute(&self, ctx: &mut ExCommandContext<'_>, args: &[&str]) -> Result<(), ExCommandError>;

    /// Command completion suggestions.
    ///
    /// Returns possible completions for the given partial input.
    fn complete(&self, _partial: &str) -> Vec<String> {
        vec![]
    }

    /// Help text for the command.
    fn help(&self) -> &'static str {
        ""
    }
}

/// Context passed to ex-command execution.
pub struct ExCommandContext<'a> {
    /// Kernel context for accessing services.
    pub kernel: &'a KernelContext,
    /// Current buffer (if any).
    pub buffer_id: Option<BufferId>,
    /// Current window (if any).
    pub window_id: Option<WindowId>,
    /// Whether command was invoked with ! (e.g., :q!).
    pub bang: bool,
    /// Command range (e.g., :1,5d has range `Some((1,5))`).
    pub range: Option<ExCommandRange>,
    /// VFS driver for file operations.
    pub vfs: Option<Arc<dyn VfsDriver>>,
}

impl<'a> ExCommandContext<'a> {
    /// Create a new context.
    #[must_use]
    pub fn new(kernel: &'a KernelContext) -> Self {
        Self {
            kernel,
            buffer_id: None,
            window_id: None,
            bang: false,
            range: None,
            vfs: None,
        }
    }

    /// Set the buffer ID.
    #[must_use]
    pub const fn with_buffer(mut self, buffer_id: BufferId) -> Self {
        self.buffer_id = Some(buffer_id);
        self
    }

    /// Set the window ID.
    #[must_use]
    pub const fn with_window(mut self, window_id: WindowId) -> Self {
        self.window_id = Some(window_id);
        self
    }

    /// Set the bang flag.
    #[must_use]
    pub const fn with_bang(mut self, bang: bool) -> Self {
        self.bang = bang;
        self
    }

    /// Set the range.
    #[must_use]
    pub const fn with_range(mut self, range: ExCommandRange) -> Self {
        self.range = Some(range);
        self
    }

    /// Set the VFS driver.
    #[must_use]
    pub fn with_vfs(mut self, vfs: Arc<dyn VfsDriver>) -> Self {
        self.vfs = Some(vfs);
        self
    }

    /// Get the VFS driver, if available.
    #[must_use]
    pub fn vfs(&self) -> Option<&dyn VfsDriver> {
        self.vfs.as_deref()
    }
}

/// Ex-command execution errors.
#[derive(Debug, Clone)]
pub enum ExCommandError {
    /// No buffer available.
    NoBuffer,
    /// No window available.
    NoWindow,
    /// Invalid arguments.
    InvalidArguments(String),
    /// Execution failed.
    ExecutionFailed(String),
    /// Unknown command.
    UnknownCommand(String),
}

impl std::fmt::Display for ExCommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoBuffer => write!(f, "no buffer"),
            Self::NoWindow => write!(f, "no window"),
            Self::InvalidArguments(msg) => write!(f, "invalid arguments: {msg}"),
            Self::ExecutionFailed(msg) => write!(f, "execution failed: {msg}"),
            Self::UnknownCommand(name) => write!(f, "unknown command: {name}"),
        }
    }
}

impl std::error::Error for ExCommandError {}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_range_new() {
        let range = ExCommandRange::new(Position::new(0, 0), Position::new(5, 0));
        assert_eq!(range.start.line, 0);
        assert_eq!(range.end.line, 5);
    }

    #[test]
    fn test_command_error_display() {
        let err = ExCommandError::NoBuffer;
        assert_eq!(err.to_string(), "no buffer");

        let err = ExCommandError::UnknownCommand("foo".into());
        assert!(err.to_string().contains("unknown command"));
    }

    #[test]
    fn test_ex_command_handler_object_safe() {
        fn _accepts_ref(_: &dyn ExCommandHandler) {}
        fn _accepts_box(_: Box<dyn ExCommandHandler>) {}
    }
}
