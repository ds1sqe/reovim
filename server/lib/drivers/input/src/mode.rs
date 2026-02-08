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
/// This enables user-configurable mode bindings like `<C-t>` to enter Tetris mode.
///
/// # TOML Syntax
///
/// ```toml
/// [bindings.normal]
/// "dd" = "editor:delete-line"      # Command (default)
/// "i" = { enter = "vim:insert" }   # Mode (explicit)
/// "<C-t>" = { enter = "tetris:play" }
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

#[cfg(test)]
mod tests {
    use {super::*, reovim_kernel::api::v1::ModuleId};

    #[test]
    fn test_key_sequence_new() {
        let seq = KeySequence::new();
        assert!(seq.is_empty());
        assert_eq!(seq.len(), 0);
    }

    #[test]
    fn test_key_sequence_push() {
        let mut seq = KeySequence::new();
        seq.push(KeyEvent::new(KeyCode::Char('g')));
        seq.push(KeyEvent::new(KeyCode::Char('g')));
        assert!(!seq.is_empty());
        assert_eq!(seq.len(), 2);
    }

    #[test]
    fn test_key_sequence_from_keys() {
        let keys = [
            KeyEvent::new(KeyCode::Char('d')),
            KeyEvent::new(KeyCode::Char('d')),
        ];
        let seq = KeySequence::from_keys(&keys);
        assert_eq!(seq.len(), 2);
        assert_eq!(seq.as_slice(), &keys);
    }

    #[test]
    fn test_key_sequence_clear() {
        let mut seq = KeySequence::from_keys(&[
            KeyEvent::new(KeyCode::Char('a')),
            KeyEvent::new(KeyCode::Char('b')),
        ]);
        assert!(!seq.is_empty());
        seq.clear();
        assert!(seq.is_empty());
    }

    #[test]
    fn test_key_sequence_starts_with() {
        let full = KeySequence::from_keys(&[
            KeyEvent::new(KeyCode::Char('g')),
            KeyEvent::new(KeyCode::Char('g')),
        ]);
        let prefix = KeySequence::from_keys(&[KeyEvent::new(KeyCode::Char('g'))]);
        let other = KeySequence::from_keys(&[KeyEvent::new(KeyCode::Char('d'))]);

        assert!(full.starts_with(&prefix));
        assert!(full.starts_with(&full));
        assert!(!full.starts_with(&other));
    }

    #[test]
    fn test_key_sequence_parse_simple() {
        let seq = KeySequence::parse("gg").unwrap();
        assert_eq!(seq.len(), 2);
        assert_eq!(seq.as_slice()[0].code, KeyCode::Char('g'));
        assert_eq!(seq.as_slice()[1].code, KeyCode::Char('g'));
    }

    #[test]
    fn test_key_sequence_parse_special() {
        let seq = KeySequence::parse("<Esc>").unwrap();
        assert_eq!(seq.len(), 1);
        assert_eq!(seq.as_slice()[0].code, KeyCode::Escape);
    }

    #[test]
    fn test_key_sequence_parse_ctrl() {
        let seq = KeySequence::parse("<C-w>").unwrap();
        assert_eq!(seq.len(), 1);
        assert_eq!(seq.as_slice()[0].code, KeyCode::Char('w'));
        assert!(seq.as_slice()[0].modifiers.contains(Modifiers::CTRL));
    }

    #[test]
    fn test_key_sequence_parse_mixed() {
        let seq = KeySequence::parse("<C-w>h").unwrap();
        assert_eq!(seq.len(), 2);
        assert_eq!(seq.as_slice()[0].code, KeyCode::Char('w'));
        assert!(seq.as_slice()[0].modifiers.contains(Modifiers::CTRL));
        assert_eq!(seq.as_slice()[1].code, KeyCode::Char('h'));
    }

    #[test]
    fn test_key_sequence_parse_empty() {
        assert!(KeySequence::parse("").is_none());
    }

