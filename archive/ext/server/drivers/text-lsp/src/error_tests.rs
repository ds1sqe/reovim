use super::*;

#[test]
fn test_error_display() {
    let err = LspError::NotInitialized;
    assert_eq!(format!("{err}"), "LSP server not initialized");

    let err = LspError::ServerError {
        code: -32600,
        message: "Invalid request".to_string(),
    };
    let display = format!("{err}");
    assert!(display.contains("-32600"));
    assert!(display.contains("Invalid request"));
}

#[test]
fn test_spawn_failed_display() {
    let err = LspError::SpawnFailed("command not found".to_string());
    assert!(format!("{err}").contains("command not found"));
}

#[test]
fn test_transport_error_display() {
    let err = LspError::TransportError("connection reset".to_string());
    assert!(format!("{err}").contains("connection reset"));
}

#[test]
fn test_timeout_display() {
    let err = LspError::Timeout;
    assert_eq!(format!("{err}"), "LSP request timed out");
}

#[test]
fn test_channel_closed_display() {
    let err = LspError::ChannelClosed;
    assert_eq!(format!("{err}"), "LSP channel closed");
}

#[test]
fn test_serialization_display() {
    let err = LspError::Serialization("invalid JSON".to_string());
    assert!(format!("{err}").contains("invalid JSON"));
}

#[test]
fn test_server_not_running_display() {
    let err = LspError::ServerNotRunning;
    assert_eq!(format!("{err}"), "LSP server not running");
}

#[test]
fn test_invalid_response_display() {
    let err = LspError::InvalidResponse("unexpected field".to_string());
    assert!(format!("{err}").contains("unexpected field"));
}

#[test]
fn test_error_is_error_trait() {
    fn assert_error<E: std::error::Error>() {}
    assert_error::<LspError>();
}

#[test]
fn test_from_transport_error() {
    let transport_err = crate::transport::TransportError::Closed;
    let lsp_err: LspError = transport_err.into();
    assert!(matches!(lsp_err, LspError::TransportError(_)));
    assert!(format!("{lsp_err}").contains("Connection closed"));
}

#[test]
fn test_from_serde_json_error() {
    let json_err = serde_json::from_str::<serde_json::Value>("invalid").unwrap_err();
    let lsp_err: LspError = json_err.into();
    assert!(matches!(lsp_err, LspError::Serialization(_)));
}
