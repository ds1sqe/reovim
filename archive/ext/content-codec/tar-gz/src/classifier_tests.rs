//! Tests for tar.gz classifier.

use reovim_content_codec::ContentClassifier;

use super::*;

const GZIP_HEADER: &[u8] = &[0x1f, 0x8b, 0x08, 0x00];

#[test]
fn tar_gz_extension_with_gzip_magic_detected() {
    let c = TarGzClassifier::new();
    let result = c.classify(GZIP_HEADER, "archive.tar.gz");
    assert_eq!(result.unwrap().as_str(), TAR_GZ);
}

#[test]
fn tgz_extension_with_gzip_magic_detected() {
    let c = TarGzClassifier::new();
    let result = c.classify(GZIP_HEADER, "archive.tgz");
    assert_eq!(result.unwrap().as_str(), TAR_GZ);
}

#[test]
fn tar_gz_without_gzip_magic_rejected() {
    let c = TarGzClassifier::new();
    let result = c.classify(b"not gzip data", "archive.tar.gz");
    assert!(result.is_none());
}

#[test]
fn tar_extension_without_gzip_rejected() {
    let c = TarGzClassifier::new();
    // .tar is not .tar.gz
    let result = c.classify(GZIP_HEADER, "archive.tar");
    assert!(result.is_none());
}

#[test]
fn non_matching_extension_rejected() {
    let c = TarGzClassifier::new();
    let result = c.classify(GZIP_HEADER, "archive.zip");
    assert!(result.is_none());
}

#[test]
fn no_extension_rejected() {
    let c = TarGzClassifier::new();
    assert!(c.classify(GZIP_HEADER, "Makefile").is_none());
}

#[test]
fn empty_bytes_rejected() {
    let c = TarGzClassifier::new();
    assert!(c.classify(b"", "archive.tar.gz").is_none());
}

#[test]
fn single_byte_rejected() {
    let c = TarGzClassifier::new();
    assert!(c.classify(&[0x1f], "archive.tar.gz").is_none());
}

#[test]
fn priority_is_30() {
    let c = TarGzClassifier::new();
    assert_eq!(c.priority(), 30);
}

#[test]
fn name_is_tar_gz() {
    let c = TarGzClassifier::new();
    assert_eq!(c.name(), "tar-gz");
}

#[test]
fn case_insensitive_extension() {
    let c = TarGzClassifier::new();
    let result = c.classify(GZIP_HEADER, "ARCHIVE.TAR.GZ");
    assert_eq!(result.unwrap().as_str(), TAR_GZ);
}

#[test]
fn default_impl() {
    let c = TarGzClassifier;
    assert_eq!(c.name(), "tar-gz");
}
