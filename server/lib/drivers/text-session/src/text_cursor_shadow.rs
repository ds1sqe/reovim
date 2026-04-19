//! Driver-owned text cursor shadow for bridge/module consumers.
//!
//! This keeps text-specific cursor coordinates out of opaque subsys-session
//! `CursorSnapshot` while still giving text modules a current `(buffer, line,
//! col)` view for tick-time behavior.

use {
    reovim_kernel::api::v1::BufferId,
    reovim_subsys_session::{CursorSnapshot, SessionExtension},
};

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
    const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x00000100000001B3;

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

    /// Build an opaque cursor snapshot token from the current shadow state.
    #[must_use]
    pub fn cursor_snapshot(&self) -> CursorSnapshot {
        if !self.valid {
            return CursorSnapshot::SENTINEL;
        }

        let mut hash = Self::FNV_OFFSET_BASIS;
        for byte in (self.buffer_id.as_usize() as u64)
            .to_le_bytes()
            .into_iter()
            .chain((self.line as u64).to_le_bytes())
            .chain((self.col as u64).to_le_bytes())
        {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(Self::FNV_PRIME);
        }

        if hash == 0 {
            hash = 1;
        }

        CursorSnapshot::from_bytes(hash.to_le_bytes())
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
