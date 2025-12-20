//! Keystroke and `KeySequence` types for explicit key definitions.
//!
//! This module provides structured types for defining keyboard input,
//! replacing implicit string-based key definitions with explicit structs.

mod macros;
mod notation;

pub use notation::KeyNotationFormat;

use {
    reovim_sys::event::{KeyCode, KeyEvent, KeyModifiers},
    std::fmt,
};

/// Represents a keyboard key without modifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    /// A character key (a-z, 0-9, punctuation, etc.)
    Char(char),
    /// Space key (explicit for leader key clarity)
    Space,
    /// Escape key
    Escape,
    /// Enter/Return key
    Enter,
    /// Tab key
    Tab,
    /// Backspace key
    Backspace,
    /// Delete key
    Delete,
    /// Insert key
    Insert,
    /// Home key
    Home,
    /// End key
    End,
    /// Page Up key
    PageUp,
    /// Page Down key
    PageDown,
    /// Arrow Up
    Up,
    /// Arrow Down
    Down,
    /// Arrow Left
    Left,
    /// Arrow Right
    Right,
    /// Function key (F1-F12)
    F(u8),
}

/// Modifier keys for a keystroke.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Modifiers {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

impl Modifiers {
    pub const NONE: Self = Self {
        ctrl: false,
        shift: false,
        alt: false,
    };

    pub const CTRL: Self = Self {
        ctrl: true,
        shift: false,
        alt: false,
    };

    pub const SHIFT: Self = Self {
        ctrl: false,
        shift: true,
        alt: false,
    };

    pub const ALT: Self = Self {
        ctrl: false,
        shift: false,
        alt: true,
    };

    #[must_use]
    pub const fn with_ctrl(self) -> Self {
        Self { ctrl: true, ..self }
    }

    #[must_use]
    pub const fn with_shift(self) -> Self {
        Self {
            shift: true,
            ..self
        }
    }

    #[must_use]
    pub const fn with_alt(self) -> Self {
        Self { alt: true, ..self }
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        !self.ctrl && !self.shift && !self.alt
    }
}

impl From<KeyModifiers> for Modifiers {
    fn from(km: KeyModifiers) -> Self {
        Self {
            ctrl: km.contains(KeyModifiers::CONTROL),
            shift: km.contains(KeyModifiers::SHIFT),
            alt: km.contains(KeyModifiers::ALT),
        }
    }
}

/// A single keystroke: a key with optional modifiers.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Keystroke {
    pub key: Key,
    pub modifiers: Modifiers,
}

impl Keystroke {
    #[must_use]
    pub const fn new(key: Key, modifiers: Modifiers) -> Self {
        Self { key, modifiers }
    }

    #[must_use]
    pub const fn char(c: char) -> Self {
        Self {
            key: Key::Char(c),
            modifiers: Modifiers::NONE,
        }
    }

    #[must_use]
    pub const fn ctrl(key: Key) -> Self {
        Self {
            key,
            modifiers: Modifiers::CTRL,
        }
    }

    #[must_use]
    pub const fn shift(key: Key) -> Self {
        Self {
            key,
            modifiers: Modifiers::SHIFT,
        }
    }

    #[must_use]
    pub const fn alt(key: Key) -> Self {
        Self {
            key,
            modifiers: Modifiers::ALT,
        }
    }

    #[must_use]
    pub const fn escape() -> Self {
        Self {
            key: Key::Escape,
            modifiers: Modifiers::NONE,
        }
    }

    #[must_use]
    pub const fn enter() -> Self {
        Self {
            key: Key::Enter,
            modifiers: Modifiers::NONE,
        }
    }

    #[must_use]
    pub const fn space() -> Self {
        Self {
            key: Key::Space,
            modifiers: Modifiers::NONE,
        }
    }

    #[must_use]
    pub const fn tab() -> Self {
        Self {
            key: Key::Tab,
            modifiers: Modifiers::NONE,
        }
    }

    #[must_use]
    pub const fn backspace() -> Self {
        Self {
            key: Key::Backspace,
            modifiers: Modifiers::NONE,
        }
    }
}

