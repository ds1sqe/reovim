//! Tests for binary classifier.

use reovim_driver_codec::{ContentClassifier, ContentType};

use super::*;

#[test]
fn null_byte_detected() {
    let c = BinaryClassifier::new();
    let data = b"hello\x00world";
    assert_eq!(c.classify(data, "test.txt"), Some(ContentType::new(ContentType::BINARY_RAW)));
}

#[test]
fn valid_utf8_not_binary() {
    let c = BinaryClassifier::new();
    assert!(c.classify(b"Hello, world!\n", "test.txt").is_none());
}

#[test]
fn empty_file_not_binary() {
    let c = BinaryClassifier::new();
    assert!(c.classify(b"", "test.txt").is_none());
}

#[test]
fn high_non_printable_ratio() {
    let c = BinaryClassifier::new();
    // 50% non-printable (control chars 0x01-0x08)
    let data: Vec<u8> = (0..100)
        .map(|i| if i % 2 == 0 { 0x01 } else { b'a' })
        .collect();
    assert_eq!(c.classify(&data, "test.dat"), Some(ContentType::new(ContentType::BINARY_RAW)));
}

#[test]
fn low_non_printable_ratio_not_binary() {
    let c = BinaryClassifier::new();
    // 5% non-printable (well below 30% threshold)
    let mut data = vec![b'a'; 95];
    data.extend_from_slice(&[0x01; 5]);
    assert!(c.classify(&data, "test.txt").is_none());
}

#[test]
fn known_binary_extension_exe() {
    let c = BinaryClassifier::new();
    // Even with valid UTF-8 content, .exe is binary
    assert_eq!(
        c.classify(b"valid text", "app.exe"),
        Some(ContentType::new(ContentType::BINARY_RAW))
    );
}

#[test]
fn known_binary_extension_png() {
    let c = BinaryClassifier::new();
    assert_eq!(
        c.classify(b"text", "image.png"),
        Some(ContentType::new(ContentType::BINARY_RAW))
    );
}

#[test]
fn known_binary_extension_zip() {
    let c = BinaryClassifier::new();
    assert_eq!(
        c.classify(b"text", "archive.zip"),
        Some(ContentType::new(ContentType::BINARY_RAW))
    );
}

#[test]
fn known_binary_extension_case_insensitive() {
    let c = BinaryClassifier::new();
    assert_eq!(
        c.classify(b"text", "image.PNG"),
        Some(ContentType::new(ContentType::BINARY_RAW))
    );
    assert_eq!(c.classify(b"text", "app.EXE"), Some(ContentType::new(ContentType::BINARY_RAW)));
}

#[test]
fn unknown_extension_not_fast_path() {
    let c = BinaryClassifier::new();
    assert!(c.classify(b"hello world", "test.rs").is_none());
}

#[test]
fn no_extension_not_fast_path() {
    let c = BinaryClassifier::new();
    assert!(c.classify(b"hello world", "Makefile").is_none());
}

#[test]
fn priority_is_20() {
    let c = BinaryClassifier::new();
    assert_eq!(c.priority(), 20);
}

#[test]
fn name_is_binary() {
    let c = BinaryClassifier::new();
    assert_eq!(c.name(), "binary");
}

#[test]
fn large_sample_truncated() {
    let c = BinaryClassifier::new();
    // Create 16KB of valid text — should sample only first 8KB
    let data = vec![b'a'; 16384];
    assert!(c.classify(&data, "test.txt").is_none());
}

#[test]
fn null_byte_beyond_sample_not_detected() {
    let c = BinaryClassifier::new();
    // Null byte at position 9000 (beyond 8192 sample)
    let mut data = vec![b'a'; 9000];
    data[8999] = 0x00;
    // Still detected because sample covers 8192 bytes — no null in sample
    assert!(c.classify(&data, "test.txt").is_none());
}

#[test]
fn utf8_multibyte_not_binary() {
    let c = BinaryClassifier::new();
    // Korean and Japanese text with high bytes (0x80+)
    let text = "한글 日本語 中文";
    assert!(c.classify(text.as_bytes(), "test.txt").is_none());
}

#[test]
fn tabs_and_newlines_are_printable() {
    let c = BinaryClassifier::new();
    let data = b"hello\tworld\n\rnext line\n";
    assert!(c.classify(data, "test.txt").is_none());
}

#[test]
fn pdf_extension() {
    let c = BinaryClassifier::new();
    assert_eq!(c.classify(b"text", "doc.pdf"), Some(ContentType::new(ContentType::BINARY_RAW)));
}

#[test]
fn wasm_extension() {
    let c = BinaryClassifier::new();
    assert_eq!(c.classify(b"text", "app.wasm"), Some(ContentType::new(ContentType::BINARY_RAW)));
}

#[test]
fn default_impl() {
    let c = BinaryClassifier;
    assert_eq!(c.name(), "binary");
}

#[test]
fn is_printable_boundary_values() {
    // 0x08 (backspace) is NOT printable
    assert!(!is_printable(0x08));
    // 0x09 (tab) IS printable
    assert!(is_printable(0x09));
    // 0x0A (LF) IS printable
    assert!(is_printable(0x0A));
    // 0x0B (VT) is NOT printable
    assert!(!is_printable(0x0B));
    // 0x0D (CR) IS printable
    assert!(is_printable(0x0D));
    // 0x1F (unit separator) is NOT printable
    assert!(!is_printable(0x1F));
    // 0x20 (space) IS printable
    assert!(is_printable(0x20));
    // 0x7E (~) IS printable
    assert!(is_printable(0x7E));
    // 0x7F (DEL) is NOT printable
    assert!(!is_printable(0x7F));
    // 0x80 IS printable (UTF-8 continuation byte)
    assert!(is_printable(0x80));
    // 0xFF IS printable (high byte, could be UTF-8)
    assert!(is_printable(0xFF));
}

#[test]
fn has_binary_extension_tests() {
    assert!(has_binary_extension("test.exe"));
    assert!(has_binary_extension("test.dll"));
    assert!(has_binary_extension("test.so"));
    assert!(has_binary_extension("/path/to/lib.dylib"));
    assert!(!has_binary_extension("test.rs"));
    assert!(!has_binary_extension("Makefile"));
    assert!(!has_binary_extension("test.txt"));
}
