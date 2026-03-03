//! Per-client snippet session state (#136).
//!
//! `SnippetSessionState` is a `SessionExtension` that tracks the active
//! snippet for each client. Stored in `client_extensions` (per-client isolated).

use reovim_driver_session::SessionExtension;

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
mod tests {
    use super::*;

    #[test]
    fn test_default_no_active_snippet() {
        let state = SnippetSessionState::default();
        assert!(state.active.is_none());
    }

    #[test]
    fn test_create_returns_default() {
        let state = SnippetSessionState::create();
        assert!(state.active.is_none());
    }

    #[test]
    fn test_set_active_snippet() {
        use {
            crate::{parser, variables::VariableContext},
            reovim_kernel::api::v1::Position,
        };

        let mut state = SnippetSessionState::default();
        let body = parser::parse("$1").unwrap();
        let (_, snippet) =
            ActiveSnippet::expand(&body, Position::origin(), &VariableContext::empty());
        state.active = Some(snippet);
        assert!(state.active.is_some());
    }

    #[test]
    fn test_clear_active_snippet() {
        use {
            crate::{parser, variables::VariableContext},
            reovim_kernel::api::v1::Position,
        };

        let mut state = SnippetSessionState::default();
        let body = parser::parse("$1").unwrap();
        let (_, snippet) =
            ActiveSnippet::expand(&body, Position::origin(), &VariableContext::empty());
        state.active = Some(snippet);
        state.active = None;
        assert!(state.active.is_none());
    }
}
