//! `PythonModule` wrapper implementing the kernel `Module` trait.
//!
//! This module provides the bridge between Python module classes and the
//! Rust module system. `PythonModule` wraps a Python object and implements
//! the `Module` trait by delegating to Python methods.

use std::sync::{Arc, OnceLock};

use {
    pyo3::{prelude::*, types::PyDict},
    reovim_kernel::api::v1::{
        CommandRegistration, EventHandlerRegistration, KeybindingRegistration, Module,
        ModuleContext, ModuleError, ModuleId, ProbeResult, Version,
    },
    tracing::{debug, error, warn},
};

use crate::types::{
    PyCommandRegistration, PyEventHandlerRegistration, PyKeybindingRegistration, PyModuleId,
    PyProbeResult, PyVersion,
};

// ============================================================================
// PyModuleContext - Python-visible context
// ============================================================================

/// Python-visible module context.
///
/// Provides a simplified view of `ModuleContext` for Python modules.
#[pyclass(name = "ModuleContext", module = "reovim")]
#[derive(Clone)]
pub struct PyModuleContext {
    /// Path to module's data directory.
    #[pyo3(get)]
    pub data_dir: String,
    /// Path to module's cache directory.
    #[pyo3(get)]
    pub cache_dir: String,
    /// Loaded optional dependencies.
    optional_deps: Vec<String>,
}

#[pymethods]
impl PyModuleContext {
    /// Check if an optional dependency is available.
    #[must_use]
    pub fn has_optional_dep(&self, id: &str) -> bool {
        self.optional_deps.iter().any(|dep| dep == id)
    }

    /// Get all loaded optional dependencies.
    #[must_use]
    pub fn optional_deps(&self) -> Vec<String> {
        self.optional_deps.clone()
    }

    fn __repr__(&self) -> String {
        format!("ModuleContext(data_dir='{}', cache_dir='{}')", self.data_dir, self.cache_dir)
    }
}

impl PyModuleContext {
    /// Create from kernel `ModuleContext`.
    #[must_use]
    pub fn from_kernel(ctx: &ModuleContext) -> Self {
        Self {
            data_dir: ctx.data_dir.to_string_lossy().to_string(),
            cache_dir: ctx.cache_dir.to_string_lossy().to_string(),
            optional_deps: ctx
                .optional_deps()
                .iter()
                .map(|id| id.as_str().to_string())
                .collect(),
        }
    }
}

// ============================================================================
// Cached Module Metadata
// ============================================================================

/// Cached identity metadata from Python module.
///
/// These values are read once and cached for performance,
/// since they shouldn't change during the module's lifetime.
struct CachedMetadata {
    id: ModuleId,
    name: String,
    version: Version,
    api_version: Version,
}

// ============================================================================
// PythonModule
// ============================================================================

/// A Rust wrapper around a Python module instance.
///
/// Implements the kernel `Module` trait by delegating to Python methods.
/// The GIL is acquired for each method call to Python.
///
/// # Thread Safety
///
/// `PythonModule` is `Send + Sync` because:
/// - The `Py<PyAny>` is wrapped in `Arc` for shared ownership
/// - All Python calls acquire the GIL before accessing the object
/// - Cached metadata uses `OnceLock` for thread-safe initialization
///
/// # Example
///
/// ```ignore
/// use pyo3::prelude::*;
/// use reovim_driver_ffi_python::PythonModule;
///
/// // Load a Python module class and instantiate it
/// Python::attach(|py| {
///     let code = r#"
/// from reovim import Module, ModuleId, ProbeResult
///
/// class MyModule(Module):
///     def id(self):
///         return ModuleId("my-module")
///     def name(self):
///         return "My Module"
///     def version(self):
///         return (1, 0, 0)
///     def init(self, ctx):
///         return ProbeResult.Success
///     def exit(self):
///         pass
/// "#;
///     // ... create PythonModule from MyModule instance
/// });
/// ```
pub struct PythonModule {
    /// The Python module instance.
    py_object: Arc<Py<PyAny>>,
    /// Path to the source file (for debugging/hot reload).
    source_path: Option<String>,
    /// Cached metadata (populated on first access).
    metadata: OnceLock<CachedMetadata>,
}

// SAFETY: Py<PyAny> is thread-safe when accessed through the GIL.
// All Python calls in PythonModule acquire the GIL first via Python::attach().
#[allow(unsafe_code)]
unsafe impl Send for PythonModule {}
#[allow(unsafe_code)]
unsafe impl Sync for PythonModule {}

