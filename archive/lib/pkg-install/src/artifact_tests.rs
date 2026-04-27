//! Unit tests for the [`artifact`] module.

use std::fs;

use {reovim_dylib_loader::cdylib_filename, tempfile::tempdir};

use {super::discover, crate::error::InstallError};

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
