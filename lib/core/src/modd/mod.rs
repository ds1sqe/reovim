//! Mode state management for the editor
//!
//! Uses `ComponentId` for focus context and `EditMode`/`SubMode` for input handling.

/// Unique identifier for UI components
///
/// This is the unified identifier for the component system.
/// Components are identified by a static string name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ComponentId(pub &'static str);

impl ComponentId {
    // === Core constants ===
    // Plugins define their own IDs using ComponentId("plugin_name")

    /// Editor component (main text editing area)
    pub const EDITOR: Self = Self("editor");
    /// Command line component (`:` commands)
    pub const COMMAND_LINE: Self = Self("command_line");
    /// Status line component (bottom bar)
    pub const STATUS_LINE: Self = Self("status_line");
    /// Tab line component (top bar with tabs)
    pub const TAB_LINE: Self = Self("tab_line");
    /// Window mode (Ctrl-W)
    pub const WINDOW: Self = Self("window");
}

impl std::fmt::Display for ComponentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Default for ComponentId {
    fn default() -> Self {
        Self::EDITOR
    }
}

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
    /// Generic interactor sub-mode for plugins
    ///
    /// This variant allows plugins to define their own sub-modes without
    /// modifying the core `SubMode` enum. The `ComponentId` identifies the
    /// plugin/feature that owns this sub-mode.
    ///
    /// Feature-specific state should be stored in the plugin's state
    /// via `PluginStateRegistry`.
    Interactor(ComponentId),
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

/// Complete mode state combining component context, edit mode, and sub-mode
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ModeState {
    /// The component ID - identifies which component has focus
    pub interactor_id: ComponentId,
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
            interactor_id: ComponentId::EDITOR,
            edit_mode: EditMode::Normal,
            sub_mode: SubMode::None,
        }
    }

    /// Create `ModeState` with specific interactor ID and edit mode
    #[must_use]
    pub const fn with_interactor_id_and_mode(
        interactor_id: ComponentId,
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
        interactor_id: ComponentId,
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
    pub const fn set_interactor_id(mut self, interactor_id: ComponentId) -> Self {
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
            interactor_id: ComponentId::EDITOR,
            edit_mode: EditMode::Normal,
            sub_mode: SubMode::None,
        }
    }

    /// Editor + Insert mode
    #[must_use]
    pub const fn insert() -> Self {
        Self {
            interactor_id: ComponentId::EDITOR,
            edit_mode: EditMode::Insert(InsertVariant::Standard),
            sub_mode: SubMode::None,
        }
    }

    /// Editor + Visual mode (character-wise)
    #[must_use]
    pub const fn visual() -> Self {
        Self {
            interactor_id: ComponentId::EDITOR,
            edit_mode: EditMode::Visual(VisualVariant::Char),
            sub_mode: SubMode::None,
        }
    }

    /// Editor + Visual Block mode
    #[must_use]
    pub const fn visual_block() -> Self {
        Self {
            interactor_id: ComponentId::EDITOR,
            edit_mode: EditMode::Visual(VisualVariant::Block),
            sub_mode: SubMode::None,
        }
    }

    /// Editor + Visual Line mode
    #[must_use]
    pub const fn visual_line() -> Self {
        Self {
            interactor_id: ComponentId::EDITOR,
            edit_mode: EditMode::Visual(VisualVariant::Line),
            sub_mode: SubMode::None,
        }
    }

    /// Editor + Command sub-mode
    #[must_use]
    pub const fn command() -> Self {
        Self {
            interactor_id: ComponentId::EDITOR,
            edit_mode: EditMode::Normal,
            sub_mode: SubMode::Command,
        }
    }

    /// Operator-pending mode
    #[must_use]
    pub const fn operator_pending(operator: OperatorType, count: Option<usize>) -> Self {
        Self {
            interactor_id: ComponentId::EDITOR,
            edit_mode: EditMode::Normal,
            sub_mode: SubMode::OperatorPending { operator, count },
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
        self.interactor_id.0 == "editor"
    }

    /// Check if the mode accepts character input, using registry for interactor lookup
    ///
    /// This method queries the [`InteractorRegistry`] to determine whether an
    /// interactor accepts character input, avoiding hardcoded component checks.
    ///
    /// # Arguments
    ///
    /// * `registry` - The interactor registry to query for input behavior
    ///
    /// # Returns
    ///
    /// * `true` if in Insert mode, Command mode, or an Interactor that accepts input
    /// * `false` for Normal, Visual, `OperatorPending`, or Interactors using keymap
    ///
    /// [`InteractorRegistry`]: crate::interactor::InteractorRegistry
    #[must_use]
    pub fn accepts_char_input_with(
        &self,
        registry: &crate::interactor::InteractorRegistry,
    ) -> bool {
        // Insert mode or Command mode always accept char input
        if self.is_insert() || self.is_command() {
            return true;
        }

        // For Interactor sub-modes, check the registry
        if let SubMode::Interactor(id) = &self.sub_mode {
            return registry.accepts_char_input(id);
        }

        // Other sub-modes (None, OperatorPending) don't accept char input
        false
    }

    /// Check if the mode accepts character input (insert mode or command mode)
    ///
    /// Note: Window mode (`SubMode::Interactor(ComponentId::WINDOW)`) does NOT accept char input -
    /// it uses the keymap for commands like h/j/k/l to navigate windows.
    ///
    /// **Deprecated**: Use [`accepts_char_input_with()`] with an [`InteractorRegistry`]
    /// for accurate results. This method uses hardcoded fallbacks for backwards compatibility.
    ///
    /// [`accepts_char_input_with()`]: ModeState::accepts_char_input_with
    /// [`InteractorRegistry`]: crate::interactor::InteractorRegistry
    #[must_use]
    pub fn accepts_char_input(&self) -> bool {
        // Insert mode or Command mode accept char input
        if self.is_insert() || self.is_command() {
            return true;
        }

        // Some Interactor sub-modes accept char input (e.g., Leap for search chars)
        // but Window mode does NOT - it uses keymap commands
        matches!(self.sub_mode, SubMode::Interactor(id) if id != ComponentId::WINDOW)
    }

    /// Build hierarchical mode display: "Kind | Mode | `SubMode`"
    ///
    /// Format: ` Kind | Mode ` or ` Kind | Mode | SubMode `
    /// Examples: `Editor | Normal`, `Editor | Normal | Window`, `Explorer | Insert`
    #[must_use]
    pub fn hierarchical_display(&self) -> String {
        let mut parts = Vec::with_capacity(3);

        // Part 1: Kind (interactor)
        let kind = match self.interactor_id.0 {
            "editor" | "microscope" => "Editor", // microscope is an overlay on editor
            "command_line" => "Cmd",
            "explorer" => "Explorer",
            other => other,
        };
        parts.push(kind);

        // Part 2: Edit mode
        let edit = match &self.edit_mode {
            EditMode::Normal => "Normal",
            EditMode::Insert(_) => "Insert",
            EditMode::Visual(VisualVariant::Char) => "Visual",
            EditMode::Visual(VisualVariant::Line) => "V-Line",
            EditMode::Visual(VisualVariant::Block) => "V-Block",
        };
        parts.push(edit);

        // Part 3: Sub-mode (if active)
        let sub = match &self.sub_mode {
            SubMode::None => None,
            SubMode::Command => Some("Command"),
            SubMode::OperatorPending { .. } => Some("Operator"),
            SubMode::Interactor(id) => Some(match id.0 {
                "window" => "Window",
                "leap" => "Leap",
                other => other,
            }),
        };
        if let Some(s) = sub {
            parts.push(s);
        }

        parts.join(" | ")
    }
}
