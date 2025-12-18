//! Key binding system for mapping key sequences to commands

use {
    crate::{
        command::{
            CommandId, CommandTrait, builtin::ToggleExplorerCommand, id::builtin,
            registry::CommandRegistry,
        },
        event::WhichKeyBinding,
        modd::{EditMode, Focus, ModeState, SubMode},
    },
    std::{collections::HashMap, sync::Arc},
};

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

/// Keymap for each mode
///
/// Keymaps are organized by Focus + `EditMode` + `SubMode`:
/// - Editor + Normal → normal
/// - Editor + Insert → insert
/// - Editor + Visual → visual
/// - Explorer + Normal → explorer
/// - Explorer + Insert → `explorer_input`
/// - Telescope + Normal → `telescope_normal`
/// - Telescope + Insert → `telescope_insert`
/// - Any + Command → command
/// - Any + `OperatorPending` → `operator_pending`
#[derive(Default)]
pub struct KeyMap {
    pub insert: HashMap<String, KeyMapInner>,
    pub normal: HashMap<String, KeyMapInner>,
    pub command: HashMap<String, KeyMapInner>,
    pub visual: HashMap<String, KeyMapInner>,
    pub explorer: HashMap<String, KeyMapInner>,
    pub explorer_input: HashMap<String, KeyMapInner>,
    pub operator_pending: HashMap<String, KeyMapInner>,
    pub telescope_normal: HashMap<String, KeyMapInner>,
    pub telescope_insert: HashMap<String, KeyMapInner>,
    pub leap: HashMap<String, KeyMapInner>,
    pub settings_menu: HashMap<String, KeyMapInner>,
}

impl KeyMap {
    #[must_use]
    pub fn with_defaults() -> Self {
        let mut km = Self::default();
        Self::setup_normal_mode(&mut km.normal);
        Self::setup_insert_mode(&mut km.insert);
        Self::setup_visual_mode(&mut km.visual);
        Self::setup_command_mode(&mut km.command);
        Self::setup_explorer_mode(&mut km.explorer);
        Self::setup_explorer_input_mode(&mut km.explorer_input);
        Self::setup_operator_pending_mode(&mut km.operator_pending);
        Self::setup_telescope_normal_mode(&mut km.telescope_normal);
        Self::setup_telescope_insert_mode(&mut km.telescope_insert);
        Self::setup_leap_mode(&mut km.leap);
        Self::setup_settings_menu_mode(&mut km.settings_menu);
        km
    }

    /// Bind a key sequence to a command reference at runtime
    ///
    /// # Arguments
    /// * `mode` - Mode identifier: n/normal, i/insert, v/visual, c/command,
    ///   tn/telescope\_normal, ti/telescope\_insert
    /// * `keys` - Key sequence (e.g., "jj", "<leader>x")
    /// * `cmd` - Command reference to bind
    pub fn bind(&mut self, mode: &str, keys: &str, cmd: CommandRef) {
        let map = match mode {
            "n" | "normal" => &mut self.normal,
            "i" | "insert" => &mut self.insert,
            "v" | "visual" => &mut self.visual,
            "c" | "command" => &mut self.command,
            "e" | "explorer" => &mut self.explorer,
            "E" | "explorer_input" => &mut self.explorer_input,
            "o" | "operator_pending" => &mut self.operator_pending,
            "tn" | "telescope_normal" => &mut self.telescope_normal,
            "ti" | "telescope_insert" => &mut self.telescope_insert,
            "l" | "leap" => &mut self.leap,
            _ => return,
        };
        map.insert(keys.to_string(), KeyMapInner::with_command_ref(cmd));
    }

    /// Bind a key sequence to a registered command ID at runtime
    pub fn bind_id(&mut self, mode: &str, keys: &str, id: CommandId) {
        self.bind(mode, keys, CommandRef::Registered(id));
    }

