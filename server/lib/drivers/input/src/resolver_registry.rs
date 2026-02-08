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
/// use reovim_driver_input::ResolverRegistry;
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

#[cfg(test)]
mod tests {
    use reovim_kernel::api::v1::ModuleId;

    use {
        super::*,
        crate::{ModeState, ResolveResult},
    };

    fn test_module() -> ModuleId {
        ModuleId::new("test")
    }

    fn normal_mode() -> ModeId {
        ModeId::with_discriminant(test_module(), "normal", 0)
    }

    fn insert_mode() -> ModeId {
        ModeId::with_discriminant(test_module(), "insert", 1)
    }

    fn visual_mode() -> ModeId {
        ModeId::with_discriminant(test_module(), "visual", 2)
    }

    fn op_pending_mode() -> ModeId {
        ModeId::with_discriminant(test_module(), "operator-pending", 3)
    }

    /// A minimal resolver for testing registry operations.
    struct StubResolver {
        mode: ModeId,
        parent: Option<ModeId>,
    }

    impl StubResolver {
        fn new(mode: ModeId) -> Self {
            Self { mode, parent: None }
        }

        fn with_parent(mode: ModeId, parent: ModeId) -> Self {
            Self {
                mode,
                parent: Some(parent),
            }
        }
    }

    impl ModeKeyResolver for StubResolver {
        fn resolve_with_keymap(
            &self,
            _key: &KeyEvent,
            _state: &mut ModeState,
            _input: &ResolveInput<'_>,
        ) -> ResolveResult {
            ResolveResult::Pending
        }

        fn mode_id(&self) -> &ModeId {
            &self.mode
        }

        fn inherits_from(&self) -> Option<&ModeId> {
            self.parent.as_ref()
        }
    }

    /// A resolver that returns `NotHandled`, for testing parent fallback.
    struct NotHandledResolver {
        mode: ModeId,
        parent: Option<ModeId>,
    }

    impl NotHandledResolver {
        fn with_parent(mode: ModeId, parent: ModeId) -> Self {
            Self {
                mode,
                parent: Some(parent),
            }
        }
    }

    impl ModeKeyResolver for NotHandledResolver {
        fn resolve_with_keymap(
            &self,
            _key: &KeyEvent,
            _state: &mut ModeState,
            _input: &ResolveInput<'_>,
        ) -> ResolveResult {
            ResolveResult::NotHandled
        }

        fn mode_id(&self) -> &ModeId {
            &self.mode
        }

        fn inherits_from(&self) -> Option<&ModeId> {
            self.parent.as_ref()
        }
    }

    /// Mock keymap that always returns `NotFound`.
    struct NoOpKeymap;

    impl KeymapQuery for NoOpKeymap {
        fn query(&self, _mode: &ModeId, _keys: &crate::KeySequence) -> crate::KeyLookupState {
            crate::KeyLookupState::NotFound
        }
    }

    #[test]
    fn test_default_registry() {
        let registry = ResolverRegistry::default();
        assert!(registry.is_empty());
    }

    #[test]
    fn test_register_and_get() {
        let registry = ResolverRegistry::new();
        let mode = normal_mode();

        registry.register(StubResolver::new(mode.clone()));

        assert!(registry.has(&mode));
        assert_eq!(registry.len(), 1);

        let resolver = registry.get(&mode);
        assert!(resolver.is_some());
        assert_eq!(resolver.unwrap().mode_id(), &mode);
    }

