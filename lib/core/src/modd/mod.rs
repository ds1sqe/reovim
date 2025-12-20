// Allow deprecated Focus enum during transition to InteractorId
#![allow(deprecated)]

use crate::leap::LeapDirection;

// Re-export InteractorId for backward compatibility
pub use crate::interactor::InteractorId;

/// Operator type for operator-pending mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OperatorType {
    #[default]
    Delete,
    Yank,
    Change,
}

/// Focus context - where you are in the editor
///
/// DEPRECATED: Use [`InteractorId`] directly instead.
#[deprecated(since = "0.7.0", note = "Use InteractorId instead")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Focus {
    #[default]
    Editor,
    Explorer,
    Telescope,
    SettingsMenu,
}

#[allow(deprecated)]
impl From<Focus> for InteractorId {
    fn from(focus: Focus) -> Self {
        match focus {
            Focus::Editor => Self::EDITOR,
            Focus::Explorer => Self::EXPLORER,
            Focus::Telescope => Self::TELESCOPE,
            Focus::SettingsMenu => Self::SETTINGS,
        }
    }
}

/// Edit mode - how you're interacting
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum EditMode {
    #[default]
    Normal,
    Insert(InsertVariant),
    Visual(VisualVariant),
}

/// Sub-mode - special overlay states
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum SubMode {
    #[default]
    None,
    Command,
    OperatorPending {
        operator: OperatorType,
        count: Option<usize>,
    },
    /// Leap motion mode (s/S to jump to two-character sequence)
    Leap {
        direction: LeapDirection,
        operator: Option<OperatorType>,
        count: Option<usize>,
    },
}

/// Insert mode variants
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InsertVariant {
    /// Standard insert mode (i, a, o, O)
    #[default]
    Standard,
    /// Replace mode (R) - overwrites characters
    Replace,
}

/// Visual mode variants
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VisualVariant {
    /// Character-wise visual (v)
    #[default]
    Char,
    /// Line-wise visual (V)
    Line,
    /// Block-wise visual (Ctrl-V)
    Block,
}

/// Complete mode state combining focus, edit mode, and sub-mode
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[allow(deprecated)]
pub struct ModeState {
    /// The interactor ID (extensible, trait-based)
    pub interactor_id: InteractorId,
    /// Deprecated: Use `interactor_id` instead. Kept for backward compatibility with
    /// code that pattern-matches on Focus enum.
    pub focus: Focus,
    pub edit_mode: EditMode,
    pub sub_mode: SubMode,
}

#[allow(deprecated)]
impl ModeState {
    /// Create a new `ModeState` with defaults (Editor + Normal + None)
    #[must_use]
    pub const fn new() -> Self {
        Self {
            interactor_id: InteractorId::EDITOR,
            focus: Focus::Editor,
            edit_mode: EditMode::Normal,
            sub_mode: SubMode::None,
        }
    }

    /// Create `ModeState` with specific focus and edit mode
    #[must_use]
    #[deprecated(since = "0.7.0", note = "Use with_interactor_id_and_mode instead")]
    pub const fn with_focus_and_mode(focus: Focus, edit_mode: EditMode) -> Self {
        let interactor_id = match focus {
            Focus::Editor => InteractorId::EDITOR,
            Focus::Explorer => InteractorId::EXPLORER,
            Focus::Telescope => InteractorId::TELESCOPE,
            Focus::SettingsMenu => InteractorId::SETTINGS,
        };
        Self {
            interactor_id,
            focus,
            edit_mode,
            sub_mode: SubMode::None,
        }
    }

    /// Create `ModeState` with specific interactor ID and edit mode
    #[must_use]
    pub const fn with_interactor_id_and_mode(
        interactor_id: InteractorId,
        edit_mode: EditMode,
    ) -> Self {
        let focus = match interactor_id.0.as_bytes() {
            b"explorer" => Focus::Explorer,
            b"telescope" => Focus::Telescope,
            b"settings" => Focus::SettingsMenu,
            // "editor" and custom interactor IDs default to Editor
            _ => Focus::Editor,
        };
        Self {
            interactor_id,
            focus,
            edit_mode,
            sub_mode: SubMode::None,
        }
    }

    /// Create `ModeState` with sub-mode
    #[must_use]
    #[deprecated(since = "0.7.0", note = "Use with_interactor_id_sub_mode instead")]
    pub const fn with_sub_mode(focus: Focus, edit_mode: EditMode, sub_mode: SubMode) -> Self {
        let interactor_id = match focus {
            Focus::Editor => InteractorId::EDITOR,
            Focus::Explorer => InteractorId::EXPLORER,
            Focus::Telescope => InteractorId::TELESCOPE,
            Focus::SettingsMenu => InteractorId::SETTINGS,
        };
        Self {
            interactor_id,
            focus,
            edit_mode,
            sub_mode,
        }
    }

