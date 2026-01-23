//! Vim mode definitions.
//!
//! This module defines the `VimMode` enum implementing the kernel's `Mode` trait.
//! All Vim modes are owned by this policy module - the kernel only provides the
//! trait contract.
//!
//! # Mode Ownership
//!
//! This is the canonical definition of Vim modes. Previously, mode identities
//! were scattered across kernel, runner, and modules. Now they live here.
//!
//! # Inheritance
//!
//! Modes form an inheritance hierarchy for keybinding fallback:
//!
//! ```text
//! Normal ─┬─ OperatorPending (deprecated)
//!         ├─ Delete
//!         ├─ Yank
//!         ├─ Change
//!         ├─ Window
//!         └─ Visual ─┬─ VisualLine
//!                    └─ VisualBlock
//!
//! Insert ─── Replace
//!
//! CommandLine (no parent)
//! ```

use reovim_kernel::api::v1::{CursorStyle, Mode, ModeId, ModuleId};

/// The Vim module ID.
pub const VIM_MODULE: ModuleId = ModuleId::new("vim");

/// Vim editing modes.
///
/// Each variant represents a distinct Vim mode with specific behavior:
/// - Cursor style (block, bar, underline)
/// - Whether it accepts character input
/// - Whether it has an active selection
/// - Parent mode for keybinding inheritance
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum VimMode {
    /// Normal mode - default command mode.
    Normal = 0,
    /// Insert mode - text input mode.
    Insert = 1,
    /// Visual character mode - character-wise selection.
    Visual = 2,
    /// Visual line mode - line-wise selection.
    VisualLine = 3,
    /// Visual block mode - block/column selection.
    VisualBlock = 4,
    /// Replace mode - overwrite text.
    Replace = 5,
    /// Command line mode - ex commands.
    CommandLine = 6,
    /// Operator-pending mode - waiting for motion/text object.
    /// DEPRECATED: Use Delete, Yank, or Change modes instead.
    #[deprecated(note = "Use Delete, Yank, or Change modes instead")]
    OperatorPending = 7,
    /// Window mode - window management (<C-w> prefix).
    Window = 8,
    /// Delete operator mode - waiting for motion to delete.
    Delete = 9,
    /// Yank operator mode - waiting for motion to yank.
    Yank = 10,
    /// Change operator mode - waiting for motion to change.
    Change = 11,
}

impl VimMode {
    /// All Vim modes for registration.
    #[allow(deprecated)]
    pub const ALL: &'static [Self] = &[
        Self::Normal,
        Self::Insert,
        Self::Visual,
        Self::VisualLine,
        Self::VisualBlock,
        Self::Replace,
        Self::CommandLine,
        Self::OperatorPending, // Deprecated but kept for compatibility
        Self::Window,
        Self::Delete,
        Self::Yank,
        Self::Change,
    ];

    // ========================================================================
    // Mode ID Constants
    // ========================================================================

    /// Mode ID for Normal mode.
    pub const NORMAL_ID: ModeId = ModeId::with_discriminant(VIM_MODULE, "normal", 0);

    /// Mode ID for Insert mode.
    pub const INSERT_ID: ModeId = ModeId::with_discriminant(VIM_MODULE, "insert", 1);

    /// Mode ID for Visual mode (character-wise).
    pub const VISUAL_ID: ModeId = ModeId::with_discriminant(VIM_MODULE, "visual", 2);

    /// Mode ID for Visual Line mode.
    pub const VISUAL_LINE_ID: ModeId = ModeId::with_discriminant(VIM_MODULE, "visual-line", 3);

    /// Mode ID for Visual Block mode.
    pub const VISUAL_BLOCK_ID: ModeId = ModeId::with_discriminant(VIM_MODULE, "visual-block", 4);

    /// Mode ID for Replace mode.
    pub const REPLACE_ID: ModeId = ModeId::with_discriminant(VIM_MODULE, "replace", 5);

    /// Mode ID for Command-line mode.
    pub const COMMANDLINE_ID: ModeId = ModeId::with_discriminant(VIM_MODULE, "command", 6);

    /// Mode ID for Operator-pending mode.
    pub const OPERATOR_PENDING_ID: ModeId =
        ModeId::with_discriminant(VIM_MODULE, "operator-pending", 7);

    /// Mode ID for Window mode.
    pub const WINDOW_ID: ModeId = ModeId::with_discriminant(VIM_MODULE, "window", 8);

    /// Mode ID for Delete mode.
    pub const DELETE_ID: ModeId = ModeId::with_discriminant(VIM_MODULE, "delete", 9);

    /// Mode ID for Yank mode.
    pub const YANK_ID: ModeId = ModeId::with_discriminant(VIM_MODULE, "yank", 10);

    /// Mode ID for Change mode.
    pub const CHANGE_ID: ModeId = ModeId::with_discriminant(VIM_MODULE, "change", 11);
}

