//! Module registry with dependency resolution and lifecycle management.
//!
//! Provides thread-safe module management with:
//! - Dependency resolution (topological sort)
//! - Linux-style deferred probing
//! - State tracking
//! - Safe unload with dependency checks

use std::{
    collections::{HashMap, HashSet},
    path::Path,
    sync::Arc,
};

use {
    reovim_arch::sync::Mutex,
    reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ModuleState},
};

use super::{dependency::resolve_dependencies, handle::InitResult, loader::ModuleLoader};

/// Maximum retry passes for deferred modules (Linux uses ~8).
const MAX_DEFER_PASSES: usize = 3;

/// Module registry with dependency resolution.
///
/// All mutable state is wrapped in a single Mutex for thread safety.
/// This prevents data races on `init_order` and other fields.
pub struct ModuleRegistry {
    inner: Mutex<ModuleRegistryInner>,
}

/// Inner state protected by mutex.
struct ModuleRegistryInner {
    /// Module loader.
    loader: ModuleLoader,

    /// Module states.
    states: HashMap<ModuleId, ModuleState>,

    /// Initialization order (topologically sorted).
    init_order: Vec<ModuleId>,

    /// Reverse dependency map: module -> modules that depend on it.
    /// CRITICAL for safe unload - must check before removing.
    dependents: HashMap<ModuleId, HashSet<ModuleId>>,
}

impl Default for ModuleRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ModuleRegistry {
    /// Create new registry with default loader.
    #[must_use]
    pub fn new() -> Self {
        Self::with_loader(ModuleLoader::new())
    }

    /// Create registry with custom loader.
    #[must_use]
    pub fn with_loader(loader: ModuleLoader) -> Self {
        Self {
            inner: Mutex::new(ModuleRegistryInner {
                loader,
                states: HashMap::new(),
                init_order: Vec::new(),
                dependents: HashMap::new(),
            }),
        }
    }

    /// Create a thread-safe reference to this registry.
    #[must_use]
    pub fn into_arc(self) -> Arc<Self> {
        Arc::new(self)
    }

    /// Register a module (does not initialize).
    ///
    /// # Errors
    ///
    /// Returns error if module with same ID already exists.
    pub fn register<M: Module>(&self, module: M) -> Result<ModuleId, ModuleError> {
        let mut inner = self.inner.lock();
        let id = inner.loader.register_static(module)?;
        inner.states.insert(id.clone(), ModuleState::Loaded);
        drop(inner);
        Ok(id)
    }

    /// Load a dynamic module from path.
    ///
    /// # Safety
    ///
    /// Caller must ensure the shared library is ABI-compatible.
    ///
    /// # Errors
    ///
    /// Returns error if loading fails.
    pub unsafe fn load_dynamic(&self, path: &Path) -> Result<ModuleId, ModuleError> {
        let mut inner = self.inner.lock();
        // SAFETY: Caller ensures the shared library is ABI-compatible
        let id = unsafe { inner.loader.load_dynamic(path)? };
        inner.states.insert(id.clone(), ModuleState::Loaded);
        drop(inner);
        Ok(id)
    }

