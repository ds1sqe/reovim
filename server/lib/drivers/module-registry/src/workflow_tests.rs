use std::path::PathBuf;

use super::*;

fn create_module_crate(
    root: &std::path::Path,
    id: &str,
    version: &str,
    dependencies: &[(&str, &str)],
) -> PathBuf {
    std::fs::create_dir_all(root.join("src")).unwrap();

    let dependency_lines = if dependencies.is_empty() {
        String::new()
    } else {
        let deps = dependencies
            .iter()
            .map(|(target, range)| format!("{target} = \"{range}\""))
            .collect::<Vec<_>>()
            .join("\n");
        format!("\n[dependencies]\n{deps}\n")
    };

    std::fs::write(
        root.join("module.toml"),
        format!(
            "[module]\nid = \"{id}\"\nname = \"{id}\"\nversion = \"{version}\"\n{dependency_lines}"
        ),
    )
    .unwrap();

    std::fs::write(
        root.join("Cargo.toml"),
        format!(
            "[package]\nname = \"{id}\"\nversion = \"{version}\"\nedition = \"2024\"\n\n[lib]\ncrate-type = [\"cdylib\"]\n\n[features]\ndynamic = []\n"
        ),
    )
    .unwrap();

    std::fs::write(root.join("src/lib.rs"), "pub fn sample() {}\n").unwrap();
    root.to_path_buf()
}

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

// XDG/HOME env var branches in `default_paths` require `set_var`/`remove_var`
// which are unsafe in Rust 2024 and forbidden by workspace `deny(unsafe_code)`.
// The branches are covered by the existing `test_registry_paths_default` test
// (which exercises the current environment's branch). The alternate branches
// are marked `coverage(off)` in the source.

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

    let err = RegistryError::ConstraintViolation("dep mismatch".into());
    assert!(err.to_string().contains("version constraint violation"));
    assert!(err.to_string().contains("dep mismatch"));
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

