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
/// Searches in order:
/// 1. XDG data dir (`~/.local/share/reovim/modules/`) - installed modules with FFI symbols
/// 2. `target/release/` - release build with FFI symbols
/// 3. `target/debug/` - debug build (fallback, may not have FFI symbols)
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
    let lib_name = demo_module_filename();

    // First, check XDG data dir for installed modules (these have FFI symbols)
    // XDG_DATA_HOME defaults to ~/.local/share
    let xdg_data_home = std::env::var("XDG_DATA_HOME").map_or_else(
        |_| {
            std::env::var("HOME")
                .map_or_else(|_| PathBuf::new(), |h| PathBuf::from(h).join(".local/share"))
        },
        PathBuf::from,
    );
    let xdg_path = xdg_data_home.join("reovim/modules").join(lib_name);

    if xdg_path.exists() {
        return xdg_path;
    }

    // Second, check release build (built with --features dynamic)
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let release_path = PathBuf::from(manifest_dir)
        .parent()
        .expect("runner should have parent directory")
        .join("target/release")
        .join(lib_name);

    if release_path.exists() {
        return release_path;
    }

    // Fallback to debug build (may not have FFI symbols without --features dynamic)
    PathBuf::from(manifest_dir)
        .parent()
        .expect("runner should have parent directory")
        .join("target/debug")
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
