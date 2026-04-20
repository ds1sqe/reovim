//! Tests for the verification harness itself.
//!
//! The harness is domain-neutral. Tests use a byte-oriented codec that
//! accepts `DecodedEdit::Bytes` directly — no domain types are required
//! to exercise the four harness gates.

use std::sync::Arc;

use {reovim_kernel::api::v1::ByteEdit, reovim_subsys_vfs::ByteSource};

use {
    super::{HarnessError, HarnessFixture, verify_codec},
    crate::{
        CodecError, CodecMetadata, ContentCodec, ContentType, DecodeResult, DecodedEdit,
        TranslateEditError,
    },
};

// ============================================================================
// Minimal test codec: a faithful byte codec that accepts `DecodedEdit::Bytes`
// and translates each to a one-for-one byte edit.
// ============================================================================

struct HarnessByteCodec;

impl ContentCodec for HarnessByteCodec {
    fn decode(&self, raw: &[u8]) -> Result<DecodeResult, CodecError> {
        Ok(DecodeResult {
            content: String::from_utf8_lossy(raw).into_owned(),
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
            DecodedEdit::Bytes {
                offset,
                old_len,
                new_bytes,
            } => {
                let raw = bytes.read(0..bytes.len()).into_owned();
                let end = offset + old_len;
                if end > raw.len() {
                    return Err(TranslateEditError::ConstraintViolation {
                        reason: "harness: edit exceeds source length",
                    });
                }
                Ok(Some(ByteEdit {
                    offset: *offset,
                    old_bytes: raw[*offset..end].to_vec(),
                    new_bytes: new_bytes.clone(),
                }))
            }
            DecodedEdit::Domain(_) => Err(TranslateEditError::UnsupportedEdit {
                reason: "harness: no domain edits",
            }),
            DecodedEdit::Tree { .. } => Err(TranslateEditError::UnsupportedEdit {
                reason: "harness: no tree edits",
            }),
        }
    }
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
        vec![DecodedEdit::Bytes {
            offset: 0,
            old_len: 0,
            new_bytes: b"x".to_vec(),
        }],
    );
    assert_eq!(fx.name, "empty");
    assert_eq!(fx.initial_bytes, b"hello");
    assert_eq!(fx.edits.len(), 1);

    let debug = format!("{fx:?}");
    assert!(debug.contains("HarnessFixture"));
    assert!(debug.contains("empty"));
}

#[test]
fn verify_codec_happy_path_with_insertion() {
    let codec: Arc<dyn ContentCodec> = Arc::new(HarnessByteCodec);
    let fx = HarnessFixture::new(
        "insertion",
        b"abcdef".to_vec(),
        vec![DecodedEdit::Bytes {
            offset: 3,
            old_len: 0,
            new_bytes: b"X".to_vec(),
        }],
    );
    let result = verify_codec(&codec, &fx);
    assert!(result.is_ok(), "expected OK, got {result:?}");
}

#[test]
fn verify_codec_noop_round_trip_passes_with_empty_edit_list() {
    let codec: Arc<dyn ContentCodec> = Arc::new(HarnessByteCodec);
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
        vec![DecodedEdit::Bytes {
            offset: 0,
            old_len: 0,
            new_bytes: b"x".to_vec(),
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
    let fx = HarnessFixture::new(
        "mutating",
        b"contents".to_vec(),
        vec![DecodedEdit::Bytes {
            offset: 0,
            old_len: 0,
            new_bytes: b"z".to_vec(),
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
