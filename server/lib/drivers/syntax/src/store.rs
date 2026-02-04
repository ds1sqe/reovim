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
mod tests {
    use {
        super::*,
        crate::{HighlightSpan, SyntaxDriver, SyntaxEdit},
        std::ops::Range,
    };

    /// Mock driver for testing.
    struct MockDriver;

    impl SyntaxDriver for MockDriver {
        #[allow(clippy::unnecessary_literal_bound)]
        fn language(&self) -> &str {
            "mock"
        }

        fn parse(&mut self, _content: &str) {}

        fn update(&mut self, _content: &str, _edit: &SyntaxEdit) {}

        fn highlights(&self, _byte_range: Range<usize>) -> Vec<HighlightSpan> {
            Vec::new()
        }

        fn is_parsed(&self) -> bool {
            false
        }
    }

    /// Mock factory for testing.
    struct MockFactory {
        language: &'static str,
    }

    impl SyntaxDriverFactory for MockFactory {
        fn create(&self, language_id: &str) -> Option<Box<dyn SyntaxDriver>> {
            if language_id == self.language {
                Some(Box::new(MockDriver))
            } else {
                None
            }
        }

        fn supported_languages(&self) -> Vec<&str> {
            vec![self.language]
        }

        fn supports(&self, language_id: &str) -> bool {
            language_id == self.language
        }
    }

    #[test]
    fn test_store_new_empty() {
        let store = SyntaxFactoryStore::new();
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
    }

    #[test]
    fn test_store_add_and_find() {
        let store = SyntaxFactoryStore::new();

        store.add(Arc::new(MockFactory { language: "rust" }));
        store.add(Arc::new(MockFactory { language: "python" }));

        assert_eq!(store.len(), 2);
        assert!(store.find("rust").is_some());
        assert!(store.find("python").is_some());
        assert!(store.find("javascript").is_none());
    }

    #[test]
    fn test_store_take_drains() {
        let store = SyntaxFactoryStore::new();

        store.add(Arc::new(MockFactory { language: "rust" }));
        store.add(Arc::new(MockFactory { language: "python" }));

        let factories = store.take_factories();
        assert_eq!(factories.len(), 2);

        // Store should now be empty
        assert!(store.is_empty());
        assert!(store.take_factories().is_empty());
    }
}
