//! Key binding system for mapping key sequences to commands

use {
    crate::{
        command::{CommandId, CommandTrait, id::builtin, registry::CommandRegistry},
        keys,
        keystroke::KeySequence,
        modd::{ComponentId, EditMode, ModeState, SubMode},
    },
    std::{collections::HashMap, sync::Arc},
};

// ============================================================================
// Keymap Scope Types
// ============================================================================

/// Simplified edit mode for keybinding scope (no variants)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EditModeKind {
    Normal,
    Insert,
    Visual,
}

impl From<&EditMode> for EditModeKind {
    fn from(mode: &EditMode) -> Self {
        match mode {
            EditMode::Normal => Self::Normal,
            EditMode::Insert(_) => Self::Insert,
            EditMode::Visual(_) => Self::Visual,
        }
    }
}

/// Simplified sub-mode for keybinding scope
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SubModeKind {
    Command,
    OperatorPending,
    /// Generic interactor sub-mode (plugin-defined)
    Interactor(ComponentId),
}

/// Identifies which keymap a binding belongs to
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum KeymapScope {
    /// Component-specific scope: `ComponentId` + `EditModeKind`
    Component { id: ComponentId, mode: EditModeKind },
    /// Global sub-mode scope (applies across components)
    SubMode(SubModeKind),
    /// Default fallback for any component in Normal mode
    /// Used for bindings like Ctrl-W that should work in all plugin windows
    DefaultNormal,
}

impl KeymapScope {
    /// Create a component scope for Editor + Normal mode
    #[must_use]
    pub const fn editor_normal() -> Self {
        Self::Component {
            id: ComponentId::EDITOR,
            mode: EditModeKind::Normal,
        }
    }

    /// Create a component scope for Editor + Insert mode
    #[must_use]
    pub const fn editor_insert() -> Self {
        Self::Component {
            id: ComponentId::EDITOR,
            mode: EditModeKind::Insert,
        }
    }

    /// Create a component scope for Editor + Visual mode
    #[must_use]
    pub const fn editor_visual() -> Self {
        Self::Component {
            id: ComponentId::EDITOR,
            mode: EditModeKind::Visual,
        }
    }
}

/// A declarative keybinding definition
#[derive(Clone)]
pub struct KeyBinding {
    /// Key sequence
    pub keys: KeySequence,
    /// Command to execute
    pub command: CommandRef,
    /// Optional description for help display (overrides command description)
    pub description: Option<&'static str>,
    /// Category for help system grouping (e.g., "motion", "operator")
    pub category: Option<&'static str>,
}

impl KeyBinding {
    /// Create a binding with a registered command ID
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // KeySequence contains Vec which can't be const
    pub fn id(keys: KeySequence, id: CommandId) -> Self {
        Self {
            keys,
            command: CommandRef::Registered(id),
            description: None,
            category: None,
        }
    }

    /// Create a binding with a command ID and category
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // KeySequence contains Vec which can't be const
    pub fn id_category(keys: KeySequence, id: CommandId, category: &'static str) -> Self {
        Self {
            keys,
            command: CommandRef::Registered(id),
            description: None,
            category: Some(category),
        }
    }

    /// Create a description-only binding (prefix node for help display)
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // KeySequence contains Vec which can't be const
    pub fn with_description(
        keys: KeySequence,
        description: &'static str,
        category: &'static str,
    ) -> Self {
        Self {
            keys,
            command: CommandRef::Registered(CommandId::new("")),
            description: Some(description),
            category: Some(category),
        }
    }
}

impl std::fmt::Debug for KeyBinding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeyBinding")
            .field("keys", &self.keys)
            .field("description", &self.description)
            .field("category", &self.category)
            .finish_non_exhaustive()
    }
}

// ============================================================================
// Command Reference
// ============================================================================

/// Reference to a command - either by ID (for registered commands) or inline
#[derive(Debug, Clone)]
pub enum CommandRef {
    /// Reference to a registered command by ID
    Registered(CommandId),
    /// Inline command with parameters (for parameterized commands like `InsertChar`)
    Inline(Arc<dyn CommandTrait>),
}

/// Node in the keymap trie
#[derive(Clone)]
pub struct KeyMapInner {
    /// If this is a terminal node, the command to execute
    pub command: Option<CommandRef>,
    /// Optional description for help display (overrides command description)
    /// Used for dynamically-handled keys like operator motions
    pub description: Option<String>,
    /// Category for help system grouping (e.g., "motion", "operator", "mode")
    pub category: Option<&'static str>,
    /// Children for multi-key sequences (e.g., "dd", "gg")
    #[allow(dead_code)] // Infrastructure for proper trie-based lookup
    pub next: HashMap<KeySequence, Self>,
}

