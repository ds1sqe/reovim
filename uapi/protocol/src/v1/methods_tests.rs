use super::*;

#[test]
fn test_method_names_format() {
    // All method names should follow category/action format
    assert!(INPUT_KEYS.contains('/'));
    assert!(STATE_MODE.contains('/'));
    assert!(BUFFER_GET_CONTENT.contains('/'));
    assert!(EDITOR_RESIZE.contains('/'));
}

#[test]
fn test_tui_capture_method() {
    assert_eq!(TUI_CAPTURE, "tui/capture");
    assert!(TUI_CAPTURE.contains('/'));
}

#[test]
fn test_all_method_constants_nonempty() {
    let methods = [
        INPUT_KEYS,
        COMMAND_EXECUTE,
        BUFFER_GET_CONTENT,
        BUFFER_SET_CONTENT,
        BUFFER_OPEN_FILE,
        BUFFER_LIST,
        BUFFER_WRITE_FILE,
        STATE_MODE,
        STATE_CURSOR,
        STATE_SELECTION,
        STATE_SCREEN,
        STATE_SCREEN_CONTENT,
        STATE_TELESCOPE,
        STATE_MICROSCOPE,
        STATE_WINDOWS,
        STATE_LAYOUT,
        STATE_WINDOW_CONTENT,
        STATE_OPTIONS,
        STATE_VISUAL_SNAPSHOT,
        STATE_ASCII_ART,
        STATE_LAYER_INFO,
        EDITOR_RESIZE,
        EDITOR_SET_ACTIVE_BUFFER,
        EDITOR_QUIT,
        TUI_CAPTURE,
        SERVER_KILL,
        MODULE_LOAD,
        MODULE_UNLOAD,
        MODULE_RELOAD,
        MODULE_LIST,
        DEBUG_VERSION,
        DEBUG_UPTIME,
        DEBUG_KERNEL_STATE,
        DEBUG_REGISTERS,
        DEBUG_MARKS,
        DEBUG_MODE_STACK,
        DEBUG_METRICS,
        DEBUG_HANDLERS,
        DEBUG_LOG_LEVEL,
        DEBUG_LOG_TAIL,
        DEBUG_LOG_SUBSCRIBE,
        DEBUG_LOG_UNSUBSCRIBE,
        DEBUG_VISUAL_SNAPSHOT,
    ];
    for method in methods {
        assert!(!method.is_empty(), "Method constant should not be empty");
        assert!(method.contains('/'), "Method '{method}' should contain '/'");
    }
}

#[test]
fn test_debug_method_names() {
    assert_eq!(DEBUG_VERSION, "debug/version");
    assert_eq!(DEBUG_UPTIME, "debug/uptime");
    assert_eq!(DEBUG_KERNEL_STATE, "debug/kernel_state");
    assert_eq!(DEBUG_REGISTERS, "debug/registers");
    assert_eq!(DEBUG_MARKS, "debug/marks");
    assert_eq!(DEBUG_MODE_STACK, "debug/mode_stack");
    assert_eq!(DEBUG_METRICS, "debug/metrics");
    assert_eq!(DEBUG_HANDLERS, "debug/handlers");
    assert_eq!(DEBUG_LOG_LEVEL, "debug/log_level");
    assert_eq!(DEBUG_LOG_TAIL, "debug/log_tail");
    assert_eq!(DEBUG_LOG_SUBSCRIBE, "debug/log_subscribe");
    assert_eq!(DEBUG_LOG_UNSUBSCRIBE, "debug/log_unsubscribe");
    assert_eq!(DEBUG_VISUAL_SNAPSHOT, "debug/visual_snapshot");
}

#[test]
fn test_module_method_names() {
    assert_eq!(MODULE_LOAD, "module/load");
    assert_eq!(MODULE_UNLOAD, "module/unload");
    assert_eq!(MODULE_RELOAD, "module/reload");
    assert_eq!(MODULE_LIST, "module/list");
}

#[test]
fn test_editor_method_names() {
    assert_eq!(EDITOR_RESIZE, "editor/resize");
    assert_eq!(EDITOR_SET_ACTIVE_BUFFER, "editor/set_active_buffer");
    assert_eq!(EDITOR_QUIT, "editor/quit");
}

#[test]
fn test_state_method_names() {
    assert_eq!(STATE_MODE, "state/mode");
    assert_eq!(STATE_CURSOR, "state/cursor");
    assert_eq!(STATE_SELECTION, "state/selection");
    assert_eq!(STATE_SCREEN, "state/screen");
    assert_eq!(STATE_SCREEN_CONTENT, "state/screen_content");
    assert_eq!(STATE_WINDOWS, "state/windows");
    assert_eq!(STATE_LAYOUT, "state/layout");
    assert_eq!(STATE_WINDOW_CONTENT, "state/window_content");
    assert_eq!(STATE_OPTIONS, "state/options");
}

#[test]
fn test_server_method_names() {
    assert_eq!(SERVER_KILL, "server/kill");
}
