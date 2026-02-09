//! JSON-RPC 2.0 message types.
//!
//! This module defines the core message types for the JSON-RPC protocol:
//! - [`RpcRequest`]: Incoming requests from clients
//! - [`RpcResponse`]: Responses to requests
//! - [`RpcNotification`]: Server-to-client notifications
//! - [`RpcError`]: Error information

use serde::{Deserialize, Serialize};

use super::codes::{
    BUFFER_NOT_FOUND, COMMAND_NOT_FOUND, INTERNAL_ERROR, INVALID_KEY_NOTATION, INVALID_PARAMS,
    INVALID_REQUEST, METHOD_NOT_FOUND, PARSE_ERROR,
};

/// JSON-RPC 2.0 request message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcRequest {
    /// Protocol version (always "2.0").
    pub jsonrpc: String,

    /// Request ID (omitted for notifications).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,

    /// Method name (e.g., "input/keys", "state/cursor").
    pub method: String,

    /// Method parameters.
    #[serde(default)]
    pub params: serde_json::Value,
}

impl RpcRequest {
    /// Create a new request with the given method and params.
    #[must_use]
    pub fn new(id: u64, method: impl Into<String>, params: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: Some(id),
            method: method.into(),
            params,
        }
    }

    /// Create a notification (request without ID).
    #[must_use]
    pub fn notification(method: impl Into<String>, params: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: None,
            method: method.into(),
            params,
        }
    }

    /// Check if this is a notification (no id).
    #[must_use]
    pub const fn is_notification(&self) -> bool {
        self.id.is_none()
    }
}

/// JSON-RPC 2.0 response message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcResponse {
    /// Protocol version (always "2.0").
    pub jsonrpc: String,

    /// Request ID this response corresponds to.
    pub id: u64,

    /// Result value (mutually exclusive with error).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,

    /// Error information (mutually exclusive with result).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

impl RpcResponse {
    /// Create a success response with the given result.
    #[must_use]
    pub fn success(id: u64, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    /// Create an error response.
    #[must_use]
    pub fn error(id: u64, error: RpcError) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(error),
        }
    }

    /// Create a success response with null result.
    #[must_use]
    pub fn ok(id: u64) -> Self {
        Self::success(id, serde_json::Value::Null)
    }
}

/// JSON-RPC 2.0 error object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcError {
    /// Error code.
    pub code: i32,

    /// Human-readable error message.
    pub message: String,

    /// Additional error data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl RpcError {
    /// Create a new error.
    #[must_use]
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }

    /// Create an error with additional data.
    #[must_use]
    pub fn with_data(code: i32, message: impl Into<String>, data: serde_json::Value) -> Self {
        Self {
            code,
            message: message.into(),
            data: Some(data),
        }
    }

    // Standard JSON-RPC 2.0 error codes

    /// Invalid JSON was received.
    #[must_use]
    pub fn parse_error() -> Self {
        Self::new(PARSE_ERROR, "Parse error")
    }

    /// The JSON sent is not a valid Request object.
    #[must_use]
    pub fn invalid_request() -> Self {
        Self::new(INVALID_REQUEST, "Invalid Request")
    }

    /// The method does not exist / is not available.
    #[must_use]
    pub fn method_not_found(method: &str) -> Self {
        Self::new(METHOD_NOT_FOUND, format!("Method not found: {method}"))
    }

    /// Invalid method parameter(s).
    #[must_use]
    pub fn invalid_params(details: impl Into<String>) -> Self {
        Self::new(INVALID_PARAMS, format!("Invalid params: {}", details.into()))
    }

    /// Internal JSON-RPC error.
    #[must_use]
    pub fn internal_error(details: impl Into<String>) -> Self {
        Self::new(INTERNAL_ERROR, format!("Internal error: {}", details.into()))
    }

    // Application-specific error codes

    /// Buffer not found.
    #[must_use]
    pub fn buffer_not_found(buffer_id: usize) -> Self {
        Self::new(BUFFER_NOT_FOUND, format!("Buffer not found: {buffer_id}"))
    }

    /// Command not found.
    #[must_use]
    pub fn command_not_found(command: &str) -> Self {
        Self::new(COMMAND_NOT_FOUND, format!("Command not found: {command}"))
    }

    /// Invalid key notation.
    #[must_use]
    pub fn invalid_key_notation(details: impl Into<String>) -> Self {
        Self::new(INVALID_KEY_NOTATION, format!("Invalid key notation: {}", details.into()))
    }
}

/// JSON-RPC 2.0 notification message (server to client).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcNotification {
    /// Protocol version (always "2.0").
    pub jsonrpc: String,

    /// Notification method name.
    pub method: String,

    /// Notification parameters.
    pub params: serde_json::Value,
}