impl KeyMapInner {
    #[must_use]
    pub fn new() -> Self {
        Self {
            command: None,
            description: None,
            category: None,
            next: HashMap::new(),
        }
    }

    /// Create a node with a `CommandRef`
    #[must_use]
    pub fn with_command_ref(cmd: CommandRef) -> Self {
        Self {
            command: Some(cmd),
            description: None,
            category: None,
            next: HashMap::new(),
        }
    }

    /// Create a node with a registered command ID
    #[must_use]
    pub fn with_command_id(id: CommandId) -> Self {
        Self {
            command: Some(CommandRef::Registered(id)),
            description: None,
            category: None,
            next: HashMap::new(),
        }
    }

    /// Create a node with an inline command
    #[must_use]
    pub fn with_inline_command(cmd: Arc<dyn CommandTrait>) -> Self {
        Self {
            command: Some(CommandRef::Inline(cmd)),
            description: None,
            category: None,
            next: HashMap::new(),
        }
    }

    /// Create a description-only node for help display (no command)
    /// Used for dynamically-handled keys like operator motions
    #[must_use]
    pub fn with_description(text: impl Into<String>) -> Self {
        Self {
            command: None,
            description: Some(text.into()),
            category: None,
            next: HashMap::new(),
        }
    }

    /// Set the category for this binding (builder pattern)
    #[must_use]
    pub const fn with_category(mut self, category: &'static str) -> Self {
        self.category = Some(category);
        self
    }

    /// Set the category if Some (builder pattern)
    #[must_use]
    pub const fn with_category_opt(mut self, category: Option<&'static str>) -> Self {
        self.category = category;
        self
    }

    /// Get the description for this binding
    ///
    /// Priority: description > command description > prefix indicator
    #[must_use]
    pub fn get_description(&self, registry: &CommandRegistry) -> String {
        // Explicit description overrides everything
        if let Some(desc) = &self.description {
            return desc.clone();
        }
        match &self.command {
            Some(CommandRef::Registered(id)) => registry
                .get(id)
                .map_or_else(|| id.as_str().to_string(), |cmd| cmd.description().to_string()),
            Some(CommandRef::Inline(cmd)) => cmd.description().to_string(),
            None => "+prefix".to_string(),
        }
    }

    /// Check if this is a prefix node (has no command and no description)
    #[must_use]
    pub const fn is_prefix(&self) -> bool {
        self.command.is_none() && self.description.is_none()
    }
}

impl Default for KeyMapInner {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for KeyMapInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeyMapInner")
            .field("command", &self.command)
            .field("description", &self.description)
            .field("category", &self.category)
            .field("next_keys", &self.next.keys().collect::<Vec<_>>())
            .finish()
    }
}

/// Empty keymap for fallback returns
static EMPTY_MAP: std::sync::LazyLock<HashMap<KeySequence, KeyMapInner>> =
    std::sync::LazyLock::new(HashMap::new);

/// Keymap for each mode
///
/// Keymaps are organized by `KeymapScope`:
/// - `Component { id, mode }` for component-specific keymaps
/// - `SubMode` for global sub-mode keymaps (Command, `OperatorPending`, Leap)
#[derive(Default, Clone)]
pub struct KeyMap {
    /// All keymaps indexed by scope
    maps: HashMap<KeymapScope, HashMap<KeySequence, KeyMapInner>>,
}

impl KeyMap {
    /// Create a new empty keymap
    #[must_use]
    pub fn new() -> Self {
        Self {
            maps: HashMap::new(),
        }
    }

    #[must_use]
    pub fn with_defaults() -> Self {
        let mut km = Self::new();
        km.setup_editor_keybindings();
        km.setup_submode_keybindings();
        km.setup_default_normal_keybindings();
        km
    }

    // ========================================================================
    // Scope-based API (new)
    // ========================================================================

    /// Bind a key to a command in a specific scope
    pub fn bind_scoped(&mut self, scope: KeymapScope, keys: KeySequence, cmd: CommandRef) {
        self.maps
            .entry(scope)
            .or_default()
            .insert(keys, KeyMapInner::with_command_ref(cmd));
    }

    /// Bind with full metadata (description, category)
    pub fn bind_with_metadata(&mut self, scope: KeymapScope, binding: KeyBinding) {
        let inner = if binding.description.is_some() {
            KeyMapInner::with_description(binding.description.unwrap_or_default())
                .with_category_opt(binding.category)
        } else {
            KeyMapInner::with_command_ref(binding.command).with_category_opt(binding.category)
        };
        self.maps
            .entry(scope)
            .or_default()
            .insert(binding.keys, inner);
    }

