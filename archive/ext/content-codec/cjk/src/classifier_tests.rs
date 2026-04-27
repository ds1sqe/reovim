//! Tests for CJK encoding classifier.

use reovim_content_codec::ContentClassifier;

use super::*;

#[test]
fn empty_input() {
    let c = CjkClassifier::new();
    assert!(c.classify(b"", "test.txt").is_none());
}

#[test]
fn pure_ascii_not_cjk() {
    let c = CjkClassifier::new();
    assert!(c.classify(b"Hello, world!", "test.txt").is_none());
}

#[test]
fn valid_utf8_cjk_not_classified() {
    let c = CjkClassifier::new();
    // UTF-8 encoded Korean text — should be handled by UTF-8 classifier, not CJK
    let text = "한글 테스트";
    assert!(c.classify(text.as_bytes(), "test.txt").is_none());
}

#[test]
fn binary_content_not_cjk() {
    let c = CjkClassifier::new();
    let data = b"hello\x00world";
    assert!(c.classify(data, "test.bin").is_none());
}

#[test]
fn euc_kr_detected() {
    let c = CjkClassifier::new();
    // "한글" in EUC-KR encoding
    let euc_kr_bytes: &[u8] = &[0xC7, 0xD1, 0xB1, 0xDB];
    let result = c.classify(euc_kr_bytes, "test.txt");
    assert!(result.is_some());
    // Should detect as one of the CJK encodings
    let ct = result.unwrap();
    assert!(ct.as_str().starts_with("encoding/"));
}

#[test]
fn shift_jis_detected() {
    let c = CjkClassifier::new();
    // "日本語" in Shift-JIS encoding
    let shift_jis_bytes: &[u8] = &[0x93, 0xFA, 0x96, 0x7B, 0x8C, 0xEA];
    let result = c.classify(shift_jis_bytes, "test.txt");
    assert!(result.is_some());
    let ct = result.unwrap();
    assert!(ct.as_str().starts_with("encoding/"));
}

#[test]
fn priority_is_50() {
    let c = CjkClassifier::new();
    assert_eq!(c.priority(), 50);
}

#[test]
fn name_is_cjk() {
    let c = CjkClassifier::new();
    assert_eq!(c.name(), "cjk");
}

#[test]
fn default_impl() {
    let c = CjkClassifier;
    assert_eq!(c.name(), "cjk");
}

#[test]
fn no_high_bytes_rejected() {
    let c = CjkClassifier::new();
    // All ASCII — no high bytes to trigger CJK detection
    assert!(c.classify(b"abcdefghijklmnop", "test.txt").is_none());
}

#[test]
fn large_sample_truncated() {
    let c = CjkClassifier::new();
    // Create data larger than SAMPLE_SIZE that is valid UTF-8 with high bytes
    let data = "あ".repeat(5000); // UTF-8 Japanese hiragana
    assert!(c.classify(data.as_bytes(), "test.txt").is_none());
}
