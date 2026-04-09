//! Tests for codec error types.

use {super::*, crate::errors::TranslateEditError};

#[test]
fn display_invalid_sequence() {
    let err = CodecError::InvalidSequence {
        offset: 42,
        detail: "unexpected byte 0xFF".to_string(),
    };
    assert_eq!(format!("{err}"), "invalid byte sequence at offset 42: unexpected byte 0xFF");
}

#[test]
fn display_unmappable_character() {
    let err = CodecError::UnmappableCharacter {
        offset: 10,
        detail: "U+4E16 not in Latin-1".to_string(),
    };
    assert_eq!(format!("{err}"), "unmappable character at offset 10: U+4E16 not in Latin-1");
}

#[test]
fn display_other() {
    let err = CodecError::Other("something went wrong".to_string());
    assert_eq!(format!("{err}"), "something went wrong");
}

#[test]
fn debug_format() {
    let err = CodecError::Other("test".to_string());
    let debug = format!("{err:?}");
    assert!(debug.contains("Other"));
    assert!(debug.contains("test"));
}

#[test]
fn clone_and_eq() {
    let err = CodecError::InvalidSequence {
        offset: 0,
        detail: "bad".to_string(),
    };
    let cloned = err.clone();
    assert_eq!(err, cloned);
}

#[test]
fn error_trait() {
    let err = CodecError::Other("fail".to_string());
    let _: &dyn std::error::Error = &err;
}

// ============================================================================
// TranslateEditError tests (Plan 07 Phase 1)
// ============================================================================

#[test]
fn translate_edit_error_read_only_display() {
    let err = TranslateEditError::ReadOnly;
    assert_eq!(format!("{err}"), "codec is read-only");
}

#[test]
fn translate_edit_error_read_only_debug() {
    let err = TranslateEditError::ReadOnly;
    assert_eq!(format!("{err:?}"), "ReadOnly");
}

#[test]
fn translate_edit_error_read_only_match() {
    let err = TranslateEditError::ReadOnly;
    assert!(matches!(err, TranslateEditError::ReadOnly));
}

#[test]
fn translate_edit_error_unsupported_edit_display() {
    let err = TranslateEditError::UnsupportedEdit {
        reason: "utf-8 codec does not accept byte edits",
    };
    assert_eq!(
        format!("{err}"),
        "unsupported edit variant: utf-8 codec does not accept byte edits"
    );
}

#[test]
fn translate_edit_error_unsupported_edit_debug() {
    let err = TranslateEditError::UnsupportedEdit { reason: "nope" };
    let debug = format!("{err:?}");
    assert!(debug.contains("UnsupportedEdit"));
    assert!(debug.contains("nope"));
}

#[test]
fn translate_edit_error_unsupported_edit_match() {
    let err = TranslateEditError::UnsupportedEdit { reason: "no tree" };
    assert!(matches!(
        err,
        TranslateEditError::UnsupportedEdit { reason } if reason == "no tree"
    ));
}

#[test]
fn translate_edit_error_constraint_violation_display() {
    let err = TranslateEditError::ConstraintViolation {
        reason: "elf section size must not change",
    };
    assert_eq!(format!("{err}"), "constraint violation: elf section size must not change");
}

#[test]
fn translate_edit_error_constraint_violation_debug() {
    let err = TranslateEditError::ConstraintViolation {
        reason: "stored entry size fixed",
    };
    let debug = format!("{err:?}");
    assert!(debug.contains("ConstraintViolation"));
    assert!(debug.contains("stored entry size fixed"));
}

#[test]
fn translate_edit_error_constraint_violation_match() {
    let err = TranslateEditError::ConstraintViolation {
        reason: "domain rule",
    };
    assert!(matches!(
        err,
        TranslateEditError::ConstraintViolation { reason } if reason == "domain rule"
    ));
}

#[test]
fn translate_edit_error_malformed_path_display() {
    let err = TranslateEditError::MalformedPath {
        reason: "tree path component not found",
    };
    assert_eq!(format!("{err}"), "malformed tree path: tree path component not found");
}

#[test]
fn translate_edit_error_malformed_path_debug() {
    let err = TranslateEditError::MalformedPath { reason: "stale" };
    let debug = format!("{err:?}");
    assert!(debug.contains("MalformedPath"));
    assert!(debug.contains("stale"));
}

#[test]
fn translate_edit_error_malformed_path_match() {
    let err = TranslateEditError::MalformedPath { reason: "missing" };
    assert!(matches!(
        err,
        TranslateEditError::MalformedPath { reason } if reason == "missing"
    ));
}

#[test]
fn translate_edit_error_internal_display() {
    let err = TranslateEditError::Internal {
        reason: "byte source read failed",
    };
    assert_eq!(format!("{err}"), "internal translate_edit failure: byte source read failed");
}

#[test]
fn translate_edit_error_internal_debug() {
    let err = TranslateEditError::Internal {
        reason: "decode failure",
    };
    let debug = format!("{err:?}");
    assert!(debug.contains("Internal"));
    assert!(debug.contains("decode failure"));
}

#[test]
fn translate_edit_error_internal_match() {
    let err = TranslateEditError::Internal { reason: "i/o" };
    assert!(matches!(
        err,
        TranslateEditError::Internal { reason } if reason == "i/o"
    ));
}

#[test]
fn translate_edit_error_is_copy() {
    let err = TranslateEditError::ReadOnly;
    let copied = err;
    // Both usable after copy — confirms Copy impl.
    let _ = err;
    let _ = copied;
}

#[test]
fn translate_edit_error_clone_and_eq() {
    let err = TranslateEditError::UnsupportedEdit { reason: "nope" };
    let cloned = err;
    assert_eq!(err, cloned);
    assert_eq!(TranslateEditError::ReadOnly, TranslateEditError::ReadOnly);
    assert_ne!(TranslateEditError::ReadOnly, TranslateEditError::Internal { reason: "x" });
}

#[test]
fn translate_edit_error_implements_error_trait() {
    let err = TranslateEditError::ReadOnly;
    let _: &dyn std::error::Error = &err;
}
