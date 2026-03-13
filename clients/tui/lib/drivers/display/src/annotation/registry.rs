//! Presenter registry for annotation rendering.
//!
//! The registry provides lookup of presenters by annotation kind.
//! It supports pattern-based matching for flexible presenter selection.

use std::sync::Arc;

use super::{
    AnnotationKind,
    presenter::{AnnotationPresenter, KindPattern},
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
    #[cfg_attr(coverage_nightly, coverage(off))]
    const fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<PresenterRegistry>();
};

#[cfg(test)]
#[path = "registry_tests.rs"]
mod tests;
