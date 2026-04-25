//! Unit tests for the [`loader`] module.

use std::fs;

use tempfile::tempdir;

use {super::load, crate::graph::ResolveError};

const MINIMAL_MANIFEST: &str = r#"
[package]
name = "probe"
reovim-version = "^0.15"
"#;

#[test]
fn load_parses_valid_manifest() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("pkg.toml"), MINIMAL_MANIFEST).unwrap();
    let loaded = load(dir.path()).expect("load ok");
    assert_eq!(loaded.manifest.package.name, "probe");
    assert_eq!(loaded.canonical_path, dir.path().canonicalize().unwrap());
    assert_eq!(loaded.manifest_path, loaded.canonical_path.join("pkg.toml"),);
}

#[test]
fn load_reports_missing_directory() {
    let missing = std::path::PathBuf::from("/no/such/dir/sure-not-here");
    let err = load(&missing).expect_err("missing dir must fail");
    match err {
        ResolveError::ManifestNotFound { at } => assert_eq!(at, missing),
        other => panic!("expected ManifestNotFound, got {other:?}"),
    }
}

#[test]
fn load_reports_missing_pkg_toml() {
    let dir = tempdir().unwrap();
    let err = load(dir.path()).expect_err("empty dir must fail");
    match err {
        ResolveError::ManifestNotFound { at } => {
            assert_eq!(at, dir.path().canonicalize().unwrap().join("pkg.toml"));
        }
        other => panic!("expected ManifestNotFound, got {other:?}"),
    }
}

#[test]
fn load_reports_non_utf8_bytes() {
    let dir = tempdir().unwrap();
    // raw non-utf8 bytes — is_file() is true, read_to_string fails
    fs::write(dir.path().join("pkg.toml"), [0xFFu8, 0xFE, 0xFD]).unwrap();
    let err = load(dir.path()).expect_err("non-utf8 must fail");
    match err {
        ResolveError::ManifestNotFound { at } => {
            assert!(at.ends_with("pkg.toml"));
        }
        other => panic!("expected ManifestNotFound, got {other:?}"),
    }
}

#[test]
fn load_reports_malformed_toml() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("pkg.toml"), "not valid = = toml").unwrap();
    let err = load(dir.path()).expect_err("bad toml must fail");
    match err {
        ResolveError::ManifestParse { at, .. } => {
            assert_eq!(at, dir.path().canonicalize().unwrap().join("pkg.toml"));
        }
        other => panic!("expected ManifestParse, got {other:?}"),
    }
}
