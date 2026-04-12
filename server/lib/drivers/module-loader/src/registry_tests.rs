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
fn init_module_rebuilds_dependents_for_incremental_runtime_loads() {
    let registry = ModuleRegistry::new();
    registry.register(TestModule::new("a")).unwrap();
    registry
        .register(TestModule::with_deps("b", &["a"]))
        .unwrap();

    let ctx = ModuleContext::default();
    registry.init_module(&ModuleId::new("a"), &ctx).unwrap();
    registry.init_module(&ModuleId::new("b"), &ctx).unwrap();

    let result = registry.unload(&ModuleId::new("a"));
    assert!(matches!(
        result,
        Err(ModuleError::InUse { module, by })
            if module == ModuleId::new("a") && by == ModuleId::new("b")
    ));
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

// ============================================================================
// init_module — error paths
// ============================================================================

#[test]
fn init_module_already_running_fails() {
    let registry = ModuleRegistry::new();
    let id = registry.register(TestModule::new("m")).unwrap();

    let ctx = ModuleContext::default();
    registry.init_module(&id, &ctx).unwrap();

    // Module is now Running; cannot transition to Initializing again
    let result = registry.init_module(&id, &ctx);
    assert!(result.is_err());
}

#[test]
fn init_module_not_found_fails() {
    let registry = ModuleRegistry::new();
    let ctx = ModuleContext::default();
    let result = registry.init_module(&ModuleId::new("nonexistent"), &ctx);
    assert!(result.is_err());
}

#[test]
fn init_module_fail_module_records_failed_state() {
    let registry = ModuleRegistry::new();
    let id = registry.register(FailModule).unwrap();

    let ctx = ModuleContext::default();
    let result = registry.init_module(&id, &ctx);
    assert!(result.is_err());
    assert!(matches!(registry.state(&id), Some(ModuleState::Failed(_))));
}

// ============================================================================
// notify_all_loaded — skip non-running
// ============================================================================

#[test]
fn notify_all_loaded_skips_non_running() {
    let registry = ModuleRegistry::new();
    registry.register(TestModule::new("good")).unwrap();
    registry.register(FailModule).unwrap();

    let ctx = ModuleContext::default();
    registry.init_all(&ctx).unwrap();

    // "fail-module" is Failed, "good" is Running
    // notify_all_loaded should skip the failed module without panic
    registry.notify_all_loaded(&ctx);
}

// ============================================================================
// shutdown — exit failure continues
// ============================================================================

#[test]
fn shutdown_continues_on_exit_failure() {
    struct ExitFailModule;

    impl Module for ExitFailModule {
        fn id(&self) -> ModuleId {
            ModuleId::new("exit-fail")
        }
        fn name(&self) -> &'static str {
            "Exit Fail"
        }
        fn version(&self) -> Version {
            Version::new(1, 0, 0)
        }
        fn init(&mut self, _: &ModuleContext) -> ProbeResult {
            ProbeResult::Success
        }
        fn exit(&mut self) -> Result<(), ModuleError> {
            Err(ModuleError::InitFailed("exit failed".into()))
        }
    }

    let registry = ModuleRegistry::new();
    registry.register(TestModule::new("ok")).unwrap();
    registry.register(ExitFailModule).unwrap();

    let ctx = ModuleContext::default();
    registry.init_all(&ctx).unwrap();

    // Shutdown should not panic even when exit() fails
    registry.shutdown();

    // "ok" transitions to Loaded; "exit-fail" stays Running because exit() failed
    assert_eq!(registry.state(&ModuleId::new("ok")), Some(ModuleState::Loaded));
    assert_eq!(registry.state(&ModuleId::new("exit-fail")), Some(ModuleState::Running));
}

#[test]
fn shutdown_skips_non_running_modules() {
    let registry = ModuleRegistry::new();
    registry.register(TestModule::new("loaded-only")).unwrap();

    let ctx = ModuleContext::default();
    // init_all sets init_order but "loaded-only" transitions to Running.
    // Let's just register and not init — shutdown should skip Loaded modules.
    // But init_order is empty if we don't call init_all.
    // Call init_all, then shutdown, then shutdown again — second time they're Loaded.
    registry.init_all(&ctx).unwrap();
    registry.shutdown();
    // Now all are Loaded — second shutdown is a no-op
    registry.shutdown();

    assert_eq!(registry.state(&ModuleId::new("loaded-only")), Some(ModuleState::Loaded));
}

// ============================================================================
// deferred probing — retry pass with failure
// ============================================================================

