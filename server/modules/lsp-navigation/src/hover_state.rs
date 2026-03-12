//! Per-client hover popup state.
//!
//! Stored in the session's `ExtensionMap` as a `SessionExtension`.
//! The hover popup displays LSP hover information near the hovered symbol.

use reovim_driver_session::SessionExtension;

/// Content format for hover display.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoverContentType {
    /// Plain text content.
    PlainText,
    /// Markdown-formatted content.
    Markdown,
}

/// Per-client hover popup state.
///
/// Tracks whether the hover popup is visible, the content to display,
/// the content type, and the origin position (buffer position where
/// the hover was triggered).
#[derive(Debug)]
pub struct HoverState {
    /// Whether the hover popup is visible.
    pub active: bool,
    /// Hover content text.
    pub content: String,
    /// Content format (plaintext or markdown).
    pub content_type: HoverContentType,
    /// Buffer ID where hover was triggered.
    pub origin_buffer_id: u64,
    /// Line where hover was triggered (0-indexed).
    pub origin_line: u32,
    /// Column where hover was triggered (0-indexed).
    pub origin_col: u32,
}

impl HoverState {
    /// Show hover content at the given position.
    pub fn show(
        &mut self,
        content: String,
        content_type: HoverContentType,
        buffer_id: u64,
        line: u32,
        col: u32,
    ) {
        self.active = true;
        self.content = content;
        self.content_type = content_type;
        self.origin_buffer_id = buffer_id;
        self.origin_line = line;
        self.origin_col = col;
    }

    /// Dismiss the hover popup.
    pub fn dismiss(&mut self) {
        self.active = false;
        self.content.clear();
    }
}

impl SessionExtension for HoverState {
    fn create() -> Self {
        Self {
            active: false,
            content: String::new(),
            content_type: HoverContentType::PlainText,
            origin_buffer_id: 0,
            origin_line: 0,
            origin_col: 0,
        }
    }
}

#[cfg(test)]
#[path = "hover_state_tests.rs"]
mod tests;