    #[test]
    fn test_register_arc() {
        let registry = ResolverRegistry::new();
        let mode = insert_mode();

        let resolver: Arc<dyn ModeKeyResolver> = Arc::new(StubResolver::new(mode.clone()));
        registry.register_arc(resolver);

        assert!(registry.has(&mode));
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn test_get_nonexistent_returns_none() {
        let registry = ResolverRegistry::new();
        let mode = ModeId::with_discriminant(test_module(), "nonexistent", 99);

        assert!(registry.get(&mode).is_none());
        assert!(!registry.has(&mode));
    }

    #[test]
    fn test_register_replaces_existing() {
        let registry = ResolverRegistry::new();
        let mode = normal_mode();

        registry.register(StubResolver::new(mode.clone()));
        registry.register(StubResolver::new(mode));

        // Should still be 1 (replaced, not added)
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn test_remove() {
        let registry = ResolverRegistry::new();
        let mode = normal_mode();

        registry.register(StubResolver::new(mode.clone()));
        assert!(registry.has(&mode));

        let removed = registry.remove(&mode);
        assert!(removed.is_some());
        assert!(!registry.has(&mode));
        assert!(registry.is_empty());
    }

    #[test]
    fn test_remove_nonexistent_returns_none() {
        let registry = ResolverRegistry::new();
        let mode = ModeId::with_discriminant(test_module(), "nonexistent", 99);

        let removed = registry.remove(&mode);
        assert!(removed.is_none());
    }

    #[test]
    fn test_modes() {
        let registry = ResolverRegistry::new();
        let normal = normal_mode();
        let insert = insert_mode();

        registry.register(StubResolver::new(normal.clone()));
        registry.register(StubResolver::new(insert.clone()));

        let modes = registry.modes();
        assert_eq!(modes.len(), 2);
        assert!(modes.contains(&normal));
        assert!(modes.contains(&insert));
    }

    #[test]
    fn test_debug_format() {
        let registry = ResolverRegistry::new();
        registry.register(StubResolver::new(normal_mode()));

        let debug = format!("{registry:?}");
        assert!(debug.contains("ResolverRegistry"));
        assert!(debug.contains("count"));
    }

    #[test]
    fn test_resolve_with_keymap_returns_none_for_unknown_mode() {
        let registry = ResolverRegistry::new();
        let mode = ModeId::with_discriminant(test_module(), "nonexistent", 99);
        let key = KeyEvent::new(crate::KeyCode::Char('j'));
        let mut state = ModeState::new(mode.clone());
        let keymap = NoOpKeymap;

        let result = registry.resolve_with_keymap(&mode, &key, &mut state, &keymap);
        assert!(result.is_none());
    }

    #[test]
    fn test_resolve_with_keymap_returns_result() {
        let registry = ResolverRegistry::new();
        let mode = normal_mode();

        registry.register(StubResolver::new(mode.clone()));

        let key = KeyEvent::new(crate::KeyCode::Char('j'));
        let mut state = ModeState::new(mode.clone());
        let keymap = NoOpKeymap;

        let result = registry.resolve_with_keymap(&mode, &key, &mut state, &keymap);
        assert!(result.is_some());
        assert!(matches!(result.unwrap(), ResolveResult::Pending));
    }

    #[test]
    fn test_resolve_with_keymap_falls_back_to_parent() {
        let registry = ResolverRegistry::new();
        let normal = normal_mode();
        let op_pending = op_pending_mode();

        // op-pending returns NotHandled and inherits from normal
        registry.register(NotHandledResolver::with_parent(op_pending.clone(), normal.clone()));
        // normal returns Pending
        registry.register(StubResolver::new(normal));

        let key = KeyEvent::new(crate::KeyCode::Char('w'));
        let mut state = ModeState::new(op_pending.clone());
        let keymap = NoOpKeymap;

        let result = registry.resolve_with_keymap(&op_pending, &key, &mut state, &keymap);
        assert!(result.is_some());
        // Should have fallen through to normal resolver which returns Pending
        assert!(matches!(result.unwrap(), ResolveResult::Pending));
    }

    #[test]
    fn test_resolve_with_extensions_returns_none_for_unknown_mode() {
        let registry = ResolverRegistry::new();
        let mode = ModeId::with_discriminant(test_module(), "nonexistent", 99);
        let key = KeyEvent::new(crate::KeyCode::Char('j'));
        let mut state = ModeState::new(mode.clone());
        let keymap = NoOpKeymap;
        let mut extensions = crate::ExtensionMap::new();

        let result =
            registry.resolve_with_extensions(&mode, &key, &mut state, &keymap, &mut extensions);
        assert!(result.is_none());
    }

    #[test]
    fn test_resolve_with_extensions_returns_result() {
        let registry = ResolverRegistry::new();
        let mode = normal_mode();
        registry.register(StubResolver::new(mode.clone()));

        let key = KeyEvent::new(crate::KeyCode::Char('j'));
        let mut state = ModeState::new(mode.clone());
        let keymap = NoOpKeymap;
        let mut extensions = crate::ExtensionMap::new();

        let result =
            registry.resolve_with_extensions(&mode, &key, &mut state, &keymap, &mut extensions);
        assert!(result.is_some());
        assert!(matches!(result.unwrap(), ResolveResult::Pending));
    }

    #[test]
    fn test_resolve_with_extensions_falls_back_to_parent() {
        let registry = ResolverRegistry::new();
        let normal = normal_mode();
        let op_pending = op_pending_mode();

        registry.register(NotHandledResolver::with_parent(op_pending.clone(), normal.clone()));
        registry.register(StubResolver::new(normal));

        let key = KeyEvent::new(crate::KeyCode::Char('w'));
        let mut state = ModeState::new(op_pending.clone());
        let keymap = NoOpKeymap;
        let mut extensions = crate::ExtensionMap::new();

        let result = registry.resolve_with_extensions(
            &op_pending,
            &key,
            &mut state,
            &keymap,
            &mut extensions,
        );
        assert!(result.is_some());
        assert!(matches!(result.unwrap(), ResolveResult::Pending));
    }

    #[test]
    fn test_multiple_resolvers() {
        let registry = ResolverRegistry::new();
        let normal = normal_mode();
        let insert = insert_mode();
        let visual = visual_mode();

        registry.register(StubResolver::new(normal.clone()));
        registry.register(StubResolver::new(insert.clone()));
        registry.register(StubResolver::new(visual.clone()));

        assert_eq!(registry.len(), 3);
        assert!(registry.has(&normal));
        assert!(registry.has(&insert));
        assert!(registry.has(&visual));
    }

    #[test]
    fn test_resolver_with_parent_inherits_from() {
        let normal = normal_mode();
        let op_pending = op_pending_mode();

        let resolver = StubResolver::with_parent(op_pending.clone(), normal.clone());
        assert_eq!(resolver.mode_id(), &op_pending);
        assert_eq!(resolver.inherits_from(), Some(&normal));
    }

    #[test]
    fn test_resolver_without_parent() {
        let normal = normal_mode();
        let resolver = StubResolver::new(normal.clone());
        assert_eq!(resolver.mode_id(), &normal);
        assert!(resolver.inherits_from().is_none());
    }

    // ========================================================================
    // resolve_with_session tests
    // ========================================================================

    #[test]
    fn test_resolve_with_session_returns_none_for_unknown_mode() {
        let registry = ResolverRegistry::new();
        let mode = ModeId::with_discriminant(test_module(), "nonexistent", 99);
        let key = KeyEvent::new(crate::KeyCode::Char('j'));
        let mut state = ModeState::new(mode.clone());
        let keymap = NoOpKeymap;
        let mut extensions = crate::ExtensionMap::new();
        let mut test_rt = reovim_driver_session::testing::TestSessionRuntime::new();
        let mut runtime = test_rt.runtime();

        let result = registry.resolve_with_session(
            &mode,
            &key,
            &mut state,
            &keymap,
            &mut runtime,
            &mut extensions,
        );
        assert!(result.is_none());
    }

    #[test]
    fn test_resolve_with_session_returns_result() {
        let registry = ResolverRegistry::new();
        let mode = normal_mode();
        registry.register(StubResolver::new(mode.clone()));

        let key = KeyEvent::new(crate::KeyCode::Char('j'));
        let mut state = ModeState::new(mode.clone());
        let keymap = NoOpKeymap;
        let mut extensions = crate::ExtensionMap::new();
        let mut test_rt = reovim_driver_session::testing::TestSessionRuntime::new();
        let mut runtime = test_rt.runtime();

        let result = registry.resolve_with_session(
            &mode,
            &key,
            &mut state,
            &keymap,
            &mut runtime,
            &mut extensions,
        );
        assert!(result.is_some());
        assert!(matches!(result.unwrap(), ResolveResult::Pending));
    }

    #[test]
    fn test_resolve_with_session_falls_back_to_parent() {
        let registry = ResolverRegistry::new();
        let normal = normal_mode();
        let op_pending = op_pending_mode();

        registry.register(NotHandledResolver::with_parent(op_pending.clone(), normal.clone()));
        registry.register(StubResolver::new(normal));

        let key = KeyEvent::new(crate::KeyCode::Char('w'));
        let mut state = ModeState::new(op_pending.clone());
        let keymap = NoOpKeymap;
        let mut extensions = crate::ExtensionMap::new();
        let mut test_rt = reovim_driver_session::testing::TestSessionRuntime::new();
        let mut runtime = test_rt.runtime();

        let result = registry.resolve_with_session(
            &op_pending,
            &key,
            &mut state,
            &keymap,
            &mut runtime,
            &mut extensions,
        );
        assert!(result.is_some());
        assert!(matches!(result.unwrap(), ResolveResult::Pending));
    }

    // ========================================================================
    // NotHandledResolver without parent (no fallback possible)
    // ========================================================================

    #[test]
    fn test_resolve_with_keymap_not_handled_no_parent() {
        let registry = ResolverRegistry::new();
        let mode = visual_mode();

        // NotHandledResolver without parent - just returns NotHandled
        struct NoParentResolver {
            mode: ModeId,
        }
        impl ModeKeyResolver for NoParentResolver {
            fn resolve_with_keymap(
                &self,
                _key: &KeyEvent,
                _state: &mut ModeState,
                _input: &ResolveInput<'_>,
            ) -> ResolveResult {
                ResolveResult::NotHandled
            }
            fn mode_id(&self) -> &ModeId {
                &self.mode
            }
        }

        registry.register(NoParentResolver { mode: mode.clone() });

        let key = KeyEvent::new(crate::KeyCode::Char('x'));
        let mut state = ModeState::new(mode.clone());
        let keymap = NoOpKeymap;

        let result = registry.resolve_with_keymap(&mode, &key, &mut state, &keymap);
        assert!(result.is_some());
        assert!(matches!(result.unwrap(), ResolveResult::NotHandled));
    }

    #[test]
    fn test_resolve_with_extensions_not_handled_no_parent() {
        let registry = ResolverRegistry::new();
        let mode = visual_mode();

        struct NoParentResolver {
            mode: ModeId,
        }
        impl ModeKeyResolver for NoParentResolver {
            fn resolve_with_keymap(
                &self,
                _key: &KeyEvent,
                _state: &mut ModeState,
                _input: &ResolveInput<'_>,
            ) -> ResolveResult {
                ResolveResult::NotHandled
            }
            fn mode_id(&self) -> &ModeId {
                &self.mode
            }
        }

        registry.register(NoParentResolver { mode: mode.clone() });

        let key = KeyEvent::new(crate::KeyCode::Char('x'));
        let mut state = ModeState::new(mode.clone());
        let keymap = NoOpKeymap;
        let mut extensions = crate::ExtensionMap::new();

        let result =
            registry.resolve_with_extensions(&mode, &key, &mut state, &keymap, &mut extensions);
        assert!(result.is_some());
        assert!(matches!(result.unwrap(), ResolveResult::NotHandled));
    }

    #[test]
    fn test_resolve_with_session_not_handled_no_parent() {
        let registry = ResolverRegistry::new();
        let mode = visual_mode();

        struct NoParentResolver {
            mode: ModeId,
        }
        impl ModeKeyResolver for NoParentResolver {
            fn resolve_with_keymap(
                &self,
                _key: &KeyEvent,
                _state: &mut ModeState,
                _input: &ResolveInput<'_>,
            ) -> ResolveResult {
                ResolveResult::NotHandled
            }
            fn mode_id(&self) -> &ModeId {
                &self.mode
            }
        }

        registry.register(NoParentResolver { mode: mode.clone() });

        let key = KeyEvent::new(crate::KeyCode::Char('x'));
        let mut state = ModeState::new(mode.clone());
        let keymap = NoOpKeymap;
        let mut extensions = crate::ExtensionMap::new();
        let mut test_rt = reovim_driver_session::testing::TestSessionRuntime::new();
        let mut runtime = test_rt.runtime();

        let result = registry.resolve_with_session(
            &mode,
            &key,
            &mut state,
            &keymap,
            &mut runtime,
            &mut extensions,
        );
        assert!(result.is_some());
        assert!(matches!(result.unwrap(), ResolveResult::NotHandled));
    }

    // ========================================================================
    // Registry Service trait
    // ========================================================================

    #[test]
    fn test_resolver_registry_is_service() {
        fn accepts_service(_: &dyn reovim_kernel::api::v1::Service) {}
        let registry = ResolverRegistry::new();
        accepts_service(&registry);
    }

    // ========================================================================
    // Multiple register and remove operations
    // ========================================================================

    #[test]
    fn test_register_remove_register_same_mode() {
        let registry = ResolverRegistry::new();
        let mode = normal_mode();

        registry.register(StubResolver::new(mode.clone()));
        assert!(registry.has(&mode));

        registry.remove(&mode);
        assert!(!registry.has(&mode));

        registry.register(StubResolver::new(mode.clone()));
        assert!(registry.has(&mode));
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn test_modes_after_removal() {
        let registry = ResolverRegistry::new();
        let normal = normal_mode();
        let insert = insert_mode();

        registry.register(StubResolver::new(normal.clone()));
        registry.register(StubResolver::new(insert.clone()));
        assert_eq!(registry.modes().len(), 2);

        registry.remove(&normal);
        let modes = registry.modes();
        assert_eq!(modes.len(), 1);
        assert!(modes.contains(&insert));
    }

    // ========================================================================
    // resolve_with_keymap with pending keys
    // ========================================================================

    #[test]
    fn test_resolve_with_keymap_clones_pending_keys() {
        let registry = ResolverRegistry::new();
        let mode = normal_mode();
        registry.register(StubResolver::new(mode.clone()));

        let key = KeyEvent::new(crate::KeyCode::Char('j'));
        let mut state = ModeState::new(mode.clone());
        // Add pending keys to state
        state.push_pending_key(KeyEvent::new(crate::KeyCode::Char('g')));
        let keymap = NoOpKeymap;

        let result = registry.resolve_with_keymap(&mode, &key, &mut state, &keymap);
        assert!(result.is_some());
        // Pending keys should still be in state (clone, not take)
        assert!(state.has_pending_keys());
    }

    // ========================================================================
    // NotHandledResolver extensions fallback chain with missing parent
    // ========================================================================

    #[test]
    fn test_resolve_with_keymap_parent_not_registered() {
        let registry = ResolverRegistry::new();
        let normal = normal_mode();
        let op_pending = op_pending_mode();

        // op-pending inherits from normal, but normal is NOT registered
        registry.register(NotHandledResolver::with_parent(op_pending.clone(), normal));

        let key = KeyEvent::new(crate::KeyCode::Char('w'));
        let mut state = ModeState::new(op_pending.clone());
        let keymap = NoOpKeymap;

        let result = registry.resolve_with_keymap(&op_pending, &key, &mut state, &keymap);
        // Parent not found -> returns None from recursive call
        assert!(result.is_none());
    }

    #[test]
    fn test_resolve_with_extensions_parent_not_registered() {
        let registry = ResolverRegistry::new();
        let normal = normal_mode();
        let op_pending = op_pending_mode();

        registry.register(NotHandledResolver::with_parent(op_pending.clone(), normal));

        let key = KeyEvent::new(crate::KeyCode::Char('w'));
        let mut state = ModeState::new(op_pending.clone());
        let keymap = NoOpKeymap;
        let mut extensions = crate::ExtensionMap::new();

        let result = registry.resolve_with_extensions(
            &op_pending,
            &key,
            &mut state,
            &keymap,
            &mut extensions,
        );
        assert!(result.is_none());
    }

    #[test]
    fn test_resolve_with_session_parent_not_registered() {
        let registry = ResolverRegistry::new();
        let normal = normal_mode();
        let op_pending = op_pending_mode();

        registry.register(NotHandledResolver::with_parent(op_pending.clone(), normal));

        let key = KeyEvent::new(crate::KeyCode::Char('w'));
        let mut state = ModeState::new(op_pending.clone());
        let keymap = NoOpKeymap;
        let mut extensions = crate::ExtensionMap::new();
        let mut test_rt = reovim_driver_session::testing::TestSessionRuntime::new();
        let mut runtime = test_rt.runtime();

        let result = registry.resolve_with_session(
            &op_pending,
            &key,
            &mut state,
            &keymap,
            &mut runtime,
            &mut extensions,
        );
        assert!(result.is_none());
    }
}
