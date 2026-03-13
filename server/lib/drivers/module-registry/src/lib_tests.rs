use {
    crate::{
        installed::{InstalledModule, InstalledModules},
        manifest::{ManifestError, ModuleManifest},
        source::ModuleSource,
    },
    std::path::PathBuf,
};

// ============================================================================
// Manifest Tests
// ============================================================================

const FULL_MANIFEST: &str = r#"
[module]
id = "my-module"
name = "My Module"
version = "1.0.0"
description = "A cool module"
authors = ["Alice <alice@example.com>"]

[capabilities]
provides = ["my-cap"]
requires = ["lsp-provider"]

[dependencies]
vim = "^0.9.0"

[build]
crate-name = "my-module-crate"
"#;

#[test]
fn parse_full_manifest() {
    let m = ModuleManifest::parse(FULL_MANIFEST).unwrap();
    assert_eq!(m.id(), "my-module");
    assert_eq!(m.module.name, "My Module");
    assert_eq!(m.version(), "1.0.0");
    assert_eq!(m.module.description, "A cool module");
    assert_eq!(m.module.authors, vec!["Alice <alice@example.com>"]);
    assert_eq!(m.provides(), &["my-cap"]);
    assert_eq!(m.requires(), &["lsp-provider"]);
    assert_eq!(m.dependency_constraints().get("vim").unwrap(), "^0.9.0");
    assert_eq!(m.crate_name(), "my-module-crate");
}

#[test]
fn parse_minimal_manifest() {
    let toml = r#"
[module]
id = "minimal"
name = "Minimal"
version = "0.1.0"
"#;
    let m = ModuleManifest::parse(toml).unwrap();
    assert_eq!(m.id(), "minimal");
    assert!(m.provides().is_empty());
    assert!(m.requires().is_empty());
    assert!(m.dependency_constraints().is_empty());
    // crate_name falls back to module id
    assert_eq!(m.crate_name(), "minimal");
}

#[test]
fn parse_invalid_manifest() {
    let result = ModuleManifest::parse("not valid toml [[[");
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("manifest parse error"));
}

#[test]
fn parse_missing_required_fields() {
    let toml = r#"
[module]
id = "test"
"#;
    // name and version are required by the struct
    let result = ModuleManifest::parse(toml);
    assert!(result.is_err());
}

#[test]
fn manifest_error_display() {
    let parse = ManifestError::Parse("bad toml".to_string());
    assert_eq!(parse.to_string(), "manifest parse error: bad toml");

    let io = ManifestError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "not found"));
    assert!(io.to_string().contains("manifest IO error"));

    let missing = ManifestError::MissingField("id");
    assert_eq!(missing.to_string(), "manifest missing required field: id");
}

#[test]
fn manifest_error_source() {
    let parse = ManifestError::Parse("x".to_string());
    assert!(std::error::Error::source(&parse).is_none());

    let missing = ManifestError::MissingField("id");
    assert!(std::error::Error::source(&missing).is_none());

    let io = ManifestError::Io(std::io::Error::other("test"));
    assert!(std::error::Error::source(&io).is_some());
}

#[test]
fn manifest_load_nonexistent() {
    let result = ModuleManifest::load(std::path::Path::new("/nonexistent/module.toml"));
    assert!(result.is_err());
}

#[test]
fn manifest_load_from_tempfile() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("module.toml");
    std::fs::write(&path, FULL_MANIFEST).unwrap();
    let m = ModuleManifest::load(&path).unwrap();
    assert_eq!(m.id(), "my-module");
}

// ============================================================================
// Source Tests
// ============================================================================

#[test]
fn source_git_no_rev() {
    let s = ModuleSource::git("https://github.com/user/repo");
    assert!(s.is_git());
    assert!(!s.is_path());
    assert_eq!(s.to_string(), "git(https://github.com/user/repo)");
}

#[test]
fn source_git_with_rev() {
    let s = ModuleSource::git_rev("https://github.com/user/repo", "v1.0.0");
    assert!(s.is_git());
    assert_eq!(s.to_string(), "git(https://github.com/user/repo@v1.0.0)");
}

