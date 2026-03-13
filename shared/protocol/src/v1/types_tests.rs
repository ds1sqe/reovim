use super::*;

#[test]
fn test_position_serialization() {
    let pos = Position::new(10, 5);
    let json = serde_json::to_string(&pos).unwrap();
    assert_eq!(json, r#"{"line":10,"column":5}"#);
}

#[test]
fn test_selection_mode_serialization() {
    assert_eq!(serde_json::to_string(&SelectionMode::Character).unwrap(), "\"character\"");
    assert_eq!(serde_json::to_string(&SelectionMode::Line).unwrap(), "\"line\"");
    assert_eq!(serde_json::to_string(&SelectionMode::Block).unwrap(), "\"block\"");
}

#[test]
fn test_screen_format_serialization() {
    assert_eq!(serde_json::to_string(&ScreenFormat::RawAnsi).unwrap(), "\"raw_ansi\"");
    assert_eq!(serde_json::to_string(&ScreenFormat::PlainText).unwrap(), "\"plain_text\"");
    assert_eq!(serde_json::to_string(&ScreenFormat::CellGrid).unwrap(), "\"cell_grid\"");
}

#[test]
fn test_buffer_info_serialization() {
    let buffer = BufferInfo {
        id: 1,
        file_path: Some("/tmp/test.txt".to_string()),
        modified: true,
        line_count: 100,
        content_type: None,
        readonly: None,
        codec_metadata: None,
    };
    let json = serde_json::to_string(&buffer).unwrap();
    assert!(json.contains("\"id\":1"));
    assert!(json.contains("\"file_path\":\"/tmp/test.txt\""));
    assert!(json.contains("\"modified\":true"));
    // Codec fields should be absent when None (skip_serializing_if)
    assert!(!json.contains("content_type"));
    assert!(!json.contains("readonly"));
    assert!(!json.contains("codec_metadata"));
}

#[test]
fn test_cell_info_skips_defaults() {
    let cell = CellInfo {
        char: 'a',
        fg: Some("#ff0000".to_string()),
        bg: None,
        bold: false,
        italic: false,
        underline: false,
    };
    let json = serde_json::to_string(&cell).unwrap();
    // Should skip false booleans and None
    assert!(!json.contains("\"bold\""));
    assert!(!json.contains("\"bg\""));
    assert!(json.contains("\"fg\":\"#ff0000\""));
}

#[test]
fn test_wire_window_id_serialization() {
    let wire = WireWindowId(42);

    // Verify transparent serialization (just "42", not {"0": 42})
    let json = serde_json::to_string(&wire).unwrap();
    assert_eq!(json, "42");

    // Verify deserialization
    let parsed: WireWindowId = serde_json::from_str("42").unwrap();
    assert_eq!(parsed.0, 42);

    // Verify usize roundtrip
    let from_usize = WireWindowId::from(42_usize);
    let back: usize = from_usize.into();
    assert_eq!(back, 42);
}

// Layout types tests (#444)

#[test]
fn test_wire_layer_id_serialization() {
    let layer_id = WireLayerId(5);
    let json = serde_json::to_string(&layer_id).unwrap();
    assert_eq!(json, "5");

    let parsed: WireLayerId = serde_json::from_str("5").unwrap();
    assert_eq!(parsed.0, 5);
}

#[test]
fn test_wire_rect_serialization() {
    let rect = WireRect::new(10, 20, 80, 24);
    let json = serde_json::to_string(&rect).unwrap();
    assert!(json.contains("\"x\":10"));
    assert!(json.contains("\"y\":20"));
    assert!(json.contains("\"width\":80"));
    assert!(json.contains("\"height\":24"));

    let parsed: WireRect = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, rect);
}

#[test]
fn test_wire_rect_full_screen() {
    let rect = WireRect::full_screen(120, 40);
    assert_eq!(rect.x, 0);
    assert_eq!(rect.y, 0);
    assert_eq!(rect.width, 120);
    assert_eq!(rect.height, 40);
}

#[test]
fn test_wire_zone_serialization() {
    assert_eq!(serde_json::to_string(&WireZone::Tiled).unwrap(), "\"tiled\"");
    assert_eq!(serde_json::to_string(&WireZone::Float).unwrap(), "\"float\"");
    assert_eq!(serde_json::to_string(&WireZone::Overlay).unwrap(), "\"overlay\"");
}