impl From<&KeyEvent> for Keystroke {
    fn from(event: &KeyEvent) -> Self {
        let key = match event.code {
            KeyCode::Char(' ') => Key::Space,
            KeyCode::Char(c) => Key::Char(c),
            KeyCode::Esc => Key::Escape,
            KeyCode::Enter => Key::Enter,
            KeyCode::Tab | KeyCode::BackTab => Key::Tab, // BackTab has shift modifier
            KeyCode::Backspace => Key::Backspace,
            KeyCode::Delete => Key::Delete,
            KeyCode::Insert => Key::Insert,
            KeyCode::Home => Key::Home,
            KeyCode::End => Key::End,
            KeyCode::PageUp => Key::PageUp,
            KeyCode::PageDown => Key::PageDown,
            KeyCode::Up => Key::Up,
            KeyCode::Down => Key::Down,
            KeyCode::Left => Key::Left,
            KeyCode::Right => Key::Right,
            KeyCode::F(n) => Key::F(n),
            _ => Key::Char('\0'), // Unknown key
        };

        let mut modifiers = Modifiers::from(event.modifiers);

        // BackTab implies shift
        if event.code == KeyCode::BackTab {
            modifiers.shift = true;
        }

        // For uppercase ASCII letters, the shift is already encoded in the character.
        // Don't consider it as a modifier for binding matching (e.g., 'G' matches Shift+g).
        if let KeyCode::Char(c) = event.code
            && c.is_ascii_uppercase()
        {
            modifiers.shift = false;
        }

        Self { key, modifiers }
    }
}

impl From<KeyEvent> for Keystroke {
    fn from(event: KeyEvent) -> Self {
        Self::from(&event)
    }
}

impl fmt::Display for Keystroke {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.render(KeyNotationFormat::Vim))
    }
}

/// A sequence of keystrokes (e.g., "gg", "leader-f-f").
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct KeySequence(pub Vec<Keystroke>);

impl KeySequence {
    #[must_use]
    pub const fn new() -> Self {
        Self(Vec::new())
    }

    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // Vec::new() is not const in stable
    pub fn from_vec(keys: Vec<Keystroke>) -> Self {
        Self(keys)
    }

    #[must_use]
    pub fn single(key: Keystroke) -> Self {
        Self(vec![key])
    }

    pub fn push(&mut self, key: Keystroke) {
        self.0.push(key);
    }

    pub fn pop(&mut self) -> Option<Keystroke> {
        self.0.pop()
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }

    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // Vec::is_empty() is not const in stable
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // Vec::len() is not const in stable
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[must_use]
    pub fn first(&self) -> Option<&Keystroke> {
        self.0.first()
    }

    #[must_use]
    pub fn last(&self) -> Option<&Keystroke> {
        self.0.last()
    }

    /// Check if this sequence is a prefix of another.
    #[must_use]
    pub fn is_prefix_of(&self, other: &Self) -> bool {
        if self.0.len() > other.0.len() {
            return false;
        }
        self.0.iter().zip(other.0.iter()).all(|(a, b)| a == b)
    }

    /// Check if this sequence starts with the given prefix.
    #[must_use]
    pub fn starts_with(&self, prefix: &Self) -> bool {
        prefix.is_prefix_of(self)
    }

    /// Returns an iterator over the keystrokes.
    pub fn iter(&self) -> impl Iterator<Item = &Keystroke> {
        self.0.iter()
    }
}

impl FromIterator<Keystroke> for KeySequence {
    fn from_iter<I: IntoIterator<Item = Keystroke>>(iter: I) -> Self {
        Self(iter.into_iter().collect())
    }
}

impl IntoIterator for KeySequence {
    type Item = Keystroke;
    type IntoIter = std::vec::IntoIter<Keystroke>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a> IntoIterator for &'a KeySequence {
    type Item = &'a Keystroke;
    type IntoIter = std::slice::Iter<'a, Keystroke>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl fmt::Display for KeySequence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.render(KeyNotationFormat::Vim))
    }
}

impl KeySequence {
    /// Parse a key sequence from a string (for backward compatibility).
    ///
    /// Supports: "gg", " ff", "<C-v>", "dd", "y$"
    ///
    /// # Panics
    ///
    /// This function will not panic under normal usage. The internal `unwrap()`
    /// is guarded by `peek()` so the iterator always has a next element.
    #[must_use]
    pub fn parse(s: &str) -> Self {
        let mut keys = Vec::new();
        let mut chars = s.chars().peekable();

        while let Some(c) = chars.next() {
            if c == '<' {
                // Parse bracketed notation like <C-v>, <Esc>
                let mut inner = String::new();
                while let Some(&next) = chars.peek() {
                    if next == '>' {
                        chars.next();
                        break;
                    }
                    inner.push(chars.next().unwrap());
                }
                if let Some(ks) = Self::parse_bracketed(&inner) {
                    keys.push(ks);
                }
            } else if c == ' ' {
                keys.push(Keystroke::space());
            } else {
                keys.push(Keystroke::char(c));
            }
        }

        Self(keys)
    }

