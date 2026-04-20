//! Tests for legacy encoding classifier.

use reovim_content_codec::ContentClassifier;

use super::*;

#[test]
fn empty_input() {
    let c = LegacyClassifier::new();
    assert!(c.classify(b"", "test.txt").is_none());
}

#[test]
fn pure_ascii_not_legacy() {
    let c = LegacyClassifier::new();
    assert!(c.classify(b"Hello, world!", "test.txt").is_none());
}

#[test]
fn valid_utf8_not_legacy() {
    let c = LegacyClassifier::new();
    // UTF-8 accented characters
    let text = "café résumé";
    assert!(c.classify(text.as_bytes(), "test.txt").is_none());
}

#[test]
fn binary_content_not_legacy() {
    let c = LegacyClassifier::new();
    let data = b"hello\x00world";
    assert!(c.classify(data, "test.bin").is_none());
}

#[test]
fn latin1_detected() {
    let c = LegacyClassifier::new();
    // "café" in Latin-1: c a f é = 63 61 66 E9
    // 0xE9 is é in Latin-1, but NOT a valid UTF-8 start byte by itself
    let latin1_bytes: &[u8] = &[0x63, 0x61, 0x66, 0xE9];
    let result = c.classify(latin1_bytes, "test.txt");
    assert!(result.is_some());
    assert_eq!(result.unwrap().as_str(), LATIN_1);
}

#[test]
fn windows_1252_detected() {
    let c = LegacyClassifier::new();
    // Text with em-dash (0x97) and smart quotes (0x93, 0x94)
    // These are Windows-1252 specific (0x80..0x9F range)
    let cp1252_bytes: &[u8] = &[0x93, b'h', b'e', b'l', b'l', b'o', 0x94, 0x97];
    let result = c.classify(cp1252_bytes, "test.txt");
    assert!(result.is_some());
    assert_eq!(result.unwrap().as_str(), WINDOWS_1252);
}

#[test]
fn priority_is_40() {
    let c = LegacyClassifier::new();
    assert_eq!(c.priority(), 40);
}

#[test]
fn name_is_legacy() {
    let c = LegacyClassifier::new();
    assert_eq!(c.name(), "legacy");
}

#[test]
fn default_impl() {
    let c = LegacyClassifier;
    assert_eq!(c.name(), "legacy");
}

#[test]
fn no_high_bytes_rejected() {
    let c = LegacyClassifier::new();
    assert!(c.classify(b"plain text only", "test.txt").is_none());
}

#[test]
fn euro_sign_is_cp1252() {
    let c = LegacyClassifier::new();
    // 0x80 is Euro sign in Windows-1252
    let data: &[u8] = &[b'p', b'r', b'i', b'c', b'e', b' ', 0x80, b'5'];
    let result = c.classify(data, "test.txt");
    assert!(result.is_some());
    assert_eq!(result.unwrap().as_str(), WINDOWS_1252);
}

#[test]
fn high_latin1_without_cp1252_specific() {
    let c = LegacyClassifier::new();
    // Bytes in 0xA0..0xFF range (valid in both Latin-1 and CP1252)
    // No bytes in 0x80..0x9F range, so classified as Latin-1
    let data: &[u8] = &[0xA0, 0xBF, 0xC0, 0xDF, 0xE0, 0xFF];
    let result = c.classify(data, "test.txt");
    assert!(result.is_some());
    assert_eq!(result.unwrap().as_str(), LATIN_1);
}

#[test]
fn large_sample_truncated() {
    let c = LegacyClassifier::new();
    // Create data larger than SAMPLE_SIZE with valid UTF-8
    let data = "é".repeat(5000);
    assert!(c.classify(data.as_bytes(), "test.txt").is_none());
}
