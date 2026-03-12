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
