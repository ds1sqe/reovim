//! Tests for codec trait and `DecodeResult`.

use reovim_driver_annotation::{Annotation, AnnotationKind, AnnotationPayload, AnnotationTarget};

use {super::*, crate::ContentType};

#[test]
fn decode_result_valid_when_not_lossy() {
    let result = DecodeResult {
        content: "hello".to_string(),
        annotations: vec![],
        metadata: CodecMetadata::new(ContentType::new("text/utf-8")),
        lossy: false,
        readonly: false,
    };
    assert!(result.is_valid());
}

#[test]
fn decode_result_valid_when_lossy_and_readonly() {
    let result = DecodeResult {
        content: "hex dump".to_string(),
        annotations: vec![],
        metadata: CodecMetadata::new(ContentType::new("binary/raw")),
        lossy: true,
        readonly: true,
    };
    assert!(result.is_valid());
}

#[test]
fn decode_result_invalid_when_lossy_but_not_readonly() {
    let result = DecodeResult {
        content: "bad".to_string(),
        annotations: vec![],
        metadata: CodecMetadata::new(ContentType::new("text/utf-8")),
        lossy: true,
        readonly: false,
    };
    assert!(!result.is_valid());
}

#[test]
fn decode_result_valid_when_readonly_but_not_lossy() {
    let result = DecodeResult {
        content: "readonly".to_string(),
        annotations: vec![],
        metadata: CodecMetadata::new(ContentType::new("text/utf-8")),
        lossy: false,
        readonly: true,
    };
    assert!(result.is_valid());
}

#[test]
fn decode_result_with_annotations() {
    let ann = Annotation::new(
        AnnotationKind::new("content.hex.address"),
        AnnotationTarget::Line(0),
        0,
        AnnotationPayload::Text("00000000".to_string()),
    );
    let result = DecodeResult {
        content: "00000000  7f 45 4c 46".to_string(),
        annotations: vec![ann],
        metadata: CodecMetadata::new(ContentType::new("binary/raw")),
        lossy: true,
        readonly: true,
    };
    assert_eq!(result.annotations.len(), 1);
    assert!(result.is_valid());
}

#[test]
fn decode_result_clone() {
    let result = DecodeResult {
        content: "hello".to_string(),
        annotations: vec![],
        metadata: CodecMetadata::new(ContentType::new("text/utf-8")),
        lossy: false,
        readonly: false,
    };
    #[allow(clippy::redundant_clone)]
    let cloned = result.clone();
    assert_eq!(cloned.content, "hello");
    assert!(!cloned.lossy);
}

/// Mock codec for testing the trait.
struct MockCodec {
    bidirectional: bool,
}

impl ContentCodec for MockCodec {
    fn decode(&self, raw: &[u8]) -> Result<DecodeResult, CodecError> {
        let content =
            String::from_utf8(raw.to_vec()).map_err(|e| CodecError::Other(e.to_string()))?;
        Ok(DecodeResult {
            content,
            annotations: vec![],
            metadata: CodecMetadata::new(ContentType::new("text/utf-8")),
            lossy: false,
            readonly: false,
        })
    }

    fn encode(
        &self,
        content: &str,
        _metadata: &CodecMetadata,
    ) -> Option<Result<Vec<u8>, CodecError>> {
        if self.bidirectional {
            Some(Ok(content.as_bytes().to_vec()))
        } else {
            None
        }
    }
}

#[test]
fn mock_codec_bidirectional() {
    let codec = MockCodec {
        bidirectional: true,
    };
    let result = codec.decode(b"hello").unwrap();
    assert_eq!(result.content, "hello");

    let metadata = CodecMetadata::new(ContentType::new("text/utf-8"));
    let encoded = codec.encode("hello", &metadata).unwrap().unwrap();
    assert_eq!(encoded, b"hello");
}

#[test]
fn mock_codec_one_way() {
    let codec = MockCodec {
        bidirectional: false,
    };
    let result = codec.decode(b"hello").unwrap();
    assert_eq!(result.content, "hello");

    let metadata = CodecMetadata::new(ContentType::new("text/utf-8"));
    assert!(codec.encode("hello", &metadata).is_none());
}

#[test]
fn mock_codec_decode_error() {
    let codec = MockCodec {
        bidirectional: true,
    };
    let result = codec.decode(&[0xFF, 0xFE]);
    assert!(result.is_err());
}

#[test]
fn trait_object_works() {
    let codec: Box<dyn ContentCodec> = Box::new(MockCodec {
        bidirectional: true,
    });
    let result = codec.decode(b"test").unwrap();
    assert_eq!(result.content, "test");
}
