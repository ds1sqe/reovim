//! Module loader for static and dynamic modules.
//!
//! Handles loading modules from shared libraries and registering
//! static (compile-time linked) modules. Uses `libloading` for
//! cross-platform dynamic library loading.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use {
    reovim_dylib_loader::{Library, Symbol},
    reovim_kernel::api::v1::{API_VERSION, Module, ModuleError, ModuleId, Version, is_compatible},
};

use super::{
    discovery::{default_search_paths, discover_modules, find_module},
    handle::{
        DestroyFn, EntryFn, ExitFn, FfiSymbols, FreeStateFn, InitFn, ModuleHandle, OnAllLoadedFn,
        ProbeFn, RestoreStateFn, SaveStateFn, SupportsHotReloadFn,
    },
};

/// Module loader supporting static and dynamic loading.
pub struct ModuleLoader {
    /// Loaded modules by ID.
    pub(crate) modules: HashMap<ModuleId, ModuleHandle>,

    /// Search paths for module discovery.
    search_paths: Vec<PathBuf>,
}

impl Default for ModuleLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl ModuleLoader {
    /// Create new loader with default search paths.
    #[must_use]
    pub fn new() -> Self {
        Self {
            modules: HashMap::new(),
            search_paths: default_search_paths(),
        }
    }

    /// Add custom search path.
    pub fn add_search_path(&mut self, path: PathBuf) {
        if !self.search_paths.contains(&path) {
            self.search_paths.push(path);
        }
    }

    /// Register a static (compile-time linked) module.
    ///
    /// # Errors
    ///
    /// Returns `ModuleError::LoadFailed` if module with same ID already exists.
    pub fn register_static<M: Module>(&mut self, module: M) -> Result<ModuleId, ModuleError> {
        let id = module.id();

        if self.modules.contains_key(&id) {
            return Err(ModuleError::LoadFailed(format!(
                "module '{}' already loaded",
                id.as_str()
            )));
        }

        let handle = ModuleHandle::from_static(module);
        self.modules.insert(id.clone(), handle);
        tracing::debug!(module = %id, "registered static module");
        Ok(id)
    }

    /// Register a boxed module.
    ///
    /// Use this when you have a `Box<dyn Module>` instead of a concrete type.
    ///
    /// # Errors
    ///
    /// Returns `ModuleError::LoadFailed` if module with same ID already exists.
    pub fn register_static_boxed(
        &mut self,
        module: Box<dyn Module>,
    ) -> Result<ModuleId, ModuleError> {
        let id = module.id();

        if self.modules.contains_key(&id) {
            return Err(ModuleError::LoadFailed(format!(
                "module '{}' already loaded",
                id.as_str()
            )));
        }

        let handle = ModuleHandle::from_boxed(module);
        self.modules.insert(id.clone(), handle);
        tracing::debug!(module = %id, "registered boxed module");
        Ok(id)
    }

    /// Insert an already-loaded module handle.
    ///
    /// Used when another subsystem bootstraps modules first and then hands
    /// ownership of the live handles to the registry.
    ///
    /// # Errors
    ///
    /// Returns `ModuleError::LoadFailed` if a module with the same ID already exists.
    pub fn insert_handle(&mut self, handle: ModuleHandle) -> Result<ModuleId, ModuleError> {
        let id = handle.id().clone();

        if self.modules.contains_key(&id) {
            return Err(ModuleError::LoadFailed(format!(
                "module '{}' already loaded",
                id.as_str()
            )));
        }

        self.modules.insert(id.clone(), handle);
        tracing::debug!(module = %id, "inserted existing module handle");
        Ok(id)
    }