    #[test]
    fn test_key_sequence_parse_unicode_emoji() {
        // Test emoji parsing (#372)
        let seq = KeySequence::parse("🎉").unwrap();
        assert_eq!(seq.len(), 1);
        assert_eq!(seq.as_slice()[0].code, KeyCode::Char('🎉'));

        // Test emoji in sequence
        let seq = KeySequence::parse("i🎉<Esc>").unwrap();
        assert_eq!(seq.len(), 3);
        assert_eq!(seq.as_slice()[0].code, KeyCode::Char('i'));
        assert_eq!(seq.as_slice()[1].code, KeyCode::Char('🎉'));
        assert_eq!(seq.as_slice()[2].code, KeyCode::Escape);
    }

    #[test]
    fn test_key_sequence_display() {
        let seq = KeySequence::from_keys(&[
            KeyEvent::new(KeyCode::Char('g')),
            KeyEvent::new(KeyCode::Char('g')),
        ]);
        assert_eq!(format!("{seq}"), "gg");

        let ctrl_w = KeySequence::from_keys(&[KeyEvent::with_modifiers(
            KeyCode::Char('w'),
            Modifiers::CTRL,
        )]);
        assert_eq!(format!("{ctrl_w}"), "<C-w>");
    }

    #[test]
    fn test_keybinding_new() {
        let module = ModuleId::new("test");
        let mode = ModeId::new(module.clone(), "normal");
        let cmd = CommandId::new(module, "test-cmd");

        let binding = Keybinding::new(KeySequence::new(), mode.clone(), cmd.clone(), "Test");

        assert_eq!(binding.mode, mode);
        assert_eq!(binding.command, cmd);
        assert_eq!(binding.description, "Test");
    }

    #[test]
    fn test_keybinding_from_str() {
        let module = ModuleId::new("test");
        let mode = ModeId::new(module.clone(), "normal");
        let cmd = CommandId::new(module, "down");

        let binding = Keybinding::from_str("j", mode.clone(), cmd.clone(), "Move down").unwrap();

        assert_eq!(binding.keys.len(), 1);
        assert_eq!(binding.mode, mode);
        assert_eq!(binding.command, cmd);
    }

    // ========================================================================
    // KeybindingTarget tests
    // ========================================================================

    #[test]
    fn test_keybinding_target_command() {
        let module = ModuleId::new("test");
        let cmd = CommandId::new(module, "test-cmd");
        let target = KeybindingTarget::Command(cmd.clone());

        assert!(target.is_command());
        assert!(!target.is_mode());
        assert_eq!(target.as_command(), Some(&cmd));
        assert_eq!(target.as_mode(), None);
    }

    #[test]
    fn test_keybinding_target_mode() {
        let module = ModuleId::new("test");
        let mode = ModeId::new(module, "insert");
        let target = KeybindingTarget::EnterMode(mode.clone());

        assert!(!target.is_command());
        assert!(target.is_mode());
        assert_eq!(target.as_command(), None);
        assert_eq!(target.as_mode(), Some(&mode));
    }

    #[test]
    fn test_keybinding_target_from_command() {
        let module = ModuleId::new("test");
        let cmd = CommandId::new(module, "test-cmd");
        let target: KeybindingTarget = cmd.clone().into();

        assert!(target.is_command());
        assert_eq!(target.as_command(), Some(&cmd));
    }

    #[test]
    fn test_keybinding_target_from_mode() {
        let module = ModuleId::new("test");
        let mode = ModeId::new(module, "insert");
        let target: KeybindingTarget = mode.clone().into();

        assert!(target.is_mode());
        assert_eq!(target.as_mode(), Some(&mode));
    }

    #[test]
    fn test_keybinding_target_display() {
        let module = ModuleId::new("test");
        let cmd = CommandId::new(module.clone(), "delete");
        let mode = ModeId::new(module, "insert");

        let cmd_target = KeybindingTarget::Command(cmd);
        assert_eq!(format!("{cmd_target}"), "test:delete");

        let mode_target = KeybindingTarget::EnterMode(mode);
        assert_eq!(format!("{mode_target}"), "enter:test:insert");
    }

