use {
    super::*,
    reovim_kernel::api::v1::CommandId,
    reovim_subsys_command::{ArgSpec, Command, CommandPriority},
};

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_command_trait_object_safety() {
    // Verify Command trait is object-safe (inner fns never called)
    fn _accepts_ref(_: &dyn Command) {}
    fn _accepts_box(_: Box<dyn Command>) {}
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_command_handler_trait_object_safety() {
    // Verify CommandHandler trait is object-safe (inner fns never called)
    fn _accepts_ref(_: &dyn CommandHandler) {}
    fn _accepts_box(_: Box<dyn CommandHandler>) {}
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_command_handler_signature() {
    // Verify the new signature compiles with SessionRuntime
    use reovim_kernel::api::v1::ModuleId;

    struct TestCommand;

    impl Command for TestCommand {
        fn id(&self) -> CommandId {
            CommandId::new(ModuleId::new("test"), "test-cmd")
        }
        fn description(&self) -> &'static str {
            "Test command"
        }
    }

    impl CommandHandler for TestCommand {
        fn execute(
            &self,
            _runtime: &mut SessionRuntime<'_>,
            _args: &CommandContext,
        ) -> CommandResult {
            CommandResult::Success
        }
    }

    // Verify it can be used as trait object
    let cmd: &dyn CommandHandler = &TestCommand;
    assert_eq!(cmd.description(), "Test command");
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_command_default_args_is_empty() {
    use reovim_kernel::api::v1::ModuleId;

    struct MinimalCommand;

    impl Command for MinimalCommand {
        fn id(&self) -> CommandId {
            CommandId::new(ModuleId::new("test"), "minimal")
        }
        fn description(&self) -> &'static str {
            "A minimal command"
        }
    }

    let cmd = MinimalCommand;
    assert!(cmd.args().is_empty());
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_command_default_names_is_empty() {
    use reovim_kernel::api::v1::ModuleId;

    struct MinimalCommand;

    impl Command for MinimalCommand {
        fn id(&self) -> CommandId {
            CommandId::new(ModuleId::new("test"), "minimal")
        }
        fn description(&self) -> &'static str {
            "A minimal command"
        }
    }

    let cmd = MinimalCommand;
    assert!(cmd.names().is_empty());
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_command_with_custom_args() {
    use {crate::ArgKind, reovim_kernel::api::v1::ModuleId};

    struct ArgsCommand;

    impl Command for ArgsCommand {
        fn id(&self) -> CommandId {
            CommandId::new(ModuleId::new("test"), "args-cmd")
        }
        fn description(&self) -> &'static str {
            "Command with args"
        }
        fn args(&self) -> Vec<ArgSpec> {
            vec![
                ArgSpec::required("count", ArgKind::Count, "Number of times"),
                ArgSpec::optional("register", ArgKind::Register, "Target register"),
            ]
        }
    }

    let cmd = ArgsCommand;
    let args = cmd.args();
    assert_eq!(args.len(), 2);
    assert!(args[0].required);
    assert!(!args[1].required);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_command_with_custom_names() {
    use reovim_kernel::api::v1::ModuleId;

    struct NamesCommand;

    impl Command for NamesCommand {
        fn id(&self) -> CommandId {
            CommandId::new(ModuleId::new("test"), "names-cmd")
        }
        fn description(&self) -> &'static str {
            "Command with names"
        }
        fn names(&self) -> &[&'static str] {
            &["w", "write", "wr"]
        }
    }

    let cmd = NamesCommand;
    assert_eq!(cmd.names(), &["w", "write", "wr"]);
    assert_eq!(cmd.names().len(), 3);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_command_handler_upcasts_to_command() {
    use reovim_kernel::api::v1::ModuleId;

    struct TestCmd;

    impl Command for TestCmd {
        fn id(&self) -> CommandId {
            CommandId::new(ModuleId::new("test"), "test-upcast")
        }
        fn description(&self) -> &'static str {
            "Upcast test"
        }
    }

    impl CommandHandler for TestCmd {
        fn execute(
            &self,
            _runtime: &mut SessionRuntime<'_>,
            _args: &CommandContext,
        ) -> CommandResult {
            CommandResult::Success
        }
    }

    // CommandHandler implies Command, so we can call Command methods
    let handler: &dyn CommandHandler = &TestCmd;
    assert_eq!(handler.id().name(), "test-upcast");
    assert_eq!(handler.description(), "Upcast test");
    assert!(handler.args().is_empty());
    assert!(handler.names().is_empty());
}

// ========================================================================
// Command trait default method coverage
// ========================================================================

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_command_default_args_returns_empty_vec() {
    use reovim_kernel::api::v1::ModuleId;

    struct NoArgsCommand;
    impl Command for NoArgsCommand {
        fn id(&self) -> CommandId {
            CommandId::new(ModuleId::new("test"), "no-args")
        }
        fn description(&self) -> &'static str {
            "No args"
        }
    }

    let cmd = NoArgsCommand;
    let args = cmd.args();
    assert!(args.is_empty());
    assert_eq!(args.len(), 0);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_command_default_names_returns_empty_slice() {
    use reovim_kernel::api::v1::ModuleId;

    struct NoNamesCommand;
    impl Command for NoNamesCommand {
        fn id(&self) -> CommandId {
            CommandId::new(ModuleId::new("test"), "no-names")
        }
        fn description(&self) -> &'static str {
            "No names"
        }
    }

    let cmd = NoNamesCommand;
    let names = cmd.names();
    assert!(names.is_empty());
    assert_eq!(names.len(), 0);
}

