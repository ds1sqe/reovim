use reovim_subsys_coordination::{
    Cursor, CursorCodec, CursorHeader, Position, PositionCodec, PositionHeader,
};

use super::*;

const TEST_DOMAIN_ID: u32 = 42;

// ============================================================================
// TextPosition tests
// ============================================================================

#[test]
fn test_position_new() {
    let pos = TextPosition::new(TEST_DOMAIN_ID, 10, 5);
    assert_eq!(pos.line(), 10);
    assert_eq!(pos.col(), 5);
}

#[test]
fn test_position_header() {
    let pos = TextPosition::new(TEST_DOMAIN_ID, 0, 0);
    assert_eq!(pos.header().domain_id(), TEST_DOMAIN_ID);
    assert_eq!(pos.header().inner_id(), TEXT_POSITION_INNER_ID);
    assert_eq!(pos.header().flags(), 0);
}

#[test]
fn test_position_content_encoding() {
    let pos = TextPosition::new(TEST_DOMAIN_ID, 100, 50);
    let content = pos.content();
    assert_eq!(content.len(), 16);

    let line = u64::from_le_bytes(content[0..8].try_into().unwrap());
    let col = u64::from_le_bytes(content[8..16].try_into().unwrap());
    assert_eq!(line, 100);
    assert_eq!(col, 50);
}

#[test]
fn test_position_display() {
    let pos = TextPosition::new(TEST_DOMAIN_ID, 42, 7);
    assert_eq!(pos.display(), "42:7");
}

#[test]
fn test_position_clone_box() {
    let pos = TextPosition::new(TEST_DOMAIN_ID, 10, 20);
    let cloned = pos.clone_box();
    assert_eq!(cloned.header(), pos.header());
    assert_eq!(cloned.content(), pos.content());
}

#[test]
fn test_position_encode_roundtrip() {
    let pos = TextPosition::new(TEST_DOMAIN_ID, 999, 42);
    let encoded = pos.encode();
    assert_eq!(encoded.len(), 8 + 16); // header + content

    // Header portion
    let header_bytes: [u8; 8] = encoded[0..8].try_into().unwrap();
    let decoded_header = PositionHeader::new(TEST_DOMAIN_ID, TEXT_POSITION_INNER_ID, 0);
    assert_eq!(&header_bytes, decoded_header.as_bytes());

    // Content portion
    let content = &encoded[8..];
    let (line, col) = TextPosition::decode_content(content).unwrap();
    assert_eq!(line, 999);
    assert_eq!(col, 42);
}

#[test]
fn test_position_equality_via_dyn() {
    let a: Box<dyn Position> = Box::new(TextPosition::new(TEST_DOMAIN_ID, 5, 10));
    let b: Box<dyn Position> = Box::new(TextPosition::new(TEST_DOMAIN_ID, 5, 10));
    let c: Box<dyn Position> = Box::new(TextPosition::new(TEST_DOMAIN_ID, 5, 11));

    assert_eq!(*a, *b);
    assert_ne!(*a, *c);
}

#[test]
fn test_position_same_domain() {
    let a = TextPosition::new(1, 0, 0);
    let b = TextPosition::new(1, 99, 99);
    let c = TextPosition::new(2, 0, 0);

    assert!(a.header().same_domain(b.header()));
    assert!(!a.header().same_domain(c.header()));
}

#[test]
fn test_position_from_cursor_position() {
    let cp = CursorPosition::new(42, 7);
    let pos = TextPosition::from((TEST_DOMAIN_ID, cp));
    assert_eq!(pos.line(), 42);
    assert_eq!(pos.col(), 7);
}

#[test]
fn test_position_zero() {
    let pos = TextPosition::new(TEST_DOMAIN_ID, 0, 0);
    assert_eq!(pos.line(), 0);
    assert_eq!(pos.col(), 0);
    assert_eq!(pos.display(), "0:0");
}

#[test]
fn test_position_large_values() {
    let pos = TextPosition::new(TEST_DOMAIN_ID, u64::MAX, u64::MAX);
    assert_eq!(pos.line(), u64::MAX);
    assert_eq!(pos.col(), u64::MAX);

    // Roundtrip through content bytes
    let (line, col) = TextPosition::decode_content(pos.content()).unwrap();
    assert_eq!(line, u64::MAX);
    assert_eq!(col, u64::MAX);
}

// ============================================================================
// TextCursor tests
// ============================================================================