#[test]
fn test_wire_split_direction_serialization() {
    assert_eq!(
        serde_json::to_string(&WireSplitDirection::Horizontal).unwrap(),
        "\"horizontal\""
    );
    assert_eq!(serde_json::to_string(&WireSplitDirection::Vertical).unwrap(), "\"vertical\"");
}

#[test]
fn test_wire_layout_change_kind_split() {
    let kind = WireLayoutChangeKind::Split {
        new_window: WireWindowId(2),
        direction: WireSplitDirection::Vertical,
    };
    let json = serde_json::to_string(&kind).unwrap();
    assert!(json.contains("\"type\":\"split\""));
    assert!(json.contains("\"new_window\":2"));
    assert!(json.contains("\"direction\":\"vertical\""));
}

#[test]
fn test_wire_layout_change_kind_close() {
    let kind = WireLayoutChangeKind::Close {
        closed_window: WireWindowId(1),
        new_focus: Some(WireWindowId(0)),
    };
    let json = serde_json::to_string(&kind).unwrap();
    assert!(json.contains("\"type\":\"close\""));
    assert!(json.contains("\"closed_window\":1"));
    assert!(json.contains("\"new_focus\":0"));
}

#[test]
fn test_wire_layout_change_kind_focus() {
    let kind = WireLayoutChangeKind::Focus {
        from: Some(WireWindowId(0)),
        to: WireWindowId(1),
    };
    let json = serde_json::to_string(&kind).unwrap();
    assert!(json.contains("\"type\":\"focus\""));
    assert!(json.contains("\"from\":0"));
    assert!(json.contains("\"to\":1"));
}

#[test]
fn test_wire_layout_change_kind_resize() {
    let kind = WireLayoutChangeKind::Resize {
        window: WireWindowId(1),
    };
    let json = serde_json::to_string(&kind).unwrap();
    assert!(json.contains("\"type\":\"resize\""));
    assert!(json.contains("\"window\":1"));
}

#[test]
fn test_wire_layout_change_kind_equalize() {
    let kind = WireLayoutChangeKind::Equalize;
    let json = serde_json::to_string(&kind).unwrap();
    assert!(json.contains("\"type\":\"equalize\""));
}

#[test]
fn test_wire_window_placement_serialization() {
    let placement = WireWindowPlacement {
        window_id: WireWindowId(1),
        layer_id: WireLayerId(0),
        zone: WireZone::Tiled,
        bounds: WireRect::new(0, 0, 40, 24),
        z_order: 100,
        visible: true,
        focusable: true,
        buffer_id: Some(BufferId(0)),
    };
    let json = serde_json::to_string(&placement).unwrap();
    assert!(json.contains("\"window_id\":1"));
    assert!(json.contains("\"layer_id\":0"));
    assert!(json.contains("\"zone\":\"tiled\""));
    assert!(json.contains("\"z_order\":100"));
    assert!(json.contains("\"visible\":true"));
    assert!(json.contains("\"buffer_id\":0"));

    let parsed: WireWindowPlacement = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, placement);
}

#[test]
fn test_wire_window_placement_no_buffer() {
    let placement = WireWindowPlacement {
        window_id: WireWindowId(1),
        layer_id: WireLayerId(0),
        zone: WireZone::Float,
        bounds: WireRect::new(10, 10, 30, 20),
        z_order: 200,
        visible: true,
        focusable: true,
        buffer_id: None,
    };
    let json = serde_json::to_string(&placement).unwrap();
    // buffer_id should be skipped when None
    assert!(!json.contains("\"buffer_id\""));
}

#[test]
fn test_wire_layout_info_empty() {
    let layout = WireLayoutInfo::default();
    assert_eq!(layout.window_count, 0);
    assert!(layout.windows.is_empty());
    assert!(layout.focused_window.is_none());
}

#[test]
fn test_wire_layout_info_single_window() {
    let layout = WireLayoutInfo::single_window(80, 24, Some(BufferId(0)));
    assert_eq!(layout.window_count, 1);
    assert_eq!(layout.windows.len(), 1);
    assert!(layout.focused_window.is_some());
    assert!(!layout.is_multi_window());
    assert!(layout.focused().is_some());
}

