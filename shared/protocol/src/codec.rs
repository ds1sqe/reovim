//! Serialization helpers for protocol messages.
//!
//! This module provides codec functions for encoding and decoding
//! protocol messages. Currently supports JSON only.

use serde::{Serialize, de::DeserializeOwned};

/// Encode a value to JSON string.
///
/// # Errors
///
/// Returns an error if serialization fails.
pub fn to_json<T: Serialize>(value: &T) -> Result<String, serde_json::Error> {
    serde_json::to_string(value)
}

/// Encode a value to pretty-printed JSON string.
///
/// # Errors
///
/// Returns an error if serialization fails.
pub fn to_json_pretty<T: Serialize>(value: &T) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(value)
}

/// Decode a JSON string to a typed value.
///
/// # Errors
///
/// Returns an error if deserialization fails.
pub fn from_json<T: DeserializeOwned>(s: &str) -> Result<T, serde_json::Error> {
    serde_json::from_str(s)
}

/// Decode a JSON value to a typed value.
///
/// # Errors
///
/// Returns an error if conversion fails.
pub fn from_value<T: DeserializeOwned>(v: serde_json::Value) -> Result<T, serde_json::Error> {
    serde_json::from_value(v)
}

/// Convert a typed value to a JSON value.
///
/// # Errors
///
/// Returns an error if serialization fails.
pub fn to_value<T: Serialize>(value: &T) -> Result<serde_json::Value, serde_json::Error> {
    serde_json::to_value(value)
}

#[cfg(test)]
mod tests {
    use {super::*, crate::v1::types::Position};

    #[test]
    fn test_roundtrip() {
        let pos = Position::new(10, 5);
        let json = to_json(&pos).unwrap();
        let decoded: Position = from_json(&json).unwrap();
        assert_eq!(pos, decoded);
    }

    #[test]
    fn test_to_value() {
        let pos = Position::new(10, 5);
        let value = to_value(&pos).unwrap();
        assert_eq!(value["line"], 10);
        assert_eq!(value["column"], 5);
    }

    #[test]
    fn test_from_value() {
        let value = serde_json::json!({"line": 10, "column": 5});
        let pos: Position = from_value(value).unwrap();
        assert_eq!(pos.line, 10);
        assert_eq!(pos.column, 5);
    }
}
