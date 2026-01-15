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
}
