//! Bracket configuration types and store for language-aware bracket pairing.
//!
//! This module provides [`BracketConfig`] and [`BracketConfigStore`], following
//! the same self-registration pattern as [`LanguageInfoStore`](crate::LanguageInfoStore):
//!
//! - Language modules register per-language bracket configs during `init()`
//! - Consumer modules (e.g., `pair`) read configs at runtime
//!
//! # Example
//!
//! ```
//! use reovim_driver_text_syntax::bracket::{BracketConfig, BracketConfigStore, BracketPair};
//!
//! let store = BracketConfigStore::new();
//!
//! // Language module registers during init()
//! store.add(
//!     BracketConfig::new("rust")
//!         .with_rainbow([('(', ')'), ('[', ']'), ('{', '}')])
//!         .with_autopair([('(', ')'), ('[', ']'), ('{', '}'), ('"', '"')])
//!         .with_highlight([('(', ')'), ('[', ']'), ('{', '}')])
//! );
//!
//! // Consumer module reads at runtime
//! let config = store.find("rust").expect("rust config registered");
//! assert_eq!(config.rainbow_pairs().len(), 3);
//! ```

use std::sync::Arc;

use {parking_lot::RwLock, reovim_kernel::api::v1::Service};

/// A bracket pair: opening and closing characters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BracketPair {
    /// The opening character (e.g., `(`).
    pub open: char,
    /// The closing character (e.g., `)`).
    pub close: char,
}

impl BracketPair {
    /// Create a new bracket pair.
    #[must_use]
    pub const fn new(open: char, close: char) -> Self {
        Self { open, close }
    }

    /// Whether this is a symmetric pair (open == close, e.g., quotes).
    #[must_use]
    pub const fn is_symmetric(&self) -> bool {
        self.open as u32 == self.close as u32
    }
}

/// Per-language bracket configuration.
///
/// Declares which bracket pairs are used for rainbow coloring,
/// auto-pair insertion, and matched-pair highlighting.
///
/// Language modules register this during `init()`. The pair module
/// reads it at runtime for the current buffer's language.
#[derive(Debug, Clone)]
pub struct BracketConfig {
    /// Language ID this config applies to (e.g., "rust", "markdown").
    language_id: Arc<str>,
    /// Pairs used for rainbow depth coloring.
    rainbow_pairs: Vec<BracketPair>,
    /// Pairs used for auto-pair insertion (includes quotes).
    autopair_pairs: Vec<BracketPair>,
    /// Pairs used for matched-pair highlighting.
    highlight_pairs: Vec<BracketPair>,
}

impl BracketConfig {
    /// Create a new bracket config for a language.
    #[must_use]
    pub fn new(language_id: impl Into<Arc<str>>) -> Self {
        Self {
            language_id: language_id.into(),
            rainbow_pairs: Vec::new(),
            autopair_pairs: Vec::new(),
            highlight_pairs: Vec::new(),
        }
    }

    /// Set pairs used for rainbow depth coloring.
    #[must_use]
    pub fn with_rainbow(mut self, pairs: impl IntoIterator<Item = (char, char)>) -> Self {
        self.rainbow_pairs = pairs
            .into_iter()
            .map(|(o, c)| BracketPair::new(o, c))
            .collect();
        self
    }

    /// Set pairs used for auto-pair insertion.
    #[must_use]
    pub fn with_autopair(mut self, pairs: impl IntoIterator<Item = (char, char)>) -> Self {
        self.autopair_pairs = pairs
            .into_iter()
            .map(|(o, c)| BracketPair::new(o, c))
            .collect();
        self
    }

    /// Set pairs used for matched-pair highlighting.
    #[must_use]
    pub fn with_highlight(mut self, pairs: impl IntoIterator<Item = (char, char)>) -> Self {
        self.highlight_pairs = pairs
            .into_iter()
            .map(|(o, c)| BracketPair::new(o, c))
            .collect();
        self
    }

    /// Get the language ID.
    #[must_use]
    pub fn language_id(&self) -> &str {
        &self.language_id
    }

    /// Get pairs for rainbow coloring.
    #[must_use]
    pub fn rainbow_pairs(&self) -> &[BracketPair] {
        &self.rainbow_pairs
    }

    /// Get pairs for auto-pair insertion.
    #[must_use]
    pub fn autopair_pairs(&self) -> &[BracketPair] {
        &self.autopair_pairs
    }

    /// Get pairs for matched-pair highlighting.
    #[must_use]
    pub fn highlight_pairs(&self) -> &[BracketPair] {
        &self.highlight_pairs
    }
}

/// Default bracket config for languages without explicit registration.
///
/// Uses the common set: `()[]{}` for rainbow/highlight, `()[]{}""''` for autopair.
#[must_use]
pub fn default_bracket_config() -> BracketConfig {
    BracketConfig::new("*")
        .with_rainbow([('(', ')'), ('[', ']'), ('{', '}')])
        .with_autopair([('(', ')'), ('[', ']'), ('{', '}'), ('"', '"'), ('\'', '\'')])
        .with_highlight([('(', ')'), ('[', ']'), ('{', '}')])
}

/// Store for bracket configs registered by language modules during init.
///
/// Follows the same pattern as [`LanguageInfoStore`](crate::LanguageInfoStore).
#[derive(Default)]
pub struct BracketConfigStore {
    entries: RwLock<Vec<BracketConfig>>,
}

impl BracketConfigStore {
    /// Create a new empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a bracket config to the store.
    ///
    /// Called by language modules during `init()`.
    pub fn add(&self, config: BracketConfig) {
        self.entries.write().push(config);
    }

    /// Find a bracket config by language ID.
    #[must_use]
    pub fn find(&self, language_id: &str) -> Option<BracketConfig> {
        self.entries
            .read()
            .iter()
            .find(|c| &*c.language_id == language_id)
            .cloned()
    }

    /// Find a bracket config by language ID, falling back to the default.
    #[must_use]
    pub fn find_or_default(&self, language_id: &str) -> BracketConfig {
        self.find(language_id)
            .unwrap_or_else(default_bracket_config)
    }

    /// Take all registered configs.
    ///
    /// This drains the store; subsequent calls return an empty vec.
    pub fn take_all(&self) -> Vec<BracketConfig> {
        std::mem::take(&mut *self.entries.write())
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

impl Service for BracketConfigStore {}

impl std::fmt::Debug for BracketConfigStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BracketConfigStore")
            .field("count", &self.len())
            .finish()
    }
}

#[cfg(test)]
#[path = "bracket_tests.rs"]
mod tests;
