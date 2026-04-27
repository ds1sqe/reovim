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
#[path = "factory_tests.rs"]
mod tests;
