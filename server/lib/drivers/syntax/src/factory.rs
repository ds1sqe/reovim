//! Syntax driver factory trait.
//!
//! This module defines the [`SyntaxDriverFactory`] trait for creating
//! syntax drivers for specific languages.

use crate::driver::SyntaxDriver;

/// Factory for creating syntax drivers.
///
/// Implementations know how to create drivers for specific languages.
/// The factory is registered with the runtime and called when files are opened.
///
/// # Thread Safety
///
/// Implementations must be `Send + Sync` to allow use across threads.
///
/// # Example
///
/// ```ignore
/// // Factory creates drivers for supported languages
/// let factory: Box<dyn SyntaxDriverFactory> = get_factory();
///
/// // Check if language is supported
/// if factory.supports("rust") {
///     // Create driver for Rust
///     let driver = factory.create("rust").unwrap();
///     assert_eq!(driver.language(), "rust");
/// }
///
/// // List all supported languages
/// for lang in factory.supported_languages() {
///     println!("Supports: {}", lang);
/// }
/// ```
pub trait SyntaxDriverFactory: Send + Sync {
    /// Create a syntax driver for a language.
    ///
    /// # Arguments
    ///
    /// * `language_id` - The language identifier (e.g., "rust", "python")
    ///
    /// # Returns
    ///
    /// `Some(driver)` if the language is supported, `None` otherwise.
    fn create(&self, language_id: &str) -> Option<Box<dyn SyntaxDriver>>;

    /// List all supported language IDs.
    ///
    /// Returns a list of language identifiers that can be passed to
    /// [`create()`](Self::create).
    fn supported_languages(&self) -> Vec<&str>;

    /// Check if a language is supported.
    ///
    /// # Default
    ///
    /// Checks if the language ID is in [`supported_languages()`](Self::supported_languages).
    fn supports(&self, language_id: &str) -> bool {
        self.supported_languages().contains(&language_id)
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            edit::SyntaxEdit,
            highlight::{HighlightGroup, HighlightSpan},
        },
        std::ops::Range,
    };

    /// Minimal driver for testing.
    struct MinimalDriver {
        language: &'static str,
    }

    impl SyntaxDriver for MinimalDriver {
        fn language(&self) -> &str {
            self.language
        }

        fn parse(&mut self, _content: &str) {}

        fn update(&mut self, _content: &str, _edit: &SyntaxEdit) {}

        fn highlights(&self, _byte_range: Range<usize>) -> Vec<HighlightSpan> {
            vec![HighlightSpan::new(0, 1, HighlightGroup::Comment)]
        }

        fn is_parsed(&self) -> bool {
            true
        }
    }

    /// Test factory implementation.
    struct TestFactory;

    impl SyntaxDriverFactory for TestFactory {
        fn create(&self, language_id: &str) -> Option<Box<dyn SyntaxDriver>> {
            match language_id {
                "rust" => Some(Box::new(MinimalDriver { language: "rust" })),
                "python" => Some(Box::new(MinimalDriver { language: "python" })),
                _ => None,
            }
        }

        fn supported_languages(&self) -> Vec<&str> {
            vec!["rust", "python"]
        }
    }

    #[test]
    fn test_factory_create() {
        let factory = TestFactory;

        let rust = factory.create("rust");
        assert!(rust.is_some());
        assert_eq!(rust.unwrap().language(), "rust");

        let python = factory.create("python");
        assert!(python.is_some());
        assert_eq!(python.unwrap().language(), "python");

        let unknown = factory.create("unknown");
        assert!(unknown.is_none());
    }

    #[test]
    fn test_factory_supported_languages() {
        let factory = TestFactory;

        let languages = factory.supported_languages();
        assert_eq!(languages.len(), 2);
        assert!(languages.contains(&"rust"));
        assert!(languages.contains(&"python"));
    }

    #[test]
    fn test_factory_supports() {
        let factory = TestFactory;

        assert!(factory.supports("rust"));
        assert!(factory.supports("python"));
        assert!(!factory.supports("unknown"));
        assert!(!factory.supports("javascript"));
    }
}
