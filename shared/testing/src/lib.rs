#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Integration test harness for reovim.
//!
//! This crate provides the **mechanism** layer for integration testing.
//! Modules use this harness to define their test **policy** (what to test).
//!
//! # Architecture (Mechanism vs Policy)
//!
//! ```text
//! shared/testing/            ← MECHANISM (this crate)
//! ├── harness.rs             - Server process lifecycle
//! ├── integration.rs         - Single-client test builder
//! ├── multi_client.rs        - Multi-client test builder
//! ├── step_test.rs           - Per-key state tracking
//! └── assertions.rs          - Assertion macros
//!
//! server/modules/vim/tests/  ← POLICY (module-specific tests)
//! ├── operators.rs        - What operators to test
//! ├── registers.rs        - What register behaviors to verify
//! └── cursor_movement.rs  - What cursor behaviors to verify
//! ```
//!
//! # Protocol
//!
//! This crate uses **gRPC v2** for communicating with test servers.
//! The legacy JSON-RPC v1 protocol is no longer supported.
//!
//! # Example Usage (in module tests)
//!
//! ```ignore
//! use reovim_testing::{IntegrationTest, TestResult};
//!
//! #[tokio::test]
//! async fn test_dd_deletes_line() {
//!     let result = IntegrationTest::new()
//!         .await
//!         .with_buffer("hello\nworld")
//!         .send_keys("dd")
//!         .run()
//!         .await;
//!     result.assert_buffer_eq("world");
//! }
//! ```

// Allow unused code - each consumer uses different subsets
#![allow(dead_code)]

mod assertions;
pub mod frame;
mod harness;
mod integration;
mod multi_client;
mod presence;
mod step_test;

pub use {
    harness::TestServerHarness,
    integration::{IntegrationTest, RegisterInfo, TestResult},
    multi_client::{MultiClientTest, TestClient},
    presence::{MultiClientPresenceTest, PresenceTestClient},
    step_test::{StateSnapshot, StepTest, StepTrace},
};

// Macros are automatically exported at crate level via #[macro_export]

// ============================================================================
// Demo Module Helpers (for module_loading.rs tests)
// ============================================================================

use std::path::PathBuf;

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
#[cfg_attr(coverage_nightly, coverage(off))]
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
        .expect("lib/testing should have parent directory")
        .parent()
        .expect("lib should have parent directory")
        .join("target/release")
        .join(lib_name);

    if release_path.exists() {
        return release_path;
    }

    // Fallback to debug build (may not have FFI symbols without --features dynamic)
    PathBuf::from(manifest_dir)
        .parent()
        .expect("lib/testing should have parent directory")
        .parent()
        .expect("lib should have parent directory")
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
#[cfg_attr(coverage_nightly, coverage(off))]
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
