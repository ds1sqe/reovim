use std::path::PathBuf;

use super::*;

#[test]
fn default_search_paths_not_empty() {
    let paths = default_client_search_paths();
    // At minimum, the system paths are always included
    assert!(
        paths.iter().any(|p| p.ends_with("client-modules")),
        "should include at least one client-modules path"
    );
}

#[test]
fn client_library_filename_format() {
    let name = client_library_filename("statusline");
    if cfg!(target_os = "macos") {
        assert_eq!(name, "libreovim_client_module_statusline.dylib");
    } else {
        assert_eq!(name, "libreovim_client_module_statusline.so");
    }
}

#[test]
fn discover_empty_dir() {
    let temp = std::env::temp_dir().join("reovim_test_discover_empty");
    let _ = std::fs::create_dir_all(&temp);
    let found = discover_client_modules(std::slice::from_ref(&temp));
    assert!(found.is_empty());
    let _ = std::fs::remove_dir_all(&temp);
}

#[test]
fn discover_matches_prefix() {
    let temp = std::env::temp_dir().join("reovim_test_discover_prefix");
    let _ = std::fs::create_dir_all(&temp);

    // Create matching and non-matching files
    let ext = if cfg!(target_os = "macos") { "dylib" } else { "so" };
    let matching = temp.join(format!("libreovim_client_module_test.{ext}"));
    let non_matching = temp.join(format!("libsomething_else.{ext}"));
    std::fs::write(&matching, b"").unwrap();
    std::fs::write(&non_matching, b"").unwrap();

    let found = discover_client_modules(std::slice::from_ref(&temp));
    assert_eq!(found.len(), 1);
    assert_eq!(found[0], matching);

    let _ = std::fs::remove_dir_all(&temp);
}

#[test]
fn find_module_by_name() {
    let temp = std::env::temp_dir().join("reovim_test_find_by_name");
    let _ = std::fs::create_dir_all(&temp);

    let filename = client_library_filename("hover");
    let path = temp.join(&filename);
    std::fs::write(&path, b"").unwrap();

    let found = find_client_module(std::slice::from_ref(&temp), "hover");
    assert_eq!(found, Some(path));

    let not_found = find_client_module(std::slice::from_ref(&temp), "nonexistent");
    assert!(not_found.is_none());

    let _ = std::fs::remove_dir_all(&temp);
}

#[test]
fn module_name_from_path_valid() {
    let path = PathBuf::from("/usr/lib/reovim/client-modules/libreovim_client_module_statusline.so");
    assert_eq!(
        client_module_name_from_path(&path),
        Some("statusline".to_string())
    );
}

#[test]
fn module_name_from_path_dylib() {
    let path = PathBuf::from("/opt/reovim/libreovim_client_module_hover.dylib");
    assert_eq!(
        client_module_name_from_path(&path),
        Some("hover".to_string())
    );
}

#[test]
fn module_name_from_path_no_prefix() {
    let path = PathBuf::from("/lib/libsomething.so");
    assert!(client_module_name_from_path(&path).is_none());
}
