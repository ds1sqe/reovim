//! Macro recording and playback support (Epic #465 Phase 8D).
//!
//! This module provides functionality for vim-style macro recording:
//! - `q{a-z}` - Start recording to register
//! - `q` (while recording) - Stop recording
//! - `@{a-z}` - Play macro from register
//! - `@@` - Replay last played macro
//!
//! # Architecture
//!
//! Macros are stored as serialized key sequences in registers. The vim module
//! converts `Vec<KeyEvent>` to/from vim notation strings for storage.
//!
//! ```text
//! ┌──────────────┐     serialize      ┌──────────────┐
//! │ Vec<KeyEvent>│ ──────────────────▶│    String    │
//! │ (runtime)    │                    │ (register)   │
//! └──────────────┘                    └──────────────┘
//!        ▲                                   │
//!        │          deserialize              │
//!        └───────────────────────────────────┘
//! ```
//!
//! # Key Serialization
//!
//! Keys are serialized to vim notation:
//! - Plain characters: `a`, `b`, `x`
//! - Special keys: `<Esc>`, `<Enter>`, `<Tab>`
//! - Modifiers: `<C-w>`, `<A-x>`, `<S-Tab>`
//!
//! # Example
//!
//! ```ignore
//! use reovim_module_vim::macros::{key_to_notation, notation_to_keys};
//! use reovim_driver_input::{KeyEvent, KeyCode};
//!
//! // Serialize
//! let key = KeyEvent::new(KeyCode::Escape);
//! assert_eq!(key_to_notation(&key), "<Esc>");
//!
//! // Deserialize
//! let keys = notation_to_keys("<Esc>j").unwrap();
//! assert_eq!(keys.len(), 2);
//! ```

use reovim_driver_input::{KeyCode, KeyEvent, KeySequence, Modifiers};

/// Convert a single `KeyEvent` to vim notation string.
///
/// # Examples
///
/// - `KeyCode::Char('a')` → `"a"`
/// - `KeyCode::Escape` → `"<Esc>"`
/// - `KeyCode::Char('w')` + Ctrl → `"<C-w>"`
#[must_use]
pub fn key_to_notation(key: &KeyEvent) -> String {
    let has_ctrl = key.modifiers.contains(Modifiers::CTRL);
    let has_alt = key.modifiers.contains(Modifiers::ALT);
    let has_shift = key.modifiers.contains(Modifiers::SHIFT);
    let has_any_mod = has_ctrl || has_alt || has_shift;

    // Build modifier prefix
    let mut prefix = String::new();
    if has_ctrl {
        prefix.push_str("C-");
    }
    if has_alt {
        prefix.push_str("A-");
    }
    if has_shift {
        prefix.push_str("S-");
    }

    // Format based on key code
    match key.code {
        KeyCode::Char(c) => {
            if has_any_mod {
                format!("<{prefix}{c}>")
            } else if c == '<' {
                "<lt>".to_string()
            } else if c == '>' {
                "<gt>".to_string()
            } else {
                c.to_string()
            }
        }
        KeyCode::Escape => format!("<{prefix}Esc>"),
        KeyCode::Enter => format!("<{prefix}Enter>"),
        KeyCode::Tab => format!("<{prefix}Tab>"),
        KeyCode::Backspace => format!("<{prefix}BS>"),
        KeyCode::Delete => format!("<{prefix}Del>"),
        KeyCode::Up => format!("<{prefix}Up>"),
        KeyCode::Down => format!("<{prefix}Down>"),
        KeyCode::Left => format!("<{prefix}Left>"),
        KeyCode::Right => format!("<{prefix}Right>"),
        KeyCode::Home => format!("<{prefix}Home>"),
        KeyCode::End => format!("<{prefix}End>"),
        KeyCode::PageUp => format!("<{prefix}PageUp>"),
        KeyCode::PageDown => format!("<{prefix}PageDown>"),
        KeyCode::F(n) => format!("<{prefix}F{n}>"),
        KeyCode::BackTab => "<S-Tab>".to_string(),
        _ => "<?>".to_string(),
    }
}

/// Convert a slice of `KeyEvent`s to vim notation string.
///
/// # Example
///
/// ```ignore
/// let keys = [
///     KeyEvent::new(KeyCode::Char('d')),
///     KeyEvent::new(KeyCode::Char('w')),
/// ];
/// assert_eq!(keys_to_notation(&keys), "dw");
/// ```
#[must_use]
pub fn keys_to_notation(keys: &[KeyEvent]) -> String {
    keys.iter().map(key_to_notation).collect()
}

