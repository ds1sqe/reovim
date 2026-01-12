//! Module system for dynamic module loading and management.
//!
//! This module implements the runner-side module loading system as part of
//! Phase 4 of the Linux-inspired architecture overhaul.
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
//! - [`HotReloadManager`]: File watching and hot reload (feature-gated)
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
//! registry.shutdown()?;
//! ```

// Module-level clippy allows for the module system
// - dead_code: module system not yet integrated with main.rs
// - clippy::missing_const_for_fn: many methods could be const but aren't stable yet
// - clippy::option_if_let_else: prefer explicit if-let for clarity
#[allow(
    dead_code,
    clippy::missing_const_for_fn,
    clippy::option_if_let_else,
    clippy::too_many_lines,
    clippy::similar_names
)]
mod adapter;
#[allow(
    dead_code,
    clippy::missing_const_for_fn,
    clippy::option_if_let_else,
    clippy::too_many_lines,
    clippy::similar_names
)]
mod dependency;
#[allow(
    dead_code,
    clippy::missing_const_for_fn,
    clippy::option_if_let_else,
    clippy::too_many_lines,
    clippy::similar_names
)]
mod discovery;
#[allow(
    dead_code,
    clippy::missing_const_for_fn,
    clippy::option_if_let_else,
    clippy::too_many_lines,
    clippy::similar_names,
    clippy::redundant_pub_crate,
    clippy::ptr_as_ptr,
    clippy::ref_as_ptr
)]
mod handle;
#[allow(
    dead_code,
    clippy::missing_const_for_fn,
    clippy::option_if_let_else,
    clippy::too_many_lines,
    clippy::similar_names
)]
mod loader;
#[allow(
    dead_code,
    clippy::missing_const_for_fn,
    clippy::option_if_let_else,
    clippy::too_many_lines,
    clippy::similar_names,
    clippy::significant_drop_tightening
)]
mod registry;

#[cfg(feature = "hot-reload")]
#[allow(
    dead_code,
    clippy::missing_const_for_fn,
    clippy::option_if_let_else,
    clippy::too_many_lines,
    clippy::similar_names
)]
mod hot_reload;

// Re-exports - these form the public API of the module system
// They are not yet consumed from main.rs, hence the allow(unused)
#[allow(unused_imports)]
pub use adapter::PluginFromModule;
#[allow(unused_imports)]
pub use dependency::{DependencyOrder, resolve_dependencies};
#[allow(unused_imports)]
pub use discovery::{default_search_paths, discover_modules, library_extension, library_filename};
#[allow(unused_imports)]
pub use handle::{InitResult, ModuleHandle};
#[allow(unused_imports)]
pub use loader::ModuleLoader;
#[allow(unused_imports)]
pub use registry::ModuleRegistry;

#[cfg(feature = "hot-reload")]
#[allow(unused_imports)]
pub use hot_reload::HotReloadManager;
