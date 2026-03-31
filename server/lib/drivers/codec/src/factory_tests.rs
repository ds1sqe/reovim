//! Tests for content codec factory trait.

use {
    super::*,
    crate::{CodecError, CodecMetadata, DecodeResult},
};

struct NullCodec;

impl ContentCodec for NullCodec {
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

    fn encode(
        &self,
        content: &str,
        _metadata: &CodecMetadata,
    ) -> Option<Result<Vec<u8>, CodecError>> {
        Some(Ok(content.as_bytes().to_vec()))
    }
}

struct TestFactory;

impl ContentCodecFactory for TestFactory {
    fn create(&self, content_type: &ContentType) -> Option<Box<dyn ContentCodec>> {
        if content_type.as_str() == "text/utf-8" {
            Some(Box::new(NullCodec))
        } else {
            None
        }
    }

    fn supported_content_types(&self) -> Vec<&str> {
        vec!["text/utf-8"]
    }

    fn name(&self) -> &'static str {
        "test"
    }
}

#[test]
fn create_supported() {
    let factory = TestFactory;
    let ct = ContentType::new("text/utf-8");
    assert!(factory.create(&ct).is_some());
}

#[test]
fn create_unsupported() {
    let factory = TestFactory;
    let ct = ContentType::new("binary/raw");
    assert!(factory.create(&ct).is_none());
}

#[test]
fn supported_types() {
    let factory = TestFactory;
    assert_eq!(factory.supported_content_types(), vec!["text/utf-8"]);
}

#[test]
fn name() {
    let factory = TestFactory;
    assert_eq!(factory.name(), "test");
}

#[test]
fn trait_object_works() {
    let factory: Box<dyn ContentCodecFactory> = Box::new(TestFactory);
    let ct = ContentType::new("text/utf-8");
    assert!(factory.create(&ct).is_some());
    assert_eq!(factory.name(), "test");
}
