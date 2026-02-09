//! Standard RPC method names.
//!
//! This module defines constant strings for all supported RPC methods.
//! Methods are organized by category (input, command, buffer, state, etc.).

// Input methods

/// Inject key events.
pub const INPUT_KEYS: &str = "input/keys";

// Command methods

/// Execute an ex-command.
pub const COMMAND_EXECUTE: &str = "command/execute";

// Buffer methods

/// Get buffer content.
pub const BUFFER_GET_CONTENT: &str = "buffer/get_content";

/// Set buffer content.
pub const BUFFER_SET_CONTENT: &str = "buffer/set_content";

/// Open a file in a buffer.
pub const BUFFER_OPEN_FILE: &str = "buffer/open_file";

/// List all buffers.
pub const BUFFER_LIST: &str = "buffer/list";

/// Write buffer to file.
pub const BUFFER_WRITE_FILE: &str = "buffer/write_file";

// State methods

/// Get current mode.
pub const STATE_MODE: &str = "state/mode";

/// Get cursor position.
pub const STATE_CURSOR: &str = "state/cursor";

/// Get selection.
pub const STATE_SELECTION: &str = "state/selection";

/// Get screen state.
pub const STATE_SCREEN: &str = "state/screen";

/// Get screen content.
pub const STATE_SCREEN_CONTENT: &str = "state/screen_content";

/// Get telescope state.
pub const STATE_TELESCOPE: &str = "state/telescope";

/// Get microscope state.
pub const STATE_MICROSCOPE: &str = "state/microscope";

/// Get windows state.
pub const STATE_WINDOWS: &str = "state/windows";

/// Get window layout state.
///
/// Returns the complete layout tree with window positions, focus state,
/// and layer information. Used by TUI to render multi-window layouts.
pub const STATE_LAYOUT: &str = "state/layout";

/// Get content for a specific window.
///
/// Returns rendered content for a single window, allowing clients to
/// compose multi-window displays or update individual windows.
pub const STATE_WINDOW_CONTENT: &str = "state/window_content";

/// Get editor option values.
///
/// Returns option values with optional filtering by window ID and option names.
/// Options can be bool, int, or string depending on the option type.
pub const STATE_OPTIONS: &str = "state/options";

// Visual methods (for debugging and AI understanding)

/// Get visual snapshot.
pub const STATE_VISUAL_SNAPSHOT: &str = "state/visual_snapshot";

/// Get ASCII art representation.
pub const STATE_ASCII_ART: &str = "state/ascii_art";

/// Get layer information.
pub const STATE_LAYER_INFO: &str = "state/layer_info";

// Editor methods

/// Resize the editor.
pub const EDITOR_RESIZE: &str = "editor/resize";

/// Set the active buffer for a client.
pub const EDITOR_SET_ACTIVE_BUFFER: &str = "editor/set_active_buffer";

/// Quit the editor.
pub const EDITOR_QUIT: &str = "editor/quit";

// TUI methods

/// Capture TUI frame via RPC relay.
///
/// This method is relayed by the server to a connected TUI client,
/// which responds with rendered frame content. Requires a TUI client
/// to be connected (interactive or headless mode).
pub const TUI_CAPTURE: &str = "tui/capture";

// Server methods

/// Kill the server.
pub const SERVER_KILL: &str = "server/kill";

// Module methods

/// Load a module from a path.
pub const MODULE_LOAD: &str = "module/load";

/// Unload a module by ID.
pub const MODULE_UNLOAD: &str = "module/unload";

/// Hot reload a module by ID.
pub const MODULE_RELOAD: &str = "module/reload";

/// List all loaded modules.
pub const MODULE_LIST: &str = "module/list";

// Debug methods

/// Get server version information.
pub const DEBUG_VERSION: &str = "debug/version";

/// Get server uptime.
pub const DEBUG_UPTIME: &str = "debug/uptime";

/// Get kernel state summary.
pub const DEBUG_KERNEL_STATE: &str = "debug/kernel_state";

/// Get register contents.
pub const DEBUG_REGISTERS: &str = "debug/registers";

/// Get mark contents.
pub const DEBUG_MARKS: &str = "debug/marks";

/// Get mode stack.
pub const DEBUG_MODE_STACK: &str = "debug/mode_stack";

/// Get performance metrics.
pub const DEBUG_METRICS: &str = "debug/metrics";

/// Get per-handler statistics.
pub const DEBUG_HANDLERS: &str = "debug/handlers";

/// Get or set log level.
pub const DEBUG_LOG_LEVEL: &str = "debug/log_level";

/// Get recent log entries.
pub const DEBUG_LOG_TAIL: &str = "debug/log_tail";

/// Subscribe to log streaming.
pub const DEBUG_LOG_SUBSCRIBE: &str = "debug/log_subscribe";

/// Unsubscribe from log streaming.
pub const DEBUG_LOG_UNSUBSCRIBE: &str = "debug/log_unsubscribe";

/// Get full debug snapshot for AI/tooling.
pub const DEBUG_VISUAL_SNAPSHOT: &str = "debug/visual_snapshot";

#[cfg(test)]
mod tests {
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
}