/// Parse vim notation string into `Vec<KeyEvent>`.
///
/// Uses the input driver's `KeySequence::parse` for consistent parsing.
///
/// # Errors
///
/// Returns `None` if the notation string is invalid.
///
/// # Example
///
/// ```ignore
/// let keys = notation_to_keys("dw<Esc>").unwrap();
/// assert_eq!(keys.len(), 3);
/// ```
#[must_use]
pub fn notation_to_keys(notation: &str) -> Option<Vec<KeyEvent>> {
    let seq = KeySequence::parse(notation)?;
    Some(seq.as_slice().to_vec())
}

/// Content stored in a register that represents a macro.
///
/// This wraps a `Vec<KeyEvent>` with convenience methods for
/// display and serialization.
#[derive(Debug, Clone, Default)]
pub struct MacroContent {
    /// The recorded key sequence.
    pub keys: Vec<KeyEvent>,
}

impl MacroContent {
    /// Create a new macro content from key events.
    #[must_use]
    pub const fn new(keys: Vec<KeyEvent>) -> Self {
        Self { keys }
    }

    /// Create an empty macro content.
    #[must_use]
    pub const fn empty() -> Self {
        Self { keys: Vec::new() }
    }

    /// Check if the macro is empty.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // Vec::is_empty is not const
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    /// Get the number of keys in the macro.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // Vec::len is not const
    pub fn len(&self) -> usize {
        self.keys.len()
    }

    /// Convert to vim notation string for display/storage.
    #[must_use]
    pub fn to_notation(&self) -> String {
        keys_to_notation(&self.keys)
    }

    /// Create from vim notation string.
    ///
    /// # Errors
    ///
    /// Returns `None` if the notation string is invalid.
    #[must_use]
    pub fn from_notation(notation: &str) -> Option<Self> {
        notation_to_keys(notation).map(Self::new)
    }
}

