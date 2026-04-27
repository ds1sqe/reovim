//! Tests for content type.

use super::*;

#[test]
fn new_and_as_str() {
    let ct = ContentType::new("text/utf-8");
    assert_eq!(ct.as_str(), "text/utf-8");
}

#[test]
fn display() {
    let ct = ContentType::new("binary/raw");
    assert_eq!(format!("{ct}"), "binary/raw");
}

#[test]
fn is_text() {
    assert!(ContentType::new("text/utf-8").is_text());
    assert!(ContentType::new("text/euc-kr").is_text());
    assert!(!ContentType::new("binary/raw").is_text());
}

#[test]
fn is_binary() {
    assert!(ContentType::new("binary/raw").is_binary());
    assert!(!ContentType::new("text/utf-8").is_binary());
}

#[test]
fn equality() {
    let a = ContentType::new("text/utf-8");
    let b = ContentType::new("text/utf-8");
    let c = ContentType::new("binary/raw");
    assert_eq!(a, b);
    assert_ne!(a, c);
}

#[test]
fn clone_is_cheap() {
    let a = ContentType::new("text/utf-8");
    let b = a.clone();
    assert_eq!(a, b);
}

#[test]
fn hash_works() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(ContentType::new("text/utf-8"));
    set.insert(ContentType::new("text/utf-8"));
    set.insert(ContentType::new("binary/raw"));
    assert_eq!(set.len(), 2);
}

#[test]
fn well_known_constants() {
    assert_eq!(ContentType::UTF8, "text/utf-8");
    assert_eq!(ContentType::BINARY_RAW, "binary/raw");
}
