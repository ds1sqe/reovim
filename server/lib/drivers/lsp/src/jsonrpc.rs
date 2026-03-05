//! JSON-RPC 2.0 types for LSP communication.
//!
//! LSP uses JSON-RPC 2.0 as its transport protocol.
//! See: <https://www.jsonrpc.org/specification>

use {
    serde::{Deserialize, Serialize},
    serde_json::Value,
};

/// JSON-RPC request/response ID.
///
/// Can be a number or a string per the JSON-RPC 2.0 specification.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Id {
    /// Numeric ID.
    Number(i64),
    /// String ID.
    String(String),
}

impl From<i64> for Id {
    fn from(id: i64) -> Self {
        Self::Number(id)
    }
}

impl From<u64> for Id {
    fn from(id: u64) -> Self {
        #[allow(clippy::cast_possible_wrap)]
        Self::Number(id as i64)
    }
}

/// A JSON-RPC message (request, response, or notification).
///
/// Ordering matters for `#[serde(untagged)]`: Request first (has id+method),
/// Response second (has id+result/error), Notification last (method only).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Message {
    /// A request from client to server (or server to client).
    Request(Request),
    /// A response to a request.
    Response(Response),
    /// A notification (no response expected).
    Notification(Notification),
}

impl Message {
    /// Check if this message is a request.
    #[must_use]
    pub const fn is_request(&self) -> bool {
        matches!(self, Self::Request(_))
    }

    /// Check if this message is a response.
    #[must_use]
    pub const fn is_response(&self) -> bool {
        matches!(self, Self::Response(_))
    }

    /// Check if this message is a notification.
    #[must_use]
    pub const fn is_notification(&self) -> bool {
        matches!(self, Self::Notification(_))
    }
}

/// A JSON-RPC request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    /// JSON-RPC version, always "2.0".
    pub jsonrpc: String,
    /// Request ID for matching responses.
    pub id: Id,
    /// Method name to invoke.
    pub method: String,
    /// Method parameters.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

impl Request {
    /// Create a new request.
    #[must_use]
    pub fn new(id: impl Into<Id>, method: impl Into<String>, params: Option<Value>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: id.into(),
            method: method.into(),
            params,
        }
    }
}

/// A JSON-RPC response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    /// JSON-RPC version, always "2.0".
    pub jsonrpc: String,
    /// Request ID this response is for.
    pub id: Id,
    /// Result on success.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    /// Error on failure.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<Error>,
}

impl Response {
    /// Create a success response.
    #[must_use]
    pub fn success(id: impl Into<Id>, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: id.into(),
            result: Some(result),
            error: None,
        }
    }

    /// Create an error response.
    #[must_use]
    pub fn error(id: impl Into<Id>, error: Error) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: id.into(),
            result: None,
            error: Some(error),
        }
    }

    /// Check if this response is an error.
    #[must_use]
    pub const fn is_error(&self) -> bool {
        self.error.is_some()
    }
}

/// A JSON-RPC notification (no response expected).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    /// JSON-RPC version, always "2.0".
    pub jsonrpc: String,
    /// Method name.
    pub method: String,
    /// Method parameters.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

impl Notification {
    /// Create a new notification.
    #[must_use]
    pub fn new(method: impl Into<String>, params: Option<Value>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            method: method.into(),
            params,
        }
    }
}

/// A JSON-RPC error.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Error {
    /// Error code.
    pub code: i32,
    /// Error message.
    pub message: String,
    /// Additional error data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl Error {
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
    pub fn with_data(code: i32, message: impl Into<String>, data: Value) -> Self {
        Self {
            code,
            message: message.into(),
            data: Some(data),
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "JSON-RPC error {}: {}", self.code, self.message)
    }
}

impl std::error::Error for Error {}

/// Standard JSON-RPC error codes.
pub mod error_codes {
    /// Invalid JSON was received by the server.
    pub const PARSE_ERROR: i32 = -32700;
    /// The JSON sent is not a valid Request object.
    pub const INVALID_REQUEST: i32 = -32600;
    /// The method does not exist / is not available.
    pub const METHOD_NOT_FOUND: i32 = -32601;
    /// Invalid method parameter(s).
    pub const INVALID_PARAMS: i32 = -32602;
    /// Internal JSON-RPC error.
    pub const INTERNAL_ERROR: i32 = -32603;

    // LSP-specific error codes (range -32099 to -32000)

    /// Server not initialized.
    pub const SERVER_NOT_INITIALIZED: i32 = -32002;
    /// Unknown protocol version.
    pub const UNKNOWN_ERROR_CODE: i32 = -32001;

    // LSP request errors (range -32899 to -32800)

    /// Request was cancelled.
    pub const REQUEST_CANCELLED: i32 = -32800;
    /// Content was modified.
    pub const CONTENT_MODIFIED: i32 = -32801;
    /// Server was cancelled.
    pub const SERVER_CANCELLED: i32 = -32802;
    /// Request failed.
    pub const REQUEST_FAILED: i32 = -32803;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_new() {
        let request = Request::new(1_i64, "initialize", Some(serde_json::json!({"foo": "bar"})));
        assert_eq!(request.jsonrpc, "2.0");
        assert_eq!(request.id, Id::Number(1));
        assert_eq!(request.method, "initialize");
        assert!(request.params.is_some());
    }

