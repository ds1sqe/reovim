//! Per-client signature help popup state.
//!
//! Stored in the session's `ExtensionMap` as a `SessionExtension`.
//! Displays LSP signature help near the cursor.

use reovim_driver_session::SessionExtension;

/// Per-client signature help state.
///
/// Tracks whether the signature help popup is visible, the label to
/// display, and the origin position (buffer position where signature
/// help was triggered).
#[derive(Debug)]
pub struct SignatureHelpState {
    /// Whether the popup is visible.
    pub active: bool,
    /// Signature label (e.g., `fn foo(x: i32, y: &str) -> bool`).
    pub label: String,
    /// Buffer ID where signature help was triggered.
    pub origin_buffer_id: u64,
    /// Line where signature help was triggered (0-indexed).
    pub origin_line: u32,
    /// Column where signature help was triggered (0-indexed).
    pub origin_col: u32,
}

impl SignatureHelpState {
    /// Show signature help at the given position.
    pub fn show(&mut self, label: String, buffer_id: u64, line: u32, col: u32) {
        self.active = true;
        self.label = label;
        self.origin_buffer_id = buffer_id;
        self.origin_line = line;
        self.origin_col = col;
    }

    /// Dismiss the signature help popup.
    pub fn dismiss(&mut self) {
        self.active = false;
        self.label.clear();
    }
}

impl SessionExtension for SignatureHelpState {
    fn create() -> Self {
        Self {
            active: false,
            label: String::new(),
            origin_buffer_id: 0,
            origin_line: 0,
            origin_col: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_defaults() {
        let state = SignatureHelpState::create();
        assert!(!state.active);
        assert!(state.label.is_empty());
        assert_eq!(state.origin_buffer_id, 0);
        assert_eq!(state.origin_line, 0);
        assert_eq!(state.origin_col, 0);
    }

    #[test]
    fn state_debug() {
        let state = SignatureHelpState::create();
        let debug = format!("{state:?}");
        assert!(debug.contains("SignatureHelpState"));
    }

    #[test]
    fn show_sets_all_fields() {
        let mut state = SignatureHelpState::create();
        state.show("fn foo(x: i32) -> bool".to_owned(), 42, 5, 12);
        assert!(state.active);
        assert_eq!(state.label, "fn foo(x: i32) -> bool");
        assert_eq!(state.origin_buffer_id, 42);
        assert_eq!(state.origin_line, 5);
        assert_eq!(state.origin_col, 12);
    }

    #[test]
    fn show_overwrites_previous() {
        let mut state = SignatureHelpState::create();
        state.show("first".to_owned(), 1, 0, 0);
        state.show("second".to_owned(), 2, 10, 5);
        assert_eq!(state.label, "second");
        assert_eq!(state.origin_buffer_id, 2);
    }

    #[test]
    fn dismiss_clears_state() {
        let mut state = SignatureHelpState::create();
        state.show("fn foo()".to_owned(), 1, 5, 12);
        state.dismiss();
        assert!(!state.active);
        assert!(state.label.is_empty());
    }

    #[test]
    fn dismiss_when_inactive_is_noop() {
        let mut state = SignatureHelpState::create();
        state.dismiss();
        assert!(!state.active);
    }
}
