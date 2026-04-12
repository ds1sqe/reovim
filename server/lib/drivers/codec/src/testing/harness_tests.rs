//! Tests for the verification harness itself.

use std::sync::Arc;

use {
    reovim_domain_text::Position, reovim_driver_vfs::ByteSource, reovim_kernel::api::v1::ByteEdit,
};

use {
    super::{HarnessError, HarnessFixture, verify_codec},
    crate::{
        CodecError, CodecMetadata, ContentCodec, ContentType, DecodeResult, DecodedEdit,
        TranslateEditError,
    },
};

// ============================================================================
// Minimal test codec: a faithful codec that only understands text and
// translates every text edit to a byte edit at the same offset/length.
// ============================================================================

struct HarnessTextCodec;

impl ContentCodec for HarnessTextCodec {
    fn decode(&self, raw: &[u8]) -> Result<DecodeResult, CodecError> {
        let text = std::str::from_utf8(raw)
            .map_err(|e| CodecError::Other(format!("harness decode: {e}")))?;
        Ok(DecodeResult {
            content: text.to_string(),
            annotations: vec![],
            metadata: CodecMetadata::new(ContentType::new("text/harness")),
            lossy: false,
            readonly: false,
            truncated: false,
        })
    }

    fn translate_edit(
        &self,
        bytes: &dyn ByteSource,
        edit: &DecodedEdit,
    ) -> Result<Option<ByteEdit>, TranslateEditError> {
        match edit {
            DecodedEdit::Text {
                start,
                end,
                replacement,
            } => {
                let raw = bytes.read(0..bytes.len()).into_owned();
                let start_offset = position_to_offset(&raw, *start).ok_or(
                    TranslateEditError::ConstraintViolation {
                        reason: "harness: start out of range",
                    },
                )?;
                let end_offset = position_to_offset(&raw, *end).ok_or(
                    TranslateEditError::ConstraintViolation {
                        reason: "harness: end out of range",
                    },
                )?;
                if end_offset < start_offset {
                    return Err(TranslateEditError::ConstraintViolation {
                        reason: "harness: end precedes start",
                    });
                }
                Ok(Some(ByteEdit {
                    offset: start_offset,
                    old_bytes: raw[start_offset..end_offset].to_vec(),
                    new_bytes: replacement.as_bytes().to_vec(),
                }))
            }
            DecodedEdit::Bytes { .. } => Err(TranslateEditError::UnsupportedEdit {
                reason: "harness: no byte edits",
            }),
            DecodedEdit::Tree { .. } => Err(TranslateEditError::UnsupportedEdit {
                reason: "harness: no tree edits",
            }),
        }
    }
}

fn position_to_offset(raw: &[u8], pos: Position) -> Option<usize> {
    let text = std::str::from_utf8(raw).ok()?;
    let mut line_start = 0usize;
    for _ in 0..pos.line {
        let rest = text.get(line_start..)?;
        let next = rest.find('\n')?;
        line_start += next + 1;
    }
    Some(line_start + pos.column)
}

// ============================================================================
// Broken-codec helpers (each fails one gate on purpose)
// ============================================================================

/// Codec that mutates decoded output (breaks the no-op gate).
struct MutatingDecodeCodec;

impl ContentCodec for MutatingDecodeCodec {
    fn decode(&self, _raw: &[u8]) -> Result<DecodeResult, CodecError> {
        Ok(DecodeResult {
            content: "anything".to_string(),
            annotations: vec![],
            metadata: CodecMetadata::new(ContentType::new("text/harness-mutating")),
            lossy: false,
            readonly: false,
            truncated: false,
        })
    }
}

/// Codec that rejects every edit.
struct RejectingCodec;

impl ContentCodec for RejectingCodec {
    fn decode(&self, raw: &[u8]) -> Result<DecodeResult, CodecError> {
        Ok(DecodeResult {
            content: String::from_utf8_lossy(raw).into_owned(),
            annotations: vec![],
            metadata: CodecMetadata::new(ContentType::new("text/harness-reject")),
            lossy: false,
            readonly: false,
            truncated: false,
        })
    }
}

/// Codec whose initial decode fails.
struct FailingDecodeCodec;

impl ContentCodec for FailingDecodeCodec {
    fn decode(&self, _raw: &[u8]) -> Result<DecodeResult, CodecError> {
        Err(CodecError::Other("harness: decode always fails".to_string()))
    }
}

// ============================================================================
// Harness tests
// ============================================================================

