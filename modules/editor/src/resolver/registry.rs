//! Resolver registry for managing mode key resolvers.
//!
//! The registry maps mode IDs to their resolvers and provides lookup
//! functionality for the event loop.

use std::{collections::HashMap, sync::Arc};

use {
    reovim_driver_input::{
        ExtensionMap, KeyEvent, KeymapQuery, ModeKeyResolver, ModeState, ResolveInput,
        ResolveResult, SessionApiDyn,
    },
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
/// use reovim_module_editor::ResolverRegistry;
/// use reovim_module_vim::{VimNormalResolver, VimInsertResolver};
///
/// let mut registry = ResolverRegistry::new();
///
/// // Register Vim resolvers (from vim module)
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

    /// Resolve a key event for a mode with keymap access.
    ///
    /// This is the preferred method that provides resolvers with access to
    /// keymap queries for mechanism/policy separation:
    /// - Resolvers can call `keymap.query()` to get FACTS about bindings
    /// - Resolvers apply their own POLICY to decide what to do
    ///
    /// # Arguments
    ///
    /// * `mode` - The mode to resolve for
    /// * `key` - The key event to process
    /// * `state` - Mutable mode state
    /// * `keymap` - Access to keymap queries
    ///
    /// Returns `None` if no resolver is registered for the mode (or its parents).
    pub fn resolve_with_keymap(
        &self,
        mode: &ModeId,
        key: &KeyEvent,
        state: &mut ModeState,
        keymap: &dyn KeymapQuery,
    ) -> Option<ResolveResult> {
        let resolver = self.get(mode)?;

        // Clone pending keys to avoid borrow checker issues
        // (we need to borrow state mutably while also accessing pending_keys)
        let keys = state.pending_keys.clone();
        let input = ResolveInput::new(&keys, mode, keymap);
        let result = resolver.resolve_with_keymap(key, state, &input);

        // If not handled, try parent mode
        if matches!(result, ResolveResult::NotHandled)
            && let Some(parent) = resolver.inherits_from()
        {
            return self.resolve_with_keymap(parent, key, state, keymap);
        }

        Some(result)
    }

    /// Resolve a key event for a mode with keymap AND extension access.
    ///
    /// This is the preferred method for the new resolver architecture (Epic #385).
    /// Resolvers can access per-session module state via the extensions parameter,
    /// enabling them to handle vim-specific state without runner involvement.
    ///
    /// # Architecture
    ///
    /// - **Mechanism (runner)**: Routes keys, provides extensions
    /// - **Policy (resolvers)**: Access `VimSessionState` in extensions, decide what to do
    ///
    /// # Arguments
    ///
    /// * `mode` - The mode to resolve for
    /// * `key` - The key event to process
    /// * `state` - Mutable mode state
    /// * `keymap` - Access to keymap queries
    /// * `extensions` - Per-session extension storage for module state
    ///
    /// Returns `None` if no resolver is registered for the mode (or its parents).
    pub fn resolve_with_extensions(
        &self,
        mode: &ModeId,
        key: &KeyEvent,
        state: &mut ModeState,
        keymap: &dyn KeymapQuery,
        extensions: &mut ExtensionMap,
    ) -> Option<ResolveResult> {
        let resolver = self.get(mode)?;

        // Clone pending keys to avoid borrow checker issues
        let keys = state.pending_keys.clone();
        let input = ResolveInput::new(&keys, mode, keymap);
        let result = resolver.resolve_with_extensions(key, state, &input, extensions);

        // If not handled, try parent mode
        if matches!(result, ResolveResult::NotHandled)
            && let Some(parent) = resolver.inherits_from()
        {
            return self.resolve_with_extensions(parent, key, state, keymap, extensions);
        }

        Some(result)
    }

    /// Resolve a key event for a mode with full session API access.
    ///
    /// This is the preferred method for Epic #393 - Session Driver API for Resolver Actions.
    /// Resolvers can directly manipulate session state via the `session` parameter and
    /// return `ResolveResult::Completed` when done.
    ///
    /// # Architecture
    ///
    /// - **Mechanism (runner)**: Creates `SessionRuntime`, routes keys to resolvers
    /// - **Policy (resolvers)**: Perform actions directly via `session.*` methods
    /// - **Coordination**: Runner takes `StateChanges` after resolution for notifications
    ///
    /// # Arguments
    ///
    /// * `mode` - The mode to resolve for
    /// * `key` - The key event to process
    /// * `state` - Mutable mode state
    /// * `keymap` - Access to keymap queries
    /// * `session` - Dyn-compatible session API for direct state manipulation
    /// * `extensions` - Per-session extension storage for module state
    ///
    /// # Returns
    ///
    /// - `Some(result)` - Resolver handled the key
    /// - `None` - No resolver registered for the mode
    ///
    /// # Example
    ///
    /// ```ignore
    /// // In event loop:
    /// let mut runtime = SessionRuntime::new(&mut session, &kernel, &executor);
    ///
    /// if let Some(result) = registry.resolve_with_session(
    ///     &mode, &key, &mut state, &keymap, &mut runtime, &mut extensions
    /// ) {
    ///     if matches!(result, ResolveResult::Completed) {
    ///         let changes = runtime.take_changes();
    ///         broadcast_notifications(&changes);
    ///     } else {
    ///         handle_resolve_result(result);
    ///     }
    /// }
    /// ```
    pub fn resolve_with_session(
        &self,
        mode: &ModeId,
        key: &KeyEvent,
        state: &mut ModeState,
        keymap: &dyn KeymapQuery,
        session: &mut dyn SessionApiDyn,
        extensions: &mut ExtensionMap,
    ) -> Option<ResolveResult> {
        let resolver = self.get(mode)?;

        // Clone pending keys to avoid borrow checker issues
        let keys = state.pending_keys.clone();
        let input = ResolveInput::new(&keys, mode, keymap);
        let result = resolver.resolve_with_session(key, state, &input, session, extensions);

        // If not handled, try parent mode
        if matches!(result, ResolveResult::NotHandled)
            && let Some(parent) = resolver.inherits_from()
        {
            return self.resolve_with_session(parent, key, state, keymap, session, extensions);
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
    use super::*;

    #[test]
    fn test_new_registry() {
        let registry = ResolverRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }
}
