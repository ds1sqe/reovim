//! Language registry for dynamic language registration
//!
//! Language plugins implement the `LanguageSupport` trait and register
//! themselves with the treesitter plugin at runtime.

use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, RwLock},
};

use tree_sitter::Language;

/// Trait for language plugins to implement
///
/// Language plugins provide the tree-sitter grammar and query files
/// for a specific language. They register themselves with the treesitter
/// plugin using the `RegisterLanguage` event.
pub trait LanguageSupport: Send + Sync + 'static {
    /// Unique identifier for this language (e.g., "rust", "python")
    fn language_id(&self) -> &'static str;

    /// File extensions that map to this language (e.g., ["rs"] for Rust)
    fn file_extensions(&self) -> &'static [&'static str];

    /// The tree-sitter Language for this grammar
    fn tree_sitter_language(&self) -> Language;

    /// Highlight query (required)
    fn highlights_query(&self) -> &'static str;

    /// Fold query (optional)
    fn folds_query(&self) -> Option<&'static str> {
        None
    }

    /// Text objects query (optional)
    fn textobjects_query(&self) -> Option<&'static str> {
        None
    }

    /// Decorations query (optional, used by markdown)
    fn decorations_query(&self) -> Option<&'static str> {
        None
    }

    /// Injections query for embedded languages (optional)
    fn injections_query(&self) -> Option<&'static str> {
        None
    }
}

/// A registered language with all its data
pub struct RegisteredLanguage {
    /// The language support implementation
    pub support: Arc<dyn LanguageSupport>,
    /// Compiled tree-sitter Language (cached)
    language: Language,
}

impl RegisteredLanguage {
    /// Create a new registered language
    pub fn new(support: Arc<dyn LanguageSupport>) -> Self {
        let language = support.tree_sitter_language();
        Self { support, language }
    }

    /// Get the language ID
    pub fn language_id(&self) -> &'static str {
        self.support.language_id()
    }

    /// Get the tree-sitter Language
    pub fn language(&self) -> &Language {
        &self.language
    }

    /// Get file extensions
    pub fn file_extensions(&self) -> &'static [&'static str] {
        self.support.file_extensions()
    }

    /// Get highlights query
    pub fn highlights_query(&self) -> &'static str {
        self.support.highlights_query()
    }

    /// Get folds query
    pub fn folds_query(&self) -> Option<&'static str> {
        self.support.folds_query()
    }

    /// Get text objects query
    pub fn textobjects_query(&self) -> Option<&'static str> {
        self.support.textobjects_query()
    }

    /// Get decorations query
    pub fn decorations_query(&self) -> Option<&'static str> {
        self.support.decorations_query()
    }

    /// Get injections query
    pub fn injections_query(&self) -> Option<&'static str> {
        self.support.injections_query()
    }
}

/// Registry for dynamically registered languages
///
/// Language plugins register themselves at startup, and the treesitter
/// plugin uses this registry to detect languages and get their grammars.
pub struct LanguageRegistry {
    /// Registered languages by ID
    languages: RwLock<HashMap<String, Arc<RegisteredLanguage>>>,
    /// Extension to language ID mapping
    extension_map: RwLock<HashMap<String, String>>,
}

impl Default for LanguageRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl LanguageRegistry {
    /// Create a new empty language registry
    #[must_use]
    pub fn new() -> Self {
        Self {
            languages: RwLock::new(HashMap::new()),
            extension_map: RwLock::new(HashMap::new()),
        }
    }

    /// Register a language support implementation
    pub fn register(&self, support: Arc<dyn LanguageSupport>) {
        let id = support.language_id().to_string();
        let extensions = support.file_extensions();

        let registered = Arc::new(RegisteredLanguage::new(support));

        // Register in language map
        self.languages
            .write()
            .unwrap()
            .insert(id.clone(), registered);

        // Register extensions
        let mut ext_map = self.extension_map.write().unwrap();
        for ext in extensions {
            ext_map.insert((*ext).to_string(), id.clone());
        }

        tracing::debug!(language_id = %id, extensions = ?extensions, "Registered language");
    }

    /// Detect language from file path based on extension
    #[must_use]
    pub fn detect_language(&self, path: &str) -> Option<String> {
        Path::new(path)
            .extension()
            .and_then(|e| e.to_str())
            .and_then(|ext| self.extension_map.read().unwrap().get(ext).cloned())
    }

    /// Get a registered language by ID
    #[must_use]
    pub fn get(&self, id: &str) -> Option<Arc<RegisteredLanguage>> {
        self.languages.read().unwrap().get(id).cloned()
    }

    /// Get the tree-sitter Language for a language ID
    #[must_use]
    pub fn get_language(&self, id: &str) -> Option<Language> {
        self.languages
            .read()
            .unwrap()
            .get(id)
            .map(|l| l.language().clone())
    }

    /// Check if a language is registered
    #[must_use]
    pub fn is_registered(&self, id: &str) -> bool {
        self.languages.read().unwrap().contains_key(id)
    }

    /// Get all registered language IDs
    #[must_use]
    pub fn language_ids(&self) -> Vec<String> {
        self.languages.read().unwrap().keys().cloned().collect()
    }
}
