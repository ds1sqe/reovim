//! Module registry with dependency resolution and lifecycle management.
//!
//! Provides thread-safe module management with:
//! - Dependency resolution (topological sort via `depgraph`)
//! - Linux-style deferred probing (multi-pass retry)
//! - State tracking (FSM: Loaded -> Initializing -> Running -> Failed)
//! - Safe unload with reverse-dependency checks
//! - Reverse-order shutdown

use std::{
    collections::{HashMap, HashSet},
    path::Path,
    sync::Arc,
};

use {
    reovim_arch::sync::Mutex,
    reovim_depgraph::{DepEntry, resolve_dependencies},
    reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ModuleState},
};

use super::{handle::InitResult, loader::ModuleLoader};

/// Maximum retry passes for deferred modules (Linux uses ~8, we use 3).
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
    /// CRITICAL for safe unload — must check before removing.
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
    /// Returns `ModuleError::LoadFailed` if module with same ID already exists.
    pub fn register<M: Module>(&self, module: M) -> Result<ModuleId, ModuleError> {
        let mut inner = self.inner.lock();
        let id = inner.loader.register_static(module)?;
        inner.states.insert(id.clone(), ModuleState::Loaded);
        drop(inner);
        Ok(id)
    }

    /// Register a boxed module (does not initialize).
    ///
    /// # Errors
    ///
    /// Returns `ModuleError::LoadFailed` if module with same ID already exists.
    pub fn register_boxed(&self, module: Box<dyn Module>) -> Result<ModuleId, ModuleError> {
        let mut inner = self.inner.lock();
        let id = inner.loader.register_static_boxed(module)?;
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
    #[allow(unsafe_code)]
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
    #[allow(clippy::significant_drop_tightening)]
    pub fn init_all(&self, ctx: &ModuleContext) -> Result<(), ModuleError> {
        let mut inner = self.inner.lock();

        // 1. Resolve dependencies and build reverse dependency map
        let entries: Vec<DepEntry<ModuleId>> = inner
            .loader
            .modules
            .values()
            .map(|h| DepEntry {
                key: h.id().clone(),
                required: h.dependencies(),
                optional: h.optional_dependencies(),
                provides_caps: h.provides().to_vec(),
                requires_caps: h.requires().to_vec(),
            })
            .collect();

        let dep_order = resolve_dependencies(&entries)
            .map_err(|e| ModuleError::InitFailed(format!("dependency resolution failed: {e:?}")))?;

        inner.init_order = dep_order.order;
        inner.dependents = dep_order.dependents;

        // 2. First pass — try all modules
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
                    inner.states.insert(id, ModuleState::Failed(e.to_string()));
                }
            }
        }

        // 3. Retry deferred modules (Linux-style multi-pass)
        for pass in 1..=MAX_DEFER_PASSES {
            if deferred.is_empty() {
                break;
            }

            tracing::debug!(pass, count = deferred.len(), "retrying deferred modules");

            let mut still_deferred = Vec::new();
            for id in deferred {
                match inner.init_single(&id, ctx) {
                    Ok(InitResult::Success) => {
                        tracing::info!(module = %id, pass, "deferred -> success");
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

    /// Initialize single module.
    ///
    /// # Errors
    ///
    /// Returns error if module not found or init fails.
    pub fn init_module(&self, id: &ModuleId, ctx: &ModuleContext) -> Result<(), ModuleError> {
        let mut inner = self.inner.lock();
        inner.init_single(id, ctx).map(|_| ())
    }

    /// Call `on_all_loaded()` for every running module in init order.
    #[allow(clippy::significant_drop_tightening)]
    pub fn notify_all_loaded(&self, ctx: &ModuleContext) {
        let mut inner = self.inner.lock();
        let order = inner.init_order.clone();
        for id in &order {
            if inner.states.get(id) == Some(&ModuleState::Running)
                && let Some(handle) = inner.loader.modules.get_mut(id)
            {
                handle.on_all_loaded(ctx);
            }
        }
    }

    /// Get module state.
    #[must_use]
    pub fn state(&self, id: &ModuleId) -> Option<ModuleState> {
        let inner = self.inner.lock();
        inner.states.get(id).cloned()
    }

    /// Unload module (checks reverse dependencies first).
    ///
    /// The `unwrap()` on `deps.iter().next()` is safe because we already
    /// checked that `deps` is non-empty.
    ///
    /// # Errors
    ///
    /// Returns `ModuleError::InUse` if other modules depend on this one.
    /// Returns `ModuleError::NotLoaded` if module not found.
    #[allow(clippy::missing_panics_doc)]
    pub fn unload(&self, id: &ModuleId) -> Result<(), ModuleError> {
        let mut inner = self.inner.lock();

        // CRITICAL: Check if other modules depend on this one
        if let Some(deps) = inner.dependents.get(id)
            && !deps.is_empty()
        {
            return Err(ModuleError::InUse {
                module: id.clone(),
                // SAFETY: deps is non-empty (checked above)
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
    #[allow(clippy::significant_drop_tightening)]
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

    /// Atomically reload a dynamic module.
    ///
    /// This method holds the lock for the entire reload cycle to prevent
    /// TOCTOU races between checking dependents and performing the reload.
    ///
    /// The `unwrap()` on `deps.iter().next()` is safe because we already
    /// checked that `deps` is non-empty.
    ///
    /// # Safety
    ///
    /// Caller must ensure the shared library is ABI-compatible.
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Module has active dependents
    /// - Module is static (cannot reload)
    /// - Module path is unknown
    /// - Reload fails
    #[allow(
        unsafe_code,
        clippy::missing_panics_doc,
        clippy::significant_drop_tightening
    )]
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
                // SAFETY: deps is non-empty (checked above)
                by: deps.iter().next().unwrap().clone(),
            });
        }

        // 2. Get module path BEFORE unloading
        let path = inner
            .loader
            .modules
            .get(id)
            .and_then(|h| h.path().map(std::path::PathBuf::from))
            .ok_or_else(|| {
                ModuleError::LoadFailed("cannot reload static module or missing path".into())
            })?;

        // 2.5. Save state before unloading
        let saved_state = inner
            .loader
            .modules
            .get(id)
            .and_then(super::handle::ModuleHandle::save_state);
        if saved_state.is_some() {
            tracing::debug!(module = %id, "saved state for hot reload");
        }

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

        // 4.5. Validate ID didn't change
        if &new_id != id {
            inner.loader.modules.remove(&new_id);
            return Err(ModuleError::LoadFailed(format!(
                "module ID changed during reload: expected '{}', got '{}'",
                id.as_str(),
                new_id.as_str()
            )));
        }

        inner.states.insert(new_id.clone(), ModuleState::Loaded);

        // 5. Reinitialize
        match inner.init_single(&new_id, ctx) {
            Ok(InitResult::Success) => {
                // 5.5. Restore state after successful init
                if let Some(state) = saved_state
                    && let Some(handle) = inner.loader.modules.get_mut(&new_id)
                {
                    if let Err(e) = handle.restore_state(&state) {
                        tracing::warn!(
                            module = %new_id,
                            error = %e,
                            "failed to restore state after hot reload"
                        );
                    } else {
                        tracing::debug!(module = %new_id, "restored state after hot reload");
                    }
                }

                tracing::info!(module = %new_id, "hot reload successful");
                Ok(())
            }
            Ok(InitResult::Defer(reason)) => {
                tracing::warn!(module = %new_id, reason = %reason, "hot reload deferred");
                Err(ModuleError::InitFailed(format!("module deferred after reload: {reason}")))
            }
            Err(e) => {
                tracing::error!(module = %new_id, error = %e, "hot reload init failed");
                Err(e)
            }
        }
    }

    // ========================================================================
    // Accessors
    // ========================================================================

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
            .map_or_else(Vec::new, |s| s.iter().cloned().collect())
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
            .and_then(|h| h.path().map(std::path::PathBuf::from))
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
            ctx.services.clone(),
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