    /// Unbind a key sequence
    pub fn unbind(&mut self, mode: &str, keys: &str) {
        let map = match mode {
            "n" | "normal" => &mut self.normal,
            "i" | "insert" => &mut self.insert,
            "v" | "visual" => &mut self.visual,
            "c" | "command" => &mut self.command,
            "e" | "explorer" => &mut self.explorer,
            "E" | "explorer_input" => &mut self.explorer_input,
            "o" | "operator_pending" => &mut self.operator_pending,
            "tn" | "telescope_normal" => &mut self.telescope_normal,
            "ti" | "telescope_insert" => &mut self.telescope_insert,
            "l" | "leap" => &mut self.leap,
            _ => return,
        };
        map.remove(keys);
    }

    /// Get the appropriate keymap for a given `ModeState`
    ///
    /// Keymap selection priority:
    /// 1. `SubMode` takes precedence (Command, `OperatorPending`)
    /// 2. Focus + `EditMode` determines the keymap otherwise
    #[must_use]
    pub const fn get_keymap_for_mode(&self, mode: &ModeState) -> &HashMap<String, KeyMapInner> {
        // SubMode takes precedence
        match &mode.sub_mode {
            SubMode::Command => return &self.command,
            SubMode::OperatorPending { .. } => return &self.operator_pending,
            SubMode::Leap { .. } => return &self.leap,
            SubMode::None => {}
        }

        // Focus + EditMode determines keymap
        match (&mode.focus, &mode.edit_mode) {
            (Focus::Editor, EditMode::Normal) => &self.normal,
            (Focus::Editor, EditMode::Insert(_)) => &self.insert,
            (Focus::Editor, EditMode::Visual(_)) => &self.visual,
            (Focus::Explorer, EditMode::Normal | EditMode::Visual(_)) => &self.explorer,
            (Focus::Explorer, EditMode::Insert(_)) => &self.explorer_input,
            (Focus::Telescope, EditMode::Normal | EditMode::Visual(_)) => &self.telescope_normal,
            (Focus::Telescope, EditMode::Insert(_)) => &self.telescope_insert,
            (Focus::SettingsMenu, _) => &self.settings_menu,
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

    #[allow(clippy::too_many_lines)]
    fn setup_normal_mode(keymap: &mut HashMap<String, KeyMapInner>) {
        // Movement
        keymap.insert(
            "h".to_string(),
            KeyMapInner::with_command_id(builtin::CURSOR_LEFT).group("motion"),
        );
        keymap.insert(
            "j".to_string(),
            KeyMapInner::with_command_id(builtin::CURSOR_DOWN).group("motion"),
        );
        keymap.insert(
            "k".to_string(),
            KeyMapInner::with_command_id(builtin::CURSOR_UP).group("motion"),
        );
        keymap.insert(
            "l".to_string(),
            KeyMapInner::with_command_id(builtin::CURSOR_RIGHT).group("motion"),
        );
        keymap.insert(
            "0".to_string(),
            KeyMapInner::with_command_id(builtin::CURSOR_LINE_START).group("motion"),
        );
        keymap.insert(
            "$".to_string(),
            KeyMapInner::with_command_id(builtin::CURSOR_LINE_END).group("motion"),
        );
        keymap.insert(
            "w".to_string(),
            KeyMapInner::with_command_id(builtin::CURSOR_WORD_FORWARD).group("motion"),
        );
        keymap.insert(
            "b".to_string(),
            KeyMapInner::with_command_id(builtin::CURSOR_WORD_BACKWARD).group("motion"),
        );
        keymap.insert(
            "e".to_string(),
            KeyMapInner::with_command_id(builtin::CURSOR_WORD_END).group("motion"),
        );

        // Mode switching
        keymap.insert(
            "i".to_string(),
            KeyMapInner::with_command_id(builtin::ENTER_INSERT_MODE).group("mode"),
        );
        keymap.insert(
            "a".to_string(),
            KeyMapInner::with_command_id(builtin::ENTER_INSERT_MODE_AFTER).group("mode"),
        );
        keymap.insert(
            "A".to_string(),
            KeyMapInner::with_command_id(builtin::ENTER_INSERT_MODE_EOL).group("mode"),
        );
        keymap.insert(
            "o".to_string(),
            KeyMapInner::with_command_id(builtin::OPEN_LINE_BELOW).group("edit"),
        );
        keymap.insert(
            "O".to_string(),
            KeyMapInner::with_command_id(builtin::OPEN_LINE_ABOVE).group("edit"),
        );
        keymap.insert(
            "v".to_string(),
            KeyMapInner::with_command_id(builtin::ENTER_VISUAL_MODE).group("mode"),
        );
        keymap.insert(
            "V".to_string(),
            KeyMapInner::with_command_id(builtin::ENTER_VISUAL_LINE_MODE).group("mode"),
        );
        keymap.insert(
            "<C-v>".to_string(),
            KeyMapInner::with_command_id(builtin::ENTER_VISUAL_BLOCK_MODE).group("mode"),
        );
        keymap.insert(
            ":".to_string(),
            KeyMapInner::with_command_id(builtin::ENTER_COMMAND_MODE).group("mode"),
        );

        // Editing
        keymap.insert(
            "x".to_string(),
            KeyMapInner::with_command_id(builtin::DELETE_CHAR_FORWARD).group("edit"),
        );
        keymap.insert("p".to_string(), KeyMapInner::with_command_id(builtin::PASTE).group("edit"));
        keymap.insert(
            "P".to_string(),
            KeyMapInner::with_command_id(builtin::PASTE_BEFORE).group("edit"),
        );

        // g-prefix bindings
        keymap.insert("g".to_string(), KeyMapInner::new()); // prefix, no command
        keymap.insert(
            "gg".to_string(),
            KeyMapInner::with_command_id(builtin::GOTO_FIRST_LINE).group("jump"),
        );
        keymap.insert(
            "G".to_string(),
            KeyMapInner::with_command_id(builtin::GOTO_LAST_LINE).group("jump"),
        );

        // Jump list navigation
        keymap.insert(
            "<C-o>".to_string(),
            KeyMapInner::with_command_id(builtin::JUMP_OLDER).group("jump"),
        );
        keymap.insert(
            "<C-i>".to_string(),
            KeyMapInner::with_command_id(builtin::JUMP_NEWER).group("jump"),
        );

        // Undo/Redo
        keymap.insert("u".to_string(), KeyMapInner::with_command_id(builtin::UNDO).group("edit"));
        keymap
            .insert("<C-r>".to_string(), KeyMapInner::with_command_id(builtin::REDO).group("edit"));

        // Operators (d, y, c enter operator-pending mode)
        keymap.insert(
            "d".to_string(),
            KeyMapInner::with_command_id(builtin::ENTER_DELETE_OPERATOR).group("operator"),
        );
        keymap.insert(
            "dd".to_string(),
            KeyMapInner::with_command_id(builtin::DELETE_LINE).group("operator"),
        );
        keymap.insert(
            "y".to_string(),
            KeyMapInner::with_command_id(builtin::ENTER_YANK_OPERATOR).group("operator"),
        );
        keymap.insert(
            "yy".to_string(),
            KeyMapInner::with_command_id(builtin::YANK_LINE).group("operator"),
        );
        keymap.insert(
            "Y".to_string(),
            KeyMapInner::with_command_id(builtin::YANK_TO_END).group("operator"),
        );
        keymap.insert(
            "c".to_string(),
            KeyMapInner::with_command_id(builtin::ENTER_CHANGE_OPERATOR).group("operator"),
        );

        // Space (leader) bindings
        keymap.insert(" ".to_string(), KeyMapInner::new()); // prefix, no command
        keymap.insert(
            " e".to_string(),
            KeyMapInner::with_inline_command(Arc::new(ToggleExplorerCommand)).group("misc"),
        );
        keymap.insert(
            " s".to_string(),
            KeyMapInner::with_command_id(builtin::SETTINGS_MENU_OPEN).group("misc"),
        );

        // Telescope bindings (Space + f prefix)
        keymap.insert(" f".to_string(), KeyMapInner::new()); // prefix, no command
        keymap.insert(
            " ff".to_string(),
            KeyMapInner::with_command_id(builtin::TELESCOPE_FIND_FILES).group("telescope"),
        );
        keymap.insert(
            " fb".to_string(),
            KeyMapInner::with_command_id(builtin::TELESCOPE_FIND_BUFFERS).group("telescope"),
        );
        keymap.insert(
            " fg".to_string(),
            KeyMapInner::with_command_id(builtin::TELESCOPE_LIVE_GREP).group("telescope"),
        );
        keymap.insert(
            " fr".to_string(),
            KeyMapInner::with_command_id(builtin::TELESCOPE_RECENT).group("telescope"),
        );
        keymap.insert(
            " fc".to_string(),
            KeyMapInner::with_command_id(builtin::TELESCOPE_COMMANDS).group("telescope"),
        );
        keymap.insert(
            " fh".to_string(),
            KeyMapInner::with_command_id(builtin::TELESCOPE_HELP).group("telescope"),
        );
        keymap.insert(
            " fk".to_string(),
            KeyMapInner::with_command_id(builtin::TELESCOPE_KEYMAPS).group("telescope"),
        );

        // Leap motion bindings
        keymap.insert(
            "s".to_string(),
            KeyMapInner::with_command_id(builtin::LEAP_FORWARD).group("jump"),
        );
        keymap.insert(
            "S".to_string(),
            KeyMapInner::with_command_id(builtin::LEAP_BACKWARD).group("jump"),
        );

        // z-prefix bindings (code folding)
        keymap.insert("z".to_string(), KeyMapInner::new()); // prefix, no command
        keymap.insert(
            "za".to_string(),
            KeyMapInner::with_command_id(builtin::FOLD_TOGGLE).group("fold"),
        );
        keymap.insert(
            "zo".to_string(),
            KeyMapInner::with_command_id(builtin::FOLD_OPEN).group("fold"),
        );
        keymap.insert(
            "zc".to_string(),
            KeyMapInner::with_command_id(builtin::FOLD_CLOSE).group("fold"),
        );
        keymap.insert(
            "zR".to_string(),
            KeyMapInner::with_command_id(builtin::FOLD_OPEN_ALL).group("fold"),
        );
        keymap.insert(
            "zM".to_string(),
            KeyMapInner::with_command_id(builtin::FOLD_CLOSE_ALL).group("fold"),
        );

        // Window navigation (C-hjkl)
        keymap.insert(
            "<C-h>".to_string(),
            KeyMapInner::with_command_id(builtin::WINDOW_FOCUS_LEFT).group("window"),
        );
        keymap.insert(
            "<C-j>".to_string(),
            KeyMapInner::with_command_id(builtin::WINDOW_FOCUS_DOWN).group("window"),
        );
        keymap.insert(
            "<C-k>".to_string(),
            KeyMapInner::with_command_id(builtin::WINDOW_FOCUS_UP).group("window"),
        );
        keymap.insert(
            "<C-l>".to_string(),
            KeyMapInner::with_command_id(builtin::WINDOW_FOCUS_RIGHT).group("window"),
        );

        // Window movement (C-S-HJKL)
        keymap.insert(
            "<C-S-H>".to_string(),
            KeyMapInner::with_command_id(builtin::WINDOW_MOVE_LEFT).group("window"),
        );
        keymap.insert(
            "<C-S-J>".to_string(),
            KeyMapInner::with_command_id(builtin::WINDOW_MOVE_DOWN).group("window"),
        );
        keymap.insert(
            "<C-S-K>".to_string(),
            KeyMapInner::with_command_id(builtin::WINDOW_MOVE_UP).group("window"),
        );
        keymap.insert(
            "<C-S-L>".to_string(),
            KeyMapInner::with_command_id(builtin::WINDOW_MOVE_RIGHT).group("window"),
        );

        // Tab navigation
        keymap.insert(
            "gt".to_string(),
            KeyMapInner::with_command_id(builtin::TAB_NEXT).group("window"),
        );
        keymap.insert(
            "gT".to_string(),
            KeyMapInner::with_command_id(builtin::TAB_PREV).group("window"),
        );
    }

    fn setup_insert_mode(keymap: &mut HashMap<String, KeyMapInner>) {
        keymap
            .insert("Escape".to_string(), KeyMapInner::with_command_id(builtin::ENTER_NORMAL_MODE));
        keymap.insert(
            "Backspace".to_string(),
            KeyMapInner::with_command_id(builtin::DELETE_CHAR_BACKWARD),
        );
        keymap.insert("Enter".to_string(), KeyMapInner::with_command_id(builtin::INSERT_NEWLINE));

        // Completion keybindings
        keymap.insert(
            "C-Space".to_string(),
            KeyMapInner::with_command_id(builtin::COMPLETION_TRIGGER),
        );
        keymap.insert("C-n".to_string(), KeyMapInner::with_command_id(builtin::COMPLETION_NEXT));
        keymap.insert("C-p".to_string(), KeyMapInner::with_command_id(builtin::COMPLETION_PREV));
        keymap.insert("Tab".to_string(), KeyMapInner::with_command_id(builtin::COMPLETION_CONFIRM));
        keymap.insert("C-e".to_string(), KeyMapInner::with_command_id(builtin::COMPLETION_DISMISS));
    }

    fn setup_visual_mode(keymap: &mut HashMap<String, KeyMapInner>) {
        keymap
            .insert("Escape".to_string(), KeyMapInner::with_command_id(builtin::ENTER_NORMAL_MODE));
        keymap.insert("h".to_string(), KeyMapInner::with_command_id(builtin::VISUAL_EXTEND_LEFT));
        keymap.insert("j".to_string(), KeyMapInner::with_command_id(builtin::VISUAL_EXTEND_DOWN));
        keymap.insert("k".to_string(), KeyMapInner::with_command_id(builtin::VISUAL_EXTEND_UP));
        keymap.insert("l".to_string(), KeyMapInner::with_command_id(builtin::VISUAL_EXTEND_RIGHT));
        keymap.insert("d".to_string(), KeyMapInner::with_command_id(builtin::VISUAL_DELETE));
        keymap.insert("y".to_string(), KeyMapInner::with_command_id(builtin::VISUAL_YANK));
        keymap.insert(":".to_string(), KeyMapInner::with_command_id(builtin::ENTER_COMMAND_MODE));
    }

    fn setup_command_mode(keymap: &mut HashMap<String, KeyMapInner>) {
        keymap.insert(
            "Escape".to_string(),
            KeyMapInner::with_command_id(builtin::COMMAND_LINE_CANCEL),
        );
        keymap.insert(
            "Enter".to_string(),
            KeyMapInner::with_command_id(builtin::COMMAND_LINE_EXECUTE),
        );
        keymap.insert(
            "Backspace".to_string(),
            KeyMapInner::with_command_id(builtin::COMMAND_LINE_BACKSPACE),
        );
    }

    fn setup_explorer_mode(keymap: &mut HashMap<String, KeyMapInner>) {
        // Navigation
        keymap.insert("j".to_string(), KeyMapInner::with_command_id(builtin::EXPLORER_CURSOR_DOWN));
        keymap.insert("k".to_string(), KeyMapInner::with_command_id(builtin::EXPLORER_CURSOR_UP));
        keymap.insert(
            "Ctrl+d".to_string(),
            KeyMapInner::with_command_id(builtin::EXPLORER_PAGE_DOWN),
        );
        keymap
            .insert("Ctrl+u".to_string(), KeyMapInner::with_command_id(builtin::EXPLORER_PAGE_UP));
        keymap.insert("g".to_string(), KeyMapInner::new()); // prefix
        keymap.insert("gg".to_string(), KeyMapInner::with_command_id(builtin::EXPLORER_GOTO_FIRST));
        keymap.insert("G".to_string(), KeyMapInner::with_command_id(builtin::EXPLORER_GOTO_LAST));

        // Tree operations
        keymap
            .insert("Enter".to_string(), KeyMapInner::with_command_id(builtin::EXPLORER_OPEN_NODE));
        keymap.insert("o".to_string(), KeyMapInner::with_command_id(builtin::EXPLORER_TOGGLE_NODE));
        keymap
            .insert("x".to_string(), KeyMapInner::with_command_id(builtin::EXPLORER_CLOSE_PARENT));
        keymap
            .insert("u".to_string(), KeyMapInner::with_command_id(builtin::EXPLORER_GO_TO_PARENT));
        keymap.insert("R".to_string(), KeyMapInner::with_command_id(builtin::EXPLORER_REFRESH));

        // Display
        keymap
            .insert("I".to_string(), KeyMapInner::with_command_id(builtin::EXPLORER_TOGGLE_HIDDEN));
        keymap
            .insert("S".to_string(), KeyMapInner::with_command_id(builtin::EXPLORER_TOGGLE_SIZES));

        // File operations
        keymap.insert("a".to_string(), KeyMapInner::with_command_id(builtin::EXPLORER_CREATE_FILE));
        keymap.insert("A".to_string(), KeyMapInner::with_command_id(builtin::EXPLORER_CREATE_DIR));
        keymap.insert("r".to_string(), KeyMapInner::with_command_id(builtin::EXPLORER_RENAME));
        keymap.insert("D".to_string(), KeyMapInner::with_command_id(builtin::EXPLORER_DELETE));

        // Clipboard operations
        keymap.insert("y".to_string(), KeyMapInner::new()); // prefix
        keymap.insert("yy".to_string(), KeyMapInner::with_command_id(builtin::EXPLORER_YANK));
        keymap.insert("d".to_string(), KeyMapInner::new()); // prefix
        keymap.insert("dd".to_string(), KeyMapInner::with_command_id(builtin::EXPLORER_CUT));
        keymap.insert("p".to_string(), KeyMapInner::with_command_id(builtin::EXPLORER_PASTE));

        // Selection
        keymap.insert("v".to_string(), KeyMapInner::with_command_id(builtin::EXPLORER_VISUAL_MODE));
        keymap.insert("V".to_string(), KeyMapInner::with_command_id(builtin::EXPLORER_SELECT_ALL));

        // Filter
        keymap.insert("/".to_string(), KeyMapInner::with_command_id(builtin::EXPLORER_FILTER));
        keymap
            .insert("C".to_string(), KeyMapInner::with_command_id(builtin::EXPLORER_CLEAR_FILTER));

        // Window control
        keymap.insert(
            "Tab".to_string(),
            KeyMapInner::with_command_id(builtin::EXPLORER_FOCUS_EDITOR),
        );
        keymap.insert(
            "Escape".to_string(),
            KeyMapInner::with_command_id(builtin::EXPLORER_FOCUS_EDITOR),
        );

        // Window navigation (C-hjkl)
        keymap
            .insert("<C-h>".to_string(), KeyMapInner::with_command_id(builtin::WINDOW_FOCUS_LEFT));
        keymap
            .insert("<C-j>".to_string(), KeyMapInner::with_command_id(builtin::WINDOW_FOCUS_DOWN));
        keymap.insert("<C-k>".to_string(), KeyMapInner::with_command_id(builtin::WINDOW_FOCUS_UP));
        keymap
            .insert("<C-l>".to_string(), KeyMapInner::with_command_id(builtin::WINDOW_FOCUS_RIGHT));

        // Space (leader) bindings
        keymap.insert(" ".to_string(), KeyMapInner::new()); // prefix
        keymap.insert(
            " e".to_string(),
            KeyMapInner::with_inline_command(Arc::new(ToggleExplorerCommand)),
        );
    }

    fn setup_explorer_input_mode(keymap: &mut HashMap<String, KeyMapInner>) {
        // In explorer input mode, we only handle special keys
        // Characters are handled via inline commands in the command handler
        keymap.insert(
            "Escape".to_string(),
            KeyMapInner::with_command_id(builtin::EXPLORER_CANCEL_INPUT),
        );
        keymap.insert(
            "Enter".to_string(),
            KeyMapInner::with_command_id(builtin::EXPLORER_CONFIRM_INPUT),
        );
        keymap.insert(
            "Backspace".to_string(),
            KeyMapInner::with_command_id(builtin::EXPLORER_INPUT_BACKSPACE),
        );
    }

    fn setup_operator_pending_mode(keymap: &mut HashMap<String, KeyMapInner>) {
        // Escape cancels operator-pending mode
        keymap.insert(
            "<Escape>".to_string(),
            KeyMapInner::with_command_id(builtin::ENTER_NORMAL_MODE),
        );

        // Motion hints for which-key (actual handling is dynamic in CommandHandler)
        // These don't execute commands but show in which-key panel
        keymap
            .insert("d".to_string(), KeyMapInner::with_hint("delete line (dd)").group("operator"));
        keymap.insert("y".to_string(), KeyMapInner::with_hint("yank line (yy)").group("operator"));
        keymap
            .insert("c".to_string(), KeyMapInner::with_hint("change line (cc)").group("operator"));
        keymap.insert("w".to_string(), KeyMapInner::with_hint("word forward").group("motion"));
        keymap.insert("b".to_string(), KeyMapInner::with_hint("word backward").group("motion"));
        keymap.insert("e".to_string(), KeyMapInner::with_hint("word end").group("motion"));
        keymap.insert("$".to_string(), KeyMapInner::with_hint("end of line").group("motion"));
        keymap.insert("0".to_string(), KeyMapInner::with_hint("start of line").group("motion"));
        keymap.insert("^".to_string(), KeyMapInner::with_hint("first non-blank").group("motion"));
        keymap.insert("j".to_string(), KeyMapInner::with_hint("line down").group("motion"));
        keymap.insert("k".to_string(), KeyMapInner::with_hint("line up").group("motion"));
        keymap.insert("G".to_string(), KeyMapInner::with_hint("end of file").group("motion"));
        keymap.insert("g".to_string(), KeyMapInner::new()); // prefix for gg
        keymap.insert("gg".to_string(), KeyMapInner::with_hint("start of file").group("motion"));
        keymap
            .insert("i".to_string(), KeyMapInner::with_hint("inner text object").group("textobj"));
        keymap
            .insert("a".to_string(), KeyMapInner::with_hint("around text object").group("textobj"));
    }

    /// Telescope Normal mode - navigation keys (j/k/gg/G)
    fn setup_telescope_normal_mode(keymap: &mut HashMap<String, KeyMapInner>) {
        // Navigation
        keymap.insert("j".to_string(), KeyMapInner::with_command_id(builtin::TELESCOPE_NEXT));
        keymap.insert("k".to_string(), KeyMapInner::with_command_id(builtin::TELESCOPE_PREV));
        keymap.insert("g".to_string(), KeyMapInner::new()); // prefix
        keymap
            .insert("gg".to_string(), KeyMapInner::with_command_id(builtin::TELESCOPE_GOTO_FIRST));
        keymap.insert("G".to_string(), KeyMapInner::with_command_id(builtin::TELESCOPE_GOTO_LAST));
        keymap
            .insert("C-d".to_string(), KeyMapInner::with_command_id(builtin::TELESCOPE_PAGE_DOWN));
        keymap.insert("C-u".to_string(), KeyMapInner::with_command_id(builtin::TELESCOPE_PAGE_UP));

        // Mode switching
        keymap
            .insert("i".to_string(), KeyMapInner::with_command_id(builtin::TELESCOPE_ENTER_INSERT));

        // Actions
        keymap.insert("Escape".to_string(), KeyMapInner::with_command_id(builtin::TELESCOPE_CLOSE));
        keymap
            .insert("Enter".to_string(), KeyMapInner::with_command_id(builtin::TELESCOPE_CONFIRM));
    }

    /// Telescope Insert mode - typing query with Ctrl-based navigation
    fn setup_telescope_insert_mode(keymap: &mut HashMap<String, KeyMapInner>) {
        // Navigation (Ctrl-based to not interfere with typing)
        keymap.insert("C-n".to_string(), KeyMapInner::with_command_id(builtin::TELESCOPE_NEXT));
        keymap.insert("C-p".to_string(), KeyMapInner::with_command_id(builtin::TELESCOPE_PREV));
        keymap.insert("Down".to_string(), KeyMapInner::with_command_id(builtin::TELESCOPE_NEXT));
        keymap.insert("Up".to_string(), KeyMapInner::with_command_id(builtin::TELESCOPE_PREV));
        keymap.insert("Tab".to_string(), KeyMapInner::with_command_id(builtin::TELESCOPE_NEXT));
        keymap.insert("S-Tab".to_string(), KeyMapInner::with_command_id(builtin::TELESCOPE_PREV));
        keymap.insert("C-u".to_string(), KeyMapInner::with_command_id(builtin::TELESCOPE_PAGE_UP));
        keymap
            .insert("C-d".to_string(), KeyMapInner::with_command_id(builtin::TELESCOPE_PAGE_DOWN));

        // Editing
        keymap.insert(
            "Backspace".to_string(),
            KeyMapInner::with_command_id(builtin::TELESCOPE_DELETE_CHAR),
        );

        // Mode switching (ESC goes to Normal mode in Telescope, not closes)
        keymap.insert(
            "Escape".to_string(),
            KeyMapInner::with_command_id(builtin::TELESCOPE_ENTER_NORMAL),
        );

        // Actions
        keymap
            .insert("Enter".to_string(), KeyMapInner::with_command_id(builtin::TELESCOPE_CONFIRM));
    }

    fn setup_leap_mode(keymap: &mut HashMap<String, KeyMapInner>) {
        // Escape cancels leap mode
        keymap.insert("Escape".to_string(), KeyMapInner::with_command_id(builtin::LEAP_CANCEL));
        // All other keys (characters for search pattern or label selection) are handled
        // directly in CommandHandler without going through keymap
    }

    fn setup_settings_menu_mode(keymap: &mut HashMap<String, KeyMapInner>) {
        // Navigation
        keymap.insert("j".to_string(), KeyMapInner::with_command_id(builtin::SETTINGS_MENU_NEXT));
        keymap.insert("k".to_string(), KeyMapInner::with_command_id(builtin::SETTINGS_MENU_PREV));
        keymap
            .insert("Down".to_string(), KeyMapInner::with_command_id(builtin::SETTINGS_MENU_NEXT));
        keymap.insert("Up".to_string(), KeyMapInner::with_command_id(builtin::SETTINGS_MENU_PREV));

        // Toggle/Cycle
        keymap.insert(" ".to_string(), KeyMapInner::with_command_id(builtin::SETTINGS_MENU_TOGGLE));
        keymap.insert(
            "l".to_string(),
            KeyMapInner::with_command_id(builtin::SETTINGS_MENU_CYCLE_NEXT),
        );
        keymap.insert(
            "h".to_string(),
            KeyMapInner::with_command_id(builtin::SETTINGS_MENU_CYCLE_PREV),
        );
        keymap.insert(
            "Right".to_string(),
            KeyMapInner::with_command_id(builtin::SETTINGS_MENU_CYCLE_NEXT),
        );
        keymap.insert(
            "Left".to_string(),
            KeyMapInner::with_command_id(builtin::SETTINGS_MENU_CYCLE_PREV),
        );

        // Number increment/decrement
        keymap.insert(
            "+".to_string(),
            KeyMapInner::with_command_id(builtin::SETTINGS_MENU_INCREMENT),
        );
        keymap.insert(
            "-".to_string(),
            KeyMapInner::with_command_id(builtin::SETTINGS_MENU_DECREMENT),
        );

        // Action/Confirm
        keymap.insert(
            "Enter".to_string(),
            KeyMapInner::with_command_id(builtin::SETTINGS_MENU_EXECUTE),
        );

        // Close
        keymap.insert(
            "Escape".to_string(),
            KeyMapInner::with_command_id(builtin::SETTINGS_MENU_CLOSE),
        );
        keymap.insert("q".to_string(), KeyMapInner::with_command_id(builtin::SETTINGS_MENU_CLOSE));

        // Quick select (1-9)
        keymap
            .insert("1".to_string(), KeyMapInner::with_command_id(builtin::SETTINGS_MENU_QUICK_1));
        keymap
            .insert("2".to_string(), KeyMapInner::with_command_id(builtin::SETTINGS_MENU_QUICK_2));
        keymap
            .insert("3".to_string(), KeyMapInner::with_command_id(builtin::SETTINGS_MENU_QUICK_3));
        keymap
            .insert("4".to_string(), KeyMapInner::with_command_id(builtin::SETTINGS_MENU_QUICK_4));
        keymap
            .insert("5".to_string(), KeyMapInner::with_command_id(builtin::SETTINGS_MENU_QUICK_5));
        keymap
            .insert("6".to_string(), KeyMapInner::with_command_id(builtin::SETTINGS_MENU_QUICK_6));
        keymap
            .insert("7".to_string(), KeyMapInner::with_command_id(builtin::SETTINGS_MENU_QUICK_7));
        keymap
            .insert("8".to_string(), KeyMapInner::with_command_id(builtin::SETTINGS_MENU_QUICK_8));
        keymap
            .insert("9".to_string(), KeyMapInner::with_command_id(builtin::SETTINGS_MENU_QUICK_9));
    }
}
