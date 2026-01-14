//! Typed parameters for RPC methods.
//!
//! Each RPC method has a corresponding params type that defines
//! the expected input structure.

use serde::{Deserialize, Serialize};

use super::types::ScreenFormat;

/// Parameters for `input/keys` method.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputKeysParams {
    /// Key sequence in vim notation (e.g., `iHello<Esc>`).
    pub keys: String,
}

/// Parameters for `command/execute` method.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandExecuteParams {
    /// Command name.
    pub cmd: String,
    /// Command arguments.
    #[serde(default)]
    pub args: Vec<String>,
}

/// Parameters for `state/cursor` method.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StateCursorParams {
    /// Buffer ID (default: active buffer).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub buffer_id: Option<usize>,
}

/// Parameters for `state/selection` method.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StateSelectionParams {
    /// Buffer ID (default: active buffer).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub buffer_id: Option<usize>,
}

/// Parameters for `state/screen_content` method.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StateScreenContentParams {
    /// Output format.
    #[serde(default)]
    pub format: ScreenFormat,
}

/// Parameters for `buffer/get_content` method.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BufferGetContentParams {
    /// Buffer ID (default: active buffer).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub buffer_id: Option<usize>,
}

/// Parameters for `buffer/set_content` method.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BufferSetContentParams {
    /// New buffer content.
    pub content: String,
    /// Buffer ID (default: active buffer).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub buffer_id: Option<usize>,
}

/// Parameters for `buffer/open_file` method.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BufferOpenFileParams {
    /// Path to the file to open.
    pub path: String,
}

/// Parameters for `editor/resize` method.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorResizeParams {
    /// New width in columns.
    pub width: u16,
    /// New height in rows.
    pub height: u16,
}

/// Parameters for `module/load` method.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleLoadParams {
    /// Path to the module shared library.
    pub path: String,
}

/// Parameters for `module/unload` method.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleUnloadParams {
    /// Module ID to unload.
    pub id: String,
}

/// Parameters for `module/reload` method.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleReloadParams {
    /// Module ID to reload.
    pub id: String,
}

#[cfg(test)]
mod tests {
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
}