    /// Unbind a key in a specific scope
    pub fn unbind_scoped(&mut self, scope: &KeymapScope, keys: &KeySequence) {
        if let Some(map) = self.maps.get_mut(scope) {
            map.remove(keys);
        }
    }

    /// Get the keymap for a specific scope
    #[must_use]
    pub fn get_scope(&self, scope: &KeymapScope) -> &HashMap<KeySequence, KeyMapInner> {
        self.maps.get(scope).unwrap_or(&EMPTY_MAP)
    }

    /// Get mutable access to a scope's keymap
    pub fn get_scope_mut(&mut self, scope: KeymapScope) -> &mut HashMap<KeySequence, KeyMapInner> {
        self.maps.entry(scope).or_default()
    }

    /// Iterate over all scopes and their keymaps
    pub fn iter_scopes(
        &self,
    ) -> impl Iterator<Item = (&KeymapScope, &HashMap<KeySequence, KeyMapInner>)> {
        self.maps.iter()
    }

    // ========================================================================
    // Mode resolution
    // ========================================================================

    /// Get the appropriate keymap for a given `ModeState`
    ///
    /// Keymap selection priority:
    /// 1. `SubMode` takes precedence (Command, `OperatorPending`, Leap)
    /// 2. `ComponentId` + `EditMode` determines the keymap otherwise
    #[must_use]
    pub fn get_keymap_for_mode(&self, mode: &ModeState) -> &HashMap<KeySequence, KeyMapInner> {
        let scope = Self::mode_to_scope(mode);
        self.maps.get(&scope).unwrap_or(&EMPTY_MAP)
    }

    /// Lookup a binding with fallback to `DefaultNormal` scope
    ///
    /// For Component scopes in Normal mode, this checks:
    /// 1. The specific component scope first
    /// 2. Falls back to `DefaultNormal` if not found
    ///
    /// This allows bindings like Ctrl-W to work in all plugin windows.
    #[must_use]
    pub fn lookup_binding(&self, mode: &ModeState, keys: &KeySequence) -> Option<&KeyMapInner> {
        let scope = Self::mode_to_scope(mode);

        // First check the primary scope
        if let Some(keymap) = self.maps.get(&scope)
            && let Some(inner) = keymap.get(keys)
        {
            return Some(inner);
        }

        // For Component + Normal mode, fallback to DefaultNormal
        if let KeymapScope::Component {
            mode: EditModeKind::Normal,
            ..
        } = scope
            && let Some(default_map) = self.maps.get(&KeymapScope::DefaultNormal)
        {
            return default_map.get(keys);
        }

        None
    }

    /// Check if keys are a valid prefix in primary or fallback scope
    #[must_use]
    pub fn is_valid_prefix(&self, mode: &ModeState, keys: &KeySequence) -> bool {
        let scope = Self::mode_to_scope(mode);

        // Check primary scope
        if let Some(keymap) = self.maps.get(&scope)
            && keymap.keys().any(|k| k.starts_with(keys) && k != keys)
        {
            return true;
        }

        // For Component + Normal mode, also check DefaultNormal
        if let KeymapScope::Component {
            mode: EditModeKind::Normal,
            ..
        } = scope
            && let Some(default_map) = self.maps.get(&KeymapScope::DefaultNormal)
            && default_map.keys().any(|k| k.starts_with(keys) && k != keys)
        {
            return true;
        }

        false
    }

    /// Convert `ModeState` to `KeymapScope`
    #[must_use]
    pub fn mode_to_scope(mode: &ModeState) -> KeymapScope {
        // SubMode takes precedence
        match &mode.sub_mode {
            SubMode::Command => return KeymapScope::SubMode(SubModeKind::Command),
            SubMode::OperatorPending { .. } => {
                return KeymapScope::SubMode(SubModeKind::OperatorPending);
            }
            SubMode::Interactor(id) => {
                return KeymapScope::SubMode(SubModeKind::Interactor(*id));
            }
            SubMode::None => {}
        }

        // ComponentId + EditMode determines keymap
        KeymapScope::Component {
            id: mode.interactor_id,
            mode: EditModeKind::from(&mode.edit_mode),
        }
    }

    // ========================================================================
    // Default keybinding setup methods
    // ========================================================================