    /// Load dynamic module from path.
    ///
    /// # Safety
    ///
    /// Caller must ensure the shared library is ABI-compatible and was
    /// built with matching `declare_module!` macro.
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Library fails to load
    /// - Required symbols are missing
    /// - API version is incompatible
    /// - Module with same ID already loaded
    // Requires a real .so with declare_module! FFI exports.
    #[cfg_attr(coverage_nightly, coverage(off))]
    #[allow(unsafe_code)]
    pub unsafe fn load_dynamic(&mut self, path: &Path) -> Result<ModuleId, ModuleError> {
        // SAFETY: All operations in this function require the caller to ensure
        // the shared library is ABI-compatible.
        unsafe {
            // 1. Load shared library via the OS-abstraction wrapper.
            let library =
                Library::open(path).map_err(|e| ModuleError::LoadFailed(e.to_string()))?;

            // 2. Check API version FIRST (static symbol, fast)
            let api_version: Symbol<&'static Version> = library
                .symbol(b"REOVIM_MODULE_API_VERSION")
                .map_err(|e| ModuleError::NoEntryPoint(e.to_string()))?;

            let module_api = **api_version;

            // is_compatible(required, provided) — module requires, kernel provides
            if !is_compatible(module_api, API_VERSION) {
                return Err(ModuleError::IncompatibleVersion {
                    module: (module_api.major, module_api.minor),
                    kernel: (API_VERSION.major, API_VERSION.minor),
                });
            }

            // 3. Get module probe (metadata without instantiation)
            let probe_fn: Symbol<ProbeFn> = library
                .symbol(b"reovim_module_probe")
                .map_err(|e| ModuleError::NoEntryPoint(e.to_string()))?;

            let probe = probe_fn();
            let id = ModuleId::from_string(probe.id_str().to_owned());

            // 4. Check if already loaded
            if self.modules.contains_key(&id) {
                return Err(ModuleError::LoadFailed(format!(
                    "module '{}' already loaded",
                    id.as_str()
                )));
            }

            // 5. Resolve required FFI symbols
            let entry_fn: Symbol<EntryFn> = library
                .symbol(b"reovim_module_entry")
                .map_err(|e| ModuleError::NoEntryPoint(e.to_string()))?;

            let init_fn: Symbol<InitFn> = library
                .symbol(b"reovim_module_init")
                .map_err(|e| ModuleError::NoEntryPoint(e.to_string()))?;

            let exit_fn: Symbol<ExitFn> = library
                .symbol(b"reovim_module_exit")
                .map_err(|e| ModuleError::NoEntryPoint(e.to_string()))?;

            let destroy_fn: Symbol<DestroyFn> = library
                .symbol(b"reovim_module_destroy")
                .map_err(|e| ModuleError::NoEntryPoint(e.to_string()))?;

            // 5b. Optional hot reload symbols
            let supports_hot_reload_fn: Option<Symbol<SupportsHotReloadFn>> =
                library.symbol(b"reovim_module_supports_hot_reload").ok();
            let save_state_fn: Option<Symbol<SaveStateFn>> =
                library.symbol(b"reovim_module_save_state").ok();
            let restore_state_fn: Option<Symbol<RestoreStateFn>> =
                library.symbol(b"reovim_module_restore_state").ok();
            let free_state_fn: Option<Symbol<FreeStateFn>> =
                library.symbol(b"reovim_module_free_state").ok();

            // 5c. Optional lifecycle hook (#725) — absent in pre-#725 `.so`
            // files, which is why it is resolved with `.ok()` rather than
            // treated as a hard failure.
            let on_all_loaded_fn: Option<Symbol<OnAllLoadedFn>> =
                library.symbol(b"reovim_module_on_all_loaded").ok();

            // 6. Create module instance (returns OPAQUE thin pointer)
            let module_ptr = entry_fn();
            if module_ptr.is_null() {
                return Err(ModuleError::InitFailed("entry returned null".into()));
            }

            // CRITICAL: Do NOT cast to *mut dyn Module!
            // Fat pointers cannot cross FFI boundaries.
            // We keep the opaque pointer and use trampolines for all operations.

            // 7. Cache FFI symbols (avoid repeated dlsym lookup)
            let ffi = FfiSymbols {
                init: *init_fn,
                exit: *exit_fn,
                destroy: *destroy_fn,
                supports_hot_reload: supports_hot_reload_fn.map(|s| *s),
                save_state: save_state_fn.map(|s| *s),
                restore_state: restore_state_fn.map(|s| *s),
                free_state: free_state_fn.map(|s| *s),
                on_all_loaded: on_all_loaded_fn.map(|s| *s),
            };

            // 8. Create handle with opaque pointer
            let handle =
                ModuleHandle::from_dynamic(library, path.to_path_buf(), probe, module_ptr, ffi);

            tracing::info!(
                module = %id,
                path = %path.display(),
                "loaded dynamic module"
            );

            self.modules.insert(id.clone(), handle);
            Ok(id)
        }
    }

    /// Load module by name (searches paths).
    ///
    /// # Safety
    ///
    /// Caller must ensure shared libraries are ABI-compatible.
    ///
    /// # Errors
    ///
    /// Returns `ModuleError::NotFound` if not found, or loading errors.
    #[allow(unsafe_code)]
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub unsafe fn load_by_name(&mut self, name: &str) -> Result<ModuleId, ModuleError> {
        let path = find_module(&self.search_paths, name)
            .ok_or_else(|| ModuleError::NotFound(name.into()))?;

        // SAFETY: Caller ensures libraries are ABI-compatible
        unsafe { self.load_dynamic(&path) }
    }

