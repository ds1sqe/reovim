//! Module system for dynamic module loading and management.
//!
//! This module implements the runner-side module loading system as part of
//! Phase 6.1.2 of the Linux-inspired architecture overhaul.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                     runner/src/module/                          │
//! │  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────┐   │
//! │  │ ModuleLoader │  │ModuleRegistry│  │ HotReloadManager     │   │
//! │  │  (libloading)│  │(dep resolve) │  │ (notify, feat-gated) │   │
//! │  └──────┬───────┘  └──────┬───────┘  └──────────┬───────────┘   │
//! │         │                 │                     │               │
//! │         └─────────────────┴─────────────────────┘               │
//! │                            │ Policy                             │
//! └────────────────────────────┼────────────────────────────────────┘
//!                              │
//!             ═════════════════╪═════════════════ Module Interface
//!                              │
//! ┌────────────────────────────────────────────────────────────────┐
//! │              lib/kernel/src/api/module.rs                       │
//! │  Module trait │ ModuleId │ ModuleState │ ModuleProbe            │
//! │  ProbeResult │ ModuleError │ Registration types                 │
//! │                           Mechanism                             │
//! └────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Components
//!
//! - [`ModuleHandle`]: Wrapper for loaded modules (static or dynamic)
//! - [`ModuleLoader`]: Discovers and loads modules from disk
//! - [`ModuleRegistry`]: Manages module lifecycle with dependency resolution
//!
//! # Example
//!
//! ```ignore
//! use runner::module::{ModuleLoader, ModuleRegistry};
//!
//! // Create registry with default loader
//! let registry = ModuleRegistry::new();
//!
//! // Register a static module
//! registry.register(MyModule)?;
//!
//! // Initialize all modules in dependency order
//! registry.init_all(&ctx)?;
//!
//! // Shutdown in reverse order
//! registry.shutdown();
//! ```

// Module-level lint allows:
// - unsafe_code: FFI operations for dynamic loading require unsafe
// - clippy lints: complexity from FFI handling and dependency resolution
mod config;
#[allow(unsafe_code)]
mod dependency;
#[allow(unsafe_code)]
mod discovery;
#[allow(unsafe_code)]
mod handle;
#[allow(unsafe_code)]
mod loader;
#[allow(unsafe_code)]
mod registry;
mod wiring;

// Re-exports - public API of the module system
pub use {
    config::{ConfigError, ModuleConfig},
    dependency::{DependencyOrder, resolve_dependencies},
    discovery::{
        default_search_paths, discover_modules, find_module, library_extension, library_filename,
    },
    handle::{InitResult, ModuleHandle},
    loader::ModuleLoader,
    registry::ModuleRegistry,
    wiring::{WiringError, WiringResult, WiringStats, wire_module_keybindings},
};
