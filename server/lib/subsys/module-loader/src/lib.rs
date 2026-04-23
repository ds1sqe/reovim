#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Dynamic module loader driver.
//!
//! Provides runtime discovery and loading of external `.so` modules alongside
//! the existing static (builtin) modules. This is a **driver** (mechanism layer)
//! that handles:
//!
//! - Search path resolution and module file discovery (delegated to
//!   `reovim-dylib-loader`'s `PathResolver` + `scan_paths` mechanism)
//! - FFI symbol resolution (delegated to `reovim-dylib-loader`'s `Library` wrapper)
//! - Module handle management (static + dynamic)
//! - Module registry with state FSM and lifecycle
//! - Lock file generation with SHA-256 checksums
//!
//! Policy decisions (which modules to load, what to trust) live in the
//! bootstrap (app layer). This driver only provides mechanism.

pub mod discovery;
pub mod handle;
pub mod loader;
pub mod lockfile;
pub mod registry;
pub mod report;

#[cfg(feature = "hot-reload")]
pub mod hot_reload;

#[cfg(test)]
#[path = "discovery_tests.rs"]
mod discovery_tests;

#[cfg(test)]
#[path = "handle_tests.rs"]
mod handle_tests;

#[cfg(test)]
#[path = "loader_tests.rs"]
mod loader_tests;

#[cfg(test)]
#[path = "lockfile_tests.rs"]
mod lockfile_tests;

#[cfg(test)]
#[path = "registry_tests.rs"]
mod registry_tests;

#[cfg(all(test, feature = "hot-reload"))]
#[path = "hot_reload_tests.rs"]
mod hot_reload_tests;
