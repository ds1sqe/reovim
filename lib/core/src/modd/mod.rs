//! Mode state management for the editor
//!
//! Uses `InteractorId` for focus context and `EditMode`/`SubMode` for input handling.

use crate::leap::LeapDirection;

// Re-export InteractorId for convenience
pub use crate::interactor::InteractorId;

/// Operator type for operator-pending mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OperatorType {
    #[default]
    Delete,
    Yank,
    Change,
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

/// Complete mode state combining interactor context, edit mode, and sub-mode
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ModeState {
    /// The interactor ID - identifies which component has focus
    pub interactor_id: InteractorId,
    /// The current edit mode (Normal, Insert, Visual)
    pub edit_mode: EditMode,
    /// The current sub-mode (Command, `OperatorPending`, Leap, etc.)
    pub sub_mode: SubMode,
}

impl ModeState {
    /// Create a new `ModeState` with defaults (Editor + Normal + None)
    #[must_use]
    pub const fn new() -> Self {
        Self {
            interactor_id: InteractorId::EDITOR,
            edit_mode: EditMode::Normal,
            sub_mode: SubMode::None,
        }
    }

    /// Create `ModeState` with specific interactor ID and edit mode
    #[must_use]
    pub const fn with_interactor_id_and_mode(
        interactor_id: InteractorId,
        edit_mode: EditMode,
    ) -> Self {
        Self {
            interactor_id,
            edit_mode,
            sub_mode: SubMode::None,
        }
    }

    /// Create `ModeState` with interactor ID, edit mode, and sub-mode
    #[must_use]
    pub const fn with_interactor_id_sub_mode(
        interactor_id: InteractorId,
        edit_mode: EditMode,
        sub_mode: SubMode,
    ) -> Self {
        Self {
            interactor_id,
            edit_mode,
            sub_mode,
        }
    }

    /// Set interactor ID while preserving edit mode and sub-mode
    #[must_use]
    pub const fn set_interactor_id(mut self, interactor_id: InteractorId) -> Self {
        self.interactor_id = interactor_id;
        self
    }

    /// Set edit mode while preserving interactor and sub-mode
    #[must_use]
    pub const fn with_edit_mode(mut self, edit_mode: EditMode) -> Self {
        self.edit_mode = edit_mode;
        self
    }

    /// Set sub-mode while preserving interactor and edit mode
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
            edit_mode: EditMode::Normal,
            sub_mode: SubMode::None,
        }
    }

    /// Editor + Insert mode
    #[must_use]
    pub const fn insert() -> Self {
        Self {
            interactor_id: InteractorId::EDITOR,
            edit_mode: EditMode::Insert(InsertVariant::Standard),
            sub_mode: SubMode::None,
        }
    }

    /// Editor + Visual mode (character-wise)
    #[must_use]
    pub const fn visual() -> Self {
        Self {
            interactor_id: InteractorId::EDITOR,
            edit_mode: EditMode::Visual(VisualVariant::Char),
            sub_mode: SubMode::None,
        }
    }

    /// Editor + Visual Block mode
    #[must_use]
    pub const fn visual_block() -> Self {
        Self {
            interactor_id: InteractorId::EDITOR,
            edit_mode: EditMode::Visual(VisualVariant::Block),
            sub_mode: SubMode::None,
        }
    }

    /// Editor + Visual Line mode
    #[must_use]
    pub const fn visual_line() -> Self {
        Self {
            interactor_id: InteractorId::EDITOR,
            edit_mode: EditMode::Visual(VisualVariant::Line),
            sub_mode: SubMode::None,
        }
    }

    /// Editor + Command sub-mode
    #[must_use]
    pub const fn command() -> Self {
        Self {
            interactor_id: InteractorId::EDITOR,
            edit_mode: EditMode::Normal,
            sub_mode: SubMode::Command,
        }
    }

    /// Explorer + Normal mode
    #[must_use]
    pub const fn explorer() -> Self {
        Self {
            interactor_id: InteractorId::EXPLORER,
            edit_mode: EditMode::Normal,
            sub_mode: SubMode::None,
        }
    }

    /// Explorer + Insert mode (for input)
    #[must_use]
    pub const fn explorer_input() -> Self {
        Self {
            interactor_id: InteractorId::EXPLORER,
            edit_mode: EditMode::Insert(InsertVariant::Standard),
            sub_mode: SubMode::None,
        }
    }

    /// Telescope + Insert mode (default for typing)
    #[must_use]
    pub const fn telescope() -> Self {
        Self {
            interactor_id: InteractorId::TELESCOPE,
            edit_mode: EditMode::Insert(InsertVariant::Standard),
            sub_mode: SubMode::None,
        }
    }

    /// Telescope + Normal mode (for navigation)
    #[must_use]
    pub const fn telescope_normal() -> Self {
        Self {
            interactor_id: InteractorId::TELESCOPE,
            edit_mode: EditMode::Normal,
            sub_mode: SubMode::None,
        }
    }

    /// Settings menu mode
    #[must_use]
    pub const fn settings_menu() -> Self {
        Self {
            interactor_id: InteractorId::SETTINGS,
            edit_mode: EditMode::Normal,
            sub_mode: SubMode::None,
        }
    }

    /// Operator-pending mode
    #[must_use]
    pub const fn operator_pending(operator: OperatorType, count: Option<usize>) -> Self {
        Self {
            interactor_id: InteractorId::EDITOR,
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
    /// Format: ` INTERACTOR | EDIT_MODE ` or shows sub-mode when active
    #[must_use]
    pub fn display_string(&self) -> &'static str {
        // Sub-mode display (if active)
        match &self.sub_mode {
            SubMode::Command => return " COMMAND ",
            SubMode::OperatorPending { .. } => return " OPERATOR ",
            SubMode::Leap { .. } => return " LEAP ",
            SubMode::None => {}
        }

        // InteractorId × EditMode (orthogonal display)
        let is_editor = self.interactor_id == InteractorId::EDITOR;
        let is_explorer = self.interactor_id == InteractorId::EXPLORER;
        let is_telescope = self.interactor_id == InteractorId::TELESCOPE;
        let is_settings = self.interactor_id == InteractorId::SETTINGS;

        match &self.edit_mode {
            EditMode::Normal if is_editor => " NORMAL ",
            EditMode::Insert(_) if is_editor => " INSERT ",
            EditMode::Visual(VisualVariant::Char) if is_editor => " VISUAL ",
            EditMode::Visual(VisualVariant::Line) if is_editor => " V-LINE ",
            EditMode::Visual(VisualVariant::Block) if is_editor => " V-BLOCK ",
            // Explorer
            EditMode::Normal if is_explorer => " EXPLORER ",
            EditMode::Insert(_) if is_explorer => " EXPLORER | INSERT ",
            EditMode::Visual(_) if is_explorer => " EXPLORER | VISUAL ",
            // Telescope
            EditMode::Normal if is_telescope => " TELESCOPE ",
            EditMode::Insert(_) if is_telescope => " TELESCOPE | INSERT ",
            EditMode::Visual(_) if is_telescope => " TELESCOPE | VISUAL ",
            // Settings
            EditMode::Normal if is_settings => " SETTINGS ",
            EditMode::Insert(_) if is_settings => " SETTINGS | INSERT ",
            EditMode::Visual(_) if is_settings => " SETTINGS | VISUAL ",
            // Fallback for custom interactor IDs
            EditMode::Normal => " NORMAL ",
            EditMode::Insert(_) => " INSERT ",
            EditMode::Visual(_) => " VISUAL ",
        }
    }
}
