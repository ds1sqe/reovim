//! Python FFI bindings for reovim modules via `PyO3`.
//!
//! This crate provides Python bindings that allow writing reovim modules in Python.
//! It wraps kernel types (`ModuleId`, `Version`, `ProbeResult`) and provides a
//! `PythonModule` wrapper that implements the `Module` trait.
//!
//! # Architecture
//!
//! ```text
//! Python Module (my_module.py)
//!         │
//!         ▼
//! ┌─────────────────────────┐
//! │  ffi-python crate       │
//! │  PythonModule wrapper   │
//! │  impl Module trait      │
//! └─────────────────────────┘
//!         │
//!         ▼
//! ┌─────────────────────────┐
//! │  ModuleLoader           │
//! │  .py detection          │
//! └─────────────────────────┘
//! ```
//!
//! # Python Usage
//!
//! ```python
//! from reovim import Module, ModuleId, Version, ProbeResult
//!
//! class MyModule(Module):
//!     def id(self) -> ModuleId:
//!         return ModuleId("my-python-module")
//!
//!     def name(self) -> str:
//!         return "My Python Module"
//!
//!     def version(self) -> tuple[int, int, int]:
//!         return (1, 0, 0)
//!
//!     def init(self, ctx) -> ProbeResult:
//!         return ProbeResult.Success
//!
//!     def exit(self):
//!         pass
//! ```

use pyo3::prelude::*;

pub mod module;
pub mod runtime_api;
pub mod types;

/// Initialize Python with the `reovim` module registered.
///
/// This must be called BEFORE any other Python operations to make
/// `import reovim` work. Uses `append_to_inittab!` to register the
/// module as a built-in.
///
/// # Panics
///
/// Panics if called after Python has already been initialized.
///
/// # Example
///
/// ```no_run
/// use reovim_driver_ffi_python::init_python;
///
/// // Initialize Python with reovim module
/// init_python();
///
/// // Now Python code can: from reovim import Module
/// ```
pub fn init_python() {
    pyo3::append_to_inittab!(reovim);
}

pub use {
    module::{PyModuleContext, PythonModule},
    runtime_api::PyRuntimeApi,
    types::{
        PyCommandRegistration, PyEventHandlerRegistration, PyKeybindingRegistration, PyModuleId,
        PyProbeResult, PyVersion,
    },
};

/// Base class for Python modules.
///
/// Python modules must inherit from this class and implement the required methods.
/// This is a marker class - the actual implementation is in Python.
#[pyclass(name = "Module", module = "reovim", subclass)]
pub struct PyModuleBase;

// Allow clippy lints that conflict with PyO3 requirements.
// PyO3 methods must have `&self` or `&mut self` even when not used,
// and cannot be made `const` since they're called from Python.
#[allow(
    clippy::unused_self,
    clippy::missing_const_for_fn,
    clippy::new_without_default
)]
#[pymethods]
impl PyModuleBase {
    #[new]
    fn new() -> Self {
        Self
    }

    /// Unique module identifier.
    ///
    /// Override this method to return your module's ID.
    /// Convention: kebab-case, e.g., "my-module"
    fn id(&self) -> PyModuleId {
        PyModuleId::new("unnamed-module")
    }