    #[test]
    fn test_request_serialization() {
        let request = Request::new(1_i64, "initialize", Some(serde_json::json!({"foo": "bar"})));
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"jsonrpc\":\"2.0\""));
        assert!(json.contains("\"id\":1"));
        assert!(json.contains("\"method\":\"initialize\""));
    }

    #[test]
    fn test_notification_new() {
        let notification = Notification::new("initialized", None);
        assert_eq!(notification.jsonrpc, "2.0");
        assert_eq!(notification.method, "initialized");
        assert!(notification.params.is_none());
    }

    #[test]
    fn test_notification_serialization() {
        let notification = Notification::new("initialized", None);
        let json = serde_json::to_string(&notification).unwrap();
        assert!(json.contains("\"jsonrpc\":\"2.0\""));
        assert!(json.contains("\"method\":\"initialized\""));
        assert!(!json.contains("\"id\""));
        assert!(!json.contains("\"params\""));
    }

    #[test]
    fn test_response_success() {
        let response = Response::success(1_i64, serde_json::json!({"result": "ok"}));
        assert!(!response.is_error());
        assert!(response.result.is_some());
        assert!(response.error.is_none());
    }

    #[test]
    fn test_response_error() {
        let error = Error::new(error_codes::METHOD_NOT_FOUND, "Method not found");
        let response = Response::error(1_i64, error);
        assert!(response.is_error());
        assert!(response.result.is_none());
        assert!(response.error.is_some());
    }

    #[test]
    fn test_message_parsing_request() {
        let json = r#"{"jsonrpc":"2.0","id":1,"method":"test","params":null}"#;
        let message: Message = serde_json::from_str(json).unwrap();
        assert!(message.is_request());
        assert!(!message.is_response());
        assert!(!message.is_notification());
    }

    #[test]
    fn test_message_parsing_response() {
        let json = r#"{"jsonrpc":"2.0","id":1,"result":{}}"#;
        let message: Message = serde_json::from_str(json).unwrap();
        assert!(message.is_response());
        assert!(!message.is_request());
        assert!(!message.is_notification());
    }

    #[test]
    fn test_message_parsing_notification() {
        let json = r#"{"jsonrpc":"2.0","method":"test"}"#;
        let message: Message = serde_json::from_str(json).unwrap();
        assert!(message.is_notification());
        assert!(!message.is_request());
        assert!(!message.is_response());
    }

    #[test]
    fn test_id_from_i64() {
        let id = Id::from(42_i64);
        assert_eq!(id, Id::Number(42));
    }

    #[test]
    fn test_id_from_u64() {
        let id = Id::from(42_u64);
        assert_eq!(id, Id::Number(42));
    }

    #[test]
    fn test_error_new() {
        let error = Error::new(error_codes::PARSE_ERROR, "Parse error");
        assert_eq!(error.code, error_codes::PARSE_ERROR);
        assert_eq!(error.message, "Parse error");
        assert!(error.data.is_none());
    }

    #[test]
    fn test_error_with_data() {
        let data = serde_json::json!({"detail": "unexpected token"});
        let error = Error::with_data(error_codes::PARSE_ERROR, "Parse error", data);
        assert_eq!(error.code, error_codes::PARSE_ERROR);
        assert!(error.data.is_some());
    }

    #[test]
    fn test_error_display() {
        let error = Error::new(error_codes::METHOD_NOT_FOUND, "Method not found");
        let display = format!("{error}");
        assert_eq!(display, "JSON-RPC error -32601: Method not found");
    }

    #[test]
    fn test_error_is_error_trait() {
        fn assert_error<E: std::error::Error>() {}
        assert_error::<Error>();
    }

    #[test]
    fn test_message_is_predicates() {
        let req = Message::Request(Request::new(1_i64, "test", None));
        assert!(req.is_request());

        let resp = Message::Response(Response::success(1_i64, Value::Null));
        assert!(resp.is_response());

        let notif = Message::Notification(Notification::new("test", None));
        assert!(notif.is_notification());
    }

    #[test]
    fn test_response_is_error_predicate() {
        let success = Response::success(1_i64, Value::Null);
        assert!(!success.is_error());

        let error = Response::error(1_i64, Error::new(-1, "fail"));
        assert!(error.is_error());
    }

    #[test]
    fn test_error_codes_values() {
        assert_eq!(error_codes::PARSE_ERROR, -32700);
        assert_eq!(error_codes::INVALID_REQUEST, -32600);
        assert_eq!(error_codes::METHOD_NOT_FOUND, -32601);
        assert_eq!(error_codes::INVALID_PARAMS, -32602);
        assert_eq!(error_codes::INTERNAL_ERROR, -32603);
        assert_eq!(error_codes::SERVER_NOT_INITIALIZED, -32002);
        assert_eq!(error_codes::UNKNOWN_ERROR_CODE, -32001);
        assert_eq!(error_codes::REQUEST_CANCELLED, -32800);
        assert_eq!(error_codes::CONTENT_MODIFIED, -32801);
        assert_eq!(error_codes::SERVER_CANCELLED, -32802);
        assert_eq!(error_codes::REQUEST_FAILED, -32803);
    }

    #[test]
    fn test_request_no_params_serialization() {
        let request = Request::new(1_i64, "shutdown", None);
        let json = serde_json::to_string(&request).unwrap();
        assert!(!json.contains("\"params\""));
    }

    #[test]
    fn test_notification_with_params() {
        let notification =
            Notification::new("$/progress", Some(serde_json::json!({"token": "123"})));
        assert!(notification.params.is_some());
        let json = serde_json::to_string(&notification).unwrap();
        assert!(json.contains("\"params\""));
    }

    #[test]
    fn test_id_clone_eq_hash() {
        use std::collections::HashSet;
        let a = Id::Number(1);
        let b = a.clone();
        assert_eq!(a, b);

        let mut set = HashSet::new();
        set.insert(a);
        set.insert(b);
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn test_id_string_variant() {
        let id = Id::String("abc-123".to_string());
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "\"abc-123\"");

        let parsed: Id = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, id);
    }
}
