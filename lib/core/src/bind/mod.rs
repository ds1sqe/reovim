//! Key binding system for mapping key sequences to commands

use {
    crate::{
        command::{CommandId, CommandTrait, id::builtin, registry::CommandRegistry},
        keys,
        keystroke::KeySequence,
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
    /// Optional hint for which-key display (overrides command description)
    pub hint: Option<&'static str>,
    /// Group for which-key categorization (e.g., "motion", "operator")
    pub group: Option<&'static str>,
}

impl KeyBinding {
    /// Create a binding with a registered command ID
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // KeySequence contains Vec which can't be const
    pub fn id(keys: KeySequence, id: CommandId) -> Self {
        Self {
            keys,
            command: CommandRef::Registered(id),
            hint: None,
            group: None,
        }
    }

    /// Create a binding with a command ID and group
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // KeySequence contains Vec which can't be const
    pub fn id_group(keys: KeySequence, id: CommandId, group: &'static str) -> Self {
        Self {
            keys,
            command: CommandRef::Registered(id),
            hint: None,
            group: Some(group),
        }
    }

    /// Create a hint-only binding (prefix node for which-key)
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // KeySequence contains Vec which can't be const
    pub fn hint(keys: KeySequence, hint: &'static str, group: &'static str) -> Self {
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
    pub next: HashMap<KeySequence, Self>,
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
static EMPTY_MAP: std::sync::LazyLock<HashMap<KeySequence, KeyMapInner>> =
    std::sync::LazyLock::new(HashMap::new);

/// Keymap for each mode
///
/// Keymaps are organized by `KeymapScope`:
/// - `Component { id, mode }` for component-specific keymaps
/// - `SubMode` for global sub-mode keymaps (Command, `OperatorPending`, Leap)
#[derive(Default)]
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
        normal
            .insert(keys!['h'], KeyMapInner::with_command_id(builtin::CURSOR_LEFT).group("motion"));
        normal
            .insert(keys!['j'], KeyMapInner::with_command_id(builtin::CURSOR_DOWN).group("motion"));
        normal.insert(keys!['k'], KeyMapInner::with_command_id(builtin::CURSOR_UP).group("motion"));
        normal.insert(
            keys!['l'],
            KeyMapInner::with_command_id(builtin::CURSOR_RIGHT).group("motion"),
        );
        normal.insert(
            keys!['0'],
            KeyMapInner::with_command_id(builtin::CURSOR_LINE_START).group("motion"),
        );
        normal.insert(
            keys!['$'],
            KeyMapInner::with_command_id(builtin::CURSOR_LINE_END).group("motion"),
        );
        normal.insert(
            keys!['w'],
            KeyMapInner::with_command_id(builtin::CURSOR_WORD_FORWARD).group("motion"),
        );
        normal.insert(
            keys!['b'],
            KeyMapInner::with_command_id(builtin::CURSOR_WORD_BACKWARD).group("motion"),
        );
        normal.insert(
            keys!['e'],
            KeyMapInner::with_command_id(builtin::CURSOR_WORD_END).group("motion"),
        );

        // Mode switching
        normal.insert(
            keys!['i'],
            KeyMapInner::with_command_id(builtin::ENTER_INSERT_MODE).group("mode"),
        );
        normal.insert(
            keys!['a'],
            KeyMapInner::with_command_id(builtin::ENTER_INSERT_MODE_AFTER).group("mode"),
        );
        normal.insert(
            keys!['A'],
            KeyMapInner::with_command_id(builtin::ENTER_INSERT_MODE_EOL).group("mode"),
        );
        normal.insert(
            keys!['o'],
            KeyMapInner::with_command_id(builtin::OPEN_LINE_BELOW).group("edit"),
        );
        normal.insert(
            keys!['O'],
            KeyMapInner::with_command_id(builtin::OPEN_LINE_ABOVE).group("edit"),
        );
        normal.insert(
            keys!['v'],
            KeyMapInner::with_command_id(builtin::ENTER_VISUAL_MODE).group("mode"),
        );
        normal.insert(
            keys!['V'],
            KeyMapInner::with_command_id(builtin::ENTER_VISUAL_LINE_MODE).group("mode"),
        );
        normal.insert(
            keys![(Ctrl 'v')],
            KeyMapInner::with_command_id(builtin::ENTER_VISUAL_BLOCK_MODE).group("mode"),
        );
        normal.insert(
            keys![':'],
            KeyMapInner::with_command_id(builtin::ENTER_COMMAND_MODE).group("mode"),
        );

        // Editing
        normal.insert(
            keys!['x'],
            KeyMapInner::with_command_id(builtin::DELETE_CHAR_FORWARD).group("edit"),
        );
        normal.insert(keys!['p'], KeyMapInner::with_command_id(builtin::PASTE).group("edit"));
        normal
            .insert(keys!['P'], KeyMapInner::with_command_id(builtin::PASTE_BEFORE).group("edit"));

        // g-prefix bindings
        normal.insert(keys!['g'], KeyMapInner::new());
        normal.insert(
            keys!['g' 'g'],
            KeyMapInner::with_command_id(builtin::GOTO_FIRST_LINE).group("jump"),
        );
        normal.insert(
            keys!['G'],
            KeyMapInner::with_command_id(builtin::GOTO_LAST_LINE).group("jump"),
        );
        normal.insert(
            keys!['g' 't'],
            KeyMapInner::with_command_id(builtin::TAB_NEXT).group("window"),
        );
        normal.insert(
            keys!['g' 'T'],
            KeyMapInner::with_command_id(builtin::TAB_PREV).group("window"),
        );

        // Jump list
        normal.insert(
            keys![(Ctrl 'o')],
            KeyMapInner::with_command_id(builtin::JUMP_OLDER).group("jump"),
        );
        normal.insert(
            keys![(Ctrl 'i')],
            KeyMapInner::with_command_id(builtin::JUMP_NEWER).group("jump"),
        );

        // Undo/Redo
        normal.insert(keys!['u'], KeyMapInner::with_command_id(builtin::UNDO).group("edit"));
        normal.insert(keys![(Ctrl 'r')], KeyMapInner::with_command_id(builtin::REDO).group("edit"));

        // Operators
        normal.insert(
            keys!['d'],
            KeyMapInner::with_command_id(builtin::ENTER_DELETE_OPERATOR).group("operator"),
        );
        normal.insert(
            keys!['d' 'd'],
            KeyMapInner::with_command_id(builtin::DELETE_LINE).group("operator"),
        );
        normal.insert(
            keys!['y'],
            KeyMapInner::with_command_id(builtin::ENTER_YANK_OPERATOR).group("operator"),
        );
        normal.insert(
            keys!['y' 'y'],
            KeyMapInner::with_command_id(builtin::YANK_LINE).group("operator"),
        );
        normal.insert(
            keys!['Y'],
            KeyMapInner::with_command_id(builtin::YANK_TO_END).group("operator"),
        );
        normal.insert(
            keys!['c'],
            KeyMapInner::with_command_id(builtin::ENTER_CHANGE_OPERATOR).group("operator"),
        );
        normal.insert(
            keys!['c' 'c'],
            KeyMapInner::with_command_id(builtin::CHANGE_LINE).group("operator"),
        );

        // Leader bindings
        normal.insert(keys![Space], KeyMapInner::new());
        // Settings menu keybindings are registered by the settings-menu plugin
        // Telescope keybindings are registered by the telescope plugin

        // Folding
        normal.insert(keys!['z'], KeyMapInner::new());
        normal.insert(
            keys!['z' 'a'],
            KeyMapInner::with_command_id(builtin::FOLD_TOGGLE).group("fold"),
        );
        normal
            .insert(keys!['z' 'o'], KeyMapInner::with_command_id(builtin::FOLD_OPEN).group("fold"));
        normal.insert(
            keys!['z' 'c'],
            KeyMapInner::with_command_id(builtin::FOLD_CLOSE).group("fold"),
        );
        normal.insert(
            keys!['z' 'R'],
            KeyMapInner::with_command_id(builtin::FOLD_OPEN_ALL).group("fold"),
        );
        normal.insert(
            keys!['z' 'M'],
            KeyMapInner::with_command_id(builtin::FOLD_CLOSE_ALL).group("fold"),
        );

        // Buffer navigation
        normal
            .insert(keys!['H'], KeyMapInner::with_command_id(builtin::BUFFER_PREV).group("buffer"));
        normal
            .insert(keys!['L'], KeyMapInner::with_command_id(builtin::BUFFER_NEXT).group("buffer"));
        normal.insert(keys![Space 'b'], KeyMapInner::with_hint("+buffer").group("buffer"));
        normal.insert(
            keys![Space 'b' 'd'],
            KeyMapInner::with_command_id(builtin::BUFFER_DELETE).group("buffer"),
        );

        // Window management
        normal.insert(keys![Space 'w'], KeyMapInner::with_hint("+window").group("window"));
        normal.insert(
            keys![Space 'w' 'h'],
            KeyMapInner::with_command_id(builtin::WINDOW_FOCUS_OR_SPLIT_LEFT).group("window"),
        );
        normal.insert(
            keys![Space 'w' 'j'],
            KeyMapInner::with_command_id(builtin::WINDOW_FOCUS_OR_SPLIT_DOWN).group("window"),
        );
        normal.insert(
            keys![Space 'w' 'k'],
            KeyMapInner::with_command_id(builtin::WINDOW_FOCUS_OR_SPLIT_UP).group("window"),
        );
        normal.insert(
            keys![Space 'w' 'l'],
            KeyMapInner::with_command_id(builtin::WINDOW_FOCUS_OR_SPLIT_RIGHT).group("window"),
        );
        normal.insert(
            keys![Space 'w' 'v'],
            KeyMapInner::with_command_id(builtin::WINDOW_SPLIT_VERTICAL).group("window"),
        );
        normal.insert(
            keys![Space 'w' 's'],
            KeyMapInner::with_command_id(builtin::WINDOW_SPLIT_HORIZONTAL).group("window"),
        );
        normal.insert(
            keys![Space 'w' 'c'],
            KeyMapInner::with_command_id(builtin::WINDOW_CLOSE).group("window"),
        );
        normal.insert(
            keys![Space 'w' 'o'],
            KeyMapInner::with_command_id(builtin::WINDOW_ONLY).group("window"),
        );
        normal.insert(
            keys![Space 'w' '='],
            KeyMapInner::with_command_id(builtin::WINDOW_EQUALIZE).group("window"),
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

    fn setup_submode_keybindings(&mut self) {
        // Command mode
        let cmd = self.get_scope_mut(KeymapScope::SubMode(SubModeKind::Command));
        cmd.insert(keys![Escape], KeyMapInner::with_command_id(builtin::COMMAND_LINE_CANCEL));
        cmd.insert(keys![Enter], KeyMapInner::with_command_id(builtin::COMMAND_LINE_EXECUTE));
        cmd.insert(keys![Backspace], KeyMapInner::with_command_id(builtin::COMMAND_LINE_BACKSPACE));

        // Operator-pending mode
        let op = self.get_scope_mut(KeymapScope::SubMode(SubModeKind::OperatorPending));
        op.insert(keys![Escape], KeyMapInner::with_command_id(builtin::ENTER_NORMAL_MODE));
        op.insert(keys!['d'], KeyMapInner::with_hint("delete line (dd)").group("operator"));
        op.insert(keys!['y'], KeyMapInner::with_hint("yank line (yy)").group("operator"));
        op.insert(keys!['c'], KeyMapInner::with_hint("change line (cc)").group("operator"));
        op.insert(keys!['w'], KeyMapInner::with_hint("word forward").group("motion"));
        op.insert(keys!['b'], KeyMapInner::with_hint("word backward").group("motion"));
        op.insert(keys!['e'], KeyMapInner::with_hint("word end").group("motion"));
        op.insert(keys!['$'], KeyMapInner::with_hint("end of line").group("motion"));
        op.insert(keys!['0'], KeyMapInner::with_hint("start of line").group("motion"));
        op.insert(keys!['^'], KeyMapInner::with_hint("first non-blank").group("motion"));
        op.insert(keys!['j'], KeyMapInner::with_hint("line down").group("motion"));
        op.insert(keys!['k'], KeyMapInner::with_hint("line up").group("motion"));
        op.insert(keys!['G'], KeyMapInner::with_hint("end of file").group("motion"));
        op.insert(keys!['g'], KeyMapInner::new());
        op.insert(keys!['g' 'g'], KeyMapInner::with_hint("start of file").group("motion"));
        op.insert(keys!['i'], KeyMapInner::with_hint("inner text object").group("textobj"));
        op.insert(keys!['a'], KeyMapInner::with_hint("around text object").group("textobj"));
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
        assert_eq!(h_inner.group, Some("buffer"));

        // Test L (next buffer) key exists
        let l_binding = normal.get(&keys!['L']);
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
        let b_prefix = normal.get(&keys![Space 'b']);
        assert!(b_prefix.is_some(), "<leader>b prefix should be bound in normal mode");
        let b_inner = b_prefix.unwrap();
        assert_eq!(b_inner.hint, Some("+buffer".to_string()));
        assert_eq!(b_inner.group, Some("buffer"));

        // Test <leader>bd exists
        let delete_binding = normal.get(&keys![Space 'b' 'd']);
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
        let w_prefix = normal.get(&keys![Space 'w']);
        assert!(w_prefix.is_some(), "<leader>w prefix should be bound in normal mode");
        let w_inner = w_prefix.unwrap();
        assert_eq!(w_inner.hint, Some("+window".to_string()));
        assert_eq!(w_inner.group, Some("window"));
    }

    #[test]
    fn test_keymap_has_leader_window_bindings() {
        let keymap = KeyMap::with_defaults();
        let normal = get_normal_keymap(&keymap);

        // Test window bindings exist
        let window_keys = [
            (keys![Space 'w' 'h'], "focus or split left"),
            (keys![Space 'w' 'j'], "focus or split down"),
            (keys![Space 'w' 'k'], "focus or split up"),
            (keys![Space 'w' 'l'], "focus or split right"),
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
            assert_eq!(inner.group, Some("window"), "{desc} should be in window group");
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
