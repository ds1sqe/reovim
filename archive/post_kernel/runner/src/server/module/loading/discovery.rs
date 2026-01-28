//! Module discovery and search path utilities.
//!
//! Provides functions for finding module shared libraries on disk.

use std::path::PathBuf;

/// Default search paths for module discovery.
///
/// Follows XDG Base Directory specification using `reovim_arch::dirs` module
/// to maintain consistency with existing infrastructure.
#[must_use]
pub fn default_search_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // System paths (Linux/macOS)
    #[cfg(unix)]
    {
        paths.push(PathBuf::from("/usr/lib/reovim/modules"));
        paths.push(PathBuf::from("/usr/local/lib/reovim/modules"));
    }

    // User path - use reovim_arch::dirs (XDG_DATA_HOME/reovim/modules)
    if let Some(data_dir) = reovim_arch::dirs::data_local_dir() {
        paths.push(data_dir.join("reovim").join("modules"));
    }

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

/// Discover Python module files in search paths.
///
/// Scans all search paths for `.py` files.
#[cfg(feature = "python")]
#[must_use]
#[allow(dead_code)] // Public API for external use
pub fn discover_python_modules(search_paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut found = Vec::new();

    for path in search_paths {
        if !path.exists() {
            continue;
        }

        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                let file_path = entry.path();
                if file_path.extension().is_some_and(|e| e == "py") {
                    // Skip __pycache__, __init__.py, etc.
                    if let Some(name) = file_path.file_name().and_then(|n| n.to_str())
                        && !name.starts_with('_')
                    {
                        found.push(file_path);
                    }
                }
            }
        }
    }

    found
}

/// Find a Python module by name in search paths.
///
/// Searches for a `.py` file matching the given module name.
/// Returns the first match found.
#[cfg(feature = "python")]
#[must_use]
#[allow(dead_code)] // Public API for external use
pub fn find_python_module(search_paths: &[PathBuf], name: &str) -> Option<PathBuf> {
    let filename = format!("{name}.py");

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
        if let Some(data_dir) = reovim_arch::dirs::data_local_dir() {
            let user_modules = data_dir.join("reovim").join("modules");
            assert!(paths.contains(&user_modules));
        }
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

    // ========================================================================
    // Python Discovery Tests (require python feature)
    // ========================================================================

    #[cfg(feature = "python")]
    mod python_tests {
        use std::io::Write;

        use {super::*, tempfile::TempDir};

        #[test]
        fn test_discover_python_modules_empty() {
            let found = super::discover_python_modules(&[]);
            assert!(found.is_empty());
        }

        #[test]
        fn test_discover_python_modules_nonexistent_path() {
            let found = super::discover_python_modules(&[PathBuf::from("/nonexistent/path")]);
            assert!(found.is_empty());
        }

        #[test]
        fn test_discover_python_modules_finds_py_files() {
            let temp_dir = TempDir::new().unwrap();
            let module_path = temp_dir.path().join("my_module.py");
            std::fs::File::create(&module_path)
                .unwrap()
                .write_all(b"# test")
                .unwrap();

            let found = super::discover_python_modules(&[temp_dir.path().to_path_buf()]);
            assert_eq!(found.len(), 1);
            assert_eq!(found[0], module_path);
        }

        #[test]
        fn test_discover_python_modules_skips_private_files() {
            let temp_dir = TempDir::new().unwrap();

            // Create regular module
            let module_path = temp_dir.path().join("my_module.py");
            std::fs::File::create(&module_path)
                .unwrap()
                .write_all(b"# test")
                .unwrap();

            // Create private files that should be skipped
            std::fs::File::create(temp_dir.path().join("__init__.py"))
                .unwrap()
                .write_all(b"")
                .unwrap();
            std::fs::File::create(temp_dir.path().join("_private.py"))
                .unwrap()
                .write_all(b"")
                .unwrap();

            let found = super::discover_python_modules(&[temp_dir.path().to_path_buf()]);
            assert_eq!(found.len(), 1);
            assert_eq!(found[0], module_path);
        }

        #[test]
        fn test_find_python_module_not_found() {
            let result = super::find_python_module(&[PathBuf::from("/nonexistent")], "fake");
            assert!(result.is_none());
        }

        #[test]
        fn test_find_python_module_found() {
            let temp_dir = TempDir::new().unwrap();
            let module_path = temp_dir.path().join("my_module.py");
            std::fs::File::create(&module_path)
                .unwrap()
                .write_all(b"# test")
                .unwrap();

            let result = super::find_python_module(&[temp_dir.path().to_path_buf()], "my_module");
            assert_eq!(result, Some(module_path));
        }
    }
}