    /// Set focus while preserving edit mode and sub-mode
    #[must_use]
    #[deprecated(since = "0.7.0", note = "Use set_interactor_id instead")]
    pub const fn with_focus(mut self, focus: Focus) -> Self {
        self.interactor_id = match focus {
            Focus::Editor => InteractorId::EDITOR,
            Focus::Explorer => InteractorId::EXPLORER,
            Focus::Telescope => InteractorId::TELESCOPE,
            Focus::SettingsMenu => InteractorId::SETTINGS,
        };
        self.focus = focus;
        self
    }

    /// Set interactor ID while preserving edit mode and sub-mode
    #[must_use]
    pub const fn set_interactor_id(mut self, interactor_id: InteractorId) -> Self {
        self.interactor_id = interactor_id;
        self.focus = match interactor_id.0.as_bytes() {
            b"explorer" => Focus::Explorer,
            b"telescope" => Focus::Telescope,
            b"settings" => Focus::SettingsMenu,
            // "editor" and custom interactor IDs default to Editor
            _ => Focus::Editor,
        };
        self
    }

    /// Set edit mode while preserving focus and sub-mode
    #[must_use]
    pub const fn with_edit_mode(mut self, edit_mode: EditMode) -> Self {
        self.edit_mode = edit_mode;
        self
    }

    /// Set sub-mode while preserving focus and edit mode
    #[must_use]
    pub const fn with_sub(mut self, sub_mode: SubMode) -> Self {
        self.sub_mode = sub_mode;
        self
    }

    // === Convenience constructors ===

    /// Editor + Normal mode
    #[must_use]
    pub const fn normal() -> Self {
        Self {
            interactor_id: InteractorId::EDITOR,
            focus: Focus::Editor,
            edit_mode: EditMode::Normal,
            sub_mode: SubMode::None,
        }
    }

    /// Editor + Insert mode
    #[must_use]
    pub const fn insert() -> Self {
        Self {
            interactor_id: InteractorId::EDITOR,
            focus: Focus::Editor,
            edit_mode: EditMode::Insert(InsertVariant::Standard),
            sub_mode: SubMode::None,
        }
    }

    /// Editor + Visual mode (character-wise)
    #[must_use]
    pub const fn visual() -> Self {
        Self {
            interactor_id: InteractorId::EDITOR,
            focus: Focus::Editor,
            edit_mode: EditMode::Visual(VisualVariant::Char),
            sub_mode: SubMode::None,
        }
    }

    /// Editor + Visual Block mode
    #[must_use]
    pub const fn visual_block() -> Self {
        Self {
            interactor_id: InteractorId::EDITOR,
            focus: Focus::Editor,
            edit_mode: EditMode::Visual(VisualVariant::Block),
            sub_mode: SubMode::None,
        }
    }

    /// Editor + Visual Line mode
    #[must_use]
    pub const fn visual_line() -> Self {
        Self {
            interactor_id: InteractorId::EDITOR,
            focus: Focus::Editor,
            edit_mode: EditMode::Visual(VisualVariant::Line),
            sub_mode: SubMode::None,
        }
    }

    /// Editor + Command sub-mode
    #[must_use]
    pub const fn command() -> Self {
        Self {
            interactor_id: InteractorId::EDITOR,
            focus: Focus::Editor,
            edit_mode: EditMode::Normal,
            sub_mode: SubMode::Command,
        }
    }

    /// Explorer + Normal mode
    #[must_use]
    pub const fn explorer() -> Self {
        Self {
            interactor_id: InteractorId::EXPLORER,
            focus: Focus::Explorer,
            edit_mode: EditMode::Normal,
            sub_mode: SubMode::None,
        }
    }

    /// Explorer + Insert mode (for input)
    #[must_use]
    pub const fn explorer_input() -> Self {
        Self {
            interactor_id: InteractorId::EXPLORER,
            focus: Focus::Explorer,
            edit_mode: EditMode::Insert(InsertVariant::Standard),
            sub_mode: SubMode::None,
        }
    }

    /// Telescope + Insert mode (default for typing)
    #[must_use]
    pub const fn telescope() -> Self {
        Self {
            interactor_id: InteractorId::TELESCOPE,
            focus: Focus::Telescope,
            edit_mode: EditMode::Insert(InsertVariant::Standard),
            sub_mode: SubMode::None,
        }
    }

    /// Telescope + Normal mode (for navigation)
    #[must_use]
    pub const fn telescope_normal() -> Self {
        Self {
            interactor_id: InteractorId::TELESCOPE,
            focus: Focus::Telescope,
            edit_mode: EditMode::Normal,
            sub_mode: SubMode::None,
        }
    }

    /// Settings menu mode
    #[must_use]
    pub const fn settings_menu() -> Self {
        Self {
            interactor_id: InteractorId::SETTINGS,
            focus: Focus::SettingsMenu,
            edit_mode: EditMode::Normal,
            sub_mode: SubMode::None,
        }
    }

