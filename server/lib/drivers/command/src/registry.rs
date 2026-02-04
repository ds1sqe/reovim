//! Command handler registry for `ServiceRegistry`.
//!
//! This module provides a command handler store that can be stored in
//! `ServiceRegistry` for module self-registration during `init()`.
//!
//! # Architecture
//!
//! Following the Epic #417 pattern:
//! - **Mechanism (driver)**: This registry type
//! - **Policy (modules)**: Register their handlers during `init()`
//!
//! # Example
//!
//! ```ignore
//! // In module init():
//! let store = ctx.services.get_or_create::<CommandHandlerStore>();
//! for handler in self.command_handlers() {
//!     store.add(handler);
//! }
//!
//! // In runner after all modules initialized:
//! let store = services.get::<CommandHandlerStore>().unwrap();
//! for handler in store.handlers() {
//!     command_registry.register(handler);
//! }
//! ```

use std::sync::{Arc, RwLock};

use reovim_kernel::api::v1::Service;

use crate::CommandHandler;

/// Store for command handlers registered by modules.
///
/// Modules register their handlers during `init()` by calling `add()`.
/// After all modules are initialized, the runner extracts handlers
/// via `take_handlers()`.
///
/// # Thread Safety
///
/// Uses `RwLock` for interior mutability, allowing modules to register
/// handlers concurrently if needed.
pub struct CommandHandlerStore {
    handlers: RwLock<Vec<Arc<dyn CommandHandler>>>,
}

impl CommandHandlerStore {
    /// Create a new empty handler store.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // RwLock::new() is not const
    pub fn new() -> Self {
        Self {
            handlers: RwLock::new(Vec::new()),
        }
    }

    /// Add a command handler to the store.
    ///
    /// Called by modules during `init()`.
    ///
    /// # Panics
    ///
    /// Panics if the lock is poisoned.
    pub fn add(&self, handler: Box<dyn CommandHandler>) {
        self.handlers
            .write()
            .expect("CommandHandlerStore lock poisoned")
            .push(handler.into());
    }

    /// Add an Arc-wrapped command handler to the store.
    ///
    /// # Panics
    ///
    /// Panics if the lock is poisoned.
    pub fn add_arc(&self, handler: Arc<dyn CommandHandler>) {
        self.handlers
            .write()
            .expect("CommandHandlerStore lock poisoned")
            .push(handler);
    }

    /// Take all handlers, clearing the store.
    ///
    /// Called by runner after all modules are initialized.
    ///
    /// # Panics
    ///
    /// Panics if the lock is poisoned.
    pub fn take_handlers(&self) -> Vec<Arc<dyn CommandHandler>> {
        std::mem::take(
            &mut *self
                .handlers
                .write()
                .expect("CommandHandlerStore lock poisoned"),
        )
    }

    /// Get the number of registered handlers.
    ///
    /// # Panics
    ///
    /// Panics if the lock is poisoned.
    #[must_use]
    pub fn len(&self) -> usize {
        self.handlers
            .read()
            .expect("CommandHandlerStore lock poisoned")
            .len()
    }

    /// Check if the store is empty.
    ///
    /// # Panics
    ///
    /// Panics if the lock is poisoned.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.handlers
            .read()
            .expect("CommandHandlerStore lock poisoned")
            .is_empty()
    }
}

impl Default for CommandHandlerStore {
    fn default() -> Self {
        Self::new()
    }
}

// Implement Service so CommandHandlerStore can be stored in ServiceRegistry
impl Service for CommandHandlerStore {}

impl std::fmt::Debug for CommandHandlerStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CommandHandlerStore")
            .field("count", &self.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
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
        fn execute(
            &self,
            _runtime: &mut SessionRuntime<'_>,
            _args: &CommandContext,
        ) -> CommandResult {
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
    fn test_store_add() {
        let store = CommandHandlerStore::new();
        store.add(Box::new(TestCommand::new("test1")));
        store.add(Box::new(TestCommand::new("test2")));

        assert_eq!(store.len(), 2);
        assert!(!store.is_empty());
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
}
