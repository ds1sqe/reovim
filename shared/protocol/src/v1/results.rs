//! Typed results for RPC methods.
//!
//! Each RPC method has a corresponding result type that defines
//! the expected response structure.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::types::{
    BufferId, BufferInfo, CursorInfo, ModeInfo, ScreenFormat, ScreenInfo, SelectionInfo,
    WindowInfo, WireLayoutInfo, WireRect, WireWindowId,
};

/// Result type aliases for state methods.
pub type StateCursorResult = CursorInfo;
pub type StateModeResult = ModeInfo;
pub type StateSelectionResult = SelectionInfo;
pub type StateScreenResult = ScreenInfo;
/// Result for `state/layout` method.
pub type StateLayoutResult = WireLayoutInfo;

/// Result for `state/window_content` method (#444).
///
/// Contains rendered content for a specific window.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateWindowContentResult {
    /// Window ID.
    pub window_id: WireWindowId,
    /// Window bounds within screen.
    pub bounds: WireRect,
    /// Content format.
    pub format: ScreenFormat,
    /// Window content (in requested format).
    pub content: String,
}

/// Result for `state/options` method (#445).
///
/// Contains option values as a map of name to JSON value.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateOptionsResult {
    /// Option values as JSON values.
    /// Values can be bool, int, or string depending on the option type.
    pub options: HashMap<String, serde_json::Value>,
}

/// Result for `state/screen_content` method.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenContentResult {
    /// Terminal width.
    pub width: u16,
    /// Terminal height.
    pub height: u16,
    /// Content format.
    pub format: ScreenFormat,
    /// Content string (format depends on `format` field).
    pub content: String,
}

/// Result for `buffer/list` method.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BufferListResult {
    /// List of all buffers.
    pub buffers: Vec<BufferInfo>,
}

/// Result for `buffer/get_content` method.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BufferContentResult {
    /// Buffer content as a string.
    pub content: String,
}

/// Result for `buffer/open_file` method.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BufferOpenResult {
    /// ID of the opened buffer.
    pub buffer_id: BufferId,
}

/// Result for `state/windows` method.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowsResult {
    /// List of all windows.
    pub windows: Vec<WindowInfo>,
}

/// Generic OK result for methods that return success/failure.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OkResult {
    /// Always true for success.
    #[serde(default = "default_ok")]
    pub ok: bool,
}

const fn default_ok() -> bool {
    true
}

impl OkResult {
    /// Create a new OK result.
    #[must_use]
    pub const fn new() -> Self {
        Self { ok: true }
    }
}

/// Information about a loaded module.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleInfo {
    /// Unique module identifier.
    pub id: String,
    /// Human-readable module name.
    pub name: String,
    /// Module version string (e.g., "1.0.0").
    pub version: String,
    /// Current module state.
    pub state: String,
    /// Path to the module (for dynamic modules).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Whether this is a statically linked module.
    pub is_static: bool,
    /// List of module dependencies.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<String>,
}

/// Result for `module/list` method.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleListResult {
    /// List of all loaded modules.
    pub modules: Vec<ModuleInfo>,
}

/// Result for `module/load` method.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleLoadResult {
    /// Information about the loaded module.
    pub module: ModuleInfo,
}

/// Status of key lookup in keymap.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum KeyStatus {
    /// Keys matched a binding and command was executed.
    Executed,
    /// Keys are a prefix of a binding, waiting for more.
    Pending,
    /// Keys don't match any binding.
    NotFound,
}

/// Result for `input/keys` method.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputKeysResult {
    /// Whether the operation succeeded.
    pub ok: bool,
    /// Status of key lookup.
    pub status: KeyStatus,
}

impl InputKeysResult {
    /// Create result for executed command.
    #[must_use]
    pub const fn executed() -> Self {
        Self {
            ok: true,
            status: KeyStatus::Executed,
        }
    }

    /// Create result for pending key sequence.
    #[must_use]
    pub const fn pending() -> Self {
        Self {
            ok: true,
            status: KeyStatus::Pending,
        }
    }

    /// Create result for unbound keys.
    #[must_use]
    pub const fn not_found() -> Self {
        Self {
            ok: true,
            status: KeyStatus::NotFound,
        }
    }
}

#[cfg(test)]
#[path = "results_tests.rs"]
mod tests;
