//! Lifecycle management subsystem for modules.
//!
//! This module provides state management and dependency resolution for
//! loaded modules. It handles:
//!
//! - Dependency resolution (topological sort)
//! - Linux-style deferred probing
//! - State tracking (Loaded → Initializing → Running → Failed)
//! - Safe unload with dependency checks
//!
//! # Architecture
//!
//! ```text
//! lifecycle/
//! ├── dependency.rs   # Kahn's algorithm, DependencyOrder
//! └── manager.rs      # ModuleManager (registry + lifecycle)
//! ```

#[allow(unsafe_code)]
mod dependency;
#[allow(unsafe_code)]
mod manager;

// Re-exports
pub use {
    dependency::{DependencyOrder, resolve_dependencies},
    manager::ModuleManager,
};
