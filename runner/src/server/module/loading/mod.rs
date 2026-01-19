//! FFI loading subsystem for dynamic module loading.
//!
//! This module provides the low-level infrastructure for loading modules from
//! shared libraries. It handles:
//!
//! - Module discovery (searching paths for `.so`/`.dylib`/`.dll` files)
//! - FFI symbol resolution (`declare_module!` entry points)
//! - Module handle management (static and dynamic)
//!
//! # Architecture
//!
//! ```text
//! loading/
//! ├── discovery.rs   # Path search, file discovery
//! ├── handle.rs      # ModuleHandle, FFI trampolines
//! └── loader.rs      # ModuleLoader, static/dynamic loading
//! ```

#[allow(unsafe_code)]
mod discovery;
#[allow(unsafe_code)]
mod handle;
#[allow(unsafe_code)]
mod loader;
#[allow(unsafe_code)]
mod startup;

// Re-exports
pub use {
    discovery::{
        default_search_paths, discover_modules, find_module, library_extension, library_filename,
    },
    handle::{InitResult, ModuleHandle},
    loader::ModuleLoader,
    startup::{LoadResult, StartupLoadStats, StaticModuleFactory, load_from_config},
};

#[cfg(feature = "python")]
#[allow(unused_imports)] // Public API exports for Python module discovery
pub use discovery::{discover_python_modules, find_python_module};
