use std::path::PathBuf;

use super::discovery::*;

// ============================================================================
// library_extension
// ============================================================================

#[test]
fn library_extension_is_platform_correct() {
    let ext = library_extension();
    #[cfg(target_os = "linux")]
    assert_eq!(ext, "so");
    #[cfg(target_os = "macos")]
    assert_eq!(ext, "dylib");
    #[cfg(target_os = "windows")]
    assert_eq!(ext, "dll");
}

// ============================================================================
// library_filename
// ============================================================================

#[test]
fn library_filename_simple_name() {
    let name = library_filename("treesitter");
    #[cfg(target_os = "linux")]
    assert_eq!(name, "libreovim_module_treesitter.so");
    #[cfg(target_os = "macos")]
    assert_eq!(name, "libreovim_module_treesitter.dylib");
    #[cfg(target_os = "windows")]
    assert_eq!(name, "reovim_module_treesitter.dll");
}

#[test]
fn library_filename_hyphenated_name() {
    let name = library_filename("treesitter-rust");
    #[cfg(target_os = "linux")]
    assert_eq!(name, "libreovim_module_treesitter_rust.so");
    #[cfg(target_os = "macos")]
    assert_eq!(name, "libreovim_module_treesitter_rust.dylib");
    #[cfg(target_os = "windows")]
    assert_eq!(name, "reovim_module_treesitter_rust.dll");
}

#[test]
fn library_filename_single_char() {
    let name = library_filename("x");
    assert!(name.contains("reovim_module_x"));
}

// ============================================================================
// default_search_paths
// ============================================================================

#[test]
fn default_search_paths_not_empty() {
    let paths = default_search_paths();
    // At minimum, system paths exist (on Unix) or user path
    assert!(!paths.is_empty());
}

#[test]
fn default_search_paths_contains_system_paths() {
    let paths = default_search_paths();

    #[cfg(unix)]
    {
        assert!(paths.contains(&PathBuf::from("/usr/lib/reovim/modules")));
        assert!(paths.contains(&PathBuf::from("/usr/local/lib/reovim/modules")));
    }
}

#[test]
#[allow(unsafe_code)]
fn default_search_paths_env_var_prepends() {
    // SAFETY: `set_var`/`remove_var` are unsafe in Rust 2024 (not thread-safe).
    // Acceptable in test code running with `--test-threads=1` or isolated env.
    unsafe {
        let orig = std::env::var_os(MODULE_PATH_ENV);
        std::env::set_var(MODULE_PATH_ENV, "/custom/path1:/custom/path2");

        let paths = default_search_paths();
        assert!(paths.len() >= 2);
        assert_eq!(paths[0], PathBuf::from("/custom/path1"));
        assert_eq!(paths[1], PathBuf::from("/custom/path2"));

        // Restore
        if let Some(val) = orig {
            std::env::set_var(MODULE_PATH_ENV, val);
        } else {
            std::env::remove_var(MODULE_PATH_ENV);
        }
    }
}

// ============================================================================
// discover_modules
// ============================================================================

#[test]
fn discover_empty_paths() {
    let found = discover_modules(&[]);
    assert!(found.is_empty());
}

#[test]
fn discover_nonexistent_path() {
    let found = discover_modules(&[PathBuf::from("/nonexistent/path/modules")]);
    assert!(found.is_empty());
}

