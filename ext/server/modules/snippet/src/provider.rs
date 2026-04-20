//! Snippet provider trait and registry (#136, #529).
//!
//! `SnippetProvider` abstracts snippet sources. `SnippetRegistry` aggregates
//! providers. `SnippetRegistryHandle` wraps the registry in an `RwLock` for
//! hot reload support.

use std::sync::{Arc, RwLock};

/// A snippet definition loaded from a provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnippetDefinition {
    /// Human-readable snippet name (e.g., "function").
    pub name: String,
    /// Trigger prefix (e.g., "fn").
    pub prefix: String,
    /// Raw `TextMate` snippet body (lazy-parsed on expansion).
    pub body_raw: String,
    /// Optional description.
    pub description: Option<String>,
    /// Optional language scope restriction (`VSCode` `.code-snippets` format).
    ///
    /// `None` means the snippet applies to all languages.
    /// `Some(vec!["rust", "toml"])` restricts to those filetypes only.
    pub scope: Option<Vec<String>>,
}

impl SnippetDefinition {
    /// Returns true if this snippet applies to the given filetype.
    ///
    /// A snippet with no scope applies to all filetypes.
    /// A snippet with a scope list applies only to listed filetypes.
    #[must_use]
    pub fn applies_to(&self, filetype: &str) -> bool {
        self.scope
            .as_ref()
            .is_none_or(|scopes| scopes.iter().any(|s| s == filetype))
    }
}

/// Trait for snippet sources.
///
/// Providers supply snippets filtered by filetype. Multiple providers
/// can be registered to aggregate snippets from different sources
/// (JSON files, LSP, custom plugins).
pub trait SnippetProvider: Send + Sync {
    /// Get all snippets available for a given filetype.
    fn snippets_for_filetype(&self, filetype: &str) -> Vec<&SnippetDefinition>;

    /// Find a snippet by its trigger prefix for a given filetype.
    fn snippet_by_prefix(&self, filetype: &str, prefix: &str) -> Option<&SnippetDefinition>;
}

/// Registry that aggregates multiple snippet providers.
///
/// Providers are queried in registration order. The first match wins
/// for `find_by_prefix`.
pub struct SnippetRegistry {
    providers: Vec<Box<dyn SnippetProvider>>,
}

impl SnippetRegistry {
    /// Create an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    /// Register a snippet provider.
    pub fn register(&mut self, provider: Box<dyn SnippetProvider>) {
        self.providers.push(provider);
    }

    /// Find a snippet by trigger prefix across all providers.
    ///
    /// Walks the inheritance chain (e.g. `typescriptreact → typescript → javascript → global`)
    /// and returns the first matching snippet from the first provider that has a match.
    #[must_use]
    pub fn find_by_prefix(&self, filetype: &str, prefix: &str) -> Option<&SnippetDefinition> {
        let chain = crate::inheritance::resolution_chain(filetype);
        for ft in &chain {
            for provider in &self.providers {
                if let Some(def) = provider.snippet_by_prefix(ft, prefix)
                    && def.applies_to(filetype)
                {
                    return Some(def);
                }
            }
        }
        None
    }

    /// Get all snippets for a filetype from all providers.
    ///
    /// Walks the inheritance chain and includes all matching snippets.
    /// Filters by scope if present.
    #[must_use]
    pub fn all_for_filetype(&self, filetype: &str) -> Vec<&SnippetDefinition> {
        let mut result = Vec::new();
        let chain = crate::inheritance::resolution_chain(filetype);
        for ft in &chain {
            for provider in &self.providers {
                result.extend(
                    provider
                        .snippets_for_filetype(ft)
                        .into_iter()
                        .filter(|def| def.applies_to(filetype)),
                );
            }
        }
        result
    }

    /// Get the number of registered providers.
    #[must_use]
    pub fn provider_count(&self) -> usize {
        self.providers.len()
    }
}

impl Default for SnippetRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Thread-safe handle to a swappable snippet registry.
///
/// Wraps `SnippetRegistry` in `RwLock` so the registry can be atomically
/// replaced during hot reload while commands continue to read concurrently.
///
/// Returns cloned `SnippetDefinition` values to release the lock promptly.
#[derive(Clone)]
pub struct SnippetRegistryHandle(Arc<RwLock<SnippetRegistry>>);

impl SnippetRegistryHandle {
    /// Create a new handle wrapping a registry.
    #[must_use]
    pub fn new(registry: SnippetRegistry) -> Self {
        Self(Arc::new(RwLock::new(registry)))
    }

    /// Find a snippet by trigger prefix across all providers.
    ///
    /// Returns a cloned definition (lock is released before returning).
    #[must_use]
    pub fn find_by_prefix(&self, filetype: &str, prefix: &str) -> Option<SnippetDefinition> {
        let registry = self.0.read().ok()?;
        registry.find_by_prefix(filetype, prefix).cloned()
    }

    /// Get all snippets for a filetype from all providers.
    ///
    /// Returns cloned definitions (lock is released before returning).
    #[must_use]
    pub fn all_for_filetype(&self, filetype: &str) -> Vec<SnippetDefinition> {
        self.0
            .read()
            .ok()
            .map(|r| r.all_for_filetype(filetype).into_iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Replace the inner registry atomically (hot reload).
    pub fn replace(&self, new: SnippetRegistry) {
        if let Ok(mut registry) = self.0.write() {
            *registry = new;
        }
    }
}

#[cfg(test)]
#[path = "provider_tests.rs"]
mod tests;
