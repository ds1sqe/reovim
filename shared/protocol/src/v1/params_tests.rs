use super::*;

#[test]
fn test_input_keys_params() {
    let params = InputKeysParams {
        keys: "iHello<Esc>".to_string(),
    };
    let json = serde_json::to_string(&params).unwrap();
    assert!(json.contains("\"keys\":\"iHello<Esc>\""));
}

#[test]
fn test_state_cursor_params_default() {
    let params = StateCursorParams::default();
    let json = serde_json::to_string(&params).unwrap();
    // Should be empty object since buffer_id is None
    assert_eq!(json, "{}");
}

#[test]
fn test_screen_content_params_with_format() {
    let params = StateScreenContentParams {
        format: ScreenFormat::CellGrid,
    };
    let json = serde_json::to_string(&params).unwrap();
    assert!(json.contains("\"format\":\"cell_grid\""));
}

#[test]
fn test_buffer_set_content_params() {
    let params = BufferSetContentParams {
        content: "Hello, World!".to_string(),
        buffer_id: Some(1),
    };
    let json = serde_json::to_string(&params).unwrap();
    assert!(json.contains("\"content\":\"Hello, World!\""));
    assert!(json.contains("\"buffer_id\":1"));
}

#[test]
fn test_editor_resize_params() {
    let params = EditorResizeParams {
        width: 120,
        height: 40,
    };
    let json = serde_json::to_string(&params).unwrap();
    assert!(json.contains("\"width\":120"));
    assert!(json.contains("\"height\":40"));
}

#[test]
fn test_module_load_params() {
    let params = ModuleLoadParams {
        path: "/usr/lib/reovim/modules/libexample.so".to_string(),
    };
    let json = serde_json::to_string(&params).unwrap();
    assert!(json.contains("\"path\":\"/usr/lib/reovim/modules/libexample.so\""));
}

#[test]
fn test_module_unload_params() {
    let params = ModuleUnloadParams {
        id: "example-module".to_string(),
    };
    let json = serde_json::to_string(&params).unwrap();
    assert!(json.contains("\"id\":\"example-module\""));
}

#[test]
fn test_module_reload_params() {
    let params = ModuleReloadParams {
        id: "hot-reload-demo".to_string(),
    };
    let json = serde_json::to_string(&params).unwrap();
    assert!(json.contains("\"id\":\"hot-reload-demo\""));
}

#[test]
fn test_state_layout_params_default() {
    let params = StateLayoutParams::default();
    let json = serde_json::to_string(&params).unwrap();
    // Empty struct serializes as empty object
    assert_eq!(json, "{}");
}

#[test]
fn test_state_layout_params_deserialize() {
    let params: StateLayoutParams = serde_json::from_str("{}").unwrap();
    // Should deserialize without error
    let _ = params;
}

#[test]
fn test_state_options_params_default() {
    let params = StateOptionsParams::default();
    let json = serde_json::to_string(&params).unwrap();
    // Empty struct serializes as empty object
    assert_eq!(json, "{}");
}

#[test]
fn test_state_options_params_with_window_id() {
    let params = StateOptionsParams {
        window_id: Some(42),
        names: None,
    };
    let json = serde_json::to_string(&params).unwrap();
    assert!(json.contains("\"window_id\":42"));
    assert!(!json.contains("\"names\""));
}

#[test]
fn test_state_options_params_with_names() {
    let params = StateOptionsParams {
        window_id: None,
        names: Some(vec!["number".to_string(), "relativenumber".to_string()]),
    };
    let json = serde_json::to_string(&params).unwrap();
    assert!(json.contains("\"names\""));
    assert!(json.contains("\"number\""));
    assert!(!json.contains("\"window_id\""));
}

// Additional coverage tests

#[test]
fn test_input_keys_params_deserialization() {
    let json = r#"{"keys":"dd"}"#;
    let params: InputKeysParams = serde_json::from_str(json).unwrap();
    assert_eq!(params.keys, "dd");
}

#[test]
fn test_command_execute_params_default_args() {
    let json = r#"{"cmd":"w"}"#;
    let params: CommandExecuteParams = serde_json::from_str(json).unwrap();
    assert_eq!(params.cmd, "w");
    assert!(params.args.is_empty());
}

#[test]
fn test_command_execute_params_with_args() {
    let params = CommandExecuteParams {
        cmd: "e".to_string(),
        args: vec!["/tmp/test.txt".to_string()],
    };
    let json = serde_json::to_string(&params).unwrap();
    assert!(json.contains("\"cmd\":\"e\""));
    assert!(json.contains("\"/tmp/test.txt\""));
}

#[test]
fn test_state_cursor_params_with_buffer() {
    let params = StateCursorParams {
        buffer_id: Some(42),
    };
    let json = serde_json::to_string(&params).unwrap();
    assert!(json.contains("\"buffer_id\":42"));
}

#[test]
fn test_state_selection_params_default() {
    let params = StateSelectionParams::default();
    let json = serde_json::to_string(&params).unwrap();
    assert_eq!(json, "{}");
}

#[test]
fn test_state_selection_params_with_buffer() {
    let params = StateSelectionParams { buffer_id: Some(5) };
    let json = serde_json::to_string(&params).unwrap();
    assert!(json.contains("\"buffer_id\":5"));
}

#[test]
fn test_state_screen_content_params_default() {
    let params = StateScreenContentParams::default();
    // Default format is PlainText
    assert_eq!(params.format, ScreenFormat::PlainText);
}

#[test]
fn test_buffer_get_content_params_default() {
    let params = BufferGetContentParams::default();
    let json = serde_json::to_string(&params).unwrap();
    assert_eq!(json, "{}");
}

#[test]
fn test_buffer_get_content_params_with_id() {
    let params = BufferGetContentParams { buffer_id: Some(3) };
    let json = serde_json::to_string(&params).unwrap();
    assert!(json.contains("\"buffer_id\":3"));
}

#[test]
fn test_buffer_set_content_params_without_buffer_id() {
    let params = BufferSetContentParams {
        content: "new content".to_string(),
        buffer_id: None,
    };
    let json = serde_json::to_string(&params).unwrap();
    assert!(!json.contains("\"buffer_id\""));
    assert!(json.contains("\"content\":\"new content\""));
}

#[test]
fn test_buffer_open_file_params_roundtrip() {
    let params = BufferOpenFileParams {
        path: "/home/user/file.rs".to_string(),
    };
    let json = serde_json::to_string(&params).unwrap();
    let decoded: BufferOpenFileParams = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.path, "/home/user/file.rs");
}

#[test]
fn test_editor_resize_params_roundtrip() {
    let params = EditorResizeParams {
        width: 200,
        height: 50,
    };
    let json = serde_json::to_string(&params).unwrap();
    let decoded: EditorResizeParams = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.width, 200);
    assert_eq!(decoded.height, 50);
}

#[test]
fn test_state_window_content_params_roundtrip() {
    use crate::v1::types::WireWindowId;
    let params = StateWindowContentParams {
        window_id: WireWindowId(5),
        format: ScreenFormat::RawAnsi,
    };
    let json = serde_json::to_string(&params).unwrap();
    let decoded: StateWindowContentParams = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.window_id, WireWindowId(5));
    assert_eq!(decoded.format, ScreenFormat::RawAnsi);
}

#[test]
fn test_state_options_params_with_both() {
    let params = StateOptionsParams {
        window_id: Some(1),
        names: Some(vec!["number".to_string()]),
    };
    let json = serde_json::to_string(&params).unwrap();
    assert!(json.contains("\"window_id\":1"));
    assert!(json.contains("\"names\""));
}
