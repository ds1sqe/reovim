//! Key binding system for mapping key sequences to commands

use {
    crate::{
        command::{
            CommandId, CommandTrait, builtin::ToggleExplorerCommand, id::builtin,
            registry::CommandRegistry,
        },
        event::WhichKeyBinding,
        modd::{EditMode, ModeState, SubMode},
        ui_component::ComponentId,
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
    Leap,
}

/// Identifies which keymap a binding belongs to
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum KeymapScope {
    /// Component-specific scope: `ComponentId` + `EditModeKind`
    Component { id: ComponentId, mode: EditModeKind },
    /// Global sub-mode scope (applies across components)
    SubMode(SubModeKind),
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
    /// Key sequence (e.g., "h", "gg", " ff")
    pub keys: &'static str,
    /// Command to execute
    pub command: CommandRef,
    /// Optional hint for which-key display (overrides command description)
    pub hint: Option<&'static str>,
    /// Group for which-key categorization (e.g., "motion", "operator")
    pub group: Option<&'static str>,
}

impl KeyBinding {
    /// Create a binding with a registered command ID
    #[must_use]
    pub const fn id(keys: &'static str, id: CommandId) -> Self {
        Self {
            keys,
            command: CommandRef::Registered(id),
            hint: None,
            group: None,
        }
    }

    /// Create a binding with a command ID and group
    #[must_use]
    pub const fn id_group(keys: &'static str, id: CommandId, group: &'static str) -> Self {
        Self {
            keys,
            command: CommandRef::Registered(id),
            hint: None,
            group: Some(group),
        }
    }

    /// Create a hint-only binding (prefix node for which-key)
    #[must_use]
    pub const fn hint(keys: &'static str, hint: &'static str, group: &'static str) -> Self {
        Self {
            keys,
            command: CommandRef::Registered(CommandId::new("")),
            hint: Some(hint),
            group: Some(group),
        }
    }
}

impl std::fmt::Debug for KeyBinding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeyBinding")
            .field("keys", &self.keys)
            .field("hint", &self.hint)
            .field("group", &self.group)
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
pub struct KeyMapInner {
    /// If this is a terminal node, the command to execute
    pub command: Option<CommandRef>,
    /// Optional description hint for which-key (overrides command description)
    /// Used for dynamically-handled keys like operator motions
    pub hint: Option<String>,
    /// Group for which-key categorization (e.g., "motion", "operator", "mode")
    pub group: Option<&'static str>,
    /// Children for multi-key sequences (e.g., "dd", "gg")
    #[allow(dead_code)] // Infrastructure for proper trie-based lookup
    pub next: HashMap<String, Self>,
}

impl KeyMapInner {
    #[must_use]
    pub fn new() -> Self {
        Self {
            command: None,
            hint: None,
            group: None,
            next: HashMap::new(),
        }
    }

    /// Create a node with a `CommandRef`
    #[must_use]
    pub fn with_command_ref(cmd: CommandRef) -> Self {
        Self {
            command: Some(cmd),
            hint: None,
            group: None,
            next: HashMap::new(),
        }
    }

    /// Create a node with a registered command ID
    #[must_use]
    pub fn with_command_id(id: CommandId) -> Self {
        Self {
            command: Some(CommandRef::Registered(id)),
            hint: None,
            group: None,
            next: HashMap::new(),
        }
    }

    /// Create a node with an inline command
    #[must_use]
    pub fn with_inline_command(cmd: Arc<dyn CommandTrait>) -> Self {
        Self {
            command: Some(CommandRef::Inline(cmd)),
            hint: None,
            group: None,
            next: HashMap::new(),
        }
    }

    /// Create a hint-only node for which-key display (no command)
    /// Used for dynamically-handled keys like operator motions
    #[must_use]
    pub fn with_hint(description: impl Into<String>) -> Self {
        Self {
            command: None,
            hint: Some(description.into()),
            group: None,
            next: HashMap::new(),
        }
    }

    /// Set the group for this binding (builder pattern)
    #[must_use]
    pub const fn group(mut self, group: &'static str) -> Self {
        self.group = Some(group);
        self
    }

    /// Set the group if Some (builder pattern)
    #[must_use]
    pub const fn with_group_opt(mut self, group: Option<&'static str>) -> Self {
        self.group = group;
        self
    }

    /// Get the description for this binding
    ///
    /// Priority: hint > command description > prefix indicator
    #[must_use]
    pub fn get_description(&self, registry: &CommandRegistry) -> String {
        // Hint overrides everything
        if let Some(hint) = &self.hint {
            return hint.clone();
        }
        match &self.command {
            Some(CommandRef::Registered(id)) => registry
                .get(id)
                .map_or_else(|| id.as_str().to_string(), |cmd| cmd.description().to_string()),
            Some(CommandRef::Inline(cmd)) => cmd.description().to_string(),
            None => "+prefix".to_string(),
        }
    }

    /// Check if this is a prefix node (has no command and no hint)
    #[must_use]
    pub const fn is_prefix(&self) -> bool {
        self.command.is_none() && self.hint.is_none()
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
            .field("hint", &self.hint)
            .field("group", &self.group)
            .field("next_keys", &self.next.keys().collect::<Vec<_>>())
            .finish()
    }
}

/// Empty keymap for fallback returns
static EMPTY_MAP: std::sync::LazyLock<HashMap<String, KeyMapInner>> =
    std::sync::LazyLock::new(HashMap::new);

/// Keymap for each mode
///
/// Keymaps are organized by `KeymapScope`:
/// - `Component { id, mode }` for component-specific keymaps
/// - `SubMode` for global sub-mode keymaps (Command, `OperatorPending`, Leap)
#[derive(Default)]
pub struct KeyMap {
    /// All keymaps indexed by scope
    maps: HashMap<KeymapScope, HashMap<String, KeyMapInner>>,
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
        km.setup_explorer_keybindings();
        km.setup_telescope_keybindings();
        km.setup_settings_keybindings();
        km.setup_submode_keybindings();
        km
    }

    // ========================================================================
    // Scope-based API (new)
    // ========================================================================

    /// Bind a key to a command in a specific scope
    pub fn bind_scoped(&mut self, scope: KeymapScope, keys: &str, cmd: CommandRef) {
        self.maps
            .entry(scope)
            .or_default()
            .insert(keys.to_string(), KeyMapInner::with_command_ref(cmd));
    }

    /// Bind with full metadata (hint, group)
    pub fn bind_with_metadata(&mut self, scope: KeymapScope, binding: KeyBinding) {
        let inner = if binding.hint.is_some() {
            KeyMapInner::with_hint(binding.hint.unwrap_or_default()).with_group_opt(binding.group)
        } else {
            KeyMapInner::with_command_ref(binding.command).with_group_opt(binding.group)
        };
        self.maps
            .entry(scope)
            .or_default()
            .insert(binding.keys.to_string(), inner);
    }

    /// Unbind a key in a specific scope
    pub fn unbind_scoped(&mut self, scope: &KeymapScope, keys: &str) {
        if let Some(map) = self.maps.get_mut(scope) {
            map.remove(keys);
        }
    }

    /// Get the keymap for a specific scope
    #[must_use]
    pub fn get_scope(&self, scope: &KeymapScope) -> &HashMap<String, KeyMapInner> {
        self.maps.get(scope).unwrap_or(&EMPTY_MAP)
    }

    /// Get mutable access to a scope's keymap
    pub fn get_scope_mut(&mut self, scope: KeymapScope) -> &mut HashMap<String, KeyMapInner> {
        self.maps.entry(scope).or_default()
    }

    /// Iterate over all scopes and their keymaps
    pub fn iter_scopes(
        &self,
    ) -> impl Iterator<Item = (&KeymapScope, &HashMap<String, KeyMapInner>)> {
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
    pub fn get_keymap_for_mode(&self, mode: &ModeState) -> &HashMap<String, KeyMapInner> {
        let scope = Self::mode_to_scope(mode);
        self.maps.get(&scope).unwrap_or(&EMPTY_MAP)
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
            SubMode::Leap { .. } => return KeymapScope::SubMode(SubModeKind::Leap),
            SubMode::None => {}
        }

        // ComponentId + EditMode determines keymap
        KeymapScope::Component {
            id: mode.interactor_id,
            mode: EditModeKind::from(&mode.edit_mode),
        }
    }

    /// Get all available bindings for a given prefix in a mode
    ///
    /// This is used by the which-key feature to display available keybindings.
    /// Keymap selection is based on Focus + `EditMode` + `SubMode`:
    /// - `SubMode` takes precedence (Command, `OperatorPending`)
    /// - Then Focus + `EditMode` determines the keymap
    #[must_use]
    pub fn get_bindings_for_prefix(
        &self,
        mode: &ModeState,
        prefix: &str,
        registry: &CommandRegistry,
    ) -> Vec<WhichKeyBinding> {
        let keymap = self.get_keymap_for_mode(mode);

        let mut bindings = Vec::new();
        let mut seen_keys = std::collections::HashSet::new();

        for (key, inner) in keymap {
            // Check if key starts with the prefix and is longer
            if key.starts_with(prefix) && key.len() > prefix.len() {
                // Get the next key segment after the prefix
                let suffix = &key[prefix.len()..];
                let next_key = Self::first_key_segment(suffix);

                // Avoid duplicates (e.g., "gg" and "gG" both show "g" after "g" prefix)
                if seen_keys.contains(next_key) {
                    continue;
                }
                seen_keys.insert(next_key.to_string());

                let description = inner.get_description(registry);
                let is_prefix = inner.is_prefix();

                bindings.push(WhichKeyBinding {
                    key: next_key.to_string(),
                    description,
                    is_prefix,
                    group: inner.group.map(String::from),
                });
            }
        }

        // Sort by key for consistent display
        bindings.sort_by(|a, b| a.key.cmp(&b.key));
        bindings
    }

    /// Extract the first key segment from a key sequence
    ///
    /// Handles special keys like `<C-x>`, `<Escape>`, etc.
    fn first_key_segment(s: &str) -> &str {
        // Handle special keys like <C-x>, <Escape>, etc.
        if s.starts_with('<')
            && let Some(end) = s.find('>')
        {
            return &s[..=end];
        }
        // Single character key
        s.chars().next().map_or(s, |c| &s[..c.len_utf8()])
    }

    // ========================================================================
    // Default keybinding setup methods
    // ========================================================================

    #[allow(clippy::too_many_lines)]
    fn setup_editor_keybindings(&mut self) {
        // Editor Normal mode
        let normal = self.get_scope_mut(KeymapScope::editor_normal());

        // Movement
        normal
            .insert("h".into(), KeyMapInner::with_command_id(builtin::CURSOR_LEFT).group("motion"));
        normal
            .insert("j".into(), KeyMapInner::with_command_id(builtin::CURSOR_DOWN).group("motion"));
        normal.insert("k".into(), KeyMapInner::with_command_id(builtin::CURSOR_UP).group("motion"));
        normal.insert(
            "l".into(),
            KeyMapInner::with_command_id(builtin::CURSOR_RIGHT).group("motion"),
        );
        normal.insert(
            "0".into(),
            KeyMapInner::with_command_id(builtin::CURSOR_LINE_START).group("motion"),
        );
        normal.insert(
            "$".into(),
            KeyMapInner::with_command_id(builtin::CURSOR_LINE_END).group("motion"),
        );
        normal.insert(
            "w".into(),
            KeyMapInner::with_command_id(builtin::CURSOR_WORD_FORWARD).group("motion"),
        );
        normal.insert(
            "b".into(),
            KeyMapInner::with_command_id(builtin::CURSOR_WORD_BACKWARD).group("motion"),
        );
        normal.insert(
            "e".into(),
            KeyMapInner::with_command_id(builtin::CURSOR_WORD_END).group("motion"),
        );

        // Mode switching
        normal.insert(
            "i".into(),
            KeyMapInner::with_command_id(builtin::ENTER_INSERT_MODE).group("mode"),
        );
        normal.insert(
            "a".into(),
            KeyMapInner::with_command_id(builtin::ENTER_INSERT_MODE_AFTER).group("mode"),
        );
        normal.insert(
            "A".into(),
            KeyMapInner::with_command_id(builtin::ENTER_INSERT_MODE_EOL).group("mode"),
        );
        normal.insert(
            "o".into(),
            KeyMapInner::with_command_id(builtin::OPEN_LINE_BELOW).group("edit"),
        );
        normal.insert(
            "O".into(),
            KeyMapInner::with_command_id(builtin::OPEN_LINE_ABOVE).group("edit"),
        );
        normal.insert(
            "v".into(),
            KeyMapInner::with_command_id(builtin::ENTER_VISUAL_MODE).group("mode"),
        );
        normal.insert(
            "V".into(),
            KeyMapInner::with_command_id(builtin::ENTER_VISUAL_LINE_MODE).group("mode"),
        );
        normal.insert(
            "<C-v>".into(),
            KeyMapInner::with_command_id(builtin::ENTER_VISUAL_BLOCK_MODE).group("mode"),
        );
        normal.insert(
            ":".into(),
            KeyMapInner::with_command_id(builtin::ENTER_COMMAND_MODE).group("mode"),
        );

        // Editing
        normal.insert(
            "x".into(),
            KeyMapInner::with_command_id(builtin::DELETE_CHAR_FORWARD).group("edit"),
        );
        normal.insert("p".into(), KeyMapInner::with_command_id(builtin::PASTE).group("edit"));
        normal
            .insert("P".into(), KeyMapInner::with_command_id(builtin::PASTE_BEFORE).group("edit"));

        // g-prefix bindings
        normal.insert("g".into(), KeyMapInner::new());
        normal.insert(
            "gg".into(),
            KeyMapInner::with_command_id(builtin::GOTO_FIRST_LINE).group("jump"),
        );
        normal.insert(
            "G".into(),
            KeyMapInner::with_command_id(builtin::GOTO_LAST_LINE).group("jump"),
        );
        normal.insert("gt".into(), KeyMapInner::with_command_id(builtin::TAB_NEXT).group("window"));
        normal.insert("gT".into(), KeyMapInner::with_command_id(builtin::TAB_PREV).group("window"));

        // Jump list
        normal.insert(
            "<C-o>".into(),
            KeyMapInner::with_command_id(builtin::JUMP_OLDER).group("jump"),
        );
        normal.insert(
            "<C-i>".into(),
            KeyMapInner::with_command_id(builtin::JUMP_NEWER).group("jump"),
        );

        // Undo/Redo
        normal.insert("u".into(), KeyMapInner::with_command_id(builtin::UNDO).group("edit"));
        normal.insert("<C-r>".into(), KeyMapInner::with_command_id(builtin::REDO).group("edit"));

        // Operators
        normal.insert(
            "d".into(),
            KeyMapInner::with_command_id(builtin::ENTER_DELETE_OPERATOR).group("operator"),
        );
        normal.insert(
            "dd".into(),
            KeyMapInner::with_command_id(builtin::DELETE_LINE).group("operator"),
        );
        normal.insert(
            "y".into(),
            KeyMapInner::with_command_id(builtin::ENTER_YANK_OPERATOR).group("operator"),
        );
        normal.insert(
            "yy".into(),
            KeyMapInner::with_command_id(builtin::YANK_LINE).group("operator"),
        );
        normal.insert(
            "Y".into(),
            KeyMapInner::with_command_id(builtin::YANK_TO_END).group("operator"),
        );
        normal.insert(
            "c".into(),
            KeyMapInner::with_command_id(builtin::ENTER_CHANGE_OPERATOR).group("operator"),
        );
        normal.insert(
            "cc".into(),
            KeyMapInner::with_command_id(builtin::CHANGE_LINE).group("operator"),
        );

        // Leader bindings
        normal.insert(" ".into(), KeyMapInner::new());
        normal.insert(
            " e".into(),
            KeyMapInner::with_inline_command(Arc::new(ToggleExplorerCommand)).group("misc"),
        );
        normal.insert(
            " s".into(),
            KeyMapInner::with_command_id(builtin::SETTINGS_MENU_OPEN).group("misc"),
        );
        normal.insert(" f".into(), KeyMapInner::new());
        normal.insert(
            " ff".into(),
            KeyMapInner::with_command_id(builtin::TELESCOPE_FIND_FILES).group("telescope"),
        );
        normal.insert(
            " fb".into(),
            KeyMapInner::with_command_id(builtin::TELESCOPE_FIND_BUFFERS).group("telescope"),
        );
        normal.insert(
            " fg".into(),
            KeyMapInner::with_command_id(builtin::TELESCOPE_LIVE_GREP).group("telescope"),
        );
        normal.insert(
            " fr".into(),
            KeyMapInner::with_command_id(builtin::TELESCOPE_RECENT).group("telescope"),
        );
        normal.insert(
            " fc".into(),
            KeyMapInner::with_command_id(builtin::TELESCOPE_COMMANDS).group("telescope"),
        );
        normal.insert(
            " fh".into(),
            KeyMapInner::with_command_id(builtin::TELESCOPE_HELP).group("telescope"),
        );
        normal.insert(
            " fk".into(),
            KeyMapInner::with_command_id(builtin::TELESCOPE_KEYMAPS).group("telescope"),
        );

        // Leap
        normal
            .insert("s".into(), KeyMapInner::with_command_id(builtin::LEAP_FORWARD).group("jump"));
        normal
            .insert("S".into(), KeyMapInner::with_command_id(builtin::LEAP_BACKWARD).group("jump"));

        // Folding
        normal.insert("z".into(), KeyMapInner::new());
        normal
            .insert("za".into(), KeyMapInner::with_command_id(builtin::FOLD_TOGGLE).group("fold"));
        normal.insert("zo".into(), KeyMapInner::with_command_id(builtin::FOLD_OPEN).group("fold"));
        normal.insert("zc".into(), KeyMapInner::with_command_id(builtin::FOLD_CLOSE).group("fold"));
        normal.insert(
            "zR".into(),
            KeyMapInner::with_command_id(builtin::FOLD_OPEN_ALL).group("fold"),
        );
        normal.insert(
            "zM".into(),
            KeyMapInner::with_command_id(builtin::FOLD_CLOSE_ALL).group("fold"),
        );

        // Buffer navigation
        normal
            .insert("H".into(), KeyMapInner::with_command_id(builtin::BUFFER_PREV).group("buffer"));
        normal
            .insert("L".into(), KeyMapInner::with_command_id(builtin::BUFFER_NEXT).group("buffer"));
        normal.insert(" b".into(), KeyMapInner::with_hint("+buffer").group("buffer"));
        normal.insert(
            " bd".into(),
            KeyMapInner::with_command_id(builtin::BUFFER_DELETE).group("buffer"),
        );

        // Window management
        normal.insert(" w".into(), KeyMapInner::with_hint("+window").group("window"));
        normal.insert(
            " wh".into(),
            KeyMapInner::with_command_id(builtin::WINDOW_FOCUS_OR_SPLIT_LEFT).group("window"),
        );
        normal.insert(
            " wj".into(),
            KeyMapInner::with_command_id(builtin::WINDOW_FOCUS_OR_SPLIT_DOWN).group("window"),
        );
        normal.insert(
            " wk".into(),
            KeyMapInner::with_command_id(builtin::WINDOW_FOCUS_OR_SPLIT_UP).group("window"),
        );
        normal.insert(
            " wl".into(),
            KeyMapInner::with_command_id(builtin::WINDOW_FOCUS_OR_SPLIT_RIGHT).group("window"),
        );
        normal.insert(
            " wv".into(),
            KeyMapInner::with_command_id(builtin::WINDOW_SPLIT_VERTICAL).group("window"),
        );
        normal.insert(
            " ws".into(),
            KeyMapInner::with_command_id(builtin::WINDOW_SPLIT_HORIZONTAL).group("window"),
        );
        normal.insert(
            " wc".into(),
            KeyMapInner::with_command_id(builtin::WINDOW_CLOSE).group("window"),
        );
        normal.insert(
            " wo".into(),
            KeyMapInner::with_command_id(builtin::WINDOW_ONLY).group("window"),
        );
        normal.insert(
            " w=".into(),
            KeyMapInner::with_command_id(builtin::WINDOW_EQUALIZE).group("window"),
        );

        // Editor Insert mode
        let insert = self.get_scope_mut(KeymapScope::editor_insert());
        insert.insert("Escape".into(), KeyMapInner::with_command_id(builtin::ENTER_NORMAL_MODE));
        insert.insert(
            "Backspace".into(),
            KeyMapInner::with_command_id(builtin::DELETE_CHAR_BACKWARD),
        );
        insert.insert("Enter".into(), KeyMapInner::with_command_id(builtin::INSERT_NEWLINE));
        insert.insert("C-Space".into(), KeyMapInner::with_command_id(builtin::COMPLETION_TRIGGER));
        insert.insert("C-n".into(), KeyMapInner::with_command_id(builtin::COMPLETION_NEXT));
        insert.insert("C-p".into(), KeyMapInner::with_command_id(builtin::COMPLETION_PREV));
        insert.insert("Tab".into(), KeyMapInner::with_command_id(builtin::COMPLETION_CONFIRM));
        insert.insert("C-e".into(), KeyMapInner::with_command_id(builtin::COMPLETION_DISMISS));

        // Editor Visual mode
        let visual = self.get_scope_mut(KeymapScope::editor_visual());
        visual.insert("Escape".into(), KeyMapInner::with_command_id(builtin::ENTER_NORMAL_MODE));
        visual.insert("h".into(), KeyMapInner::with_command_id(builtin::VISUAL_EXTEND_LEFT));
        visual.insert("j".into(), KeyMapInner::with_command_id(builtin::VISUAL_EXTEND_DOWN));
        visual.insert("k".into(), KeyMapInner::with_command_id(builtin::VISUAL_EXTEND_UP));
        visual.insert("l".into(), KeyMapInner::with_command_id(builtin::VISUAL_EXTEND_RIGHT));
        visual.insert("d".into(), KeyMapInner::with_command_id(builtin::VISUAL_DELETE));
        visual.insert("y".into(), KeyMapInner::with_command_id(builtin::VISUAL_YANK));
        visual.insert(":".into(), KeyMapInner::with_command_id(builtin::ENTER_COMMAND_MODE));
    }

    fn setup_explorer_keybindings(&mut self) {
        // Explorer Normal mode
        let scope = KeymapScope::Component {
            id: ComponentId::EXPLORER,
            mode: EditModeKind::Normal,
        };
        let explorer = self.get_scope_mut(scope);

        // Navigation
        explorer.insert("j".into(), KeyMapInner::with_command_id(builtin::EXPLORER_CURSOR_DOWN));
        explorer.insert("k".into(), KeyMapInner::with_command_id(builtin::EXPLORER_CURSOR_UP));
        explorer.insert("Ctrl+d".into(), KeyMapInner::with_command_id(builtin::EXPLORER_PAGE_DOWN));
        explorer.insert("Ctrl+u".into(), KeyMapInner::with_command_id(builtin::EXPLORER_PAGE_UP));
        explorer.insert("g".into(), KeyMapInner::new());
        explorer.insert("gg".into(), KeyMapInner::with_command_id(builtin::EXPLORER_GOTO_FIRST));
        explorer.insert("G".into(), KeyMapInner::with_command_id(builtin::EXPLORER_GOTO_LAST));

        // Tree operations
        explorer.insert("Enter".into(), KeyMapInner::with_command_id(builtin::EXPLORER_OPEN_NODE));
        explorer.insert("o".into(), KeyMapInner::with_command_id(builtin::EXPLORER_TOGGLE_NODE));
        explorer.insert("x".into(), KeyMapInner::with_command_id(builtin::EXPLORER_CLOSE_PARENT));
        explorer.insert("u".into(), KeyMapInner::with_command_id(builtin::EXPLORER_GO_TO_PARENT));
        explorer.insert("R".into(), KeyMapInner::with_command_id(builtin::EXPLORER_REFRESH));

        // Display
        explorer.insert("I".into(), KeyMapInner::with_command_id(builtin::EXPLORER_TOGGLE_HIDDEN));
        explorer.insert("S".into(), KeyMapInner::with_command_id(builtin::EXPLORER_TOGGLE_SIZES));

        // File operations
        explorer.insert("a".into(), KeyMapInner::with_command_id(builtin::EXPLORER_CREATE_FILE));
        explorer.insert("A".into(), KeyMapInner::with_command_id(builtin::EXPLORER_CREATE_DIR));
        explorer.insert("r".into(), KeyMapInner::with_command_id(builtin::EXPLORER_RENAME));
        explorer.insert("D".into(), KeyMapInner::with_command_id(builtin::EXPLORER_DELETE));

        // Clipboard
        explorer.insert("y".into(), KeyMapInner::new());
        explorer.insert("yy".into(), KeyMapInner::with_command_id(builtin::EXPLORER_YANK));
        explorer.insert("d".into(), KeyMapInner::new());
        explorer.insert("dd".into(), KeyMapInner::with_command_id(builtin::EXPLORER_CUT));
        explorer.insert("p".into(), KeyMapInner::with_command_id(builtin::EXPLORER_PASTE));

        // Selection
        explorer.insert("v".into(), KeyMapInner::with_command_id(builtin::EXPLORER_VISUAL_MODE));
        explorer.insert("V".into(), KeyMapInner::with_command_id(builtin::EXPLORER_SELECT_ALL));

        // Filter
        explorer.insert("/".into(), KeyMapInner::with_command_id(builtin::EXPLORER_FILTER));
        explorer.insert("C".into(), KeyMapInner::with_command_id(builtin::EXPLORER_CLEAR_FILTER));

        // Window
        explorer.insert("Tab".into(), KeyMapInner::with_command_id(builtin::EXPLORER_FOCUS_EDITOR));
        explorer
            .insert("Escape".into(), KeyMapInner::with_command_id(builtin::EXPLORER_FOCUS_EDITOR));
        explorer.insert("<C-h>".into(), KeyMapInner::with_command_id(builtin::WINDOW_FOCUS_LEFT));
        explorer.insert("<C-j>".into(), KeyMapInner::with_command_id(builtin::WINDOW_FOCUS_DOWN));
        explorer.insert("<C-k>".into(), KeyMapInner::with_command_id(builtin::WINDOW_FOCUS_UP));
        explorer.insert("<C-l>".into(), KeyMapInner::with_command_id(builtin::WINDOW_FOCUS_RIGHT));

        // Leader
        explorer.insert(" ".into(), KeyMapInner::new());
        explorer
            .insert(" e".into(), KeyMapInner::with_inline_command(Arc::new(ToggleExplorerCommand)));

        // Explorer Input mode
        let input_scope = KeymapScope::Component {
            id: ComponentId::EXPLORER,
            mode: EditModeKind::Insert,
        };
        let explorer_input = self.get_scope_mut(input_scope);
        explorer_input
            .insert("Escape".into(), KeyMapInner::with_command_id(builtin::EXPLORER_CANCEL_INPUT));
        explorer_input
            .insert("Enter".into(), KeyMapInner::with_command_id(builtin::EXPLORER_CONFIRM_INPUT));
        explorer_input.insert(
            "Backspace".into(),
            KeyMapInner::with_command_id(builtin::EXPLORER_INPUT_BACKSPACE),
        );
    }

    fn setup_telescope_keybindings(&mut self) {
        // Telescope Normal mode
        let normal_scope = KeymapScope::Component {
            id: ComponentId::TELESCOPE,
            mode: EditModeKind::Normal,
        };
        let tn = self.get_scope_mut(normal_scope);
        tn.insert("j".into(), KeyMapInner::with_command_id(builtin::TELESCOPE_NEXT));
        tn.insert("k".into(), KeyMapInner::with_command_id(builtin::TELESCOPE_PREV));
        tn.insert("g".into(), KeyMapInner::new());
        tn.insert("gg".into(), KeyMapInner::with_command_id(builtin::TELESCOPE_GOTO_FIRST));
        tn.insert("G".into(), KeyMapInner::with_command_id(builtin::TELESCOPE_GOTO_LAST));
        tn.insert("C-d".into(), KeyMapInner::with_command_id(builtin::TELESCOPE_PAGE_DOWN));
        tn.insert("C-u".into(), KeyMapInner::with_command_id(builtin::TELESCOPE_PAGE_UP));
        tn.insert("i".into(), KeyMapInner::with_command_id(builtin::TELESCOPE_ENTER_INSERT));
        tn.insert("Escape".into(), KeyMapInner::with_command_id(builtin::TELESCOPE_CLOSE));
        tn.insert("Enter".into(), KeyMapInner::with_command_id(builtin::TELESCOPE_CONFIRM));

        // Telescope Insert mode
        let insert_scope = KeymapScope::Component {
            id: ComponentId::TELESCOPE,
            mode: EditModeKind::Insert,
        };
        let ti = self.get_scope_mut(insert_scope);
        ti.insert("C-n".into(), KeyMapInner::with_command_id(builtin::TELESCOPE_NEXT));
        ti.insert("C-p".into(), KeyMapInner::with_command_id(builtin::TELESCOPE_PREV));
        ti.insert("Down".into(), KeyMapInner::with_command_id(builtin::TELESCOPE_NEXT));
        ti.insert("Up".into(), KeyMapInner::with_command_id(builtin::TELESCOPE_PREV));
        ti.insert("Tab".into(), KeyMapInner::with_command_id(builtin::TELESCOPE_NEXT));
        ti.insert("S-Tab".into(), KeyMapInner::with_command_id(builtin::TELESCOPE_PREV));
        ti.insert("C-u".into(), KeyMapInner::with_command_id(builtin::TELESCOPE_PAGE_UP));
        ti.insert("C-d".into(), KeyMapInner::with_command_id(builtin::TELESCOPE_PAGE_DOWN));
        ti.insert("Backspace".into(), KeyMapInner::with_command_id(builtin::TELESCOPE_DELETE_CHAR));
        ti.insert("Escape".into(), KeyMapInner::with_command_id(builtin::TELESCOPE_ENTER_NORMAL));
        ti.insert("Enter".into(), KeyMapInner::with_command_id(builtin::TELESCOPE_CONFIRM));
    }

    fn setup_settings_keybindings(&mut self) {
        let scope = KeymapScope::Component {
            id: ComponentId::SETTINGS,
            mode: EditModeKind::Normal,
        };
        let settings = self.get_scope_mut(scope);

        // Navigation
        settings.insert("j".into(), KeyMapInner::with_command_id(builtin::SETTINGS_MENU_NEXT));
        settings.insert("k".into(), KeyMapInner::with_command_id(builtin::SETTINGS_MENU_PREV));
        settings.insert("Down".into(), KeyMapInner::with_command_id(builtin::SETTINGS_MENU_NEXT));
        settings.insert("Up".into(), KeyMapInner::with_command_id(builtin::SETTINGS_MENU_PREV));

        // Toggle/Cycle
        settings.insert(" ".into(), KeyMapInner::with_command_id(builtin::SETTINGS_MENU_TOGGLE));
        settings
            .insert("l".into(), KeyMapInner::with_command_id(builtin::SETTINGS_MENU_CYCLE_NEXT));
        settings
            .insert("h".into(), KeyMapInner::with_command_id(builtin::SETTINGS_MENU_CYCLE_PREV));
        settings.insert(
            "Right".into(),
            KeyMapInner::with_command_id(builtin::SETTINGS_MENU_CYCLE_NEXT),
        );
        settings
            .insert("Left".into(), KeyMapInner::with_command_id(builtin::SETTINGS_MENU_CYCLE_PREV));

        // Inc/Dec
        settings.insert("+".into(), KeyMapInner::with_command_id(builtin::SETTINGS_MENU_INCREMENT));
        settings.insert("-".into(), KeyMapInner::with_command_id(builtin::SETTINGS_MENU_DECREMENT));

        // Action
        settings
            .insert("Enter".into(), KeyMapInner::with_command_id(builtin::SETTINGS_MENU_EXECUTE));
        settings
            .insert("Escape".into(), KeyMapInner::with_command_id(builtin::SETTINGS_MENU_CLOSE));
        settings.insert("q".into(), KeyMapInner::with_command_id(builtin::SETTINGS_MENU_CLOSE));

        // Quick select
        settings.insert("1".into(), KeyMapInner::with_command_id(builtin::SETTINGS_MENU_QUICK_1));
        settings.insert("2".into(), KeyMapInner::with_command_id(builtin::SETTINGS_MENU_QUICK_2));
        settings.insert("3".into(), KeyMapInner::with_command_id(builtin::SETTINGS_MENU_QUICK_3));
        settings.insert("4".into(), KeyMapInner::with_command_id(builtin::SETTINGS_MENU_QUICK_4));
        settings.insert("5".into(), KeyMapInner::with_command_id(builtin::SETTINGS_MENU_QUICK_5));
        settings.insert("6".into(), KeyMapInner::with_command_id(builtin::SETTINGS_MENU_QUICK_6));
        settings.insert("7".into(), KeyMapInner::with_command_id(builtin::SETTINGS_MENU_QUICK_7));
        settings.insert("8".into(), KeyMapInner::with_command_id(builtin::SETTINGS_MENU_QUICK_8));
        settings.insert("9".into(), KeyMapInner::with_command_id(builtin::SETTINGS_MENU_QUICK_9));
    }

    fn setup_submode_keybindings(&mut self) {
        // Command mode
        let cmd = self.get_scope_mut(KeymapScope::SubMode(SubModeKind::Command));
        cmd.insert("Escape".into(), KeyMapInner::with_command_id(builtin::COMMAND_LINE_CANCEL));
        cmd.insert("Enter".into(), KeyMapInner::with_command_id(builtin::COMMAND_LINE_EXECUTE));
        cmd.insert(
            "Backspace".into(),
            KeyMapInner::with_command_id(builtin::COMMAND_LINE_BACKSPACE),
        );

        // Operator-pending mode
        let op = self.get_scope_mut(KeymapScope::SubMode(SubModeKind::OperatorPending));
        op.insert("<Escape>".into(), KeyMapInner::with_command_id(builtin::ENTER_NORMAL_MODE));
        op.insert("d".into(), KeyMapInner::with_hint("delete line (dd)").group("operator"));
        op.insert("y".into(), KeyMapInner::with_hint("yank line (yy)").group("operator"));
        op.insert("c".into(), KeyMapInner::with_hint("change line (cc)").group("operator"));
        op.insert("w".into(), KeyMapInner::with_hint("word forward").group("motion"));
        op.insert("b".into(), KeyMapInner::with_hint("word backward").group("motion"));
        op.insert("e".into(), KeyMapInner::with_hint("word end").group("motion"));
        op.insert("$".into(), KeyMapInner::with_hint("end of line").group("motion"));
        op.insert("0".into(), KeyMapInner::with_hint("start of line").group("motion"));
        op.insert("^".into(), KeyMapInner::with_hint("first non-blank").group("motion"));
        op.insert("j".into(), KeyMapInner::with_hint("line down").group("motion"));
        op.insert("k".into(), KeyMapInner::with_hint("line up").group("motion"));
        op.insert("G".into(), KeyMapInner::with_hint("end of file").group("motion"));
        op.insert("g".into(), KeyMapInner::new());
        op.insert("gg".into(), KeyMapInner::with_hint("start of file").group("motion"));
        op.insert("i".into(), KeyMapInner::with_hint("inner text object").group("textobj"));
        op.insert("a".into(), KeyMapInner::with_hint("around text object").group("textobj"));

        // Leap mode
        let leap = self.get_scope_mut(KeymapScope::SubMode(SubModeKind::Leap));
        leap.insert("Escape".into(), KeyMapInner::with_command_id(builtin::LEAP_CANCEL));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get_normal_keymap(keymap: &KeyMap) -> &HashMap<String, KeyMapInner> {
        keymap.get_scope(&KeymapScope::editor_normal())
    }

    #[test]
    fn test_keymap_has_buffer_navigation_keys() {
        let keymap = KeyMap::with_defaults();
        let normal = get_normal_keymap(&keymap);

        // Test H (prev buffer) key exists
        let h_binding = normal.get("H");
        assert!(h_binding.is_some(), "H key should be bound in normal mode");
        let h_inner = h_binding.unwrap();
        assert!(h_inner.command.is_some(), "H should have a command");
        assert_eq!(h_inner.group, Some("buffer"));

        // Test L (next buffer) key exists
        let l_binding = normal.get("L");
        assert!(l_binding.is_some(), "L key should be bound in normal mode");
        let l_inner = l_binding.unwrap();
        assert!(l_inner.command.is_some(), "L should have a command");
        assert_eq!(l_inner.group, Some("buffer"));
    }

    #[test]
    fn test_keymap_has_leader_buffer_prefix() {
        let keymap = KeyMap::with_defaults();
        let normal = get_normal_keymap(&keymap);

        // Test <leader>b prefix exists
        let b_prefix = normal.get(" b");
        assert!(b_prefix.is_some(), "<leader>b prefix should be bound in normal mode");
        let b_inner = b_prefix.unwrap();
        assert_eq!(b_inner.hint, Some("+buffer".to_string()));
        assert_eq!(b_inner.group, Some("buffer"));

        // Test <leader>bd exists
        let delete_binding = normal.get(" bd");
        assert!(delete_binding.is_some(), "<leader>bd should be bound in normal mode");
        let delete_inner = delete_binding.unwrap();
        assert!(delete_inner.command.is_some(), "<leader>bd should have a command");
        assert_eq!(delete_inner.group, Some("buffer"));
    }

    #[test]
    fn test_keymap_has_leader_window_prefix() {
        let keymap = KeyMap::with_defaults();
        let normal = get_normal_keymap(&keymap);

        // Test <leader>w prefix exists
        let w_prefix = normal.get(" w");
        assert!(w_prefix.is_some(), "<leader>w prefix should be bound in normal mode");
        let w_inner = w_prefix.unwrap();
        assert_eq!(w_inner.hint, Some("+window".to_string()));
        assert_eq!(w_inner.group, Some("window"));
    }

    #[test]
    fn test_keymap_has_leader_window_bindings() {
        let keymap = KeyMap::with_defaults();
        let normal = get_normal_keymap(&keymap);

        let window_keys = [
            (" wh", "focus or split left"),
            (" wj", "focus or split down"),
            (" wk", "focus or split up"),
            (" wl", "focus or split right"),
            (" wv", "vertical split"),
            (" ws", "horizontal split"),
            (" wc", "close window"),
            (" wo", "only window"),
            (" w=", "equalize windows"),
        ];

        for (key, desc) in window_keys {
            let binding = normal.get(key);
            assert!(binding.is_some(), "{key} ({desc}) should be bound in normal mode");
            let inner = binding.unwrap();
            assert!(inner.command.is_some(), "{key} should have a command");
            assert_eq!(inner.group, Some("window"), "{key} should be in window group");
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
    fn test_keymap_buffer_commands_use_correct_ids() {
        let keymap = KeyMap::with_defaults();
        let normal = get_normal_keymap(&keymap);

        // Check H key command is BUFFER_PREV
        let h_binding = normal.get("H").unwrap();
        if let Some(CommandRef::Registered(id)) = &h_binding.command {
            assert_eq!(id.as_str(), "buffer_prev");
        } else {
            panic!("H should have a registered command");
        }

        // Check L key command is BUFFER_NEXT
        let l_binding = normal.get("L").unwrap();
        if let Some(CommandRef::Registered(id)) = &l_binding.command {
            assert_eq!(id.as_str(), "buffer_next");
        } else {
            panic!("L should have a registered command");
        }

        // Check <leader>bd command is BUFFER_DELETE
        let bd_binding = normal.get(" bd").unwrap();
        if let Some(CommandRef::Registered(id)) = &bd_binding.command {
            assert_eq!(id.as_str(), "buffer_delete");
        } else {
            panic!("<leader>bd should have a registered command");
        }
    }
}
