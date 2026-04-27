//! Tests for legacy encoding codec.

use {
    reovim_content_codec::{ContentCodec, ContentType, DecodedEdit, DomainEdit},
    reovim_content_codec_text::TextEdit,
    reovim_domain_text::Position,
    reovim_subsys_vfs::HeapByteSource,
};

use super::*;

fn latin1_codec() -> LegacyCodec {
    LegacyCodec::new(super::super::classifier::LATIN_1, false)
}

fn cp1252_codec() -> LegacyCodec {
    LegacyCodec::new(super::super::classifier::WINDOWS_1252, true)
}

#[test]
fn latin1_decode_accented() {
    let codec = latin1_codec();
    // "café" in Latin-1: c(0x63) a(0x61) f(0x66) é(0xE9)
    let bytes: &[u8] = &[0x63, 0x61, 0x66, 0xE9];
    let result = codec.decode(bytes).unwrap();
    assert_eq!(result.content, "caf\u{e9}");
    assert!(!result.lossy);
    assert!(!result.readonly);
}

#[test]
fn latin1_round_trip() {
    let codec = latin1_codec();
    let bytes: &[u8] = &[0x63, 0x61, 0x66, 0xE9];
    let result = codec.decode(bytes).unwrap();
    let encoded = codec
        .encode_fragment(&result.content, &result.metadata)
        .unwrap();
    assert_eq!(encoded, bytes);
}

#[test]
fn cp1252_decode_smart_quotes() {
    let codec = cp1252_codec();
    // Left/right double quotes in Windows-1252: 0x93/0x94
    let bytes: &[u8] = &[0x93, b'h', b'i', 0x94];
    let result = codec.decode(bytes).unwrap();
    assert_eq!(result.content, "\u{201c}hi\u{201d}");
}

#[test]
fn cp1252_round_trip() {
    let codec = cp1252_codec();
    let bytes: &[u8] = &[0x93, b'h', b'i', 0x94];
    let result = codec.decode(bytes).unwrap();
    let encoded = codec
        .encode_fragment(&result.content, &result.metadata)
        .unwrap();
    assert_eq!(encoded, bytes);
}

#[test]
fn cp1252_em_dash() {
    let codec = cp1252_codec();
    // Em dash is 0x97 in Windows-1252
    let bytes: &[u8] = &[b'a', 0x97, b'b'];
    let result = codec.decode(bytes).unwrap();
    assert_eq!(result.content, "a\u{2014}b");
}

#[test]
fn ascii_passthrough_latin1() {
    let codec = latin1_codec();
    let result = codec.decode(b"Hello").unwrap();
    assert_eq!(result.content, "Hello");
    let encoded = codec.encode_fragment("Hello", &result.metadata).unwrap();
    assert_eq!(encoded, b"Hello");
}

#[test]
fn ascii_passthrough_cp1252() {
    let codec = cp1252_codec();
    let result = codec.decode(b"Hello").unwrap();
    assert_eq!(result.content, "Hello");
    let encoded = codec.encode_fragment("Hello", &result.metadata).unwrap();
    assert_eq!(encoded, b"Hello");
}

#[test]
fn metadata_has_encoding_latin1() {
    let codec = latin1_codec();
    let result = codec.decode(b"Hi").unwrap();
    assert_eq!(result.metadata.get("encoding"), Some("latin-1"));
}

#[test]
fn metadata_has_encoding_cp1252() {
    let codec = cp1252_codec();
    let result = codec.decode(b"Hi").unwrap();
    assert_eq!(result.metadata.get("encoding"), Some("windows-1252"));
}

#[test]
fn latin1_encode_unencodable() {
    let codec = latin1_codec();
    let metadata = CodecMetadata::new(ContentType::new("encoding/latin-1"));
    // Korean character can't be encoded in Latin-1
    let result = codec.encode_fragment("한", &metadata);
    assert!(result.is_err());
}

#[test]
fn cp1252_encode_unencodable() {
    let codec = cp1252_codec();
    let metadata = CodecMetadata::new(ContentType::new("encoding/windows-1252"));
    // Emoji can't be encoded in Windows-1252
    let result = codec.encode_fragment("🎉", &metadata);
    assert!(result.is_err());
}

