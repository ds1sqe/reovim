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
//! use reovim_driver_text_input::{KeyEvent, KeyCode};
//!
//! // Serialize
//! let key = KeyEvent::new(KeyCode::Escape);
//! assert_eq!(key_to_notation(&key), "<Esc>");
//!
//! // Deserialize
//! let keys = notation_to_keys("<Esc>j").unwrap();
//! assert_eq!(keys.len(), 2);
//! ```

use reovim_driver_text_input::{KeyCode, KeyEvent, KeySequence, Modifiers};

/// Convert a single `KeyEvent` to vim notation string.
///
/// # Examples
///
/// - `KeyCode::Char('a')` → `"a"`
/// - `KeyCode::Escape` → `"<Esc>"`
/// - `KeyCode::Char('w')` + Ctrl → `"<C-w>"`
#[must_use]
#[cfg_attr(coverage_nightly, coverage(off))]
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
