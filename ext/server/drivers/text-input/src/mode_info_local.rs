//! `ModeInfo` — text-input-specific mode registration metadata.
//!
//! Moved from `reovim-subsys-input-contracts` as part of Plan-14 I.6.

use reovim_kernel::api::v1::{CursorStyle, Mode, ModeId};

/// Information about a mode for registration.
#[derive(Debug, Clone)]
pub struct ModeInfo {
    /// The mode ID.
    pub id: ModeId,
    /// Display name for statusline.
    pub display_name: &'static str,
    /// Cursor style for this mode.
    pub cursor_style: CursorStyle,
    /// Whether this mode accepts character input.
    pub accepts_char_input: bool,
    /// Whether this mode has an active selection.
    pub has_selection: bool,
    /// Parent mode for keybinding inheritance.
    pub inherits_from: Option<ModeId>,
    /// Whether this is the entry/default mode for new sessions.
    pub is_entry: bool,
}

impl ModeInfo {
    /// Create mode info from a mode implementation.
    #[must_use]
    pub fn from_mode<M: Mode>(mode: M) -> Self {
        Self {
            id: mode.id(),
            display_name: mode.display_name(),
            cursor_style: mode.cursor_style(),
            accepts_char_input: mode.accepts_char_input(),
            has_selection: mode.has_selection(),
            inherits_from: mode.inherits_from().map(|m| m.id()),
            is_entry: mode.is_entry(),
        }
    }
}