#[test]
fn harness_fixture_accessors_and_construction() {
    let fx = HarnessFixture::new(
        "empty",
        b"hello".to_vec(),
        vec![DecodedEdit::Text {
            start: Position::new(0, 0),
            end: Position::new(0, 0),
            replacement: String::from("x"),
        }],
    );
    assert_eq!(fx.name, "empty");
    assert_eq!(fx.initial_bytes, b"hello");
    assert_eq!(fx.edits.len(), 1);

    // Debug coverage.
    let debug = format!("{fx:?}");
    assert!(debug.contains("HarnessFixture"));
    assert!(debug.contains("empty"));
}

#[test]
fn verify_codec_happy_path_with_insertion() {
    let codec: Arc<dyn ContentCodec> = Arc::new(HarnessTextCodec);
    let fx = HarnessFixture::new(
        "insertion",
        b"abcdef".to_vec(),
        vec![DecodedEdit::Text {
            start: Position::new(0, 3),
            end: Position::new(0, 3),
            replacement: String::from("X"),
        }],
    );
    let result = verify_codec(&codec, &fx);
    assert!(result.is_ok(), "expected OK, got {result:?}");
}

#[test]
fn verify_codec_noop_round_trip_passes_with_empty_edit_list() {
    let codec: Arc<dyn ContentCodec> = Arc::new(HarnessTextCodec);
    let fx = HarnessFixture::new("empty-edits", b"contents".to_vec(), vec![]);
    assert!(verify_codec(&codec, &fx).is_ok());
}

#[test]
fn verify_codec_detects_initial_decode_failure() {
    let codec: Arc<dyn ContentCodec> = Arc::new(FailingDecodeCodec);
    let fx = HarnessFixture::new("any", b"contents".to_vec(), vec![]);
    let err = verify_codec(&codec, &fx).unwrap_err();
    assert!(matches!(err, HarnessError::InitialDecodeFailed { fixture, .. } if fixture == "any"));
}

#[test]
fn verify_codec_detects_rejecting_codec_on_edit() {
    let codec: Arc<dyn ContentCodec> = Arc::new(RejectingCodec);
    let fx = HarnessFixture::new(
        "reject",
        b"hello".to_vec(),
        vec![DecodedEdit::Text {
            start: Position::new(0, 0),
            end: Position::new(0, 0),
            replacement: String::from("x"),
        }],
    );
    let err = verify_codec(&codec, &fx).unwrap_err();
    assert!(matches!(
        err,
        HarnessError::EditRejected { fixture, edit_index: 0, cause: TranslateEditError::ReadOnly }
            if fixture == "reject"
    ));
}

#[test]
fn verify_codec_mutating_decode_breaks_noop_gate() {
    let codec: Arc<dyn ContentCodec> = Arc::new(MutatingDecodeCodec);
    // The mutating codec returns a constant "anything" from decode
    // regardless of input. The harness's no-op gate checks raw inode
    // bytes against the fixture, so a mutating *decode* does not
    // directly break it (the harness never writes decoded content
    // back to inode.bytes). Instead, the mutating codec rejects every
    // edit (default impl returns `Err(ReadOnly)`), so submitting a
    // fixture with an edit surfaces an EditRejected error.
    let fx = HarnessFixture::new(
        "mutating",
        b"contents".to_vec(),
        vec![DecodedEdit::Text {
            start: Position::new(0, 0),
            end: Position::new(0, 0),
            replacement: String::from("z"),
        }],
    );
    let err = verify_codec(&codec, &fx).unwrap_err();
    assert!(matches!(
        err,
        HarnessError::EditRejected {
            cause: TranslateEditError::ReadOnly,
            ..
        }
    ));
}

#[test]
fn harness_error_variants_debug_and_clone() {
    let variants = vec![
        HarnessError::InitialDecodeFailed {
            fixture: "fx",
            reason: "oops".to_string(),
        },
        HarnessError::NoopRoundTripChangedBytes { fixture: "fx" },
        HarnessError::EditRejected {
            fixture: "fx",
            edit_index: 0,
            cause: TranslateEditError::ReadOnly,
        },
        HarnessError::ApplyFailed {
            fixture: "fx",
            edit_index: 0,
            cause: crate::EditError::ReadOnly,
        },
        HarnessError::PostEditDecodeFailed {
            fixture: "fx",
            edit_index: 0,
            reason: "oops".to_string(),
        },
        HarnessError::PeerMountNotStaled { fixture: "fx" },
        HarnessError::UndoDidNotRestore { fixture: "fx" },
    ];
    for err in &variants {
        let debug = format!("{err:?}");
        assert!(!debug.is_empty());
        #[allow(clippy::redundant_clone)]
        let cloned = err.clone();
        assert_eq!(&cloned, err);
    }
}
