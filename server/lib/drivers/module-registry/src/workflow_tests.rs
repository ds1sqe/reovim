use std::path::PathBuf;

use super::*;

// ============================================================================
// RegistryPaths tests
// ============================================================================

#[test]
fn test_registry_paths_new() {
    let paths = RegistryPaths::new(PathBuf::from("/tmp/test-modules"));
    assert_eq!(paths.modules_dir, PathBuf::from("/tmp/test-modules"));
    assert_eq!(paths.installed_json, PathBuf::from("/tmp/test-modules/installed.json"));
}

#[test]
fn test_registry_paths_default() {
    let paths = RegistryPaths::default_paths();
    let dir_str = paths.modules_dir.to_string_lossy();
    assert!(dir_str.contains("reovim"));
    assert!(dir_str.contains("modules"));
}

// ============================================================================
// RegistryError tests
// ============================================================================

#[test]
fn test_error_display() {
    let err = RegistryError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "not found"));
    assert!(err.to_string().contains("IO error"));

    let err = RegistryError::Git("clone failed".into());
    assert!(err.to_string().contains("git error"));

    let err = RegistryError::Build("compile failed".into());
    assert!(err.to_string().contains("build error"));

    let err = RegistryError::Manifest("bad toml".into());
    assert!(err.to_string().contains("manifest error"));

    let err = RegistryError::NotInstalled("foo".into());
    assert!(err.to_string().contains("not installed"));

    let err = RegistryError::AlreadyInstalled("foo".into());
    assert!(err.to_string().contains("already installed"));

    let err = RegistryError::Metadata("bad json".into());
    assert!(err.to_string().contains("metadata error"));
}

#[test]
fn test_error_source() {
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "not found");
    let err = RegistryError::Io(io_err);
    assert!(std::error::Error::source(&err).is_some());

    let err = RegistryError::Git("fail".into());
    assert!(std::error::Error::source(&err).is_none());
}

#[test]
fn test_error_from_io() {
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "not found");
    let err: RegistryError = io_err.into();
    assert!(matches!(err, RegistryError::Io(_)));
}

// ============================================================================
// CheckReport tests
// ============================================================================

#[test]
fn test_check_report_empty_is_clean() {
    let report = CheckReport::default();
    assert!(report.is_clean());
}

#[test]
fn test_check_report_with_broken_not_clean() {
    let mut report = CheckReport::default();
    report.broken.push(("foo".into(), "missing library".into()));
    assert!(!report.is_clean());
}

#[test]
fn test_check_report_with_orphaned_not_clean() {
    let mut report = CheckReport::default();
    report.orphaned.push(PathBuf::from("/tmp/orphan.so"));
    assert!(!report.is_clean());
}

// ============================================================================
// Workflow operations with filesystem
// ============================================================================

#[test]
fn test_list_empty_registry() {
    let dir = tempfile::tempdir().unwrap();
    let paths = RegistryPaths::new(dir.path().to_path_buf());
    let modules = list(&paths).unwrap();
    assert!(modules.is_empty());
}

#[test]
fn test_resolve_empty_registry() {
    let dir = tempfile::tempdir().unwrap();
    let paths = RegistryPaths::new(dir.path().to_path_buf());
    let modules = resolve(&paths).unwrap();
    assert!(modules.is_empty());
}

#[test]
fn test_check_empty_registry() {
    let dir = tempfile::tempdir().unwrap();
    let paths = RegistryPaths::new(dir.path().to_path_buf());
    let report = check(&paths).unwrap();
    assert!(report.is_clean());
}

#[test]
fn test_remove_not_installed() {
    let dir = tempfile::tempdir().unwrap();
    let paths = RegistryPaths::new(dir.path().to_path_buf());
    let result = remove("nonexistent", &paths);
    assert!(matches!(result, Err(RegistryError::NotInstalled(_))));
}

#[test]
fn test_info_not_installed() {
    let dir = tempfile::tempdir().unwrap();
    let paths = RegistryPaths::new(dir.path().to_path_buf());
    let result = info("nonexistent", &paths);
    assert!(matches!(result, Err(RegistryError::NotInstalled(_))));
}

#[test]
fn test_update_not_installed() {
    let dir = tempfile::tempdir().unwrap();
    let paths = RegistryPaths::new(dir.path().to_path_buf());
    let result = update("nonexistent", &paths);
    assert!(matches!(result, Err(RegistryError::NotInstalled(_))));
}

#[test]
fn test_check_detects_broken_library() {
    let dir = tempfile::tempdir().unwrap();
    let paths = RegistryPaths::new(dir.path().to_path_buf());

    // Manually insert a module with a nonexistent library path
    let mut installed = InstalledModules::new();
    installed.insert(InstalledModule {
        id: "test-mod".to_string(),
        version: "1.0.0".to_string(),
        source: ModuleSource::path("/tmp/fake"),
        install_path: dir.path().join("test-mod"),
        library_path: Some(dir.path().join("nonexistent.so")),
    });
    save_metadata(&installed, &paths).unwrap();

    let report = check(&paths).unwrap();
    assert!(!report.is_clean());
    assert_eq!(report.broken.len(), 1);
    assert_eq!(report.broken[0].0, "test-mod");
}

