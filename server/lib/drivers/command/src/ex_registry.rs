//! Ex-command handler registry for `ServiceRegistry`.
//!
//! This module provides an ex-command handler store that can be stored in
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
//! let store = ctx.services.get_or_create::<ExCommandHandlerStore>();
//! for handler in self.ex_commands() {
//!     store.add(handler);
//! }
//!
//! // In runner after all modules initialized:
//! let store = services.get::<ExCommandHandlerStore>().unwrap();
//! let handlers = store.take_handlers();
//! // Create ExCommandDispatcher from handlers...
//! ```

use std::sync::{Arc, RwLock};

use reovim_kernel::api::v1::Service;

use crate::ex_handler::ExCommandHandler;

/// Store for ex-command handlers registered by modules.
///
/// Modules register their handlers during `init()` by calling `add()`.
/// After all modules are initialized, the runner extracts handlers
/// via `take_handlers()`.
///
/// # Thread Safety
///
/// Uses `RwLock` for interior mutability, allowing modules to register
/// handlers concurrently if needed.
pub struct ExCommandHandlerStore {
    handlers: RwLock<Vec<Arc<dyn ExCommandHandler>>>,
}

impl ExCommandHandlerStore {
    /// Create a new empty handler store.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // RwLock::new() is not const
    pub fn new() -> Self {
        Self {
            handlers: RwLock::new(Vec::new()),
        }
    }

    /// Add an ex-command handler to the store.
    ///
    /// Called by modules during `init()`.
    ///
    /// # Panics
    ///
    /// Panics if the lock is poisoned.
    pub fn add(&self, handler: Box<dyn ExCommandHandler>) {
        self.handlers
            .write()
            .expect("ExCommandHandlerStore lock poisoned")
            .push(handler.into());
    }

    /// Add an Arc-wrapped ex-command handler to the store.
    ///
    /// # Panics
    ///
    /// Panics if the lock is poisoned.
    pub fn add_arc(&self, handler: Arc<dyn ExCommandHandler>) {
        self.handlers
            .write()
            .expect("ExCommandHandlerStore lock poisoned")
            .push(handler);
    }

    /// Take all handlers, clearing the store.
    ///
    /// Called by runner after all modules are initialized.
    ///
    /// # Panics
    ///
    /// Panics if the lock is poisoned.
    pub fn take_handlers(&self) -> Vec<Arc<dyn ExCommandHandler>> {
        std::mem::take(
            &mut *self
                .handlers
                .write()
                .expect("ExCommandHandlerStore lock poisoned"),
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
            .expect("ExCommandHandlerStore lock poisoned")
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
            .expect("ExCommandHandlerStore lock poisoned")
            .is_empty()
    }
}

impl Default for ExCommandHandlerStore {
    fn default() -> Self {
        Self::new()
    }
}

// Implement Service so ExCommandHandlerStore can be stored in ServiceRegistry
impl Service for ExCommandHandlerStore {}

impl std::fmt::Debug for ExCommandHandlerStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExCommandHandlerStore")
            .field("count", &self.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use {super::*, crate::ex_handler::ExCommandContext};

    struct TestExCommand {
        name: &'static str,
    }

    impl TestExCommand {
        const fn new(name: &'static str) -> Self {
            Self { name }
        }
    }

    impl ExCommandHandler for TestExCommand {
        fn id(&self) -> &'static str {
            self.name
        }

        fn names(&self) -> &[&'static str] {
            &[]
        }

        fn execute(
            &self,
            _ctx: &mut ExCommandContext<'_>,
            _args: &[&str],
        ) -> Result<(), crate::ex_handler::ExCommandError> {
            Ok(())
        }
    }

    #[test]
    fn test_store_new() {
        let store = ExCommandHandlerStore::new();
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
    }

    #[test]
    fn test_store_add() {
        let store = ExCommandHandlerStore::new();
        store.add(Box::new(TestExCommand::new("test1")));
        store.add(Box::new(TestExCommand::new("test2")));

        assert_eq!(store.len(), 2);
        assert!(!store.is_empty());
    }

    #[test]
    fn test_store_take_handlers() {
        let store = ExCommandHandlerStore::new();
        store.add(Box::new(TestExCommand::new("test1")));
        store.add(Box::new(TestExCommand::new("test2")));

        let handlers = store.take_handlers();
        assert_eq!(handlers.len(), 2);
        assert!(store.is_empty()); // Store should be empty after take
    }
}
