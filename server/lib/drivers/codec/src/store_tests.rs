//! Tests for codec stores.

use std::sync::Arc;

use {
    super::*,
    crate::{CodecError, CodecMetadata, DecodeResult},
};

// --- Mock types ---

struct MockCodec;

impl ContentCodec for MockCodec {
    fn decode(&self, raw: &[u8]) -> Result<DecodeResult, CodecError> {
        Ok(DecodeResult {
            content: String::from_utf8_lossy(raw).into_owned(),
            annotations: vec![],
            metadata: CodecMetadata::new(ContentType::new("text/utf-8")),
            lossy: false,
            readonly: false,
            truncated: false,
        })
    }
}

struct MockFactory {
    content_type: &'static str,
}

impl ContentCodecFactory for MockFactory {
    fn create(&self, content_type: &ContentType) -> Option<Arc<dyn ContentCodec>> {
        if content_type.as_str() == self.content_type {
            Some(Arc::new(MockCodec))
        } else {
            None
        }
    }

    fn supported_content_types(&self) -> Vec<&str> {
        vec![self.content_type]
    }

    fn name(&self) -> &'static str {
        self.content_type
    }
}

struct MockClassifier {
    result: Option<ContentType>,
    prio: u8,
    classifier_name: &'static str,
}

impl ContentClassifier for MockClassifier {
    fn classify(&self, _raw: &[u8], _path: &str) -> Option<ContentType> {
        self.result.clone()
    }

    fn priority(&self) -> u8 {
        self.prio
    }

    fn name(&self) -> &'static str {
        self.classifier_name
    }
}

// --- ContentCodecFactoryStore tests ---

#[test]
fn factory_store_new_is_empty() {
    let store = ContentCodecFactoryStore::new();
    assert!(store.is_empty());
    assert_eq!(store.len(), 0);
}

#[test]
fn factory_store_add_and_find() {
    let store = ContentCodecFactoryStore::new();
    store.add_factory(Arc::new(MockFactory {
        content_type: "text/utf-8",
    }));
    assert_eq!(store.len(), 1);
    assert!(!store.is_empty());

    let ct = ContentType::new("text/utf-8");
    assert!(store.find(&ct).is_some());

    let ct2 = ContentType::new("binary/raw");
    assert!(store.find(&ct2).is_none());
}

#[test]
fn factory_store_find_first_match() {
    let store = ContentCodecFactoryStore::new();
    store.add_factory(Arc::new(MockFactory {
        content_type: "text/utf-8",
    }));
    store.add_factory(Arc::new(MockFactory {
        content_type: "binary/raw",
    }));
    assert_eq!(store.len(), 2);

    let ct = ContentType::new("binary/raw");
    assert!(store.find(&ct).is_some());
}

#[test]
fn factory_store_take_drains() {
    let store = ContentCodecFactoryStore::new();
    store.add_factory(Arc::new(MockFactory {
        content_type: "text/utf-8",
    }));
    let taken = store.take_factories();
    assert_eq!(taken.len(), 1);
    assert!(store.is_empty());
}

#[test]
fn factory_store_debug() {
    let store = ContentCodecFactoryStore::new();
    let debug = format!("{store:?}");
    assert!(debug.contains("ContentCodecFactoryStore"));
    assert!(debug.contains("count"));
}

// --- ContentClassifierStore tests ---

#[test]
fn classifier_store_new_is_empty() {
    let store = ContentClassifierStore::new();
    assert!(store.is_empty());
    assert_eq!(store.len(), 0);
}

#[test]
fn classifier_store_add() {
    let store = ContentClassifierStore::new();
    store.add(Arc::new(MockClassifier {
        result: Some(ContentType::new("text/utf-8")),
        prio: 10,
        classifier_name: "utf8",
    }));
    assert_eq!(store.len(), 1);
    assert!(!store.is_empty());
}

#[test]
fn classifier_store_classify_returns_first_match() {
    let store = ContentClassifierStore::new();
    store.add(Arc::new(MockClassifier {
        result: Some(ContentType::new("text/utf-8")),
        prio: 10,
        classifier_name: "utf8",
    }));
    store.add(Arc::new(MockClassifier {
        result: Some(ContentType::new("binary/raw")),
        prio: 20,
        classifier_name: "binary",
    }));

    // Higher priority (20) should win
    let result = store.classify(b"hello", "test.txt");
    assert_eq!(result, Some(ContentType::new("binary/raw")));
}

#[test]
fn classifier_store_classify_skips_none() {
    let store = ContentClassifierStore::new();
    store.add(Arc::new(MockClassifier {
        result: None,
        prio: 50,
        classifier_name: "skip",
    }));
    store.add(Arc::new(MockClassifier {
        result: Some(ContentType::new("text/utf-8")),
        prio: 10,
        classifier_name: "utf8",
    }));

    let result = store.classify(b"hello", "test.txt");
    assert_eq!(result, Some(ContentType::new("text/utf-8")));
}

#[test]
fn classifier_store_classify_returns_none_when_no_match() {
    let store = ContentClassifierStore::new();
    store.add(Arc::new(MockClassifier {
        result: None,
        prio: 50,
        classifier_name: "skip",
    }));

    assert_eq!(store.classify(b"hello", "test.txt"), None);
}

#[test]
fn classifier_store_classify_empty_store() {
    let store = ContentClassifierStore::new();
    assert_eq!(store.classify(b"hello", "test.txt"), None);
}

#[test]
fn classifier_store_priority_ordering() {
    let store = ContentClassifierStore::new();
    // Add in reverse priority order to test sorting
    store.add(Arc::new(MockClassifier {
        result: Some(ContentType::new("text/utf-8")),
        prio: 10,
        classifier_name: "utf8",
    }));
    store.add(Arc::new(MockClassifier {
        result: Some(ContentType::new("text/euc-kr")),
        prio: 50,
        classifier_name: "cjk",
    }));
    store.add(Arc::new(MockClassifier {
        result: Some(ContentType::new("binary/raw")),
        prio: 20,
        classifier_name: "binary",
    }));

    // Priority 50 should win
    let result = store.classify(b"hello", "test.txt");
    assert_eq!(result, Some(ContentType::new("text/euc-kr")));
}

#[test]
fn classifier_store_take_drains() {
    let store = ContentClassifierStore::new();
    store.add(Arc::new(MockClassifier {
        result: None,
        prio: 10,
        classifier_name: "test",
    }));
    let taken = store.take_classifiers();
    assert_eq!(taken.len(), 1);
    assert!(store.is_empty());
}

#[test]
fn classifier_store_debug() {
    let store = ContentClassifierStore::new();
    let debug = format!("{store:?}");
    assert!(debug.contains("ContentClassifierStore"));
    assert!(debug.contains("count"));
}