    #[allow(clippy::too_many_lines)]
    fn setup_editor_keybindings(&mut self) {
        // Editor Normal mode
        let normal = self.get_scope_mut(KeymapScope::editor_normal());

        // Movement
        normal.insert(
            keys!['h'],
            KeyMapInner::with_command_id(builtin::CURSOR_LEFT).with_category("motion"),
        );
        normal.insert(
            keys!['j'],
            KeyMapInner::with_command_id(builtin::CURSOR_DOWN).with_category("motion"),
        );
        normal.insert(
            keys!['k'],
            KeyMapInner::with_command_id(builtin::CURSOR_UP).with_category("motion"),
        );
        normal.insert(
            keys!['l'],
            KeyMapInner::with_command_id(builtin::CURSOR_RIGHT).with_category("motion"),
        );
        normal.insert(
            keys!['0'],
            KeyMapInner::with_command_id(builtin::CURSOR_LINE_START).with_category("motion"),
        );
        normal.insert(
            keys!['$'],
            KeyMapInner::with_command_id(builtin::CURSOR_LINE_END).with_category("motion"),
        );
        normal.insert(
            keys!['w'],
            KeyMapInner::with_command_id(builtin::CURSOR_WORD_FORWARD).with_category("motion"),
        );
        normal.insert(
            keys!['b'],
            KeyMapInner::with_command_id(builtin::CURSOR_WORD_BACKWARD).with_category("motion"),
        );
        normal.insert(
            keys!['e'],
            KeyMapInner::with_command_id(builtin::CURSOR_WORD_END).with_category("motion"),
        );

        // Mode switching
        normal.insert(
            keys!['i'],
            KeyMapInner::with_command_id(builtin::ENTER_INSERT_MODE).with_category("mode"),
        );
        normal.insert(
            keys!['a'],
            KeyMapInner::with_command_id(builtin::ENTER_INSERT_MODE_AFTER).with_category("mode"),
        );
        normal.insert(
            keys!['A'],
            KeyMapInner::with_command_id(builtin::ENTER_INSERT_MODE_EOL).with_category("mode"),
        );
        normal.insert(
            keys!['o'],
            KeyMapInner::with_command_id(builtin::OPEN_LINE_BELOW).with_category("edit"),
        );
        normal.insert(
            keys!['O'],
            KeyMapInner::with_command_id(builtin::OPEN_LINE_ABOVE).with_category("edit"),
        );
        normal.insert(
            keys!['v'],
            KeyMapInner::with_command_id(builtin::ENTER_VISUAL_MODE).with_category("mode"),
        );
        normal.insert(
            keys!['V'],
            KeyMapInner::with_command_id(builtin::ENTER_VISUAL_LINE_MODE).with_category("mode"),
        );
        normal.insert(
            keys![(Ctrl 'v')],
            KeyMapInner::with_command_id(builtin::ENTER_VISUAL_BLOCK_MODE).with_category("mode"),
        );
        normal.insert(
            keys![':'],
            KeyMapInner::with_command_id(builtin::ENTER_COMMAND_MODE).with_category("mode"),
        );

        // Editing
        normal.insert(
            keys!['x'],
            KeyMapInner::with_command_id(builtin::DELETE_CHAR_FORWARD).with_category("edit"),
        );
        normal
            .insert(keys!['p'], KeyMapInner::with_command_id(builtin::PASTE).with_category("edit"));
        normal.insert(
            keys!['P'],
            KeyMapInner::with_command_id(builtin::PASTE_BEFORE).with_category("edit"),
        );

        // g-prefix bindings
        normal.insert(keys!['g'], KeyMapInner::new());
        normal.insert(
            keys!['g' 'g'],
            KeyMapInner::with_command_id(builtin::GOTO_FIRST_LINE).with_category("jump"),
        );
        normal.insert(
            keys!['G'],
            KeyMapInner::with_command_id(builtin::GOTO_LAST_LINE).with_category("jump"),
        );
        normal.insert(
            keys!['g' 't'],
            KeyMapInner::with_command_id(builtin::TAB_NEXT).with_category("window"),
        );
        normal.insert(
            keys!['g' 'T'],
            KeyMapInner::with_command_id(builtin::TAB_PREV).with_category("window"),
        );

        // Jump list
        normal.insert(
            keys![(Ctrl 'o')],
            KeyMapInner::with_command_id(builtin::JUMP_OLDER).with_category("jump"),
        );
        normal.insert(
            keys![(Ctrl 'i')],
            KeyMapInner::with_command_id(builtin::JUMP_NEWER).with_category("jump"),
        );

        // Undo/Redo
        normal
            .insert(keys!['u'], KeyMapInner::with_command_id(builtin::UNDO).with_category("edit"));
        normal.insert(
            keys![(Ctrl 'r')],
            KeyMapInner::with_command_id(builtin::REDO).with_category("edit"),
        );

        // Operators
        normal.insert(
            keys!['d'],
            KeyMapInner::with_command_id(builtin::ENTER_DELETE_OPERATOR).with_category("operator"),
        );
        normal.insert(
            keys!['d' 'd'],
            KeyMapInner::with_command_id(builtin::DELETE_LINE).with_category("operator"),
        );
        normal.insert(
            keys!['y'],
            KeyMapInner::with_command_id(builtin::ENTER_YANK_OPERATOR).with_category("operator"),
        );
        normal.insert(
            keys!['y' 'y'],
            KeyMapInner::with_command_id(builtin::YANK_LINE).with_category("operator"),
        );
        normal.insert(
            keys!['Y'],
            KeyMapInner::with_command_id(builtin::YANK_TO_END).with_category("operator"),
        );
        normal.insert(
            keys!['c'],
            KeyMapInner::with_command_id(builtin::ENTER_CHANGE_OPERATOR).with_category("operator"),
        );
        normal.insert(
            keys!['c' 'c'],
            KeyMapInner::with_command_id(builtin::CHANGE_LINE).with_category("operator"),
        );

        // Leader bindings
        normal.insert(keys![Space], KeyMapInner::new());
        // Settings menu keybindings are registered by the settings-menu plugin
        // Telescope keybindings are registered by the telescope plugin

        // Folding
        normal.insert(keys!['z'], KeyMapInner::new());
        normal.insert(
            keys!['z' 'a'],
            KeyMapInner::with_command_id(builtin::FOLD_TOGGLE).with_category("fold"),
        );
        normal.insert(
            keys!['z' 'o'],
            KeyMapInner::with_command_id(builtin::FOLD_OPEN).with_category("fold"),
        );
        normal.insert(
            keys!['z' 'c'],
            KeyMapInner::with_command_id(builtin::FOLD_CLOSE).with_category("fold"),
        );
        normal.insert(
            keys!['z' 'R'],
            KeyMapInner::with_command_id(builtin::FOLD_OPEN_ALL).with_category("fold"),
        );
        normal.insert(
            keys!['z' 'M'],
            KeyMapInner::with_command_id(builtin::FOLD_CLOSE_ALL).with_category("fold"),
        );

        // Buffer navigation
        normal.insert(
            keys!['H'],
            KeyMapInner::with_command_id(builtin::BUFFER_PREV).with_category("buffer"),
        );
        normal.insert(
            keys!['L'],
            KeyMapInner::with_command_id(builtin::BUFFER_NEXT).with_category("buffer"),
        );
        normal.insert(
            keys![Space 'b'],
            KeyMapInner::with_description("+buffer").with_category("buffer"),
        );
        normal.insert(
            keys![Space 'b' 'd'],
            KeyMapInner::with_command_id(builtin::BUFFER_DELETE).with_category("buffer"),
        );

        // Window management
        normal.insert(
            keys![Space 'w'],
            KeyMapInner::with_description("+window").with_category("window"),
        );
        normal.insert(
            keys![Space 'w' 'v'],
            KeyMapInner::with_command_id(builtin::WINDOW_SPLIT_VERTICAL).with_category("window"),
        );
        normal.insert(
            keys![Space 'w' 's'],
            KeyMapInner::with_command_id(builtin::WINDOW_SPLIT_HORIZONTAL).with_category("window"),
        );
        normal.insert(
            keys![Space 'w' 'c'],
            KeyMapInner::with_command_id(builtin::WINDOW_CLOSE).with_category("window"),
        );
        normal.insert(
            keys![Space 'w' 'o'],
            KeyMapInner::with_command_id(builtin::WINDOW_ONLY).with_category("window"),
        );
        normal.insert(
            keys![Space 'w' '='],
            KeyMapInner::with_command_id(builtin::WINDOW_EQUALIZE).with_category("window"),
        );

        // Window mode (Ctrl-W)
        normal.insert(
            keys![(Ctrl 'w')],
            KeyMapInner::with_command_id(builtin::ENTER_WINDOW_MODE).with_category("window"),
        );

        // Editor Insert mode
        let insert = self.get_scope_mut(KeymapScope::editor_insert());
        insert.insert(keys![Escape], KeyMapInner::with_command_id(builtin::ENTER_NORMAL_MODE));
        insert
            .insert(keys![Backspace], KeyMapInner::with_command_id(builtin::DELETE_CHAR_BACKWARD));
        insert.insert(keys![Enter], KeyMapInner::with_command_id(builtin::INSERT_NEWLINE));
        // Completion keybindings are registered by the completion plugin

        // Editor Visual mode
        let visual = self.get_scope_mut(KeymapScope::editor_visual());
        visual.insert(keys![Escape], KeyMapInner::with_command_id(builtin::ENTER_NORMAL_MODE));
        visual.insert(keys!['h'], KeyMapInner::with_command_id(builtin::VISUAL_EXTEND_LEFT));
        visual.insert(keys!['j'], KeyMapInner::with_command_id(builtin::VISUAL_EXTEND_DOWN));
        visual.insert(keys!['k'], KeyMapInner::with_command_id(builtin::VISUAL_EXTEND_UP));
        visual.insert(keys!['l'], KeyMapInner::with_command_id(builtin::VISUAL_EXTEND_RIGHT));
        visual.insert(keys!['d'], KeyMapInner::with_command_id(builtin::VISUAL_DELETE));
        visual.insert(keys!['y'], KeyMapInner::with_command_id(builtin::VISUAL_YANK));
        visual.insert(keys![':'], KeyMapInner::with_command_id(builtin::ENTER_COMMAND_MODE));
    }