// ========================================================================
// Command with custom args and names
// ========================================================================

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_command_with_multiple_args() {
    use {crate::ArgKind, reovim_kernel::api::v1::ModuleId};

    struct MultiArgCommand;
    impl Command for MultiArgCommand {
        fn id(&self) -> CommandId {
            CommandId::new(ModuleId::new("test"), "multi-arg")
        }
        fn description(&self) -> &'static str {
            "Multi arg command"
        }
        fn args(&self) -> Vec<ArgSpec> {
            vec![
                ArgSpec::required("count", ArgKind::Count, "Number of times"),
                ArgSpec::optional("register", ArgKind::Register, "Target register"),
                ArgSpec::optional("file", ArgKind::FilePath, "File path"),
            ]
        }
    }

    let cmd = MultiArgCommand;
    let args = cmd.args();
    assert_eq!(args.len(), 3);
    assert!(args[0].required);
    assert!(!args[1].required);
    assert!(!args[2].required);
    assert_eq!(args[0].name, "count");
    assert_eq!(args[1].name, "register");
    assert_eq!(args[2].name, "file");
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_command_with_single_name() {
    use reovim_kernel::api::v1::ModuleId;

    struct SingleNameCommand;
    impl Command for SingleNameCommand {
        fn id(&self) -> CommandId {
            CommandId::new(ModuleId::new("test"), "single-name")
        }
        fn description(&self) -> &'static str {
            "Single name"
        }
        fn names(&self) -> &[&'static str] {
            &["q"]
        }
    }

    let cmd = SingleNameCommand;
    assert_eq!(cmd.names().len(), 1);
    assert_eq!(cmd.names()[0], "q");
}

// ========================================================================
// CommandHandler with different result types
// ========================================================================

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_command_handler_returns_error() {
    use reovim_kernel::api::v1::ModuleId;

    struct ErrorCommand;
    impl Command for ErrorCommand {
        fn id(&self) -> CommandId {
            CommandId::new(ModuleId::new("test"), "error-cmd")
        }
        fn description(&self) -> &'static str {
            "Error command"
        }
    }
    impl CommandHandler for ErrorCommand {
        fn execute(
            &self,
            _runtime: &mut SessionRuntime<'_>,
            _args: &CommandContext,
        ) -> CommandResult {
            CommandResult::Error("something went wrong".to_string())
        }
    }

    let cmd: &dyn CommandHandler = &ErrorCommand;
    assert_eq!(cmd.id().name(), "error-cmd");
    assert_eq!(cmd.description(), "Error command");
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_command_handler_returns_force_quit() {
    use reovim_kernel::api::v1::ModuleId;

    struct ForceQuitCommand;
    impl Command for ForceQuitCommand {
        fn id(&self) -> CommandId {
            CommandId::new(ModuleId::new("test"), "force-quit")
        }
        fn description(&self) -> &'static str {
            "Force quit"
        }
    }
    impl CommandHandler for ForceQuitCommand {
        fn execute(
            &self,
            _runtime: &mut SessionRuntime<'_>,
            _args: &CommandContext,
        ) -> CommandResult {
            CommandResult::Success
        }
    }

    let cmd: &dyn CommandHandler = &ForceQuitCommand;
    assert_eq!(cmd.id().name(), "force-quit");
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_command_handler_returns_detach() {
    use reovim_kernel::api::v1::ModuleId;

    struct DetachCommand;
    impl Command for DetachCommand {
        fn id(&self) -> CommandId {
            CommandId::new(ModuleId::new("test"), "detach")
        }
        fn description(&self) -> &'static str {
            "Detach"
        }
    }
    impl CommandHandler for DetachCommand {
        fn execute(
            &self,
            _runtime: &mut SessionRuntime<'_>,
            _args: &CommandContext,
        ) -> CommandResult {
            CommandResult::Success
        }
    }

    let cmd: &dyn CommandHandler = &DetachCommand;
    assert_eq!(cmd.id().name(), "detach");
}

// ========================================================================
// Command trait object with id() comparison
// ========================================================================

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_command_id_module_and_name() {
    use reovim_kernel::api::v1::ModuleId;

    struct DetailedCommand;
    impl Command for DetailedCommand {
        fn id(&self) -> CommandId {
            CommandId::new(ModuleId::new("my-module"), "my-command")
        }
        fn description(&self) -> &'static str {
            "Detailed"
        }
    }

    let cmd = DetailedCommand;
    assert_eq!(cmd.id().name(), "my-command");
    assert_eq!(cmd.id().module().as_str(), "my-module");
}

