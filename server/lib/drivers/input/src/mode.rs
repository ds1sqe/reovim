//! Mode keybinding types.
//!
//! This module defines keybinding and key sequence types.
//!
//! # Architecture
//!
//! | Layer | Responsibility |
//! |-------|---------------|
//! | Kernel | Identity: `Mode` trait, `ModeId`, `CommandId` |
//! | Input Driver (this) | Input: `KeySequence`, `Keybinding` |
//! | Modules | Policy: actual mode implementations and keybindings |
//!
//! # Example
//!
//! ```ignore
//! use reovim_driver_input::{KeySequence, Keybinding, KeyEvent, KeyCode};
//! use reovim_kernel::api::v1::{ModeId, CommandId, ModuleId};
//!
//! // Create a keybinding for "j" in normal mode
//! let module = ModuleId::new("editor");
//! let normal = ModeId::new(module.clone(), "normal");
//! let cursor_down = CommandId::new(module, "cursor-down");
//!
//! let binding = Keybinding::new(
//!     KeySequence::from_keys(&[KeyEvent::new(KeyCode::Char('j'))]),
//!     normal,
//!     cursor_down,
//!     "Move cursor down",
//! );
//! ```

use {
    crate::{KeyCode, KeyEvent, Modifiers},
    reovim_kernel::api::v1::{CommandId, ModeId},
};

// ============================================================================
// KeySequence
// ============================================================================

/// A sequence of keys for multi-key bindings.
///
/// Examples: "gg", "dd", "<C-w>h", "<Leader>f"
///
/// Key sequences are used to represent multi-key commands in vim-style
/// editors where a single action may require multiple keystrokes.
///
/// # Example
///
/// ```
/// use reovim_driver_input::{KeySequence, KeyEvent, KeyCode, Modifiers};
///
/// // Create empty sequence
/// let mut seq = KeySequence::new();
/// assert!(seq.is_empty());
///
/// // Build up a sequence
/// seq.push(KeyEvent::new(KeyCode::Char('g')));
/// seq.push(KeyEvent::new(KeyCode::Char('g')));
/// assert_eq!(seq.len(), 2);
///
/// // Create from slice
/// let gg = KeySequence::from_keys(&[
///     KeyEvent::new(KeyCode::Char('g')),
///     KeyEvent::new(KeyCode::Char('g')),
/// ]);
/// assert_eq!(gg.len(), 2);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct KeySequence(Vec<KeyEvent>);

impl KeySequence {
    /// Create a new empty key sequence.
    #[must_use]
    pub const fn new() -> Self {
        Self(Vec::new())
    }

    /// Create a key sequence from a slice of key events.
    #[must_use]
    pub fn from_keys(keys: &[KeyEvent]) -> Self {
        Self(keys.to_vec())
    }

    /// Push a key event to the sequence.
    pub fn push(&mut self, key: KeyEvent) {
        self.0.push(key);
    }

    /// Clear all keys from the sequence.
    pub fn clear(&mut self) {
        self.0.clear();
    }