    /// Initialize all registered modules in dependency order.
    ///
    /// Uses Linux-style deferred probing with multi-pass retry.
    ///
    /// # Algorithm
    ///
    /// 1. First pass: try all modules in dependency order
    /// 2. If any defer, retry deferred modules
    /// 3. Repeat up to `MAX_DEFER_PASSES` times
    /// 4. If still deferred, mark as failed
    ///
    /// # Errors
    ///
    /// Returns error if dependency resolution fails.
    /// Individual module failures don't abort the entire process.
    pub fn init_all(&self, ctx: &ModuleContext) -> Result<(), ModuleError> {
        let mut inner = self.inner.lock();

        // 1. Resolve dependencies and build reverse dependency map
        let dep_order = resolve_dependencies(&inner.loader.modules)?;
        inner.init_order = dep_order.order;
        inner.dependents = dep_order.dependents;

        // 2. First pass - try all modules
        let mut deferred = Vec::new();
        for id in inner.init_order.clone() {
            match inner.init_single(&id, ctx) {
                Ok(InitResult::Success) => {
                    tracing::debug!(module = %id, "initialized");
                }
                Ok(InitResult::Defer(reason)) => {
                    tracing::info!(module = %id, reason = %reason, "deferred");
                    deferred.push(id);
                }
                Err(e) => {
                    tracing::error!(module = %id, error = %e, "init failed");
                    // Don't abort - continue with other modules
                    inner.states.insert(id, ModuleState::Failed(e.to_string()));
                }
            }
        }

        // 3. Retry deferred modules (Linux-style multi-pass)
        for pass in 1..=MAX_DEFER_PASSES {
            if deferred.is_empty() {
                break;
            }

            tracing::debug!(pass = pass, count = deferred.len(), "retrying deferred modules");

            let mut still_deferred = Vec::new();
            for id in deferred {
                match inner.init_single(&id, ctx) {
                    Ok(InitResult::Success) => {
                        tracing::info!(module = %id, pass = pass, "deferred -> success");
                    }
                    Ok(InitResult::Defer(_)) => {
                        still_deferred.push(id);
                    }
                    Err(e) => {
                        inner.states.insert(id, ModuleState::Failed(e.to_string()));
                    }
                }
            }
            deferred = still_deferred;
        }

        // 4. Mark remaining deferred as failed
        for id in deferred {
            tracing::error!(module = %id, passes = MAX_DEFER_PASSES, "permanently deferred");
            inner.states.insert(
                id,
                ModuleState::Failed(format!("deferred after {MAX_DEFER_PASSES} retry passes")),
            );
        }
        drop(inner);

        Ok(())
    }

    /// Initialize single module (resolves dependencies first).
    ///
    /// # Errors
    ///
    /// Returns error if module not found or init fails.
    pub fn init_module(&self, id: &ModuleId, ctx: &ModuleContext) -> Result<(), ModuleError> {
        let mut inner = self.inner.lock();
        inner.init_single(id, ctx).map(|_| ())
    }

    /// Get module state.
    #[must_use]
    pub fn state(&self, id: &ModuleId) -> Option<ModuleState> {
        let inner = self.inner.lock();
        inner.states.get(id).cloned()
    }

    /// Unload module (checks reverse dependencies first).
    ///
    /// # Errors
    ///
    /// Returns error if other modules depend on this one.
    pub fn unload(&self, id: &ModuleId) -> Result<(), ModuleError> {
        let mut inner = self.inner.lock();

        // CRITICAL: Check if other modules depend on this one
        if let Some(deps) = inner.dependents.get(id)
            && !deps.is_empty()
        {
            return Err(ModuleError::InUse {
                module: id.clone(),
                by: deps.iter().next().unwrap().clone(),
            });
        }

        // Call exit on module
        if let Some(handle) = inner.loader.modules.get_mut(id) {
            handle.exit()?;
        }

        // Remove from registry
        inner.loader.modules.remove(id);
        inner.states.remove(id);

        // Remove from dependents map
        inner.dependents.remove(id);

        // Remove this module from other modules' dependent lists
        for deps in inner.dependents.values_mut() {
            deps.remove(id);
        }
        drop(inner);

        tracing::info!(module = %id, "unloaded");
        Ok(())
    }

    /// Shutdown all modules in reverse initialization order.
    ///
    /// Always succeeds even if individual module exits fail (continues shutdown).
    pub fn shutdown(&self) {
        let mut inner = self.inner.lock();

        // Reverse order: last initialized = first shutdown
        let reverse_order: Vec<_> = inner.init_order.iter().rev().cloned().collect();

        for id in reverse_order {
            if inner.states.get(&id) == Some(&ModuleState::Running)
                && let Some(handle) = inner.loader.modules.get_mut(&id)
            {
                if let Err(e) = handle.exit() {
                    tracing::error!(module = %id, error = %e, "exit failed");
                    // Continue shutdown even if one module fails
                } else {
                    inner.states.insert(id, ModuleState::Loaded);
                }
            }
        }
        drop(inner);

        tracing::info!("module shutdown complete");
    }

