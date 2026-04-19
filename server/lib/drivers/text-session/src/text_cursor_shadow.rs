//! Driver-owned text cursor shadow for bridge/module consumers.
//!
//! This keeps text-specific cursor coordinates out of opaque subsys-session
//! `CursorSnapshot` while still giving text modules a current `(buffer, line,
//! col)` view for tick-time behavior.

use {reovim_kernel::api::v1::BufferId, reovim_subsys_session::SessionExtension};

/// Text-domain cursor shadow stored in the client extension map.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextCursorShadow {
    /// Active text buffer containing the cursor.
    pub buffer_id: BufferId,
    /// Cursor line (0-indexed).
    pub line: u32,
    /// Cursor column (0-indexed).
    pub col: u32,
    /// Whether the shadow currently contains authoritative data.
    pub valid: bool,
}

impl TextCursorShadow {
    /// Invalid sentinel value.
    #[must_use]
    pub const fn invalid() -> Self {
        Self {
            buffer_id: BufferId::from_raw(0),
            line: 0,
            col: 0,
            valid: false,
        }
    }

    /// Update the shadow from authoritative text cursor state.
    #[allow(clippy::cast_possible_truncation)]
    pub fn update(&mut self, buffer_id: BufferId, line: usize, col: usize) {
        self.buffer_id = buffer_id;
        self.line = line as u32;
        self.col = col as u32;
        self.valid = true;
    }

    /// Clear the shadow back to an invalid sentinel.
    pub const fn clear(&mut self) {
        *self = Self::invalid();
    }
}

impl Default for TextCursorShadow {
    fn default() -> Self {
        Self::invalid()
    }
}

impl SessionExtension for TextCursorShadow {
    fn create() -> Self {
        Self::invalid()
    }
}

#[cfg(test)]
#[path = "text_cursor_shadow_tests.rs"]
mod tests;