#[test]
fn test_wire_layout_info_multi_window() {
    let layout = WireLayoutInfo {
        screen: WireRect::full_screen(80, 24),
        windows: vec![
            WireWindowPlacement {
                window_id: WireWindowId(0),
                layer_id: WireLayerId(0),
                zone: WireZone::Tiled,
                bounds: WireRect::new(0, 0, 40, 24),
                z_order: 100,
                visible: true,
                focusable: true,
                buffer_id: Some(BufferId(0)),
            },
            WireWindowPlacement {
                window_id: WireWindowId(1),
                layer_id: WireLayerId(0),
                zone: WireZone::Tiled,
                bounds: WireRect::new(40, 0, 40, 24),
                z_order: 100,
                visible: true,
                focusable: true,
                buffer_id: Some(BufferId(0)),
            },
        ],
        focused_window: Some(WireWindowId(1)),
        active_layer: Some(WireLayerId(0)),
        window_count: 2,
    };
    assert!(layout.is_multi_window());
    let focused = layout.focused().unwrap();
    assert_eq!(focused.window_id, WireWindowId(1));
}

#[test]
fn test_wire_layout_info_serialization_roundtrip() {
    let layout = WireLayoutInfo::single_window(120, 40, Some(BufferId(1)));
    let json = serde_json::to_string(&layout).unwrap();
    let parsed: WireLayoutInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, layout);
}

// Additional coverage tests

#[test]
fn test_position_default() {
    let pos = Position::default();
    assert_eq!(pos.line, 0);
    assert_eq!(pos.column, 0);
}

#[test]
fn test_position_deserialization() {
    let json = r#"{"line":42,"column":7}"#;
    let pos: Position = serde_json::from_str(json).unwrap();
    assert_eq!(pos, Position::new(42, 7));
}

#[test]
fn test_buffer_id_from_into() {
    let id = BufferId::from(42_usize);
    let back: usize = id.into();
    assert_eq!(back, 42);
    assert_eq!(id.0, 42);
}

#[test]
fn test_buffer_id_hash() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(BufferId(1));
    set.insert(BufferId(2));
    set.insert(BufferId(1));
    assert_eq!(set.len(), 2);
}

#[test]
fn test_wire_window_id_from_usize() {
    let id = WireWindowId::from(10_usize);
    assert_eq!(id.0, 10);
    let back: usize = id.into();
    assert_eq!(back, 10);
}

#[test]
fn test_wire_layer_id_from_usize() {
    let id = WireLayerId::from(5_usize);
    assert_eq!(id.0, 5);
    let back: usize = id.into();
    assert_eq!(back, 5);
}

#[test]
fn test_wire_rect_default() {
    let rect = WireRect::default();
    assert_eq!(rect.x, 0);
    assert_eq!(rect.y, 0);
    assert_eq!(rect.width, 0);
    assert_eq!(rect.height, 0);
}

#[test]
fn test_wire_zone_default() {
    let zone = WireZone::default();
    assert!(matches!(zone, WireZone::Tiled));
}

#[test]
fn test_selection_mode_default() {
    let mode = SelectionMode::default();
    assert!(matches!(mode, SelectionMode::Character));
}

#[test]
fn test_screen_format_default() {
    let format = ScreenFormat::default();
    assert!(matches!(format, ScreenFormat::PlainText));
}

#[test]
fn test_cursor_info_default() {
    let info = CursorInfo::default();
    assert_eq!(info.line, 0);
    assert_eq!(info.column, 0);
}

#[test]
fn test_cursor_info_from_position() {
    let pos = Position::new(10, 5);
    let info = CursorInfo::from(pos);
    assert_eq!(info.line, 10);
    assert_eq!(info.column, 5);
}

#[test]
fn test_position_from_cursor_info() {
    let info = CursorInfo {
        line: 20,
        column: 15,
    };
    let pos = Position::from(info);
    assert_eq!(pos.line, 20);
    assert_eq!(pos.column, 15);
}

#[test]
fn test_cursor_info_position_roundtrip() {
    let original = Position::new(99, 42);
    let cursor = CursorInfo::from(original);
    let back = Position::from(cursor);
    assert_eq!(original, back);
}

#[test]
fn test_selection_info_default() {
    let info = SelectionInfo::default();
    assert!(!info.active);
    assert!(matches!(info.mode, SelectionMode::Character));
    assert_eq!(info.anchor, Position::default());
    assert_eq!(info.cursor, Position::default());
}

#[test]
fn test_selection_info_serialization() {
    let info = SelectionInfo {
        active: true,
        mode: SelectionMode::Line,
        anchor: Position::new(1, 0),
        cursor: Position::new(5, 0),
    };
    let json = serde_json::to_string(&info).unwrap();
    assert!(json.contains("\"active\":true"));
    assert!(json.contains("\"mode\":\"line\""));
    let decoded: SelectionInfo = serde_json::from_str(&json).unwrap();
    assert!(decoded.active);
    assert_eq!(decoded.anchor, Position::new(1, 0));
}

