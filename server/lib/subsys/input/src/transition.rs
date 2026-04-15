//! Mode transition types — domain-neutral protocol.
//!
//! These types define the mode transition protocol used by the server
//! and domain drivers. They are domain-neutral: they use `CommandId`,
//! `ModeId`, and `ArgValue` — all kernel/subsys types.

use std::collections::HashMap;

use {
    reovim_kernel::api::v1::{CommandId, ModeId},
    reovim_subsys_command_types::ArgValue,
};

/// Context passed when entering a new mode.
///
/// Contains state from the previous mode that the new mode needs:
/// - Pending operator (for operator-pending mode)
/// - Count prefix (inherited across mode transitions)
/// - Register selection
#[derive(Debug, Clone, Default)]
pub struct TransitionContext {
    /// Pending operator waiting for a motion/text-object.
    pub pending_operator: Option<CommandId>,
    /// Count prefix to pass to the new mode.
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
/// The runner executes the command — it doesn't know what the command does.
#[derive(Debug, Clone)]
pub enum PopResult {
    /// Execute a command with arguments.
    ExecuteCommand {
        /// The command to execute.
        command: CommandId,
        /// Arguments for the command (module builds these).
        args: HashMap<String, ArgValue>,
    },

    /// User cancelled the operation (Escape pressed).
    Cancelled,

    /// Data result (no command to execute, just return data).
    Data {
        /// Key-value pairs returned from the mode.
        values: HashMap<String, ArgValue>,
    },
}

/// Mode transition request.
///
/// Returned by resolvers to request a mode change. The server applies
/// the transition to the client's mode stack.
#[derive(Debug, Clone)]
pub enum ModeTransition {
    /// Push a mode onto the stack with context.
    Push {
        /// The mode to push.
        mode: ModeId,
        /// Context to pass to the new mode.
        context: TransitionContext,
    },

    /// Pop the current mode, passing result to parent.
    Pop {
        /// Result to pass to the parent mode.
        result: Option<PopResult>,
    },

    /// Replace the current mode (equivalent to pop + push, but atomic).
    Set {
        /// The mode to switch to.
        mode: ModeId,
        /// Context for the new mode.
        context: TransitionContext,
    },
}