impl PythonModule {
    /// Create a new `PythonModule` from a Python object.
    ///
    /// The object should be an instance of a class that inherits from `Module`.
    #[must_use]
    pub fn new(py_object: Py<PyAny>, source_path: Option<String>) -> Self {
        Self {
            py_object: Arc::new(py_object),
            source_path,
            metadata: OnceLock::new(),
        }
    }

    /// Get the source file path if available.
    #[must_use]
    pub fn source_path(&self) -> Option<&str> {
        self.source_path.as_deref()
    }

    /// Get or initialize cached metadata.
    fn get_metadata(&self) -> &CachedMetadata {
        self.metadata.get_or_init(|| self.read_metadata())
    }

    /// Read metadata from Python object.
    fn read_metadata(&self) -> CachedMetadata {
        Python::attach(|py| {
            let obj = self.py_object.bind(py);

            // Read id
            let id = match obj.call_method0("id") {
                Ok(result) => {
                    if let Ok(py_id) = result.extract::<PyModuleId>() {
                        py_id.0
                    } else if let Ok(id_str) = result.extract::<String>() {
                        ModuleId::from_string(id_str)
                    } else {
                        warn!("Python module id() returned unexpected type");
                        ModuleId::from_string("unknown".to_string())
                    }
                }
                Err(e) => {
                    error!("Failed to call Python module id(): {}", e);
                    ModuleId::from_string("unknown".to_string())
                }
            };

            // Read name
            let name = match obj.call_method0("name") {
                Ok(result) => result
                    .extract::<String>()
                    .unwrap_or_else(|_| "Unknown".to_string()),
                Err(e) => {
                    error!("Failed to call Python module name(): {}", e);
                    "Unknown".to_string()
                }
            };

            // Read version
            let version = match obj.call_method0("version") {
                Ok(result) => Self::extract_version(&result).unwrap_or_else(|| {
                    warn!("Python module version() returned unexpected type");
                    Version::new(0, 0, 0)
                }),
                Err(e) => {
                    error!("Failed to call Python module version(): {e}");
                    Version::new(0, 0, 0)
                }
            };

            // Read api_version
            let api_version = obj
                .call_method0("api_version")
                .ok()
                .and_then(|r| Self::extract_version(&r))
                .unwrap_or(reovim_kernel::api::v1::API_VERSION);

            CachedMetadata {
                id,
                name,
                version,
                api_version,
            }
        })
    }

    /// Extract a Version from a Python result.
    fn extract_version(result: &Bound<'_, PyAny>) -> Option<Version> {
        result.extract::<PyVersion>().map(|v| v.0).ok().or_else(|| {
            result
                .extract::<(u32, u32, u32)>()
                .map(|(major, minor, patch)| Version::new(major, minor, patch))
                .ok()
        })
    }

    /// Extract module IDs from a Python list.
    fn extract_module_ids(obj: &Bound<'_, PyAny>) -> Vec<ModuleId> {
        obj.extract::<Vec<PyModuleId>>()
            .map(|ids| ids.into_iter().map(|id| id.0).collect())
            .or_else(|_| {
                obj.extract::<Vec<String>>()
                    .map(|strings| strings.into_iter().map(ModuleId::from_string).collect())
            })
            .unwrap_or_default()
    }
}

impl Module for PythonModule {
    // ========================================================================
    // Identity (cached)
    // ========================================================================

    fn id(&self) -> ModuleId {
        self.get_metadata().id.clone()
    }

