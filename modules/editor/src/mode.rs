//! Editor modes - Normal, Insert, and Visual.
//!
//! These are the fundamental modes for vim-style text editing:
//! - **Normal**: Navigation mode with cursor movement commands
//! - **Insert**: Text input mode where characters are inserted
//! - **Visual**: Selection mode for character-wise selection
//! - **`VisualLine`**: Selection mode for line-wise selection
//! - **`VisualBlock`**: Selection mode for block (rectangular) selection

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
/// - `Visual`: Block cursor, character-wise selection
/// - `VisualLine`: Block cursor, line-wise selection
/// - `VisualBlock`: Block cursor, block (rectangular) selection
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
    /// Visual mode - character-wise selection.
    Visual,
    /// Visual line mode - line-wise selection.
    VisualLine,
    /// Visual block mode - block (rectangular) selection.
    VisualBlock,
}

impl EditorMode {
    /// Mode ID for Normal mode.
    pub const NORMAL_ID: ModeId = ModeId::new(EDITOR_MODULE, "normal");

    /// Mode ID for Insert mode.
    pub const INSERT_ID: ModeId = ModeId::new(EDITOR_MODULE, "insert");

    /// Mode ID for Visual mode (character-wise).
    pub const VISUAL_ID: ModeId = ModeId::new(EDITOR_MODULE, "visual");

    /// Mode ID for Visual Line mode.
    pub const VISUAL_LINE_ID: ModeId = ModeId::new(EDITOR_MODULE, "visual-line");

    /// Mode ID for Visual Block mode.
    pub const VISUAL_BLOCK_ID: ModeId = ModeId::new(EDITOR_MODULE, "visual-block");

    /// Get the mode ID for this mode.
    #[must_use]
    pub const fn mode_id(&self) -> ModeId {
        match self {
            Self::Normal => Self::NORMAL_ID,
            Self::Insert => Self::INSERT_ID,
            Self::Visual => Self::VISUAL_ID,
            Self::VisualLine => Self::VISUAL_LINE_ID,
            Self::VisualBlock => Self::VISUAL_BLOCK_ID,
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

    /// Check if this is Visual mode (character-wise).
    #[must_use]
    pub const fn is_visual(&self) -> bool {
        matches!(self, Self::Visual)
    }

    /// Check if this is Visual Line mode.
    #[must_use]
    pub const fn is_visual_line(&self) -> bool {
        matches!(self, Self::VisualLine)
    }

    /// Check if this is Visual Block mode.
    #[must_use]
    pub const fn is_visual_block(&self) -> bool {
        matches!(self, Self::VisualBlock)
    }

    /// Check if this is any visual mode (Visual, `VisualLine`, or `VisualBlock`).
    #[must_use]
    pub const fn is_any_visual(&self) -> bool {
        matches!(self, Self::Visual | Self::VisualLine | Self::VisualBlock)
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
            Self::Normal | Self::Visual | Self::VisualLine | Self::VisualBlock => {
                CursorStyle::Block
            }
            Self::Insert => CursorStyle::Bar,
        }
    }

    fn status_text(&self) -> &'static str {
        match self {
            Self::Normal => "NORMAL",
            Self::Insert => "INSERT",
            Self::Visual => "VISUAL",
            Self::VisualLine => "V-LINE",
            Self::VisualBlock => "V-BLOCK",
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
        assert_eq!(EditorMode::Visual.id(), EditorMode::VISUAL_ID);
        assert_eq!(EditorMode::VisualLine.id(), EditorMode::VISUAL_LINE_ID);
        assert_eq!(EditorMode::VisualBlock.id(), EditorMode::VISUAL_BLOCK_ID);
    }

    #[test]
    fn test_editor_mode_id_module() {
        assert_eq!(EditorMode::NORMAL_ID.module(), &EDITOR_MODULE);
        assert_eq!(EditorMode::INSERT_ID.module(), &EDITOR_MODULE);
        assert_eq!(EditorMode::VISUAL_ID.module(), &EDITOR_MODULE);
        assert_eq!(EditorMode::VISUAL_LINE_ID.module(), &EDITOR_MODULE);
        assert_eq!(EditorMode::VISUAL_BLOCK_ID.module(), &EDITOR_MODULE);
    }

    #[test]
    fn test_editor_mode_id_names() {
        assert_eq!(EditorMode::NORMAL_ID.name(), "normal");
        assert_eq!(EditorMode::INSERT_ID.name(), "insert");
        assert_eq!(EditorMode::VISUAL_ID.name(), "visual");
        assert_eq!(EditorMode::VISUAL_LINE_ID.name(), "visual-line");
        assert_eq!(EditorMode::VISUAL_BLOCK_ID.name(), "visual-block");
    }

    #[test]
    fn test_editor_mode_cursor_style() {
        assert_eq!(EditorMode::Normal.cursor_style(), CursorStyle::Block);
        assert_eq!(EditorMode::Insert.cursor_style(), CursorStyle::Bar);
        assert_eq!(EditorMode::Visual.cursor_style(), CursorStyle::Block);
        assert_eq!(EditorMode::VisualLine.cursor_style(), CursorStyle::Block);
        assert_eq!(EditorMode::VisualBlock.cursor_style(), CursorStyle::Block);
    }

    #[test]
    fn test_editor_mode_status_text() {
        assert_eq!(EditorMode::Normal.status_text(), "NORMAL");
        assert_eq!(EditorMode::Insert.status_text(), "INSERT");
        assert_eq!(EditorMode::Visual.status_text(), "VISUAL");
        assert_eq!(EditorMode::VisualLine.status_text(), "V-LINE");
        assert_eq!(EditorMode::VisualBlock.status_text(), "V-BLOCK");
    }

    #[test]
    fn test_editor_mode_accepts_char_input() {
        assert!(!EditorMode::Normal.accepts_char_input());
        assert!(EditorMode::Insert.accepts_char_input());
        assert!(!EditorMode::Visual.accepts_char_input());
        assert!(!EditorMode::VisualLine.accepts_char_input());
        assert!(!EditorMode::VisualBlock.accepts_char_input());
    }

    #[test]
    fn test_editor_mode_is_methods() {
        assert!(EditorMode::Normal.is_normal());
        assert!(!EditorMode::Normal.is_insert());
        assert!(!EditorMode::Insert.is_normal());
        assert!(EditorMode::Insert.is_insert());
    }

    #[test]
    fn test_visual_mode_is_methods() {
        assert!(EditorMode::Visual.is_visual());
        assert!(!EditorMode::Visual.is_visual_line());
        assert!(!EditorMode::Visual.is_visual_block());

        assert!(!EditorMode::VisualLine.is_visual());
        assert!(EditorMode::VisualLine.is_visual_line());
        assert!(!EditorMode::VisualLine.is_visual_block());

        assert!(!EditorMode::VisualBlock.is_visual());
        assert!(!EditorMode::VisualBlock.is_visual_line());
        assert!(EditorMode::VisualBlock.is_visual_block());
    }

    #[test]
    fn test_is_any_visual() {
        assert!(!EditorMode::Normal.is_any_visual());
        assert!(!EditorMode::Insert.is_any_visual());
        assert!(EditorMode::Visual.is_any_visual());
        assert!(EditorMode::VisualLine.is_any_visual());
        assert!(EditorMode::VisualBlock.is_any_visual());
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