#[test]
fn test_cursor_new_no_selection() {
    let cursor = TextCursor::new(TEST_DOMAIN_ID, 10, 5);
    assert_eq!(cursor.line(), 10);
    assert_eq!(cursor.col(), 5);
    assert!(!cursor.has_selection());
    assert!(cursor.selection().is_none());
}

#[test]
fn test_cursor_header_no_selection() {
    let cursor = TextCursor::new(TEST_DOMAIN_ID, 0, 0);
    assert_eq!(cursor.header().domain_id(), TEST_DOMAIN_ID);
    assert_eq!(cursor.header().inner_id(), TEXT_CURSOR_INNER_ID);
    assert_eq!(cursor.header().flags(), 0);
}

#[test]
fn test_cursor_with_char_selection() {
    let sel = SelectionData {
        start_line: 5,
        start_col: 0,
        mode: SelectionMode::Character,
    };
    let cursor = TextCursor::with_selection(TEST_DOMAIN_ID, 10, 20, sel);

    assert_eq!(cursor.line(), 10);
    assert_eq!(cursor.col(), 20);
    assert!(cursor.has_selection());

    let sel = cursor.selection().unwrap();
    assert_eq!(sel.start_line, 5);
    assert_eq!(sel.start_col, 0);
    assert_eq!(sel.mode, SelectionMode::Character);
}

#[test]
fn test_cursor_with_line_selection() {
    let sel = SelectionData {
        start_line: 0,
        start_col: 0,
        mode: SelectionMode::Line,
    };
    let cursor = TextCursor::with_selection(TEST_DOMAIN_ID, 3, 0, sel);
    let sel = cursor.selection().unwrap();
    assert_eq!(sel.mode, SelectionMode::Line);
}

#[test]
fn test_cursor_with_block_selection() {
    let sel = SelectionData {
        start_line: 1,
        start_col: 5,
        mode: SelectionMode::Block,
    };
    let cursor = TextCursor::with_selection(TEST_DOMAIN_ID, 4, 10, sel);
    let sel = cursor.selection().unwrap();
    assert_eq!(sel.mode, SelectionMode::Block);
}

#[test]
fn test_cursor_header_with_selection_flag() {
    let sel = SelectionData {
        start_line: 0,
        start_col: 0,
        mode: SelectionMode::Character,
    };
    let cursor = TextCursor::with_selection(TEST_DOMAIN_ID, 0, 0, sel);
    assert_eq!(cursor.header().flags() & FLAG_HAS_SELECTION, 1);
}

#[test]
fn test_cursor_content_length_no_selection() {
    let cursor = TextCursor::new(TEST_DOMAIN_ID, 0, 0);
    assert_eq!(cursor.content().len(), 16);
}

#[test]
fn test_cursor_content_length_with_selection() {
    let sel = SelectionData {
        start_line: 0,
        start_col: 0,
        mode: SelectionMode::Character,
    };
    let cursor = TextCursor::with_selection(TEST_DOMAIN_ID, 0, 0, sel);
    assert_eq!(cursor.content().len(), 33);
}

#[test]
fn test_cursor_display_no_selection() {
    let cursor = TextCursor::new(TEST_DOMAIN_ID, 42, 7);
    assert_eq!(cursor.display(), "42:7");
}

#[test]
fn test_cursor_display_with_selection() {
    let sel = SelectionData {
        start_line: 5,
        start_col: 3,
        mode: SelectionMode::Character,
    };
    let cursor = TextCursor::with_selection(TEST_DOMAIN_ID, 10, 20, sel);
    assert!(cursor.display().contains("10:20"));
    assert!(cursor.display().contains("sel=5:3"));
}

#[test]
fn test_cursor_clone_box() {
    let cursor = TextCursor::new(TEST_DOMAIN_ID, 10, 20);
    let cloned = cursor.clone_box();
    assert_eq!(cloned.header(), cursor.header());
    assert_eq!(cloned.content(), cursor.content());
}

#[test]
fn test_cursor_clone_box_with_selection() {
    let sel = SelectionData {
        start_line: 1,
        start_col: 2,
        mode: SelectionMode::Line,
    };
    let cursor = TextCursor::with_selection(TEST_DOMAIN_ID, 10, 20, sel);
    let cloned = cursor.clone_box();
    assert_eq!(cloned.header(), cursor.header());
    assert_eq!(cloned.content(), cursor.content());
}

