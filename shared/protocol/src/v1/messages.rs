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
#[path = "messages_tests.rs"]
mod tests;
