//! Manager RPC protocol.
//!
//! Defines the JSON-RPC messages for manager communication.

use serde::{Deserialize, Serialize};

use crate::server::instance::InstanceInfo;

/// Manager RPC request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagerRequest {
    /// Request ID for correlation.
    pub id: u64,
    /// Method and parameters.
    #[serde(flatten)]
    pub method: ManagerMethod,
}

impl ManagerRequest {
    /// Create a new request with the given ID and method.
    #[must_use]
    pub const fn new(id: u64, method: ManagerMethod) -> Self {
        Self { id, method }
    }
}

/// Manager RPC methods.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", content = "params")]
pub enum ManagerMethod {
    /// Register an instance with the manager.
    Register(InstanceInfo),

    /// Unregister an instance by name.
    Unregister {
        /// Instance name to unregister.
        name: String,
    },

    /// Query an instance by name.
    Query {
        /// Instance name to query.
        name: String,
    },

    /// List all registered instances.
    List,

    /// Ping the manager (health check).
    Ping,

    /// Shutdown the manager daemon.
    Shutdown,
}

/// Manager RPC response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagerResponse {
    /// Request ID this response correlates to.
    pub id: u64,
    /// Response result.
    pub result: ManagerResult,
}

impl ManagerResponse {
    /// Create a successful response.
    #[must_use]
    pub const fn ok(id: u64, value: serde_json::Value) -> Self {
        Self {
            id,
            result: ManagerResult::Ok(value),
        }
    }

    /// Create an error response.
    #[must_use]
    pub fn error(id: u64, code: i32, message: impl Into<String>) -> Self {
        Self {
            id,
            result: ManagerResult::Error {
                code,
                message: message.into(),
            },
        }
    }

    /// Create a "not found" error response.
    #[must_use]
    pub fn not_found(id: u64, name: &str) -> Self {
        Self::error(id, -32001, format!("Instance '{name}' not found"))
    }

    /// Create an "already exists" error response.
    #[must_use]
    pub fn already_exists(id: u64, name: &str) -> Self {
        Self::error(id, -32002, format!("Instance '{name}' already exists"))
    }
}

/// Manager RPC result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ManagerResult {
    /// Successful result with JSON value.
    Ok(serde_json::Value),
    /// Error result with code and message.
    Error {
        /// Error code.
        code: i32,
        /// Error message.
        message: String,
    },
}

impl ManagerResult {
    /// Check if this is a successful result.
    #[must_use]
    pub const fn is_ok(&self) -> bool {
        matches!(self, Self::Ok(_))
    }

    /// Check if this is an error result.
    #[must_use]
    pub const fn is_error(&self) -> bool {
        matches!(self, Self::Error { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manager_request_serialize() {
        let req = ManagerRequest::new(1, ManagerMethod::Ping);
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"id\":1"));
        assert!(json.contains("\"method\":\"Ping\""));
    }

    #[test]
    fn test_manager_request_deserialize() {
        let json = r#"{"id":1,"method":"Ping"}"#;
        let req: ManagerRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.id, 1);
        assert!(matches!(req.method, ManagerMethod::Ping));
    }

    #[test]
    fn test_manager_response_ok() {
        let resp = ManagerResponse::ok(1, serde_json::json!({"status": "ok"}));
        assert!(resp.result.is_ok());
    }

    #[test]
    fn test_manager_response_error() {
        let resp = ManagerResponse::error(1, -32000, "test error");
        assert!(resp.result.is_error());
    }

    #[test]
    fn test_manager_method_list() {
        let json = r#"{"id":1,"method":"List"}"#;
        let req: ManagerRequest = serde_json::from_str(json).unwrap();
        assert!(matches!(req.method, ManagerMethod::List));
    }

    #[test]
    fn test_manager_method_query() {
        let json = r#"{"id":1,"method":"Query","params":{"name":"default"}}"#;
        let req: ManagerRequest = serde_json::from_str(json).unwrap();
        match req.method {
            ManagerMethod::Query { name } => assert_eq!(name, "default"),
            _ => panic!("Expected Query method"),
        }
    }

    #[test]
    fn test_manager_method_unregister() {
        let json = r#"{"id":1,"method":"Unregister","params":{"name":"test"}}"#;
        let req: ManagerRequest = serde_json::from_str(json).unwrap();
        match req.method {
            ManagerMethod::Unregister { name } => assert_eq!(name, "test"),
            _ => panic!("Expected Unregister method"),
        }
    }
}
