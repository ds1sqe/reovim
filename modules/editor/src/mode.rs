//! Editor modes - Normal and Insert.
//!
//! These are the two fundamental modes for basic text editing:
//! - **Normal**: Navigation mode with cursor movement commands
//! - **Insert**: Text input mode where characters are inserted

use {
    reovim_driver_display::{CursorStyle, ModeDisplay},
    reovim_driver_input::ModeInput,
    reovim_kernel::api::v1::{Mode, ModeId, ModuleId},
};

/// Module identifier for the editor module.
pub const EDITOR_MODULE: ModuleId = ModuleId::new("editor");

/// Standard editor modes.
///
/// These implement the core vim-style modes:
/// - `Normal`: Block cursor, movement commands, doesn't accept direct char input
/// - `Insert`: Bar cursor, accepts character input directly
///
/// # Trait Implementations
///
/// Each mode implements three traits following the kernel's separation:
/// - [`Mode`]: Provides identity via `ModeId`
/// - [`ModeDisplay`]: Cursor style and status text
/// - [`ModeInput`]: Whether the mode accepts character input
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum EditorMode {
    /// Normal mode - navigation and commands.
    #[default]
    Normal,
    /// Insert mode - text input.
    Insert,
}

impl EditorMode {
    /// Mode ID for Normal mode.
    pub const NORMAL_ID: ModeId = ModeId::new(EDITOR_MODULE, "normal");

    /// Mode ID for Insert mode.
    pub const INSERT_ID: ModeId = ModeId::new(EDITOR_MODULE, "insert");

    /// Get the mode ID for this mode.
    #[must_use]
    pub const fn mode_id(&self) -> ModeId {
        match self {
            Self::Normal => Self::NORMAL_ID,
            Self::Insert => Self::INSERT_ID,
        }
    }

    /// Check if this is Normal mode.
    #[must_use]
    pub const fn is_normal(&self) -> bool {
        matches!(self, Self::Normal)
    }

    /// Check if this is Insert mode.
    #[must_use]
    pub const fn is_insert(&self) -> bool {
        matches!(self, Self::Insert)
    }
}

// =============================================================================
// Trait Implementations
// =============================================================================

impl Mode for EditorMode {
    fn id(&self) -> ModeId {
        self.mode_id()
    }
}

impl ModeDisplay for EditorMode {
    fn cursor_style(&self) -> CursorStyle {
        match self {
            Self::Normal => CursorStyle::Block,
            Self::Insert => CursorStyle::Bar,
        }
    }

    fn status_text(&self) -> &'static str {
        match self {
            Self::Normal => "NORMAL",
            Self::Insert => "INSERT",
        }
    }
}

impl ModeInput for EditorMode {
    fn accepts_char_input(&self) -> bool {
        matches!(self, Self::Insert)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_editor_mode_ids() {
        assert_eq!(EditorMode::Normal.id(), EditorMode::NORMAL_ID);
        assert_eq!(EditorMode::Insert.id(), EditorMode::INSERT_ID);
    }

    #[test]
    fn test_editor_mode_id_module() {
        assert_eq!(EditorMode::NORMAL_ID.module(), &EDITOR_MODULE);
        assert_eq!(EditorMode::INSERT_ID.module(), &EDITOR_MODULE);
    }

    #[test]
    fn test_editor_mode_id_names() {
        assert_eq!(EditorMode::NORMAL_ID.name(), "normal");
        assert_eq!(EditorMode::INSERT_ID.name(), "insert");
    }

    #[test]
    fn test_editor_mode_cursor_style() {
        assert_eq!(EditorMode::Normal.cursor_style(), CursorStyle::Block);
        assert_eq!(EditorMode::Insert.cursor_style(), CursorStyle::Bar);
    }

    #[test]
    fn test_editor_mode_status_text() {
        assert_eq!(EditorMode::Normal.status_text(), "NORMAL");
        assert_eq!(EditorMode::Insert.status_text(), "INSERT");
    }

    #[test]
    fn test_editor_mode_accepts_char_input() {
        assert!(!EditorMode::Normal.accepts_char_input());
        assert!(EditorMode::Insert.accepts_char_input());
    }

    #[test]
    fn test_editor_mode_is_methods() {
        assert!(EditorMode::Normal.is_normal());
        assert!(!EditorMode::Normal.is_insert());
        assert!(!EditorMode::Insert.is_normal());
        assert!(EditorMode::Insert.is_insert());
    }

    #[test]
    fn test_editor_mode_default() {
        assert_eq!(EditorMode::default(), EditorMode::Normal);
    }

    #[test]
    fn test_editor_mode_clone() {
        let mode = EditorMode::Normal;
        let cloned = mode;
        assert_eq!(mode, cloned);
    }
}
