use super::*;

#[test]
fn test_py_runtime_api_creation() {
    let _api = PyRuntimeApi::new();
}

#[test]
fn test_active_buffer_no_runtime() {
    let api = PyRuntimeApi;
    assert!(api.active_buffer().is_err());
}

#[test]
fn test_buffer_line_no_runtime() {
    let api = PyRuntimeApi;
    assert!(api.buffer_line(1, 0).is_err());
}

#[test]
fn test_buffer_line_count_no_runtime() {
    let api = PyRuntimeApi;
    assert!(api.buffer_line_count(1).is_err());
}

#[test]
fn test_insert_text_no_runtime() {
    let api = PyRuntimeApi;
    assert!(api.insert_text(1, 0, 0, "hello").is_err());
}

#[test]
fn test_delete_range_no_runtime() {
    let api = PyRuntimeApi;
    assert!(api.delete_range(1, 0, 0, 0, 5).is_err());
}

#[test]
fn test_create_buffer_no_runtime() {
    let api = PyRuntimeApi;
    assert!(api.create_buffer().is_err());
}

#[test]
fn test_active_window_no_runtime() {
    let api = PyRuntimeApi;
    assert!(api.active_window().is_err());
}

#[test]
fn test_cursor_position_no_runtime() {
    let api = PyRuntimeApi;
    assert!(api.cursor_position().is_err());
}

#[test]
fn test_window_count_no_runtime() {
    let api = PyRuntimeApi;
    assert!(api.window_count().is_err());
}

#[test]
fn test_current_mode_no_runtime() {
    let api = PyRuntimeApi;
    assert!(api.current_mode().is_err());
}

#[test]
fn test_mode_depth_no_runtime() {
    let api = PyRuntimeApi;
    assert!(api.mode_depth().is_err());
}

#[test]
fn test_push_mode_no_runtime() {
    let api = PyRuntimeApi;
    assert!(api.push_mode("vim:insert").is_err());
}

#[test]
fn test_push_mode_invalid_format() {
    let api = PyRuntimeApi;
    assert!(api.push_mode("nocolon").is_err());
}

#[test]
fn test_push_mode_empty_module() {
    let api = PyRuntimeApi;
    assert!(api.push_mode(":name").is_err());
}

#[test]
fn test_push_mode_empty_name() {
    let api = PyRuntimeApi;
    assert!(api.push_mode("module:").is_err());
}

#[test]
fn test_pop_mode_no_runtime() {
    let api = PyRuntimeApi;
    assert!(api.pop_mode().is_err());
}

#[test]
fn test_set_mode_no_runtime() {
    let api = PyRuntimeApi;
    assert!(api.set_mode("vim:visual").is_err());
}

#[test]
fn test_set_mode_invalid_format() {
    let api = PyRuntimeApi;
    assert!(api.set_mode("nocolon").is_err());
}

#[test]
fn test_execute_command_no_runtime() {
    let api = PyRuntimeApi;
    assert!(api.execute_command("vim:delete", None, None).is_err());
}

#[test]
fn test_parse_mode_id_valid() {
    let mode = parse_mode_id("vim:normal").unwrap();
    assert_eq!(mode.module().as_str(), "vim");
    assert_eq!(mode.name(), "normal");
}

#[test]
fn test_parse_mode_id_no_colon() {
    assert!(parse_mode_id("nocolon").is_none());
}

#[test]
fn test_parse_mode_id_empty_parts() {
    assert!(parse_mode_id(":name").is_none());
    assert!(parse_mode_id("module:").is_none());
}

#[test]
fn test_parse_mode_id_multiple_colons() {
    let mode = parse_mode_id("a:b:c").unwrap();
    assert_eq!(mode.module().as_str(), "a");
    assert_eq!(mode.name(), "b:c");
}

// ========================================================================
// Clipboard API tests
// ========================================================================

#[test]
fn test_copy_to_clipboard_no_runtime() {
    let api = PyRuntimeApi;
    assert!(api.copy_to_clipboard("text").is_err());
}

#[test]
fn test_paste_from_clipboard_no_runtime() {
    let api = PyRuntimeApi;
    assert!(api.paste_from_clipboard().is_err());
}

#[test]
fn test_copy_to_selection_no_runtime() {
    let api = PyRuntimeApi;
    assert!(api.copy_to_selection("text").is_err());
}

#[test]
fn test_paste_from_selection_no_runtime() {
    let api = PyRuntimeApi;
    assert!(api.paste_from_selection().is_err());
}

// ========================================================================
// Register API tests
// ========================================================================

#[test]
fn test_get_register_no_runtime() {
    let api = PyRuntimeApi;
    assert!(api.get_register(None).is_err());
}

#[test]
fn test_get_register_named_no_runtime() {
    let api = PyRuntimeApi;
    assert!(api.get_register(Some('a')).is_err());
}

#[test]
fn test_set_register_no_runtime() {
    let api = PyRuntimeApi;
    assert!(api.set_register("hello", None, "characterwise").is_err());
}

#[test]
fn test_set_register_invalid_yank_type() {
    let api = PyRuntimeApi;
    let result = api.set_register("hello", None, "invalid");
    assert!(result.is_err());
}

// ========================================================================
// Undo API tests
// ========================================================================

#[test]
fn test_undo_no_runtime() {
    let api = PyRuntimeApi;
    assert!(api.undo(1).is_err());
}

#[test]
fn test_redo_no_runtime() {
    let api = PyRuntimeApi;
    assert!(api.redo(1).is_err());
}

#[test]
fn test_can_undo_no_runtime() {
    let api = PyRuntimeApi;
    assert!(api.can_undo(1).is_err());
}

#[test]
fn test_can_redo_no_runtime() {
    let api = PyRuntimeApi;
    assert!(api.can_redo(1).is_err());
}