    #[test]
    fn test_keybinding_target_equality() {
        let module = ModuleId::new("test");
        let cmd1 = CommandId::new(module.clone(), "cmd");
        let cmd2 = CommandId::new(module.clone(), "cmd");
        let mode = ModeId::new(module, "insert");

        let target1 = KeybindingTarget::Command(cmd1);
        let target2 = KeybindingTarget::Command(cmd2);
        let target3 = KeybindingTarget::EnterMode(mode);

        assert_eq!(target1, target2);
        assert_ne!(target1, target3);
    }

    // ========================================================================
    // Additional KeySequence::parse tests for uncovered paths
    // ========================================================================

    #[test]
    fn test_key_sequence_parse_alt_modifier() {
        let seq = KeySequence::parse("<A-x>").unwrap();
        assert_eq!(seq.len(), 1);
        assert_eq!(seq.as_slice()[0].code, KeyCode::Char('x'));
        assert!(seq.as_slice()[0].modifiers.contains(Modifiers::ALT));
    }

    #[test]
    fn test_key_sequence_parse_meta_alias_for_alt() {
        // M- is alias for Alt
        let seq = KeySequence::parse("<M-x>").unwrap();
        assert_eq!(seq.len(), 1);
        assert_eq!(seq.as_slice()[0].code, KeyCode::Char('x'));
        assert!(seq.as_slice()[0].modifiers.contains(Modifiers::ALT));
    }

    #[test]
    fn test_key_sequence_parse_shift_modifier() {
        let seq = KeySequence::parse("<S-Tab>").unwrap();
        assert_eq!(seq.len(), 1);
        assert_eq!(seq.as_slice()[0].code, KeyCode::Tab);
        assert!(seq.as_slice()[0].modifiers.contains(Modifiers::SHIFT));
    }

    #[test]
    fn test_key_sequence_parse_combined_modifiers() {
        let seq = KeySequence::parse("<C-A-S-x>").unwrap();
        assert_eq!(seq.len(), 1);
        assert!(seq.as_slice()[0].modifiers.contains(Modifiers::CTRL));
        assert!(seq.as_slice()[0].modifiers.contains(Modifiers::ALT));
        assert!(seq.as_slice()[0].modifiers.contains(Modifiers::SHIFT));
    }

    #[test]
    fn test_key_sequence_parse_enter_aliases() {
        for alias in ["<Enter>", "<CR>", "<Return>"] {
            let seq = KeySequence::parse(alias).unwrap();
            assert_eq!(seq.len(), 1);
            assert_eq!(
                seq.as_slice()[0].code,
                KeyCode::Enter,
                "alias {alias} did not parse to Enter"
            );
        }
    }

    #[test]
    fn test_key_sequence_parse_escape_aliases() {
        for alias in ["<Esc>", "<Escape>"] {
            let seq = KeySequence::parse(alias).unwrap();
            assert_eq!(seq.len(), 1);
            assert_eq!(
                seq.as_slice()[0].code,
                KeyCode::Escape,
                "alias {alias} did not parse to Escape"
            );
        }
    }

    #[test]
    fn test_key_sequence_parse_space() {
        let seq = KeySequence::parse("<Space>").unwrap();
        assert_eq!(seq.len(), 1);
        assert_eq!(seq.as_slice()[0].code, KeyCode::Char(' '));
    }

    #[test]
    fn test_key_sequence_parse_backspace_aliases() {
        for alias in ["<BS>", "<Backspace>"] {
            let seq = KeySequence::parse(alias).unwrap();
            assert_eq!(seq.len(), 1);
            assert_eq!(
                seq.as_slice()[0].code,
                KeyCode::Backspace,
                "alias {alias} did not parse to Backspace"
            );
        }
    }

    #[test]
    fn test_key_sequence_parse_delete_aliases() {
        for alias in ["<Del>", "<Delete>"] {
            let seq = KeySequence::parse(alias).unwrap();
            assert_eq!(seq.len(), 1);
            assert_eq!(
                seq.as_slice()[0].code,
                KeyCode::Delete,
                "alias {alias} did not parse to Delete"
            );
        }
    }

