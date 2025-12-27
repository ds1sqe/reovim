//! LSP progress events for the event bus.
//!
//! These events are emitted by the saturator and handled by the LSP plugin,
//! which converts them to notification plugin events.

/// LSP progress started
#[derive(Debug, Clone)]
pub struct LspProgressBegin {
    /// Progress ID (from token)
    pub id: String,
    /// Operation title
    pub title: String,
    /// Optional message (e.g., "294/295")
    pub message: Option<String>,
    /// Percentage (0-100)
    pub percentage: Option<u8>,
}

/// LSP progress update
#[derive(Debug, Clone)]
pub struct LspProgressReport {
    /// Progress ID (from token)
    pub id: String,
    /// Operation title
    pub title: String,
    /// Optional message
    pub message: Option<String>,
    /// Percentage (0-100)
    pub percentage: Option<u8>,
}

/// LSP progress completed
#[derive(Debug, Clone)]
pub struct LspProgressEnd {
    /// Progress ID (from token)
    pub id: String,
    /// Optional completion message
    pub message: Option<String>,
}
