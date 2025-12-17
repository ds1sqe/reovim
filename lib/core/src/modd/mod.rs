use crate::leap::LeapDirection;

/// Operator type for operator-pending mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OperatorType {
    #[default]
    Delete,
    Yank,
    Change,
}

/// Focus context - where you are in the editor
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Focus {
    #[default]
    Editor,
    Explorer,
    Telescope,
}

/// Edit mode - how you're interacting
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum EditMode {
    #[default]
    Normal,
    Insert(ModExtension),
    Visual(ModExtension),
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

/// Mode extension for Insert and Visual modes
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ModExtension {
    #[default]
    Normal,
    Block,
    Column,
    MultiCursor,
}

/// Complete mode state combining focus, edit mode, and sub-mode
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ModeState {
    pub focus: Focus,
    pub edit_mode: EditMode,
    pub sub_mode: SubMode,
}

impl ModeState {
    /// Create a new `ModeState` with defaults (Editor + Normal + None)
    #[must_use]
    pub const fn new() -> Self {
        Self {
            focus: Focus::Editor,
            edit_mode: EditMode::Normal,
            sub_mode: SubMode::None,
        }
    }

    /// Create `ModeState` with specific focus and edit mode
    #[must_use]
    pub const fn with_focus_and_mode(focus: Focus, edit_mode: EditMode) -> Self {
        Self {
            focus,
            edit_mode,
            sub_mode: SubMode::None,
        }
    }

    /// Create `ModeState` with sub-mode
    #[must_use]
    pub const fn with_sub_mode(focus: Focus, edit_mode: EditMode, sub_mode: SubMode) -> Self {
        Self {
            focus,
            edit_mode,
            sub_mode,
        }
    }

    /// Set focus while preserving edit mode and sub-mode
    #[must_use]
    pub const fn with_focus(mut self, focus: Focus) -> Self {
        self.focus = focus;
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
            focus: Focus::Editor,
            edit_mode: EditMode::Normal,
            sub_mode: SubMode::None,
        }
    }

    /// Editor + Insert mode
    #[must_use]
    pub const fn insert() -> Self {
        Self {
            focus: Focus::Editor,
            edit_mode: EditMode::Insert(ModExtension::Normal),
            sub_mode: SubMode::None,
        }
    }

    /// Editor + Visual mode
    #[must_use]
    pub const fn visual() -> Self {
        Self {
            focus: Focus::Editor,
            edit_mode: EditMode::Visual(ModExtension::Normal),
            sub_mode: SubMode::None,
        }
    }

    /// Editor + Visual Block mode
    #[must_use]
    pub const fn visual_block() -> Self {
        Self {
            focus: Focus::Editor,
            edit_mode: EditMode::Visual(ModExtension::Block),
            sub_mode: SubMode::None,
        }
    }

    /// Editor + Command sub-mode
    #[must_use]
    pub const fn command() -> Self {
        Self {
            focus: Focus::Editor,
            edit_mode: EditMode::Normal,
            sub_mode: SubMode::Command,
        }
    }

    /// Explorer + Normal mode
    #[must_use]
    pub const fn explorer() -> Self {
        Self {
            focus: Focus::Explorer,
            edit_mode: EditMode::Normal,
            sub_mode: SubMode::None,
        }
    }

    /// Explorer + Insert mode (for input)
    #[must_use]
    pub const fn explorer_input() -> Self {
        Self {
            focus: Focus::Explorer,
            edit_mode: EditMode::Insert(ModExtension::Normal),
            sub_mode: SubMode::None,
        }
    }

    /// Telescope + Insert mode (default for typing)
    #[must_use]
    pub const fn telescope() -> Self {
        Self {
            focus: Focus::Telescope,
            edit_mode: EditMode::Insert(ModExtension::Normal),
            sub_mode: SubMode::None,
        }
    }

    /// Telescope + Normal mode (for navigation)
    #[must_use]
    pub const fn telescope_normal() -> Self {
        Self {
            focus: Focus::Telescope,
            edit_mode: EditMode::Normal,
            sub_mode: SubMode::None,
        }
    }

    /// Operator-pending mode
    #[must_use]
    pub const fn operator_pending(operator: OperatorType, count: Option<usize>) -> Self {
        Self {
            focus: Focus::Editor,
            edit_mode: EditMode::Normal,
            sub_mode: SubMode::OperatorPending { operator, count },
        }
    }

    /// Leap motion mode
    #[must_use]
    pub const fn leap(direction: LeapDirection, operator: Option<OperatorType>, count: Option<usize>) -> Self {
        Self {
            focus: Focus::Editor,
            edit_mode: EditMode::Normal,
            sub_mode: SubMode::Leap { direction, operator, count },
        }
    }

    // === State checks ===

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
    pub const fn is_editor_focus(&self) -> bool {
        matches!(self.focus, Focus::Editor)
    }

    /// Check if focused on explorer
    #[must_use]
    pub const fn is_explorer_focus(&self) -> bool {
        matches!(self.focus, Focus::Explorer)
    }

    /// Check if focused on telescope
    #[must_use]
    pub const fn is_telescope_focus(&self) -> bool {
        matches!(self.focus, Focus::Telescope)
    }

    /// Get display string for status line
    #[must_use]
    pub const fn display_string(&self) -> &'static str {
        // Sub-mode takes precedence
        match &self.sub_mode {
            SubMode::Command => return "",
            SubMode::OperatorPending { .. } => return " OPERATOR ",
            SubMode::Leap { .. } => return " LEAP ",
            SubMode::None => {}
        }

        // Then check focus + edit_mode
        match (&self.focus, &self.edit_mode) {
            (Focus::Editor, EditMode::Normal) => " NORMAL ",
            (Focus::Editor, EditMode::Insert(_)) => " INSERT ",
            (Focus::Editor, EditMode::Visual(_)) => " VISUAL ",
            (Focus::Explorer, _) => " EXPLORER ",
            (Focus::Telescope, EditMode::Insert(_)) => "",
            (Focus::Telescope, EditMode::Normal | EditMode::Visual(_)) => " TELESCOPE ",
        }
    }
}
