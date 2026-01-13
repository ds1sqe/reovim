//! Factory for creating tree-sitter based syntax drivers.
//!
//! This module provides `TreeSitterDriverFactory` which implements the
//! `SyntaxDriverFactory` trait to create `TreeSitterDriver` instances
//! for registered languages.

use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use reovim_driver_syntax::{SyntaxDriver, SyntaxDriverFactory};

use crate::{
    capture_mapper::CaptureMapper,
    queries::{QueryCache, QueryType},
    syntax_driver::TreeSitterDriver,
};

/// Configuration for a language.
///
/// Contains the tree-sitter grammar and query sources.
/// Used by `TreeSitterDriverFactory` to create drivers.
pub struct LanguageConfig {
    /// Unique language identifier (e.g., "rust", "python")
    pub id: &'static str,
    /// Tree-sitter language grammar
    pub language: tree_sitter::Language,
    /// Highlights query source
    pub highlights_query: &'static str,
    /// Optional folds query source
    pub folds_query: Option<&'static str>,
    /// Optional injections query source
    pub injections_query: Option<&'static str>,
    /// File extensions (without dot)
    pub extensions: &'static [&'static str],
}

/// Internal storage for registered language
struct RegisteredLanguage {
    language: tree_sitter::Language,
    highlights_query: &'static str,
    folds_query: Option<&'static str>,
    injections_query: Option<&'static str>,
    extensions: Vec<&'static str>,
}

/// Factory for creating tree-sitter based syntax drivers.
///
/// Languages are registered via `register()`, then drivers are
/// created via the `SyntaxDriverFactory::create()` method.
///
/// # Thread Safety
///
/// This factory is `Send + Sync` through interior mutability with `RwLock`.
/// The query cache is shared between all drivers.
///
/// # Example
///
/// ```ignore
/// use reovim_plugin_treesitter::driver_factory::{TreeSitterDriverFactory, LanguageConfig};
/// use reovim_driver_syntax::SyntaxDriverFactory;
///
/// let mut factory = TreeSitterDriverFactory::new();
///
/// // Register a language
/// factory.register(LanguageConfig {
///     id: "rust",
///     language: tree_sitter_rust::LANGUAGE.into(),
///     highlights_query: include_str!("queries/highlights.scm"),
///     folds_query: None,
///     injections_query: None,
///     extensions: &["rs"],
/// });
///
/// // Create a driver
/// let driver = factory.create("rust").unwrap();
/// assert_eq!(driver.language(), "rust");
/// ```
pub struct TreeSitterDriverFactory {
    /// Registered languages
    languages: RwLock<HashMap<String, RegisteredLanguage>>,
    /// Extension to language ID mapping for quick lookup
    extension_map: RwLock<HashMap<String, String>>,
    /// Shared capture mapper
    capture_mapper: Arc<CaptureMapper>,
    /// Query cache for compiled queries
    query_cache: QueryCache,
}

impl TreeSitterDriverFactory {
    /// Create a new factory with default capture mapper.
    #[must_use]
    pub fn new() -> Self {
        Self {
            languages: RwLock::new(HashMap::new()),
            extension_map: RwLock::new(HashMap::new()),
            capture_mapper: Arc::new(CaptureMapper::new()),
            query_cache: QueryCache::new(),
        }
    }

    /// Create a new factory with a custom capture mapper.
    #[must_use]
    pub fn with_capture_mapper(capture_mapper: CaptureMapper) -> Self {
        Self {
            languages: RwLock::new(HashMap::new()),
            extension_map: RwLock::new(HashMap::new()),
            capture_mapper: Arc::new(capture_mapper),
            query_cache: QueryCache::new(),
        }
    }

    /// Register a language configuration.
    ///
    /// After registration, drivers can be created for this language.
    pub fn register(&self, config: LanguageConfig) {
        let id = config.id.to_string();

        // Register extension mappings
        {
            let mut ext_map = self.extension_map.write().unwrap();
            for &ext in config.extensions {
                ext_map.insert(ext.to_string(), id.clone());
            }
        }

        // Register language
        {
            let mut languages = self.languages.write().unwrap();
            languages.insert(
                id.clone(),
                RegisteredLanguage {
                    language: config.language,
                    highlights_query: config.highlights_query,
                    folds_query: config.folds_query,
                    injections_query: config.injections_query,
                    extensions: config.extensions.to_vec(),
                },
            );
        }

        tracing::debug!(
            language_id = %id,
            extensions = ?config.extensions,
            has_folds = config.folds_query.is_some(),
            has_injections = config.injections_query.is_some(),
            "Registered language with TreeSitterDriverFactory"
        );
    }

    /// Detect language from file extension.
    ///
    /// Returns the language ID for the given file path, or None if unknown.
    #[must_use]
    pub fn detect_language(&self, file_path: &str) -> Option<String> {
        let extension = file_path.rsplit('.').next()?;
        self.extension_map.read().unwrap().get(extension).cloned()
    }

    /// Create a driver from a file path (auto-detecting language).
    ///
    /// Returns None if the language cannot be detected or isn't supported.
    #[must_use]
    pub fn create_for_file(&self, file_path: &str) -> Option<Box<dyn SyntaxDriver>> {
        let language_id = self.detect_language(file_path)?;
        self.create(&language_id)
    }

