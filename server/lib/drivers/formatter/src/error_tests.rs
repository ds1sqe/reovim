use super::*;

#[test]
fn test_command_failed_display() {
    let err = FormatError::CommandFailed {
        command: "rustfmt".to_string(),
        stderr: "syntax error".to_string(),
        exit_code: Some(1),
    };
    let msg = err.to_string();
    assert!(msg.contains("rustfmt"));
    assert!(msg.contains("exit 1"));
    assert!(msg.contains("syntax error"));
}

#[test]
fn test_command_failed_no_exit_code() {
    let err = FormatError::CommandFailed {
        command: "black".to_string(),
        stderr: String::new(),
        exit_code: None,
    };
    let msg = err.to_string();
    assert!(msg.contains("black"));
    assert!(!msg.contains("exit"));
}

#[test]
fn test_command_not_found_display() {
    let err = FormatError::CommandNotFound("stylua".to_string());
    assert_eq!(err.to_string(), "formatter not found: stylua");
}

#[test]
fn test_timeout_display() {
    let err = FormatError::Timeout;
    assert_eq!(err.to_string(), "formatting timed out");
}

#[test]
fn test_lsp_error_display() {
    let err = FormatError::LspError("server crashed".to_string());
    assert!(err.to_string().contains("server crashed"));
}

#[test]
fn test_content_error_display() {
    let err = FormatError::ContentError("invalid utf-8".to_string());
    assert!(err.to_string().contains("invalid utf-8"));
}

#[test]
fn test_debug() {
    let err = FormatError::Timeout;
    let debug = format!("{err:?}");
    assert!(debug.contains("Timeout"));
}

#[test]
fn test_clone() {
    let err = FormatError::Timeout;
    let cloned = err.clone();
    assert_eq!(err, cloned);
}

#[test]
fn test_eq() {
    assert_eq!(FormatError::Timeout, FormatError::Timeout);
    assert_ne!(FormatError::Timeout, FormatError::CommandNotFound("x".to_string()));
}

#[test]
fn test_error_trait() {
    let err: &dyn std::error::Error = &FormatError::Timeout;
    assert!(err.to_string().contains("timed out"));
}
