//! Lookup helpers for the split `reovim-*` bins (#769).
//!
//! The integration harness spawns the dedicated `reovim-server` bin
//! directly, and any client-side harness code spawns `reovim-tui`.
//!
//! Resolution order for each helper:
//! 1. `REOVIM_TEST_<KIND>_BINARY` env var — explicit override (useful
//!    for release-build smoke or out-of-tree builds).
//! 2. `CARGO_BIN_EXE_<bin>` env var — set by cargo for integration
//!    tests of the crate that owns the bin.
//! 3. Inferred from `std::env::current_exe()` — test binaries live in
//!    `target/<target-dir>/debug/deps/`, so `../../<bin>` resolves the
//!    sibling bin. Works under `cargo-llvm-cov`'s
//!    `target/llvm-cov-target/`.
//! 4. Compile-time fallback via `CARGO_MANIFEST_DIR`.

use std::path::{Path, PathBuf};

/// Platform-specific bin filename (adds `.exe` on Windows).
#[cfg_attr(coverage_nightly, coverage(off))]
fn bin_filename(stem: &str) -> String {
    #[cfg(windows)]
    {
        format!("{stem}.exe")
    }
    #[cfg(not(windows))]
    {
        stem.to_string()
    }
}

/// Shared resolution logic. `env_override` is the `REOVIM_TEST_*`
/// override variable; `cargo_bin_env` is the `CARGO_BIN_EXE_<bin>`
/// variable cargo populates during integration-test builds.
#[cfg_attr(coverage_nightly, coverage(off))]
fn resolve_bin(bin_stem: &str, env_override: &str, cargo_bin_env: &str) -> PathBuf {
    // 1. Explicit override.
    if let Ok(path) = std::env::var(env_override) {
        return PathBuf::from(path);
    }

    // 2. CARGO_BIN_EXE_<bin> — cargo sets this for the owning crate's
    //    integration tests.
    if let Ok(path) = std::env::var(cargo_bin_env) {
        return PathBuf::from(path);
    }

    let filename = bin_filename(bin_stem);

    // 3. Infer from running test binary's location.
    if let Ok(exe) = std::env::current_exe() {
        let debug_dir = exe.parent().and_then(Path::parent);
        if let Some(dir) = debug_dir {
            let candidate = dir.join(&filename);
            if candidate.exists() {
                return candidate;
            }
        }
    }

    // 4. Compile-time workspace root fallback.
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    PathBuf::from(manifest_dir)
        .parent()
        .expect("tools/testing should have parent")
        .parent()
        .expect("tools should have parent (workspace root)")
        .join("target/debug")
        .join(filename)
}

/// Absolute path to the `reovim-server` bin (`apps/server/`).
///
/// See module docs for the resolution order. The returned path is not
/// checked for existence beyond the `current_exe`-relative probe —
/// callers that spawn it will get an OS error if the path is wrong,
/// which is the desired debugging signal.
#[must_use]
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn which_reovim_server() -> PathBuf {
    resolve_bin("reovim-server", "REOVIM_TEST_SERVER_BINARY", "CARGO_BIN_EXE_reovim-server")
}

/// Absolute path to the `reovim-tui` bin (`apps/tui/`).
///
/// See module docs for the resolution order.
#[must_use]
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn which_reovim_tui() -> PathBuf {
    resolve_bin("reovim-tui", "REOVIM_TEST_TUI_BINARY", "CARGO_BIN_EXE_reovim-tui")
}

#[cfg(test)]
#[path = "bin_paths_tests.rs"]
mod bin_paths_tests;
