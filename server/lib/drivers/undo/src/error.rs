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
mod tests {
    use super::*;

    #[test]
    fn display_serialize() {
        let err = UndoPersistError::Serialize("bad format".into());
        assert!(err.to_string().contains("Serialization"));
        assert!(err.to_string().contains("bad format"));
    }

    #[test]
    fn display_deserialize() {
        let err = UndoPersistError::Deserialize("corrupt data".into());
        assert!(err.to_string().contains("Deserialization"));
        assert!(err.to_string().contains("corrupt data"));
    }

    #[test]
    fn display_io() {
        let err = UndoPersistError::Io("permission denied".into());
        assert!(err.to_string().contains("I/O"));
        assert!(err.to_string().contains("permission denied"));
    }

    #[test]
    fn debug() {
        let err = UndoPersistError::Serialize("test".into());
        let debug = format!("{err:?}");
        assert!(debug.contains("Serialize"));
    }

    #[test]
    fn is_error() {
        let err = UndoPersistError::Io("test".into());
        let _: &dyn std::error::Error = &err;
    }
}
