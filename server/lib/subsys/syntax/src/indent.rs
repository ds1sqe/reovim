//! Indent configuration types and store for language-aware indent guides.
//!
//! This module provides [`IndentConfig`] and [`IndentConfigStore`], following
//! the same self-registration pattern as [`BracketConfigStore`](crate::BracketConfigStore):
//!
//! - Language modules register per-language indent configs during `init()`
//! - Consumer modules (e.g., `indent-guide`) read configs at runtime
//!
//! # Example
//!
//! ```
//! use reovim_subsys_syntax::indent::{IndentConfig, IndentConfigStore};
//!
//! let store = IndentConfigStore::new();
//!
//! // Language module registers during init()
//! store.add(IndentConfig::new("rust").with_tab_size(4).with_use_tabs(false));
//!
//! // Consumer module reads at runtime
//! let config = store.find("rust").expect("rust config registered");
//! assert_eq!(config.tab_size(), Some(4));
//! ```

use std::sync::Arc;

use {parking_lot::RwLock, reovim_kernel::api::v1::Service};

/// Per-language indent configuration.
///
/// Declares tab size and tab/space preference for a language.
/// Language modules register this during `init()`. The indent-guide module
/// reads it at runtime for the current buffer's language.
#[derive(Debug, Clone)]
pub struct IndentConfig {
    /// Language ID this config applies to (e.g., "rust", "python").
    language_id: Arc<str>,
    /// Preferred tab size (number of spaces per indent level).
    tab_size: Option<u32>,
    /// Whether to use tabs instead of spaces.
    use_tabs: Option<bool>,
}

impl IndentConfig {
    /// Create a new indent config for a language.
    #[must_use]
    pub fn new(language_id: impl Into<Arc<str>>) -> Self {
        Self {
            language_id: language_id.into(),
            tab_size: None,
            use_tabs: None,
        }
    }

    /// Set the preferred tab size.
    #[must_use]
    pub const fn with_tab_size(mut self, size: u32) -> Self {
        self.tab_size = Some(size);
        self
    }

    /// Set whether to use tabs instead of spaces.
    #[must_use]
    pub const fn with_use_tabs(mut self, use_tabs: bool) -> Self {
        self.use_tabs = Some(use_tabs);
        self
    }

    /// Get the language ID.
    #[must_use]
    pub fn language_id(&self) -> &str {
        &self.language_id
    }

    /// Get the preferred tab size, if set.
    #[must_use]
    pub const fn tab_size(&self) -> Option<u32> {
        self.tab_size
    }

    /// Get whether to use tabs, if set.
    #[must_use]
    pub const fn use_tabs(&self) -> Option<bool> {
        self.use_tabs
    }

    /// Get effective tab size, using provided fallback if not set.
    #[must_use]
    pub const fn effective_tab_size(&self, fallback: u32) -> u32 {
        match self.tab_size {
            Some(size) => size,
            None => fallback,
        }
    }
}

/// Default indent config for languages without explicit registration.
///
/// Uses 4-space indentation with spaces.
#[must_use]
pub fn default_indent_config() -> IndentConfig {
    IndentConfig::new("*").with_tab_size(4).with_use_tabs(false)
}

/// Store for indent configs registered by language modules during init.
///
/// Follows the same pattern as [`BracketConfigStore`](crate::BracketConfigStore).
#[derive(Default)]
pub struct IndentConfigStore {
    entries: RwLock<Vec<IndentConfig>>,
}

impl IndentConfigStore {
    /// Create a new empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an indent config to the store.
    ///
    /// Called by language modules during `init()`.
    pub fn add(&self, config: IndentConfig) {
        self.entries.write().push(config);
    }

    /// Find an indent config by language ID.
    #[must_use]
    pub fn find(&self, language_id: &str) -> Option<IndentConfig> {
        self.entries
            .read()
            .iter()
            .find(|c| &*c.language_id == language_id)
            .cloned()
    }

    /// Find an indent config by language ID, falling back to the default.
    #[must_use]
    pub fn find_or_default(&self, language_id: &str) -> IndentConfig {
        self.find(language_id).unwrap_or_else(default_indent_config)
    }

    /// Get the number of registered configs.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.read().len()
    }

    /// Check if no configs are registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.read().is_empty()
    }
}

impl Service for IndentConfigStore {}

impl std::fmt::Debug for IndentConfigStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IndentConfigStore")
            .field("count", &self.len())
            .finish()
    }
}

#[cfg(test)]
#[path = "indent_tests.rs"]
mod tests;
