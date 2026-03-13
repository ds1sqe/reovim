//! Module discovery and search path utilities.
//!
//! Provides functions for finding module shared libraries on disk.
//! Follows XDG Base Directory specification for user-local paths and
//! supports custom search paths via `$REOVIM_MODULE_PATH`.

use std::path::PathBuf;

use reovim_kernel::api::v1::ConfigPaths;

/// Environment variable for custom module search paths (colon-separated on
/// Unix, semicolon-separated on Windows).
pub const MODULE_PATH_ENV: &str = "REOVIM_MODULE_PATH";

/// Default search paths for module discovery.
///
/// Returns paths in priority order (first match wins):
/// 1. `$REOVIM_MODULE_PATH` entries (if set)
/// 2. `$XDG_DATA_HOME/reovim/modules/` (user-installed)
/// 3. `/usr/local/lib/reovim/modules/` (locally-compiled)
/// 4. `/usr/lib/reovim/modules/` (system packages)
#[must_use]
pub fn default_search_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // Custom paths from environment variable (highest priority)
    if let Some(env_paths) = env_search_paths() {
        paths.extend(env_paths);
    }

    // User path (XDG_DATA_HOME/reovim/modules)
    if let Ok(data_dir) = ConfigPaths::data_dir() {
        paths.push(data_dir.join("modules"));
    }

    // System paths (Linux/macOS)
    #[cfg(unix)]
    {
        paths.push(PathBuf::from("/usr/local/lib/reovim/modules"));
        paths.push(PathBuf::from("/usr/lib/reovim/modules"));
    }

    paths
}

/// Parse custom search paths from the `REOVIM_MODULE_PATH` environment variable.
///
/// Returns `None` if the variable is not set.
/// Paths are split by `:` on Unix and `;` on Windows.
#[must_use]
fn env_search_paths() -> Option<Vec<PathBuf>> {
    std::env::var_os(MODULE_PATH_ENV).map(|val| {
        #[cfg(unix)]
        let sep = b':';
        #[cfg(windows)]
        let sep = b';';
        let _ = sep; // suppress unused warning on non-target platforms

        std::env::split_paths(&val).collect()
    })
}

/// Get shared library extension for the current platform.
#[must_use]
pub const fn library_extension() -> &'static str {
    #[cfg(target_os = "linux")]
    {
        "so"
    }
    #[cfg(target_os = "macos")]
    {
        "dylib"
    }
    #[cfg(target_os = "windows")]
    {
        "dll"
    }
}

/// Get library filename for a module name.
///
/// Follows platform conventions for shared library naming:
/// - Linux: `libreovim_module_{name}.so`
/// - macOS: `libreovim_module_{name}.dylib`
/// - Windows: `reovim_module_{name}.dll`
#[must_use]
pub fn library_filename(name: &str) -> String {
    let normalized = name.replace('-', "_");
    #[cfg(target_os = "windows")]
    {
        format!("reovim_module_{normalized}.{}", library_extension())
    }
    #[cfg(not(target_os = "windows"))]
    {
        format!("libreovim_module_{normalized}.{}", library_extension())
    }
}

/// Discover module files in search paths.
///
/// Scans all search paths for files matching the `libreovim_module_*.{ext}`
/// naming convention. Non-existent directories are silently skipped.
///
/// Returns paths to discovered library files (does not load them).
#[must_use]
pub fn discover_modules(search_paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let ext = library_extension();

    for path in search_paths {
        if !path.exists() {
            continue;
        }

        let Ok(entries) = std::fs::read_dir(path) else {
            tracing::debug!(?path, "Cannot read module directory");
            continue;
        };

        for entry in entries.flatten() {
            let file_path = entry.path();
            let matches_ext = file_path.extension().is_some_and(|e| e == ext);
            let matches_prefix = file_path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| {
                    n.starts_with("libreovim_module_") || n.starts_with("reovim_module_")
                });
            if matches_ext && matches_prefix {
                found.push(file_path);
            }
        }
    }

    found
}

/// Find a module by name in search paths.
///
/// Searches for a library file matching the given module name.
/// Returns the first match found (search paths are in priority order).
#[must_use]
pub fn find_module(search_paths: &[PathBuf], name: &str) -> Option<PathBuf> {
    let filename = library_filename(name);

    for path in search_paths {
        let full_path = path.join(&filename);
        if full_path.exists() {
            return Some(full_path);
        }
    }

    None
}

/// Extract module name from a library filename.
///
/// Reverses [`library_filename()`]: strips prefix and extension.
/// Returns `None` if the filename doesn't match the expected pattern.
#[must_use]
pub fn module_name_from_path(path: &std::path::Path) -> Option<String> {
    let stem = path.file_stem()?.to_str()?;
    let name = stem
        .strip_prefix("libreovim_module_")
        .or_else(|| stem.strip_prefix("reovim_module_"))?;
    Some(name.replace('_', "-"))
}
