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
mod tests {
    use {super::*, reovim_kernel::api::v1::ModuleId};

    fn test_command() -> CommandId {
        CommandId::new(ModuleId::new("test"), "delete")
    }

    fn test_mode() -> ModeId {
        ModeId::new(ModuleId::new("test"), "insert")
    }

    #[test]
    fn transition_context_builders_cover_all_fields() {
        let op = test_command();
        let ctx = TransitionContext::new()
            .operator(op.clone())
            .count(2)
            .register('a');
        assert_eq!(ctx.pending_operator, Some(op));
        assert_eq!(ctx.count, Some(2));
        assert_eq!(ctx.register, Some('a'));

        let only_op = TransitionContext::with_operator(test_command());
        assert!(only_op.pending_operator.is_some());
        assert!(only_op.count.is_none());
        assert!(only_op.register.is_none());
    }

    #[test]
    fn pop_result_variants_preserve_payloads() {
        let result = PopResult::Cancelled;
        assert!(matches!(result, PopResult::Cancelled));

        let execute = PopResult::ExecuteCommand {
            command: test_command(),
            args: HashMap::from([("count".to_owned(), ArgValue::Count(2))]),
        };
        match execute {
            PopResult::ExecuteCommand { command, args } => {
                assert_eq!(command.name(), "delete");
                assert_eq!(args.get("count"), Some(&ArgValue::Count(2)));
            }
            PopResult::Cancelled | PopResult::Data { .. } => panic!("expected execute"),
        }

        let data = PopResult::Data {
            values: HashMap::from([("pattern".to_owned(), ArgValue::String("foo".to_owned()))]),
        };
        match data {
            PopResult::Data { values } => {
                assert_eq!(values.get("pattern"), Some(&ArgValue::String("foo".to_owned())));
            }
            PopResult::ExecuteCommand { .. } | PopResult::Cancelled => panic!("expected data"),
        }
    }

    #[test]
    fn mode_transition_variants_store_mode_and_context() {
        let context = TransitionContext::new().count(3);
        let mode = test_mode();

        match (ModeTransition::Push {
            mode: mode.clone(),
            context: context.clone(),
        }) {
            ModeTransition::Push {
                mode: pushed,
                context,
            } => {
                assert_eq!(pushed, mode);
                assert_eq!(context.count, Some(3));
            }
            ModeTransition::Pop { .. } | ModeTransition::Set { .. } => panic!("expected push"),
        }

        match (ModeTransition::Set {
            mode: test_mode(),
            context: TransitionContext::default(),
        }) {
            ModeTransition::Set { mode, context } => {
                assert_eq!(mode.name(), "insert");
                assert!(context.pending_operator.is_none());
            }
            ModeTransition::Push { .. } | ModeTransition::Pop { .. } => panic!("expected set"),
        }
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn debug_output_mentions_types() {
        assert!(format!("{:?}", TransitionContext::default()).contains("TransitionContext"));
        assert!(format!("{:?}", PopResult::Cancelled).contains("Cancelled"));
    }
}
