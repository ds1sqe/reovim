//! Tests for `DecodedEdit`.

use super::DecodedEdit;

#[test]
fn insertion_detection() {
    let edit = DecodedEdit::Bytes {
        offset: 4,
        old_len: 0,
        new_bytes: b"abc".to_vec(),
    };
    assert!(edit.is_insertion());
    assert!(!edit.is_deletion());
}

#[test]
fn deletion_detection() {
    let edit = DecodedEdit::Text {
        start: reovim_types_text::Position::new(0, 0),
        end: reovim_types_text::Position::new(0, 3),
        replacement: String::new(),
    };
    assert!(!edit.is_insertion());
    assert!(edit.is_deletion());
}

#[test]
fn text_edit_variant_fields_used() {
    let edit = DecodedEdit::Text {
        start: reovim_types_text::Position::new(0, 2),
        end: reovim_types_text::Position::new(0, 3),
        replacement: "x".to_string(),
    };
    let expected_start = reovim_types_text::Position::new(0, 2);
    let expected_end = reovim_types_text::Position::new(0, 3);

    assert!(matches!(
        edit,
        DecodedEdit::Text {
            start,
            end,
            replacement
        } if replacement == "x"
            && start == expected_start
            && end == expected_end
    ));
}

#[test]
fn bytes_edit_variant_fields_used() {
    let edit = DecodedEdit::Bytes {
        offset: 8,
        old_len: 2,
        new_bytes: vec![0xAA, 0xBB],
    };
    assert!(matches!(
        edit,
        DecodedEdit::Bytes {
            offset: 8,
            old_len: 2,
            new_bytes
        } if new_bytes == vec![0xAA, 0xBB]
    ));
}

#[test]
fn reserved_variant_present() {
    let edit = DecodedEdit::_Reserved;
    assert!(!edit.is_insertion());
    assert!(!edit.is_deletion());
}