// ========================================================================
// Command box vs ref trait object
// ========================================================================

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_command_as_boxed_trait_object() {
    use reovim_kernel::api::v1::ModuleId;

    struct BoxableCommand;
    impl Command for BoxableCommand {
        fn id(&self) -> CommandId {
            CommandId::new(ModuleId::new("test"), "boxable")
        }
        fn description(&self) -> &'static str {
            "Boxable"
        }
        fn args(&self) -> Vec<ArgSpec> {
            vec![]
        }
        fn names(&self) -> &[&'static str] {
            &["boxable"]
        }
    }

    let cmd: Box<dyn Command> = Box::new(BoxableCommand);
    assert_eq!(cmd.id().name(), "boxable");
    assert_eq!(cmd.description(), "Boxable");
    assert!(cmd.args().is_empty());
    assert_eq!(cmd.names(), &["boxable"]);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_command_handler_as_boxed_trait_object() {
    use reovim_kernel::api::v1::ModuleId;

    struct BoxableHandler;
    impl Command for BoxableHandler {
        fn id(&self) -> CommandId {
            CommandId::new(ModuleId::new("test"), "boxable-handler")
        }
        fn description(&self) -> &'static str {
            "Boxable handler"
        }
    }
    impl CommandHandler for BoxableHandler {
        fn execute(
            &self,
            _runtime: &mut SessionRuntime<'_>,
            _args: &CommandContext,
        ) -> CommandResult {
            CommandResult::Success
        }
    }

    let cmd: Box<dyn CommandHandler> = Box::new(BoxableHandler);
    assert_eq!(cmd.id().name(), "boxable-handler");
    assert_eq!(cmd.description(), "Boxable handler");
}

// ========================================================================
// Command::complete() tests (#547)
// ========================================================================

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_command_default_complete_is_empty() {
    use reovim_kernel::api::v1::ModuleId;

    struct NoCompleteCommand;
    impl Command for NoCompleteCommand {
        fn id(&self) -> CommandId {
            CommandId::new(ModuleId::new("test"), "no-complete")
        }
        fn description(&self) -> &'static str {
            "No completions"
        }
    }

    let cmd = NoCompleteCommand;
    assert!(cmd.complete("").is_empty());
    assert!(cmd.complete("foo").is_empty());
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_command_custom_complete() {
    use reovim_kernel::api::v1::ModuleId;

    struct CompletingCommand;
    impl Command for CompletingCommand {
        fn id(&self) -> CommandId {
            CommandId::new(ModuleId::new("test"), "completing")
        }
        fn description(&self) -> &'static str {
            "With completions"
        }
        fn complete(&self, partial: &str) -> Vec<String> {
            vec![format!("{partial}-dark"), format!("{partial}-light")]
        }
    }

    let cmd = CompletingCommand;
    let completions = cmd.complete("gru");
    assert_eq!(completions.len(), 2);
    assert_eq!(completions[0], "gru-dark");
    assert_eq!(completions[1], "gru-light");
}

// ========================================================================
// CommandPriority tests (#545)
// ========================================================================

#[test]
fn test_command_priority_default_is_normal() {
    assert_eq!(CommandPriority::default(), CommandPriority::Normal);
}

#[test]
fn test_command_priority_ordering() {
    assert!(CommandPriority::Normal < CommandPriority::Override);
}

#[test]
fn test_command_priority_equality() {
    assert_eq!(CommandPriority::Normal, CommandPriority::Normal);
    assert_eq!(CommandPriority::Override, CommandPriority::Override);
    assert_ne!(CommandPriority::Normal, CommandPriority::Override);
}

#[test]
fn test_command_priority_debug() {
    let debug = format!("{:?}", CommandPriority::Normal);
    assert!(debug.contains("Normal"));
    let debug = format!("{:?}", CommandPriority::Override);
    assert!(debug.contains("Override"));
}

#[test]
fn test_command_priority_copy() {
    let p = CommandPriority::Override;
    let copied = p;
    assert_eq!(p, copied);
}

#[test]
fn test_command_priority_hash() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(CommandPriority::Normal);
    set.insert(CommandPriority::Override);
    assert_eq!(set.len(), 2);
    set.insert(CommandPriority::Normal);
    assert_eq!(set.len(), 2);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_command_default_priority_is_normal() {
    use reovim_kernel::api::v1::ModuleId;

    struct MinimalCommand;
    impl Command for MinimalCommand {
        fn id(&self) -> CommandId {
            CommandId::new(ModuleId::new("test"), "minimal")
        }
        fn description(&self) -> &'static str {
            "Minimal"
        }
    }

    let cmd = MinimalCommand;
    assert_eq!(cmd.priority(), CommandPriority::Normal);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_command_custom_priority_override() {
    use reovim_kernel::api::v1::ModuleId;

    struct OverrideCommand;
    impl Command for OverrideCommand {
        fn id(&self) -> CommandId {
            CommandId::new(ModuleId::new("test"), "override")
        }
        fn description(&self) -> &'static str {
            "Override"
        }
        fn priority(&self) -> CommandPriority {
            CommandPriority::Override
        }
    }

    let cmd = OverrideCommand;
    assert_eq!(cmd.priority(), CommandPriority::Override);
    assert!(cmd.priority() > CommandPriority::Normal);
}
