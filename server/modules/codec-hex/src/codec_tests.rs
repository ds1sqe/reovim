//! Tests for hex dump codec.

use {
    reovim_driver_codec::{ContentCodec, ContentType, DecodedEdit},
    reovim_driver_vfs::HeapByteSource,
};

use super::*;

#[test]
fn empty_input() {
    let codec = HexCodec::new();
    let result = codec.decode(b"").unwrap();
    assert_eq!(result.content, "");
    assert!(result.lossy);
    assert!(result.readonly);
}

#[test]
fn single_byte() {
    let codec = HexCodec::new();
    let result = codec.decode(&[0x41]).unwrap();
    // "00000000  41                                                |A               |\n"
    assert!(result.content.starts_with("00000000  41"));
    assert!(result.content.contains("|A"));
    assert_eq!(result.content.lines().count(), 1);
}

#[test]
fn full_line_16_bytes() {
    let codec = HexCodec::new();
    let data: Vec<u8> = (0x41..=0x50).collect(); // A-P (16 bytes)
    let result = codec.decode(&data).unwrap();
    let line = result.content.lines().next().unwrap();
    assert!(line.starts_with("00000000  41 42 43 44 45 46 47 48  49 4a 4b 4c 4d 4e 4f 50"));
    assert!(line.ends_with("|ABCDEFGHIJKLMNOP|"));
    assert_eq!(result.content.lines().count(), 1);
}

#[test]
fn two_lines_17_bytes() {
    let codec = HexCodec::new();
    let data: Vec<u8> = (0x41..=0x51).collect(); // A-Q (17 bytes)
    let result = codec.decode(&data).unwrap();
    assert_eq!(result.content.lines().count(), 2);
    let lines: Vec<&str> = result.content.lines().collect();
    assert!(lines[0].starts_with("00000000"));
    assert!(lines[1].starts_with("00000010"));
}

#[test]
fn non_printable_shown_as_dot() {
    let codec = HexCodec::new();
    let data = [0x00, 0x01, 0x7F, 0xFF, b'A'];
    let result = codec.decode(&data).unwrap();
    // Non-printable bytes: 0x00, 0x01, 0x7F, 0xFF → dots; 'A' → 'A'
    assert!(result.content.contains("|....A"));
}

#[test]
fn offset_increments() {
    let codec = HexCodec::new();
    let data = vec![0x00; 48]; // 3 lines
    let result = codec.decode(&data).unwrap();
    let lines: Vec<&str> = result.content.lines().collect();
    assert_eq!(lines.len(), 3);
    assert!(lines[0].starts_with("00000000"));
    assert!(lines[1].starts_with("00000010"));
    assert!(lines[2].starts_with("00000020"));
}

#[test]
fn encode_returns_none() {
    let codec = HexCodec::new();
    let metadata = CodecMetadata::new(ContentType::new(ContentType::BINARY_RAW));
    assert!(codec.encode("any content", &metadata).is_none());
}

#[test]
fn translate_edit_bytes_variant_replaces_canonical_byte() {
    let codec = HexCodec::new();
    let bytes = HeapByteSource::new(b"hello");
    let edit = DecodedEdit::Bytes {
        offset: 1,
        old_len: 1,
        new_bytes: vec![b'x'],
    };

    let translated = codec.translate_edit(&bytes, &edit).unwrap();

    assert_eq!(translated.offset, 1);
    assert_eq!(translated.old_bytes, vec![b'e']);
    assert_eq!(translated.new_bytes, vec![b'x']);
}

#[test]
fn translate_edit_text_variant_is_not_supported() {
    let codec = HexCodec::new();
    let bytes = HeapByteSource::new(b"A");
    let edit = DecodedEdit::Text {
        start: reovim_types_text::Position::new(0, 0),
        end: reovim_types_text::Position::new(0, 2),
        replacement: "58".to_string(),
    };

    assert!(codec.translate_edit(&bytes, &edit).is_none());
}

#[test]
fn metadata_is_readonly() {
    let codec = HexCodec::new();
    let result = codec.decode(b"hello").unwrap();
    assert_eq!(result.metadata.get("readonly"), Some("true"));
    assert_eq!(result.metadata.content_type().as_str(), ContentType::BINARY_RAW);
}

#[test]
fn hello_world_format() {
    let codec = HexCodec::new();
    let result = codec.decode(b"Hello World\n").unwrap();
    let line = result.content.lines().next().unwrap();
    // Verify hex bytes
    assert!(line.contains("48 65 6c 6c 6f 20 57 6f  72 6c 64 0a"));
    // Verify ASCII sidebar (0x0a shows as '.')
    assert!(line.contains("|Hello World."));
}

#[test]
fn space_is_printable_in_ascii() {
    let codec = HexCodec::new();
    let result = codec.decode(b" ").unwrap();
    assert!(result.content.contains("| "));
}

