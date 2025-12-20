//! Language renderer registry
//!
//! Manages registration and lookup of language-specific renderers.

use std::collections::HashMap;

use crate::treesitter::LanguageId;

use super::types::LanguageRenderer;

/// Registry for language-specific renderers
///
/// Allows dynamic dispatch to the appropriate renderer based on language ID.
pub struct LanguageRendererRegistry {
    renderers: HashMap<LanguageId, Box<dyn LanguageRenderer>>,
}

impl Default for LanguageRendererRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl LanguageRendererRegistry {
    /// Create a new empty registry
    #[must_use]
    pub fn new() -> Self {
        Self {
            renderers: HashMap::new(),
        }
    }

    /// Register a language renderer
    ///
    /// If a renderer for this language already exists, it will be replaced.
    pub fn register(&mut self, renderer: Box<dyn LanguageRenderer>) {
        let id = renderer.language_id();
        self.renderers.insert(id, renderer);
    }

    /// Unregister a language renderer
    pub fn unregister(&mut self, id: LanguageId) -> Option<Box<dyn LanguageRenderer>> {
        self.renderers.remove(&id)
    }

    /// Get a renderer for a language (immutable)
    #[must_use]
    pub fn get(&self, id: LanguageId) -> Option<&dyn LanguageRenderer> {
        self.renderers.get(&id).map(AsRef::as_ref)
    }

    /// Get a renderer for a language (mutable)
    pub fn get_mut(&mut self, id: LanguageId) -> Option<&mut Box<dyn LanguageRenderer>> {
        self.renderers.get_mut(&id)
    }

    /// Check if a renderer is registered for a language
    #[must_use]
    pub fn has(&self, id: LanguageId) -> bool {
        self.renderers.contains_key(&id)
    }

    /// Get all registered language IDs
    #[must_use]
    pub fn languages(&self) -> Vec<LanguageId> {
        self.renderers.keys().copied().collect()
    }

    /// Check if registry is empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.renderers.is_empty()
    }

    /// Get number of registered renderers
    #[must_use]
    pub fn len(&self) -> usize {
        self.renderers.len()
    }

    /// Enable or disable a renderer
    pub fn set_enabled(&mut self, id: LanguageId, enabled: bool) {
        if let Some(renderer) = self.renderers.get_mut(&id) {
            renderer.set_enabled(enabled);
        }
    }

    /// Check if a renderer is enabled
    #[must_use]
    pub fn is_enabled(&self, id: LanguageId) -> bool {
        self.renderers.get(&id).is_some_and(|r| r.is_enabled())
    }
}

impl std::fmt::Debug for LanguageRendererRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LanguageRendererRegistry")
            .field("languages", &self.languages())
            .finish()
    }
}
