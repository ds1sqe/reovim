//! Unit tests for the [`hash`] module.

use std::fs;

use tempfile::tempdir;

use {super::digest, crate::error::InstallError};

#[test]
fn digest_of_empty_file_is_known_vector() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("empty");
    fs::write(&path, b"").unwrap();
    assert_eq!(
        digest(&path).unwrap(),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    );
}

#[test]
fn digest_of_single_byte_is_known_vector() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("a");
    fs::write(&path, b"a").unwrap();
    assert_eq!(
        digest(&path).unwrap(),
        "ca978112ca1bbdcafac231b39a23dc4da786eff8147c4e72b9807785afee48bb",
    );
}

#[test]
fn digest_of_large_file_streams() {
    // 200 KiB exercises >3 chunks of the 64 KiB streaming buffer.
    let dir = tempdir().unwrap();
    let path = dir.path().join("big");
    let bytes = vec![0x42u8; 200 * 1024];
    fs::write(&path, &bytes).unwrap();
    let hex = digest(&path).unwrap();
    assert_eq!(hex.len(), 64);
    assert!(
        hex.chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_uppercase())
    );
}

#[test]
fn digest_missing_file_errors() {
    let err = digest(std::path::Path::new("/definitely/no/such/file")).expect_err("must fail");
    assert!(matches!(err, InstallError::ArtifactReadFailed { .. }));
}