    /// Check if the sequence is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Get the number of keys in the sequence.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.0.len()
    }

    /// Get the keys as a slice.
    #[must_use]
    pub fn as_slice(&self) -> &[KeyEvent] {
        &self.0
    }

    /// Check if this sequence starts with another sequence.
    ///
    /// Used for prefix matching in keymap lookup.
    #[must_use]
    pub fn starts_with(&self, prefix: &Self) -> bool {
        self.0.starts_with(&prefix.0)
    }

    /// Parse a key sequence from a string notation.
    ///
    /// Supports:
    /// - Single characters: "j", "k", "g"
    /// - Modifiers: "<C-w>" (Ctrl+W), "<A-x>" (Alt+X), "<S-Tab>" (Shift+Tab)
    /// - Special keys: "<Esc>", "<Enter>", "<Tab>", "<Space>", "<BS>"
    ///
    /// # Errors
    ///
    /// Returns `None` if the string contains invalid key notation.
    ///
    /// # Example
    ///
    /// ```
    /// use reovim_driver_input::{KeySequence, KeyCode, Modifiers};
    ///
    /// let seq = KeySequence::parse("gg").unwrap();
    /// assert_eq!(seq.len(), 2);
    ///
    /// let ctrl_w_h = KeySequence::parse("<C-w>h").unwrap();
    /// assert_eq!(ctrl_w_h.len(), 2);
    /// ```
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        let mut keys = Vec::new();
        let mut chars = s.chars().peekable();

        while let Some(c) = chars.next() {
            if c == '<' {
                // Parse special key notation
                let mut spec = String::new();
                while let Some(&next) = chars.peek() {
                    if next == '>' {
                        chars.next();
                        break;
                    }
                    spec.push(chars.next()?);
                }

                let event = Self::parse_special(&spec)?;
                keys.push(event);
            } else {
                // Plain character
                keys.push(KeyEvent::new(KeyCode::Char(c)));
            }
        }

        if keys.is_empty() {
            None
        } else {
            Some(Self(keys))
        }
    }

    /// Parse a special key notation like "C-w", "Esc", "Enter".
    fn parse_special(spec: &str) -> Option<KeyEvent> {
        // Check for modifier prefixes
        let mut modifiers = Modifiers::empty();
        let mut remaining = spec;

        // Parse modifier chain: "C-A-S-x" -> Ctrl+Alt+Shift+x
        loop {
            if let Some(rest) = remaining.strip_prefix("C-") {
                modifiers |= Modifiers::CTRL;
                remaining = rest;
            } else if let Some(rest) = remaining.strip_prefix("A-") {
                modifiers |= Modifiers::ALT;
                remaining = rest;
            } else if let Some(rest) = remaining.strip_prefix("M-") {
                // M- is alias for Alt
                modifiers |= Modifiers::ALT;
                remaining = rest;
            } else if let Some(rest) = remaining.strip_prefix("S-") {
                modifiers |= Modifiers::SHIFT;
                remaining = rest;
            } else {
                break;
            }
        }

        // Parse the key itself
        let code = match remaining.to_lowercase().as_str() {
            "esc" | "escape" => KeyCode::Escape,
            "enter" | "cr" | "return" => KeyCode::Enter,
            "tab" => KeyCode::Tab,
            "space" => KeyCode::Char(' '),
            "bs" | "backspace" => KeyCode::Backspace,
            "del" | "delete" => KeyCode::Delete,
            "up" => KeyCode::Up,
            "down" => KeyCode::Down,
            "left" => KeyCode::Left,
            "right" => KeyCode::Right,
            "home" => KeyCode::Home,
            "end" => KeyCode::End,
            "pageup" => KeyCode::PageUp,
            "pagedown" => KeyCode::PageDown,
            "f1" => KeyCode::F(1),
            "f2" => KeyCode::F(2),
            "f3" => KeyCode::F(3),
            "f4" => KeyCode::F(4),
            "f5" => KeyCode::F(5),
            "f6" => KeyCode::F(6),
            "f7" => KeyCode::F(7),
            "f8" => KeyCode::F(8),
            "f9" => KeyCode::F(9),
            "f10" => KeyCode::F(10),
            "f11" => KeyCode::F(11),
            "f12" => KeyCode::F(12),
            "lt" => KeyCode::Char('<'),
            "gt" => KeyCode::Char('>'),
            s if s.len() == 1 => {
                let ch = s.chars().next()?;
                KeyCode::Char(ch)
            }
            _ => return None,
        };

        Some(KeyEvent::with_modifiers(code, modifiers))
    }
}

impl std::fmt::Display for KeySequence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for key in &self.0 {
            // Format modifiers
            if key.modifiers.contains(Modifiers::CTRL) {
                write!(f, "<C-")?;
            }
            if key.modifiers.contains(Modifiers::ALT) {
                write!(f, "<A-")?;
            }
            if key.modifiers.contains(Modifiers::SHIFT) {
                write!(f, "<S-")?;
            }

            let has_mod = key
                .modifiers
                .intersects(Modifiers::CTRL | Modifiers::ALT | Modifiers::SHIFT);

            // Format key
            match &key.code {
                KeyCode::Char(c) => {
                    if has_mod {
                        write!(f, "{c}>")?;
                    } else {
                        write!(f, "{c}")?;
                    }
                }
                KeyCode::Escape => write!(f, "<Esc>")?,
                KeyCode::Enter => write!(f, "<Enter>")?,
                KeyCode::Tab => write!(f, "<Tab>")?,
                KeyCode::Backspace => write!(f, "<BS>")?,
                KeyCode::Delete => write!(f, "<Del>")?,
                KeyCode::Up => write!(f, "<Up>")?,
                KeyCode::Down => write!(f, "<Down>")?,
                KeyCode::Left => write!(f, "<Left>")?,
                KeyCode::Right => write!(f, "<Right>")?,
                KeyCode::Home => write!(f, "<Home>")?,
                KeyCode::End => write!(f, "<End>")?,
                KeyCode::PageUp => write!(f, "<PageUp>")?,
                KeyCode::PageDown => write!(f, "<PageDown>")?,
                KeyCode::F(n) => write!(f, "<F{n}>")?,
                _ => write!(f, "<?>")?,
            }
        }
        Ok(())
    }
}