#[test]
fn deferred_then_fail_on_retry() {
    static CALL_COUNT: AtomicUsize = AtomicUsize::new(0);

    struct DeferThenFail;

    impl Module for DeferThenFail {
        fn id(&self) -> ModuleId {
            ModuleId::new("defer-then-fail")
        }
        fn name(&self) -> &'static str {
            "DeferThenFail"
        }
        fn version(&self) -> Version {
            Version::new(1, 0, 0)
        }
        fn init(&mut self, _: &ModuleContext) -> ProbeResult {
            let count = CALL_COUNT.fetch_add(1, Ordering::SeqCst);
            if count == 0 {
                ProbeResult::Defer("not ready".into())
            } else {
                ProbeResult::Failed(ModuleError::InitFailed("failed on retry".into()))
            }
        }
        fn exit(&mut self) -> Result<(), ModuleError> {
            Ok(())
        }
    }

    CALL_COUNT.store(0, Ordering::SeqCst);

    let registry = ModuleRegistry::new();
    registry.register(DeferThenFail).unwrap();

    let ctx = ModuleContext::default();
    registry.init_all(&ctx).unwrap();

    assert!(matches!(
        registry.state(&ModuleId::new("defer-then-fail")),
        Some(ModuleState::Failed(_))
    ));
}

// ============================================================================
// module_path / discover
// ============================================================================

#[test]
fn module_path_static_returns_none() {
    let registry = ModuleRegistry::new();
    let id = registry.register(TestModule::new("static-mod")).unwrap();
    assert!(registry.module_path(&id).is_none());
}

#[test]
fn module_path_unknown_returns_none() {
    let registry = ModuleRegistry::new();
    assert!(registry.module_path(&ModuleId::new("nope")).is_none());
}

#[test]
fn discover_returns_vec() {
    let registry = ModuleRegistry::new();
    // discover scans default search paths; result depends on environment
    // but should not panic
    let _paths = registry.discover();
}

// ============================================================================
// register_boxed — duplicate
// ============================================================================

#[test]
fn register_boxed_duplicate_fails() {
    let registry = ModuleRegistry::new();
    let m1: Box<dyn Module> = Box::new(TestModule::new("dup"));
    let m2: Box<dyn Module> = Box::new(TestModule::new("dup"));
    registry.register_boxed(m1).unwrap();
    let result = registry.register_boxed(m2);
    assert!(matches!(result, Err(ModuleError::LoadFailed(_))));
}

// ============================================================================
// unload — module not in registry
// ============================================================================

#[test]
fn unload_nonexistent_module() {
    let registry = ModuleRegistry::new();
    let ctx = ModuleContext::default();
    // Register and init so dependents map exists.
    registry.register(TestModule::new("x")).unwrap();
    registry.init_all(&ctx).unwrap();

    let unknown = ModuleId::new("unknown");
    let result = registry.unload(&unknown);
    assert!(matches!(result, Err(ModuleError::NotLoaded(id)) if id == unknown));
}

// ============================================================================
// unload — MC/DC branch coverage
// ============================================================================

/// Branch 332:1 — dependents map has an entry for the module but the set is
/// empty (a former dependent was already unloaded).  The guard
/// `if let Some(deps) && !deps.is_empty()` must NOT return `InUse`, allowing
/// execution to fall through to the exit call on line 342-344.
#[test]
fn unload_with_empty_dependents_set_succeeds() {
    let registry = ModuleRegistry::new();
    registry.register(TestModule::new("a")).unwrap();
    registry
        .register(TestModule::with_deps("b", &["a"]))
        .unwrap();

    let ctx = ModuleContext::default();
    registry.init_all(&ctx).unwrap();

    // Unload the leaf first: this removes "b" from "a"'s dependents set,
    // leaving dependents["a"] = {} (Some but empty).
    registry.unload(&ModuleId::new("b")).unwrap();

    // Now "a"'s dependents entry exists but is empty — branch 332:1 is taken
    // (Some matches) and the !is_empty() guard is false, so we fall through.
    // Lines 342-344 are also exercised: handle exists and exit() is called.
    let result = registry.unload(&ModuleId::new("a"));
    assert!(result.is_ok(), "expected Ok, got {result:?}");
    assert!(registry.state(&ModuleId::new("a")).is_none());
}

/// Branch 342:1 and line 344 — the module is present in the modules map so
/// `get_mut` returns Some, and `handle.exit()` is called successfully.
/// Uses a module with a custom exit to confirm the call is actually reached.
#[test]
fn unload_calls_exit_on_module() {
    use std::sync::atomic::{AtomicBool, Ordering};

    static EXIT_CALLED: AtomicBool = AtomicBool::new(false);

    struct ExitTracker;

    impl Module for ExitTracker {
        fn id(&self) -> ModuleId {
            ModuleId::new("exit-tracker")
        }
        fn name(&self) -> &'static str {
            "ExitTracker"
        }
        fn version(&self) -> Version {
            Version::new(1, 0, 0)
        }
        fn init(&mut self, _: &ModuleContext) -> ProbeResult {
            ProbeResult::Success
        }
        fn exit(&mut self) -> Result<(), ModuleError> {
            EXIT_CALLED.store(true, Ordering::SeqCst);
            Ok(())
        }
    }

    EXIT_CALLED.store(false, Ordering::SeqCst);

    let registry = ModuleRegistry::new();
    registry.register(ExitTracker).unwrap();

    let ctx = ModuleContext::default();
    registry.init_all(&ctx).unwrap();

    registry.unload(&ModuleId::new("exit-tracker")).unwrap();

    assert!(EXIT_CALLED.load(Ordering::SeqCst), "exit() was not called during unload");
}
