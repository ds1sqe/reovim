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

use {
    reovim_codec_tui_input::{KeyCode, KeyEvent, KeyEventKind, Modifiers},
    reovim_driver_text_input::KeySequence,
};

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
/// Parses the notation using `KeySequence::parse` for token splitting, then
/// converts each token to a `KeyEvent` using `reovim_codec_tui_input` types.
/// No codec registry is required.
///
/// # Errors
///
/// Returns `None` if the notation string is invalid or contains an unknown token.
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
    seq.as_slice()
        .iter()
        .map(|token| notation_token_to_key_event(token))
        .collect()
}

/// Convert a single vim-notation token (as produced by `KeySequence::parse`) to
/// a `KeyEvent`.  Returns `None` for unrecognised tokens.
fn notation_token_to_key_event(token: &str) -> Option<KeyEvent> {
    // Single printable character (no angle brackets)
    if token.chars().count() == 1 {
        let c = token.chars().next()?;
        return Some(KeyEvent::new(KeyCode::Char(c)));
    }

    // Literal `<lt>` / `<gt>` — produced by KeySequence for '<' and '>'
    if token == "<lt>" {
        return Some(KeyEvent::new(KeyCode::Char('<')));
    }
    if token == "<gt>" {
        return Some(KeyEvent::new(KeyCode::Char('>')));
    }

    // Angle-bracket notation: `<Esc>`, `<C-w>`, `<S-Tab>`, etc.
    if let Some(inner) = token.strip_prefix('<').and_then(|s| s.strip_suffix('>')) {
        return parse_bracket_notation(inner);
    }

    None
}

/// Parse the inner content of an `<...>` token into a `KeyEvent`.
fn parse_bracket_notation(inner: &str) -> Option<KeyEvent> {
    let mut remaining = inner;
    let mut mods = Modifiers::NONE;

    loop {
        if let Some(rest) = remaining.strip_prefix("C-") {
            mods |= Modifiers::CTRL;
            remaining = rest;
        } else if let Some(rest) = remaining.strip_prefix("A-") {
            mods |= Modifiers::ALT;
            remaining = rest;
        } else if let Some(rest) = remaining.strip_prefix("S-") {
            mods |= Modifiers::SHIFT;
            remaining = rest;
        } else {
            break;
        }
    }

    let code = match remaining {
        "Esc" => KeyCode::Escape,
        "Enter" => KeyCode::Enter,
        "Tab" => KeyCode::Tab,
        "BS" => KeyCode::Backspace,
        "Del" => KeyCode::Delete,
        "Up" => KeyCode::Up,
        "Down" => KeyCode::Down,
        "Left" => KeyCode::Left,
        "Right" => KeyCode::Right,
        "Home" => KeyCode::Home,
        "End" => KeyCode::End,
        "PageUp" => KeyCode::PageUp,
        "PageDown" => KeyCode::PageDown,
        "Insert" => KeyCode::Insert,
        "F1" => KeyCode::F(1),
        "F2" => KeyCode::F(2),
        "F3" => KeyCode::F(3),
        "F4" => KeyCode::F(4),
        "F5" => KeyCode::F(5),
        "F6" => KeyCode::F(6),
        "F7" => KeyCode::F(7),
        "F8" => KeyCode::F(8),
        "F9" => KeyCode::F(9),
        "F10" => KeyCode::F(10),
        "F11" => KeyCode::F(11),
        "F12" => KeyCode::F(12),
        // Single char after modifiers
        c if c.chars().count() == 1 => KeyCode::Char(c.chars().next()?),
        _ => return None,
    };

    Some(KeyEvent::full(code, mods, KeyEventKind::Press))
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
