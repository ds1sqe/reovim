use {crate::command::Command, std::collections::HashMap};

/// Node in the keymap trie
pub struct KeyMapInner {
    /// If this is a terminal node, the command to execute
    pub command: Option<Command>,
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

    #[must_use]
    pub fn with_command(cmd: Command) -> Self {
        Self {
            command: Some(cmd),
            next: HashMap::new(),
        }
    }
}

impl Default for KeyMapInner {
    fn default() -> Self {
        Self::new()
    }
}

/// Keymap for each mode
#[derive(Default)]
pub struct KeyMap {
    pub insert: HashMap<String, KeyMapInner>,
    pub normal: HashMap<String, KeyMapInner>,
    pub command: HashMap<String, KeyMapInner>,
    pub visual: HashMap<String, KeyMapInner>,
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
        km
    }

    fn setup_normal_mode(keymap: &mut HashMap<String, KeyMapInner>) {
        // Movement
        keymap.insert("h".to_string(), KeyMapInner::with_command(Command::CursorLeft));
        keymap.insert("j".to_string(), KeyMapInner::with_command(Command::CursorDown));
        keymap.insert("k".to_string(), KeyMapInner::with_command(Command::CursorUp));
        keymap.insert("l".to_string(), KeyMapInner::with_command(Command::CursorRight));
        keymap.insert("0".to_string(), KeyMapInner::with_command(Command::CursorLineStart));
        keymap.insert("$".to_string(), KeyMapInner::with_command(Command::CursorLineEnd));
        keymap.insert("w".to_string(), KeyMapInner::with_command(Command::CursorWordForward));
        keymap.insert("b".to_string(), KeyMapInner::with_command(Command::CursorWordBackward));

        // Mode switching
        keymap.insert("i".to_string(), KeyMapInner::with_command(Command::EnterInsertMode));
        keymap.insert("a".to_string(), KeyMapInner::with_command(Command::EnterInsertModeAfter));
        keymap.insert("A".to_string(), KeyMapInner::with_command(Command::EnterInsertModeEndOfLine));
        keymap.insert("o".to_string(), KeyMapInner::with_command(Command::OpenLineBelow));
        keymap.insert("O".to_string(), KeyMapInner::with_command(Command::OpenLineAbove));
        keymap.insert("v".to_string(), KeyMapInner::with_command(Command::EnterVisualMode));
        keymap.insert(":".to_string(), KeyMapInner::with_command(Command::EnterCommandMode));

        // Editing
        keymap.insert("x".to_string(), KeyMapInner::with_command(Command::DeleteCharForward));
        keymap.insert("p".to_string(), KeyMapInner::with_command(Command::Paste));
        keymap.insert("P".to_string(), KeyMapInner::with_command(Command::PasteBefore));

        // g-prefix bindings
        keymap.insert("g".to_string(), KeyMapInner::new()); // prefix, no command
        keymap.insert("gg".to_string(), KeyMapInner::with_command(Command::GotoFirstLine));
        keymap.insert("G".to_string(), KeyMapInner::with_command(Command::GotoLastLine));
    }

    fn setup_insert_mode(keymap: &mut HashMap<String, KeyMapInner>) {
        keymap.insert("Escape".to_string(), KeyMapInner::with_command(Command::EnterNormalMode));
        keymap.insert("Backspace".to_string(), KeyMapInner::with_command(Command::DeleteCharBackward));
    }

    fn setup_visual_mode(keymap: &mut HashMap<String, KeyMapInner>) {
        keymap.insert("Escape".to_string(), KeyMapInner::with_command(Command::EnterNormalMode));
        keymap.insert("h".to_string(), KeyMapInner::with_command(Command::VisualExtendLeft));
        keymap.insert("j".to_string(), KeyMapInner::with_command(Command::VisualExtendDown));
        keymap.insert("k".to_string(), KeyMapInner::with_command(Command::VisualExtendUp));
        keymap.insert("l".to_string(), KeyMapInner::with_command(Command::VisualExtendRight));
        keymap.insert("d".to_string(), KeyMapInner::with_command(Command::VisualDelete));
        keymap.insert("y".to_string(), KeyMapInner::with_command(Command::VisualYank));
    }

    fn setup_command_mode(keymap: &mut HashMap<String, KeyMapInner>) {
        keymap.insert("Escape".to_string(), KeyMapInner::with_command(Command::CommandLineCancel));
        keymap.insert("Enter".to_string(), KeyMapInner::with_command(Command::CommandLineExecute));
        keymap.insert("Backspace".to_string(), KeyMapInner::with_command(Command::CommandLineBackspace));
    }
}
