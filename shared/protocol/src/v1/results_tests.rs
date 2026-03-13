use {super::*, crate::v1::types::Position};

#[test]
fn test_screen_content_result() {
    let result = ScreenContentResult {
        width: 80,
        height: 24,
        format: ScreenFormat::PlainText,
        content: "Hello, World!".to_string(),
    };
    let json = serde_json::to_string(&result).unwrap();
    assert!(json.contains("\"width\":80"));
    assert!(json.contains("\"format\":\"plain_text\""));
}

#[test]
fn test_buffer_list_result() {
    let result = BufferListResult {
        buffers: vec![
            BufferInfo {
                id: 1,
                file_path: Some("/tmp/a.txt".to_string()),
                modified: false,
                line_count: 10,
                content_type: None,
                readonly: None,
                codec_metadata: None,
            },
            BufferInfo {
                id: 2,
                file_path: None,
                modified: true,
                line_count: 5,
                content_type: None,
                readonly: None,
                codec_metadata: None,
            },
        ],
    };
    let json = serde_json::to_string(&result).unwrap();
    assert!(json.contains("\"buffers\""));
    assert!(json.contains("\"id\":1"));
    assert!(json.contains("\"id\":2"));
}

#[test]
fn test_windows_result() {
    use crate::v1::types::{BufferId, WireWindowId};
    let result = WindowsResult {
        windows: vec![WindowInfo {
            id: WireWindowId::from(1),
            buffer_id: BufferId::from(1),
            is_active: true,
            cursor: Position::new(5, 10),
        }],
    };
    let json = serde_json::to_string(&result).unwrap();
    assert!(json.contains("\"is_active\":true"));
}

#[test]
fn test_ok_result() {
    let result = OkResult::new();
    let json = serde_json::to_string(&result).unwrap();
    assert_eq!(json, r#"{"ok":true}"#);
}

#[test]
fn test_module_info() {
    let info = ModuleInfo {
        id: "example-module".to_string(),
        name: "Example Module".to_string(),
        version: "1.0.0".to_string(),
        state: "Running".to_string(),
        path: Some("/usr/lib/reovim/modules/libexample.so".to_string()),
        is_static: false,
        dependencies: vec!["core".to_string()],
    };
    let json = serde_json::to_string(&info).unwrap();
    assert!(json.contains("\"id\":\"example-module\""));
    assert!(json.contains("\"is_static\":false"));
    assert!(json.contains("\"dependencies\":[\"core\"]"));
}

#[test]
fn test_module_info_static_no_deps() {
    let info = ModuleInfo {
        id: "builtin-module".to_string(),
        name: "Builtin Module".to_string(),
        version: "0.9.0".to_string(),
        state: "Running".to_string(),
        path: None,
        is_static: true,
        dependencies: vec![],
    };
    let json = serde_json::to_string(&info).unwrap();
    assert!(json.contains("\"is_static\":true"));
    // path and dependencies should be skipped when empty/None
    assert!(!json.contains("\"path\""));
    assert!(!json.contains("\"dependencies\""));
}

#[test]
fn test_module_list_result() {
    let result = ModuleListResult {
        modules: vec![ModuleInfo {
            id: "module-a".to_string(),
            name: "Module A".to_string(),
            version: "1.0.0".to_string(),
            state: "Running".to_string(),
            path: None,
            is_static: true,
            dependencies: vec![],
        }],
    };
    let json = serde_json::to_string(&result).unwrap();
    assert!(json.contains("\"modules\""));
    assert!(json.contains("\"id\":\"module-a\""));
}

#[test]
fn test_module_load_result() {
    let result = ModuleLoadResult {
        module: ModuleInfo {
            id: "hot-reload-demo".to_string(),
            name: "Hot Reload Demo".to_string(),
            version: "0.9.0-dev".to_string(),
            state: "Loaded".to_string(),
            path: Some("/tmp/libhot_reload_demo.so".to_string()),
            is_static: false,
            dependencies: vec![],
        },
    };
    let json = serde_json::to_string(&result).unwrap();
    assert!(json.contains("\"module\""));
    assert!(json.contains("\"id\":\"hot-reload-demo\""));
}

#[test]
fn test_key_status_serialization() {
    assert_eq!(serde_json::to_string(&KeyStatus::Executed).unwrap(), "\"executed\"");
    assert_eq!(serde_json::to_string(&KeyStatus::Pending).unwrap(), "\"pending\"");
    assert_eq!(serde_json::to_string(&KeyStatus::NotFound).unwrap(), "\"not_found\"");
}

#[test]
fn test_input_keys_result_constructors() {
    let executed = InputKeysResult::executed();
    assert!(executed.ok);
    assert_eq!(executed.status, KeyStatus::Executed);

    let pending = InputKeysResult::pending();
    assert!(pending.ok);
    assert_eq!(pending.status, KeyStatus::Pending);

    let not_found = InputKeysResult::not_found();
    assert!(not_found.ok);
    assert_eq!(not_found.status, KeyStatus::NotFound);
}

#[test]
fn test_input_keys_result_serialization() {
    let result = InputKeysResult::executed();
    let json = serde_json::to_string(&result).unwrap();
    assert!(json.contains("\"ok\":true"));
    assert!(json.contains("\"status\":\"executed\""));

    let result = InputKeysResult::pending();
    let json = serde_json::to_string(&result).unwrap();
    assert!(json.contains("\"status\":\"pending\""));

    let result = InputKeysResult::not_found();
    let json = serde_json::to_string(&result).unwrap();
    assert!(json.contains("\"status\":\"not_found\""));
}

#[test]
fn test_state_window_content_result() {
    let result = StateWindowContentResult {
        window_id: WireWindowId::from(1),
        bounds: WireRect::new(0, 0, 40, 12),
        format: ScreenFormat::PlainText,
        content: "Hello, Window!".to_string(),
    };
    let json = serde_json::to_string(&result).unwrap();
    assert!(json.contains("\"window_id\":1"));
    assert!(json.contains("\"bounds\""));
    assert!(json.contains("\"format\":\"plain_text\""));
    assert!(json.contains("\"content\":\"Hello, Window!\""));
}

#[test]
fn test_state_options_result() {
    use std::collections::HashMap;
    let mut options = HashMap::new();
    options.insert("number".to_string(), serde_json::json!(true));
    options.insert("relativenumber".to_string(), serde_json::json!(false));

    let result = StateOptionsResult { options };
    let json = serde_json::to_string(&result).unwrap();
    assert!(json.contains("\"options\""));
    assert!(json.contains("\"number\":true"));
    assert!(json.contains("\"relativenumber\":false"));
}

// Additional coverage tests

#[test]
fn test_ok_result_default() {
    let result = OkResult::default();
    // Default derive uses bool::default() which is false
    assert!(!result.ok);
}

#[test]
fn test_ok_result_deserialization_empty() {
    let result: OkResult = serde_json::from_str("{}").unwrap();
    assert!(result.ok); // default_ok = true
}

#[test]
fn test_ok_result_deserialization_explicit() {
    let result: OkResult = serde_json::from_str(r#"{"ok":false}"#).unwrap();
    assert!(!result.ok);
}

#[test]
fn test_buffer_content_result_roundtrip() {
    let result = BufferContentResult {
        content: "hello\nworld\n".to_string(),
    };
    let json = serde_json::to_string(&result).unwrap();
    let decoded: BufferContentResult = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.content, "hello\nworld\n");
}

#[test]
fn test_buffer_open_result_roundtrip() {
    let result = BufferOpenResult {
        buffer_id: BufferId::from(42),
    };
    let json = serde_json::to_string(&result).unwrap();
    let decoded: BufferOpenResult = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.buffer_id, BufferId::from(42));
}