#[test]
fn source_path() {
    let s = ModuleSource::path("/home/user/my-module");
    assert!(s.is_path());
    assert!(!s.is_git());
    assert_eq!(s.to_string(), "path(/home/user/my-module)");
}

#[test]
fn source_serde_roundtrip_git() {
    let s = ModuleSource::git_rev("https://github.com/user/repo", "main");
    let json = serde_json::to_string(&s).unwrap();
    let deserialized: ModuleSource = serde_json::from_str(&json).unwrap();
    assert_eq!(s, deserialized);
}

#[test]
fn source_serde_roundtrip_path() {
    let s = ModuleSource::path("/tmp/my-module");
    let json = serde_json::to_string(&s).unwrap();
    let deserialized: ModuleSource = serde_json::from_str(&json).unwrap();
    assert_eq!(s, deserialized);
}

// ============================================================================
// InstalledModule Tests
// ============================================================================

fn sample_installed() -> InstalledModule {
    InstalledModule {
        id: "test-module".to_string(),
        version: "1.0.0".to_string(),
        source: ModuleSource::git("https://github.com/user/test-module"),
        install_path: PathBuf::from("/home/user/.reovim/modules/test-module"),
        library_path: None,
    }
}

#[test]
fn installed_module_not_built() {
    let m = sample_installed();
    assert!(!m.is_built());
}

#[test]
fn installed_module_built() {
    let mut m = sample_installed();
    m.library_path =
        Some(PathBuf::from("/home/user/.reovim/modules/test-module/libtest_module.so"));
    assert!(m.is_built());
}

#[test]
fn installed_module_display() {
    let m = sample_installed();
    assert_eq!(m.to_string(), "test-module v1.0.0 (git(https://github.com/user/test-module))");
}

// ============================================================================
// InstalledModules Collection Tests
// ============================================================================

#[test]
fn installed_modules_empty() {
    let modules = InstalledModules::new();
    assert!(modules.is_empty());
    assert_eq!(modules.len(), 0);
    assert!(modules.ids().is_empty());
}

#[test]
fn installed_modules_insert_and_get() {
    let mut modules = InstalledModules::new();
    modules.insert(sample_installed());
    assert!(!modules.is_empty());
    assert_eq!(modules.len(), 1);
    assert!(modules.contains("test-module"));
    assert!(!modules.contains("nonexistent"));

    let m = modules.get("test-module").unwrap();
    assert_eq!(m.version, "1.0.0");
}

#[test]
fn installed_modules_remove() {
    let mut modules = InstalledModules::new();
    modules.insert(sample_installed());
    let removed = modules.remove("test-module");
    assert!(removed.is_some());
    assert!(modules.is_empty());

    let nothing = modules.remove("test-module");
    assert!(nothing.is_none());
}

#[test]
fn installed_modules_ids() {
    let mut modules = InstalledModules::new();
    modules.insert(sample_installed());
    let mut m2 = sample_installed();
    m2.id = "other-module".to_string();
    modules.insert(m2);

    let mut ids = modules.ids();
    ids.sort_unstable();
    assert_eq!(ids, vec!["other-module", "test-module"]);
}

#[test]
fn installed_modules_json_roundtrip() {
    let mut modules = InstalledModules::new();
    modules.insert(sample_installed());

    let json = serde_json::to_string_pretty(&modules).unwrap();
    let parsed = InstalledModules::parse(&json).unwrap();
    assert_eq!(modules, parsed);
}

#[test]
fn installed_modules_parse_invalid() {
    let result = InstalledModules::parse("not json");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("parse error"));
}

#[test]
fn installed_modules_save_and_load() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("installed.json");

    let mut modules = InstalledModules::new();
    modules.insert(sample_installed());
    modules.save(&path).unwrap();

    let loaded = InstalledModules::load(&path).unwrap();
    assert_eq!(modules, loaded);
}

#[test]
fn installed_modules_load_nonexistent() {
    let loaded =
        InstalledModules::load(std::path::Path::new("/nonexistent/installed.json")).unwrap();
    assert!(loaded.is_empty());
}