#[test]
fn test_check_detects_orphaned_so() {
    let dir = tempfile::tempdir().unwrap();
    let paths = RegistryPaths::new(dir.path().to_path_buf());
    std::fs::create_dir_all(&paths.modules_dir).unwrap();

    // Create an orphan .so file
    let orphan = paths.modules_dir.join("orphan.so");
    std::fs::write(&orphan, b"fake").unwrap();

    let report = check(&paths).unwrap();
    assert!(!report.is_clean());
    assert_eq!(report.orphaned.len(), 1);
}

#[test]
fn test_list_with_installed_modules() {
    let dir = tempfile::tempdir().unwrap();
    let paths = RegistryPaths::new(dir.path().to_path_buf());

    let mut installed = InstalledModules::new();
    installed.insert(InstalledModule {
        id: "beta".to_string(),
        version: "2.0.0".to_string(),
        source: ModuleSource::path("/tmp/beta"),
        install_path: dir.path().join("beta"),
        library_path: None,
    });
    installed.insert(InstalledModule {
        id: "alpha".to_string(),
        version: "1.0.0".to_string(),
        source: ModuleSource::path("/tmp/alpha"),
        install_path: dir.path().join("alpha"),
        library_path: None,
    });
    save_metadata(&installed, &paths).unwrap();

    let modules = list(&paths).unwrap();
    assert_eq!(modules.len(), 2);
    // Should be sorted alphabetically
    assert_eq!(modules[0].id, "alpha");
    assert_eq!(modules[1].id, "beta");
}

#[test]
fn test_info_returns_details() {
    let dir = tempfile::tempdir().unwrap();
    let paths = RegistryPaths::new(dir.path().to_path_buf());

    // Create a module with a manifest
    let mod_dir = dir.path().join("test-mod");
    std::fs::create_dir_all(&mod_dir).unwrap();
    std::fs::write(
        mod_dir.join("module.toml"),
        r#"
[module]
id = "test-mod"
name = "Test Module"
version = "1.2.3"

[capabilities]
provides = ["test-cap"]
requires = ["lsp-provider"]
"#,
    )
    .unwrap();

    let lib_path = mod_dir.join("libtest_mod.so");
    std::fs::write(&lib_path, b"fake").unwrap();

    let mut installed = InstalledModules::new();
    installed.insert(InstalledModule {
        id: "test-mod".to_string(),
        version: "1.2.3".to_string(),
        source: ModuleSource::path(mod_dir.to_string_lossy().into_owned()),
        install_path: mod_dir.clone(),
        library_path: Some(lib_path),
    });
    save_metadata(&installed, &paths).unwrap();

    let module_info = info("test-mod", &paths).unwrap();
    assert_eq!(module_info.id, "test-mod");
    assert_eq!(module_info.version, "1.2.3");
    assert!(module_info.library_exists);
    assert_eq!(module_info.provides, vec!["test-cap"]);
    assert_eq!(module_info.requires, vec!["lsp-provider"]);
}

#[test]
fn test_remove_installed_module() {
    let dir = tempfile::tempdir().unwrap();
    let paths = RegistryPaths::new(dir.path().to_path_buf());

    // Install a git-sourced module (we own the directory)
    let mod_dir = paths.modules_dir.join("test-mod");
    std::fs::create_dir_all(&mod_dir).unwrap();

    let mut installed = InstalledModules::new();
    installed.insert(InstalledModule {
        id: "test-mod".to_string(),
        version: "1.0.0".to_string(),
        source: ModuleSource::git("https://example.com/test.git"),
        install_path: mod_dir.clone(),
        library_path: None,
    });
    save_metadata(&installed, &paths).unwrap();

    remove("test-mod", &paths).unwrap();

    // Directory should be removed
    assert!(!mod_dir.exists());
    // Metadata should be updated
    let remaining = list(&paths).unwrap();
    assert!(remaining.is_empty());
}

#[test]
fn test_remove_path_source_preserves_directory() {
    let dir = tempfile::tempdir().unwrap();
    let paths = RegistryPaths::new(dir.path().to_path_buf());

    // For path sources, we don't own the directory
    let source_dir = tempfile::tempdir().unwrap();

    let mut installed = InstalledModules::new();
    installed.insert(InstalledModule {
        id: "local-mod".to_string(),
        version: "1.0.0".to_string(),
        source: ModuleSource::path(source_dir.path().to_string_lossy().into_owned()),
        install_path: source_dir.path().to_path_buf(),
        library_path: None,
    });
    save_metadata(&installed, &paths).unwrap();

    remove("local-mod", &paths).unwrap();

    // Source directory should still exist (we don't own it)
    assert!(source_dir.path().exists());
}
