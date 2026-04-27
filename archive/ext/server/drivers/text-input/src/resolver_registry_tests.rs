use std::sync::Arc;

use reovim_kernel::api::v1::{ModeId, ModuleId};

use crate::{
    KeyEvent, KeymapQuery, ModeKeyResolver, ModeState, ResolveInput, ResolveResult,
    ResolverRegistry,
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

#[cfg_attr(coverage_nightly, coverage(off))]
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
    let mut client_ext = crate::ExtensionMap::new();

    let result = registry.resolve_with_extensions(
        &mode,
        &key,
        &mut state,
        &keymap,
        &mut extensions,
        &mut client_ext,
    );
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
    let mut client_ext = crate::ExtensionMap::new();

    let result = registry.resolve_with_extensions(
        &mode,
        &key,
        &mut state,
        &keymap,
        &mut extensions,
        &mut client_ext,
    );
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
    let mut client_ext = crate::ExtensionMap::new();

    let result = registry.resolve_with_extensions(
        &op_pending,
        &key,
        &mut state,
        &keymap,
        &mut extensions,
        &mut client_ext,
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
    let mut client_ext = crate::ExtensionMap::new();
    let mut test_rt = reovim_driver_text_session::testing::TestSessionRuntime::new();
    let mut runtime = test_rt.runtime();

    let result = registry.resolve_with_session(
        &mode,
        &key,
        &mut state,
        &keymap,
        &mut runtime,
        &mut extensions,
        &mut client_ext,
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
    let mut client_ext = crate::ExtensionMap::new();
    let mut test_rt = reovim_driver_text_session::testing::TestSessionRuntime::new();
    let mut runtime = test_rt.runtime();

    let result = registry.resolve_with_session(
        &mode,
        &key,
        &mut state,
        &keymap,
        &mut runtime,
        &mut extensions,
        &mut client_ext,
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
    let mut client_ext = crate::ExtensionMap::new();
    let mut test_rt = reovim_driver_text_session::testing::TestSessionRuntime::new();
    let mut runtime = test_rt.runtime();

    let result = registry.resolve_with_session(
        &op_pending,
        &key,
        &mut state,
        &keymap,
        &mut runtime,
        &mut extensions,
        &mut client_ext,
    );
    assert!(result.is_some());
    assert!(matches!(result.unwrap(), ResolveResult::Pending));
}

// ========================================================================
// NotHandledResolver without parent (no fallback possible)
// ========================================================================

#[test]
fn test_resolve_with_keymap_not_handled_no_parent() {
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

    let registry = ResolverRegistry::new();
    let mode = visual_mode();

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

    let registry = ResolverRegistry::new();
    let mode = visual_mode();

    registry.register(NoParentResolver { mode: mode.clone() });

    let key = KeyEvent::new(crate::KeyCode::Char('x'));
    let mut state = ModeState::new(mode.clone());
    let keymap = NoOpKeymap;
    let mut extensions = crate::ExtensionMap::new();
    let mut client_ext = crate::ExtensionMap::new();

    let result = registry.resolve_with_extensions(
        &mode,
        &key,
        &mut state,
        &keymap,
        &mut extensions,
        &mut client_ext,
    );
    assert!(result.is_some());
    assert!(matches!(result.unwrap(), ResolveResult::NotHandled));
}

#[test]
fn test_resolve_with_session_not_handled_no_parent() {
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

    let registry = ResolverRegistry::new();
    let mode = visual_mode();

    registry.register(NoParentResolver { mode: mode.clone() });

    let key = KeyEvent::new(crate::KeyCode::Char('x'));
    let mut state = ModeState::new(mode.clone());
    let keymap = NoOpKeymap;
    let mut extensions = crate::ExtensionMap::new();
    let mut client_ext = crate::ExtensionMap::new();
    let mut test_rt = reovim_driver_text_session::testing::TestSessionRuntime::new();
    let mut runtime = test_rt.runtime();

    let result = registry.resolve_with_session(
        &mode,
        &key,
        &mut state,
        &keymap,
        &mut runtime,
        &mut extensions,
        &mut client_ext,
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
    let mut client_ext = crate::ExtensionMap::new();

    let result = registry.resolve_with_extensions(
        &op_pending,
        &key,
        &mut state,
        &keymap,
        &mut extensions,
        &mut client_ext,
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
    let mut client_ext = crate::ExtensionMap::new();
    let mut test_rt = reovim_driver_text_session::testing::TestSessionRuntime::new();
    let mut runtime = test_rt.runtime();

    let result = registry.resolve_with_session(
        &op_pending,
        &key,
        &mut state,
        &keymap,
        &mut runtime,
        &mut extensions,
        &mut client_ext,
    );
    assert!(result.is_none());
}

// ========================================================================
// pending_keys_for tests (#468)
// ========================================================================

#[test]
fn test_pending_keys_for_unknown_mode() {
    let registry = ResolverRegistry::new();
    let keys = registry.pending_keys_for(&normal_mode());
    assert!(keys.is_empty());
}

#[test]
fn test_pending_keys_for_resolver_with_default() {
    let registry = ResolverRegistry::new();
    registry.register(StubResolver::new(normal_mode()));
    // StubResolver doesn't override pending_keys(), so default (empty) is returned
    let keys = registry.pending_keys_for(&normal_mode());
    assert!(keys.is_empty());
}