    #[test]
    fn test_key_sequence_parse_arrow_keys() {
        let cases = [
            ("<Up>", KeyCode::Up),
            ("<Down>", KeyCode::Down),
            ("<Left>", KeyCode::Left),
            ("<Right>", KeyCode::Right),
        ];
        for (notation, expected) in cases {
            let seq = KeySequence::parse(notation).unwrap();
            assert_eq!(seq.len(), 1);
            assert_eq!(
                seq.as_slice()[0].code,
                expected,
                "notation {notation} did not parse correctly"
            );
        }
    }

    #[test]
    fn test_key_sequence_parse_navigation_keys() {
        let cases = [
            ("<Home>", KeyCode::Home),
            ("<End>", KeyCode::End),
            ("<PageUp>", KeyCode::PageUp),
            ("<PageDown>", KeyCode::PageDown),
        ];
        for (notation, expected) in cases {
            let seq = KeySequence::parse(notation).unwrap();
            assert_eq!(seq.len(), 1);
            assert_eq!(
                seq.as_slice()[0].code,
                expected,
                "notation {notation} did not parse correctly"
            );
        }
    }

    #[test]
    fn test_key_sequence_parse_function_keys() {
        for n in 1..=12u8 {
            let notation = format!("<F{n}>");
            let seq = KeySequence::parse(&notation).unwrap();
            assert_eq!(seq.len(), 1);
            assert_eq!(seq.as_slice()[0].code, KeyCode::F(n), "F{n} did not parse correctly");
        }
    }

    #[test]
    fn test_key_sequence_parse_tab() {
        let seq = KeySequence::parse("<Tab>").unwrap();
        assert_eq!(seq.len(), 1);
        assert_eq!(seq.as_slice()[0].code, KeyCode::Tab);
    }

    #[test]
    fn test_key_sequence_parse_lt_gt() {
        let seq = KeySequence::parse("<lt>").unwrap();
        assert_eq!(seq.len(), 1);
        assert_eq!(seq.as_slice()[0].code, KeyCode::Char('<'));

        let seq = KeySequence::parse("<gt>").unwrap();
        assert_eq!(seq.len(), 1);
        assert_eq!(seq.as_slice()[0].code, KeyCode::Char('>'));
    }

    #[test]
    fn test_key_sequence_parse_invalid_special() {
        // Unknown key name should return None
        assert!(KeySequence::parse("<Unknown>").is_none());
        assert!(KeySequence::parse("<InvalidKey>").is_none());
    }

    #[test]
    fn test_key_sequence_parse_case_insensitive() {
        // parse_special converts to lowercase
        let seq = KeySequence::parse("<ESC>").unwrap();
        assert_eq!(seq.as_slice()[0].code, KeyCode::Escape);

        let seq = KeySequence::parse("<ENTER>").unwrap();
        assert_eq!(seq.as_slice()[0].code, KeyCode::Enter);
    }

    // ========================================================================
    // Additional KeySequence::Display tests
    // ========================================================================

    #[test]
    fn test_key_sequence_display_special_keys() {
        let test_cases = [
            (KeyCode::Escape, "<Esc>"),
            (KeyCode::Enter, "<Enter>"),
            (KeyCode::Tab, "<Tab>"),
            (KeyCode::Backspace, "<BS>"),
            (KeyCode::Delete, "<Del>"),
            (KeyCode::Up, "<Up>"),
            (KeyCode::Down, "<Down>"),
            (KeyCode::Left, "<Left>"),
            (KeyCode::Right, "<Right>"),
            (KeyCode::Home, "<Home>"),
            (KeyCode::End, "<End>"),
            (KeyCode::PageUp, "<PageUp>"),
            (KeyCode::PageDown, "<PageDown>"),
        ];

        for (code, expected) in test_cases {
            let seq = KeySequence::from_keys(&[KeyEvent::new(code)]);
            assert_eq!(format!("{seq}"), expected, "Display for {code:?} was wrong");
        }
    }

