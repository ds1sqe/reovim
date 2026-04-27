//! Unit tests for the [`probe`] module.

use std::fs;

use tempfile::tempdir;

use {
    super::probe,
    crate::{artifact::DiscoveredArtifact, error::InstallError},
};

#[test]
fn probe_rejects_text_file_as_cdylib() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("libreovim_pkg_fake.so");
    fs::write(&path, b"not an ELF file").unwrap();
    let art = DiscoveredArtifact {
        abs_path: path,
        filename: "libreovim_pkg_fake.so".into(),
    };
    let err = probe(&art).expect_err("text file must fail");
    assert!(matches!(err, InstallError::NotLoadable { .. }));
}

#[test]
fn probe_rejects_missing_file() {
    let art = DiscoveredArtifact {
        abs_path: std::path::PathBuf::from("/no/such/cdylib.so"),
        filename: "cdylib.so".into(),
    };
    let err = probe(&art).expect_err("missing file must fail");
    assert!(matches!(err, InstallError::NotLoadable { .. }));
}