    /// Parse bracketed notation like "C-v", "Esc", "S-Tab"
    fn parse_bracketed(s: &str) -> Option<Keystroke> {
        let mut modifiers = Modifiers::NONE;
        let mut remaining = s;

        // Parse modifier prefixes
        loop {
            if let Some(rest) = remaining.strip_prefix("C-") {
                modifiers.ctrl = true;
                remaining = rest;
            } else if let Some(rest) = remaining.strip_prefix("S-") {
                modifiers.shift = true;
                remaining = rest;
            } else if let Some(rest) = remaining
                .strip_prefix("A-")
                .or_else(|| remaining.strip_prefix("M-"))
            {
                modifiers.alt = true;
                remaining = rest;
            } else {
                break;
            }
        }

        // Parse key name
        let key = Self::parse_key_name(remaining)?;
        Some(Keystroke::new(key, modifiers))
    }

    fn parse_key_name(s: &str) -> Option<Key> {
        let upper = s.to_uppercase();
        match upper.as_str() {
            "ESC" | "ESCAPE" => Some(Key::Escape),
            "CR" | "ENTER" | "RETURN" => Some(Key::Enter),
            "TAB" => Some(Key::Tab),
            "BS" | "BACKSPACE" => Some(Key::Backspace),
            "DEL" | "DELETE" => Some(Key::Delete),
            "INS" | "INSERT" => Some(Key::Insert),
            "HOME" => Some(Key::Home),
            "END" => Some(Key::End),
            "PAGEUP" | "PGUP" => Some(Key::PageUp),
            "PAGEDOWN" | "PGDN" => Some(Key::PageDown),
            "UP" => Some(Key::Up),
            "DOWN" => Some(Key::Down),
            "LEFT" => Some(Key::Left),
            "RIGHT" => Some(Key::Right),
            "SPACE" | "SPC" => Some(Key::Space),
            _ if s.len() == 1 => Some(Key::Char(s.chars().next().unwrap())),
            _ if upper.starts_with('F') => {
                let n: u8 = upper[1..].parse().ok()?;
                Some(Key::F(n))
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keystroke_char() {
        let k = Keystroke::char('a');
        assert_eq!(k.key, Key::Char('a'));
        assert!(k.modifiers.is_empty());
    }

    #[test]
    fn test_keystroke_ctrl() {
        let k = Keystroke::ctrl(Key::Char('d'));
        assert_eq!(k.key, Key::Char('d'));
        assert!(k.modifiers.ctrl);
        assert!(!k.modifiers.shift);
    }

    #[test]
    fn test_keystroke_from_key_event() {
        let event = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL);
        let k = Keystroke::from(&event);
        assert_eq!(k.key, Key::Char('x'));
        assert!(k.modifiers.ctrl);
    }

    #[test]
    fn test_key_sequence_prefix() {
        let seq1 = KeySequence::from_vec(vec![Keystroke::char('g')]);
        let seq2 = KeySequence::from_vec(vec![Keystroke::char('g'), Keystroke::char('g')]);
        assert!(seq1.is_prefix_of(&seq2));
        assert!(!seq2.is_prefix_of(&seq1));
    }

    #[test]
    fn test_uppercase_letter_ignores_shift_modifier() {
        // When pressing Shift+G, crossterm sends Char('G') with SHIFT modifier.
        // For uppercase letters, shift is redundant (already encoded in the character).
        // The keystroke should match keys!['G'] which has no modifiers.
        let event = KeyEvent::new(KeyCode::Char('G'), KeyModifiers::SHIFT);
        let k = Keystroke::from(&event);

        assert_eq!(k.key, Key::Char('G'));
        // Shift should NOT be set for uppercase letters
        assert!(!k.modifiers.shift, "Shift should be cleared for uppercase letters");
        assert!(k.modifiers.is_empty());

        // This should match the binding keys!['G']
        let binding = Keystroke::char('G');
        assert_eq!(k, binding);
    }

    #[test]
    fn test_ctrl_uppercase_preserves_ctrl() {
        // Ctrl+Shift+G should preserve ctrl but clear shift
        let event = KeyEvent::new(
            KeyCode::Char('G'),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        );
        let k = Keystroke::from(&event);

        assert_eq!(k.key, Key::Char('G'));
        assert!(k.modifiers.ctrl);
        assert!(!k.modifiers.shift, "Shift should be cleared for uppercase letters");
    }
}