#[test]
fn test_screen_content_result_roundtrip() {
    let result = ScreenContentResult {
        width: 120,
        height: 40,
        format: ScreenFormat::RawAnsi,
        content: "escape codes here".to_string(),
    };
    let json = serde_json::to_string(&result).unwrap();
    let decoded: ScreenContentResult = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.width, 120);
    assert_eq!(decoded.height, 40);
    assert_eq!(decoded.format, ScreenFormat::RawAnsi);
    assert_eq!(decoded.content, "escape codes here");
}

#[test]
fn test_key_status_deserialization() {
    let s: KeyStatus = serde_json::from_str("\"executed\"").unwrap();
    assert_eq!(s, KeyStatus::Executed);
    let s: KeyStatus = serde_json::from_str("\"pending\"").unwrap();
    assert_eq!(s, KeyStatus::Pending);
    let s: KeyStatus = serde_json::from_str("\"not_found\"").unwrap();
    assert_eq!(s, KeyStatus::NotFound);
}

#[test]
fn test_input_keys_result_deserialization() {
    let json = r#"{"ok":true,"status":"executed"}"#;
    let result: InputKeysResult = serde_json::from_str(json).unwrap();
    assert!(result.ok);
    assert_eq!(result.status, KeyStatus::Executed);
}

#[test]
fn test_state_window_content_result_roundtrip() {
    let result = StateWindowContentResult {
        window_id: WireWindowId::from(3),
        bounds: WireRect::new(10, 5, 60, 20),
        format: ScreenFormat::CellGrid,
        content: r#"{"cells":[]}"#.to_string(),
    };
    let json = serde_json::to_string(&result).unwrap();
    let decoded: StateWindowContentResult = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.window_id, WireWindowId::from(3));
    assert_eq!(decoded.bounds.x, 10);
    assert_eq!(decoded.bounds.y, 5);
}

#[test]
fn test_module_info_roundtrip() {
    let info = ModuleInfo {
        id: "test-mod".to_string(),
        name: "Test Module".to_string(),
        version: "1.2.3".to_string(),
        state: "Active".to_string(),
        path: Some("/path/to/mod.so".to_string()),
        is_static: false,
        dependencies: vec!["core".to_string(), "vim".to_string()],
    };
    let json = serde_json::to_string(&info).unwrap();
    let decoded: ModuleInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.id, "test-mod");
    assert_eq!(decoded.dependencies.len(), 2);
}

#[test]
fn test_state_options_result_roundtrip() {
    let mut options = HashMap::new();
    options.insert("tabstop".to_string(), serde_json::json!(4));
    options.insert("expandtab".to_string(), serde_json::json!(true));
    options.insert("filetype".to_string(), serde_json::json!("rust"));

    let result = StateOptionsResult { options };
    let json = serde_json::to_string(&result).unwrap();
    let decoded: StateOptionsResult = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.options["tabstop"], serde_json::json!(4));
    assert_eq!(decoded.options["expandtab"], serde_json::json!(true));
    assert_eq!(decoded.options["filetype"], serde_json::json!("rust"));
}

#[test]
fn test_state_options_result_empty() {
    let result = StateOptionsResult {
        options: HashMap::new(),
    };
    let json = serde_json::to_string(&result).unwrap();
    assert!(json.contains("\"options\":{}"));
}