#[test]
fn test_cursor_equality_via_dyn() {
    let a: Box<dyn Cursor> = Box::new(TextCursor::new(TEST_DOMAIN_ID, 5, 10));
    let b: Box<dyn Cursor> = Box::new(TextCursor::new(TEST_DOMAIN_ID, 5, 10));
    let c: Box<dyn Cursor> = Box::new(TextCursor::new(TEST_DOMAIN_ID, 5, 11));

    assert_eq!(*a, *b);
    assert_ne!(*a, *c);
}

#[test]
fn test_cursor_equality_selection_matters() {
    let no_sel: Box<dyn Cursor> = Box::new(TextCursor::new(TEST_DOMAIN_ID, 5, 10));
    let with_sel: Box<dyn Cursor> = Box::new(TextCursor::with_selection(
        TEST_DOMAIN_ID,
        5,
        10,
        SelectionData {
            start_line: 0,
            start_col: 0,
            mode: SelectionMode::Character,
        },
    ));
    // Different because content differs (16 vs 33 bytes) and flags differ
    assert_ne!(*no_sel, *with_sel);
}

#[test]
fn test_cursor_same_domain() {
    let a = TextCursor::new(1, 0, 0);
    let b = TextCursor::new(1, 99, 99);
    let c = TextCursor::new(2, 0, 0);

    assert!(a.header().same_domain(b.header()));
    assert!(!a.header().same_domain(c.header()));
}

#[test]
fn test_cursor_from_cursor_position() {
    let cp = CursorPosition::new(42, 7);
    let cursor = TextCursor::from((TEST_DOMAIN_ID, cp));
    assert_eq!(cursor.line(), 42);
    assert_eq!(cursor.col(), 7);
    assert!(!cursor.has_selection());
}

#[test]
fn test_cursor_encode_roundtrip_no_selection() {
    let cursor = TextCursor::new(TEST_DOMAIN_ID, 100, 50);
    let encoded = cursor.encode();
    assert_eq!(encoded.len(), 8 + 16); // header + content

    let content = &encoded[8..];
    let header = CursorHeader::new(TEST_DOMAIN_ID, TEXT_CURSOR_INNER_ID, 0);
    let (line, col, sel) = TextCursor::decode_content(&header, content).unwrap();
    assert_eq!(line, 100);
    assert_eq!(col, 50);
    assert!(sel.is_none());
}

#[test]
fn test_cursor_encode_roundtrip_with_selection() {
    let sel = SelectionData {
        start_line: 5,
        start_col: 3,
        mode: SelectionMode::Block,
    };
    let cursor = TextCursor::with_selection(TEST_DOMAIN_ID, 10, 20, sel);
    let encoded = cursor.encode();
    assert_eq!(encoded.len(), 8 + 33); // header + content

    let content = &encoded[8..];
    let header = CursorHeader::new(TEST_DOMAIN_ID, TEXT_CURSOR_INNER_ID, FLAG_HAS_SELECTION);
    let (line, col, sel) = TextCursor::decode_content(&header, content).unwrap();
    assert_eq!(line, 10);
    assert_eq!(col, 20);
    let sel = sel.unwrap();
    assert_eq!(sel.start_line, 5);
    assert_eq!(sel.start_col, 3);
    assert_eq!(sel.mode, SelectionMode::Block);
}

// ============================================================================
// Codec tests
// ============================================================================

#[test]
fn test_position_codec_decode() {
    let codec = TextPositionCodec::new(TEST_DOMAIN_ID);
    assert_eq!(codec.domain_id(), TEST_DOMAIN_ID);
    assert_eq!(codec.inner_id(), TEXT_POSITION_INNER_ID);

    let pos = TextPosition::new(TEST_DOMAIN_ID, 42, 7);
    let header = PositionHeader::new(TEST_DOMAIN_ID, TEXT_POSITION_INNER_ID, 0);
    let decoded = codec.decode(&header, pos.content()).unwrap();

    assert_eq!(decoded.header(), pos.header());
    assert_eq!(decoded.content(), pos.content());
    assert_eq!(decoded.display(), "42:7");
}

#[test]
fn test_position_codec_decode_invalid_short() {
    let codec = TextPositionCodec::new(TEST_DOMAIN_ID);
    let header = PositionHeader::new(TEST_DOMAIN_ID, TEXT_POSITION_INNER_ID, 0);
    assert!(codec.decode(&header, &[0; 15]).is_none()); // Too short
}

