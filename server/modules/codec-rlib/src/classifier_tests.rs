//! Tests for rlib classifier.

use reovim_driver_codec::ContentClassifier;

use super::*;

#[test]
fn rlib_extension_detected() {
    let c = RlibClassifier::new();
    let result = c.classify(b"!<arch>\n", "target/debug/deps/libfoo-abc123.rlib");
    assert_eq!(result.unwrap().as_str(), RLIB);
}

#[test]
fn rlib_extension_without_ar_magic() {
    let c = RlibClassifier::new();
    // Extension alone is enough (file may be truncated)
    let result = c.classify(b"something", "foo.rlib");
    assert_eq!(result.unwrap().as_str(), RLIB);
}

#[test]
fn ar_magic_without_rlib_extension_rejected() {
    let c = RlibClassifier::new();
    assert!(c.classify(b"!<arch>\n", "libfoo.a").is_none());
}

#[test]
fn text_content_rejected() {
    let c = RlibClassifier::new();
    assert!(c.classify(b"hello world", "test.txt").is_none());
}

#[test]
fn empty_bytes_rejected() {
    let c = RlibClassifier::new();
    assert!(c.classify(b"", "test.rs").is_none());
}

#[test]
fn too_short_bytes_with_rlib_ext() {
    let c = RlibClassifier::new();
    // Short bytes but .rlib extension — still classified
    let result = c.classify(b"!", "foo.rlib");
    assert_eq!(result.unwrap().as_str(), RLIB);
}

#[test]
fn priority_is_32() {
    let c = RlibClassifier::new();
    assert_eq!(c.priority(), 32);
}

#[test]
fn name_is_rlib() {
    let c = RlibClassifier::new();
    assert_eq!(c.name(), "rlib");
}

#[test]
fn case_insensitive_extension() {
    let c = RlibClassifier::new();
    let result = c.classify(b"!<arch>\n", "foo.RLIB");
    assert_eq!(result.unwrap().as_str(), RLIB);
}

#[test]
fn no_extension_rejected() {
    let c = RlibClassifier::new();
    assert!(c.classify(b"!<arch>\n", "Makefile").is_none());
}

#[test]
fn default_impl() {
    let c = RlibClassifier;
    assert_eq!(c.name(), "rlib");
}