// ============================================================================
// KeybindingTarget
// ============================================================================

/// What a keybinding triggers.
///
/// Keybindings can either execute a command or enter a mode.
/// This enables user-configurable mode bindings like `<C-t>` to enter Tetromino mode.
///
/// # TOML Syntax
///
/// ```toml
/// [bindings.normal]
/// "dd" = "editor:delete-line"      # Command (default)
/// "i" = { enter = "vim:insert" }   # Mode (explicit)
/// "<C-t>" = { enter = "tetromino:play" }
/// ```
///
/// # Example
///
/// ```ignore
/// use reovim_driver_input::KeybindingTarget;
/// use reovim_kernel::api::v1::{CommandId, ModeId, ModuleId};
///
/// let module = ModuleId::new("editor");
///
/// // Command target
/// let cmd = CommandId::new(module.clone(), "delete-line");
/// let target = KeybindingTarget::Command(cmd);
///
/// // Mode target
/// let mode = ModeId::new(module, "insert");
/// let target = KeybindingTarget::EnterMode(mode);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeybindingTarget {
    /// Execute a command.
    Command(CommandId),
    /// Enter a mode (push onto mode stack).
    EnterMode(ModeId),
}

impl KeybindingTarget {
    /// Check if this is a command target.
    #[must_use]
    pub const fn is_command(&self) -> bool {
        matches!(self, Self::Command(_))
    }

    /// Check if this is a mode target.
    #[must_use]
    pub const fn is_mode(&self) -> bool {
        matches!(self, Self::EnterMode(_))
    }

    /// Get the command if this is a command target.
    #[must_use]
    pub const fn as_command(&self) -> Option<&CommandId> {
        match self {
            Self::Command(cmd) => Some(cmd),
            Self::EnterMode(_) => None,
        }
    }

    /// Get the mode if this is a mode target.
    #[must_use]
    pub const fn as_mode(&self) -> Option<&ModeId> {
        match self {
            Self::Command(_) => None,
            Self::EnterMode(mode) => Some(mode),
        }
    }
}

impl From<CommandId> for KeybindingTarget {
    fn from(cmd: CommandId) -> Self {
        Self::Command(cmd)
    }
}

impl From<ModeId> for KeybindingTarget {
    fn from(mode: ModeId) -> Self {
        Self::EnterMode(mode)
    }
}

impl std::fmt::Display for KeybindingTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Command(cmd) => write!(f, "{cmd}"),
            Self::EnterMode(mode) => write!(f, "enter:{mode}"),
        }
    }
}

// ============================================================================
// Keybinding
// ============================================================================

/// A keybinding maps a key sequence in a mode to a command.
///
/// Keybindings are the fundamental unit of the keymap system. They specify:
/// - What keys trigger the binding
/// - Which mode the binding is active in
/// - What command to execute
/// - A human-readable description
///
/// # Example
///
/// ```ignore
/// use reovim_driver_input::{Keybinding, KeySequence, KeyEvent, KeyCode};
/// use reovim_kernel::api::v1::{ModeId, CommandId, ModuleId};
///
/// let module = ModuleId::new("editor");
/// let normal = ModeId::new(module.clone(), "normal");
/// let cmd = CommandId::new(module, "cursor-down");
///
/// let binding = Keybinding::new(
///     KeySequence::parse("j").unwrap(),
///     normal,
///     cmd,
///     "Move cursor down",
/// );
/// ```
#[derive(Debug, Clone)]
pub struct Keybinding {
    /// The key sequence that triggers this binding.
    pub keys: KeySequence,
    /// The mode in which this binding is active.
    pub mode: ModeId,
    /// The command to execute when triggered.
    pub command: CommandId,
    /// Human-readable description of the binding.
    pub description: &'static str,
}

impl Keybinding {
    /// Create a new keybinding.
    #[must_use]
    pub const fn new(
        keys: KeySequence,
        mode: ModeId,
        command: CommandId,
        description: &'static str,
    ) -> Self {
        Self {
            keys,
            mode,
            command,
            description,
        }
    }

    /// Create a keybinding from a string notation.
    ///
    /// # Errors
    ///
    /// Returns `None` if the key string is invalid.
    #[must_use]
    pub fn from_str(
        keys: &str,
        mode: ModeId,
        command: CommandId,
        description: &'static str,
    ) -> Option<Self> {
        let keys = KeySequence::parse(keys)?;
        Some(Self {
            keys,
            mode,
            command,
            description,
        })
    }
}

// ============================================================================
// Tests
// ============================================================================