    /// Get the number of registered languages.
    #[must_use]
    pub fn language_count(&self) -> usize {
        self.languages.read().unwrap().len()
    }

    /// Get the query cache for pre-compilation.
    #[must_use]
    pub fn query_cache(&self) -> &QueryCache {
        &self.query_cache
    }

    /// Pre-compile all queries for a language.
    ///
    /// Called during startup to avoid lazy compilation delays.
    pub fn precompile_language(&self, language_id: &str) -> usize {
        let languages = self.languages.read().unwrap();
        let Some(lang) = languages.get(language_id) else {
            return 0;
        };

        let mut count = 0;

        // Compile highlights query
        if self
            .query_cache
            .compile_and_cache(
                language_id,
                QueryType::Highlights,
                &lang.language,
                lang.highlights_query,
            )
            .is_some()
        {
            count += 1;
        }

        // Compile folds query if present
        if let Some(source) = lang.folds_query
            && self
                .query_cache
                .compile_and_cache(language_id, QueryType::Folds, &lang.language, source)
                .is_some()
        {
            count += 1;
        }

        // Compile injections query if present
        if let Some(source) = lang.injections_query
            && self
                .query_cache
                .compile_and_cache(language_id, QueryType::Injections, &lang.language, source)
                .is_some()
        {
            count += 1;
        }

        count
    }

    /// Pre-compile all queries for all registered languages.
    ///
    /// Returns the total number of queries compiled.
    pub fn precompile_all(&self) -> usize {
        let language_ids: Vec<String> = self.languages.read().unwrap().keys().cloned().collect();

        let mut total = 0;
        for id in language_ids {
            total += self.precompile_language(&id);
        }

        tracing::info!(
            languages = self.language_count(),
            queries = total,
            "Pre-compiled all queries"
        );

        total
    }
}

impl Default for TreeSitterDriverFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl SyntaxDriverFactory for TreeSitterDriverFactory {
    fn create(&self, language_id: &str) -> Option<Box<dyn SyntaxDriver>> {
        let languages = self.languages.read().unwrap();
        let lang = languages.get(language_id)?;

        // Get or compile highlights query (required)
        let highlight_query = self.query_cache.get_or_compile(
            language_id,
            QueryType::Highlights,
            &lang.language,
            lang.highlights_query,
        )?;

        // Get or compile folds query (optional)
        let folds_query = lang.folds_query.and_then(|source| {
            self.query_cache
                .get_or_compile(language_id, QueryType::Folds, &lang.language, source)
        });

        // Get or compile injections query (optional)
        let injections_query = lang.injections_query.and_then(|source| {
            self.query_cache.get_or_compile(
                language_id,
                QueryType::Injections,
                &lang.language,
                source,
            )
        });

        // Create driver
        let driver = TreeSitterDriver::new(
            language_id,
            &lang.language,
            highlight_query,
            folds_query,
            injections_query,
            Arc::clone(&self.capture_mapper),
        )?;

        tracing::trace!(
            language_id = %language_id,
            has_folds = driver.supports_folds(),
            has_injections = driver.supports_injections(),
            "Created TreeSitterDriver"
        );

        Some(Box::new(driver))
    }

    fn supported_languages(&self) -> Vec<&str> {
        // Note: This returns an owned Vec because we can't return references
        // to the HashMap keys through the RwLock. The trait requires &str slices,
        // so we need to collect and leak the strings (or use a different approach).
        // For now, we return an empty vec and recommend using language_ids() instead.
        //
        // TODO: Consider changing the trait to return Vec<String> or using
        // a more sophisticated approach with a cached list.
        Vec::new()
    }

    fn supports(&self, language_id: &str) -> bool {
        self.languages.read().unwrap().contains_key(language_id)
    }
}

// Additional methods that don't have lifetime issues
impl TreeSitterDriverFactory {
    /// Get a list of supported language IDs (owned strings).
    ///
    /// Use this instead of `supported_languages()` for a complete list.
    #[must_use]
    pub fn language_ids(&self) -> Vec<String> {
        self.languages.read().unwrap().keys().cloned().collect()
    }

    /// Get the file extensions for a language.
    ///
    /// Returns an empty vector if the language is not registered.
    #[must_use]
    pub fn extensions_for(&self, language_id: &str) -> Vec<&'static str> {
        self.languages
            .read()
            .unwrap()
            .get(language_id)
            .map(|lang| lang.extensions.clone())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_factory_new() {
        let factory = TreeSitterDriverFactory::new();
        assert_eq!(factory.language_count(), 0);
    }

    #[test]
    fn test_factory_extension_detection() {
        let factory = TreeSitterDriverFactory::new();

        // No languages registered
        assert!(factory.detect_language("test.rs").is_none());

        // Note: Full registration tests require actual tree-sitter grammars,
        // which are in language plugins. Integration tests will be added there.
    }

    #[test]
    fn test_factory_supports_empty() {
        let factory = TreeSitterDriverFactory::new();

        // No languages registered
        assert!(!factory.supports("rust"));
        assert!(!factory.supports("python"));
    }
}
