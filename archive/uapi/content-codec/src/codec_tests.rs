//! Tests for codec trait and `DecodeResult`.

use reovim_subsys_vfs::HeapByteSource;

use {
    super::*,
    crate::{
        Annotation, AnnotationKind, AnnotationPayload, AnnotationTarget, ContentType,
        TranslateEditError,
    },
};

#[test]
fn decode_result_valid_when_not_lossy() {
    let result = DecodeResult {
        content: "hello".to_string(),
        annotations: vec![],
        metadata: CodecMetadata::new(ContentType::new("text/utf-8")),
        lossy: false,
        readonly: false,
        truncated: false,
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
        truncated: false,
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
        truncated: false,
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
        truncated: false,
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
        truncated: false,
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
        truncated: false,
    };
    #[allow(clippy::redundant_clone)]
    let cloned = result.clone();
    assert_eq!(cloned.content, "hello");
    assert!(!cloned.lossy);
}

#[test]
fn decode_result_truncated_field() {
    let result = DecodeResult {
        content: "truncated content".to_string(),
        annotations: vec![],
        metadata: CodecMetadata::new(ContentType::new("binary/raw")),
        lossy: true,
        readonly: true,
        truncated: true,
    };
    assert!(result.truncated);
    assert!(result.is_valid());

    let not_truncated = DecodeResult {
        content: "full content".to_string(),
        annotations: vec![],
        metadata: CodecMetadata::new(ContentType::new("text/utf-8")),
        lossy: false,
        readonly: false,
        truncated: false,
    };
    assert!(!not_truncated.truncated);
}

#[test]
fn codec_view_default_has_expected_values() {
    assert_eq!(CodecView::DEFAULT.name, "default");
    assert_eq!(CodecView::DEFAULT.display, "Default");
}

#[test]
fn codec_view_equality() {
    let a = CodecView {
        name: "hex",
        display: "Hex Dump",
    };
    let b = CodecView {
        name: "hex",
        display: "Hex Dump",
    };
    assert_eq!(a, b);
}

#[test]
fn codec_view_debug() {
    let v = CodecView::DEFAULT;
    let debug = format!("{v:?}");
    assert!(debug.contains("default"));
}

/// Mock codec for testing the trait.
struct MockCodec;

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
            truncated: false,
        })
    }
}

#[test]
fn mock_codec_decode_roundtrip() {
    let codec = MockCodec;
    let result = codec.decode(b"hello").unwrap();
    assert_eq!(result.content, "hello");
}

#[test]
fn mock_codec_translate_edit_defaults_to_read_only() {
    // A synthetic domain-edit payload — the default impl rejects any edit,
    // so the wrapped value shape is irrelevant.
    #[derive(Debug, Clone, PartialEq, Eq)]
    struct DummyEdit;
    crate::impl_domain_edit!(DummyEdit);

    let codec = MockCodec;
    let bytes = HeapByteSource::new(b"hello");
    let edit = DecodedEdit::Domain(crate::DomainEdit::new(DummyEdit));

    // Default impl now returns Err(ReadOnly) so codecs that do not
    // override translate_edit behave read-only by construction (Plan 07
    // Phase 1).
    assert!(matches!(codec.translate_edit(&bytes, &edit), Err(TranslateEditError::ReadOnly)));
}

#[test]
fn mock_codec_decode_error() {
    let codec = MockCodec;
    let result = codec.decode(&[0xFF, 0xFE]);
    assert!(result.is_err());
}

#[test]
fn views_default_returns_single_default() {
    let codec = MockCodec;
    let views = codec.views();
    assert_eq!(views.len(), 1);
    assert_eq!(views[0], CodecView::DEFAULT);
}

#[test]
fn decode_view_default_delegates_to_decode() {
    let codec = MockCodec;
    let direct = codec.decode(b"test").unwrap();
    let via_view = codec.decode_view(b"test", "default").unwrap();
    assert_eq!(direct.content, via_view.content);
    assert_eq!(direct.lossy, via_view.lossy);
}

#[test]
fn trait_object_works() {
    let codec: Box<dyn ContentCodec> = Box::new(MockCodec);
    let result = codec.decode(b"test").unwrap();
    assert_eq!(result.content, "test");
}

#[test]
fn decode_streaming_default_returns_none() {
    use std::path::Path;

    use reovim_subsys_vfs::{FileMetadata, SeekFrom, VfsError};

    struct DummyHandle;
    impl reovim_subsys_vfs::FileHandle for DummyHandle {
        fn read(&mut self, _buf: &mut [u8]) -> Result<usize, VfsError> {
            Ok(0)
        }
        fn write(&mut self, _buf: &[u8]) -> Result<usize, VfsError> {
            Ok(0)
        }
        fn seek(&mut self, _pos: SeekFrom) -> Result<u64, VfsError> {
            Ok(0)
        }
        fn flush(&mut self) -> Result<(), VfsError> {
            Ok(())
        }
        fn metadata(&self) -> Result<FileMetadata, VfsError> {
            Err(VfsError::NotFound("dummy".into()))
        }
        fn path(&self) -> &Path {
            Path::new("/dev/null")
        }
        fn position(&self) -> u64 {
            0
        }
    }

    let codec = MockCodec;
    let mut handle = DummyHandle;
    assert!(codec.decode_streaming(&mut handle, 1024).is_none());
}
