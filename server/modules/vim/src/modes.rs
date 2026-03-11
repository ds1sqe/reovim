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
//! Normal ─┬─ Delete
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
    /// Window mode - window management (`<C-w>` prefix).
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
    pub const ALL: &'static [Self] = &[
        Self::Normal,
        Self::Insert,
        Self::Visual,
        Self::VisualLine,
        Self::VisualBlock,
        Self::Replace,
        Self::CommandLine,
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

    /// Mode ID for Window mode.
    pub const WINDOW_ID: ModeId = ModeId::with_discriminant(VIM_MODULE, "window", 8);

    /// Mode ID for Delete mode.
    pub const DELETE_ID: ModeId = ModeId::with_discriminant(VIM_MODULE, "delete", 9);

    /// Mode ID for Yank mode.
    pub const YANK_ID: ModeId = ModeId::with_discriminant(VIM_MODULE, "yank", 10);

    /// Mode ID for Change mode.
    pub const CHANGE_ID: ModeId = ModeId::with_discriminant(VIM_MODULE, "change", 11);
}

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
            Self::Window | Self::Delete | Self::Yank | Self::Change => CursorStyle::Block,
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
            // Operator modes, window mode, and visual inherit from normal
            Self::Window | Self::Delete | Self::Yank | Self::Change | Self::Visual => {
                Some(Self::Normal)
            }
            Self::VisualLine | Self::VisualBlock => Some(Self::Visual),
            Self::Replace => Some(Self::Insert),
            Self::Normal | Self::Insert | Self::CommandLine => None,
        }
    }

    fn is_entry(&self) -> bool {
        matches!(self, Self::Normal)
    }
}
