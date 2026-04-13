//! Mode transition types.
//!
//! This module provides types for mode transitions:
//! - [`TransitionContext`] - Context passed when entering a new mode
//! - [`PopResult`] - Result returned when exiting a mode
//!
//! # Design
//!
//! These types enable mode stacking patterns:
//! - Push operator-pending mode with pending operator
//! - Pop with operator range result
//! - Pass counts and registers across mode boundaries
//!
//! # Example
//!
//! ```ignore
//! use reovim_driver_session::{TransitionContext, PopResult};
//!
//! // Enter operator-pending mode after pressing 'd'
//! let ctx = TransitionContext::with_operator(delete_cmd);
//! session.push_mode(op_pending_mode, ctx);
//!
//! // Motion provides range, pop back with result
//! let result = PopResult::OperatorRange {
//!     operator: delete_cmd,
//!     start,
//!     end,
//!     linewise: false,
//!     count: None,
//!     register: None,
//! };
//! session.pop_mode(Some(result));
//! ```

use std::collections::HashMap;

use {reovim_kernel::api::v1::CommandId, reovim_subsys_command_types::ArgValue};

/// Context passed when entering a new mode.
///
/// Contains state from the previous mode that the new mode needs:
/// - Pending operator (for operator-pending mode)
/// - Count prefix (inherited across mode transitions)
/// - Register selection
#[derive(Debug, Clone, Default)]
pub struct TransitionContext {
    /// Pending operator waiting for a motion/text-object.
    ///
    /// Set when entering operator-pending mode (e.g., after pressing `d`).
    pub pending_operator: Option<CommandId>,

    /// Count prefix to pass to the new mode.
    ///
    /// Counts can be combined: `2d3w` = delete 6 words.
    pub count: Option<usize>,

    /// Register for the operation.
    pub register: Option<char>,
}

impl TransitionContext {
    /// Create an empty context.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a context with a pending operator.
    #[must_use]
    pub fn with_operator(operator: CommandId) -> Self {
        Self {
            pending_operator: Some(operator),
            ..Default::default()
        }
    }

    /// Set the pending operator (builder pattern).
    #[must_use]
    pub fn operator(mut self, op: CommandId) -> Self {
        self.pending_operator = Some(op);
        self
    }

    /// Set the count (builder pattern).
    #[must_use]
    pub const fn count(mut self, count: usize) -> Self {
        self.count = Some(count);
        self
    }

    /// Set the register (builder pattern).
    #[must_use]
    pub const fn register(mut self, register: char) -> Self {
        self.register = Some(register);
        self
    }
}

/// Result returned when popping from a mode.
///
/// This is the mechanism for modes to request command execution when popping.
/// The runner executes the command - it doesn't know what the command does.
///
/// # Design
///
/// Runner is pure mechanism. Modules (vim, etc.) build the complete command
/// with all arguments. Runner just executes what it's given.
#[derive(Debug, Clone)]
pub enum PopResult {
    /// Execute a command with arguments.
    ///
    /// The mode has built a complete command with all necessary arguments.
    /// Runner executes the command without knowing its semantics.
    ///
    /// # Example
    ///
    /// Vim operator-pending mode builds: `delete` command with range args.
    /// Runner sees: "execute this command with these args" - no vim knowledge.
    ExecuteCommand {
        /// The command to execute.
        command: CommandId,
        /// Arguments for the command (module builds these).
        args: HashMap<String, ArgValue>,
    },

    /// User cancelled the operation (Escape pressed).
    Cancelled,

    /// Data result (no command to execute, just return data).
    ///
    /// Used when a mode needs to return information to its parent
    /// without executing a command.
    Data {
        /// Key-value pairs returned from the mode.
        values: HashMap<String, ArgValue>,
    },
}
#[cfg(test)]
#[path = "transition_tests.rs"]
mod tests;
