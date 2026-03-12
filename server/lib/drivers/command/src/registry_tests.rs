use {
    super::*,
    crate::{ArgSpec, Command, CommandContext, CommandResult},
    reovim_driver_session::SessionRuntime,
    reovim_kernel::api::v1::{CommandId, ModuleId},
};

struct TestCommand {
    id: CommandId,
}

impl TestCommand {
    fn new(name: &'static str) -> Self {
        Self {
            id: CommandId::new(ModuleId::new("test"), name),
        }
    }
}

impl Command for TestCommand {
    fn id(&self) -> CommandId {
        self.id.clone()
    }

    fn description(&self) -> &'static str {
        "Test command"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![]
    }
}

impl CommandHandler for TestCommand {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        CommandResult::Success
    }
}

#[test]
fn test_store_new() {
    let store = CommandHandlerStore::new();
    assert!(store.is_empty());
    assert_eq!(store.len(), 0);
}

#[test]
fn test_store_default() {
    let store = CommandHandlerStore::default();
    assert!(store.is_empty());
    assert_eq!(store.len(), 0);
}

#[test]
fn test_store_add() {
    let store = CommandHandlerStore::new();
    store.add(Box::new(TestCommand::new("test1")));
    store.add(Box::new(TestCommand::new("test2")));

    assert_eq!(store.len(), 2);
    assert!(!store.is_empty());
}

#[test]
fn test_store_add_arc() {
    let store = CommandHandlerStore::new();
    let handler: Arc<dyn CommandHandler> = Arc::new(TestCommand::new("arc-cmd"));
    store.add_arc(handler);

    assert_eq!(store.len(), 1);
    assert!(!store.is_empty());
}

#[test]
fn test_store_add_arc_multiple() {
    let store = CommandHandlerStore::new();
    store.add_arc(Arc::new(TestCommand::new("arc1")));
    store.add_arc(Arc::new(TestCommand::new("arc2")));
    store.add_arc(Arc::new(TestCommand::new("arc3")));

    assert_eq!(store.len(), 3);
}

#[test]
fn test_store_add_mixed_box_and_arc() {
    let store = CommandHandlerStore::new();
    store.add(Box::new(TestCommand::new("boxed")));
    store.add_arc(Arc::new(TestCommand::new("arced")));

    assert_eq!(store.len(), 2);
}

#[test]
fn test_store_take_handlers() {
    let store = CommandHandlerStore::new();
    store.add(Box::new(TestCommand::new("test1")));
    store.add(Box::new(TestCommand::new("test2")));

    let handlers = store.take_handlers();
    assert_eq!(handlers.len(), 2);
    assert!(store.is_empty()); // Store should be empty after take
}

#[test]
fn test_store_take_handlers_empty() {
    let store = CommandHandlerStore::new();
    let handlers = store.take_handlers();
    assert!(handlers.is_empty());
    assert!(store.is_empty());
}

#[test]
fn test_store_take_handlers_twice() {
    let store = CommandHandlerStore::new();
    store.add(Box::new(TestCommand::new("test1")));

    let first = store.take_handlers();
    assert_eq!(first.len(), 1);

    let second = store.take_handlers();
    assert!(second.is_empty());
}

#[test]
fn test_store_add_after_take() {
    let store = CommandHandlerStore::new();
    store.add(Box::new(TestCommand::new("test1")));

    let _ = store.take_handlers();
    assert!(store.is_empty());

    store.add(Box::new(TestCommand::new("test2")));
    assert_eq!(store.len(), 1);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_store_debug() {
    let store = CommandHandlerStore::new();
    store.add(Box::new(TestCommand::new("test1")));
    store.add(Box::new(TestCommand::new("test2")));

    let debug_str = format!("{store:?}");
    assert!(debug_str.contains("CommandHandlerStore"));
    assert!(debug_str.contains('2'));
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_store_debug_empty() {
    let store = CommandHandlerStore::new();
    let debug_str = format!("{store:?}");
    assert!(debug_str.contains("CommandHandlerStore"));
    assert!(debug_str.contains('0'));
}

#[test]
fn test_store_service_impl() {
    // Verify CommandHandlerStore implements Service
    fn accepts_service(_: &dyn Service) {}
    let store = CommandHandlerStore::new();
    accepts_service(&store);
}

#[test]
fn test_store_take_handlers_preserves_command_metadata() {
    let store = CommandHandlerStore::new();
    store.add(Box::new(TestCommand::new("my-command")));

    let handlers = store.take_handlers();
    assert_eq!(handlers.len(), 1);
    assert_eq!(handlers[0].id().name(), "my-command");
    assert_eq!(handlers[0].description(), "Test command");
    assert!(handlers[0].args().is_empty());
}
