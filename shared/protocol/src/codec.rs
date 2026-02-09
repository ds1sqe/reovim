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

    #[test]
    fn test_to_json_pretty() {
        let pos = Position::new(3, 7);
        let pretty = to_json_pretty(&pos).unwrap();
        assert!(pretty.contains('\n'));
        assert!(pretty.contains("\"line\": 3"));
        assert!(pretty.contains("\"column\": 7"));
    }

    #[test]
    fn test_from_json_error() {
        let result: Result<Position, _> = from_json("not json");
        assert!(result.is_err());
    }

    #[test]
    fn test_from_value_error() {
        let value = serde_json::json!("not a position");
        let result: Result<Position, _> = from_value(value);
        assert!(result.is_err());
    }

    #[test]
    fn test_to_json_empty_struct() {
        let val = serde_json::json!({});
        let json = to_json(&val).unwrap();
        assert_eq!(json, "{}");
    }

    #[test]
    fn test_to_value_and_back() {
        let pos = Position::new(0, 0);
        let value = to_value(&pos).unwrap();
        let decoded: Position = from_value(value).unwrap();
        assert_eq!(decoded, pos);
    }
}
