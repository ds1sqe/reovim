//! Typed results for RPC methods.
//!
//! Each RPC method has a corresponding result type that defines
//! the expected response structure.

use serde::{Deserialize, Serialize};

use super::types::{
    BufferInfo, CursorInfo, ModeInfo, ScreenFormat, ScreenInfo, SelectionInfo, WindowInfo,
};

/// Result type aliases for state methods.
pub type StateCursorResult = CursorInfo;
pub type StateModeResult = ModeInfo;
pub type StateSelectionResult = SelectionInfo;
pub type StateScreenResult = ScreenInfo;

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

#[cfg(test)]
mod tests {
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
                },
                BufferInfo {
                    id: 2,
                    file_path: None,
                    modified: true,
                    line_count: 5,
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
        let result = WindowsResult {
            windows: vec![WindowInfo {
                id: 1,
                buffer_id: 1,
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
}