#[test]
fn empty_input_latin1() {
    let codec = latin1_codec();
    let result = codec.decode(b"").unwrap();
    assert_eq!(result.content, "");
}

#[test]
fn empty_input_cp1252() {
    let codec = cp1252_codec();
    let result = codec.decode(b"").unwrap();
    assert_eq!(result.content, "");
}

#[test]
fn is_windows_1252_accessor() {
    assert!(!latin1_codec().is_windows_1252());
    assert!(cp1252_codec().is_windows_1252());
}

#[test]
fn latin1_full_range_decode() {
    let codec = latin1_codec();
    // All bytes 0xA0..0xFF should decode to their Unicode equivalents
    let bytes: Vec<u8> = (0xA0..=0xFF).collect();
    let result = codec.decode(&bytes).unwrap();
    assert_eq!(result.content.chars().count(), bytes.len());
    for (i, ch) in result.content.chars().enumerate() {
        #[allow(clippy::cast_possible_truncation)]
        let expected = (0xA0 + i) as u32;
        assert_eq!(ch as u32, expected);
    }
}

#[test]
fn translate_edit_ascii_replacement() {
    let codec = latin1_codec();
    let bytes = HeapByteSource::new(b"abc");
    let edit = DecodedEdit::Domain(DomainEdit::new(TextEdit {
        start: Position::new(0, 1),
        end: Position::new(0, 2),
        replacement: "Z".to_string(),
    }));

    let translated = codec
        .translate_edit(&bytes, &edit)
        .expect("latin1 replacement accepted")
        .expect("produced a byte edit");

    assert_eq!(translated.offset, 1);
    assert_eq!(translated.old_bytes, b"b");
    assert_eq!(translated.new_bytes, b"Z");
}

#[test]
fn translate_edit_cp1252_replacement() {
    let codec = cp1252_codec();

    // left quote in Windows-1252: 0x93
    let raw = [0x93, b'h', b'i', 0x94];
    let bytes = HeapByteSource::new(raw);
    let edit = DecodedEdit::Domain(DomainEdit::new(TextEdit {
        start: Position::new(0, 0),
        end: Position::new(0, 1),
        replacement: "A".to_string(),
    }));

    let translated = codec
        .translate_edit(&bytes, &edit)
        .expect("cp1252 replacement accepted")
        .expect("produced a byte edit");

    assert_eq!(translated.offset, 0);
    assert_eq!(translated.old_bytes, vec![0x93]);
    assert_eq!(translated.new_bytes, b"A");
}

#[test]
fn translate_edit_end_before_start_rejected() {
    // MC/DC 121:0: end_decoded < start_decoded → ConstraintViolation.
    use reovim_content_codec::TranslateEditError;
    let codec = latin1_codec();
    let bytes = HeapByteSource::new(b"abcdef");
    let edit = DecodedEdit::Domain(DomainEdit::new(TextEdit {
        start: Position::new(0, 4),
        end: Position::new(0, 1),
        replacement: String::new(),
    }));
    assert!(matches!(
        codec.translate_edit(&bytes, &edit),
        Err(TranslateEditError::ConstraintViolation { .. })
    ));
}

#[test]
fn translate_edit_bytes_variant_is_not_supported() {
    use reovim_content_codec::TranslateEditError;
    let codec = latin1_codec();
    let bytes = HeapByteSource::new(b"abc");
    let edit = DecodedEdit::Bytes {
        offset: 1,
        old_len: 1,
        new_bytes: b"x".to_vec(),
    };

    assert!(matches!(
        codec.translate_edit(&bytes, &edit),
        Err(TranslateEditError::UnsupportedEdit { .. })
    ));
}

#[test]
fn translate_edit_tree_variant_is_not_supported() {
    // Exercises codec.rs lines 91-93 (merged `_` arm that covers Tree and
    // any future #[non_exhaustive] variants).
    use reovim_content_codec::{TranslateEditError, TreeOp, TreePath};

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct TestOp;
    reovim_content_codec::impl_tree_op!(TestOp);

    let codec = latin1_codec();
    let bytes = HeapByteSource::new(b"abc");
    let edit = DecodedEdit::Tree {
        path: TreePath::root(),
        op: TreeOp::new(TestOp),
    };

    assert!(matches!(
        codec.translate_edit(&bytes, &edit),
        Err(TranslateEditError::UnsupportedEdit { .. })
    ));
}