    #[test]
    fn test_key_sequence_display_function_key() {
        let seq = KeySequence::from_keys(&[KeyEvent::new(KeyCode::F(5))]);
        assert_eq!(format!("{seq}"), "<F5>");
    }

    #[test]
    fn test_key_sequence_display_alt_modifier() {
        let seq =
            KeySequence::from_keys(&[KeyEvent::with_modifiers(KeyCode::Char('x'), Modifiers::ALT)]);
        assert_eq!(format!("{seq}"), "<A-x>");
    }

    #[test]
    fn test_key_sequence_display_shift_modifier() {
        let seq = KeySequence::from_keys(&[KeyEvent::with_modifiers(
            KeyCode::Char('a'),
            Modifiers::SHIFT,
        )]);
        assert_eq!(format!("{seq}"), "<S-a>");
    }

    #[test]
    fn test_key_sequence_display_combined_modifiers() {
        let seq = KeySequence::from_keys(&[KeyEvent::with_modifiers(
            KeyCode::Char('x'),
            Modifiers::CTRL | Modifiers::ALT,
        )]);
        let display = format!("{seq}");
        assert!(display.contains("C-"));
        assert!(display.contains("A-"));
    }

    #[test]
    fn test_key_sequence_display_unknown_key() {
        // Test fallback display for keys not in the match
        let seq = KeySequence::from_keys(&[KeyEvent::new(KeyCode::Insert)]);
        assert_eq!(format!("{seq}"), "<?>");
    }

    #[test]
    fn test_key_sequence_display_mixed() {
        let seq = KeySequence::from_keys(&[
            KeyEvent::with_modifiers(KeyCode::Char('w'), Modifiers::CTRL),
            KeyEvent::new(KeyCode::Char('h')),
        ]);
        assert_eq!(format!("{seq}"), "<C-w>h");
    }

    #[test]
    fn test_key_sequence_display_empty() {
        let seq = KeySequence::new();
        assert_eq!(format!("{seq}"), "");
    }

    // ========================================================================
    // KeySequence default and equality tests
    // ========================================================================

    #[test]
    fn test_key_sequence_default_is_empty() {
        let seq = KeySequence::default();
        assert!(seq.is_empty());
    }

    #[test]
    fn test_key_sequence_hash() {
        use std::collections::HashSet;
        let seq1 = KeySequence::from_keys(&[KeyEvent::new(KeyCode::Char('j'))]);
        let seq2 = KeySequence::from_keys(&[KeyEvent::new(KeyCode::Char('j'))]);
        let seq3 = KeySequence::from_keys(&[KeyEvent::new(KeyCode::Char('k'))]);

        let mut set = HashSet::new();
        set.insert(seq1);
        set.insert(seq2);
        assert_eq!(set.len(), 1); // seq1 and seq2 are equal
        set.insert(seq3);
        assert_eq!(set.len(), 2);
    }

    // ========================================================================
    // Keybinding::from_str additional test
    // ========================================================================

    #[test]
    fn test_keybinding_from_str_invalid_returns_none() {
        let module = ModuleId::new("test");
        let mode = ModeId::new(module.clone(), "normal");
        let cmd = CommandId::new(module, "cmd");

        // Empty string
        let binding = Keybinding::from_str("", mode, cmd, "empty");
        assert!(binding.is_none());
    }

    #[test]
    fn test_keybinding_from_str_special_key() {
        let module = ModuleId::new("test");
        let mode = ModeId::new(module.clone(), "normal");
        let cmd = CommandId::new(module, "escape");

        let binding = Keybinding::from_str("<Esc>", mode, cmd, "Escape").unwrap();
        assert_eq!(binding.keys.len(), 1);
        assert_eq!(binding.keys.as_slice()[0].code, KeyCode::Escape);
    }

    #[test]
    fn test_keybinding_from_str_multi_key() {
        let module = ModuleId::new("test");
        let mode = ModeId::new(module.clone(), "normal");
        let cmd = CommandId::new(module, "goto-top");

        let binding = Keybinding::from_str("gg", mode, cmd, "Go to top").unwrap();
        assert_eq!(binding.keys.len(), 2);
    }
}