    /// Human-readable module name.
    ///
    /// Override this method to return your module's display name.
    fn name(&self) -> &'static str {
        "Unnamed Module"
    }

    /// Module version as (major, minor, patch) tuple.
    ///
    /// Override this method to return your module's version.
    fn version(&self) -> (u32, u32, u32) {
        (0, 0, 0)
    }

    /// Required API version.
    ///
    /// Override to require a specific kernel API version.
    /// Defaults to the current API version.
    fn api_version(&self) -> PyVersion {
        PyVersion::new(
            reovim_kernel::api::v1::API_VERSION.major,
            reovim_kernel::api::v1::API_VERSION.minor,
            reovim_kernel::api::v1::API_VERSION.patch,
        )
    }

    /// Required dependencies.
    ///
    /// Override to specify modules that must be loaded before this one.
    fn dependencies(&self) -> Vec<PyModuleId> {
        Vec::new()
    }

    /// Optional dependencies.
    ///
    /// Override to specify modules that should be loaded before this one if available.
    fn optional_dependencies(&self) -> Vec<PyModuleId> {
        Vec::new()
    }

    /// Initialize the module.
    ///
    /// Override this method to perform module initialization.
    /// Return `ProbeResult.Success` on success, `ProbeResult.Defer(reason)` to retry later,
    /// or `ProbeResult.Failed(reason)` for permanent failure.
    #[pyo3(signature = (ctx))]
    #[allow(unused_variables)]
    fn init(&mut self, ctx: &Bound<'_, PyAny>) -> PyProbeResult {
        PyProbeResult::success()
    }

    /// Clean up the module.
    ///
    /// Override this method to perform cleanup before the module is unloaded.
    fn exit(&mut self) {}

    /// Whether this module supports hot reload.
    ///
    /// Override to return `True` if your module can preserve state across reloads.
    fn supports_hot_reload(&self) -> bool {
        false
    }

    /// Save module state for hot reload.
    ///
    /// Override to serialize your module's state. Return `None` if no state to save.
    /// Use `pickle.dumps()` for simple serialization.
    fn save_state(&self) -> Option<Vec<u8>> {
        None
    }

    /// Restore module state after hot reload.
    ///
    /// Override to deserialize state saved by `save_state()`.
    #[pyo3(signature = (state))]
    #[allow(unused_variables)]
    fn restore_state(&mut self, state: &[u8]) {}

    // ========================================================================
    // Registrations
    // ========================================================================

    /// Return command registrations.
    ///
    /// Override to register commands that can be invoked by keybindings.
    ///
    /// # Example
    ///
    /// ```python
    /// def commands(self):
    ///     return [
    ///         CommandRegistration("my-command")
    ///             .with_name("My Command")
    ///             .with_description("Does something useful")
    ///     ]
    /// ```
    fn commands(&self) -> Vec<PyCommandRegistration> {
        Vec::new()
    }

    /// Return keybinding registrations.
    ///
    /// Override to register keybindings that invoke commands.
    ///
    /// # Example
    ///
    /// ```python
    /// def keybindings(self):
    ///     return [
    ///         KeybindingRegistration("<C-m>", "my-command")
    ///             .with_modes(["normal"])
    ///             .with_description("Run my command")
    ///     ]
    /// ```
    fn keybindings(&self) -> Vec<PyKeybindingRegistration> {
        Vec::new()
    }

    /// Return event handler registrations.
    ///
    /// Override to register handlers for editor events.
    ///
    /// # Example
    ///
    /// ```python
    /// def event_handlers(self):
    ///     return [
    ///         EventHandlerRegistration("BufferChanged")
    ///             .with_description("Handle buffer changes")
    ///     ]
    /// ```
    fn event_handlers(&self) -> Vec<PyEventHandlerRegistration> {
        Vec::new()
    }
}

/// The reovim Python module.
///
/// Provides types for writing reovim modules in Python:
/// - `Module` - Base class for Python modules
/// - `ModuleId` - Unique module identifier
/// - `Version` - Semantic version
/// - `ProbeResult` - Initialization result
/// - `CommandRegistration` - Command registration builder
/// - `KeybindingRegistration` - Keybinding registration builder
/// - `EventHandlerRegistration` - Event handler registration builder
#[pymodule]
fn reovim(m: &Bound<'_, PyModule>) -> PyResult<()> {
    // Base class
    m.add_class::<PyModuleBase>()?;

    // Core types
    m.add_class::<PyModuleId>()?;
    m.add_class::<PyVersion>()?;
    m.add_class::<PyProbeResult>()?;
    m.add_class::<PyModuleContext>()?;

    // Runtime API
    m.add_class::<PyRuntimeApi>()?;

    // Registration types
    m.add_class::<PyCommandRegistration>()?;
    m.add_class::<PyKeybindingRegistration>()?;
    m.add_class::<PyEventHandlerRegistration>()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_base_defaults() {
        let module = PyModuleBase::new();
        assert_eq!(module.id().as_str(), "unnamed-module");
        assert_eq!(module.name(), "Unnamed Module");
        assert_eq!(module.version(), (0, 0, 0));
        assert!(!module.supports_hot_reload());
        assert!(module.save_state().is_none());
        assert!(module.dependencies().is_empty());
        assert!(module.optional_dependencies().is_empty());
        // Registration methods default to empty
        assert!(module.commands().is_empty());
        assert!(module.keybindings().is_empty());
        assert!(module.event_handlers().is_empty());
    }

    #[test]
    fn test_module_base_api_version() {
        let module = PyModuleBase::new();
        let api_ver = module.api_version();
        assert_eq!(api_ver.major(), reovim_kernel::api::v1::API_VERSION.major);
        assert_eq!(api_ver.minor(), reovim_kernel::api::v1::API_VERSION.minor);
        assert_eq!(api_ver.patch(), reovim_kernel::api::v1::API_VERSION.patch);
    }

    #[test]
    fn test_module_base_restore_state_is_noop() {
        let mut module = PyModuleBase::new();
        // Should not panic
        module.restore_state(&[1, 2, 3]);
    }

    #[test]
    fn test_module_base_exit_is_noop() {
        let mut module = PyModuleBase::new();
        // Should not panic
        module.exit();
    }
}
