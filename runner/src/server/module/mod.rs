//! Module system for dynamic module loading and management.
//!
//! This module implements the runner-side module loading system as part of
//! Phase 6.1.2 of the Linux-inspired architecture overhaul.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                     runner/src/server/module/                   │
//! │  ┌──────────────────────────────────────────────────────────┐   │
//! │  │ loading/       │ lifecycle/       │ wiring/              │   │
//! │  │  ModuleLoader  │  ModuleManager   │  wire_keybindings    │   │
//! │  │  ModuleHandle  │  DependencyOrder │  WiringError         │   │
//! │  │  discovery     │  resolve_deps    │  WiringStats         │   │
//! │  └──────────────────────────────────────────────────────────┘   │
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
//! # Sub-modules
//!
//! - **loading**: FFI loading subsystem (discovery, handle, loader)
//! - **lifecycle**: State management subsystem (dependency resolution, manager)
//! - **wiring**: Registry integration subsystem (keybinding wiring)
//!
//! # Example
//!
//! ```ignore
//! use runner::module::{ModuleLoader, ModuleManager};
//!
//! // Create manager with default loader
//! let manager = ModuleManager::new();
//!
//! // Register a static module
//! manager.register(MyModule)?;
//!
//! // Initialize all modules in dependency order
//! manager.init_all(&ctx)?;
//!
//! // Shutdown in reverse order
//! manager.shutdown();
//! ```

// Configuration stays at top level (used across sub-modules)
mod config;
mod defaults;

// Sub-modules organized by responsibility
mod lifecycle;
mod loading;
mod wiring;

// Re-exports - public API of the module system
pub use {
    // Configuration
    config::{ConfigError, ModuleConfig},
    // Defaults
    defaults::DEFAULT_MODULES,
    // Lifecycle management
    lifecycle::{DependencyOrder, ModuleManager, resolve_dependencies},
    // Loading subsystem
    loading::{
        InitResult, ModuleHandle, ModuleLoader, default_search_paths, discover_modules,
        find_module, library_extension, library_filename,
    },
    // Wiring subsystem
    wiring::{
        CommandWiringError, CommandWiringResult, CommandWiringStats, WiringError, WiringResult,
        WiringStats, wire_module_commands, wire_module_keybindings,
    },
};

// Backwards compatibility alias: ModuleRegistry -> ModuleManager
// This allows existing code to use the old name during migration
#[doc(hidden)]
#[deprecated(since = "0.9.0", note = "Use ModuleManager instead")]
pub type ModuleRegistry = ModuleManager;