    #[allow(clippy::too_many_lines)]
    fn setup_submode_keybindings(&mut self) {
        // Command mode
        let cmd = self.get_scope_mut(KeymapScope::SubMode(SubModeKind::Command));
        cmd.insert(keys![Escape], KeyMapInner::with_command_id(builtin::COMMAND_LINE_CANCEL));
        cmd.insert(keys![Enter], KeyMapInner::with_command_id(builtin::COMMAND_LINE_EXECUTE));
        cmd.insert(keys![Backspace], KeyMapInner::with_command_id(builtin::COMMAND_LINE_BACKSPACE));

        // Operator-pending mode
        let op = self.get_scope_mut(KeymapScope::SubMode(SubModeKind::OperatorPending));
        op.insert(keys![Escape], KeyMapInner::with_command_id(builtin::ENTER_NORMAL_MODE));
        op.insert(
            keys!['d'],
            KeyMapInner::with_description("delete line (dd)").with_category("operator"),
        );
        op.insert(
            keys!['y'],
            KeyMapInner::with_description("yank line (yy)").with_category("operator"),
        );
        op.insert(
            keys!['c'],
            KeyMapInner::with_description("change line (cc)").with_category("operator"),
        );
        op.insert(
            keys!['w'],
            KeyMapInner::with_description("word forward").with_category("motion"),
        );
        op.insert(
            keys!['b'],
            KeyMapInner::with_description("word backward").with_category("motion"),
        );
        op.insert(keys!['e'], KeyMapInner::with_description("word end").with_category("motion"));
        op.insert(keys!['$'], KeyMapInner::with_description("end of line").with_category("motion"));
        op.insert(
            keys!['0'],
            KeyMapInner::with_description("start of line").with_category("motion"),
        );
        op.insert(
            keys!['^'],
            KeyMapInner::with_description("first non-blank").with_category("motion"),
        );
        op.insert(keys!['j'], KeyMapInner::with_description("line down").with_category("motion"));
        op.insert(keys!['k'], KeyMapInner::with_description("line up").with_category("motion"));
        op.insert(keys!['G'], KeyMapInner::with_description("end of file").with_category("motion"));
        op.insert(keys!['g'], KeyMapInner::new());
        op.insert(
            keys!['g' 'g'],
            KeyMapInner::with_description("start of file").with_category("motion"),
        );
        op.insert(
            keys!['i'],
            KeyMapInner::with_description("inner text object").with_category("textobj"),
        );
        op.insert(
            keys!['a'],
            KeyMapInner::with_description("around text object").with_category("textobj"),
        );

        // Window mode (Ctrl-W sub-mode)
        let window =
            self.get_scope_mut(KeymapScope::SubMode(SubModeKind::Interactor(ComponentId::WINDOW)));
        // Cancel
        window.insert(keys![Escape], KeyMapInner::with_command_id(builtin::ENTER_NORMAL_MODE));
        // Focus navigation (exit after)
        window.insert(
            keys!['h'],
            KeyMapInner::with_command_id(builtin::WINDOW_MODE_FOCUS_LEFT).with_category("focus"),
        );
        window.insert(
            keys!['j'],
            KeyMapInner::with_command_id(builtin::WINDOW_MODE_FOCUS_DOWN).with_category("focus"),
        );
        window.insert(
            keys!['k'],
            KeyMapInner::with_command_id(builtin::WINDOW_MODE_FOCUS_UP).with_category("focus"),
        );
        window.insert(
            keys!['l'],
            KeyMapInner::with_command_id(builtin::WINDOW_MODE_FOCUS_RIGHT).with_category("focus"),
        );
        // Move window (H/J/K/L)
        window.insert(
            keys!['H'],
            KeyMapInner::with_command_id(builtin::WINDOW_MODE_MOVE_LEFT).with_category("move"),
        );
        window.insert(
            keys!['J'],
            KeyMapInner::with_command_id(builtin::WINDOW_MODE_MOVE_DOWN).with_category("move"),
        );
        window.insert(
            keys!['K'],
            KeyMapInner::with_command_id(builtin::WINDOW_MODE_MOVE_UP).with_category("move"),
        );
        window.insert(
            keys!['L'],
            KeyMapInner::with_command_id(builtin::WINDOW_MODE_MOVE_RIGHT).with_category("move"),
        );
        // Swap (x + direction)
        window.insert(keys!['x'], KeyMapInner::with_description("+swap").with_category("swap"));
        window.insert(
            keys!['x' 'h'],
            KeyMapInner::with_command_id(builtin::WINDOW_MODE_SWAP_LEFT).with_category("swap"),
        );
        window.insert(
            keys!['x' 'j'],
            KeyMapInner::with_command_id(builtin::WINDOW_MODE_SWAP_DOWN).with_category("swap"),
        );
        window.insert(
            keys!['x' 'k'],
            KeyMapInner::with_command_id(builtin::WINDOW_MODE_SWAP_UP).with_category("swap"),
        );
        window.insert(
            keys!['x' 'l'],
            KeyMapInner::with_command_id(builtin::WINDOW_MODE_SWAP_RIGHT).with_category("swap"),
        );
        // Split/close
        window.insert(
            keys!['s'],
            KeyMapInner::with_command_id(builtin::WINDOW_MODE_SPLIT_H).with_category("split"),
        );
        window.insert(
            keys!['v'],
            KeyMapInner::with_command_id(builtin::WINDOW_MODE_SPLIT_V).with_category("split"),
        );
        window.insert(
            keys!['c'],
            KeyMapInner::with_command_id(builtin::WINDOW_MODE_CLOSE).with_category("manage"),
        );
        window.insert(
            keys!['o'],
            KeyMapInner::with_command_id(builtin::WINDOW_MODE_ONLY).with_category("manage"),
        );
        window.insert(
            keys!['='],
            KeyMapInner::with_command_id(builtin::WINDOW_MODE_EQUALIZE).with_category("manage"),
        );
    }

