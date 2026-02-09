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

use {reovim_driver_command_types::ArgValue, reovim_kernel::api::v1::CommandId};

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
mod tests {
    use {super::*, reovim_kernel::api::v1::ModuleId};

    fn test_command() -> CommandId {
        CommandId::new(ModuleId::new("test"), "delete")
    }

    #[test]
    fn test_transition_context_new() {
        let ctx = TransitionContext::new();
        assert!(ctx.pending_operator.is_none());
        assert!(ctx.count.is_none());
        assert!(ctx.register.is_none());
    }

    #[test]
    fn test_transition_context_with_operator() {
        let op = test_command();
        let ctx = TransitionContext::with_operator(op.clone());
        assert_eq!(ctx.pending_operator, Some(op));
    }

    #[test]
    fn test_transition_context_builder() {
        let op = test_command();
        let ctx = TransitionContext::new()
            .operator(op.clone())
            .count(2)
            .register('a');

        assert_eq!(ctx.pending_operator, Some(op));
        assert_eq!(ctx.count, Some(2));
        assert_eq!(ctx.register, Some('a'));
    }

    #[test]
    fn test_pop_result_cancelled() {
        let result = PopResult::Cancelled;
        assert!(matches!(result, PopResult::Cancelled));
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_pop_result_execute_command() {
        let mut args = HashMap::new();
        args.insert("count".to_string(), ArgValue::Count(2));
        args.insert("register".to_string(), ArgValue::Register('a'));

        let result = PopResult::ExecuteCommand {
            command: test_command(),
            args,
        };

        if let PopResult::ExecuteCommand { command, args } = result {
            assert_eq!(command.name(), "delete");
            assert_eq!(args.get("count"), Some(&ArgValue::Count(2)));
            assert_eq!(args.get("register"), Some(&ArgValue::Register('a')));
        } else {
            panic!("expected ExecuteCommand");
        }
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_pop_result_data() {
        let mut values = HashMap::new();
        values.insert("pattern".to_string(), ArgValue::String("foo".to_string()));

        let result = PopResult::Data { values };

        if let PopResult::Data { values } = result {
            assert_eq!(values.get("pattern"), Some(&ArgValue::String("foo".to_string())));
        } else {
            panic!("expected Data");
        }
    }

    #[test]
    fn test_transition_context_default() {
        let ctx = TransitionContext::default();
        assert!(ctx.pending_operator.is_none());
        assert!(ctx.count.is_none());
        assert!(ctx.register.is_none());
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_transition_context_debug() {
        let ctx = TransitionContext::new();
        let debug = format!("{ctx:?}");
        assert!(debug.contains("TransitionContext"));
    }

    #[test]
    fn test_transition_context_clone() {
        let op = test_command();
        let ctx = TransitionContext::new().operator(op).count(5).register('b');
        #[allow(clippy::redundant_clone)]
        let cloned = ctx.clone();
        assert_eq!(cloned.count, Some(5));
        assert_eq!(cloned.register, Some('b'));
        assert!(cloned.pending_operator.is_some());
    }

    #[test]
    fn test_transition_context_operator_only() {
        let op = test_command();
        let ctx = TransitionContext::new().operator(op.clone());
        assert_eq!(ctx.pending_operator, Some(op));
        assert!(ctx.count.is_none());
        assert!(ctx.register.is_none());
    }

    #[test]
    fn test_transition_context_count_only() {
        let ctx = TransitionContext::new().count(10);
        assert!(ctx.pending_operator.is_none());
        assert_eq!(ctx.count, Some(10));
        assert!(ctx.register.is_none());
    }

    #[test]
    fn test_transition_context_register_only() {
        let ctx = TransitionContext::new().register('z');
        assert!(ctx.pending_operator.is_none());
        assert!(ctx.count.is_none());
        assert_eq!(ctx.register, Some('z'));
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_pop_result_debug() {
        let result = PopResult::Cancelled;
        let debug = format!("{result:?}");
        assert!(debug.contains("Cancelled"));

        let result = PopResult::Data {
            values: HashMap::new(),
        };
        let debug = format!("{result:?}");
        assert!(debug.contains("Data"));
    }

    #[test]
    fn test_pop_result_clone() {
        let result = PopResult::Cancelled;
        #[allow(clippy::redundant_clone)]
        let cloned = result.clone();
        assert!(matches!(cloned, PopResult::Cancelled));
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_pop_result_execute_command_with_empty_args() {
        let result = PopResult::ExecuteCommand {
            command: test_command(),
            args: HashMap::new(),
        };

        if let PopResult::ExecuteCommand { command, args } = result {
            assert_eq!(command.name(), "delete");
            assert!(args.is_empty());
        } else {
            panic!("expected ExecuteCommand");
        }
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_pop_result_data_empty() {
        let result = PopResult::Data {
            values: HashMap::new(),
        };

        if let PopResult::Data { values } = result {
            assert!(values.is_empty());
        } else {
            panic!("expected Data");
        }
    }
}