    /// List all registered module IDs.
    #[must_use]
    pub fn registered_ids(&self) -> Vec<ModuleId> {
        let inner = self.inner.lock();
        inner.loader.modules.keys().cloned().collect()
    }

    /// List modules in initialization order.
    #[must_use]
    pub fn init_order(&self) -> Vec<ModuleId> {
        let inner = self.inner.lock();
        inner.init_order.clone()
    }

    /// Get modules that depend on the given module.
    #[must_use]
    pub fn dependents_of(&self, id: &ModuleId) -> Vec<ModuleId> {
        let inner = self.inner.lock();
        inner
            .dependents
            .get(id)
            .map(|s| s.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Get number of registered modules.
    #[must_use]
    pub fn len(&self) -> usize {
        let inner = self.inner.lock();
        inner.loader.len()
    }

    /// Check if no modules are registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        let inner = self.inner.lock();
        inner.loader.is_empty()
    }

    /// Discover modules in search paths.
    #[must_use]
    pub fn discover(&self) -> Vec<std::path::PathBuf> {
        let inner = self.inner.lock();
        inner.loader.discover()
    }

    /// Get the path of a dynamic module.
    #[must_use]
    pub fn module_path(&self, id: &ModuleId) -> Option<std::path::PathBuf> {
        let inner = self.inner.lock();
        inner
            .loader
            .modules
            .get(id)
            .and_then(|h| h.path().map(std::path::Path::to_path_buf))
    }

    /// Atomically reload a dynamic module.
    ///
    /// This method holds the lock for the entire reload cycle to prevent
    /// TOCTOU races between checking dependents and performing the reload.
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Module has active dependents
    /// - Module is static (cannot reload)
    /// - Module path is unknown
    /// - Reload fails
    ///
    /// # Safety
    ///
    /// Caller must ensure the shared library is ABI-compatible.
    pub unsafe fn reload_atomic(
        &self,
        id: &ModuleId,
        ctx: &ModuleContext,
    ) -> Result<(), ModuleError> {
        let mut inner = self.inner.lock();

        // 1. Check if module has dependents (atomic with unload)
        if let Some(deps) = inner.dependents.get(id)
            && !deps.is_empty()
        {
            return Err(ModuleError::InUse {
                module: id.clone(),
                by: deps.iter().next().unwrap().clone(),
            });
        }

        // 2. Get module path BEFORE unloading
        let path = inner
            .loader
            .modules
            .get(id)
            .and_then(|h| h.path().map(std::path::Path::to_path_buf))
            .ok_or_else(|| {
                ModuleError::LoadFailed("cannot reload static module or missing path".into())
            })?;

        // 3. Unload the module
        if let Some(handle) = inner.loader.modules.get_mut(id) {
            handle.exit()?;
        }
        inner.loader.modules.remove(id);
        inner.states.remove(id);
        inner.dependents.remove(id);
        for deps in inner.dependents.values_mut() {
            deps.remove(id);
        }

        // 4. Reload from same path
        // SAFETY: Caller ensures shared library is ABI-compatible
        let new_id = unsafe { inner.loader.load_dynamic(&path)? };
        inner.states.insert(new_id.clone(), ModuleState::Loaded);

        // 5. Reinitialize
        match inner.init_single(&new_id, ctx) {
            Ok(super::handle::InitResult::Success) => {
                tracing::info!(module = %new_id, "hot reload successful");
                Ok(())
            }
            Ok(super::handle::InitResult::Defer(reason)) => {
                tracing::warn!(module = %new_id, reason = %reason, "hot reload deferred");
                Err(ModuleError::InitFailed(format!("module deferred after reload: {reason}")))
            }
            Err(e) => {
                tracing::error!(module = %new_id, error = %e, "hot reload init failed");
                Err(e)
            }
        }
    }
}

impl ModuleRegistryInner {
    fn init_single(
        &mut self,
        id: &ModuleId,
        ctx: &ModuleContext,
    ) -> Result<InitResult, ModuleError> {
        // Validate state transition
        let current_state = self.states.get(id).cloned().unwrap_or_default();
        if !current_state.can_transition_to(&ModuleState::Initializing) {
            return Err(ModuleError::LoadFailed(format!(
                "cannot init module in {current_state:?} state"
            )));
        }

        self.states.insert(id.clone(), ModuleState::Initializing);

        let handle = self
            .loader
            .modules
            .get_mut(id)
            .ok_or_else(|| ModuleError::NotLoaded(id.clone()))?;

        // Create module-specific context with per-module directories
        let module_ctx = ModuleContext::new(
            ctx.kernel.clone(),
            ctx.data_dir.join(id.as_str()),
            ctx.cache_dir.join(id.as_str()),
        );

        // Ensure directories exist
        let _ = std::fs::create_dir_all(&module_ctx.data_dir);
        let _ = std::fs::create_dir_all(&module_ctx.cache_dir);

        match handle.init(&module_ctx) {
            Ok(InitResult::Success) => {
                self.states.insert(id.clone(), ModuleState::Running);
                Ok(InitResult::Success)
            }
            Ok(InitResult::Defer(reason)) => {
                // Keep in Loaded state for retry
                self.states.insert(id.clone(), ModuleState::Loaded);
                Ok(InitResult::Defer(reason))
            }
            Err(err) => {
                self.states
                    .insert(id.clone(), ModuleState::Failed(err.to_string()));
                Err(err)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        reovim_kernel::api::v1::{ProbeResult, Version},
        std::sync::atomic::{AtomicUsize, Ordering},
    };

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

    #[test]
    fn test_register_and_state() {
        let registry = ModuleRegistry::new();
        let id = registry.register(TestModule::new("test")).unwrap();

        assert_eq!(registry.state(&id), Some(ModuleState::Loaded));
    }

    #[test]
    fn test_init_all() {
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
    fn test_init_order() {
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
    fn test_unload_with_dependents_fails() {
        let registry = ModuleRegistry::new();
        registry.register(TestModule::new("a")).unwrap();
        registry
            .register(TestModule::with_deps("b", &["a"]))
            .unwrap();

        let ctx = ModuleContext::default();
        registry.init_all(&ctx).unwrap();

        // Try to unload A while B depends on it
        let result = registry.unload(&ModuleId::new("a"));
        assert!(matches!(result, Err(ModuleError::InUse { .. })));
    }

    #[test]
    fn test_unload_without_dependents() {
        let registry = ModuleRegistry::new();
        registry.register(TestModule::new("a")).unwrap();

        let ctx = ModuleContext::default();
        registry.init_all(&ctx).unwrap();

        let result = registry.unload(&ModuleId::new("a"));
        assert!(result.is_ok());
        assert!(registry.state(&ModuleId::new("a")).is_none());
    }

    #[test]
    fn test_dependents_of() {
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
    fn test_deferred_probing() {
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

        // Should be Running after retry
        assert_eq!(registry.state(&ModuleId::new("deferring")), Some(ModuleState::Running));
        // init should have been called twice
        assert_eq!(INIT_COUNT.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn test_shutdown() {
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

    #[test]
    fn test_len_and_is_empty() {
        let registry = ModuleRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);

        registry.register(TestModule::new("a")).unwrap();
        assert!(!registry.is_empty());
        assert_eq!(registry.len(), 1);
    }
}