#[test]
fn test_mode_info_default() {
    let info = ModeInfo::default();
    assert!(info.focus.is_empty());
    assert!(info.edit_mode.is_empty());
    assert!(info.sub_mode.is_empty());
    assert!(info.display.is_empty());
}

#[test]
fn test_mode_info_serialization() {
    let info = ModeInfo {
        focus: "Editor".to_string(),
        edit_mode: "Normal".to_string(),
        sub_mode: "None".to_string(),
        display: "NORMAL".to_string(),
    };
    let json = serde_json::to_string(&info).unwrap();
    assert!(json.contains("\"focus\":\"Editor\""));
    assert!(json.contains("\"edit_mode\":\"Normal\""));
    let decoded: ModeInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.display, "NORMAL");
}

#[test]
fn test_cell_info_default() {
    let cell = CellInfo::default();
    assert_eq!(cell.char, ' ');
    assert!(cell.fg.is_none());
    assert!(cell.bg.is_none());
    assert!(!cell.bold);
    assert!(!cell.italic);
    assert!(!cell.underline);
}

#[test]
fn test_cell_info_with_all_attributes() {
    let cell = CellInfo {
        char: 'X',
        fg: Some("#ff0000".to_string()),
        bg: Some("#0000ff".to_string()),
        bold: true,
        italic: true,
        underline: true,
    };
    let json = serde_json::to_string(&cell).unwrap();
    assert!(json.contains("\"bold\":true"));
    assert!(json.contains("\"italic\":true"));
    assert!(json.contains("\"underline\":true"));
    assert!(json.contains("\"fg\":\"#ff0000\""));
    assert!(json.contains("\"bg\":\"#0000ff\""));

    let decoded: CellInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.char, 'X');
    assert!(decoded.bold);
}

#[test]
fn test_buffer_info_without_file_path() {
    let buffer = BufferInfo {
        id: 1,
        file_path: None,
        modified: false,
        line_count: 0,
        content_type: None,
        readonly: None,
        codec_metadata: None,
    };
    let json = serde_json::to_string(&buffer).unwrap();
    assert!(!json.contains("file_path"));
}

#[test]
fn test_screen_info_serialization() {
    let info = ScreenInfo {
        width: 80,
        height: 24,
        active_buffer_id: BufferId(0),
        active_window_id: Some(WireWindowId(1)),
        window_count: 2,
    };
    let json = serde_json::to_string(&info).unwrap();
    assert!(json.contains("\"width\":80"));
    assert!(json.contains("\"active_window_id\":1"));

    let decoded: ScreenInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.window_count, 2);
}

#[test]
fn test_screen_info_without_active_window() {
    let info = ScreenInfo {
        width: 80,
        height: 24,
        active_buffer_id: BufferId(0),
        active_window_id: None,
        window_count: 0,
    };
    let json = serde_json::to_string(&info).unwrap();
    assert!(!json.contains("active_window_id"));
}

#[test]
fn test_window_info_serialization() {
    let info = WindowInfo {
        id: WireWindowId(0),
        buffer_id: BufferId(1),
        is_active: false,
        cursor: Position::new(0, 0),
    };
    let json = serde_json::to_string(&info).unwrap();
    assert!(json.contains("\"id\":0"));
    assert!(json.contains("\"buffer_id\":1"));
    assert!(json.contains("\"is_active\":false"));

    let decoded: WindowInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.id, WireWindowId(0));
}

#[test]
fn test_wire_layout_info_focused_none() {
    let layout = WireLayoutInfo::default();
    assert!(layout.focused().is_none());
}

#[test]
fn test_wire_layout_info_focused_missing_window() {
    let layout = WireLayoutInfo {
        focused_window: Some(WireWindowId(999)),
        windows: vec![],
        ..WireLayoutInfo::default()
    };
    // focused_window points to non-existent window
    assert!(layout.focused().is_none());
}

#[test]
fn test_wire_layout_info_single_window_no_buffer() {
    let layout = WireLayoutInfo::single_window(80, 24, None);
    assert_eq!(layout.windows.len(), 1);
    assert!(layout.windows[0].buffer_id.is_none());
}

#[test]
fn test_selection_mode_deserialization() {
    let block: SelectionMode = serde_json::from_str("\"block\"").unwrap();
    assert_eq!(block, SelectionMode::Block);
}

#[test]
fn test_screen_format_deserialization() {
    let format: ScreenFormat = serde_json::from_str("\"cell_grid\"").unwrap();
    assert_eq!(format, ScreenFormat::CellGrid);
}

