//! Module discovery and search path utilities.
//!
//! Provides functions for finding module shared libraries on disk.

use std::path::PathBuf;

/// Default search paths for module discovery.
///
/// Follows XDG Base Directory specification using `crate::dirs` module
/// to maintain consistency with existing runner infrastructure.
#[must_use]
pub fn default_search_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // System paths (Linux/macOS)
    #[cfg(unix)]
    {
        paths.push(PathBuf::from("/usr/lib/reovim/modules"));
        paths.push(PathBuf::from("/usr/local/lib/reovim/modules"));
    }

    // User path - reuse existing dirs module (XDG_DATA_HOME/reovim/modules)
    paths.push(crate::dirs::data_dir().join("modules"));

    paths
}

/// Get shared library extension for current platform.
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
/// # Example
///
/// ```ignore
/// assert_eq!(library_filename("treesitter"), "libtreesitter.so");  // Linux
/// assert_eq!(library_filename("treesitter"), "libtreesitter.dylib");  // macOS
/// assert_eq!(library_filename("treesitter"), "treesitter.dll");  // Windows
/// ```
#[must_use]
pub fn library_filename(name: &str) -> String {
    #[cfg(target_os = "windows")]
    {
        format!("{name}.{}", library_extension())
    }
    #[cfg(not(target_os = "windows"))]
    {
        format!("lib{name}.{}", library_extension())
    }
}

/// Discover module files in search paths.
///
/// Scans all search paths for files with the appropriate library extension
/// (`.so` on Linux, `.dylib` on macOS, `.dll` on Windows).
#[must_use]
pub fn discover_modules(search_paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let ext = library_extension();

    for path in search_paths {
        if !path.exists() {
            continue;
        }

        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                let file_path = entry.path();
                if file_path.extension().is_some_and(|e| e == ext) {
                    found.push(file_path);
                }
            }
        }
    }

    found
}

/// Find a module by name in search paths.
///
/// Searches for a library file matching the given module name.
/// Returns the first match found.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_library_extension() {
        let ext = library_extension();
        #[cfg(target_os = "linux")]
        assert_eq!(ext, "so");
        #[cfg(target_os = "macos")]
        assert_eq!(ext, "dylib");
        #[cfg(target_os = "windows")]
        assert_eq!(ext, "dll");
    }

    #[test]
    fn test_library_filename() {
        let name = library_filename("treesitter");
        #[cfg(target_os = "linux")]
        assert_eq!(name, "libtreesitter.so");
        #[cfg(target_os = "macos")]
        assert_eq!(name, "libtreesitter.dylib");
        #[cfg(target_os = "windows")]
        assert_eq!(name, "treesitter.dll");
    }

    #[test]
    fn test_default_search_paths_not_empty() {
        let paths = default_search_paths();
        assert!(!paths.is_empty());
    }

    #[test]
    fn test_default_search_paths_contains_user_dir() {
        let paths = default_search_paths();
        let user_modules = crate::dirs::data_dir().join("modules");
        assert!(paths.contains(&user_modules));
    }

    #[test]
    fn test_discover_empty_paths() {
        let found = discover_modules(&[]);
        assert!(found.is_empty());
    }

    #[test]
    fn test_discover_nonexistent_path() {
        let found = discover_modules(&[PathBuf::from("/nonexistent/path")]);
        assert!(found.is_empty());
    }

    #[test]
    fn test_find_module_not_found() {
        let result = find_module(&[PathBuf::from("/nonexistent")], "fake-module");
        assert!(result.is_none());
    }
}