    /// Setup default keybindings that apply to all components in Normal mode
    ///
    /// These serve as fallbacks when a component doesn't have its own binding.
    /// Plugins can override these by registering their own bindings.
    fn setup_default_normal_keybindings(&mut self) {
        let default = self.get_scope_mut(KeymapScope::DefaultNormal);

        // Window mode (Ctrl-W) - works in all plugin windows
        default.insert(
            keys![(Ctrl 'w')],
            KeyMapInner::with_command_id(builtin::ENTER_WINDOW_MODE).with_category("window"),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get_normal_keymap(keymap: &KeyMap) -> &HashMap<KeySequence, KeyMapInner> {
        keymap.get_scope(&KeymapScope::editor_normal())
    }

    #[test]
    fn test_keymap_has_buffer_navigation_keys() {
        let keymap = KeyMap::with_defaults();
        let normal = get_normal_keymap(&keymap);

        // Test H (prev buffer) key exists
        let h_binding = normal.get(&keys!['H']);
        assert!(h_binding.is_some(), "H key should be bound in normal mode");
        let h_inner = h_binding.unwrap();
        assert!(h_inner.command.is_some(), "H should have a command");
        assert_eq!(h_inner.category, Some("buffer"));

        // Test L (next buffer) key exists
        let l_binding = normal.get(&keys!['L']);
        assert!(l_binding.is_some(), "L key should be bound in normal mode");
        let l_inner = l_binding.unwrap();
        assert!(l_inner.command.is_some(), "L should have a command");
        assert_eq!(l_inner.category, Some("buffer"));
    }

    #[test]
    fn test_keymap_has_leader_buffer_prefix() {
        let keymap = KeyMap::with_defaults();
        let normal = get_normal_keymap(&keymap);

        // Test <leader>b prefix exists
        let b_prefix = normal.get(&keys![Space 'b']);
        assert!(b_prefix.is_some(), "<leader>b prefix should be bound in normal mode");
        let b_inner = b_prefix.unwrap();
        assert_eq!(b_inner.description, Some("+buffer".to_string()));
        assert_eq!(b_inner.category, Some("buffer"));

        // Test <leader>bd exists
        let delete_binding = normal.get(&keys![Space 'b' 'd']);
        assert!(delete_binding.is_some(), "<leader>bd should be bound in normal mode");
        let delete_inner = delete_binding.unwrap();
        assert!(delete_inner.command.is_some(), "<leader>bd should have a command");
        assert_eq!(delete_inner.category, Some("buffer"));
    }

    #[test]
    fn test_keymap_has_leader_window_prefix() {
        let keymap = KeyMap::with_defaults();
        let normal = get_normal_keymap(&keymap);

        // Test <leader>w prefix exists
        let w_prefix = normal.get(&keys![Space 'w']);
        assert!(w_prefix.is_some(), "<leader>w prefix should be bound in normal mode");
        let w_inner = w_prefix.unwrap();
        assert_eq!(w_inner.description, Some("+window".to_string()));
        assert_eq!(w_inner.category, Some("window"));
    }

    #[test]
    fn test_keymap_has_leader_window_bindings() {
        let keymap = KeyMap::with_defaults();
        let normal = get_normal_keymap(&keymap);

        // Test window bindings exist
        let window_keys = [
            (keys![Space 'w' 'v'], "vertical split"),
            (keys![Space 'w' 's'], "horizontal split"),
            (keys![Space 'w' 'c'], "close window"),
            (keys![Space 'w' 'o'], "only window"),
            (keys![Space 'w' '='], "equalize windows"),
        ];

        for (key, desc) in window_keys {
            let binding = normal.get(&key);
            assert!(binding.is_some(), "{desc} should be bound in normal mode");
            let inner = binding.unwrap();
            assert!(inner.command.is_some(), "{desc} should have a command");
            assert_eq!(inner.category, Some("window"), "{desc} should be in window group");
        }
    }

    #[test]
    fn test_buffer_command_ids_are_registered() {
        // Verify that buffer command IDs are properly defined
        assert_eq!(builtin::BUFFER_PREV.as_str(), "buffer_prev");
        assert_eq!(builtin::BUFFER_NEXT.as_str(), "buffer_next");
        assert_eq!(builtin::BUFFER_DELETE.as_str(), "buffer_delete");
    }

    #[test]
    fn test_ctrl_w_binding_exists() {
        let keymap = KeyMap::with_defaults();
        let normal = get_normal_keymap(&keymap);

        // Test Ctrl-W binding exists in editor_normal
        let ctrl_w = keys![(Ctrl 'w')];
        let binding = normal.get(&ctrl_w);
        assert!(binding.is_some(), "Ctrl-W should be bound in editor normal mode");
        let inner = binding.unwrap();
        assert!(inner.command.is_some(), "Ctrl-W should have a command");
    }

    #[test]
    fn test_lookup_binding_finds_ctrl_w() {
        use crate::modd::ModeState;

        let keymap = KeyMap::with_defaults();
        let mode = ModeState::normal();
        let ctrl_w = keys![(Ctrl 'w')];

        // lookup_binding should find Ctrl-W in editor normal mode
        let binding = keymap.lookup_binding(&mode, &ctrl_w);
        assert!(binding.is_some(), "lookup_binding should find Ctrl-W");
        let inner = binding.unwrap();
        assert!(inner.command.is_some(), "Ctrl-W binding should have a command");
    }

    #[test]
    fn test_default_normal_fallback() {
        use crate::modd::{ComponentId, ModeState};

        let keymap = KeyMap::with_defaults();

        // Create a mode for a hypothetical plugin component
        let plugin_mode = ModeState::new().set_interactor_id(ComponentId("my_plugin"));
        let ctrl_w = keys![(Ctrl 'w')];

        // lookup_binding should fallback to DefaultNormal and find Ctrl-W
        let binding = keymap.lookup_binding(&plugin_mode, &ctrl_w);
        assert!(binding.is_some(), "lookup_binding should fallback to DefaultNormal for Ctrl-W");
    }

    #[test]
    fn test_keymap_buffer_commands_use_correct_ids() {
        let keymap = KeyMap::with_defaults();
        let normal = get_normal_keymap(&keymap);

        // Check H key command is BUFFER_PREV
        let h_binding = normal.get(&keys!['H']).unwrap();
        if let Some(CommandRef::Registered(id)) = &h_binding.command {
            assert_eq!(id.as_str(), "buffer_prev");
        } else {
            panic!("H should have a registered command");
        }

        // Check L key command is BUFFER_NEXT
        let l_binding = normal.get(&keys!['L']).unwrap();
        if let Some(CommandRef::Registered(id)) = &l_binding.command {
            assert_eq!(id.as_str(), "buffer_next");
        } else {
            panic!("L should have a registered command");
        }

        // Check <leader>bd command is BUFFER_DELETE
        let bd_binding = normal.get(&keys![Space 'b' 'd']).unwrap();
        if let Some(CommandRef::Registered(id)) = &bd_binding.command {
            assert_eq!(id.as_str(), "buffer_delete");
        } else {
            panic!("<leader>bd should have a registered command");
        }
    }
}