impl RpcNotification {
    /// Create a new notification.
    #[must_use]
    pub fn new(method: impl Into<String>, params: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            method: method.into(),
            params,
        }
    }
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
    fn test_request_notification() {
        let request = RpcRequest::notification("event/ping", serde_json::json!({}));
        assert!(request.is_notification());
        let json = serde_json::to_string(&request).unwrap();
        assert!(!json.contains("\"id\""));
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

    #[test]
    fn test_error_codes() {
        assert_eq!(RpcError::parse_error().code, PARSE_ERROR);
        assert_eq!(RpcError::invalid_request().code, INVALID_REQUEST);
        assert_eq!(RpcError::method_not_found("test").code, METHOD_NOT_FOUND);
        assert_eq!(RpcError::invalid_params("test").code, INVALID_PARAMS);
        assert_eq!(RpcError::internal_error("test").code, INTERNAL_ERROR);
    }

    #[test]
    fn test_application_error_codes() {
        assert_eq!(RpcError::buffer_not_found(0).code, BUFFER_NOT_FOUND);
        assert_eq!(RpcError::command_not_found("test").code, COMMAND_NOT_FOUND);
        assert_eq!(RpcError::invalid_key_notation("test").code, INVALID_KEY_NOTATION);
    }

    #[test]
    fn test_response_ok() {
        let response = RpcResponse::ok(42);
        assert_eq!(response.id, 42);
        assert_eq!(response.result, Some(serde_json::Value::Null));
        assert!(response.error.is_none());
    }

    #[test]
    fn test_rpc_error_with_data() {
        let data = serde_json::json!({"detail": "extra info"});
        let error = RpcError::with_data(-1, "custom error", data.clone());
        assert_eq!(error.code, -1);
        assert_eq!(error.message, "custom error");
        assert_eq!(error.data, Some(data));
    }

    #[test]
    fn test_rpc_error_without_data_serialization() {
        let error = RpcError::new(-100, "no data");
        let json = serde_json::to_string(&error).unwrap();
        assert!(!json.contains("\"data\""));
        assert!(json.contains("\"code\":-100"));
        assert!(json.contains("\"message\":\"no data\""));
    }

    #[test]
    fn test_rpc_error_with_data_serialization() {
        let error = RpcError::with_data(-200, "has data", serde_json::json!([1, 2, 3]));
        let json = serde_json::to_string(&error).unwrap();
        assert!(json.contains("\"data\":[1,2,3]"));
    }

    #[test]
    fn test_rpc_error_message_content() {
        let err = RpcError::parse_error();
        assert_eq!(err.message, "Parse error");

        let err = RpcError::invalid_request();
        assert_eq!(err.message, "Invalid Request");

        let err = RpcError::method_not_found("foo/bar");
        assert_eq!(err.message, "Method not found: foo/bar");

        let err = RpcError::invalid_params("missing keys");
        assert_eq!(err.message, "Invalid params: missing keys");

        let err = RpcError::internal_error("crash");
        assert_eq!(err.message, "Internal error: crash");

        let err = RpcError::buffer_not_found(42);
        assert_eq!(err.message, "Buffer not found: 42");

        let err = RpcError::command_not_found("wq");
        assert_eq!(err.message, "Command not found: wq");

        let err = RpcError::invalid_key_notation("<Invalid>");
        assert_eq!(err.message, "Invalid key notation: <Invalid>");
    }

    #[test]
    fn test_request_is_not_notification() {
        let request = RpcRequest::new(1, "test", serde_json::json!({}));
        assert!(!request.is_notification());
    }

    #[test]
    fn test_response_deserialization_success() {
        let json = r#"{"jsonrpc":"2.0","id":1,"result":{"ok":true}}"#;
        let response: RpcResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.id, 1);
        assert!(response.result.is_some());
        assert!(response.error.is_none());
    }

    #[test]
    fn test_response_deserialization_error() {
        let json = r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32601,"message":"Method not found: test"}}"#;
        let response: RpcResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.id, 1);
        assert!(response.result.is_none());
        let err = response.error.unwrap();
        assert_eq!(err.code, METHOD_NOT_FOUND);
    }

    #[test]
    fn test_notification_deserialization() {
        let json = r#"{"jsonrpc":"2.0","method":"notification/test","params":{"key":"value"}}"#;
        let notification: RpcNotification = serde_json::from_str(json).unwrap();
        assert_eq!(notification.method, "notification/test");
        assert_eq!(notification.params["key"], "value");
    }

    #[test]
    fn test_request_default_params() {
        let json = r#"{"jsonrpc":"2.0","id":1,"method":"test"}"#;
        let request: RpcRequest = serde_json::from_str(json).unwrap();
        // params should default to null when missing
        assert!(request.params.is_null());
    }

    #[test]
    fn test_rpc_error_roundtrip() {
        let error = RpcError::with_data(-32000, "app error", serde_json::json!({"id": 1}));
        let json = serde_json::to_string(&error).unwrap();
        let decoded: RpcError = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.code, -32000);
        assert_eq!(decoded.message, "app error");
        assert!(decoded.data.is_some());
    }
}
