//! Module loader for static and dynamic modules.
//!
//! Handles loading modules from shared libraries and registering
//! static (compile-time linked) modules.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use {
    libloading::{Library, Symbol},
    reovim_kernel::api::v1::{
        API_VERSION, Module, ModuleError, ModuleId, ModuleProbe, Version, is_compatible,
    },
};

#[cfg(feature = "python")]
use reovim_driver_ffi_python::PythonModule;

use super::{
    discovery::{default_search_paths, discover_modules, find_module},
    handle::{
        ApiVersionPtrFn, DestroyFn, EntryFn, ExitFn, FfiSymbols, FreeStateFn, InitFn, ModuleHandle,
        ProbeFn, ProbePtrFn, RestoreStateFn, SaveStateFn, SupportsHotReloadFn,
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
    /// Returns error if module with same ID already exists.
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
    /// Returns error if module with same ID already exists.
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
    #[allow(clippy::too_many_lines)]
    pub unsafe fn load_dynamic(&mut self, path: &Path) -> Result<ModuleId, ModuleError> {
        // SAFETY: All operations in this function require the caller to ensure
        // the shared library is ABI-compatible.
        unsafe {
            // 1. Load shared library
            let library = Library::new(path).map_err(|e| ModuleError::LoadFailed(e.to_string()))?;

            // 2. Check API version FIRST
            // Try static symbol first (fast), then fallback to pointer function (Haskell)
            let module_api = if let Ok(api_version) =
                library.get::<&'static Version>(b"REOVIM_MODULE_API_VERSION")
            {
                // NOTE: Macro generates `pub static`, so Symbol returns a reference
                **api_version // Double dereference: Symbol -> & -> Version
            } else if let Ok(api_version_ptr_fn) =
                library.get::<ApiVersionPtrFn>(b"reovim_module_api_version_ptr")
            {
                // Fallback for languages that can't export static data (e.g., Haskell)
                let mut version = Version::new(0, 0, 0);
                api_version_ptr_fn(&raw mut version);
                version
            } else {
                return Err(ModuleError::NoEntryPoint(
                    "neither REOVIM_MODULE_API_VERSION nor reovim_module_api_version_ptr found"
                        .to_string(),
                ));
            };

            // is_compatible(required, provided) - module requires, kernel provides
            if !is_compatible(module_api, API_VERSION) {
                return Err(ModuleError::IncompatibleVersion {
                    module: (module_api.major, module_api.minor),
                    kernel: (API_VERSION.major, API_VERSION.minor),
                });
            }

            // 3. Get module probe (metadata without instantiation)
            // Try return-by-value first, then fallback to pointer (Haskell)
            let probe = if let Ok(probe_fn) = library.get::<ProbeFn>(b"reovim_module_probe") {
                probe_fn()
            } else if let Ok(probe_ptr_fn) = library.get::<ProbePtrFn>(b"reovim_module_probe") {
                // Fallback for languages that can't return structs by value (e.g., Haskell)
                // SAFETY: ModuleProbe is repr(C) and all-zero is a valid state
                let mut probe: ModuleProbe = std::mem::zeroed();
                probe_ptr_fn(&raw mut probe);
                probe
            } else {
                return Err(ModuleError::NoEntryPoint("reovim_module_probe not found".to_string()));
            };
            let id = ModuleId::from_string(probe.id_str().to_owned());

            // 4. Check if already loaded
            if self.modules.contains_key(&id) {
                return Err(ModuleError::LoadFailed(format!(
                    "module '{}' already loaded",
                    id.as_str()
                )));
            }

            // 5. Get all FFI symbols BEFORE creating instance
            let entry_fn: Symbol<EntryFn> = library
                .get(b"reovim_module_entry")
                .map_err(|e| ModuleError::NoEntryPoint(e.to_string()))?;

            let init_fn: Symbol<InitFn> = library
                .get(b"reovim_module_init")
                .map_err(|e| ModuleError::NoEntryPoint(e.to_string()))?;

            let exit_fn: Symbol<ExitFn> = library
                .get(b"reovim_module_exit")
                .map_err(|e| ModuleError::NoEntryPoint(e.to_string()))?;

            let destroy_fn: Symbol<DestroyFn> = library
                .get(b"reovim_module_destroy")
                .map_err(|e| ModuleError::NoEntryPoint(e.to_string()))?;

            // 5b. Optional hot reload symbols
            let supports_hot_reload_fn: Option<Symbol<SupportsHotReloadFn>> =
                library.get(b"reovim_module_supports_hot_reload").ok();
            let save_state_fn: Option<Symbol<SaveStateFn>> =
                library.get(b"reovim_module_save_state").ok();
            let restore_state_fn: Option<Symbol<RestoreStateFn>> =
                library.get(b"reovim_module_restore_state").ok();
            let free_state_fn: Option<Symbol<FreeStateFn>> =
                library.get(b"reovim_module_free_state").ok();

            // 6. Create module instance (returns OPAQUE thin pointer)
            let module_ptr = entry_fn();
            if module_ptr.is_null() {
                return Err(ModuleError::InitFailed("entry returned null".into()));
            }

            // CRITICAL: Do NOT cast to *mut dyn Module!
            // Fat pointers cannot cross FFI boundaries.
            // We keep the opaque pointer and use trampolines for all operations.

            // 7. Cache FFI symbols (avoid repeated lookup)
            let ffi = FfiSymbols {
                init: *init_fn,
                exit: *exit_fn,
                destroy: *destroy_fn,
                supports_hot_reload: supports_hot_reload_fn.map(|s| *s),
                save_state: save_state_fn.map(|s| *s),
                restore_state: restore_state_fn.map(|s| *s),
                free_state: free_state_fn.map(|s| *s),
            };

            // 8. Create handle with OPAQUE pointer
            let handle =
                ModuleHandle::from_dynamic(library, path.to_path_buf(), &probe, module_ptr, ffi);

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
    /// Returns error if module not found or loading fails.
    pub unsafe fn load_by_name(&mut self, name: &str) -> Result<ModuleId, ModuleError> {
        let path = find_module(&self.search_paths, name)
            .ok_or_else(|| ModuleError::NotFound(name.into()))?;

        // SAFETY: Caller ensures libraries are ABI-compatible
        unsafe { self.load_dynamic(&path) }
    }

    /// Unload a module.
    ///
    /// Note: This only removes from the loader. Use `ModuleManager` for
    /// proper dependency-aware unloading.
    ///
    /// # Errors
    ///
    /// Returns error if module not found.
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

    /// Load Python module from path.
    ///
    /// Executes the Python file, finds a class inheriting from `Module`,
    /// instantiates it, and wraps it in `PythonModule`.
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Python fails to execute the file
    /// - No `Module` subclass is found
    /// - Module with same ID already loaded
    #[cfg(feature = "python")]
    pub fn load_python(&mut self, path: &Path) -> Result<ModuleId, ModuleError> {
        use std::ffi::CString;

        use pyo3::{ffi::c_str, prelude::*, types::PyDict};

        Python::attach(|py| {
            // 1. Read the Python file
            let code = std::fs::read_to_string(path).map_err(|e| {
                ModuleError::LoadFailed(format!("failed to read {}: {e}", path.display()))
            })?;

            // 2. Create a module name from the file
            let module_name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("python_module");

            // 3. Convert strings to CString for PyO3 0.27
            let code_cstring = CString::new(code)
                .map_err(|e| ModuleError::LoadFailed(format!("invalid Python code: {e}")))?;
            let filename_cstring = CString::new(path.to_string_lossy().as_bytes())
                .map_err(|e| ModuleError::LoadFailed(format!("invalid filename: {e}")))?;
            let module_name_cstring = CString::new(module_name)
                .map_err(|e| ModuleError::LoadFailed(format!("invalid module name: {e}")))?;

            // 4. Create a new Python module and execute the code in it
            let py_module = pyo3::types::PyModule::from_code(
                py,
                &code_cstring,
                &filename_cstring,
                &module_name_cstring,
            )
            .map_err(|e| ModuleError::LoadFailed(format!("Python execution failed: {e}")))?;

            // 5. Import our base Module class from the reovim module
            let reovim_module = py.import(c_str!("reovim")).map_err(|e| {
                ModuleError::LoadFailed(format!("failed to import reovim module: {e}"))
            })?;
            let base_module_class = reovim_module
                .getattr(c_str!("Module"))
                .map_err(|e| ModuleError::LoadFailed(format!("failed to get Module class: {e}")))?;

            // 6. Find user's subclass of Module
            let builtins = py
                .import(c_str!("builtins"))
                .map_err(|e| ModuleError::LoadFailed(format!("failed to import builtins: {e}")))?;
            let issubclass_fn = builtins
                .getattr(c_str!("issubclass"))
                .map_err(|e| ModuleError::LoadFailed(format!("failed to get issubclass: {e}")))?;
            let isinstance_fn = builtins
                .getattr(c_str!("isinstance"))
                .map_err(|e| ModuleError::LoadFailed(format!("failed to get isinstance: {e}")))?;
            let type_class = builtins
                .getattr(c_str!("type"))
                .map_err(|e| ModuleError::LoadFailed(format!("failed to get type: {e}")))?;

            let module_dict = py_module.getattr(c_str!("__dict__")).map_err(|e| {
                ModuleError::LoadFailed(format!("failed to get module __dict__: {e}"))
            })?;
            let dict: &Bound<'_, PyDict> = module_dict
                .cast()
                .map_err(|e| ModuleError::LoadFailed(format!("__dict__ is not a dict: {e}")))?;

            let mut module_class: Option<Bound<'_, PyAny>> = None;
            for (key, value) in dict.iter() {
                let name: String = key
                    .extract::<String>()
                    .map_err(|e| ModuleError::LoadFailed(format!("failed to extract key: {e}")))?;

                // Skip private names
                if name.starts_with('_') {
                    continue;
                }

                // Check if it's a class (isinstance(value, type))
                let is_class: bool = isinstance_fn
                    .call1((value.clone(), type_class.clone()))
                    .and_then(|r| r.extract::<bool>())
                    .unwrap_or(false);

                if !is_class {
                    continue;
                }

                // Check if it's a subclass of our Module (but not Module itself)
                let is_subclass: bool = issubclass_fn
                    .call1((value.clone(), base_module_class.clone()))
                    .and_then(|r| r.extract::<bool>())
                    .unwrap_or(false);

                // Make sure it's not the base Module class itself
                let is_base = value.is(&base_module_class);

                if is_subclass && !is_base {
                    module_class = Some(value);
                    break;
                }
            }

            let module_class = module_class.ok_or_else(|| {
                ModuleError::LoadFailed(format!("no Module subclass found in {}", path.display()))
            })?;

            // 7. Instantiate the module class
            let instance = module_class.call0().map_err(|e| {
                ModuleError::LoadFailed(format!("failed to instantiate module: {e}"))
            })?;

            // 8. Wrap in PythonModule
            let python_module =
                PythonModule::new(instance.unbind(), Some(path.to_string_lossy().to_string()));

            // 9. Register using the existing boxed registration
            self.register_static_boxed(Box::new(python_module))
        })
    }

    /// Load Python module by name (searches paths).
    ///
    /// # Errors
    ///
    /// Returns error if module not found or loading fails.
    #[cfg(feature = "python")]
    pub fn load_python_by_name(&mut self, name: &str) -> Result<ModuleId, ModuleError> {
        let filename = format!("{name}.py");

        for search_path in &self.search_paths {
            let full_path = search_path.join(&filename);
            if full_path.exists() {
                return self.load_python(&full_path);
            }
        }

        Err(ModuleError::NotFound(name.into()))
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        reovim_kernel::api::v1::{ModuleContext, ProbeResult, Version},
    };

    struct TestModule {
        initialized: bool,
    }

    impl Module for TestModule {
        fn id(&self) -> ModuleId {
            ModuleId::new("test-module")
        }

        fn name(&self) -> &'static str {
            "Test Module"
        }

        fn version(&self) -> Version {
            Version::new(1, 0, 0)
        }

        fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult {
            self.initialized = true;
            ProbeResult::Success
        }

        fn exit(&mut self) -> Result<(), ModuleError> {
            self.initialized = false;
            Ok(())
        }
    }

    #[test]
    fn test_static_registration() {
        let mut loader = ModuleLoader::new();
        let id = loader
            .register_static(TestModule { initialized: false })
            .unwrap();
        assert_eq!(id.as_str(), "test-module");
        assert!(loader.get(&id).is_some());
    }

    #[test]
    fn test_duplicate_registration_fails() {
        let mut loader = ModuleLoader::new();
        loader
            .register_static(TestModule { initialized: false })
            .unwrap();

        let result = loader.register_static(TestModule { initialized: false });
        assert!(matches!(result, Err(ModuleError::LoadFailed(_))));
    }

    #[test]
    fn test_unload() {
        let mut loader = ModuleLoader::new();
        let id = loader
            .register_static(TestModule { initialized: false })
            .unwrap();
        loader.unload(&id).unwrap();
        assert!(loader.get(&id).is_none());
    }

    #[test]
    fn test_unload_not_found() {
        let mut loader = ModuleLoader::new();
        let result = loader.unload(&ModuleId::new("nonexistent"));
        assert!(matches!(result, Err(ModuleError::NotLoaded(_))));
    }

    #[test]
    fn test_search_paths() {
        let loader = ModuleLoader::new();
        assert!(!loader.search_paths().is_empty());
    }

    #[test]
    fn test_add_search_path() {
        let mut loader = ModuleLoader::new();
        let new_path = PathBuf::from("/custom/modules");
        loader.add_search_path(new_path.clone());
        assert!(loader.search_paths().contains(&new_path));
    }

    #[test]
    fn test_loaded_ids() {
        let mut loader = ModuleLoader::new();
        loader
            .register_static(TestModule { initialized: false })
            .unwrap();

        let ids: Vec<_> = loader.loaded_ids().collect();
        assert_eq!(ids.len(), 1);
        assert_eq!(ids[0].as_str(), "test-module");
    }

    #[test]
    fn test_len_and_is_empty() {
        let mut loader = ModuleLoader::new();
        assert!(loader.is_empty());
        assert_eq!(loader.len(), 0);

        loader
            .register_static(TestModule { initialized: false })
            .unwrap();

        assert!(!loader.is_empty());
        assert_eq!(loader.len(), 1);
    }

    // ========================================================================
    // Python Loading Tests (require python feature + Python runtime)
    // ========================================================================
    //
    // These tests require:
    // 1. The `python` feature enabled
    // 2. Python 3.11+ installed
    // 3. Proper linking against Python library
    //
    // All tests are marked #[ignore] because they require Python runtime setup.
    // Run with: cargo test -p reovim --features python -- --ignored

    #[cfg(feature = "python")]
    mod python_tests {
        use std::io::Write;

        use {super::*, tempfile::NamedTempFile};

        /// Test loading a non-existent file.
        #[test]
        #[ignore = "requires Python runtime linked"]
        fn test_load_python_file_not_found() {
            let mut loader = ModuleLoader::new();
            let result = loader.load_python(Path::new("/nonexistent/module.py"));
            assert!(matches!(result, Err(ModuleError::LoadFailed(_))));
        }

        /// Test loading by name when module doesn't exist.
        #[test]
        #[ignore = "requires Python runtime linked"]
        fn test_load_python_by_name_not_found() {
            let mut loader = ModuleLoader::new();
            let result = loader.load_python_by_name("nonexistent-module");
            assert!(matches!(result, Err(ModuleError::NotFound(_))));
        }

        /// Test loading an empty Python file (no Module subclass).
        #[test]
        #[ignore = "requires Python runtime linked"]
        fn test_load_python_empty_file() {
            let mut file = NamedTempFile::with_suffix(".py").unwrap();
            writeln!(file, "# Empty module").unwrap();

            let mut loader = ModuleLoader::new();
            let result = loader.load_python(file.path());

            // Should fail because no Module subclass found
            assert!(
                matches!(result, Err(ModuleError::LoadFailed(msg)) if msg.contains("no Module subclass"))
            );
        }

        /// Test loading a Python file with syntax error.
        #[test]
        #[ignore = "requires Python runtime linked"]
        fn test_load_python_syntax_error() {
            let mut file = NamedTempFile::with_suffix(".py").unwrap();
            writeln!(file, "def invalid syntax {{}}").unwrap();

            let mut loader = ModuleLoader::new();
            let result = loader.load_python(file.path());

            // Should fail with execution error
            assert!(matches!(result, Err(ModuleError::LoadFailed(_))));
        }

        /// Full integration test - loads a valid Python module.
        ///
        /// Requires:
        /// 1. Python 3.11+ installed
        /// 2. The reovim Python module built and importable
        ///    (set PYTHONPATH to include the built .so file)
        #[test]
        #[ignore = "requires Python runtime and reovim module importable"]
        fn test_load_valid_python_module() {
            let mut file = NamedTempFile::with_suffix(".py").unwrap();
            writeln!(
                file,
                r#"
from reovim import Module, ModuleId, ProbeResult

class TestPythonModule(Module):
    def id(self):
        return ModuleId("test-python-module")

    def name(self):
        return "Test Python Module"

    def version(self):
        return (1, 0, 0)

    def init(self, ctx):
        return ProbeResult.Success

    def exit(self):
        pass
"#
            )
            .unwrap();

            let mut loader = ModuleLoader::new();
            let result = loader.load_python(file.path());

            assert!(result.is_ok(), "Failed to load: {:?}", result.err());
            let id = result.unwrap();
            assert_eq!(id.as_str(), "test-python-module");

            // Verify the module is registered
            let handle = loader.get(&id);
            assert!(handle.is_some());
            assert!(handle.unwrap().is_static()); // Python modules use boxed registration
        }
    }
}