#[test]
fn large_offset_formatting() {
    let codec = HexCodec::new();
    let data = vec![b'A'; 256]; // 16 lines
    let result = codec.decode(&data).unwrap();
    let lines: Vec<&str> = result.content.lines().collect();
    assert_eq!(lines.len(), 16);
    assert!(lines[15].starts_with("000000f0"));
}

#[test]
fn default_impl() {
    let codec = HexCodec;
    let result = codec.decode(b"x").unwrap();
    assert!(result.lossy);
}

#[test]
fn partial_line_padding() {
    let codec = HexCodec::new();
    let result = codec.decode(&[0x41, 0x42]).unwrap(); // "AB"
    let line = result.content.lines().next().unwrap();
    // Should have padding spaces for missing bytes
    assert!(line.contains("41 42 "));
    // ASCII sidebar should be padded
    assert!(line.contains("|AB"));
    assert!(line.ends_with('|'));
}

#[test]
fn format_hex_dump_empty() {
    let (content, annotations) = format_hex_dump(b"");
    assert_eq!(content, "");
    assert!(annotations.is_empty());
}

#[test]
fn all_printable_ascii() {
    let codec = HexCodec::new();
    let data = b"abcdefghijklmnop";
    let result = codec.decode(data).unwrap();
    assert!(result.content.contains("|abcdefghijklmnop|"));
}

#[test]
fn annotations_empty_input() {
    let codec = HexCodec::new();
    let result = codec.decode(b"").unwrap();
    assert!(result.annotations.is_empty());
}

#[test]
fn annotations_single_line_three_kinds() {
    let codec = HexCodec::new();
    let result = codec.decode(b"A").unwrap();
    assert_eq!(result.annotations.len(), 3);
    assert_eq!(result.annotations[0].kind.name(), HEX_ADDRESS_KIND);
    assert_eq!(result.annotations[1].kind.name(), HEX_BYTE_KIND);
    assert_eq!(result.annotations[2].kind.name(), HEX_ASCII_KIND);
}

#[test]
fn annotations_target_correct_lines() {
    let codec = HexCodec::new();
    let data = vec![0x00; 48]; // 3 lines
    let result = codec.decode(&data).unwrap();
    assert_eq!(result.annotations.len(), 9); // 3 lines * 3 annotations

    // Line 0 annotations
    assert!(result.annotations[0].target.affects_line(0));
    assert!(result.annotations[1].target.affects_line(0));
    assert!(result.annotations[2].target.affects_line(0));

    // Line 1 annotations
    assert!(result.annotations[3].target.affects_line(1));
    assert!(result.annotations[4].target.affects_line(1));
    assert!(result.annotations[5].target.affects_line(1));

    // Line 2 annotations
    assert!(result.annotations[6].target.affects_line(2));
    assert!(result.annotations[7].target.affects_line(2));
    assert!(result.annotations[8].target.affects_line(2));
}

#[test]
fn annotations_all_have_none_payload() {
    let codec = HexCodec::new();
    let result = codec.decode(b"AB").unwrap();
    for annotation in &result.annotations {
        assert!(annotation.payload.is_none());
    }
}

#[test]
fn annotations_namespace_is_content() {
    let codec = HexCodec::new();
    let result = codec.decode(b"A").unwrap();
    for annotation in &result.annotations {
        assert_eq!(annotation.kind.namespace(), Some("content"));
    }
}

#[test]
fn small_input_not_truncated() {
    let codec = HexCodec::new();
    let result = codec.decode(b"hello").unwrap();
    assert!(!result.truncated);
    assert!(!result.content.contains("Truncated"));
}

#[test]
fn exactly_max_bytes_not_truncated() {
    let codec = HexCodec::new();
    let data = vec![0xAB; MAX_HEX_INPUT_BYTES];
    let result = codec.decode(&data).unwrap();
    assert!(!result.truncated);
    assert!(!result.content.contains("Truncated"));
    assert!(result.metadata.get("truncated_at").is_none());
}

#[test]
fn one_over_max_truncated() {
    let codec = HexCodec::new();
    let data = vec![0xCD; MAX_HEX_INPUT_BYTES + 1];
    let result = codec.decode(&data).unwrap();
    assert!(result.truncated);
    assert!(result.content.contains("Truncated"));
    assert!(result.content.contains("1 bytes omitted"));
    assert_eq!(
        result.metadata.get("truncated_at"),
        Some(MAX_HEX_INPUT_BYTES.to_string()).as_deref()
    );
    assert_eq!(
        result.metadata.get("total_size"),
        Some((MAX_HEX_INPUT_BYTES + 1).to_string()).as_deref()
    );
}

#[test]
fn truncated_footer_format() {
    let codec = HexCodec::new();
    let total = MAX_HEX_INPUT_BYTES + 500_000;
    let data = vec![0xFF; total];
    let result = codec.decode(&data).unwrap();
    assert!(result.truncated);
    let expected_footer = format!(
        "--- Truncated: showing {} of {} bytes ({} bytes omitted) ---",
        MAX_HEX_INPUT_BYTES, total, 500_000,
    );
    assert!(result.content.contains(&expected_footer));
}
