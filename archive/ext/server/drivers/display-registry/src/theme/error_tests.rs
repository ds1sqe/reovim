use std::{error::Error as _, io};

use super::ThemeError;

#[test]
fn display_io() {
    let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
    let err = ThemeError::Io(io_err);
    let msg = err.to_string();
    assert!(msg.starts_with("IO error:"), "msg = {msg}");
    assert!(msg.contains("file not found"), "msg = {msg}");
}

#[test]
fn display_parse() {
    let inner: Box<dyn std::error::Error + Send + Sync> = "boom".into();
    let err = ThemeError::Parse(inner);
    let msg = err.to_string();
    assert!(msg.starts_with("TOML parse error:"), "msg = {msg}");
    assert!(msg.contains("boom"), "msg = {msg}");
}

#[test]
fn display_invalid_color() {
    let err = ThemeError::InvalidColor {
        key: "keyword".to_string(),
        value: "not-a-color".to_string(),
    };
    assert_eq!(err.to_string(), "Invalid color 'not-a-color' for key 'keyword'");
}

#[test]
fn display_palette_not_found() {
    let err = ThemeError::PaletteNotFound {
        key: "keyword".to_string(),
        reference: "my_red".to_string(),
    };
    assert_eq!(err.to_string(), "Palette color 'my_red' not found for key 'keyword'");
}

#[test]
fn display_invalid_base() {
    let err = ThemeError::InvalidBase {
        name: "no-such-theme".to_string(),
    };
    assert_eq!(err.to_string(), "Invalid base theme: 'no-such-theme'");
}

#[test]
fn source_io_is_some() {
    let io_err = io::Error::new(io::ErrorKind::NotFound, "test");
    let err = ThemeError::Io(io_err);
    assert!(err.source().is_some());
}

#[test]
fn source_parse_is_some() {
    let inner: Box<dyn std::error::Error + Send + Sync> = "boom".into();
    let err = ThemeError::Parse(inner);
    assert!(err.source().is_some());
}

#[test]
fn source_invalid_color_is_none() {
    let err = ThemeError::InvalidColor {
        key: "k".to_string(),
        value: "v".to_string(),
    };
    assert!(err.source().is_none());
}

#[test]
fn source_palette_not_found_is_none() {
    let err = ThemeError::PaletteNotFound {
        key: "k".to_string(),
        reference: "r".to_string(),
    };
    assert!(err.source().is_none());
}

#[test]
fn source_invalid_base_is_none() {
    let err = ThemeError::InvalidBase {
        name: "x".to_string(),
    };
    assert!(err.source().is_none());
}

#[test]
fn from_io_error() {
    let io_err = io::Error::new(io::ErrorKind::PermissionDenied, "denied");
    let err: ThemeError = io_err.into();
    assert!(matches!(err, ThemeError::Io(_)));
}

#[test]
fn debug_formats() {
    let err = ThemeError::InvalidBase {
        name: "x".to_string(),
    };
    let dbg = format!("{err:?}");
    assert!(dbg.contains("InvalidBase"));
}