#[test]
fn discover_with_temp_dir_finds_matching_files() {
    let dir = std::env::temp_dir().join(format!(
        "reovim-discover-test-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();

    // Create files matching the naming convention
    let ext = library_extension();
    let matching = dir.join(format!("libreovim_module_test.{ext}"));
    let non_matching = dir.join(format!("libother.{ext}"));
    let wrong_ext = dir.join("libreovim_module_test.txt");

    std::fs::write(&matching, b"fake").unwrap();
    std::fs::write(&non_matching, b"fake").unwrap();
    std::fs::write(&wrong_ext, b"fake").unwrap();

    let found = discover_modules(std::slice::from_ref(&dir));
    assert_eq!(found.len(), 1);
    assert_eq!(found[0], matching);

    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn discover_multiple_modules_in_dir() {
    let dir = std::env::temp_dir().join(format!(
        "reovim-discover-multi-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();

    let ext = library_extension();
    let m1 = dir.join(format!("libreovim_module_foo.{ext}"));
    let m2 = dir.join(format!("libreovim_module_bar.{ext}"));
    std::fs::write(&m1, b"fake").unwrap();
    std::fs::write(&m2, b"fake").unwrap();

    let found = discover_modules(std::slice::from_ref(&dir));
    assert_eq!(found.len(), 2);

    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn discover_searches_multiple_paths() {
    let dir1 = std::env::temp_dir().join(format!(
        "reovim-discover-p1-{}",
        std::process::id()
    ));
    let dir2 = std::env::temp_dir().join(format!(
        "reovim-discover-p2-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir1).unwrap();
    std::fs::create_dir_all(&dir2).unwrap();

    let ext = library_extension();
    let m1 = dir1.join(format!("libreovim_module_a.{ext}"));
    let m2 = dir2.join(format!("libreovim_module_b.{ext}"));
    std::fs::write(&m1, b"fake").unwrap();
    std::fs::write(&m2, b"fake").unwrap();

    let found = discover_modules(&[dir1.clone(), dir2.clone()]);
    assert_eq!(found.len(), 2);

    std::fs::remove_dir_all(&dir1).unwrap();
    std::fs::remove_dir_all(&dir2).unwrap();
}

// ============================================================================
// find_module
// ============================================================================

#[test]
fn find_module_not_found() {
    let result = find_module(&[PathBuf::from("/nonexistent")], "fake-module");
    assert!(result.is_none());
}

#[test]
fn find_module_in_temp_dir() {
    let dir = std::env::temp_dir().join(format!(
        "reovim-find-test-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();

    let filename = library_filename("test-mod");
    let file_path = dir.join(&filename);
    std::fs::write(&file_path, b"fake").unwrap();

    let found = find_module(std::slice::from_ref(&dir), "test-mod");
    assert_eq!(found, Some(file_path));

    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn find_module_returns_first_match() {
    let dir1 = std::env::temp_dir().join(format!(
        "reovim-find-first1-{}",
        std::process::id()
    ));
    let dir2 = std::env::temp_dir().join(format!(
        "reovim-find-first2-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir1).unwrap();
    std::fs::create_dir_all(&dir2).unwrap();

    let filename = library_filename("dup");
    let path1 = dir1.join(&filename);
    let path2 = dir2.join(&filename);
    std::fs::write(&path1, b"v1").unwrap();
    std::fs::write(&path2, b"v2").unwrap();

    // dir1 is first in search paths, so it should be found first
    let found = find_module(&[dir1.clone(), dir2.clone()], "dup");
    assert_eq!(found, Some(path1));

    std::fs::remove_dir_all(&dir1).unwrap();
    std::fs::remove_dir_all(&dir2).unwrap();
}

#[test]
fn find_module_empty_search_paths() {
    let result = find_module(&[], "anything");
    assert!(result.is_none());
}

// ============================================================================
// module_name_from_path
// ============================================================================

#[test]
fn module_name_from_path_linux_style() {
    let path = std::path::Path::new("/usr/lib/reovim/modules/libreovim_module_treesitter_rust.so");
    assert_eq!(
        module_name_from_path(path),
        Some("treesitter-rust".to_string())
    );
}

#[test]
fn module_name_from_path_without_lib_prefix() {
    // On Windows the prefix is "reovim_module_" not "libreovim_module_"
    // Test the no-lib-prefix variant using a plain filename (works on all platforms)
    let path = std::path::Path::new("reovim_module_foo_bar.dll");
    assert_eq!(
        module_name_from_path(path),
        Some("foo-bar".to_string())
    );
}

#[test]
fn module_name_from_path_non_module_file() {
    let path = std::path::Path::new("/usr/lib/libother.so");
    assert!(module_name_from_path(path).is_none());
}

#[test]
fn module_name_from_path_no_extension() {
    let path = std::path::Path::new("/usr/lib/libreovim_module_test");
    assert_eq!(
        module_name_from_path(path),
        Some("test".to_string())
    );
}

#[test]
fn module_name_from_path_simple_name() {
    let ext = library_extension();
    let filename = format!("libreovim_module_vim.{ext}");
    let path = std::path::Path::new(&filename);
    assert_eq!(module_name_from_path(path), Some("vim".to_string()));
}
