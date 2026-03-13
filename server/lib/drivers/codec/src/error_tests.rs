//! Tests for codec error types.

use super::*;

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
