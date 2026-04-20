//! Tests for codec metadata.

use super::*;

fn utf8_metadata() -> CodecMetadata {
    CodecMetadata::new(ContentType::new("text/utf-8"))
}

#[test]
fn new_is_empty() {
    let m = utf8_metadata();
    assert!(m.is_empty());
    assert_eq!(m.len(), 0);
    assert_eq!(m.content_type().as_str(), "text/utf-8");
}

#[test]
fn set_and_get() {
    let mut m = utf8_metadata();
    m.set("bom", "true");
    assert_eq!(m.get("bom"), Some("true"));
    assert_eq!(m.len(), 1);
    assert!(!m.is_empty());
}

#[test]
fn contains_key() {
    let mut m = utf8_metadata();
    assert!(!m.contains_key("bom"));
    m.set("bom", "true");
    assert!(m.contains_key("bom"));
}

#[test]
fn remove() {
    let mut m = utf8_metadata();
    m.set("bom", "true");
    assert_eq!(m.remove("bom"), Some("true".to_string()));
    assert!(m.is_empty());
    assert_eq!(m.remove("bom"), None);
}

#[test]
fn get_missing_key() {
    let m = utf8_metadata();
    assert_eq!(m.get("nonexistent"), None);
}

#[test]
fn data_returns_map() {
    let mut m = utf8_metadata();
    m.set("a", "1");
    m.set("b", "2");
    let data = m.data();
    assert_eq!(data.len(), 2);
    assert_eq!(data.get("a").map(String::as_str), Some("1"));
}

#[test]
fn equality() {
    let mut a = CodecMetadata::new(ContentType::new("text/utf-8"));
    let mut b = CodecMetadata::new(ContentType::new("text/utf-8"));
    assert_eq!(a, b);

    a.set("bom", "true");
    assert_ne!(a, b);

    b.set("bom", "true");
    assert_eq!(a, b);
}

#[test]
fn different_content_type_not_equal() {
    let a = CodecMetadata::new(ContentType::new("text/utf-8"));
    let b = CodecMetadata::new(ContentType::new("binary/raw"));
    assert_ne!(a, b);
}

#[test]
fn clone() {
    let mut m = utf8_metadata();
    m.set("key", "value");
    let cloned = m.clone();
    assert_eq!(m, cloned);
}

#[test]
fn overwrite_value() {
    let mut m = utf8_metadata();
    m.set("key", "old");
    m.set("key", "new");
    assert_eq!(m.get("key"), Some("new"));
    assert_eq!(m.len(), 1);
}
