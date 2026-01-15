//! Common test utilities for integration tests.
//!
//! This module provides:
//! - `IntegrationTest`: Fluent builder for single-client tests
//! - `MultiClientTest`: Builder for concurrent multi-client tests
//! - Assertion macros for buffer, cursor, register, and mode verification
//! - Helper functions for locating demo modules

// Allow unused code in test utilities - each test file uses different subsets
#![allow(dead_code)]
#![allow(unused_imports)]

mod assertions;
mod harness;
mod integration;
mod multi_client;

use std::path::PathBuf;

pub use {
    harness::TestServerHarness,
    integration::{IntegrationTest, RegisterInfo, TestResult},
    multi_client::{MultiClientTest, TestClient},
};

// ============================================================================
// Demo Module Helpers (for module_loading.rs tests)
// ============================================================================

/// Get the path to the demo module shared library.
///
/// The demo module is built as a cdylib when running tests because it's
/// listed as a dev-dependency in runner/Cargo.toml.
///
/// # Returns
///
/// Path to `libreovim_module_hot_reload_demo.so` (or `.dylib` on macOS,
/// `.dll` on Windows).
///
/// # Panics
///
/// Panics if the parent directory of `CARGO_MANIFEST_DIR` doesn't exist.
#[must_use]
pub fn demo_module_path() -> PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };

    let lib_name = demo_module_filename();

    PathBuf::from(manifest_dir)
        .parent()
        .expect("runner should have parent directory")
        .join("target")
        .join(profile)
        .join(lib_name)
}

/// Get the filename for the demo module library.
///
/// Platform-specific:
/// - Linux: `libreovim_module_hot_reload_demo.so`
/// - macOS: `libreovim_module_hot_reload_demo.dylib`
/// - Windows: `reovim_module_hot_reload_demo.dll`
#[must_use]
pub const fn demo_module_filename() -> &'static str {
    #[cfg(target_os = "linux")]
    {
        "libreovim_module_hot_reload_demo.so"
    }
    #[cfg(target_os = "macos")]
    {
        "libreovim_module_hot_reload_demo.dylib"
    }
    #[cfg(target_os = "windows")]
    {
        "reovim_module_hot_reload_demo.dll"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_demo_module_path_exists() {
        let path = demo_module_path();
        assert!(
            path.exists(),
            "Demo module should exist at {path:?}. Make sure to run `cargo build -p reovim-module-hot-reload-demo` first.",
        );
    }

    #[test]
    fn test_demo_module_filename() {
        let filename = demo_module_filename();

        #[cfg(target_os = "linux")]
        assert!(
            std::path::Path::new(filename)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("so"))
        );

        #[cfg(target_os = "macos")]
        assert!(
            std::path::Path::new(filename)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("dylib"))
        );

        #[cfg(target_os = "windows")]
        assert!(
            std::path::Path::new(filename)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("dll"))
        );
    }
}
