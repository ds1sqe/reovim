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

/// Parameters for `state/layout` method.
///
/// Empty params - just queries current layout state.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StateLayoutParams {
    // Empty for now - future: could add viewport hints
}

/// Parameters for `state/window_content` method (#444).
///
/// Requests content for a specific window.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateWindowContentParams {
    /// Window ID to get content for.
    pub window_id: super::types::WireWindowId,
    /// Output format.
    #[serde(default)]
    pub format: ScreenFormat,
}

/// Parameters for `state/options` method (#445).
///
/// Queries editor option values with optional filtering.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StateOptionsParams {
    /// Window ID for window-scoped options. If None, uses focused window.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_id: Option<usize>,
    /// Filter by option names. If None, returns all registered options.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub names: Option<Vec<String>>,
}

#[cfg(test)]
#[path = "params_tests.rs"]
mod tests;
