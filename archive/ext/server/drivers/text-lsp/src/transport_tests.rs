use {
    super::*,
    crate::jsonrpc::{Notification, Request, Response},
    serde_json::Value,
    std::io::Cursor,
};

#[tokio::test]
async fn test_recv_request() {
    let input =
        b"Content-Length: 54\r\n\r\n{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"test\",\"params\":null}";
    let mut reader = Cursor::new(input);

    let message = Transport::recv(&mut reader).await.unwrap();
    assert!(message.is_request());

    if let Message::Request(req) = message {
        assert_eq!(req.method, "test");
        assert_eq!(req.id, crate::jsonrpc::Id::Number(1));
    }
}

#[tokio::test]
async fn test_recv_notification() {
    let input = b"Content-Length: 40\r\n\r\n{\"jsonrpc\":\"2.0\",\"method\":\"initialized\"}";
    let mut reader = Cursor::new(input);

    let message = Transport::recv(&mut reader).await.unwrap();
    assert!(message.is_notification());
}

#[tokio::test]
async fn test_send_request() {
    let mut buffer = Vec::new();
    let request = Request::new(1_i64, "test", None);

    Transport::send_request(&mut buffer, &request)
        .await
        .unwrap();

    let output = String::from_utf8(buffer).unwrap();
    assert!(output.starts_with("Content-Length:"));
    assert!(output.contains("\"method\":\"test\""));
}

#[tokio::test]
async fn test_send_notification() {
    let mut buffer = Vec::new();
    let notification = Notification::new("initialized", None);

    Transport::send_notification(&mut buffer, &notification)
        .await
        .unwrap();

    let output = String::from_utf8(buffer).unwrap();
    assert!(output.starts_with("Content-Length:"));
    assert!(output.contains("\"method\":\"initialized\""));
    assert!(!output.contains("\"id\""));
}

#[tokio::test]
async fn test_recv_missing_content_length() {
    let input = b"\r\n{\"jsonrpc\":\"2.0\",\"method\":\"test\"}";
    let mut reader = Cursor::new(input);

    let result = Transport::recv(&mut reader).await;
    assert!(matches!(result, Err(TransportError::MissingContentLength)));
}

#[tokio::test]
async fn test_recv_invalid_content_length() {
    let input = b"Content-Length: abc\r\n\r\n{}";
    let mut reader = Cursor::new(input);

    let result = Transport::recv(&mut reader).await;
    assert!(matches!(result, Err(TransportError::InvalidContentLength(_))));
}

#[tokio::test]
async fn test_recv_with_extra_headers() {
    // LSP spec allows Content-Type header which we should ignore
    let input = b"Content-Length: 40\r\nContent-Type: application/json\r\n\r\n{\"jsonrpc\":\"2.0\",\"method\":\"initialized\"}";
    let mut reader = Cursor::new(input);

    let message = Transport::recv(&mut reader).await.unwrap();
    assert!(message.is_notification());
}

#[tokio::test]
async fn test_recv_closed() {
    let input = b"";
    let mut reader = Cursor::new(input);

    let result = Transport::recv(&mut reader).await;
    assert!(matches!(result, Err(TransportError::Closed)));
}

#[tokio::test]
async fn test_round_trip() {
    // Send a message, then receive it back
    let mut buffer = Vec::new();
    let request = Request::new(42_i64, "textDocument/hover", None);

    Transport::send_request(&mut buffer, &request)
        .await
        .unwrap();

    let mut reader = Cursor::new(buffer);
    let received = Transport::recv(&mut reader).await.unwrap();

    assert!(received.is_request());
    if let Message::Request(req) = received {
        assert_eq!(req.method, "textDocument/hover");
        assert_eq!(req.id, crate::jsonrpc::Id::Number(42));
    }
}

#[tokio::test]
async fn test_round_trip_response() {
    let mut buffer = Vec::new();
    let response = Response::success(1_i64, Value::Null);
    let message = Message::Response(response);

    Transport::send(&mut buffer, &message).await.unwrap();

    let mut reader = Cursor::new(buffer);
    let received = Transport::recv(&mut reader).await.unwrap();
    assert!(received.is_response());
}

#[test]
fn test_transport_error_display() {
    let io_err = TransportError::Io(io::Error::new(io::ErrorKind::BrokenPipe, "broken"));
    assert!(format!("{io_err}").contains("broken"));

    let json_err = TransportError::Json(serde_json::from_str::<Value>("invalid").unwrap_err());
    assert!(format!("{json_err}").contains("JSON error"));

    let missing = TransportError::MissingContentLength;
    assert_eq!(format!("{missing}"), "Missing Content-Length header");

    let invalid = TransportError::InvalidContentLength("xyz".to_string());
    assert!(format!("{invalid}").contains("xyz"));

    let closed = TransportError::Closed;
    assert_eq!(format!("{closed}"), "Connection closed");
}

#[test]
fn test_transport_error_source() {
    use std::error::Error;

    let io_err = TransportError::Io(io::Error::other("test"));
    assert!(io_err.source().is_some());

    let json_err = TransportError::Json(serde_json::from_str::<Value>("invalid").unwrap_err());
    assert!(json_err.source().is_some());

    let missing = TransportError::MissingContentLength;
    assert!(missing.source().is_none());

    let invalid = TransportError::InvalidContentLength("abc".to_string());
    assert!(invalid.source().is_none());

    let closed = TransportError::Closed;
    assert!(closed.source().is_none());
}

#[test]
fn test_transport_error_from_io() {
    let io_err = io::Error::new(io::ErrorKind::BrokenPipe, "test");
    let transport_err: TransportError = io_err.into();
    assert!(matches!(transport_err, TransportError::Io(_)));
}

#[test]
fn test_transport_error_from_json() {
    let json_err = serde_json::from_str::<Value>("invalid").unwrap_err();
    let transport_err: TransportError = json_err.into();
    assert!(matches!(transport_err, TransportError::Json(_)));
}