#[test]
fn translate_edit_multiline_second_line() {
    // Exercises codec.rs lines 198-200: `line_start += next_newline + 1`
    // inside the `for _ in 0..pos.line` loop.  Without this test the loop
    // body is never reached because all other tests target line 0.
    let codec = latin1_codec();
    // "abc\ndef" is pure ASCII, which is a subset of Latin-1.
    let bytes = HeapByteSource::new(b"abc\ndef");
    let edit = DecodedEdit::Domain(DomainEdit::new(TextEdit {
        start: Position::new(1, 0),
        end: Position::new(1, 1),
        replacement: "X".to_string(),
    }));

    let translated = codec
        .translate_edit(&bytes, &edit)
        .expect("multiline edit accepted")
        .expect("produced a byte edit");

    // "abc\n" is 4 bytes → 'd' is at raw offset 4.
    assert_eq!(translated.offset, 4);
    assert_eq!(translated.old_bytes, b"d");
    assert_eq!(translated.new_bytes, b"X");
}

#[test]
fn translate_edit_column_at_line_end() {
    // Exercises codec.rs line 222:
    //   `(chars_seen == pos.column).then_some(line_end)`
    //
    // This branch executes when `pos.column` equals the character count of
    // the line (position at end-of-line, past the last char).  Existing
    // tests all target positions strictly inside the line.
    let codec = latin1_codec();
    // "abc" has 3 chars; position (0, 3) is past the last char.
    let bytes = HeapByteSource::new(b"abc");
    let edit = DecodedEdit::Domain(DomainEdit::new(TextEdit {
        start: Position::new(0, 3),
        end: Position::new(0, 3),
        replacement: "X".to_string(),
    }));

    let translated = codec
        .translate_edit(&bytes, &edit)
        .expect("end-of-line insert accepted")
        .expect("produced a byte edit");

    // Inserting at offset 3 (after 'c') appends to the string.
    assert_eq!(translated.offset, 3);
    assert_eq!(translated.old_bytes, b"");
    assert_eq!(translated.new_bytes, b"X");
}

#[test]
fn translate_edit_start_position_out_of_range() {
    // Exercises codec.rs line 115: start position None → ConstraintViolation.
    use reovim_content_codec::TranslateEditError;
    let codec = latin1_codec();
    let bytes = HeapByteSource::new(b"abc");
    let edit = DecodedEdit::Domain(DomainEdit::new(TextEdit {
        start: Position::new(5, 0), // line 5 does not exist in "abc"
        end: Position::new(5, 1),
        replacement: "X".to_string(),
    }));

    assert!(matches!(
        codec.translate_edit(&bytes, &edit),
        Err(TranslateEditError::ConstraintViolation { .. })
    ));
}

#[test]
fn translate_edit_end_position_out_of_range() {
    // Exercises codec.rs line 120: end position None → ConstraintViolation,
    // while start position is valid so line 115 is NOT triggered.
    use reovim_content_codec::TranslateEditError;
    let codec = latin1_codec();
    let bytes = HeapByteSource::new(b"abc");
    let edit = DecodedEdit::Domain(DomainEdit::new(TextEdit {
        start: Position::new(0, 0), // valid
        end: Position::new(5, 0),   // line 5 does not exist in "abc"
        replacement: "X".to_string(),
    }));

    assert!(matches!(
        codec.translate_edit(&bytes, &edit),
        Err(TranslateEditError::ConstraintViolation { .. })
    ));
}

#[test]
fn translate_edit_unencodable_replacement() {
    // Exercises codec.rs line 146: encode_fragment error path → ConstraintViolation.
    // Korean characters are outside the Latin-1 (U+0000..U+00FF) range.
    use reovim_content_codec::TranslateEditError;
    let codec = latin1_codec();
    let bytes = HeapByteSource::new(b"abc");
    let edit = DecodedEdit::Domain(DomainEdit::new(TextEdit {
        start: Position::new(0, 0),
        end: Position::new(0, 1),
        replacement: "한".to_string(), // U+D55C — not representable in Latin-1
    }));

    assert!(matches!(
        codec.translate_edit(&bytes, &edit),
        Err(TranslateEditError::ConstraintViolation { .. })
    ));
}
