//! Presenter registry for annotation rendering.
//!
//! The registry provides lookup of presenters by annotation kind.
//! It supports pattern-based matching for flexible presenter selection.

use std::sync::Arc;

use super::{
    presenter::{AnnotationPresenter, KindPattern},
    types::AnnotationKind,
};

/// Entry in the presenter registry.
#[derive(Clone)]
struct RegistryEntry {
    /// The presenter.
    presenter: Arc<dyn AnnotationPresenter>,
    /// Pattern for matching (cached from presenter).
    pattern: KindPattern,
}

/// Registry for annotation presenters.
///
/// Manages presenters and provides lookup by annotation kind.
/// Lookup uses first-match semantics: the first registered presenter
/// whose pattern matches the kind is returned.
///
/// # Registration Order
///
/// More specific patterns should be registered before general patterns.
/// For example:
/// 1. `KindPattern::exact("diagnostic.error")` - Most specific
/// 2. `KindPattern::prefix("diagnostic")` - Less specific
/// 3. `KindPattern::All` - Least specific (catch-all)
///
/// # Example
///
/// ```ignore
/// let mut registry = PresenterRegistry::new();
///
/// // Register a presenter for line numbers
/// registry.register(Arc::new(LineNumberPresenter::new()));
///
/// // Find presenter for a kind
/// let kind = AnnotationKind::new("line_number");
/// if let Some(presenter) = registry.find(&kind) {
///     let output = presenter.present(&annotation, &ctx);
/// }
/// ```
#[derive(Default)]
pub struct PresenterRegistry {
    /// Registered presenters (order matters for lookup).
    entries: Vec<RegistryEntry>,
}

impl PresenterRegistry {
    /// Create a new empty registry.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Register a presenter.
    ///
    /// Presenters are matched in registration order (first match wins).
    /// Register more specific patterns before general ones.
    pub fn register(&mut self, presenter: Arc<dyn AnnotationPresenter>) {
        let pattern = presenter.handles();
        self.entries.push(RegistryEntry { presenter, pattern });
    }

    /// Find a presenter that handles the given kind.
    ///
    /// Returns the first presenter whose pattern matches the kind,
    /// or `None` if no presenter matches.
    #[must_use]
    pub fn find(&self, kind: &AnnotationKind) -> Option<Arc<dyn AnnotationPresenter>> {
        self.entries
            .iter()
            .find(|entry| entry.pattern.matches(kind))
            .map(|entry| Arc::clone(&entry.presenter))
    }

    /// Find a presenter by ID.
    #[must_use]
    pub fn find_by_id(&self, id: &str) -> Option<Arc<dyn AnnotationPresenter>> {
        self.entries
            .iter()
            .find(|entry| entry.presenter.id() == id)
            .map(|entry| Arc::clone(&entry.presenter))
    }

    /// Check if a presenter is registered for the given kind.
    #[must_use]
    pub fn has_presenter(&self, kind: &AnnotationKind) -> bool {
        self.entries.iter().any(|entry| entry.pattern.matches(kind))
    }

    /// Get the number of registered presenters.
    ///
    /// Note: Cannot be const because `Vec::len()` is not const.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the registry is empty.
    ///
    /// Note: Cannot be const because `Vec::is_empty()` is not const.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get all registered presenter IDs.
    pub fn presenter_ids(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.entries.iter().map(|entry| entry.presenter.id())
    }

    /// Remove a presenter by ID.
    ///
    /// Returns `true` if a presenter was removed.
    pub fn unregister(&mut self, id: &str) -> bool {
        let len_before = self.entries.len();
        self.entries.retain(|entry| entry.presenter.id() != id);
        self.entries.len() < len_before
    }

    /// Clear all registered presenters.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

impl std::fmt::Debug for PresenterRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PresenterRegistry")
            .field("count", &self.entries.len())
            .field(
                "presenters",
                &self
                    .entries
                    .iter()
                    .map(|e| e.presenter.id())
                    .collect::<Vec<_>>(),
            )
            .finish()
    }
}

