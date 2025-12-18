//! JSON-RPC 2.0 message types for server mode
//!
//! This module defines the core message types for the JSON-RPC protocol:
//! - `RpcRequest`: Incoming requests from clients
//! - `RpcResponse`: Responses to requests
//! - `RpcNotification`: Server-to-client notifications
//! - `RpcError`: Error information

use serde::{Deserialize, Serialize};

/// JSON-RPC 2.0 request message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcRequest {
    /// Protocol version (always "2.0")
    pub jsonrpc: String,

    /// Request ID (omitted for notifications)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,

    /// Method name (e.g., "input/keys", "state/cursor")
    pub method: String,

    /// Method parameters
    #[serde(default)]
    pub params: serde_json::Value,
}

impl RpcRequest {
    /// Create a new request with the given method and params
    #[must_use]
    pub fn new(id: u64, method: impl Into<String>, params: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: Some(id),
            method: method.into(),
            params,
        }
    }

    /// Check if this is a notification (no id)
    #[must_use]
    pub const fn is_notification(&self) -> bool {
        self.id.is_none()
    }
}

/// JSON-RPC 2.0 response message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcResponse {
    /// Protocol version (always "2.0")
    pub jsonrpc: String,

    /// Request ID this response corresponds to
    pub id: u64,

    /// Result value (mutually exclusive with error)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,

    /// Error information (mutually exclusive with result)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

impl RpcResponse {
    /// Create a success response with the given result
    #[must_use]
    pub fn success(id: u64, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    /// Create an error response
    #[must_use]
    pub fn error(id: u64, error: RpcError) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(error),
        }
    }

    /// Create a success response with null result
    #[must_use]
    pub fn ok(id: u64) -> Self {
        Self::success(id, serde_json::Value::Null)
    }
}

/// JSON-RPC 2.0 error object
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcError {
    /// Error code
    pub code: i32,

    /// Human-readable error message
    pub message: String,

    /// Additional error data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl RpcError {
    /// Create a new error
    #[must_use]
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }

    /// Create an error with additional data
    #[must_use]
    pub fn with_data(code: i32, message: impl Into<String>, data: serde_json::Value) -> Self {
        Self {
            code,
            message: message.into(),
            data: Some(data),
        }
    }

    // Standard JSON-RPC 2.0 error codes

    /// Invalid JSON was received
    #[must_use]
    pub fn parse_error() -> Self {
        Self::new(-32700, "Parse error")
    }

    /// The JSON sent is not a valid Request object
    #[must_use]
    pub fn invalid_request() -> Self {
        Self::new(-32600, "Invalid Request")
    }

    /// The method does not exist / is not available
    #[must_use]
    pub fn method_not_found(method: &str) -> Self {
        Self::new(-32601, format!("Method not found: {method}"))
    }

    /// Invalid method parameter(s)
    #[must_use]
    pub fn invalid_params(details: impl Into<String>) -> Self {
        Self::new(-32602, format!("Invalid params: {}", details.into()))
    }

    /// Internal JSON-RPC error
    #[must_use]
    pub fn internal_error(details: impl Into<String>) -> Self {
        Self::new(-32603, format!("Internal error: {}", details.into()))
    }

    // Application-specific error codes (-32000 to -32099)

    /// Buffer not found
    #[must_use]
    pub fn buffer_not_found(buffer_id: usize) -> Self {
        Self::new(-32001, format!("Buffer not found: {buffer_id}"))
    }

    /// Command not found
    #[must_use]
    pub fn command_not_found(command: &str) -> Self {
        Self::new(-32002, format!("Command not found: {command}"))
    }

    /// Invalid key notation
    #[must_use]
    pub fn invalid_key_notation(details: impl Into<String>) -> Self {
        Self::new(-32003, format!("Invalid key notation: {}", details.into()))
    }
}

/// JSON-RPC 2.0 notification message (server to client)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcNotification {
    /// Protocol version (always "2.0")
    pub jsonrpc: String,

    /// Notification method name
    pub method: String,

    /// Notification parameters
    pub params: serde_json::Value,
}

impl RpcNotification {
    /// Create a new notification
    #[must_use]
    pub fn new(method: impl Into<String>, params: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            method: method.into(),
            params,
        }
    }
}

/// Methods supported by the RPC server
pub mod methods {
    // Input methods
    pub const INPUT_KEYS: &str = "input/keys";

    // Command methods
    pub const COMMAND_EXECUTE: &str = "command/execute";

    // Buffer methods
    pub const BUFFER_GET_CONTENT: &str = "buffer/get_content";
    pub const BUFFER_SET_CONTENT: &str = "buffer/set_content";
    pub const BUFFER_OPEN_FILE: &str = "buffer/open_file";
    pub const BUFFER_LIST: &str = "buffer/list";

    // State methods
    pub const STATE_MODE: &str = "state/mode";
    pub const STATE_CURSOR: &str = "state/cursor";
    pub const STATE_SELECTION: &str = "state/selection";
    pub const STATE_SCREEN: &str = "state/screen";
    pub const STATE_SCREEN_CONTENT: &str = "state/screen_content";

    // Editor methods
    pub const EDITOR_RESIZE: &str = "editor/resize";
    pub const EDITOR_QUIT: &str = "editor/quit";

    // Server methods
    pub const SERVER_KILL: &str = "server/kill";
}

/// Notification types sent by the server
pub mod notifications {
    pub const MODE_CHANGED: &str = "notification/mode_changed";
    pub const CURSOR_MOVED: &str = "notification/cursor_moved";
    pub const BUFFER_MODIFIED: &str = "notification/buffer_modified";
    pub const RENDER_COMPLETE: &str = "notification/render_complete";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_serialization() {
        let request = RpcRequest::new(1, "state/cursor", serde_json::json!({"buffer_id": 0}));
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"jsonrpc\":\"2.0\""));
        assert!(json.contains("\"id\":1"));
        assert!(json.contains("\"method\":\"state/cursor\""));
    }

    #[test]
    fn test_response_success() {
        let response = RpcResponse::success(1, serde_json::json!({"x": 10, "y": 5}));
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"result\""));
        assert!(!json.contains("\"error\""));
    }

    #[test]
    fn test_response_error() {
        let response = RpcResponse::error(1, RpcError::method_not_found("unknown"));
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"error\""));
        assert!(!json.contains("\"result\""));
    }

    #[test]
    fn test_notification() {
        let notification = RpcNotification::new(
            "notification/mode_changed",
            serde_json::json!({"mode": "Insert"}),
        );
        let json = serde_json::to_string(&notification).unwrap();
        assert!(!json.contains("\"id\""));
        assert!(json.contains("\"method\":\"notification/mode_changed\""));
    }

    #[test]
    fn test_request_deserialization() {
        let json =
            r#"{"jsonrpc":"2.0","id":42,"method":"input/keys","params":{"keys":"iHello<Esc>"}}"#;
        let request: RpcRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.id, Some(42));
        assert_eq!(request.method, "input/keys");
    }
}
