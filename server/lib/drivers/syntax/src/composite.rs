//! Composite syntax driver factory.
//!
//! Routes `create()` calls to the correct factory by language ID.
//! This replaces the single-factory limitation in bootstrap where
//! only the first registered factory was used.

use std::sync::Arc;

use crate::{SyntaxDriver, SyntaxDriverFactory};

/// A factory that aggregates multiple [`SyntaxDriverFactory`] instances.
///
/// When `create()` is called, it iterates the inner factories in registration
/// order and returns the first successful result. This allows multiple
/// language-specific factories (e.g., Rust + Markdown) to coexist.
///
/// # Example
///
/// ```ignore
/// let composite = CompositeFactory::new(vec![
///     Arc::new(RustSyntaxFactory::new()),
///     Arc::new(MarkdownSyntaxFactory::new()),
/// ]);
/// assert!(composite.supports("rust"));
/// assert!(composite.supports("markdown"));
/// ```
pub struct CompositeFactory {
    factories: Vec<Arc<dyn SyntaxDriverFactory>>,
}

impl CompositeFactory {
    /// Create a composite from a list of factories.
    #[must_use]
    pub fn new(factories: Vec<Arc<dyn SyntaxDriverFactory>>) -> Self {
        Self { factories }
    }

    /// Get the number of inner factories.
    #[must_use]
    pub fn factory_count(&self) -> usize {
        self.factories.len()
    }
}

impl SyntaxDriverFactory for CompositeFactory {
    fn create(&self, language_id: &str) -> Option<Box<dyn SyntaxDriver>> {
        for factory in &self.factories {
            if let Some(driver) = factory.create(language_id) {
                return Some(driver);
            }
        }
        None
    }

    fn supported_languages(&self) -> Vec<&str> {
        let mut languages = Vec::new();
        for factory in &self.factories {
            for lang in factory.supported_languages() {
                if !languages.contains(&lang) {
                    languages.push(lang);
                }
            }
        }
        languages
    }

    fn supports(&self, language_id: &str) -> bool {
        self.factories.iter().any(|f| f.supports(language_id))
    }
}

impl std::fmt::Debug for CompositeFactory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompositeFactory")
            .field("factory_count", &self.factories.len())
            .field("languages", &self.supported_languages())
            .finish()
    }
}

#[cfg(test)]
#[path = "composite_tests.rs"]
mod tests;
