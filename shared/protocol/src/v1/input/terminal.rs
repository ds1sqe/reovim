//! Terminal event types for RPC protocol.
//!
//! Terminal-level events: resize, focus, paste.

use serde::{Deserialize, Serialize};

/// Terminal resize event (serializable).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResizeEvent {
    /// New width in columns.
    pub width: u16,
    /// New height in rows.
    pub height: u16,
}

impl ResizeEvent {
    /// Create a new resize event.
    #[must_use]
    pub const fn new(width: u16, height: u16) -> Self {
        Self { width, height }
    }
}

/// Focus event kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FocusKind {
    /// Terminal gained focus.
    Gained,
    /// Terminal lost focus.
    Lost,
}

/// Terminal focus event (serializable).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FocusEvent {
    /// Focus kind (gained or lost).
    pub kind: FocusKind,
}

impl FocusEvent {
    /// Create a focus gained event.
    #[must_use]
    pub const fn gained() -> Self {
        Self {
            kind: FocusKind::Gained,
        }
    }

    /// Create a focus lost event.
    #[must_use]
    pub const fn lost() -> Self {
        Self {
            kind: FocusKind::Lost,
        }
    }
}

/// Paste event (serializable).
///
/// Represents bracketed paste data from the terminal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PasteEvent {
    /// The pasted text content.
    pub content: String,
}

impl PasteEvent {
    /// Create a new paste event.
    #[must_use]
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
        }
    }
}

#[cfg(test)]
#[path = "terminal_tests.rs"]
mod tests;
