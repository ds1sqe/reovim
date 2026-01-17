//! Resolver registry for managing mode key resolvers.
//!
//! The registry maps mode IDs to their resolvers and provides lookup
//! functionality for the event loop.

use std::{collections::HashMap, sync::Arc};

use {
    reovim_driver_input::{KeyEvent, ModeKeyResolver, ModeState, ResolveResult},
    reovim_kernel::api::v1::ModeId,
};

/// Registry for mode key resolvers.
///
/// Maps mode IDs to resolver implementations. The event loop uses this
/// to find the appropriate resolver for the current mode.
///
/// # Thread Safety
///
/// Resolvers are stored as `Arc<dyn ModeKeyResolver>` to allow shared
/// access from multiple threads (e.g., during async command execution).
///
/// # Example
///
/// ```ignore
/// use editor::resolver::{ResolverRegistry, VimNormalResolver, VimInsertResolver};
///
/// let mut registry = ResolverRegistry::new();
///
/// // Register Vim resolvers
/// registry.register(VimNormalResolver::new());
/// registry.register(VimInsertResolver::new());
///
/// // Look up resolver for current mode
/// if let Some(resolver) = registry.get(&current_mode) {
///     let result = resolver.resolve(&key, &mut state);
/// }
/// ```
#[derive(Default)]
pub struct ResolverRegistry {
    /// Resolvers indexed by mode ID.
    resolvers: HashMap<ModeId, Arc<dyn ModeKeyResolver>>,
}

impl ResolverRegistry {
    /// Create a new empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a resolver for its mode.
    ///
    /// The resolver's `mode_id()` is used as the key. If a resolver
    /// for that mode already exists, it is replaced.
    pub fn register<R: ModeKeyResolver + 'static>(&mut self, resolver: R) {
        let mode_id = resolver.mode_id().clone();
        self.resolvers.insert(mode_id, Arc::new(resolver));
    }

    /// Register a resolver wrapped in Arc.
    ///
    /// Use this when you need to share the resolver reference.
    pub fn register_arc(&mut self, resolver: Arc<dyn ModeKeyResolver>) {
        let mode_id = resolver.mode_id().clone();
        self.resolvers.insert(mode_id, resolver);
    }

    /// Get the resolver for a mode.
    #[must_use]
    pub fn get(&self, mode: &ModeId) -> Option<Arc<dyn ModeKeyResolver>> {
        self.resolvers.get(mode).cloned()
    }

    /// Check if a resolver is registered for a mode.
    #[must_use]
    pub fn has(&self, mode: &ModeId) -> bool {
        self.resolvers.contains_key(mode)
    }

    /// Remove a resolver for a mode.
    ///
    /// Returns the removed resolver, if any.
    pub fn remove(&mut self, mode: &ModeId) -> Option<Arc<dyn ModeKeyResolver>> {
        self.resolvers.remove(mode)
    }

    /// Get the number of registered resolvers.
    #[must_use]
    pub fn len(&self) -> usize {
        self.resolvers.len()
    }

    /// Check if the registry is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.resolvers.is_empty()
    }

    /// Get all registered mode IDs.
    pub fn modes(&self) -> impl Iterator<Item = &ModeId> {
        self.resolvers.keys()
    }

    /// Resolve a key event for a mode.
    ///
    /// This is a convenience method that:
    /// 1. Looks up the resolver for the mode
    /// 2. Calls `resolve()` on it
    /// 3. If `NotHandled`, tries the parent mode (if `inherits_from()` is set)
    ///
    /// Returns `None` if no resolver is registered for the mode (or its parents).
    pub fn resolve(
        &self,
        mode: &ModeId,
        key: &KeyEvent,
        state: &mut ModeState,
    ) -> Option<ResolveResult> {
        let resolver = self.get(mode)?;
        let result = resolver.resolve(key, state);

        // If not handled, try parent mode
        if matches!(result, ResolveResult::NotHandled)
            && let Some(parent) = resolver.inherits_from()
        {
            return self.resolve(parent, key, state);
        }

        Some(result)
    }
}

impl std::fmt::Debug for ResolverRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResolverRegistry")
            .field("modes", &self.resolvers.keys().collect::<Vec<_>>())
            .field("count", &self.resolvers.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            mode::EditorMode,
            resolver::{VimInsertResolver, VimNormalResolver, VimOperatorPendingResolver},
        },
    };

    #[test]
    fn test_new_registry() {
        let registry = ResolverRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn test_register_resolver() {
        let mut registry = ResolverRegistry::new();

        registry.register(VimNormalResolver::new());

        assert_eq!(registry.len(), 1);
        assert!(registry.has(&EditorMode::NORMAL_ID));
    }

    #[test]
    fn test_register_multiple() {
        let mut registry = ResolverRegistry::new();

        registry.register(VimNormalResolver::new());
        registry.register(VimInsertResolver::new());
        registry.register(VimOperatorPendingResolver::new());

        assert_eq!(registry.len(), 3);
        assert!(registry.has(&EditorMode::NORMAL_ID));
        assert!(registry.has(&EditorMode::INSERT_ID));
        assert!(registry.has(&EditorMode::OPERATOR_PENDING_ID));
    }

    #[test]
    fn test_get_resolver() {
        let mut registry = ResolverRegistry::new();
        registry.register(VimNormalResolver::new());

        let resolver = registry.get(&EditorMode::NORMAL_ID);
        assert!(resolver.is_some());
        assert_eq!(resolver.unwrap().mode_id(), &EditorMode::NORMAL_ID);
    }

    #[test]
    fn test_get_nonexistent() {
        let registry = ResolverRegistry::new();
        assert!(registry.get(&EditorMode::NORMAL_ID).is_none());
    }

    #[test]
    fn test_remove_resolver() {
        let mut registry = ResolverRegistry::new();
        registry.register(VimNormalResolver::new());

        let removed = registry.remove(&EditorMode::NORMAL_ID);
        assert!(removed.is_some());
        assert!(!registry.has(&EditorMode::NORMAL_ID));
        assert!(registry.is_empty());
    }

    #[test]
    fn test_modes_iterator() {
        let mut registry = ResolverRegistry::new();
        registry.register(VimNormalResolver::new());
        registry.register(VimInsertResolver::new());

        assert_eq!(registry.modes().count(), 2);
    }

    #[test]
    fn test_register_replaces() {
        let mut registry = ResolverRegistry::new();

        registry.register(VimNormalResolver::new());
        assert_eq!(registry.len(), 1);

        // Register another normal mode resolver
        registry.register(VimNormalResolver::new());
        assert_eq!(registry.len(), 1); // Still 1, replaced
    }

    #[test]
    fn test_register_arc() {
        let mut registry = ResolverRegistry::new();
        let resolver: Arc<dyn ModeKeyResolver> = Arc::new(VimNormalResolver::new());

        registry.register_arc(resolver);

        assert!(registry.has(&EditorMode::NORMAL_ID));
    }

    #[test]
    fn test_debug_impl() {
        let mut registry = ResolverRegistry::new();
        registry.register(VimNormalResolver::new());

        let debug = format!("{registry:?}");
        assert!(debug.contains("ResolverRegistry"));
        assert!(debug.contains("count: 1"));
    }
}
