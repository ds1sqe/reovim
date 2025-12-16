//! Key binding system for mapping key sequences to commands

use crate::command::builtin::ToggleExplorerCommand;
use crate::command::{id::builtin, registry::CommandRegistry, CommandId, CommandTrait};
use crate::event::WhichKeyBinding;
use crate::modd::Mod;
use std::collections::HashMap;
use std::sync::Arc;

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
    /// Children for multi-key sequences (e.g., "dd", "gg")
    #[allow(dead_code)] // Infrastructure for proper trie-based lookup
    pub next: HashMap<String, Self>,
}

impl KeyMapInner {
    #[must_use]
    pub fn new() -> Self {
        Self {
            command: None,
            next: HashMap::new(),
        }
    }

    /// Create a node with a `CommandRef`
    #[must_use]
    pub fn with_command_ref(cmd: CommandRef) -> Self {
        Self {
            command: Some(cmd),
            next: HashMap::new(),
        }
    }

    /// Create a node with a registered command ID
    #[must_use]
    pub fn with_command_id(id: CommandId) -> Self {
        Self {
            command: Some(CommandRef::Registered(id)),
            next: HashMap::new(),
        }
    }

    /// Create a node with an inline command
    #[must_use]
    pub fn with_inline_command(cmd: Arc<dyn CommandTrait>) -> Self {
        Self {
            command: Some(CommandRef::Inline(cmd)),
            next: HashMap::new(),
        }
    }

    /// Get the description for this binding
    ///
    /// Returns the command's description if available, or a prefix indicator.
    #[must_use]
    pub fn get_description(&self, registry: &CommandRegistry) -> String {
        match &self.command {
            Some(CommandRef::Registered(id)) => registry
                .get(id)
                .map_or_else(|| id.as_str().to_string(), |cmd| cmd.description().to_string()),
            Some(CommandRef::Inline(cmd)) => cmd.description().to_string(),
            None => "+prefix".to_string(),
        }
    }

    /// Check if this is a prefix node (has no command, only children)
    #[must_use]
    pub const fn is_prefix(&self) -> bool {
        self.command.is_none()
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
            .field("next_keys", &self.next.keys().collect::<Vec<_>>())
            .finish()
    }
}