    /// Operator-pending mode
    #[must_use]
    pub const fn operator_pending(operator: OperatorType, count: Option<usize>) -> Self {
        Self {
            interactor_id: InteractorId::EDITOR,
            focus: Focus::Editor,
            edit_mode: EditMode::Normal,
            sub_mode: SubMode::OperatorPending { operator, count },
        }
    }

    /// Leap motion mode
    #[must_use]
    pub const fn leap(
        direction: LeapDirection,
        operator: Option<OperatorType>,
        count: Option<usize>,
    ) -> Self {
        Self {
            interactor_id: InteractorId::EDITOR,
            focus: Focus::Editor,
            edit_mode: EditMode::Normal,
            sub_mode: SubMode::Leap {
                direction,
                operator,
                count,
            },
        }
    }

    // === State checks ===

    /// Check if the current interactor matches the given interactor ID
    #[must_use]
    pub fn is_interactor(&self, id: InteractorId) -> bool {
        self.interactor_id.0 == id.0
    }

    /// Check if in command sub-mode
    #[must_use]
    pub const fn is_command(&self) -> bool {
        matches!(self.sub_mode, SubMode::Command)
    }

    /// Check if in operator-pending sub-mode
    #[must_use]
    pub const fn is_operator_pending(&self) -> bool {
        matches!(self.sub_mode, SubMode::OperatorPending { .. })
    }

    /// Check if in leap sub-mode
    #[must_use]
    pub const fn is_leap(&self) -> bool {
        matches!(self.sub_mode, SubMode::Leap { .. })
    }

    /// Check if in normal edit mode (any focus)
    #[must_use]
    pub const fn is_normal(&self) -> bool {
        matches!(self.edit_mode, EditMode::Normal) && matches!(self.sub_mode, SubMode::None)
    }

    /// Check if in insert edit mode (any focus)
    #[must_use]
    pub const fn is_insert(&self) -> bool {
        matches!(self.edit_mode, EditMode::Insert(_))
    }

    /// Check if in visual edit mode (any focus)
    #[must_use]
    pub const fn is_visual(&self) -> bool {
        matches!(self.edit_mode, EditMode::Visual(_))
    }

    /// Check if focused on editor
    #[must_use]
    pub fn is_editor_focus(&self) -> bool {
        self.is_interactor(InteractorId::EDITOR)
    }

    /// Check if focused on explorer
    #[must_use]
    pub fn is_explorer_focus(&self) -> bool {
        self.is_interactor(InteractorId::EXPLORER)
    }

    /// Check if focused on telescope
    #[must_use]
    pub fn is_telescope_focus(&self) -> bool {
        self.is_interactor(InteractorId::TELESCOPE)
    }

    /// Check if the mode accepts character input (insert mode or command mode)
    #[must_use]
    pub const fn accepts_char_input(&self) -> bool {
        self.is_insert() || self.is_command()
    }

    /// Get display string for status line (orthogonal format)
    ///
    /// Format: ` FOCUS | EDIT_MODE ` or ` FOCUS | EDIT_MODE | SUB_MODE `
    #[must_use]
    pub const fn display_string(&self) -> &'static str {
        // Sub-mode display (if active)
        match &self.sub_mode {
            SubMode::Command => return " COMMAND ",
            SubMode::OperatorPending { .. } => return " OPERATOR ",
            SubMode::Leap { .. } => return " LEAP ",
            SubMode::None => {}
        }

        // Focus × EditMode (orthogonal display)
        match (&self.focus, &self.edit_mode) {
            // Editor focus
            (Focus::Editor, EditMode::Normal) => " NORMAL ",
            (Focus::Editor, EditMode::Insert(_)) => " INSERT ",
            (Focus::Editor, EditMode::Visual(VisualVariant::Char)) => " VISUAL ",
            (Focus::Editor, EditMode::Visual(VisualVariant::Line)) => " V-LINE ",
            (Focus::Editor, EditMode::Visual(VisualVariant::Block)) => " V-BLOCK ",
            // Explorer focus
            (Focus::Explorer, EditMode::Normal) => " EXPLORER ",
            (Focus::Explorer, EditMode::Insert(_)) => " EXPLORER | INSERT ",
            (Focus::Explorer, EditMode::Visual(_)) => " EXPLORER | VISUAL ",
            // Telescope focus
            (Focus::Telescope, EditMode::Normal) => " TELESCOPE ",
            (Focus::Telescope, EditMode::Insert(_)) => " TELESCOPE | INSERT ",
            (Focus::Telescope, EditMode::Visual(_)) => " TELESCOPE | VISUAL ",
            // Settings focus
            (Focus::SettingsMenu, EditMode::Normal) => " SETTINGS ",
            (Focus::SettingsMenu, EditMode::Insert(_)) => " SETTINGS | INSERT ",
            (Focus::SettingsMenu, EditMode::Visual(_)) => " SETTINGS | VISUAL ",
        }
    }
}
