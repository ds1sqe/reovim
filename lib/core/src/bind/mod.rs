use {
    crate::command::Command,
    crate::modd::{ExtraMod, Mod},
    std::collections::{HashMap, HashSet},
};

pub struct KeyBind {
    pub on: HashSet<Mod>,
    pub on_extra: HashSet<Box<dyn ExtraMod>>,
}

pub struct KeyBindSet {
    pub privileged: bool,
    pub on: HashSet<String>,
    pub keys: String,
}

/// Node in the keymap trie
pub struct KeyMapInner {
    /// If this is a terminal node, the command to execute
    pub command: Option<Command>,
    /// Children for multi-key sequences (e.g., "dd", "gg")
    pub next: HashMap<String, KeyMapInner>,
}

impl KeyMapInner {
    pub fn new() -> Self {
        Self {
            command: None,
            next: HashMap::new(),
        }
    }

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

/// Trie for keymap
pub struct KeyMap {
    pub imap: HashMap<String, KeyMapInner>,
    pub nmap: HashMap<String, KeyMapInner>,
    pub cmap: HashMap<String, KeyMapInner>,
    pub vmap: HashMap<String, KeyMapInner>,
    pub extra: HashMap<String, KeyMapInner>,
}

impl Default for KeyMap {
    fn default() -> Self {
        Self {
            imap: HashMap::new(),
            nmap: HashMap::new(),
            cmap: HashMap::new(),
            vmap: HashMap::new(),
            extra: HashMap::new(),
        }
    }
}

impl KeyMap {
    pub fn with_defaults() -> Self {
        let mut km = Self::default();

        // Normal mode bindings
        km.nmap
            .insert("h".to_string(), KeyMapInner::with_command(Command::CursorLeft));
        km.nmap
            .insert("j".to_string(), KeyMapInner::with_command(Command::CursorDown));
        km.nmap
            .insert("k".to_string(), KeyMapInner::with_command(Command::CursorUp));
        km.nmap
            .insert("l".to_string(), KeyMapInner::with_command(Command::CursorRight));
        km.nmap
            .insert("0".to_string(), KeyMapInner::with_command(Command::CursorLineStart));
        km.nmap
            .insert("$".to_string(), KeyMapInner::with_command(Command::CursorLineEnd));
        km.nmap.insert(
            "w".to_string(),
            KeyMapInner::with_command(Command::CursorWordForward),
        );
        km.nmap.insert(
            "b".to_string(),
            KeyMapInner::with_command(Command::CursorWordBackward),
        );
        km.nmap
            .insert("i".to_string(), KeyMapInner::with_command(Command::EnterInsertMode));
        km.nmap.insert(
            "a".to_string(),
            KeyMapInner::with_command(Command::EnterInsertModeAfter),
        );
        km.nmap.insert(
            "v".to_string(),
            KeyMapInner::with_command(Command::EnterVisualMode),
        );
        km.nmap.insert(
            ":".to_string(),
            KeyMapInner::with_command(Command::EnterCommandMode),
        );
        km.nmap.insert(
            "x".to_string(),
            KeyMapInner::with_command(Command::DeleteCharForward),
        );
        km.nmap
            .insert("q".to_string(), KeyMapInner::with_command(Command::Quit));
        km.nmap
            .insert("p".to_string(), KeyMapInner::with_command(Command::Paste));
        km.nmap
            .insert("P".to_string(), KeyMapInner::with_command(Command::PasteBefore));

        // Insert mode bindings
        km.imap.insert(
            "Escape".to_string(),
            KeyMapInner::with_command(Command::EnterNormalMode),
        );
        km.imap.insert(
            "Backspace".to_string(),
            KeyMapInner::with_command(Command::DeleteCharBackward),
        );

        // Visual mode bindings
        km.vmap.insert(
            "Escape".to_string(),
            KeyMapInner::with_command(Command::EnterNormalMode),
        );
        km.vmap.insert(
            "h".to_string(),
            KeyMapInner::with_command(Command::VisualExtendLeft),
        );
        km.vmap.insert(
            "j".to_string(),
            KeyMapInner::with_command(Command::VisualExtendDown),
        );
        km.vmap.insert(
            "k".to_string(),
            KeyMapInner::with_command(Command::VisualExtendUp),
        );
        km.vmap.insert(
            "l".to_string(),
            KeyMapInner::with_command(Command::VisualExtendRight),
        );
        km.vmap.insert(
            "d".to_string(),
            KeyMapInner::with_command(Command::VisualDelete),
        );
        km.vmap.insert(
            "y".to_string(),
            KeyMapInner::with_command(Command::VisualYank),
        );

        // Command mode bindings
        km.cmap.insert(
            "Escape".to_string(),
            KeyMapInner::with_command(Command::CommandLineCancel),
        );
        km.cmap.insert(
            "Enter".to_string(),
            KeyMapInner::with_command(Command::CommandLineExecute),
        );
        km.cmap.insert(
            "Backspace".to_string(),
            KeyMapInner::with_command(Command::CommandLineBackspace),
        );

        km
    }
}
