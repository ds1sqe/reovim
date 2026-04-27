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

// --- ContentCodecFactoryStore priority tests (#740 Phase 6) ---

/// Mock codec that records the name of the factory that built it so
/// tests can assert which factory won priority dispatch.
struct MockNamedCodec {
    source: &'static str,
}

impl ContentCodec for MockNamedCodec {
    fn decode(&self, _raw: &[u8]) -> Result<DecodeResult, CodecError> {
        Ok(DecodeResult {
            content: self.source.to_string(),
            annotations: vec![],
            metadata: CodecMetadata::new(ContentType::new("text/utf-8")),
            lossy: false,
            readonly: false,
            truncated: false,
        })
    }
}

struct MockNamedFactory {
    content_type: &'static str,
    name: &'static str,
}

impl ContentCodecFactory for MockNamedFactory {
    fn create(&self, content_type: &ContentType) -> Option<Arc<dyn ContentCodec>> {
        if content_type.as_str() == self.content_type {
            Some(Arc::new(MockNamedCodec { source: self.name }))
        } else {
            None
        }
    }

    fn supported_content_types(&self) -> Vec<&str> {
        vec![self.content_type]
    }

    fn name(&self) -> &'static str {
        self.name
    }
}

#[test]
fn factory_store_higher_priority_wins() {
    let store = ContentCodecFactoryStore::new();
    // Register low priority first, then high priority.
    store.add_factory_with_priority(
        Arc::new(MockNamedFactory {
            content_type: "text/utf-8",
            name: "low",
        }),
        10,
    );
    store.add_factory_with_priority(
        Arc::new(MockNamedFactory {
            content_type: "text/utf-8",
            name: "high",
        }),
        200,
    );

    let codec = store
        .find(&ContentType::new("text/utf-8"))
        .expect("high-priority codec found");
    let result = codec.decode(b"ignored").unwrap();
    assert_eq!(result.content, "high");
}

#[test]
fn factory_store_equal_priority_first_registered_wins() {
    let store = ContentCodecFactoryStore::new();
    store.add_factory_with_priority(
        Arc::new(MockNamedFactory {
            content_type: "text/utf-8",
            name: "first",
        }),
        100,
    );
    store.add_factory_with_priority(
        Arc::new(MockNamedFactory {
            content_type: "text/utf-8",
            name: "second",
        }),
        100,
    );

    let codec = store.find(&ContentType::new("text/utf-8")).unwrap();
    let result = codec.decode(b"ignored").unwrap();
    assert_eq!(result.content, "first");
}

#[test]
fn factory_store_add_factory_uses_default_priority() {
    let store = ContentCodecFactoryStore::new();
    // Default-priority factory first.
    store.add_factory(Arc::new(MockNamedFactory {
        content_type: "text/utf-8",
        name: "default",
    }));
    // Explicit high-priority factory second — still wins.
    store.add_factory_with_priority(
        Arc::new(MockNamedFactory {
            content_type: "text/utf-8",
            name: "override",
        }),
        DEFAULT_FACTORY_PRIORITY + 1,
    );

    let codec = store.find(&ContentType::new("text/utf-8")).unwrap();
    let result = codec.decode(b"ignored").unwrap();
    assert_eq!(result.content, "override");
}

#[test]
fn factory_store_low_priority_used_as_fallback() {
    let store = ContentCodecFactoryStore::new();
    store.add_factory_with_priority(
        Arc::new(MockNamedFactory {
            content_type: "text/utf-8",
            name: "fallback",
        }),
        1,
    );
    store.add_factory_with_priority(
        Arc::new(MockNamedFactory {
            content_type: "application/octet-stream",
            name: "primary",
        }),
        1000,
    );

    // Primary does not claim text/utf-8 → fallback wins.
    let codec = store.find(&ContentType::new("text/utf-8")).unwrap();
    let result = codec.decode(b"ignored").unwrap();
    assert_eq!(result.content, "fallback");
}

#[test]
fn factory_store_take_preserves_priority_order() {
    let store = ContentCodecFactoryStore::new();
    store.add_factory_with_priority(
        Arc::new(MockNamedFactory {
            content_type: "a",
            name: "low",
        }),
        10,
    );
    store.add_factory_with_priority(
        Arc::new(MockNamedFactory {
            content_type: "b",
            name: "high",
        }),
        1000,
    );
    store.add_factory_with_priority(
        Arc::new(MockNamedFactory {
            content_type: "c",
            name: "mid",
        }),
        500,
    );

    let taken = store.take_factories();
    assert_eq!(taken.len(), 3);
    assert_eq!(taken[0].name(), "high");
    assert_eq!(taken[1].name(), "mid");
    assert_eq!(taken[2].name(), "low");
}

#[test]
fn factory_store_available_lists_in_priority_order() {
    let store = ContentCodecFactoryStore::new();
    store.add_factory_with_priority(
        Arc::new(MockNamedFactory {
            content_type: "a",
            name: "lowest",
        }),
        1,
    );
    store.add_factory_with_priority(
        Arc::new(MockNamedFactory {
            content_type: "b",
            name: "middle",
        }),
        50,
    );
    store.add_factory_with_priority(
        Arc::new(MockNamedFactory {
            content_type: "c",
            name: "highest",
        }),
        999,
    );

    let available = store.available();
    assert_eq!(available[0].0, "highest");
    assert_eq!(available[1].0, "middle");
    assert_eq!(available[2].0, "lowest");
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
