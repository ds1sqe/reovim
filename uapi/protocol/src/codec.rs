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
#[path = "codec_tests.rs"]
mod tests;
