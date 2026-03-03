//! Snippet provider trait and registry (#136).
//!
//! `SnippetProvider` abstracts snippet sources. Phase 1: JSON files.
//! Future: LSP provider, completion integration, custom providers.

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
    /// Returns the first matching snippet from the first provider
    /// that has a match.
    #[must_use]
    pub fn find_by_prefix(&self, filetype: &str, prefix: &str) -> Option<&SnippetDefinition> {
        for provider in &self.providers {
            if let Some(def) = provider.snippet_by_prefix(filetype, prefix) {
                return Some(def);
            }
        }
        None
    }

    /// Get all snippets for a filetype from all providers.
    #[must_use]
    pub fn all_for_filetype(&self, filetype: &str) -> Vec<&SnippetDefinition> {
        let mut result = Vec::new();
        for provider in &self.providers {
            result.extend(provider.snippets_for_filetype(filetype));
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

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // SnippetDefinition
    // =========================================================================

    #[test]
    fn test_snippet_definition_clone() {
        let def = SnippetDefinition {
            name: "test".to_string(),
            prefix: "t".to_string(),
            body_raw: "$1".to_string(),
            description: Some("A test snippet".to_string()),
        };
        let cloned = def.clone();
        assert_eq!(def, cloned);
    }

    #[test]
    fn test_snippet_definition_debug() {
        let def = SnippetDefinition {
            name: "test".to_string(),
            prefix: "t".to_string(),
            body_raw: "$1".to_string(),
            description: None,
        };
        let debug = format!("{def:?}");
        assert!(debug.contains("SnippetDefinition"));
    }

    #[test]
    fn test_snippet_definition_no_description() {
        let def = SnippetDefinition {
            name: "bare".to_string(),
            prefix: "b".to_string(),
            body_raw: "body".to_string(),
            description: None,
        };
        assert!(def.description.is_none());
    }

    // =========================================================================
    // Mock provider for testing
    // =========================================================================

    struct MockProvider {
        snippets: Vec<(String, SnippetDefinition)>, // (filetype, def)
    }

    impl MockProvider {
        fn new(snippets: Vec<(String, SnippetDefinition)>) -> Self {
            Self { snippets }
        }
    }

    impl SnippetProvider for MockProvider {
        fn snippets_for_filetype(&self, filetype: &str) -> Vec<&SnippetDefinition> {
            self.snippets
                .iter()
                .filter(|(ft, _)| ft == filetype)
                .map(|(_, def)| def)
                .collect()
        }

        fn snippet_by_prefix(&self, filetype: &str, prefix: &str) -> Option<&SnippetDefinition> {
            self.snippets
                .iter()
                .find(|(ft, def)| ft == filetype && def.prefix == prefix)
                .map(|(_, def)| def)
        }
    }

    fn make_def(name: &str, prefix: &str, body: &str) -> SnippetDefinition {
        SnippetDefinition {
            name: name.to_string(),
            prefix: prefix.to_string(),
            body_raw: body.to_string(),
            description: None,
        }
    }

    // =========================================================================
    // SnippetRegistry
    // =========================================================================

    #[test]
    fn test_registry_new_empty() {
        let registry = SnippetRegistry::new();
        assert_eq!(registry.provider_count(), 0);
    }

    #[test]
    fn test_registry_default() {
        let registry = SnippetRegistry::default();
        assert_eq!(registry.provider_count(), 0);
    }

    #[test]
    fn test_registry_register() {
        let mut registry = SnippetRegistry::new();
        registry.register(Box::new(MockProvider::new(vec![])));
        assert_eq!(registry.provider_count(), 1);
    }

    #[test]
    fn test_registry_find_by_prefix() {
        let mut registry = SnippetRegistry::new();
        registry.register(Box::new(MockProvider::new(vec![(
            "rust".to_string(),
            make_def("function", "fn", "fn $1() {}"),
        )])));

        let found = registry.find_by_prefix("rust", "fn");
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "function");
    }

    #[test]
    fn test_registry_find_by_prefix_not_found() {
        let mut registry = SnippetRegistry::new();
        registry.register(Box::new(MockProvider::new(vec![(
            "rust".to_string(),
            make_def("function", "fn", "fn $1() {}"),
        )])));

        assert!(registry.find_by_prefix("rust", "xyz").is_none());
    }

    #[test]
    fn test_registry_find_by_prefix_wrong_filetype() {
        let mut registry = SnippetRegistry::new();
        registry.register(Box::new(MockProvider::new(vec![(
            "rust".to_string(),
            make_def("function", "fn", "fn $1() {}"),
        )])));

        assert!(registry.find_by_prefix("python", "fn").is_none());
    }

    #[test]
    fn test_registry_find_by_prefix_first_provider_wins() {
        let mut registry = SnippetRegistry::new();
        registry.register(Box::new(MockProvider::new(vec![(
            "rust".to_string(),
            make_def("first", "fn", "first body"),
        )])));
        registry.register(Box::new(MockProvider::new(vec![(
            "rust".to_string(),
            make_def("second", "fn", "second body"),
        )])));

        let found = registry.find_by_prefix("rust", "fn").unwrap();
        assert_eq!(found.name, "first");
    }

    #[test]
    fn test_registry_all_for_filetype() {
        let mut registry = SnippetRegistry::new();
        registry.register(Box::new(MockProvider::new(vec![
            ("rust".to_string(), make_def("function", "fn", "fn $1()")),
            ("rust".to_string(), make_def("struct", "st", "struct $1 {}")),
            ("python".to_string(), make_def("def", "def", "def $1(): pass")),
        ])));

        let rust_snippets = registry.all_for_filetype("rust");
        assert_eq!(rust_snippets.len(), 2);
    }

    #[test]
    fn test_registry_all_for_filetype_aggregates_providers() {
        let mut registry = SnippetRegistry::new();
        registry.register(Box::new(MockProvider::new(vec![(
            "rust".to_string(),
            make_def("fn", "fn", "fn $1()"),
        )])));
        registry.register(Box::new(MockProvider::new(vec![(
            "rust".to_string(),
            make_def("struct", "st", "struct $1 {}"),
        )])));

        let all = registry.all_for_filetype("rust");
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn test_registry_all_for_filetype_empty() {
        let registry = SnippetRegistry::new();
        assert!(registry.all_for_filetype("rust").is_empty());
    }

    #[test]
    fn test_registry_find_empty() {
        let registry = SnippetRegistry::new();
        assert!(registry.find_by_prefix("rust", "fn").is_none());
    }
}
