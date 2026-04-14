//! Resolver registry for managing mode key resolvers.
//!
//! The registry maps mode IDs to their resolvers and provides lookup
//! functionality for the event loop.
//!
//! # Architecture
//!
//! This is a **mechanism** (storage/lookup) - policy implementations
//! live in modules (e.g., vim, editor). The registry:
//!
//! - Stores `Arc<dyn ModeKeyResolver>` indexed by `ModeId`
//! - Provides resolution methods with increasing capability
//! - Supports mode inheritance (fallback to parent mode)
//!
//! # Note
//!
//! Moved from `server/modules/editor/` to `server/lib/drivers/input/` in Epic #417
//! to maintain proper mechanism/policy separation. The registry is pure
//! mechanism (`HashMap` storage), while resolver implementations are policy.

use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use reovim_kernel::api::v1::ModeId;

use crate::{
    ExtensionMap, KeyEvent, KeymapQuery, ModeKeyResolver, ModeState, ResolveInput, ResolveResult,
    SessionApiDyn,
};

/// Registry for mode key resolvers.
///
/// Maps mode IDs to resolver implementations. The event loop uses this
/// to find the appropriate resolver for the current mode.
///
/// # Thread Safety
///
/// Uses `RwLock` for interior mutability, allowing modules to register
/// resolvers during `init()` through a shared reference.
/// Resolvers are stored as `Arc<dyn ModeKeyResolver>` for shared access.
///
/// # Example
///
/// ```ignore
/// use reovim_driver_text_input::ResolverRegistry;
/// use reovim_module_vim::{VimNormalResolver, VimInsertResolver};
///
/// let registry = ResolverRegistry::new();
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
pub struct ResolverRegistry {
    /// Resolvers indexed by mode ID.
    resolvers: RwLock<HashMap<ModeId, Arc<dyn ModeKeyResolver>>>,
}

impl ResolverRegistry {
    /// Create a new empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            resolvers: RwLock::new(HashMap::new()),
        }
    }

    /// Register a resolver for its mode.
    ///
    /// The resolver's `mode_id()` is used as the key. If a resolver
    /// for that mode already exists, it is replaced.
    ///
    /// # Panics
    ///
    /// Panics if the lock is poisoned.
    pub fn register<R: ModeKeyResolver + 'static>(&self, resolver: R) {
        let mode_id = resolver.mode_id().clone();
        self.resolvers
            .write()
            .expect("ResolverRegistry lock poisoned")
            .insert(mode_id, Arc::new(resolver));
    }

    /// Register a resolver wrapped in Arc.
    ///
    /// Use this when you need to share the resolver reference.
    ///
    /// # Panics
    ///
    /// Panics if the lock is poisoned.
    pub fn register_arc(&self, resolver: Arc<dyn ModeKeyResolver>) {
        let mode_id = resolver.mode_id().clone();
        self.resolvers
            .write()
            .expect("ResolverRegistry lock poisoned")
            .insert(mode_id, resolver);
    }

    /// Get the resolver for a mode.
    ///
    /// # Panics
    ///
    /// Panics if the lock is poisoned.
    #[must_use]
    pub fn get(&self, mode: &ModeId) -> Option<Arc<dyn ModeKeyResolver>> {
        self.resolvers
            .read()
            .expect("ResolverRegistry lock poisoned")
            .get(mode)
            .cloned()
    }

    /// Check if a resolver is registered for a mode.
    ///
    /// # Panics
    ///
    /// Panics if the lock is poisoned.
    #[must_use]
    pub fn has(&self, mode: &ModeId) -> bool {
        self.resolvers
            .read()
            .expect("ResolverRegistry lock poisoned")
            .contains_key(mode)
    }

    /// Remove a resolver for a mode.
    ///
    /// Returns the removed resolver, if any.
    ///
    /// # Panics
    ///
    /// Panics if the lock is poisoned.
    pub fn remove(&self, mode: &ModeId) -> Option<Arc<dyn ModeKeyResolver>> {
        self.resolvers
            .write()
            .expect("ResolverRegistry lock poisoned")
            .remove(mode)
    }

    /// Get the number of registered resolvers.
    ///
    /// # Panics
    ///
    /// Panics if the lock is poisoned.
    #[must_use]
    pub fn len(&self) -> usize {
        self.resolvers
            .read()
            .expect("ResolverRegistry lock poisoned")
            .len()
    }

    /// Check if the registry is empty.
    ///
    /// # Panics
    ///
    /// Panics if the lock is poisoned.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.resolvers
            .read()
            .expect("ResolverRegistry lock poisoned")
            .is_empty()
    }

    /// Get all registered mode IDs.
    ///
    /// # Panics
    ///
    /// Panics if the lock is poisoned.
    #[must_use]
    pub fn modes(&self) -> Vec<ModeId> {
        self.resolvers
            .read()
            .expect("ResolverRegistry lock poisoned")
            .keys()
            .cloned()
            .collect()
    }

    /// Get pending keys from the resolver for a mode.
    ///
    /// Returns empty `KeySequence` if no resolver is registered or if
    /// the resolver has no pending keys.
    ///
    /// # Panics
    ///
    /// Panics if the lock is poisoned.
    #[must_use]
    pub fn pending_keys_for(&self, mode: &ModeId) -> crate::KeySequence {
        self.get(mode)
            .map_or_else(crate::KeySequence::new, |r| r.pending_keys())
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
        shared_extensions: &mut ExtensionMap,
        client_extensions: &mut ExtensionMap,
    ) -> Option<ResolveResult> {
        let resolver = self.get(mode)?;

        // Clone pending keys to avoid borrow checker issues
        let keys = state.pending_keys.clone();
        let input = ResolveInput::new(&keys, mode, keymap);
        let result = resolver.resolve_with_extensions(
            key,
            state,
            &input,
            shared_extensions,
            client_extensions,
        );

        // If not handled, try parent mode
        if matches!(result, ResolveResult::NotHandled)
            && let Some(parent) = resolver.inherits_from()
        {
            return self.resolve_with_extensions(
                parent,
                key,
                state,
                keymap,
                shared_extensions,
                client_extensions,
            );
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
    #[allow(clippy::too_many_arguments)]
    pub fn resolve_with_session(
        &self,
        mode: &ModeId,
        key: &KeyEvent,
        state: &mut ModeState,
        keymap: &dyn KeymapQuery,
        session: &mut dyn SessionApiDyn,
        shared_extensions: &mut ExtensionMap,
        client_extensions: &mut ExtensionMap,
    ) -> Option<ResolveResult> {
        let resolver = self.get(mode)?;

        // Clone pending keys to avoid borrow checker issues
        let keys = state.pending_keys.clone();
        let input = ResolveInput::new(&keys, mode, keymap);
        let result = resolver.resolve_with_session(
            key,
            state,
            &input,
            session,
            shared_extensions,
            client_extensions,
        );

        // If not handled, try parent mode
        if matches!(result, ResolveResult::NotHandled)
            && let Some(parent) = resolver.inherits_from()
        {
            return self.resolve_with_session(
                parent,
                key,
                state,
                keymap,
                session,
                shared_extensions,
                client_extensions,
            );
        }

        Some(result)
    }
}

impl Default for ResolverRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for ResolverRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResolverRegistry")
            .field("modes", &self.modes())
            .field("count", &self.len())
            .finish()
    }
}

// Implement Service so ResolverRegistry can be stored in ServiceRegistry (Epic #417 Part 3)
impl reovim_kernel::api::v1::Service for ResolverRegistry {}
