//! Tests for the xxd codec.

use reovim_driver_codec::ContentCodec;

use super::*;

// --- format_xxd_dump tests ---

#[test]
fn empty_input() {
    let (content, annotations) = format_xxd_dump(&[], 16, 8);
    assert!(content.is_empty());
    assert!(annotations.is_empty());
}

#[test]
fn single_full_line() {
    let input: Vec<u8> = (0..16).collect();
    let (content, annotations) = format_xxd_dump(&input, 16, 8);

    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines.len(), 1);
    assert!(lines[0].starts_with("00000000  "));
    // 3 annotations per line
    assert_eq!(annotations.len(), 3);
}

#[test]
fn partial_line() {
    let input = b"Hello";
    let (content, annotations) = format_xxd_dump(input, 16, 8);

    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines.len(), 1);
    // Partial line should still have padding to align ASCII column
    assert!(lines[0].contains("|Hello"));
    assert_eq!(annotations.len(), 3);
}

#[test]
fn multiple_lines() {
    let input: Vec<u8> = (0..48).collect();
    let (content, annotations) = format_xxd_dump(&input, 16, 8);

    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines.len(), 3);
    assert!(lines[0].starts_with("00000000"));
    assert!(lines[1].starts_with("00000010"));
    assert!(lines[2].starts_with("00000020"));
    // 3 annotations per line * 3 lines
    assert_eq!(annotations.len(), 9);
}

#[test]
fn address_format() {
    let input = vec![0u8; 256];
    let (content, _) = format_xxd_dump(&input, 16, 8);

    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines.len(), 16);
    assert!(lines[0].starts_with("00000000"));
    assert!(lines[15].starts_with("000000f0"));
}

#[test]
fn ascii_printable_and_non_printable() {
    let input = b"Hello\x00\x01\xff";
    let (content, _) = format_xxd_dump(input, 16, 8);

    let line = content.lines().next().unwrap();
    // ASCII section should show: Hello...
    assert!(line.contains("|Hello..."));
}

#[test]
fn group_gap() {
    // With 16 bytes per line and group_size=8, there should be a gap after byte 8
    let input: Vec<u8> = (0..16).collect();
    let (content, _) = format_xxd_dump(&input, 16, 8);

    let line = content.lines().next().unwrap();
    // After the offset "00000000  ", the hex section has groups separated by extra space
    // Pattern: "XX XX XX XX XX XX XX XX  XX XX XX XX XX XX XX XX"
    let hex_section = &line[10..]; // skip "00000000  "
    // After 8 bytes (8*3=24 chars), there should be an extra space
    assert_eq!(&hex_section[24..25], " ");
}

#[test]
fn custom_bytes_per_line() {
    let input: Vec<u8> = (0..32).collect();
    let (content, _) = format_xxd_dump(&input, 8, 4);

    // 32 bytes / 8 per line = 4 lines
    assert_eq!(content.lines().count(), 4);
}

#[test]
fn custom_group_size() {
    let input: Vec<u8> = (0..16).collect();
    let (content, _) = format_xxd_dump(&input, 16, 4);

    let line = content.lines().next().unwrap();
    let hex_section = &line[10..];
    // With group_size=4: gap after every 4 bytes (4*3=12 chars)
    assert_eq!(&hex_section[12..13], " ");
}

#[test]
fn zero_group_size_no_gaps() {
    let input: Vec<u8> = (0..16).collect();
    let (content, _) = format_xxd_dump(&input, 16, 0);

    let line = content.lines().next().unwrap();
    let hex_section = &line[10..];
    // No double spaces in hex section (before the ASCII marker)
    let hex_end = hex_section.find(" |").unwrap();
    let hex_only = &hex_section[..hex_end];
    // Should be continuous "XX " pattern with no double spaces
    assert!(!hex_only.contains("  "));
}

// --- Annotation tests ---

#[test]
fn annotations_per_line() {
    let input = vec![0u8; 32]; // 2 lines
    let (_, annotations) = format_xxd_dump(&input, 16, 8);

    assert_eq!(annotations.len(), 6); // 3 per line * 2 lines

    // Line 0: address, byte, ascii
    assert_eq!(annotations[0].kind.name(), HEX_ADDRESS_KIND);
    assert_eq!(annotations[0].target, AnnotationTarget::Line(0));
    assert_eq!(annotations[1].kind.name(), HEX_BYTE_KIND);
    assert_eq!(annotations[2].kind.name(), HEX_ASCII_KIND);

    // Line 1
    assert_eq!(annotations[3].target, AnnotationTarget::Line(1));
}

// --- XxdCodec trait tests ---

#[test]
fn decode_produces_readonly_lossy() {
    let codec = XxdCodec::new();
    let result = codec.decode(b"test").unwrap();
    assert!(result.lossy);
    assert!(result.readonly);
    assert!(!result.truncated);
    assert!(result.is_valid());
}

#[test]
fn decode_empty() {
    let codec = XxdCodec::new();
    let result = codec.decode(b"").unwrap();
    assert!(result.content.is_empty());
    assert!(result.annotations.is_empty());
}

#[test]
fn views_returns_hex() {
    let codec = XxdCodec::new();
    let views = codec.views();
    assert_eq!(views.len(), 1);
    assert_eq!(views[0].name, "hex");
    assert_eq!(views[0].display, "Hex Dump");
}

#[test]
fn decode_view_hex_delegates() {
    let codec = XxdCodec::new();
    let direct = codec.decode(b"test").unwrap();
    let via_view = codec.decode_view(b"test", "hex").unwrap();
    assert_eq!(direct.content, via_view.content);
}

#[test]
fn decode_view_unknown_errors() {
    let codec = XxdCodec::new();
    let err = codec.decode_view(b"test", "nonexistent");
    assert!(err.is_err());
}

#[test]
fn truncation() {
    let codec = XxdCodec::new().with_max_bytes(32);
    let input = vec![0u8; 64];
    let result = codec.decode(&input).unwrap();
    assert!(result.truncated);
    assert!(result.content.contains("Truncated"));
    assert!(result.content.contains("32 of 64"));
}

#[test]
fn no_truncation_at_boundary() {
    let codec = XxdCodec::new().with_max_bytes(32);
    let input = vec![0u8; 32];
    let result = codec.decode(&input).unwrap();
    assert!(!result.truncated);
}

#[test]
fn builder_methods() {
    let codec = XxdCodec::new()
        .with_bytes_per_line(8)
        .with_group_size(4)
        .with_max_bytes(512);

    let input: Vec<u8> = (0..16).collect();
    let result = codec.decode(&input).unwrap();
    // 16 bytes / 8 per line = 2 lines
    assert_eq!(result.content.lines().count(), 2);
}

#[test]
fn content_type_is_binary_raw() {
    let codec = XxdCodec::new();
    let result = codec.decode(b"\x7fELF").unwrap();
    assert_eq!(result.metadata.content_type().as_str(), "binary/raw");
}

#[test]
fn trait_object_works() {
    let codec: Box<dyn ContentCodec> = Box::new(XxdCodec::new());
    let result = codec.decode(b"test").unwrap();
    assert!(result.lossy);
}