    fn name(&self) -> &'static str {
        // SAFETY: We leak the string to get a 'static lifetime.
        // This is acceptable because:
        // 1. Module names are small and there are few modules
        // 2. Modules typically live for the entire program lifetime
        let name = self.get_metadata().name.clone();
        Box::leak(name.into_boxed_str())
    }

    fn version(&self) -> Version {
        self.get_metadata().version
    }

    fn api_version(&self) -> Version {
        self.get_metadata().api_version
    }

    // ========================================================================
    // Dependencies (not cached - may change between loads)
    // ========================================================================

    fn dependencies(&self) -> Vec<ModuleId> {
        Python::attach(|py| {
            let obj = self.py_object.bind(py);
            match obj.call_method0("dependencies") {
                Ok(result) => Self::extract_module_ids(&result),
                Err(e) => {
                    debug!("Python module dependencies() failed: {}", e);
                    Vec::new()
                }
            }
        })
    }

    fn optional_dependencies(&self) -> Vec<ModuleId> {
        Python::attach(|py| {
            let obj = self.py_object.bind(py);
            match obj.call_method0("optional_dependencies") {
                Ok(result) => Self::extract_module_ids(&result),
                Err(e) => {
                    debug!("Python module optional_dependencies() failed: {}", e);
                    Vec::new()
                }
            }
        })
    }

    // ========================================================================
    // Lifecycle
    // ========================================================================

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        Python::attach(|py| {
            let obj = self.py_object.bind(py);
            let py_ctx = PyModuleContext::from_kernel(ctx);

            // Convert to Python dict for easier access
            let ctx_dict = PyDict::new(py);
            ctx_dict
                .set_item("data_dir", &py_ctx.data_dir)
                .expect("set data_dir");
            ctx_dict
                .set_item("cache_dir", &py_ctx.cache_dir)
                .expect("set cache_dir");
            ctx_dict
                .set_item("optional_deps", py_ctx.optional_deps())
                .expect("set optional_deps");

            match obj.call_method1("init", (ctx_dict,)) {
                Ok(result) => result
                    .extract::<PyProbeResult>()
                    .map_or(ProbeResult::Success, |py_result| py_result.to_kernel()),
                Err(e) => {
                    error!("Python module init() raised exception: {e}");
                    ProbeResult::Failed(ModuleError::InitFailed(format!("Python exception: {e}")))
                }
            }
        })
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        Python::attach(|py| {
            let obj = self.py_object.bind(py);
            match obj.call_method0("exit") {
                Ok(_) => Ok(()),
                Err(e) => {
                    error!("Python module exit() raised exception: {e}");
                    Err(ModuleError::InitFailed(format!("Python exception: {e}")))
                }
            }
        })
    }

    // ========================================================================
    // Registrations
    // ========================================================================

    fn commands(&self) -> Vec<CommandRegistration> {
        Python::attach(|py| {
            let obj = self.py_object.bind(py);
            match obj.call_method0("commands") {
                Ok(result) => {
                    // Try to extract as list of PyCommandRegistration
                    if let Ok(py_regs) = result.extract::<Vec<PyCommandRegistration>>() {
                        return py_regs
                            .iter()
                            .map(PyCommandRegistration::to_kernel)
                            .collect();
                    }
                    // Fallback: empty list if method returns None or wrong type
                    if result.is_none() {
                        return Vec::new();
                    }
                    debug!("Python module commands() returned unexpected type for {}", self.id());
                    Vec::new()
                }
                Err(e) => {
                    // Method not implemented or raised exception - that's OK, use default
                    debug!("Python module commands() not available: {}", e);
                    Vec::new()
                }
            }
        })
    }

    fn keybindings(&self) -> Vec<KeybindingRegistration> {
        Python::attach(|py| {
            let obj = self.py_object.bind(py);
            match obj.call_method0("keybindings") {
                Ok(result) => {
                    // Try to extract as list of PyKeybindingRegistration
                    if let Ok(py_regs) = result.extract::<Vec<PyKeybindingRegistration>>() {
                        return py_regs
                            .iter()
                            .map(PyKeybindingRegistration::to_kernel)
                            .collect();
                    }
                    // Fallback: empty list if method returns None or wrong type
                    if result.is_none() {
                        return Vec::new();
                    }
                    debug!(
                        "Python module keybindings() returned unexpected type for {}",
                        self.id()
                    );
                    Vec::new()
                }
                Err(e) => {
                    // Method not implemented or raised exception - that's OK, use default
                    debug!("Python module keybindings() not available: {}", e);
                    Vec::new()
                }
            }
        })
    }

    fn event_handlers(&self) -> Vec<EventHandlerRegistration> {
        Python::attach(|py| {
            let obj = self.py_object.bind(py);
            match obj.call_method0("event_handlers") {
                Ok(result) => {
                    // Try to extract as list of PyEventHandlerRegistration
                    if let Ok(py_regs) = result.extract::<Vec<PyEventHandlerRegistration>>() {
                        return py_regs
                            .iter()
                            .map(PyEventHandlerRegistration::to_kernel)
                            .collect();
                    }
                    // Fallback: empty list if method returns None or wrong type
                    if result.is_none() {
                        return Vec::new();
                    }
                    debug!(
                        "Python module event_handlers() returned unexpected type for {}",
                        self.id()
                    );
                    Vec::new()
                }
                Err(e) => {
                    // Method not implemented or raised exception - that's OK, use default
                    debug!("Python module event_handlers() not available: {}", e);
                    Vec::new()
                }
            }
        })
    }

    // ========================================================================
    // Hot Reload
    // ========================================================================

    fn supports_hot_reload(&self) -> bool {
        Python::attach(|py| {
            let obj = self.py_object.bind(py);
            obj.call_method0("supports_hot_reload")
                .is_ok_and(|result| result.extract::<bool>().unwrap_or(false))
        })
    }

    fn save_state(&self) -> Option<Box<[u8]>> {
        Python::attach(|py| {
            let obj = self.py_object.bind(py);
            match obj.call_method0("save_state") {
                Ok(result) => {
                    if result.is_none() {
                        None
                    } else {
                        match result.extract::<Vec<u8>>() {
                            Ok(bytes) => Some(bytes.into_boxed_slice()),
                            Err(e) => {
                                warn!("Python module save_state() returned non-bytes: {}", e);
                                None
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("Python module save_state() raised exception: {}", e);
                    None
                }
            }
        })
    }

    fn restore_state(&mut self, state: &[u8]) -> Result<(), ModuleError> {
        Python::attach(|py| {
            let obj = self.py_object.bind(py);
            let py_bytes = pyo3::types::PyBytes::new(py, state);

            match obj.call_method1("restore_state", (py_bytes,)) {
                Ok(_) => Ok(()),
                Err(e) => {
                    error!("Python module restore_state() raised exception: {e}");
                    Err(ModuleError::InitFailed(format!("restore_state failed: {e}")))
                }
            }
        })
    }
}

impl std::fmt::Debug for PythonModule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PythonModule")
            .field("id", &self.id())
            .field("source_path", &self.source_path)
            .finish_non_exhaustive()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_py_module_context_from_kernel() {
        let kernel_ctx = ModuleContext::default();
        let py_ctx = PyModuleContext::from_kernel(&kernel_ctx);

        assert!(py_ctx.data_dir.contains("reovim-test"));
        assert!(py_ctx.cache_dir.contains("reovim-test"));
        assert!(py_ctx.optional_deps().is_empty());
    }

    #[test]
    fn test_py_module_context_has_optional_dep() {
        let py_ctx = PyModuleContext {
            data_dir: "/tmp/data".to_string(),
            cache_dir: "/tmp/cache".to_string(),
            optional_deps: vec!["lsp".to_string(), "treesitter".to_string()],
        };

        assert!(py_ctx.has_optional_dep("lsp"));
        assert!(py_ctx.has_optional_dep("treesitter"));
        assert!(!py_ctx.has_optional_dep("unknown"));
    }

    #[test]
    fn test_py_module_context_repr() {
        let py_ctx = PyModuleContext {
            data_dir: "/data".to_string(),
            cache_dir: "/cache".to_string(),
            optional_deps: vec![],
        };

        let repr = py_ctx.__repr__();
        assert!(repr.contains("/data"));
        assert!(repr.contains("/cache"));
    }

    #[test]
    fn test_py_module_context_clone() {
        let py_ctx = PyModuleContext {
            data_dir: "/data".to_string(),
            cache_dir: "/cache".to_string(),
            optional_deps: vec!["dep1".to_string()],
        };
        #[allow(clippy::redundant_clone)]
        let cloned = py_ctx.clone();
        assert_eq!(cloned.data_dir, "/data");
        assert_eq!(cloned.cache_dir, "/cache");
        assert_eq!(cloned.optional_deps(), vec!["dep1".to_string()]);
    }

    #[test]
    fn test_py_module_context_optional_deps_empty() {
        let py_ctx = PyModuleContext {
            data_dir: String::new(),
            cache_dir: String::new(),
            optional_deps: vec![],
        };
        assert!(py_ctx.optional_deps().is_empty());
        assert!(!py_ctx.has_optional_dep("anything"));
    }

    #[test]
    fn test_py_module_context_multiple_optional_deps() {
        let py_ctx = PyModuleContext {
            data_dir: String::new(),
            cache_dir: String::new(),
            optional_deps: vec![
                "lsp".to_string(),
                "treesitter".to_string(),
                "git".to_string(),
            ],
        };
        assert_eq!(py_ctx.optional_deps().len(), 3);
        assert!(py_ctx.has_optional_dep("lsp"));
        assert!(py_ctx.has_optional_dep("treesitter"));
        assert!(py_ctx.has_optional_dep("git"));
        assert!(!py_ctx.has_optional_dep("debug"));
    }

    // Integration tests with actual Python require the GIL and are tested
    // via integration tests in server/lib/server/tests/
}