impl std::fmt::Display for MacroContent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_notation())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c))
    }

    fn key_with_mod(c: char, mods: Modifiers) -> KeyEvent {
        KeyEvent::with_modifiers(KeyCode::Char(c), mods)
    }

    // ========================================================================
    // key_to_notation tests
    // ========================================================================

    #[test]
    fn test_key_to_notation_char() {
        assert_eq!(key_to_notation(&key('a')), "a");
        assert_eq!(key_to_notation(&key('Z')), "Z");
        assert_eq!(key_to_notation(&key('5')), "5");
        assert_eq!(key_to_notation(&key(' ')), " ");
    }

    #[test]
    fn test_key_to_notation_special() {
        assert_eq!(key_to_notation(&KeyEvent::new(KeyCode::Escape)), "<Esc>");
        assert_eq!(key_to_notation(&KeyEvent::new(KeyCode::Enter)), "<Enter>");
        assert_eq!(key_to_notation(&KeyEvent::new(KeyCode::Tab)), "<Tab>");
        assert_eq!(key_to_notation(&KeyEvent::new(KeyCode::Backspace)), "<BS>");
        assert_eq!(key_to_notation(&KeyEvent::new(KeyCode::Delete)), "<Del>");
    }

    #[test]
    fn test_key_to_notation_arrows() {
        assert_eq!(key_to_notation(&KeyEvent::new(KeyCode::Up)), "<Up>");
        assert_eq!(key_to_notation(&KeyEvent::new(KeyCode::Down)), "<Down>");
        assert_eq!(key_to_notation(&KeyEvent::new(KeyCode::Left)), "<Left>");
        assert_eq!(key_to_notation(&KeyEvent::new(KeyCode::Right)), "<Right>");
    }

    #[test]
    fn test_key_to_notation_function_keys() {
        assert_eq!(key_to_notation(&KeyEvent::new(KeyCode::F(1))), "<F1>");
        assert_eq!(key_to_notation(&KeyEvent::new(KeyCode::F(12))), "<F12>");
    }

    #[test]
    fn test_key_to_notation_modifiers() {
        assert_eq!(key_to_notation(&key_with_mod('w', Modifiers::CTRL)), "<C-w>");
        assert_eq!(key_to_notation(&key_with_mod('x', Modifiers::ALT)), "<A-x>");
        assert_eq!(key_to_notation(&key_with_mod('a', Modifiers::SHIFT)), "<S-a>");
    }

    #[test]
    fn test_key_to_notation_combined_modifiers() {
        let mods = Modifiers::CTRL | Modifiers::ALT;
        assert_eq!(key_to_notation(&key_with_mod('d', mods)), "<C-A-d>");
    }

    #[test]
    fn test_key_to_notation_angle_brackets() {
        assert_eq!(key_to_notation(&key('<')), "<lt>");
        assert_eq!(key_to_notation(&key('>')), "<gt>");
    }

    // ========================================================================
    // keys_to_notation tests
    // ========================================================================

    #[test]
    fn test_keys_to_notation_empty() {
        assert_eq!(keys_to_notation(&[]), "");
    }

    #[test]
    fn test_keys_to_notation_simple() {
        let keys = [key('d'), key('w')];
        assert_eq!(keys_to_notation(&keys), "dw");
    }

    #[test]
    fn test_keys_to_notation_mixed() {
        let keys = [
            key('i'),
            key('h'),
            key('e'),
            key('l'),
            key('l'),
            key('o'),
            KeyEvent::new(KeyCode::Escape),
        ];
        assert_eq!(keys_to_notation(&keys), "ihello<Esc>");
    }

    // ========================================================================
    // notation_to_keys tests
    // ========================================================================

    #[test]
    fn test_notation_to_keys_simple() {
        let keys = notation_to_keys("dw").unwrap();
        assert_eq!(keys.len(), 2);
        assert_eq!(keys[0].code, KeyCode::Char('d'));
        assert_eq!(keys[1].code, KeyCode::Char('w'));
    }

    #[test]
    fn test_notation_to_keys_special() {
        let keys = notation_to_keys("<Esc>").unwrap();
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].code, KeyCode::Escape);
    }

    #[test]
    fn test_notation_to_keys_mixed() {
        let keys = notation_to_keys("ihello<Esc>").unwrap();
        assert_eq!(keys.len(), 7);
        assert_eq!(keys[0].code, KeyCode::Char('i'));
        assert_eq!(keys[6].code, KeyCode::Escape);
    }

    #[test]
    fn test_notation_to_keys_ctrl() {
        let keys = notation_to_keys("<C-w>h").unwrap();
        assert_eq!(keys.len(), 2);
        assert_eq!(keys[0].code, KeyCode::Char('w'));
        assert!(keys[0].modifiers.contains(Modifiers::CTRL));
        assert_eq!(keys[1].code, KeyCode::Char('h'));
    }

    #[test]
    fn test_notation_to_keys_empty() {
        assert!(notation_to_keys("").is_none());
    }

    // ========================================================================
    // MacroContent tests
    // ========================================================================

    #[test]
    fn test_macro_content_new() {
        let keys = vec![key('d'), key('w')];
        let macro_content = MacroContent::new(keys.clone());
        assert_eq!(macro_content.keys, keys);
    }

    #[test]
    fn test_macro_content_empty() {
        let macro_content = MacroContent::empty();
        assert!(macro_content.is_empty());
        assert_eq!(macro_content.len(), 0);
    }

    #[test]
    fn test_macro_content_to_notation() {
        let macro_content =
            MacroContent::new(vec![key('d'), key('w'), KeyEvent::new(KeyCode::Escape)]);
        assert_eq!(macro_content.to_notation(), "dw<Esc>");
    }

    #[test]
    fn test_macro_content_from_notation() {
        let macro_content = MacroContent::from_notation("dw<Esc>").unwrap();
        assert_eq!(macro_content.len(), 3);
        assert_eq!(macro_content.keys[0].code, KeyCode::Char('d'));
        assert_eq!(macro_content.keys[2].code, KeyCode::Escape);
    }

    #[test]
    fn test_macro_content_display() {
        let macro_content = MacroContent::new(vec![key('j'), key('j')]);
        assert_eq!(format!("{macro_content}"), "jj");
    }

    // ========================================================================
    // Roundtrip tests (serialize -> deserialize -> same)
    // ========================================================================

    #[test]
    fn test_roundtrip_simple() {
        let original = vec![key('d'), key('w')];
        let notation = keys_to_notation(&original);
        let parsed = notation_to_keys(&notation).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn test_roundtrip_with_special() {
        let original = vec![key('i'), key('h'), key('i'), KeyEvent::new(KeyCode::Escape)];
        let notation = keys_to_notation(&original);
        let parsed = notation_to_keys(&notation).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn test_roundtrip_with_ctrl() {
        let original = vec![key_with_mod('w', Modifiers::CTRL), key('h')];
        let notation = keys_to_notation(&original);
        let parsed = notation_to_keys(&notation).unwrap();

        assert_eq!(original.len(), parsed.len());
        assert_eq!(original[0].code, parsed[0].code);
        assert!(parsed[0].modifiers.contains(Modifiers::CTRL));
        assert_eq!(original[1].code, parsed[1].code);
    }
}
