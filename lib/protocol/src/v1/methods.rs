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

/// Quit the editor.
pub const EDITOR_QUIT: &str = "editor/quit";

// Server methods

/// Kill the server.
pub const SERVER_KILL: &str = "server/kill";

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
