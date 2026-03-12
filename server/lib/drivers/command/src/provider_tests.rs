use {
    super::*,
    crate::{ArgSpec, Command, CommandContext, CommandResult},
    reovim_driver_session::SessionRuntime,
    reovim_kernel::api::v1::{CommandId, ModuleId},
};

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_command_provider_object_safety() {
    // Inner fns verify compilation only, never called
    fn _accepts_ref(_: &dyn CommandProvider) {}
    fn _accepts_box(_: Box<dyn CommandProvider>) {}
}

struct TestCmd;

impl Command for TestCmd {
    fn id(&self) -> CommandId {
        CommandId::new(ModuleId::new("test"), "test-cmd")
    }
    fn description(&self) -> &'static str {
        "Test command"
    }
}

impl CommandHandler for TestCmd {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        CommandResult::Success
    }
}

struct TestProvider;

impl CommandProvider for TestProvider {
    fn command_handlers(&self) -> Vec<Box<dyn CommandHandler>> {
        vec![Box::new(TestCmd)]
    }
}

#[test]
fn test_command_provider_returns_handlers() {
    let provider = TestProvider;
    let handlers = provider.command_handlers();
    assert_eq!(handlers.len(), 1);
    assert_eq!(handlers[0].id().name(), "test-cmd");
    assert_eq!(handlers[0].description(), "Test command");
}

struct EmptyProvider;

impl CommandProvider for EmptyProvider {
    fn command_handlers(&self) -> Vec<Box<dyn CommandHandler>> {
        vec![]
    }
}

#[test]
fn test_command_provider_empty() {
    let provider = EmptyProvider;
    let handlers = provider.command_handlers();
    assert!(handlers.is_empty());
}

struct MultiProvider;

impl CommandProvider for MultiProvider {
    fn command_handlers(&self) -> Vec<Box<dyn CommandHandler>> {
        vec![Box::new(TestCmd), Box::new(TestCmd)]
    }
}

#[test]
fn test_command_provider_multiple_handlers() {
    let provider = MultiProvider;
    let handlers = provider.command_handlers();
    assert_eq!(handlers.len(), 2);
}

#[test]
fn test_command_provider_as_trait_object() {
    let provider: &dyn CommandProvider = &TestProvider;
    let handlers = provider.command_handlers();
    assert_eq!(handlers.len(), 1);
}

#[test]
fn test_command_provider_fresh_vec_each_call() {
    let provider = TestProvider;
    let handlers1 = provider.command_handlers();
    let handlers2 = provider.command_handlers();
    // Each call returns a fresh vector
    assert_eq!(handlers1.len(), 1);
    assert_eq!(handlers2.len(), 1);
}

#[test]
fn test_command_provider_handler_has_no_args_by_default() {
    let provider = TestProvider;
    let handlers = provider.command_handlers();
    let args: Vec<ArgSpec> = handlers[0].args();
    assert!(args.is_empty());
}
