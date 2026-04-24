//! Unit tests for the [`artifact`] module.

use std::fs;

use tempfile::tempdir;

use {
    super::{cdylib_filename, discover},
    crate::error::InstallError,
};

#[test]
fn filename_normalizes_hyphens_to_underscores() {
    let name = cdylib_filename("my-cool-theme");
    #[cfg(target_os = "linux")]
    assert_eq!(name, "libreovim_pkg_my_cool_theme.so");
    #[cfg(target_os = "macos")]
    assert_eq!(name, "libreovim_pkg_my_cool_theme.dylib");
    #[cfg(windows)]
    assert_eq!(name, "reovim_pkg_my_cool_theme.dll");
}

#[test]
fn discover_finds_artifact_in_dist() {
    let dir = tempdir().unwrap();
    let dist = dir.path().join("dist");
    fs::create_dir_all(&dist).unwrap();
    let fname = cdylib_filename("alpha");
    fs::write(dist.join(&fname), b"stub-bytes").unwrap();

    let art = discover(dir.path(), "alpha").expect("ok");
    assert_eq!(art.filename, fname);
    assert!(art.abs_path.ends_with(format!("dist/{fname}").as_str()));
}

#[test]
fn discover_reports_missing_dist() {
    let dir = tempdir().unwrap();
    let err = discover(dir.path(), "alpha").expect_err("must fail");
    match err {
        InstallError::MissingDistDir { pkg, at } => {
            assert_eq!(pkg, "alpha");
            assert!(at.ends_with("dist"));
        }
        other => panic!("expected MissingDistDir, got {other:?}"),
    }
}

#[test]
fn discover_reports_missing_artifact() {
    let dir = tempdir().unwrap();
    fs::create_dir_all(dir.path().join("dist")).unwrap();
    let err = discover(dir.path(), "alpha").expect_err("must fail");
    assert!(matches!(err, InstallError::MissingArtifact { ref pkg, .. } if pkg == "alpha"));
}
