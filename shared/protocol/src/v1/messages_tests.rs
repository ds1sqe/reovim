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
