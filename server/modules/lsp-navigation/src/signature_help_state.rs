//! Per-client signature help popup state.
//!
//! Stored in the session's `ExtensionMap` as a `SessionExtension`.
//! Displays LSP signature help near the cursor.

use reovim_driver_text_session::SessionExtension;

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
#[path = "signature_help_state_tests.rs"]
mod tests;