#[test]
fn test_cursor_codec_decode_no_selection() {
    let codec = TextCursorCodec::new(TEST_DOMAIN_ID);
    assert_eq!(codec.domain_id(), TEST_DOMAIN_ID);
    assert_eq!(codec.inner_id(), TEXT_CURSOR_INNER_ID);

    let cursor = TextCursor::new(TEST_DOMAIN_ID, 100, 50);
    let header = CursorHeader::new(TEST_DOMAIN_ID, TEXT_CURSOR_INNER_ID, 0);
    let decoded = codec.decode(&header, cursor.content()).unwrap();

    assert_eq!(decoded.header(), cursor.header());
    assert_eq!(decoded.content(), cursor.content());
}

#[test]
fn test_cursor_codec_decode_with_selection() {
    let codec = TextCursorCodec::new(TEST_DOMAIN_ID);

    let sel = SelectionData {
        start_line: 1,
        start_col: 2,
        mode: SelectionMode::Line,
    };
    let cursor = TextCursor::with_selection(TEST_DOMAIN_ID, 10, 20, sel);
    let header = CursorHeader::new(TEST_DOMAIN_ID, TEXT_CURSOR_INNER_ID, FLAG_HAS_SELECTION);
    let decoded = codec.decode(&header, cursor.content()).unwrap();

    assert_eq!(decoded.header(), cursor.header());
    assert_eq!(decoded.content(), cursor.content());
}

#[test]
fn test_cursor_codec_decode_invalid_short_no_selection() {
    let codec = TextCursorCodec::new(TEST_DOMAIN_ID);
    let header = CursorHeader::new(TEST_DOMAIN_ID, TEXT_CURSOR_INNER_ID, 0);
    assert!(codec.decode(&header, &[0; 15]).is_none());
}

#[test]
fn test_cursor_codec_decode_invalid_short_with_selection() {
    let codec = TextCursorCodec::new(TEST_DOMAIN_ID);
    let header = CursorHeader::new(TEST_DOMAIN_ID, TEXT_CURSOR_INNER_ID, FLAG_HAS_SELECTION);
    // 16 bytes is enough for no-selection but too short for with-selection (needs 33)
    assert!(codec.decode(&header, &[0; 16]).is_none());
}

#[test]
fn test_cursor_codec_decode_invalid_selection_mode() {
    let codec = TextCursorCodec::new(TEST_DOMAIN_ID);
    let header = CursorHeader::new(TEST_DOMAIN_ID, TEXT_CURSOR_INNER_ID, FLAG_HAS_SELECTION);

    // 33 bytes with invalid mode byte (255)
    let mut content = vec![0u8; 33];
    content[32] = 255; // Invalid selection mode
    assert!(codec.decode(&header, &content).is_none());
}

// ============================================================================
// Selection mode roundtrip
// ============================================================================

#[test]
fn test_selection_mode_roundtrip_character() {
    let sel = SelectionData {
        start_line: 0,
        start_col: 0,
        mode: SelectionMode::Character,
    };
    assert_eq!(sel.mode_byte(), 0);
    assert_eq!(SelectionData::mode_from_byte(0), Some(SelectionMode::Character));
}

#[test]
fn test_selection_mode_roundtrip_line() {
    let sel = SelectionData {
        start_line: 0,
        start_col: 0,
        mode: SelectionMode::Line,
    };
    assert_eq!(sel.mode_byte(), 1);
    assert_eq!(SelectionData::mode_from_byte(1), Some(SelectionMode::Line));
}

#[test]
fn test_selection_mode_roundtrip_block() {
    let sel = SelectionData {
        start_line: 0,
        start_col: 0,
        mode: SelectionMode::Block,
    };
    assert_eq!(sel.mode_byte(), 2);
    assert_eq!(SelectionData::mode_from_byte(2), Some(SelectionMode::Block));
}

#[test]
fn test_selection_mode_invalid() {
    assert!(SelectionData::mode_from_byte(3).is_none());
    assert!(SelectionData::mode_from_byte(255).is_none());
}

// ============================================================================
// Debug format
// ============================================================================

#[test]
fn test_position_debug() {
    let pos: Box<dyn Position> = Box::new(TextPosition::new(TEST_DOMAIN_ID, 5, 10));
    let debug = format!("{pos:?}");
    assert!(debug.contains("dyn Position"));
    assert!(debug.contains("5:10"));
}

#[test]
fn test_cursor_debug() {
    let cursor: Box<dyn Cursor> = Box::new(TextCursor::new(TEST_DOMAIN_ID, 5, 10));
    let debug = format!("{cursor:?}");
    assert!(debug.contains("dyn Cursor"));
    assert!(debug.contains("5:10"));
}
