//! Syntax factory store for module self-registration.
//!
//! This module provides `SyntaxFactoryStore`, a registry where syntax modules
//! can register their factories during `init()`. This follows the same pattern
//! as `ModeInfoStore`, `CommandHandlerStore`, etc.
//!
//! # Self-Registration Pattern
//!
//! ```ignore
//! // In treesitter-rust module's init():
//! impl Module for TreesitterRustModule {
//!     fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
//!         let store = ctx.services.get_or_create::<SyntaxFactoryStore>();
//!         store.add(Arc::new(RustSyntaxFactory::new()));
//!         ProbeResult::Success
//!     }
//! }
//! ```

use std::sync::Arc;

use {parking_lot::RwLock, reovim_kernel::api::v1::Service};

use crate::SyntaxDriverFactory;

/// Store for syntax driver factories registered by modules during init.
///
/// Similar to `ModeInfoStore`, `CommandHandlerStore`, etc.
/// Modules register their factories here, and bootstrap extracts them
/// after module initialization.
#[derive(Default)]
pub struct SyntaxFactoryStore {
    /// Registered factories (populated during module init).
    factories: RwLock<Vec<Arc<dyn SyntaxDriverFactory>>>,
}

impl SyntaxFactoryStore {
    /// Create a new empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a factory to the store.
    ///
    /// Called by syntax modules during `init()`.
    pub fn add(&self, factory: Arc<dyn SyntaxDriverFactory>) {
        self.factories.write().push(factory);
    }

    /// Take all registered factories.
    ///
    /// Called by bootstrap after all modules have initialized.
    /// This drains the store, so subsequent calls return empty vec.
    pub fn take_factories(&self) -> Vec<Arc<dyn SyntaxDriverFactory>> {
        std::mem::take(&mut *self.factories.write())
    }

    /// Find a factory that supports the given language.
    ///
    /// Returns the first factory that reports supporting the language.
    #[must_use]
    pub fn find(&self, language_id: &str) -> Option<Arc<dyn SyntaxDriverFactory>> {
        self.factories
            .read()
            .iter()
            .find(|f| f.supports(language_id))
            .cloned()
    }

    /// Get the number of registered factories.
    #[must_use]
    pub fn len(&self) -> usize {
        self.factories.read().len()
    }

    /// Check if no factories are registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.factories.read().is_empty()
    }
}

/// Implement `Service` trait for `ServiceRegistry` compatibility.
impl Service for SyntaxFactoryStore {}

impl std::fmt::Debug for SyntaxFactoryStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SyntaxFactoryStore")
            .field("count", &self.len())
            .finish()
    }
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
