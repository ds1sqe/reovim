#![allow(unsafe_code)] // FFI loading requires unsafe
//! Dynamic TUI extension loading from `.so` files (#624).
//!
//! This driver loads TUI extensions compiled as shared libraries.
//! Extensions use `declare_extension!` to generate FFI entry points.
//!
//! # Architecture
//!
//! This is a **driver** (mechanism layer). It provides:
//! - `.so` file discovery and loading via `libloading`
//! - FFI symbol resolution (`reovim_extension_kind`, `reovim_extension_entry`)
//! - Conversion from raw FFI pointer to `Box<dyn TuiExtension>`
//!
//! Policy (which extensions to load, search paths) is in the app layer.
//!
//! # FFI Protocol
//!
//! Each extension `.so` exports:
//! - `reovim_extension_kind() -> *const c_char` — extension kind identifier
//! - `reovim_extension_entry() -> *mut c_void` — creates a `Box<Box<dyn TuiExtension>>`
//! - `reovim_extension_destroy(*mut c_void)` — cleanup (not used when Box is taken)

use std::{ffi::CStr, path::Path};

use reovim_driver_display::render_backend::TuiExtension;

/// Error from extension loading.
#[derive(Debug)]
pub enum ExtensionLoadError {
    /// Failed to load the shared library.
    LibraryLoad(String),
    /// Required FFI symbol not found.
    SymbolNotFound(String),
    /// Extension kind query returned invalid data.
    InvalidKind(String),
    /// Entry point returned null.
    NullEntry,
}

impl std::fmt::Display for ExtensionLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LibraryLoad(msg) => write!(f, "extension library load error: {msg}"),
            Self::SymbolNotFound(sym) => write!(f, "extension symbol not found: {sym}"),
            Self::InvalidKind(msg) => write!(f, "extension kind error: {msg}"),
            Self::NullEntry => write!(f, "extension entry returned null"),
        }
    }
}

impl std::error::Error for ExtensionLoadError {}

/// A loaded dynamic TUI extension with its library handle.
///
/// The library handle is kept alive to prevent unloading the `.so`
/// while the extension is in use.
pub struct LoadedExtension {
    /// The extension instance.
    extension: Box<dyn TuiExtension>,
    /// Library handle — kept alive to prevent premature unloading.
    /// SAFETY: Must be dropped AFTER the extension.
    library: libloading::Library,
}

impl LoadedExtension {
    /// Take the extension, consuming the loaded handle.
    ///
    /// The library handle is leaked (not unloaded) since the extension's
    /// vtable still points into the library code. This is the same approach
    /// used by server module loading.
    #[must_use]
    pub fn into_extension(self) -> Box<dyn TuiExtension> {
        // Leak the library handle so code stays loaded
        std::mem::forget(self.library);
        self.extension
    }

    /// Get the extension kind.
    #[must_use]
    pub fn kind(&self) -> &str {
        self.extension.kind()
    }
}

/// Query the extension kind from a `.so` without creating an instance.
///
/// # Errors
///
/// Returns [`ExtensionLoadError`] if the library cannot be loaded or
/// the kind symbol is missing.
///
/// # Safety
///
/// Loading a `.so` file executes arbitrary code. Only load trusted libraries.
pub fn probe_extension_kind(path: &Path) -> Result<String, ExtensionLoadError> {
    // SAFETY: We're loading a trusted .so file and calling its exported function.
    let library = unsafe { libloading::Library::new(path) }
        .map_err(|e| ExtensionLoadError::LibraryLoad(e.to_string()))?;

    let kind_fn: libloading::Symbol<'_, unsafe extern "C" fn() -> *const std::ffi::c_char> =
        unsafe { library.get(b"reovim_extension_kind") }
            .map_err(|_| ExtensionLoadError::SymbolNotFound("reovim_extension_kind".to_string()))?;

    let kind_ptr = unsafe { kind_fn() };
    if kind_ptr.is_null() {
        return Err(ExtensionLoadError::InvalidKind("null pointer".to_string()));
    }

    let kind = unsafe { CStr::from_ptr(kind_ptr) }
        .to_str()
        .map_err(|e| ExtensionLoadError::InvalidKind(e.to_string()))?
        .to_string();

    Ok(kind)
}

/// Load a TUI extension from a `.so` file.
///
/// # Errors
///
/// Returns [`ExtensionLoadError`] if the library cannot be loaded,
/// required symbols are missing, or the entry point returns null.
///
/// # Safety
///
/// Loading a `.so` file executes arbitrary code. Only load trusted libraries.
pub fn load_extension(path: &Path) -> Result<LoadedExtension, ExtensionLoadError> {
    tracing::debug!("loading extension from {}", path.display());

    // SAFETY: We're loading a trusted .so file.
    let library = unsafe { libloading::Library::new(path) }
        .map_err(|e| ExtensionLoadError::LibraryLoad(e.to_string()))?;

    // Resolve the entry point
    let entry_fn: libloading::Symbol<'_, unsafe extern "C" fn() -> *mut std::ffi::c_void> =
        unsafe { library.get(b"reovim_extension_entry") }.map_err(|_| {
            ExtensionLoadError::SymbolNotFound("reovim_extension_entry".to_string())
        })?;

    let raw_ptr = unsafe { entry_fn() };
    if raw_ptr.is_null() {
        return Err(ExtensionLoadError::NullEntry);
    }

    // The entry point returns a Box<Box<dyn TuiExtension>> as *mut c_void.
    // We reconstruct the outer Box to get the inner Box<dyn TuiExtension>.
    // SAFETY: The pointer came from Box::into_raw(Box::new(ext)) in declare_extension!
    let extension: Box<dyn TuiExtension> =
        unsafe { *Box::from_raw(raw_ptr.cast::<Box<dyn TuiExtension>>()) };

    tracing::info!("loaded extension '{}' from {}", extension.kind(), path.display());

    Ok(LoadedExtension { extension, library })
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
