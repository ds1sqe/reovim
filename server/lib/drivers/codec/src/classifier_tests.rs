//! Tests for content classifier trait.

use super::*;

struct AlwaysClassifier(ContentType);

impl ContentClassifier for AlwaysClassifier {
    fn classify(&self, _raw: &[u8], _path: &str) -> Option<ContentType> {
        Some(self.0.clone())
    }

    fn priority(&self) -> u8 {
        50
    }

    fn name(&self) -> &'static str {
        "always"
    }
}

struct NeverClassifier;

impl ContentClassifier for NeverClassifier {
    fn classify(&self, _raw: &[u8], _path: &str) -> Option<ContentType> {
        None
    }

    fn name(&self) -> &'static str {
        "never"
    }
}

#[test]
fn always_classifier() {
    let c = AlwaysClassifier(ContentType::new("text/utf-8"));
    assert_eq!(c.classify(b"hello", "test.txt"), Some(ContentType::new("text/utf-8")));
}

#[test]
fn never_classifier() {
    let c = NeverClassifier;
    assert_eq!(c.classify(b"hello", "test.txt"), None);
}

#[test]
fn default_priority() {
    let c = NeverClassifier;
    assert_eq!(c.priority(), 50);
}

#[test]
fn custom_priority() {
    let c = AlwaysClassifier(ContentType::new("text/utf-8"));
    assert_eq!(c.priority(), 50);
}

#[test]
fn trait_object_works() {
    let c: Box<dyn ContentClassifier> = Box::new(AlwaysClassifier(ContentType::new("text/utf-8")));
    assert!(c.classify(b"hello", "test.txt").is_some());
    assert_eq!(c.name(), "always");
}