/// Keymap for each mode
#[derive(Default)]
pub struct KeyMap {
    pub insert: HashMap<String, KeyMapInner>,
    pub normal: HashMap<String, KeyMapInner>,
    pub command: HashMap<String, KeyMapInner>,
    pub visual: HashMap<String, KeyMapInner>,
    pub explorer: HashMap<String, KeyMapInner>,
    pub explorer_input: HashMap<String, KeyMapInner>,
    #[allow(dead_code)] // Infrastructure for extra modes (Telescope, etc.)
    pub extra: HashMap<String, KeyMapInner>,
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
        km
    }

    /// Bind a key sequence to a command reference at runtime
    ///
    /// # Arguments
    /// * `mode` - Mode identifier: "n"/"normal", "i"/"insert", "v"/"visual", "c"/"command"
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
            _ => return,
        };
        map.remove(keys);
    }

    /// Get all available bindings for a given prefix in a mode
    ///
    /// This is used by the which-key feature to display available keybindings.
    #[must_use]
    pub fn get_bindings_for_prefix(
        &self,
        mode: &Mod,
        prefix: &str,
        registry: &CommandRegistry,
    ) -> Vec<WhichKeyBinding> {
        let keymap = match mode {
            Mod::Normal => &self.normal,
            Mod::Insert(_) => &self.insert,
            Mod::Visual(_) => &self.visual,
            Mod::Command => &self.command,
            Mod::Explorer => &self.explorer,
            Mod::ExplorerInput => &self.explorer_input,
        };

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

    fn setup_normal_mode(keymap: &mut HashMap<String, KeyMapInner>) {
        // Movement
        keymap.insert("h".to_string(), KeyMapInner::with_command_id(builtin::CURSOR_LEFT));
        keymap.insert("j".to_string(), KeyMapInner::with_command_id(builtin::CURSOR_DOWN));
        keymap.insert("k".to_string(), KeyMapInner::with_command_id(builtin::CURSOR_UP));
        keymap.insert("l".to_string(), KeyMapInner::with_command_id(builtin::CURSOR_RIGHT));
        keymap.insert("0".to_string(), KeyMapInner::with_command_id(builtin::CURSOR_LINE_START));
        keymap.insert("$".to_string(), KeyMapInner::with_command_id(builtin::CURSOR_LINE_END));
        keymap.insert("w".to_string(), KeyMapInner::with_command_id(builtin::CURSOR_WORD_FORWARD));
        keymap.insert("b".to_string(), KeyMapInner::with_command_id(builtin::CURSOR_WORD_BACKWARD));

        // Mode switching
        keymap.insert("i".to_string(), KeyMapInner::with_command_id(builtin::ENTER_INSERT_MODE));
        keymap.insert("a".to_string(), KeyMapInner::with_command_id(builtin::ENTER_INSERT_MODE_AFTER));
        keymap.insert("A".to_string(), KeyMapInner::with_command_id(builtin::ENTER_INSERT_MODE_EOL));
        keymap.insert("o".to_string(), KeyMapInner::with_command_id(builtin::OPEN_LINE_BELOW));
        keymap.insert("O".to_string(), KeyMapInner::with_command_id(builtin::OPEN_LINE_ABOVE));
        keymap.insert("v".to_string(), KeyMapInner::with_command_id(builtin::ENTER_VISUAL_MODE));
        keymap.insert("<C-v>".to_string(), KeyMapInner::with_command_id(builtin::ENTER_VISUAL_BLOCK_MODE));
        keymap.insert(":".to_string(), KeyMapInner::with_command_id(builtin::ENTER_COMMAND_MODE));

        // Editing
        keymap.insert("x".to_string(), KeyMapInner::with_command_id(builtin::DELETE_CHAR_FORWARD));
        keymap.insert("p".to_string(), KeyMapInner::with_command_id(builtin::PASTE));
        keymap.insert("P".to_string(), KeyMapInner::with_command_id(builtin::PASTE_BEFORE));

        // g-prefix bindings
        keymap.insert("g".to_string(), KeyMapInner::new()); // prefix, no command
        keymap.insert("gg".to_string(), KeyMapInner::with_command_id(builtin::GOTO_FIRST_LINE));
        keymap.insert("G".to_string(), KeyMapInner::with_command_id(builtin::GOTO_LAST_LINE));

        // Jump list navigation
        keymap.insert("<C-o>".to_string(), KeyMapInner::with_command_id(builtin::JUMP_OLDER));
        keymap.insert("<C-i>".to_string(), KeyMapInner::with_command_id(builtin::JUMP_NEWER));

        // Space (leader) bindings
        keymap.insert(" ".to_string(), KeyMapInner::new()); // prefix, no command
        keymap.insert(
            " e".to_string(),
            KeyMapInner::with_inline_command(Arc::new(ToggleExplorerCommand)),
        );
    }

    fn setup_insert_mode(keymap: &mut HashMap<String, KeyMapInner>) {
        keymap.insert("Escape".to_string(), KeyMapInner::with_command_id(builtin::ENTER_NORMAL_MODE));
        keymap.insert("Backspace".to_string(), KeyMapInner::with_command_id(builtin::DELETE_CHAR_BACKWARD));
        keymap.insert("Enter".to_string(), KeyMapInner::with_command_id(builtin::INSERT_NEWLINE));
    }

    fn setup_visual_mode(keymap: &mut HashMap<String, KeyMapInner>) {
        keymap.insert("Escape".to_string(), KeyMapInner::with_command_id(builtin::ENTER_NORMAL_MODE));
        keymap.insert("h".to_string(), KeyMapInner::with_command_id(builtin::VISUAL_EXTEND_LEFT));
        keymap.insert("j".to_string(), KeyMapInner::with_command_id(builtin::VISUAL_EXTEND_DOWN));
        keymap.insert("k".to_string(), KeyMapInner::with_command_id(builtin::VISUAL_EXTEND_UP));
        keymap.insert("l".to_string(), KeyMapInner::with_command_id(builtin::VISUAL_EXTEND_RIGHT));
        keymap.insert("d".to_string(), KeyMapInner::with_command_id(builtin::VISUAL_DELETE));
        keymap.insert("y".to_string(), KeyMapInner::with_command_id(builtin::VISUAL_YANK));
    }

    fn setup_command_mode(keymap: &mut HashMap<String, KeyMapInner>) {
        keymap.insert("Escape".to_string(), KeyMapInner::with_command_id(builtin::COMMAND_LINE_CANCEL));
        keymap.insert("Enter".to_string(), KeyMapInner::with_command_id(builtin::COMMAND_LINE_EXECUTE));
        keymap.insert("Backspace".to_string(), KeyMapInner::with_command_id(builtin::COMMAND_LINE_BACKSPACE));
    }

    fn setup_explorer_mode(keymap: &mut HashMap<String, KeyMapInner>) {
        // Navigation
        keymap.insert("j".to_string(), KeyMapInner::with_command_id(builtin::EXPLORER_CURSOR_DOWN));
        keymap.insert("k".to_string(), KeyMapInner::with_command_id(builtin::EXPLORER_CURSOR_UP));
        keymap.insert("Ctrl+d".to_string(), KeyMapInner::with_command_id(builtin::EXPLORER_PAGE_DOWN));
        keymap.insert("Ctrl+u".to_string(), KeyMapInner::with_command_id(builtin::EXPLORER_PAGE_UP));
        keymap.insert("g".to_string(), KeyMapInner::new()); // prefix
        keymap.insert("gg".to_string(), KeyMapInner::with_command_id(builtin::EXPLORER_GOTO_FIRST));
        keymap.insert("G".to_string(), KeyMapInner::with_command_id(builtin::EXPLORER_GOTO_LAST));

        // Tree operations
        keymap.insert("Enter".to_string(), KeyMapInner::with_command_id(builtin::EXPLORER_OPEN_NODE));
        keymap.insert("o".to_string(), KeyMapInner::with_command_id(builtin::EXPLORER_TOGGLE_NODE));
        keymap.insert("x".to_string(), KeyMapInner::with_command_id(builtin::EXPLORER_CLOSE_PARENT));
        keymap.insert("u".to_string(), KeyMapInner::with_command_id(builtin::EXPLORER_GO_TO_PARENT));
        keymap.insert("R".to_string(), KeyMapInner::with_command_id(builtin::EXPLORER_REFRESH));

        // Display
        keymap.insert("I".to_string(), KeyMapInner::with_command_id(builtin::EXPLORER_TOGGLE_HIDDEN));

        // File operations
        keymap.insert("a".to_string(), KeyMapInner::with_command_id(builtin::EXPLORER_CREATE_FILE));
        keymap.insert("A".to_string(), KeyMapInner::with_command_id(builtin::EXPLORER_CREATE_DIR));
        keymap.insert("r".to_string(), KeyMapInner::with_command_id(builtin::EXPLORER_RENAME));
        keymap.insert("d".to_string(), KeyMapInner::with_command_id(builtin::EXPLORER_DELETE));

        // Filter
        keymap.insert("/".to_string(), KeyMapInner::with_command_id(builtin::EXPLORER_FILTER));
        keymap.insert("C".to_string(), KeyMapInner::with_command_id(builtin::EXPLORER_CLEAR_FILTER));

        // Window control
        keymap.insert("q".to_string(), KeyMapInner::with_command_id(builtin::EXPLORER_CLOSE));
        keymap.insert("Tab".to_string(), KeyMapInner::with_command_id(builtin::EXPLORER_FOCUS_EDITOR));
        keymap.insert("Escape".to_string(), KeyMapInner::with_command_id(builtin::EXPLORER_FOCUS_EDITOR));
    }

    fn setup_explorer_input_mode(keymap: &mut HashMap<String, KeyMapInner>) {
        // In explorer input mode, we only handle special keys
        // Characters are handled via inline commands in the command handler
        keymap.insert("Escape".to_string(), KeyMapInner::with_command_id(builtin::EXPLORER_CANCEL_INPUT));
        keymap.insert("Enter".to_string(), KeyMapInner::with_command_id(builtin::EXPLORER_CONFIRM_INPUT));
        keymap.insert("Backspace".to_string(), KeyMapInner::with_command_id(builtin::EXPLORER_INPUT_BACKSPACE));
    }
}
