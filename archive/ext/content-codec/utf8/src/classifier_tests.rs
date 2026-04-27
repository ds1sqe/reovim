//! Tests for UTF-8 classifier.

use reovim_content_codec::{ContentClassifier, ContentType};

use super::Utf8Classifier;

#[test]
fn classifies_valid_utf8() {
    let c = Utf8Classifier::new();
    assert_eq!(c.classify(b"hello world", "test.txt"), Some(ContentType::new("text/utf-8")));
}

#[test]
fn classifies_utf8_multibyte() {
    let c = Utf8Classifier::new();
    let korean = "한글 텍스트";
    assert_eq!(c.classify(korean.as_bytes(), "test.txt"), Some(ContentType::new("text/utf-8")));
}

#[test]
fn classifies_empty_as_utf8() {
    let c = Utf8Classifier::new();
    assert_eq!(c.classify(b"", "test.txt"), Some(ContentType::new("text/utf-8")));
}

#[test]
fn rejects_invalid_utf8() {
    let c = Utf8Classifier::new();
    assert_eq!(c.classify(&[0xFF, 0xFE, 0x00], "test.bin"), None);
}

#[test]
fn rejects_truncated_utf8() {
    let c = Utf8Classifier::new();
    // Truncated 2-byte sequence
    assert_eq!(c.classify(&[0xC0], "test.txt"), None);
}

#[test]
fn priority_is_10() {
    let c = Utf8Classifier::new();
    assert_eq!(c.priority(), 10);
}

#[test]
fn name() {
    let c = Utf8Classifier::new();
    assert_eq!(c.name(), "utf-8");
}

#[test]
fn classifies_utf8_with_bom() {
    let c = Utf8Classifier::new();
    let mut bytes = vec![0xEF, 0xBB, 0xBF]; // UTF-8 BOM
    bytes.extend_from_slice(b"hello");
    assert_eq!(c.classify(&bytes, "test.txt"), Some(ContentType::new("text/utf-8")));
}

#[test]
fn classifies_crlf_text() {
    let c = Utf8Classifier::new();
    assert_eq!(
        c.classify(b"line1\r\nline2\r\n", "test.txt"),
        Some(ContentType::new("text/utf-8"))
    );
}
