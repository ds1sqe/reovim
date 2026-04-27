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
    let dir = std::env::temp_dir().join(format!("reovim-discover-test-{}", std::process::id()));
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
    let dir = std::env::temp_dir().join(format!("reovim-discover-multi-{}", std::process::id()));
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
    let dir1 = std::env::temp_dir().join(format!("reovim-discover-p1-{}", std::process::id()));
    let dir2 = std::env::temp_dir().join(format!("reovim-discover-p2-{}", std::process::id()));
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
    let dir = std::env::temp_dir().join(format!("reovim-find-test-{}", std::process::id()));
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
    let dir1 = std::env::temp_dir().join(format!("reovim-find-first1-{}", std::process::id()));
    let dir2 = std::env::temp_dir().join(format!("reovim-find-first2-{}", std::process::id()));
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
    assert_eq!(module_name_from_path(path), Some("treesitter-rust".to_string()));
}

#[test]
fn module_name_from_path_without_lib_prefix() {
    // On Windows the prefix is "reovim_module_" not "libreovim_module_"
    // Test the no-lib-prefix variant using a plain filename (works on all platforms)
    let path = std::path::Path::new("reovim_module_foo_bar.dll");
    assert_eq!(module_name_from_path(path), Some("foo-bar".to_string()));
}

#[test]
fn module_name_from_path_non_module_file() {
    let path = std::path::Path::new("/usr/lib/libother.so");
    assert!(module_name_from_path(path).is_none());
}

#[test]
fn module_name_from_path_no_extension() {
    let path = std::path::Path::new("/usr/lib/libreovim_module_test");
    assert_eq!(module_name_from_path(path), Some("test".to_string()));
}

#[test]
fn module_name_from_path_simple_name() {
    let ext = library_extension();
    let filename = format!("libreovim_module_vim.{ext}");
    let path = std::path::Path::new(&filename);
    assert_eq!(module_name_from_path(path), Some("vim".to_string()));
}

// ============================================================================
// Edge cases: Unicode module names
// ============================================================================

#[test]
fn library_filename_unicode_name() {
    // Module names with unicode characters get hyphens replaced with underscores
    // just like ASCII names. The function does not reject them.
    let name = library_filename("treesitter-\u{00e9}ditor");
    assert!(name.contains("reovim_module_treesitter_\u{00e9}ditor"));
}

