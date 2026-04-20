//! Typed notification deserialization helpers.
//!
//! Available when the `serde` feature is enabled. Eliminates manual JSON
//! parsing boilerplate in client modules.
//!
//! # Example
//!
//! ```ignore
//! #[derive(serde::Deserialize)]
//! struct WhichKeyPayload { active: bool, prefix: String }
//!
//! fn on_notification(&mut self, data: &str) {
//!     if let Some(payload) = parse_notification::<WhichKeyPayload>(data) {
//!         self.active = payload.active;
//!     }
//! }
//! ```

/// Deserialize a notification JSON string into a typed struct.
///
/// Returns `None` if deserialization fails (lenient: modules silently
/// ignore notifications they don't understand).
#[must_use]
pub fn parse_notification<T: serde::de::DeserializeOwned>(data: &str) -> Option<T> {
    serde_json::from_str(data).ok()
}

/// Deserialize and extract a specific field from a notification JSON object.
///
/// Returns `None` if the JSON is invalid, the field is missing, or the field
/// value doesn't match type `T`.
#[must_use]
pub fn parse_notification_field<T: serde::de::DeserializeOwned>(
    data: &str,
    field: &str,
) -> Option<T> {
    let value: serde_json::Value = serde_json::from_str(data).ok()?;
    serde_json::from_value(value.get(field)?.clone()).ok()
}

#[cfg(test)]
#[path = "notification_tests.rs"]
mod tests;
