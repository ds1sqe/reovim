use std::sync::atomic::{AtomicUsize, Ordering};

use reovim_kernel::api::v1::{
    Module, ModuleContext, ModuleError, ModuleId, ModuleState, ProbeResult, Version,
};

use super::registry::ModuleRegistry;

// ============================================================================
// Test modules
// ============================================================================

struct TestModule {
    id: &'static str,
    deps: Vec<&'static str>,
}

impl TestModule {
    fn new(id: &'static str) -> Self {
        Self {
            id,
            deps: Vec::new(),
        }
    }

    fn with_deps(id: &'static str, deps: &[&'static str]) -> Self {
        Self {
            id,
            deps: deps.to_vec(),
        }
    }
}

impl Module for TestModule {
    fn id(&self) -> ModuleId {
        ModuleId::new(self.id)
    }
    fn name(&self) -> &'static str {
        self.id
    }
    fn version(&self) -> Version {
        Version::new(1, 0, 0)
    }
    fn dependencies(&self) -> Vec<ModuleId> {
        self.deps.iter().map(|s| ModuleId::new(s)).collect()
    }
    fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult {
        ProbeResult::Success
    }
    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }
}

struct FailModule;

impl Module for FailModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("fail-module")
    }
    fn name(&self) -> &'static str {
        "Fail Module"
    }
    fn version(&self) -> Version {
        Version::new(1, 0, 0)
    }
    fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult {
        ProbeResult::Failed(ModuleError::InitFailed("deliberate".into()))
    }
    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }
}

// ============================================================================
// new / default
// ============================================================================

#[test]
fn new_registry_is_empty() {
    let registry = ModuleRegistry::new();
    assert!(registry.is_empty());
    assert_eq!(registry.len(), 0);
}

#[test]
fn default_registry_is_empty() {
    let registry = ModuleRegistry::default();
    assert!(registry.is_empty());
}

// ============================================================================
// register
// ============================================================================

#[test]
fn register_sets_loaded_state() {
    let registry = ModuleRegistry::new();
    let id = registry.register(TestModule::new("test")).unwrap();
    assert_eq!(registry.state(&id), Some(ModuleState::Loaded));
    assert_eq!(registry.len(), 1);
}

#[test]
fn register_duplicate_fails() {
    let registry = ModuleRegistry::new();
    registry.register(TestModule::new("test")).unwrap();
    let result = registry.register(TestModule::new("test"));
    assert!(matches!(result, Err(ModuleError::LoadFailed(_))));
}

#[test]
fn register_boxed_sets_loaded_state() {
    let registry = ModuleRegistry::new();
    let module: Box<dyn Module> = Box::new(TestModule::new("test"));
    let id = registry.register_boxed(module).unwrap();
    assert_eq!(registry.state(&id), Some(ModuleState::Loaded));
}

// ============================================================================
// init_all
// ============================================================================

#[test]
fn init_all_sets_running_state() {
    let registry = ModuleRegistry::new();
    registry.register(TestModule::new("a")).unwrap();
    registry
        .register(TestModule::with_deps("b", &["a"]))
        .unwrap();

    let ctx = ModuleContext::default();
    registry.init_all(&ctx).unwrap();

    assert_eq!(registry.state(&ModuleId::new("a")), Some(ModuleState::Running));
    assert_eq!(registry.state(&ModuleId::new("b")), Some(ModuleState::Running));
}

#[test]
fn init_all_preserves_dependency_order() {
    let registry = ModuleRegistry::new();
    registry.register(TestModule::new("a")).unwrap();
    registry
        .register(TestModule::with_deps("b", &["a"]))
        .unwrap();
    registry
        .register(TestModule::with_deps("c", &["b"]))
        .unwrap();

    let ctx = ModuleContext::default();
    registry.init_all(&ctx).unwrap();

    let order = registry.init_order();
    let a_idx = order.iter().position(|id| id.as_str() == "a").unwrap();
    let b_idx = order.iter().position(|id| id.as_str() == "b").unwrap();
    let c_idx = order.iter().position(|id| id.as_str() == "c").unwrap();

    assert!(a_idx < b_idx);
    assert!(b_idx < c_idx);
}

#[test]
fn init_all_handles_failed_module() {
    let registry = ModuleRegistry::new();
    registry.register(TestModule::new("good")).unwrap();
    registry.register(FailModule).unwrap();

    let ctx = ModuleContext::default();
    registry.init_all(&ctx).unwrap();

    assert_eq!(registry.state(&ModuleId::new("good")), Some(ModuleState::Running));
    assert!(matches!(
        registry.state(&ModuleId::new("fail-module")),
        Some(ModuleState::Failed(_))
    ));
}

// ============================================================================
// deferred probing
// ============================================================================

#[test]
fn deferred_probing_retries() {
    static INIT_COUNT: AtomicUsize = AtomicUsize::new(0);

    struct DeferringModule;

    impl Module for DeferringModule {
        fn id(&self) -> ModuleId {
            ModuleId::new("deferring")
        }
        fn name(&self) -> &'static str {
            "Deferring"
        }
        fn version(&self) -> Version {
            Version::new(1, 0, 0)
        }
        fn init(&mut self, _: &ModuleContext) -> ProbeResult {
            let count = INIT_COUNT.fetch_add(1, Ordering::SeqCst);
            if count == 0 {
                ProbeResult::Defer("not ready".into())
            } else {
                ProbeResult::Success
            }
        }
        fn exit(&mut self) -> Result<(), ModuleError> {
            Ok(())
        }
    }

    INIT_COUNT.store(0, Ordering::SeqCst);

    let registry = ModuleRegistry::new();
    registry.register(DeferringModule).unwrap();

    let ctx = ModuleContext::default();
    registry.init_all(&ctx).unwrap();

    assert_eq!(registry.state(&ModuleId::new("deferring")), Some(ModuleState::Running));
    // init called twice: first deferred, second succeeded
    assert_eq!(INIT_COUNT.load(Ordering::SeqCst), 2);
}

