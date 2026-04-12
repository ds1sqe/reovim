use {
    super::*,
    reovim_driver_module_registry::{InstalledModule, InstalledModules, RegistryError},
};

/// Create a unique temp directory for the test, returning its path.
/// The caller is responsible for removing it when done.
fn make_test_dir(label: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir()
        .join(format!("reovim-bin-test-{label}-{:?}", std::thread::current().id()));
    // Remove any leftover from a prior run, then create fresh.
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create test dir");
    dir
}

#[test]
fn test_parse_source_git_https() {
    let source = parse_source("https://github.com/user/repo.git", None);
    assert!(source.is_git());
    assert!(!source.is_path());
}

#[test]
fn test_parse_source_git_ssh() {
    let source = parse_source("git@github.com:user/repo.git", None);
    assert!(source.is_git());
}

#[test]
fn test_parse_source_git_with_rev() {
    let source = parse_source("https://github.com/user/repo.git", Some("v1.0.0"));
    match source {
        ModuleSource::Git { url, rev } => {
            assert_eq!(url, "https://github.com/user/repo.git");
            assert_eq!(rev, Some("v1.0.0".to_string()));
        }
        ModuleSource::Path { .. } => panic!("expected git source"),
    }
}

#[test]
fn test_parse_source_local_path() {
    let source = parse_source("/home/user/my-module", None);
    assert!(source.is_path());
    assert!(!source.is_git());
}

#[test]
fn test_parse_source_relative_path() {
    let source = parse_source("./my-module", None);
    assert!(source.is_path());
}

#[test]
fn test_parse_source_http() {
    let source = parse_source("http://example.com/repo.git", None);
    assert!(source.is_git());
}

// ============================================================================
// parse_source edge cases
// ============================================================================

#[test]
fn test_parse_source_empty_string() {
    // An empty string is not a URL prefix, so it is treated as a local path.
    let source = parse_source("", None);
    assert!(source.is_path());
    assert!(!source.is_git());
}

#[test]
fn test_parse_source_with_spaces() {
    // Paths with spaces should be treated as local paths.
    let source = parse_source("/home/user/my module", None);
    assert!(source.is_path());
    assert!(!source.is_git());
}

// ============================================================================
// Workflow error-path tests via RegistryPaths injection
// ============================================================================

#[test]
fn test_remove_nonexistent_module() {
    let dir = make_test_dir("remove-nonexistent");
    let paths = RegistryPaths::new(dir.clone());

    let result = workflow::remove("nonexistent", &paths);

    std::fs::remove_dir_all(&dir).ok();
    assert!(
        matches!(result, Err(RegistryError::NotInstalled(_))),
        "expected NotInstalled, got: {result:?}"
    );
}

#[test]
fn test_info_nonexistent_module() {
    let dir = make_test_dir("info-nonexistent");
    let paths = RegistryPaths::new(dir.clone());

    let result = workflow::info("nonexistent", &paths);

    std::fs::remove_dir_all(&dir).ok();
    assert!(
        matches!(result, Err(RegistryError::NotInstalled(_))),
        "expected NotInstalled, got: {result:?}"
    );
}

#[test]
fn test_check_empty_registry() {
    let dir = make_test_dir("check-empty");
    let paths = RegistryPaths::new(dir.clone());

    let report = workflow::check(&paths).expect("check should succeed on empty registry");

    std::fs::remove_dir_all(&dir).ok();
    assert!(report.is_clean(), "expected clean report for empty registry");
    assert!(report.valid.is_empty());
    assert!(report.broken.is_empty());
    assert!(report.orphaned.is_empty());
}

#[test]
fn test_list_empty_registry() {
    let dir = make_test_dir("list-empty");
    let paths = RegistryPaths::new(dir.clone());

    let modules = workflow::list(&paths).expect("list should succeed on empty registry");

    std::fs::remove_dir_all(&dir).ok();
    assert!(modules.is_empty(), "expected no modules in empty registry");
}

#[test]
fn test_resolve_empty_registry() {
    let dir = make_test_dir("resolve-empty");
    let paths = RegistryPaths::new(dir.clone());

    let modules = workflow::resolve(&paths).expect("resolve should succeed on empty registry");

    std::fs::remove_dir_all(&dir).ok();
    assert!(modules.is_empty(), "expected no modules in empty registry");
}

#[test]
fn test_install_constraint_violation_reported() {
    let dir = make_test_dir("install-constraint");
    let paths = RegistryPaths::new(dir.clone());
    std::fs::create_dir_all(&paths.modules_dir).unwrap();

    // Write installed.json with a module at version 0.8.0.
    let vim_install_dir = dir.join("vim");
    std::fs::create_dir_all(&vim_install_dir).unwrap();
    let mut installed = InstalledModules::new();
    installed.insert(InstalledModule {
        id: "vim".to_string(),
        version: "0.8.0".to_string(),
        source: ModuleSource::path(vim_install_dir.to_string_lossy().into_owned()),
        install_path: vim_install_dir,
        library_path: None,
    });
    installed
        .save(&paths.installed_json)
        .expect("save installed.json");

    // Create a source dir with a module.toml requiring vim ^0.9.0 (violated by 0.8.0).
    let source_dir = dir.join("new-mod-src");
    std::fs::create_dir_all(&source_dir).unwrap();
    std::fs::write(
        source_dir.join("module.toml"),
        "[module]\nid = \"new-mod\"\nname = \"New Mod\"\nversion = \"1.0.0\"\n\n[dependencies]\nvim = \"^0.9.0\"\n",
    )
    .unwrap();

    // workflow::install checks constraints before calling cargo build, so the
    // path-source install path returns ConstraintViolation without shelling out.
    let result =
        workflow::install(&ModuleSource::path(source_dir.to_string_lossy().into_owned()), &paths);

    std::fs::remove_dir_all(&dir).ok();
    assert!(
        matches!(result, Err(RegistryError::ConstraintViolation(_))),
        "expected ConstraintViolation, got: {result:?}"
    );
}