#[test]
fn discover_ignores_unicode_named_non_matching_files() {
    let dir = std::env::temp_dir().join(format!("reovim-discover-unicode-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    let ext = library_extension();
    // A file with unicode in the module name part — should still be discovered
    let matching = dir.join(format!("libreovim_module_\u{00e9}ditor.{ext}"));
    // A file with unicode but not matching the prefix
    let non_matching = dir.join(format!("lib\u{00e9}diteur.{ext}"));

    std::fs::write(&matching, b"fake").unwrap();
    std::fs::write(&non_matching, b"fake").unwrap();

    let found = discover_modules(std::slice::from_ref(&dir));
    assert_eq!(found.len(), 1);
    assert_eq!(found[0], matching);

    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn module_name_from_path_unicode_module() {
    let path = std::path::Path::new("libreovim_module_\u{00e9}diteur.so");
    assert_eq!(module_name_from_path(path), Some("\u{00e9}diteur".to_string()));
}

#[test]
fn find_module_unicode_name() {
    let dir = std::env::temp_dir().join(format!("reovim-find-unicode-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    let filename = library_filename("caf\u{00e9}");
    let file_path = dir.join(&filename);
    std::fs::write(&file_path, b"fake").unwrap();

    let found = find_module(std::slice::from_ref(&dir), "caf\u{00e9}");
    assert_eq!(found, Some(file_path));

    std::fs::remove_dir_all(&dir).unwrap();
}

// ============================================================================
// Edge cases: Very long path names
// ============================================================================

#[test]
fn discover_with_very_long_directory_name() {
    // Create a deeply nested directory with a long total path
    let long_component = "a".repeat(200);
    let dir = std::env::temp_dir()
        .join(format!("reovim-longpath-{}", std::process::id()))
        .join(&long_component);
    std::fs::create_dir_all(&dir).unwrap();

    let ext = library_extension();
    let matching = dir.join(format!("libreovim_module_test.{ext}"));
    std::fs::write(&matching, b"fake").unwrap();

    let found = discover_modules(std::slice::from_ref(&dir));
    assert_eq!(found.len(), 1);
    assert_eq!(found[0], matching);

    // Clean up from the top-level temp dir
    let top = std::env::temp_dir().join(format!("reovim-longpath-{}", std::process::id()));
    std::fs::remove_dir_all(&top).unwrap();
}

#[test]
fn find_module_in_long_path() {
    let long_component = "b".repeat(200);
    let dir = std::env::temp_dir()
        .join(format!("reovim-findlong-{}", std::process::id()))
        .join(&long_component);
    std::fs::create_dir_all(&dir).unwrap();

    let filename = library_filename("long-path-mod");
    let file_path = dir.join(&filename);
    std::fs::write(&file_path, b"fake").unwrap();

    let found = find_module(std::slice::from_ref(&dir), "long-path-mod");
    assert_eq!(found, Some(file_path));

    let top = std::env::temp_dir().join(format!("reovim-findlong-{}", std::process::id()));
    std::fs::remove_dir_all(&top).unwrap();
}

#[test]
fn library_filename_very_long_name() {
    let long_name = "x".repeat(255);
    let filename = library_filename(&long_name);
    // Should contain the full name (no truncation in the function)
    assert!(filename.contains(&format!("reovim_module_{long_name}")));
}

// ============================================================================
// Edge cases: Symlink handling
// ============================================================================

#[cfg(unix)]
#[test]
fn discover_follows_symlinked_files() {
    let dir = std::env::temp_dir().join(format!("reovim-symlink-{}", std::process::id()));
    let real_dir = dir.join("real");
    let link_dir = dir.join("links");
    std::fs::create_dir_all(&real_dir).unwrap();
    std::fs::create_dir_all(&link_dir).unwrap();

    let ext = library_extension();
    let real_file = real_dir.join(format!("libreovim_module_sym.{ext}"));
    std::fs::write(&real_file, b"fake").unwrap();

    // Create a symlink to the real file inside the link directory
    let link_file = link_dir.join(format!("libreovim_module_sym.{ext}"));
    std::os::unix::fs::symlink(&real_file, &link_file).unwrap();

    let found = discover_modules(std::slice::from_ref(&link_dir));
    assert_eq!(found.len(), 1);
    assert_eq!(found[0], link_file);

    std::fs::remove_dir_all(&dir).unwrap();
}

#[cfg(unix)]
#[test]
fn discover_follows_symlinked_directory() {
    let dir = std::env::temp_dir().join(format!("reovim-symdir-{}", std::process::id()));
    let real_dir = dir.join("real_modules");
    std::fs::create_dir_all(&real_dir).unwrap();

    let ext = library_extension();
    let module_file = real_dir.join(format!("libreovim_module_linked.{ext}"));
    std::fs::write(&module_file, b"fake").unwrap();

    // Create a symlink to the real directory
    let link_dir = dir.join("linked_modules");
    std::os::unix::fs::symlink(&real_dir, &link_dir).unwrap();

    let found = discover_modules(std::slice::from_ref(&link_dir));
    assert_eq!(found.len(), 1);

    std::fs::remove_dir_all(&dir).unwrap();
}

#[cfg(unix)]
#[test]
fn find_module_through_symlink() {
    let dir = std::env::temp_dir().join(format!("reovim-findsym-{}", std::process::id()));
    let real_dir = dir.join("real");
    let link_dir = dir.join("search");
    std::fs::create_dir_all(&real_dir).unwrap();
    std::fs::create_dir_all(&link_dir).unwrap();

    let filename = library_filename("symmod");
    let real_file = real_dir.join(&filename);
    std::fs::write(&real_file, b"fake").unwrap();

    let link_file = link_dir.join(&filename);
    std::os::unix::fs::symlink(&real_file, &link_file).unwrap();

    let found = find_module(std::slice::from_ref(&link_dir), "symmod");
    assert_eq!(found, Some(link_file));

    std::fs::remove_dir_all(&dir).unwrap();
}

#[cfg(unix)]
#[test]
fn discover_handles_broken_symlink() {
    let dir = std::env::temp_dir().join(format!("reovim-brokensym-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    let ext = library_extension();
    // Create a symlink pointing to a non-existent file
    let broken_link = dir.join(format!("libreovim_module_broken.{ext}"));
    std::os::unix::fs::symlink("/nonexistent/target", &broken_link).unwrap();

    // Also create a valid file to ensure discovery still works
    let valid_file = dir.join(format!("libreovim_module_valid.{ext}"));
    std::fs::write(&valid_file, b"fake").unwrap();

    let found = discover_modules(std::slice::from_ref(&dir));
    // Broken symlinks may or may not appear depending on OS behavior with
    // read_dir; the important thing is that discovery doesn't panic and
    // still finds the valid file.
    assert!(found.contains(&valid_file));

    std::fs::remove_dir_all(&dir).unwrap();
}
