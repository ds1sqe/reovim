//! Mode transition types for the text session driver.
//!
//! These types were originally defined in `reovim-subsys-input-contracts` (now
//! deleted as part of Plan-14 I.6). The canonical home is now
//! `reovim-driver-text-input`, but since `text-input` depends on this crate
//! (circular dep), the definitions live here and `text-input` re-exports them.

use std::collections::HashMap;

use {
    reovim_kernel::api::v1::{CommandId, ModeId},
    reovim_subsys_command_types::ArgValue,
};

/// Context passed when entering a new mode.
#[derive(Debug, Clone, Default)]
pub struct TransitionContext {
    /// Pending operator waiting for a motion or text object.
    pub pending_operator: Option<CommandId>,
    /// Count prefix to pass into the new mode.
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

    /// Set the pending operator.
    #[must_use]
    pub fn operator(mut self, op: CommandId) -> Self {
        self.pending_operator = Some(op);
        self
    }

    /// Set the count.
    #[must_use]
    pub const fn count(mut self, count: usize) -> Self {
        self.count = Some(count);
        self
    }

    /// Set the register.
    #[must_use]
    pub const fn register(mut self, register: char) -> Self {
        self.register = Some(register);
        self
    }
}

/// Result returned when popping from a mode.
#[derive(Debug, Clone)]
pub enum PopResult {
    /// Execute a command with arguments.
    ExecuteCommand {
        /// The command to execute.
        command: CommandId,
        /// Command arguments.
        args: HashMap<String, ArgValue>,
    },
    /// User cancelled the operation.
    Cancelled,
    /// Return data to the parent mode.
    Data {
        /// Key-value result payload.
        values: HashMap<String, ArgValue>,
    },
}

/// Mode transition request.
#[derive(Debug, Clone)]
pub enum ModeTransition {
    /// Push a mode onto the stack.
    Push {
        /// The mode to push.
        mode: ModeId,
        /// Context for the new mode.
        context: TransitionContext,
    },
    /// Pop the current mode.
    Pop {
        /// Optional result to pass to the parent mode.
        result: Option<PopResult>,
    },
    /// Replace the current mode atomically.
    Set {
        /// The mode to switch to.
        mode: ModeId,
        /// Context for the new mode.
        context: TransitionContext,
    },
}

#[cfg(test)]
#[path = "transition_tests.rs"]
mod tests;