    /// Scan `<root>/modules/` for cdylibs and load every valid module
    /// into `self`. Returns one per-entry result per candidate file —
    /// successes yield the loaded [`ModuleId`], failures yield a
    /// [`ModuleError`] whose text folds in the underlying
    /// `reovim_dylib_loader::ScanEntryError` when the scan layer
    /// refused the file.
    ///
    /// The scan is infallible at the API level: a malformed cdylib,
    /// unreadable file, or missing `<root>/modules/` directory never
    /// panics — it lands as an `Err` on the corresponding entry.
    /// System fallback paths (XDG, `/usr/lib/reovim/`) are not
    /// consulted; the caller names the single search root.
    ///
    /// # Shape
    ///
    /// Plan 02 called for a `LoadedModule::from_path_scan` returning
    /// `Vec<Result<Self, ScanEntryError>>` symmetric with the client
    /// side's [`crate::LoadedClientRender::from_path_scan`]. Server
    /// modules cannot own themselves: a [`super::handle::ModuleHandle`]
    /// carries FFI symbol pointers plus the `Library` handle that
    /// keeps the cdylib mapped, and the registry layer depends on
    /// those handles living inside `ModuleLoader`'s own `HashMap`
    /// for the `ModuleId` → handle lookup to work. The method
    /// therefore registers each successful module into `&mut self`
    /// and returns the resulting `ModuleId`. The client-side type is
    /// a plain owned struct so direct-ownership return works; this
    /// divergence falls out of existing ownership, not a new rule.
    ///
    /// # Safety
    ///
    /// Same contract as [`Self::load_dynamic`]: the caller must ensure
    /// every `.so` under `<root>/modules/` is ABI-compatible with the
    /// host kernel.
    ///
    // TODO(#769-phase-1.5): hoist a shared `PathScannable` trait so
    // this impl and `LoadedClientRender::from_path_scan` share one
    // signature instead of two parallel ones.
    #[allow(unsafe_code)]
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub unsafe fn from_path_scan(&mut self, root: &Path) -> Vec<Result<ModuleId, ModuleError>> {
        use reovim_dylib_loader::{Kind, PathResolverBuilder, scan_paths};

        let resolver = PathResolverBuilder::for_kind(Kind::Module)
            .without_system_fallback()
            .push_cli_path(root.join(Kind::Module.subdir()))
            .build();

        scan_paths(resolver.paths())
            .into_entries()
            .into_iter()
            .map(|entry| match entry.outcome {
                // SAFETY: the caller's contract on `from_path_scan`
                // forwards to `load_dynamic`; ABI compatibility is
                // owned by the caller.
                Ok(_lib) => unsafe { self.load_dynamic(&entry.path) },
                Err(e) => Err(ModuleError::LoadFailed(e.to_string())),
            })
            .collect()
    }

    /// Unload a module.
    ///
    /// Note: This only removes from the loader. Use `ModuleRegistry` for
    /// proper dependency-aware unloading.
    ///
    /// # Errors
    ///
    /// Returns `ModuleError::NotLoaded` if module not found.
    pub fn unload(&mut self, id: &ModuleId) -> Result<ModuleHandle, ModuleError> {
        self.modules
            .remove(id)
            .ok_or_else(|| ModuleError::NotLoaded(id.clone()))
    }

    /// Get module handle by ID.
    #[must_use]
    pub fn get(&self, id: &ModuleId) -> Option<&ModuleHandle> {
        self.modules.get(id)
    }

    /// Get mutable module handle by ID.
    #[must_use]
    pub fn get_mut(&mut self, id: &ModuleId) -> Option<&mut ModuleHandle> {
        self.modules.get_mut(id)
    }

    /// Take ownership of a module handle, removing it from the loader (#725).
    ///
    /// Unlike [`Self::unload`], this returns an `Option` so callers can
    /// distinguish "not present" from "error". Used by bootstrap to move
    /// external module handles into the unified `tracked` init list.
    ///
    /// After calling `take`, the returned handle is the sole owner of the
    /// underlying `libloading::Library`; dropping it runs the dynamic
    /// module's `destroy` trampoline before unloading the `.so`.
    pub fn take(&mut self, id: &ModuleId) -> Option<ModuleHandle> {
        self.modules.remove(id)
    }

    /// List all loaded module IDs.
    pub fn loaded_ids(&self) -> impl Iterator<Item = &ModuleId> {
        self.modules.keys()
    }

    /// Get number of loaded modules.
    #[must_use]
    pub fn len(&self) -> usize {
        self.modules.len()
    }

    /// Check if no modules are loaded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.modules.is_empty()
    }

    /// Discover modules in search paths.
    ///
    /// Returns paths to potential module files (does not load them).
    #[must_use]
    pub fn discover(&self) -> Vec<PathBuf> {
        discover_modules(&self.search_paths)
    }

    /// Get search paths.
    #[must_use]
    pub fn search_paths(&self) -> &[PathBuf] {
        &self.search_paths
    }
}
