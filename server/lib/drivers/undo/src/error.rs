//! Undo persistence error types.

use std::fmt;

/// Errors that can occur during undo persistence.
#[derive(Debug)]
pub enum UndoPersistError {
    /// Serialization error.
    Serialize(String),
    /// Deserialization error.
    Deserialize(String),
    /// I/O error.
    Io(String),
}

impl fmt::Display for UndoPersistError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Serialize(e) => write!(f, "Serialization error: {e}"),
            Self::Deserialize(e) => write!(f, "Deserialization error: {e}"),
            Self::Io(e) => write!(f, "I/O error: {e}"),
        }
    }
}

impl std::error::Error for UndoPersistError {}

#[cfg(test)]
#[path = "error_tests.rs"]
mod tests;