#[test]
fn test_wire_layout_change_kind_close_no_focus() {
    let kind = WireLayoutChangeKind::Close {
        closed_window: WireWindowId(1),
        new_focus: None,
    };
    let json = serde_json::to_string(&kind).unwrap();
    assert!(json.contains("\"type\":\"close\""));
    assert!(json.contains("\"new_focus\":null"));
}

#[test]
fn test_wire_layout_change_kind_focus_no_from() {
    let kind = WireLayoutChangeKind::Focus {
        from: None,
        to: WireWindowId(0),
    };
    let json = serde_json::to_string(&kind).unwrap();
    assert!(json.contains("\"from\":null"));
    assert!(json.contains("\"to\":0"));
}

#[test]
fn test_buffer_info_with_codec_metadata() {
    let buffer = BufferInfo {
        id: 1,
        file_path: Some("test.txt".to_string()),
        modified: false,
        line_count: 42,
        content_type: Some("text/utf-8".to_string()),
        readonly: Some(false),
        codec_metadata: Some(CodecMetadataWire {
            codec_name: "utf-8".to_string(),
            line_ending: Some("crlf".to_string()),
            has_bom: true,
        }),
    };
    let json = serde_json::to_string(&buffer).unwrap();
    assert!(json.contains("\"content_type\":\"text/utf-8\""));
    assert!(json.contains("\"codec_name\":\"utf-8\""));
    assert!(json.contains("\"line_ending\":\"crlf\""));
    assert!(json.contains("\"has_bom\":true"));

    // Round-trip deserialization
    let decoded: BufferInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.content_type.as_deref(), Some("text/utf-8"));
    assert_eq!(decoded.readonly, Some(false));
    let meta = decoded.codec_metadata.unwrap();
    assert_eq!(meta.codec_name, "utf-8");
    assert_eq!(meta.line_ending.as_deref(), Some("crlf"));
    assert!(meta.has_bom);
}

#[test]
fn test_buffer_info_codec_fields_absent_when_none() {
    let buffer = BufferInfo {
        id: 1,
        file_path: None,
        modified: false,
        line_count: 0,
        content_type: None,
        readonly: None,
        codec_metadata: None,
    };
    let json = serde_json::to_string(&buffer).unwrap();
    // Codec fields should be omitted entirely
    assert!(!json.contains("content_type"));
    assert!(!json.contains("readonly"));
    assert!(!json.contains("codec_metadata"));
}

#[test]
fn test_buffer_info_deserialize_without_codec_fields() {
    // Legacy JSON without codec fields should deserialize with defaults
    let json = r#"{"id":1,"modified":false,"line_count":10}"#;
    let buffer: BufferInfo = serde_json::from_str(json).unwrap();
    assert_eq!(buffer.id, 1);
    assert!(buffer.content_type.is_none());
    assert!(buffer.readonly.is_none());
    assert!(buffer.codec_metadata.is_none());
}

#[test]
fn test_codec_metadata_wire_without_bom() {
    let meta = CodecMetadataWire {
        codec_name: "utf-8".to_string(),
        line_ending: Some("lf".to_string()),
        has_bom: false,
    };
    let json = serde_json::to_string(&meta).unwrap();
    // has_bom should be skipped when false
    assert!(!json.contains("has_bom"));
    assert!(json.contains("\"codec_name\":\"utf-8\""));
    assert!(json.contains("\"line_ending\":\"lf\""));
}

#[test]
fn test_codec_metadata_wire_without_line_ending() {
    let meta = CodecMetadataWire {
        codec_name: "hex".to_string(),
        line_ending: None,
        has_bom: false,
    };
    let json = serde_json::to_string(&meta).unwrap();
    assert!(!json.contains("line_ending"));
    assert!(!json.contains("has_bom"));
    assert!(json.contains("\"codec_name\":\"hex\""));
}

#[test]
fn test_codec_metadata_wire_equality() {
    let a = CodecMetadataWire {
        codec_name: "utf-8".to_string(),
        line_ending: Some("lf".to_string()),
        has_bom: true,
    };
    let b = a.clone();
    assert_eq!(a, b);
}

#[test]
fn test_codec_metadata_wire_debug() {
    let meta = CodecMetadataWire {
        codec_name: "utf-8".to_string(),
        line_ending: None,
        has_bom: false,
    };
    let debug = format!("{meta:?}");
    assert!(debug.contains("CodecMetadataWire"));
    assert!(debug.contains("utf-8"));
}