// Ensure registry is Send + Sync
const _: () = {
    const fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<PresenterRegistry>();
};

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{annotation::Annotation, highlight::Style},
    };

    use super::super::presenter::{ColumnWidth, PresentedOutput, PresenterContext};

    // Mock presenters for testing
    struct ExactPresenter {
        id: &'static str,
        kind: String,
    }

    impl ExactPresenter {
        fn new(id: &'static str, kind: &str) -> Self {
            Self {
                id,
                kind: kind.to_string(),
            }
        }
    }

    impl AnnotationPresenter for ExactPresenter {
        fn id(&self) -> &'static str {
            self.id
        }

        fn handles(&self) -> KindPattern {
            KindPattern::exact(&self.kind)
        }

        fn present(&self, _: &Annotation, _: &PresenterContext) -> PresentedOutput {
            PresentedOutput::cell('E', Style::default())
        }

        fn column_width(&self, _: &PresenterContext) -> ColumnWidth {
            ColumnWidth::fixed(1)
        }
    }

    struct PrefixPresenter {
        id: &'static str,
        prefix: String,
    }

    impl PrefixPresenter {
        fn new(id: &'static str, prefix: &str) -> Self {
            Self {
                id,
                prefix: prefix.to_string(),
            }
        }
    }

    impl AnnotationPresenter for PrefixPresenter {
        fn id(&self) -> &'static str {
            self.id
        }

        fn handles(&self) -> KindPattern {
            KindPattern::prefix(&self.prefix)
        }

        fn present(&self, _: &Annotation, _: &PresenterContext) -> PresentedOutput {
            PresentedOutput::cell('P', Style::default())
        }

        fn column_width(&self, _: &PresenterContext) -> ColumnWidth {
            ColumnWidth::fixed(1)
        }
    }

    struct CatchAllPresenter;

    impl AnnotationPresenter for CatchAllPresenter {
        fn id(&self) -> &'static str {
            "catch-all"
        }

        fn handles(&self) -> KindPattern {
            KindPattern::All
        }

        fn present(&self, _: &Annotation, _: &PresenterContext) -> PresentedOutput {
            PresentedOutput::cell('*', Style::default())
        }

        fn column_width(&self, _: &PresenterContext) -> ColumnWidth {
            ColumnWidth::fixed(1)
        }
    }

    #[test]
    fn test_registry_new() {
        let registry = PresenterRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn test_registry_register() {
        let mut registry = PresenterRegistry::new();
        registry.register(Arc::new(ExactPresenter::new("test", "line_number")));
        assert_eq!(registry.len(), 1);
        assert!(!registry.is_empty());
    }

    #[test]
    fn test_registry_find_exact() {
        let mut registry = PresenterRegistry::new();
        registry.register(Arc::new(ExactPresenter::new("line", "line_number")));

        let kind = AnnotationKind::new("line_number");
        let presenter = registry.find(&kind);
        assert!(presenter.is_some());
        assert_eq!(presenter.unwrap().id(), "line");
    }

    #[test]
    fn test_registry_find_prefix() {
        let mut registry = PresenterRegistry::new();
        registry.register(Arc::new(PrefixPresenter::new("diag", "diagnostic")));

        let kind = AnnotationKind::new("diagnostic.error");
        let presenter = registry.find(&kind);
        assert!(presenter.is_some());
        assert_eq!(presenter.unwrap().id(), "diag");
    }

    #[test]
    fn test_registry_find_not_found() {
        let mut registry = PresenterRegistry::new();
        registry.register(Arc::new(ExactPresenter::new("line", "line_number")));

        let kind = AnnotationKind::new("diagnostic.error");
        let presenter = registry.find(&kind);
        assert!(presenter.is_none());
    }

    #[test]
    fn test_registry_first_match_wins() {
        let mut registry = PresenterRegistry::new();

        // Register exact match first
        registry.register(Arc::new(ExactPresenter::new("exact", "diagnostic.error")));
        // Then prefix match
        registry.register(Arc::new(PrefixPresenter::new("prefix", "diagnostic")));
        // Then catch-all
        registry.register(Arc::new(CatchAllPresenter));

        // Exact should win for diagnostic.error
        let kind = AnnotationKind::new("diagnostic.error");
        let presenter = registry.find(&kind).unwrap();
        assert_eq!(presenter.id(), "exact");

        // Prefix should win for diagnostic.warning (no exact match)
        let kind = AnnotationKind::new("diagnostic.warning");
        let presenter = registry.find(&kind).unwrap();
        assert_eq!(presenter.id(), "prefix");

        // Catch-all should win for unknown kinds
        let kind = AnnotationKind::new("unknown");
        let presenter = registry.find(&kind).unwrap();
        assert_eq!(presenter.id(), "catch-all");
    }

    #[test]
    fn test_registry_find_by_id() {
        let mut registry = PresenterRegistry::new();
        registry.register(Arc::new(ExactPresenter::new("test1", "kind1")));
        registry.register(Arc::new(ExactPresenter::new("test2", "kind2")));

        let presenter = registry.find_by_id("test1");
        assert!(presenter.is_some());
        assert_eq!(presenter.unwrap().id(), "test1");

        let presenter = registry.find_by_id("nonexistent");
        assert!(presenter.is_none());
    }

    #[test]
    fn test_registry_has_presenter() {
        let mut registry = PresenterRegistry::new();
        registry.register(Arc::new(ExactPresenter::new("line", "line_number")));

        assert!(registry.has_presenter(&AnnotationKind::new("line_number")));
        assert!(!registry.has_presenter(&AnnotationKind::new("diagnostic")));
    }

    #[test]
    fn test_registry_presenter_ids() {
        let mut registry = PresenterRegistry::new();
        registry.register(Arc::new(ExactPresenter::new("a", "kind_a")));
        registry.register(Arc::new(ExactPresenter::new("b", "kind_b")));

        let ids: Vec<_> = registry.presenter_ids().collect();
        assert_eq!(ids, vec!["a", "b"]);
    }

    #[test]
    fn test_registry_unregister() {
        let mut registry = PresenterRegistry::new();
        registry.register(Arc::new(ExactPresenter::new("a", "kind_a")));
        registry.register(Arc::new(ExactPresenter::new("b", "kind_b")));

        assert!(registry.unregister("a"));
        assert_eq!(registry.len(), 1);
        assert!(registry.find_by_id("a").is_none());
        assert!(registry.find_by_id("b").is_some());
    }

    #[test]
    fn test_registry_unregister_nonexistent() {
        let mut registry = PresenterRegistry::new();
        assert!(!registry.unregister("nonexistent"));
    }

    #[test]
    fn test_registry_clear() {
        let mut registry = PresenterRegistry::new();
        registry.register(Arc::new(ExactPresenter::new("a", "kind_a")));
        registry.register(Arc::new(ExactPresenter::new("b", "kind_b")));

        registry.clear();
        assert!(registry.is_empty());
    }

    #[test]
    fn test_registry_debug() {
        let mut registry = PresenterRegistry::new();
        registry.register(Arc::new(ExactPresenter::new("test", "kind")));

        let debug = format!("{registry:?}");
        assert!(debug.contains("PresenterRegistry"));
        assert!(debug.contains("test"));
    }

    #[test]
    fn test_registry_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<PresenterRegistry>();
    }
}
