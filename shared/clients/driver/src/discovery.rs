//! Client module discovery — search paths and `.so` file conventions.
//!
//! Follows the server-side discovery pattern adapted for client modules.
//! Uses `REOVIM_CLIENT_MODULE_PATH` env var and XDG-compliant default paths.

use std::path::{Path, PathBuf};

/// Environment variable for additional client module search paths.
pub const CLIENT_MODULE_PATH_ENV: &str = "REOVIM_CLIENT_MODULE_PATH";

/// File prefix for client module shared libraries.
const CLIENT_MODULE_PREFIX: &str = "libreovim_client_module_";

/// Default search paths for client module `.so` files.
#[must_use]
// Uses env vars + filesystem paths — not unit testable.
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn default_client_search_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // $REOVIM_CLIENT_MODULE_PATH entries (colon-separated)
    if let Ok(env_path) = std::env::var(CLIENT_MODULE_PATH_ENV) {
        for entry in env_path.split(':') {
            if !entry.is_empty() {
                paths.push(PathBuf::from(entry));
            }
        }
    }

    // $XDG_DATA_HOME/reovim/client-modules/ (or ~/.local/share/reovim/client-modules/)
    if let Ok(xdg_data) = std::env::var("XDG_DATA_HOME") {
        paths.push(PathBuf::from(xdg_data).join("reovim/client-modules"));
    } else if let Ok(home) = std::env::var("HOME") {
        paths.push(PathBuf::from(home).join(".local/share/reovim/client-modules"));
    }

    // System paths
    paths.push(PathBuf::from("/usr/local/lib/reovim/client-modules"));
    paths.push(PathBuf::from("/usr/lib/reovim/client-modules"));

    paths
}

/// Platform-correct filename for a client module shared library.
///
/// - Linux: `libreovim_client_module_{name}.so`
/// - macOS: `libreovim_client_module_{name}.dylib`
#[must_use]
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn client_library_filename(name: &str) -> String {
    let ext = if cfg!(target_os = "macos") {
        "dylib"
    } else {
        "so"
    };
    format!("{CLIENT_MODULE_PREFIX}{name}.{ext}")
}

/// Discover all client module `.so` files in the given search paths.
#[must_use]
// Filesystem directory traversal — not unit testable.
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn discover_client_modules(paths: &[PathBuf]) -> Vec<PathBuf> {
    let ext = if cfg!(target_os = "macos") {
        "dylib"
    } else {
        "so"
    };

    let mut found = Vec::new();
    for dir in paths {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(name) = path.file_name().and_then(|n| n.to_str())
                    && name.starts_with(CLIENT_MODULE_PREFIX)
                    && name.ends_with(ext)
                {
                    found.push(path);
                }
            }
        }
    }
    found
}

/// Find a specific client module by name in the search paths.
#[must_use]
pub fn find_client_module(paths: &[PathBuf], name: &str) -> Option<PathBuf> {
    let filename = client_library_filename(name);
    for dir in paths {
        let candidate = dir.join(&filename);
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

/// Extract the module name from a client module `.so` path.
///
/// `libreovim_client_module_foo.so` -> `Some("foo")`
#[must_use]
pub fn client_module_name_from_path(path: &Path) -> Option<String> {
    let stem = path.file_stem()?.to_str()?;
    stem.strip_prefix(CLIENT_MODULE_PREFIX)
        .map(ToString::to_string)
}

#[cfg(test)]
#[path = "discovery_tests.rs"]
mod tests;