#[allow(deprecated)]
impl Mode for VimMode {
    fn module() -> ModuleId {
        VIM_MODULE
    }

    fn discriminant(&self) -> u16 {
        *self as u16
    }

    fn id(&self) -> ModeId {
        // Use lowercase names for ModeId (for programmatic matching)
        // display_name() returns uppercase for statusline display
        let name = match self {
            Self::Normal => "normal",
            Self::Insert => "insert",
            Self::Visual => "visual",
            Self::VisualLine => "visual-line",
            Self::VisualBlock => "visual-block",
            Self::Replace => "replace",
            Self::CommandLine => "command",
            Self::OperatorPending => "operator-pending",
            Self::Window => "window",
            Self::Delete => "delete",
            Self::Yank => "yank",
            Self::Change => "change",
        };
        ModeId::with_discriminant(VIM_MODULE, name, self.discriminant())
    }

    fn display_name(&self) -> &'static str {
        // Uppercase names for statusline display
        match self {
            Self::Normal => "NORMAL",
            Self::Insert => "INSERT",
            Self::Visual => "VISUAL",
            Self::VisualLine => "V-LINE",
            Self::VisualBlock => "V-BLOCK",
            Self::Replace => "REPLACE",
            Self::CommandLine => "COMMAND",
            Self::OperatorPending => "OP-PENDING",
            Self::Window => "WINDOW",
            Self::Delete => "DELETE",
            Self::Yank => "YANK",
            Self::Change => "CHANGE",
        }
    }

    fn cursor_style(&self) -> CursorStyle {
        match self {
            Self::Normal | Self::Visual | Self::VisualLine | Self::VisualBlock => {
                CursorStyle::Block
            }
            Self::Insert | Self::CommandLine => CursorStyle::Bar,
            Self::Replace => CursorStyle::Underline,
            Self::OperatorPending | Self::Window | Self::Delete | Self::Yank | Self::Change => {
                CursorStyle::Block
            }
        }
    }

    fn accepts_char_input(&self) -> bool {
        matches!(self, Self::Insert | Self::Replace | Self::CommandLine)
    }

    fn has_selection(&self) -> bool {
        matches!(self, Self::Visual | Self::VisualLine | Self::VisualBlock)
    }

    fn inherits_from(&self) -> Option<Self> {
        match self {
            // Operator modes and window mode inherit from normal
            Self::OperatorPending | Self::Window | Self::Delete | Self::Yank | Self::Change => {
                Some(Self::Normal)
            }
            Self::Visual => Some(Self::Normal),
            Self::VisualLine | Self::VisualBlock => Some(Self::Visual),
            Self::Replace => Some(Self::Insert),
            Self::Normal | Self::Insert | Self::CommandLine => None,
        }
    }

    fn is_entry(&self) -> bool {
        matches!(self, Self::Normal)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[allow(deprecated)]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vim_mode_module() {
        assert_eq!(VimMode::module(), VIM_MODULE);
        assert_eq!(VIM_MODULE.as_str(), "vim");
    }

    #[test]
    fn test_vim_mode_discriminants() {
        assert_eq!(VimMode::Normal.discriminant(), 0);
        assert_eq!(VimMode::Insert.discriminant(), 1);
        assert_eq!(VimMode::Visual.discriminant(), 2);
        assert_eq!(VimMode::VisualLine.discriminant(), 3);
        assert_eq!(VimMode::VisualBlock.discriminant(), 4);
        assert_eq!(VimMode::Replace.discriminant(), 5);
        assert_eq!(VimMode::CommandLine.discriminant(), 6);
        assert_eq!(VimMode::OperatorPending.discriminant(), 7);
        assert_eq!(VimMode::Window.discriminant(), 8);
        // New operator modes
        assert_eq!(VimMode::Delete.discriminant(), 9);
        assert_eq!(VimMode::Yank.discriminant(), 10);
        assert_eq!(VimMode::Change.discriminant(), 11);
    }

    #[test]
    fn test_vim_mode_display_names() {
        assert_eq!(VimMode::Normal.display_name(), "NORMAL");
        assert_eq!(VimMode::Insert.display_name(), "INSERT");
        assert_eq!(VimMode::Visual.display_name(), "VISUAL");
        assert_eq!(VimMode::VisualLine.display_name(), "V-LINE");
        assert_eq!(VimMode::VisualBlock.display_name(), "V-BLOCK");
        assert_eq!(VimMode::Replace.display_name(), "REPLACE");
        assert_eq!(VimMode::CommandLine.display_name(), "COMMAND");
        assert_eq!(VimMode::OperatorPending.display_name(), "OP-PENDING");
        assert_eq!(VimMode::Window.display_name(), "WINDOW");
        // New operator modes show more informative names
        assert_eq!(VimMode::Delete.display_name(), "DELETE");
        assert_eq!(VimMode::Yank.display_name(), "YANK");
        assert_eq!(VimMode::Change.display_name(), "CHANGE");
    }

    #[test]
    fn test_vim_mode_cursor_styles() {
        assert_eq!(VimMode::Normal.cursor_style(), CursorStyle::Block);
        assert_eq!(VimMode::Insert.cursor_style(), CursorStyle::Bar);
        assert_eq!(VimMode::Visual.cursor_style(), CursorStyle::Block);
        assert_eq!(VimMode::VisualLine.cursor_style(), CursorStyle::Block);
        assert_eq!(VimMode::VisualBlock.cursor_style(), CursorStyle::Block);
        assert_eq!(VimMode::Replace.cursor_style(), CursorStyle::Underline);
        assert_eq!(VimMode::CommandLine.cursor_style(), CursorStyle::Bar);
        assert_eq!(VimMode::OperatorPending.cursor_style(), CursorStyle::Block);
        assert_eq!(VimMode::Window.cursor_style(), CursorStyle::Block);
        // New operator modes use block cursor
        assert_eq!(VimMode::Delete.cursor_style(), CursorStyle::Block);
        assert_eq!(VimMode::Yank.cursor_style(), CursorStyle::Block);
        assert_eq!(VimMode::Change.cursor_style(), CursorStyle::Block);
    }

    #[test]
    fn test_vim_mode_accepts_char_input() {
        assert!(!VimMode::Normal.accepts_char_input());
        assert!(VimMode::Insert.accepts_char_input());
        assert!(!VimMode::Visual.accepts_char_input());
        assert!(!VimMode::VisualLine.accepts_char_input());
        assert!(!VimMode::VisualBlock.accepts_char_input());
        assert!(VimMode::Replace.accepts_char_input());
        assert!(VimMode::CommandLine.accepts_char_input());
        assert!(!VimMode::OperatorPending.accepts_char_input());
        assert!(!VimMode::Window.accepts_char_input());
        // New operator modes don't accept char input
        assert!(!VimMode::Delete.accepts_char_input());
        assert!(!VimMode::Yank.accepts_char_input());
        assert!(!VimMode::Change.accepts_char_input());
    }

    #[test]
    fn test_vim_mode_has_selection() {
        assert!(!VimMode::Normal.has_selection());
        assert!(!VimMode::Insert.has_selection());
        assert!(VimMode::Visual.has_selection());
        assert!(VimMode::VisualLine.has_selection());
        assert!(VimMode::VisualBlock.has_selection());
        assert!(!VimMode::Replace.has_selection());
        assert!(!VimMode::CommandLine.has_selection());
        assert!(!VimMode::OperatorPending.has_selection());
        assert!(!VimMode::Window.has_selection());
        // New operator modes don't have selection
        assert!(!VimMode::Delete.has_selection());
        assert!(!VimMode::Yank.has_selection());
        assert!(!VimMode::Change.has_selection());
    }

    #[test]
    fn test_vim_mode_inheritance() {
        // Normal: no parent
        assert_eq!(VimMode::Normal.inherits_from(), None);

        // Insert: no parent
        assert_eq!(VimMode::Insert.inherits_from(), None);

        // CommandLine: no parent
        assert_eq!(VimMode::CommandLine.inherits_from(), None);

        // Visual: inherits from Normal
        assert_eq!(VimMode::Visual.inherits_from(), Some(VimMode::Normal));

        // VisualLine/VisualBlock: inherit from Visual
        assert_eq!(VimMode::VisualLine.inherits_from(), Some(VimMode::Visual));
        assert_eq!(VimMode::VisualBlock.inherits_from(), Some(VimMode::Visual));

        // Replace: inherits from Insert
        assert_eq!(VimMode::Replace.inherits_from(), Some(VimMode::Insert));

        // OperatorPending/Window: inherit from Normal
        assert_eq!(VimMode::OperatorPending.inherits_from(), Some(VimMode::Normal));
        assert_eq!(VimMode::Window.inherits_from(), Some(VimMode::Normal));

        // New operator modes: inherit from Normal (allows motion keys)
        assert_eq!(VimMode::Delete.inherits_from(), Some(VimMode::Normal));
        assert_eq!(VimMode::Yank.inherits_from(), Some(VimMode::Normal));
        assert_eq!(VimMode::Change.inherits_from(), Some(VimMode::Normal));
    }

    #[test]
    fn test_vim_mode_id_round_trip() {
        for mode in VimMode::ALL {
            let id = mode.id();
            assert_eq!(id.module(), &VIM_MODULE);
            // ModeId name is lowercase, display_name is uppercase
            assert_ne!(id.name(), mode.display_name());
            assert_eq!(id.discriminant(), mode.discriminant());
        }
    }

    #[test]
    fn test_vim_mode_id_equality() {
        // Same mode produces equal IDs
        assert_eq!(VimMode::Normal.id(), VimMode::Normal.id());
        assert_eq!(VimMode::Insert.id(), VimMode::Insert.id());

        // Different modes produce different IDs
        assert_ne!(VimMode::Normal.id(), VimMode::Insert.id());
        assert_ne!(VimMode::Visual.id(), VimMode::VisualLine.id());
    }

    #[test]
    fn test_vim_mode_all_count() {
        // 9 original modes + 3 new operator modes = 12
        assert_eq!(VimMode::ALL.len(), 12);
    }

    #[test]
    fn test_vim_mode_copy_clone() {
        // Verify Copy and Clone work
        let mode = VimMode::Normal;
        let copied = mode;
        #[allow(clippy::clone_on_copy)]
        let cloned = mode.clone(); // Explicit clone to test Clone impl
        assert_eq!(mode, copied);
        assert_eq!(mode, cloned);
    }

    #[test]
    fn test_vim_mode_hash() {
        use std::collections::HashSet;

        let mut set = HashSet::new();
        for mode in VimMode::ALL {
            set.insert(*mode);
        }
        assert_eq!(set.len(), VimMode::ALL.len());
    }

    #[test]
    fn test_vim_mode_into_mode_id() {
        // Test blanket impl From<M> for ModeId
        let mode = VimMode::Insert;
        let id: ModeId = mode.into();
        assert_eq!(id.discriminant(), VimMode::Insert.discriminant());
    }

    #[test]
    fn test_vim_mode_is_entry() {
        // Only Normal mode is the entry mode
        assert!(VimMode::Normal.is_entry());

        // All other modes are not entry modes
        assert!(!VimMode::Insert.is_entry());
        assert!(!VimMode::Visual.is_entry());
        assert!(!VimMode::VisualLine.is_entry());
        assert!(!VimMode::VisualBlock.is_entry());
        assert!(!VimMode::Replace.is_entry());
        assert!(!VimMode::CommandLine.is_entry());
        assert!(!VimMode::OperatorPending.is_entry());
        assert!(!VimMode::Window.is_entry());
    }
}
