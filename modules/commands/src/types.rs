//! Ex-command types - POLICY.
//!
//! These types define the ex-command system for vim-style editing.
//! They were moved from kernel to this module following mechanism-vs-policy.
//!
//! - **Mechanism (Kernel)**: Buffer management, position types
//! - **Policy (This Module)**: What commands exist, how they behave
//!
//! Note: These are the "legacy" command types. The new architecture uses
//! `reovim-driver-command` with `Command` and `CommandHandler` traits.
//! This module may be refactored to use the new system in the future.

use reovim_kernel::api::v1::{BufferId, KernelContext, Position, WindowId};

// ============================================================================
// Range Type (for command ranges like :1,5d)
// ============================================================================

/// Text range for command execution (line-based).
///
/// Used for ex-commands that operate on line ranges (e.g., `:1,5d`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Range {
    /// Start position (inclusive).
    pub start: Position,
    /// End position (exclusive).
    pub end: Position,
}

impl Range {
    /// Create a new range.
    #[must_use]
    pub const fn new(start: Position, end: Position) -> Self {
        Self { start, end }
    }
}

// ============================================================================
// CommandHandler Trait
// ============================================================================

/// Handles ex-commands (commands entered via :).
///
/// - **Mechanism (Kernel)**: Command parsing, execution context
/// - **Policy (Module)**: What :w, :q, :set actually do
///
/// # Implementation
///
/// Modules implement this trait to define command behavior:
///
/// ```ignore
/// use reovim_module_commands::{CommandHandler, CommandContext, CommandError};
///
/// struct WriteCommand;
///
/// impl CommandHandler for WriteCommand {
///     fn id(&self) -> &'static str { "write" }
///     fn names(&self) -> &[&'static str] { &["w", "write"] }
///
///     fn execute(&self, ctx: &mut CommandContext<'_>, args: &[&str])
///         -> Result<(), CommandError>
///     {
///         let buffer_id = ctx.buffer_id
///             .ok_or(CommandError::NoBuffer)?;
///         // Write the buffer to disk...
///         Ok(())
///     }
///
///     fn help(&self) -> &'static str {
///         "Write the current buffer to disk"
///     }
/// }
/// ```
pub trait CommandHandler: Send + Sync {
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
    /// Returns `CommandError` if the command fails.
    fn execute(&self, ctx: &mut CommandContext<'_>, args: &[&str]) -> Result<(), CommandError>;

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

/// Context passed to command execution.
pub struct CommandContext<'a> {
    /// Kernel context for accessing services.
    pub kernel: &'a KernelContext,
    /// Current buffer (if any).
    pub buffer_id: Option<BufferId>,
    /// Current window (if any).
    pub window_id: Option<WindowId>,
    /// Whether command was invoked with ! (e.g., :q!).
    pub bang: bool,
    /// Command range (e.g., :1,5d has range `Some((1,5))`).
    pub range: Option<Range>,
}

/// Command execution errors.
#[derive(Debug, Clone)]
pub enum CommandError {
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

impl std::fmt::Display for CommandError {
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

impl std::error::Error for CommandError {}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_range_new() {
        let range = Range::new(Position::new(0, 0), Position::new(5, 0));
        assert_eq!(range.start.line, 0);
        assert_eq!(range.end.line, 5);
    }

    #[test]
    fn test_command_error_display() {
        let err = CommandError::NoBuffer;
        assert_eq!(err.to_string(), "no buffer");

        let err = CommandError::UnknownCommand("foo".into());
        assert!(err.to_string().contains("unknown command"));
    }
}
