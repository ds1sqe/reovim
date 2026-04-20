//! Per-client snippet session state (#136).
//!
//! `SnippetSessionState` is a `SessionExtension` that tracks the active
//! snippet for each client. Stored in `client_extensions` (per-client isolated).

use reovim_driver_text_session::SessionExtension;

use crate::engine::ActiveSnippet;

/// Per-client state for active snippet sessions.
///
/// Each client has its own cursor and therefore its own active snippet.
/// Stored in `client_extensions` via `get_or_insert::<SnippetSessionState>()`.
#[derive(Default)]
pub struct SnippetSessionState {
    /// The currently active snippet, if any.
    pub active: Option<ActiveSnippet>,
}

impl SessionExtension for SnippetSessionState {
    fn create() -> Self {
        Self::default()
    }
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