#[test]
fn test_check_report_with_constraint_violations_not_clean() {
    let mut report = CheckReport::default();
    report.constraint_violations.push("vim requires ^0.9.0".into());
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
fn test_check_install_constraints_no_deps_ok() {
    let manifest = ModuleManifest::parse(
        r#"
        [module]
        id = "new-mod"
        name = "New"
        version = "1.0.0"
        "#,
    )
    .unwrap();
    let installed = InstalledModules::new();
    assert!(check_install_constraints(&manifest, &installed).is_ok());
}

#[test]
fn test_check_install_constraints_satisfied() {
    let dir = tempfile::tempdir().unwrap();
    let vim_dir = dir.path().join("vim");
    create_module_crate(&vim_dir, "vim", "0.9.1", &[]);

    let manifest = ModuleManifest::parse(
        r#"
        [module]
        id = "new-mod"
        name = "New"
        version = "1.0.0"

        [dependencies]
        vim = "^0.9.0"
        "#,
    )
    .unwrap();
    let mut installed = InstalledModules::new();
    installed.insert(InstalledModule {
        id: "vim".into(),
        version: "0.9.1".into(),
        source: ModuleSource::path(vim_dir.to_string_lossy().into_owned()),
        install_path: vim_dir,
        library_path: None,
    });
    assert!(check_install_constraints(&manifest, &installed).is_ok());
}

#[test]
fn test_check_install_constraints_violated() {
    let dir = tempfile::tempdir().unwrap();
    let vim_dir = dir.path().join("vim");
    create_module_crate(&vim_dir, "vim", "0.8.0", &[]);

    let manifest = ModuleManifest::parse(
        r#"
        [module]
        id = "new-mod"
        name = "New"
        version = "1.0.0"

        [dependencies]
        vim = "^0.9.0"
        "#,
    )
    .unwrap();
    let mut installed = InstalledModules::new();
    installed.insert(InstalledModule {
        id: "vim".into(),
        version: "0.8.0".into(),
        source: ModuleSource::path(vim_dir.to_string_lossy().into_owned()),
        install_path: vim_dir,
        library_path: None,
    });

    let result = check_install_constraints(&manifest, &installed);
    assert!(matches!(result, Err(RegistryError::ConstraintViolation(_))));
}

#[test]
fn test_check_install_constraints_reverse_violation() {
    let dir = tempfile::tempdir().unwrap();
    let existing_dir = dir.path().join("existing-mod");
    create_module_crate(&existing_dir, "existing-mod", "1.0.0", &[("new-mod", "^2.0.0")]);

    let new_manifest = ModuleManifest::parse(
        r#"
        [module]
        id = "new-mod"
        name = "New"
        version = "1.0.0"
        "#,
    )
    .unwrap();

    let mut installed = InstalledModules::new();
    installed.insert(InstalledModule {
        id: "existing-mod".into(),
        version: "1.0.0".into(),
        source: ModuleSource::path(existing_dir.to_string_lossy().into_owned()),
        install_path: existing_dir,
        library_path: None,
    });

    let result = check_install_constraints(&new_manifest, &installed);
    assert!(matches!(result, Err(RegistryError::ConstraintViolation(_))));
}

#[test]
fn test_install_constraint_violation_does_not_write_installed_json() {
    let dir = tempfile::tempdir().unwrap();
    let paths = RegistryPaths::new(dir.path().join("registry"));
    std::fs::create_dir_all(&paths.modules_dir).unwrap();

    let mut installed = InstalledModules::new();
    installed.insert(InstalledModule {
        id: "vim".into(),
        version: "0.8.0".into(),
        source: ModuleSource::path("/fake"),
        install_path: dir.path().join("vim"),
        library_path: None,
    });
    save_metadata(&installed, &paths).unwrap();
    let before = std::fs::read_to_string(&paths.installed_json).unwrap();

    let source_dir = dir.path().join("new-mod");
    create_module_crate(&source_dir, "new-mod", "1.0.0", &[("vim", "^0.9.0")]);

    let result = install(&ModuleSource::path(source_dir.to_string_lossy().into_owned()), &paths);
    assert!(matches!(result, Err(RegistryError::ConstraintViolation(_))));

    let after = std::fs::read_to_string(&paths.installed_json).unwrap();
    assert_eq!(before, after);
}

#[test]
fn test_parse_semver_str_valid() {
    assert_eq!(parse_semver_str("1.2.3"), Some((1, 2, 3)));
    assert_eq!(parse_semver_str("0.9.0"), Some((0, 9, 0)));
}

#[test]
fn test_parse_semver_str_with_metadata() {
    assert_eq!(parse_semver_str("1.0.0-alpha"), Some((1, 0, 0)));
    assert_eq!(parse_semver_str("1.0.0+build.1"), Some((1, 0, 0)));
}

#[test]
fn test_parse_semver_str_invalid() {
    assert_eq!(parse_semver_str("not-a-version"), None);
    assert_eq!(parse_semver_str("1.2"), None);
}

#[test]
fn test_check_install_constraints_invalid_new_constraint_string_is_violation() {
    let manifest = ModuleManifest::parse(
        r#"
        [module]
        id = "new-mod"
        name = "New"
        version = "1.0.0"

        [dependencies]
        vim = "not-a-range"
        "#,
    )
    .unwrap();

    let result = check_install_constraints(&manifest, &InstalledModules::new());
    assert!(matches!(result, Err(RegistryError::ConstraintViolation(_))));
}

#[test]
fn test_check_install_constraints_invalid_installed_version_is_violation() {
    let manifest = ModuleManifest::parse(
        r#"
        [module]
        id = "new-mod"
        name = "New"
        version = "1.0.0"

        [dependencies]
        vim = "^0.9.0"
        "#,
    )
    .unwrap();
    let mut installed = InstalledModules::new();
    installed.insert(InstalledModule {
        id: "vim".into(),
        version: "bad-version".into(),
        source: ModuleSource::path("/fake"),
        install_path: "/fake".into(),
        library_path: None,
    });

    let result = check_install_constraints(&manifest, &installed);
    assert!(matches!(result, Err(RegistryError::ConstraintViolation(_))));
}

#[test]
fn test_check_install_constraints_invalid_new_version_is_violation() {
    let manifest = ModuleManifest::parse(
        r#"
        [module]
        id = "new-mod"
        name = "New"
        version = "bad-version"
        "#,
    )
    .unwrap();

    let result = check_install_constraints(&manifest, &InstalledModules::new());
    assert!(matches!(result, Err(RegistryError::ConstraintViolation(_))));
}

#[test]
fn test_git_constraint_check_happens_before_permanent_install_dir_exists() {
    let dir = tempfile::tempdir().unwrap();
    let paths = RegistryPaths::new(dir.path().join("registry"));
    std::fs::create_dir_all(&paths.modules_dir).unwrap();

    let vim_dir = dir.path().join("vim");
    create_module_crate(&vim_dir, "vim", "0.8.0", &[]);
    let mut installed = InstalledModules::new();
    installed.insert(InstalledModule {
        id: "vim".into(),
        version: "0.8.0".into(),
        source: ModuleSource::path(vim_dir.to_string_lossy().into_owned()),
        install_path: vim_dir,
        library_path: None,
    });
    save_metadata(&installed, &paths).unwrap();

    let clone_dir = paths.modules_dir.join(".tmp-clone");
    create_module_crate(&clone_dir, "new-mod", "1.0.0", &[("vim", "^0.9.0")]);
    let manifest = load_manifest(&clone_dir).expect("cloned manifest should load");
    let result = check_install_constraints(&manifest, &installed);
    assert!(matches!(result, Err(RegistryError::ConstraintViolation(_))));
    assert!(!paths.modules_dir.join("new-mod").exists());
    let _ = std::fs::remove_dir_all(&clone_dir);
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

// ============================================================================
// remove — git-source where install dir is missing on disk
// ============================================================================

#[test]
fn test_remove_git_source_missing_install_dir() {
    let dir = tempfile::tempdir().unwrap();
    let paths = RegistryPaths::new(dir.path().to_path_buf());

    // Register a git-sourced module but do NOT create its install directory.
    let missing_dir = dir.path().join("ghost-mod");
    let mut installed = InstalledModules::new();
    installed.insert(InstalledModule {
        id: "ghost-mod".to_string(),
        version: "1.0.0".to_string(),
        source: ModuleSource::git("https://example.com/ghost.git"),
        install_path: missing_dir.clone(),
        library_path: None,
    });
    save_metadata(&installed, &paths).unwrap();

    // Remove should succeed — install_path.exists() is false so remove_dir_all is skipped.
    remove("ghost-mod", &paths).unwrap();

    let remaining = list(&paths).unwrap();
    assert!(remaining.is_empty());
    assert!(!missing_dir.exists());
}

// ============================================================================
// check — library_path = None branch
// ============================================================================

#[test]
fn test_check_library_path_none() {
    let dir = tempfile::tempdir().unwrap();
    let paths = RegistryPaths::new(dir.path().to_path_buf());

    let mut installed = InstalledModules::new();
    installed.insert(InstalledModule {
        id: "no-lib".to_string(),
        version: "1.0.0".to_string(),
        source: ModuleSource::path("/tmp/fake"),
        install_path: dir.path().join("no-lib"),
        library_path: None,
    });
    save_metadata(&installed, &paths).unwrap();

    let report = check(&paths).unwrap();
    assert!(!report.is_clean());
    assert_eq!(report.broken.len(), 1);
    assert_eq!(report.broken[0].0, "no-lib");
    assert!(report.broken[0].1.contains("no library path recorded"));
}

#[test]
fn test_check_reports_constraint_violations() {
    let dir = tempfile::tempdir().unwrap();
    let paths = RegistryPaths::new(dir.path().to_path_buf());

    let vim_dir = dir.path().join("vim");
    create_module_crate(&vim_dir, "vim", "0.8.0", &[]);
    let consumer_dir = dir.path().join("consumer");
    create_module_crate(&consumer_dir, "consumer", "1.0.0", &[("vim", "^0.9.0")]);

    let mut installed = InstalledModules::new();
    installed.insert(InstalledModule {
        id: "vim".to_string(),
        version: "0.8.0".to_string(),
        source: ModuleSource::path(vim_dir.to_string_lossy().into_owned()),
        install_path: vim_dir,
        library_path: None,
    });
    installed.insert(InstalledModule {
        id: "consumer".to_string(),
        version: "1.0.0".to_string(),
        source: ModuleSource::path(consumer_dir.to_string_lossy().into_owned()),
        install_path: consumer_dir,
        library_path: None,
    });
    save_metadata(&installed, &paths).unwrap();

    let report = check(&paths).unwrap();
    assert_eq!(report.constraint_violations.len(), 1);
    assert!(report.constraint_violations[0].contains("consumer"));
    // MC/DC: is_clean() where broken=[], orphaned=[], constraint_violations=[1]
    assert!(!report.is_clean());
}

#[test]
fn test_check_reports_invalid_installed_version_as_violation() {
    let dir = tempfile::tempdir().unwrap();
    let paths = RegistryPaths::new(dir.path().to_path_buf());

    let vim_dir = dir.path().join("vim");
    create_module_crate(&vim_dir, "vim", "0.9.0", &[]);

    let mut installed = InstalledModules::new();
    installed.insert(InstalledModule {
        id: "vim".to_string(),
        version: "bad-version".to_string(),
        source: ModuleSource::path(vim_dir.to_string_lossy().into_owned()),
        install_path: vim_dir,
        library_path: None,
    });
    save_metadata(&installed, &paths).unwrap();

    let report = check(&paths).unwrap();
    assert!(
        report
            .constraint_violations
            .iter()
            .any(|msg| msg.contains("invalid version 'bad-version'"))
    );
}

#[test]
fn test_check_reports_invalid_constraint_string_as_violation() {
    let dir = tempfile::tempdir().unwrap();
    let paths = RegistryPaths::new(dir.path().to_path_buf());

    let bad_dir = dir.path().join("bad-mod");
    create_module_crate(&bad_dir, "bad-mod", "1.0.0", &[("vim", "not-a-range")]);

    let mut installed = InstalledModules::new();
    installed.insert(InstalledModule {
        id: "bad-mod".to_string(),
        version: "1.0.0".to_string(),
        source: ModuleSource::path(bad_dir.to_string_lossy().into_owned()),
        install_path: bad_dir,
        library_path: None,
    });
    save_metadata(&installed, &paths).unwrap();

    let report = check(&paths).unwrap();
    assert!(
        report
            .constraint_violations
            .iter()
            .any(|msg| msg.contains("invalid constraint"))
    );
}

// ============================================================================
// check — tracked .so not orphaned
// ============================================================================

#[test]
fn test_check_tracked_so_not_orphaned() {
    let dir = tempfile::tempdir().unwrap();
    let paths = RegistryPaths::new(dir.path().to_path_buf());
    std::fs::create_dir_all(&paths.modules_dir).unwrap();

    // Create a .so file and register it in metadata
    let so_path = paths.modules_dir.join("tracked.so");
    std::fs::write(&so_path, b"fake").unwrap();

    let mut installed = InstalledModules::new();
    installed.insert(InstalledModule {
        id: "tracked-mod".to_string(),
        version: "1.0.0".to_string(),
        source: ModuleSource::path("/tmp/fake"),
        install_path: dir.path().join("tracked-mod"),
        library_path: Some(so_path),
    });
    save_metadata(&installed, &paths).unwrap();

    let report = check(&paths).unwrap();
    // The .so exists and is tracked, so it should appear as valid, not orphaned.
    assert!(report.orphaned.is_empty());
    assert_eq!(report.valid.len(), 1);
    assert_eq!(report.valid[0], "tracked-mod");
    assert!(report.broken.is_empty());
}

// ============================================================================
// info — library_path = None
// ============================================================================

#[test]
fn test_info_library_path_none() {
    let dir = tempfile::tempdir().unwrap();
    let paths = RegistryPaths::new(dir.path().to_path_buf());

    let mod_dir = dir.path().join("no-lib-mod");
    std::fs::create_dir_all(&mod_dir).unwrap();
    std::fs::write(
        mod_dir.join("module.toml"),
        r#"
[module]
id = "no-lib-mod"
name = "No Lib Module"
version = "0.1.0"
"#,
    )
    .unwrap();

    let mut installed = InstalledModules::new();
    installed.insert(InstalledModule {
        id: "no-lib-mod".to_string(),
        version: "0.1.0".to_string(),
        source: ModuleSource::path(mod_dir.to_string_lossy().into_owned()),
        install_path: mod_dir,
        library_path: None,
    });
    save_metadata(&installed, &paths).unwrap();

    let module_info = info("no-lib-mod", &paths).unwrap();
    assert_eq!(module_info.id, "no-lib-mod");
    assert!(!module_info.library_exists);
}

// ============================================================================
// info — library_path = Some(missing)
// ============================================================================

#[test]
fn test_info_library_path_missing() {
    let dir = tempfile::tempdir().unwrap();
    let paths = RegistryPaths::new(dir.path().to_path_buf());

    let mod_dir = dir.path().join("missing-lib-mod");
    std::fs::create_dir_all(&mod_dir).unwrap();
    std::fs::write(
        mod_dir.join("module.toml"),
        r#"
[module]
id = "missing-lib-mod"
name = "Missing Lib Module"
version = "0.2.0"
"#,
    )
    .unwrap();

    let mut installed = InstalledModules::new();
    installed.insert(InstalledModule {
        id: "missing-lib-mod".to_string(),
        version: "0.2.0".to_string(),
        source: ModuleSource::path(mod_dir.to_string_lossy().into_owned()),
        install_path: mod_dir,
        library_path: Some(dir.path().join("does_not_exist.so")),
    });
    save_metadata(&installed, &paths).unwrap();

    let module_info = info("missing-lib-mod", &paths).unwrap();
    assert_eq!(module_info.id, "missing-lib-mod");
    assert!(!module_info.library_exists);
}

// ============================================================================
// info — manifest fallback (no module.toml on disk)
// ============================================================================

#[test]
fn test_info_manifest_fallback() {
    let dir = tempfile::tempdir().unwrap();
    let paths = RegistryPaths::new(dir.path().to_path_buf());

    // Create module dir but WITHOUT a module.toml
    let mod_dir = dir.path().join("no-toml-mod");
    std::fs::create_dir_all(&mod_dir).unwrap();

    let mut installed = InstalledModules::new();
    installed.insert(InstalledModule {
        id: "no-toml-mod".to_string(),
        version: "0.3.0".to_string(),
        source: ModuleSource::path(mod_dir.to_string_lossy().into_owned()),
        install_path: mod_dir,
        library_path: None,
    });
    save_metadata(&installed, &paths).unwrap();

    let module_info = info("no-toml-mod", &paths).unwrap();
    assert_eq!(module_info.id, "no-toml-mod");
    assert_eq!(module_info.version, "0.3.0");
    // No manifest on disk — provides/requires should be empty (unwrap_or_default).
    assert!(module_info.provides.is_empty());
    assert!(module_info.requires.is_empty());
    assert!(!module_info.library_exists);
}

// ============================================================================
// parse_semver_str — 4+ component version returns None (line 567)
// ============================================================================

#[test]
fn test_parse_semver_str_four_parts_returns_none() {
    assert_eq!(parse_semver_str("1.2.3.4"), None);
    assert_eq!(parse_semver_str("1.2.3.4.5"), None);
}

// ============================================================================
// check_install_constraints — reverse check skips non-matching deps (line 522)
// ============================================================================

#[test]
fn test_check_install_constraints_installed_module_with_unrelated_dep_is_skipped() {
    // "existing-mod" depends on "other-mod", not "new-mod".
    // The reverse constraint loop should hit the `continue` at line 522
    // and produce no violation.
    let dir = tempfile::tempdir().unwrap();
    let existing_dir = dir.path().join("existing-mod");
    // depends on "other-mod", NOT on "new-mod"
    create_module_crate(&existing_dir, "existing-mod", "1.0.0", &[("other-mod", "^1.0.0")]);

    let new_manifest = ModuleManifest::parse(
        r#"
        [module]
        id = "new-mod"
        name = "New"
        version = "2.0.0"
        "#,
    )
    .unwrap();

    let mut installed = InstalledModules::new();
    installed.insert(InstalledModule {
        id: "existing-mod".into(),
        version: "1.0.0".into(),
        source: ModuleSource::path(existing_dir.to_string_lossy().into_owned()),
        install_path: existing_dir,
        library_path: None,
    });

    // "existing-mod" has a dep on "other-mod", not "new-mod", so no reverse
    // violation — this should succeed.
    assert!(check_install_constraints(&new_manifest, &installed).is_ok());
}

// ============================================================================
// check_install_constraints — installed module has unreadable manifest (lines 512-517)
// ============================================================================

#[test]
fn test_check_install_constraints_unreadable_installed_manifest() {
    let dir = tempfile::tempdir().unwrap();

    // Create "existing-mod" directory but write a corrupt module.toml so
    // ModuleManifest::load fails during the reverse constraint scan.
    let existing_dir = dir.path().join("existing-mod");
    std::fs::create_dir_all(&existing_dir).unwrap();
    std::fs::write(existing_dir.join("module.toml"), b"not valid toml [[[").unwrap();

    let new_manifest = ModuleManifest::parse(
        r#"
        [module]
        id = "new-mod"
        name = "New"
        version = "1.0.0"
        "#,
    )
    .unwrap();

    let mut installed = InstalledModules::new();
    installed.insert(InstalledModule {
        id: "existing-mod".into(),
        version: "1.0.0".into(),
        source: ModuleSource::path(existing_dir.to_string_lossy().into_owned()),
        install_path: existing_dir,
        library_path: None,
    });

    let result = check_install_constraints(&new_manifest, &installed);
    assert!(matches!(result, Err(RegistryError::ConstraintViolation(_))));
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("unreadable manifest"));
}

// ============================================================================
// check_install_constraints — reverse constraint (MC/DC)
// ============================================================================

#[test]
fn test_check_install_constraints_reverse_constraint_not_satisfied() {
    // "consumer" requires "new-mod ^2.0.0", but we are installing "new-mod" 1.0.0.
    // That is a reverse violation — the already-installed consumer's constraint
    // is violated by the version we are about to install.
    let dir = tempfile::tempdir().unwrap();
    let consumer_dir = dir.path().join("consumer");
    create_module_crate(&consumer_dir, "consumer", "1.0.0", &[("new-mod", "^2.0.0")]);

    let new_manifest = ModuleManifest::parse(
        r#"
        [module]
        id = "new-mod"
        name = "New"
        version = "1.0.0"
        "#,
    )
    .unwrap();

    let mut installed = InstalledModules::new();
    installed.insert(InstalledModule {
        id: "consumer".into(),
        version: "1.0.0".into(),
        source: ModuleSource::path(consumer_dir.to_string_lossy().into_owned()),
        install_path: consumer_dir,
        library_path: None,
    });

    let result = check_install_constraints(&new_manifest, &installed);
    assert!(matches!(result, Err(RegistryError::ConstraintViolation(_))));
}

#[test]
fn test_check_install_constraints_reverse_constraint_satisfied() {
    // "consumer" requires "new-mod ^2.0.0" and we install "new-mod" 2.5.0.
    // Reverse constraint is satisfied — violations list is empty.
    let dir = tempfile::tempdir().unwrap();
    let consumer_dir = dir.path().join("consumer");
    create_module_crate(&consumer_dir, "consumer", "1.0.0", &[("new-mod", "^2.0.0")]);

    let new_manifest = ModuleManifest::parse(
        r#"
        [module]
        id = "new-mod"
        name = "New"
        version = "2.5.0"
        "#,
    )
    .unwrap();

    let mut installed = InstalledModules::new();
    installed.insert(InstalledModule {
        id: "consumer".into(),
        version: "1.0.0".into(),
        source: ModuleSource::path(consumer_dir.to_string_lossy().into_owned()),
        install_path: consumer_dir,
        library_path: None,
    });

    let result = check_install_constraints(&new_manifest, &installed);
    assert!(result.is_ok(), "expected Ok, got {result:?}");
}