#[test]
fn permanently_deferred_marked_failed() {
    struct AlwaysDeferModule;

    impl Module for AlwaysDeferModule {
        fn id(&self) -> ModuleId {
            ModuleId::new("always-defer")
        }
        fn name(&self) -> &'static str {
            "Always Defer"
        }
        fn version(&self) -> Version {
            Version::new(1, 0, 0)
        }
        fn init(&mut self, _: &ModuleContext) -> ProbeResult {
            ProbeResult::Defer("never ready".into())
        }
        fn exit(&mut self) -> Result<(), ModuleError> {
            Ok(())
        }
    }

    let registry = ModuleRegistry::new();
    registry.register(AlwaysDeferModule).unwrap();

    let ctx = ModuleContext::default();
    registry.init_all(&ctx).unwrap();

    assert!(matches!(
        registry.state(&ModuleId::new("always-defer")),
        Some(ModuleState::Failed(_))
    ));
}

// ============================================================================
// unload
// ============================================================================

#[test]
fn unload_with_dependents_fails() {
    let registry = ModuleRegistry::new();
    registry.register(TestModule::new("a")).unwrap();
    registry
        .register(TestModule::with_deps("b", &["a"]))
        .unwrap();

    let ctx = ModuleContext::default();
    registry.init_all(&ctx).unwrap();

    let result = registry.unload(&ModuleId::new("a"));
    assert!(matches!(result, Err(ModuleError::InUse { .. })));
}

#[test]
fn unload_without_dependents_succeeds() {
    let registry = ModuleRegistry::new();
    registry.register(TestModule::new("a")).unwrap();

    let ctx = ModuleContext::default();
    registry.init_all(&ctx).unwrap();

    let result = registry.unload(&ModuleId::new("a"));
    assert!(result.is_ok());
    assert!(registry.state(&ModuleId::new("a")).is_none());
    assert!(registry.is_empty());
}

#[test]
fn unload_leaf_module_succeeds() {
    let registry = ModuleRegistry::new();
    registry.register(TestModule::new("a")).unwrap();
    registry
        .register(TestModule::with_deps("b", &["a"]))
        .unwrap();

    let ctx = ModuleContext::default();
    registry.init_all(&ctx).unwrap();

    // Unload leaf (b has no dependents)
    assert!(registry.unload(&ModuleId::new("b")).is_ok());

    // Now a can be unloaded too
    assert!(registry.unload(&ModuleId::new("a")).is_ok());
}

// ============================================================================
// shutdown
// ============================================================================

#[test]
fn shutdown_transitions_to_loaded() {
    let registry = ModuleRegistry::new();
    registry.register(TestModule::new("a")).unwrap();
    registry
        .register(TestModule::with_deps("b", &["a"]))
        .unwrap();

    let ctx = ModuleContext::default();
    registry.init_all(&ctx).unwrap();

    registry.shutdown();

    assert_eq!(registry.state(&ModuleId::new("a")), Some(ModuleState::Loaded));
    assert_eq!(registry.state(&ModuleId::new("b")), Some(ModuleState::Loaded));
}

// ============================================================================
// accessors
// ============================================================================

#[test]
fn registered_ids_returns_all() {
    let registry = ModuleRegistry::new();
    registry.register(TestModule::new("a")).unwrap();
    registry.register(TestModule::new("b")).unwrap();

    let mut ids: Vec<_> = registry
        .registered_ids()
        .iter()
        .map(ModuleId::as_str)
        .map(String::from)
        .collect();
    ids.sort_unstable();
    assert_eq!(ids, vec!["a", "b"]);
}

#[test]
fn dependents_of_returns_dependents() {
    let registry = ModuleRegistry::new();
    registry.register(TestModule::new("a")).unwrap();
    registry
        .register(TestModule::with_deps("b", &["a"]))
        .unwrap();
    registry
        .register(TestModule::with_deps("c", &["a"]))
        .unwrap();

    let ctx = ModuleContext::default();
    registry.init_all(&ctx).unwrap();

    let deps = registry.dependents_of(&ModuleId::new("a"));
    assert_eq!(deps.len(), 2);
}

#[test]
fn dependents_of_unknown_returns_empty() {
    let registry = ModuleRegistry::new();
    assert!(registry.dependents_of(&ModuleId::new("nope")).is_empty());
}

#[test]
fn state_unknown_module_returns_none() {
    let registry = ModuleRegistry::new();
    assert!(registry.state(&ModuleId::new("nope")).is_none());
}

#[test]
fn into_arc_works() {
    let registry = ModuleRegistry::new();
    let arc = registry.into_arc();
    assert!(arc.is_empty());
}

// ============================================================================
// init_module
// ============================================================================

#[test]
fn init_module_single() {
    let registry = ModuleRegistry::new();
    let id = registry.register(TestModule::new("single")).unwrap();

    let ctx = ModuleContext::default();
    registry.init_module(&id, &ctx).unwrap();

    assert_eq!(registry.state(&id), Some(ModuleState::Running));
}

// ============================================================================
// notify_all_loaded
// ============================================================================

#[test]
fn notify_all_loaded_does_not_panic() {
    let registry = ModuleRegistry::new();
    registry.register(TestModule::new("a")).unwrap();

    let ctx = ModuleContext::default();
    registry.init_all(&ctx).unwrap();
    registry.notify_all_loaded(&ctx);
    // No panic = success
}
