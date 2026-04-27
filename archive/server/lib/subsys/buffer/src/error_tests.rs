//! Display and trait-shape tests for `BufferError`.

use {super::BufferError, reovim_kernel::api::v1::BufferId, std::num::NonZeroU32};

fn make_attachment_id() -> crate::buffer::CodecAttachmentId {
    crate::buffer::CodecAttachmentId(NonZeroU32::new(1).unwrap())
}

#[test]
fn display_not_found() {
    let err = BufferError::NotFound(BufferId::from_raw(42));
    assert!(err.to_string().contains("buffer not found"), "{err}");
}

#[test]
fn display_invalid_edit() {
    let err = BufferError::InvalidEdit("offset out of bounds".into());
    assert_eq!(err.to_string(), "invalid edit: offset out of bounds");
}

#[test]
fn display_codec_poisoned() {
    let err = BufferError::CodecPoisoned(make_attachment_id(), "thread panicked".into());
    let s = err.to_string();
    assert!(s.contains("poisoned"), "{s}");
    assert!(s.contains("thread panicked"), "{s}");
}

#[test]
fn display_duplicate_codec_name() {
    let err = BufferError::DuplicateCodecName("utf8-index".into());
    assert_eq!(err.to_string(), "attached codec name conflict: utf8-index");
}

#[test]
fn display_io() {
    let err = BufferError::Io("broken pipe".into());
    assert_eq!(err.to_string(), "io: broken pipe");
}

#[test]
fn display_driver() {
    let err = BufferError::Driver("cdylib not loaded".into());
    assert_eq!(err.to_string(), "driver lifecycle: cdylib not loaded");
}

#[test]
fn debug_contains_variant_name() {
    let err = BufferError::NotFound(BufferId::from_raw(1));
    assert!(format!("{err:?}").contains("NotFound"));
}

#[test]
fn is_std_error() {
    let err: Box<dyn std::error::Error> = Box::new(BufferError::InvalidEdit("x".into()));
    assert!(err.to_string().starts_with("invalid edit"));
}

#[test]
fn clone_preserves_message() {
    let err = BufferError::Io("disk full".into());
    let err2 = err.clone();
    assert_eq!(err.to_string(), err2.to_string());
}
