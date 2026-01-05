//! Vim-style register system with system clipboard integration

use {arboard::Clipboard, std::collections::HashMap};

/// Type of yank operation (affects how paste behaves)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum YankType {
    /// Characterwise yank - paste at cursor position (e.g., yw, y$)
    Characterwise,
    /// Linewise yank - paste below/above current line (e.g., yy, yj, yk)
    Linewise,
}

/// Register content with yank type information
#[derive(Debug, Clone)]
pub struct RegisterContent {
    pub text: String,
    pub yank_type: YankType,
}

impl RegisterContent {
    /// Create new register content with specified yank type
    #[must_use]
    pub const fn new(text: String, yank_type: YankType) -> Self {
        Self { text, yank_type }
    }

    /// Create characterwise register content
    #[must_use]
    pub const fn characterwise(text: String) -> Self {
        Self::new(text, YankType::Characterwise)
    }

    /// Create linewise register content
    #[must_use]
    pub const fn linewise(text: String) -> Self {
        Self::new(text, YankType::Linewise)
    }
}

/// Vim-style register storage with system clipboard integration
pub struct Registers {
    /// Unnamed register ("") - used by default for yank/delete/paste
    unnamed: RegisterContent,
    /// Named registers ("a-"z)
    named: HashMap<char, String>,
    /// System clipboard handle
    system_clipboard: Option<Clipboard>,
}

impl Default for Registers {
    fn default() -> Self {
        Self::new()
    }
}

impl Registers {
    /// Create a new register storage with system clipboard
    #[must_use]
    pub fn new() -> Self {
        let system_clipboard = Clipboard::new().ok();
        Self {
            unnamed: RegisterContent::characterwise(String::new()),
            named: HashMap::new(),
            system_clipboard,
        }
    }

    /// Get the unnamed register content (returns `RegisterContent` with yank type)
    #[must_use]
    pub const fn get(&self) -> &RegisterContent {
        &self.unnamed
    }

    /// Set the unnamed register content with yank type
    pub fn set_with_type(&mut self, text: String, yank_type: YankType) {
        self.unnamed = RegisterContent::new(text, yank_type);
    }

    /// Set the unnamed register content (defaults to characterwise for backward compatibility)
    pub fn set(&mut self, text: String) {
        self.unnamed = RegisterContent::characterwise(text);
    }

    /// Get a named register content ("a-"z)
    #[must_use]
    pub fn get_named(&self, name: char) -> Option<&str> {
        if name.is_ascii_lowercase() {
            self.named.get(&name).map(String::as_str)
        } else {
            None
        }
    }

    /// Set a named register content ("a-"z)
    /// Returns false if the register name is invalid
    pub fn set_named(&mut self, name: char, text: String) -> bool {
        if name.is_ascii_lowercase() {
            self.named.insert(name, text);
            true
        } else {
            false
        }
    }

    /// Get system clipboard content ("+" register)
    pub fn get_system(&mut self) -> Option<String> {
        self.system_clipboard
            .as_mut()
            .and_then(|cb| cb.get_text().ok())
    }

    /// Set system clipboard content ("+" register)
    pub fn set_system(&mut self, text: &str) {
        if let Some(cb) = self.system_clipboard.as_mut() {
            let _ = cb.set_text(text.to_string());
        }
    }

    /// Get selection clipboard content ("*" register)
    /// On Linux/Windows, this is the same as "+" register
    /// On macOS, this would be the selection buffer (not implemented separately)
    pub fn get_selection(&mut self) -> Option<String> {
        // For simplicity, treat "*" same as "+"
        self.get_system()
    }

    /// Set selection clipboard content ("*" register)
    pub fn set_selection(&mut self, text: &str) {
        // For simplicity, treat "*" same as "+"
        self.set_system(text);
    }

    /// Get content from a register by name
    /// - None or '"' -> unnamed register
    /// - 'a'-'z' -> named register
    /// - '+' -> system clipboard
    /// - '*' -> selection (same as system on Linux/Windows)
    pub fn get_by_name(&mut self, name: Option<char>) -> Option<RegisterContent> {
        match name {
            None | Some('"') => Some(self.unnamed.clone()),
            Some('+') => self.get_system().map(RegisterContent::characterwise),
            Some('*') => self.get_selection().map(RegisterContent::characterwise),
            Some(c) if c.is_ascii_lowercase() => self
                .get_named(c)
                .map(|s| RegisterContent::characterwise(s.to_string())),
            _ => None,
        }
    }

    /// Set content to a register by name (defaults to characterwise for unnamed)
    /// - None or '"' -> unnamed register
    /// - 'a'-'z' -> named register
    /// - '+' -> system clipboard
    /// - '*' -> selection (same as system on Linux/Windows)
    pub fn set_by_name(&mut self, name: Option<char>, text: String) {
        match name {
            None | Some('"') => self.unnamed = RegisterContent::characterwise(text),
            Some('+') => self.set_system(&text),
            Some('*') => self.set_selection(&text),
            Some(c) if c.is_ascii_lowercase() => {
                self.named.insert(c, text);
            }
            _ => {}
        }
    }
}
