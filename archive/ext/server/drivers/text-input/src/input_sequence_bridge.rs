//! Bridge between `KeySequence` (notation-token) and `InputSequence` (opaque bytes).
//!
//! This helper converts a text-input `KeySequence` to a `reovim-subsys-input`
//! `InputSequence` using the TUI key codec registered with an `InputCodecRegistry`.
//!
//! # Responsibility boundary
//!
//! This function is a bridge that operates at the driver layer — it is NOT part
//! of the subsys contract or the server registry.  The server registry works
//! exclusively with opaque `InputSequence`; this helper is for modules that
//! still hold `KeySequence` values (e.g., when registering keybindings declared
//! in vim notation).

use {
    reovim_codec_tui_input::{KeyEvent, encode_key_event},
    reovim_subsys_input::{InputCodecRegistry, InputPayloadError, InputSequence},
};

use crate::KeySequence;

/// Error returned by `key_sequence_to_input_sequence`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum InputSequenceBridgeError {
    /// The `InputCodecRegistry` does not have a codec for kind 0x0001.
    TuiKeyCodecNotRegistered,
    /// A payload could not be encoded.
    EncodeError(InputPayloadError),
    /// A notation token could not be converted to a `KeyEvent`.
    UnknownToken(String),
}

impl std::fmt::Display for InputSequenceBridgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TuiKeyCodecNotRegistered => {
                write!(f, "TUI key codec (kind=0x0001) is not registered")
            }
            Self::EncodeError(e) => write!(f, "encode error: {e}"),
            Self::UnknownToken(t) => write!(f, "unknown key token: {t:?}"),
        }
    }
}

impl std::error::Error for InputSequenceBridgeError {}

/// Convert a `KeySequence` to an `InputSequence` via the TUI key codec.
///
/// Looks up `TuiKeyCodec` (kind=0x0001) in `registry`.  For each notation
/// token in `ks`, constructs a `KeyEvent` and encodes it via the codec.
///
/// # Errors
///
/// - `TuiKeyCodecNotRegistered` — codec not registered (module not loaded).
/// - `UnknownToken` — a notation token has no canonical `KeyEvent` mapping.
/// - `EncodeError` — codec reported an encoding failure.
///
/// # Notes
///
/// This is a best-effort conversion: vim notation is richer than the binary
/// codec's key codes.  Tokens that cannot be mapped return `UnknownToken`.
/// During Phase I the mapping covers only the tokens used by in-tree modules.
pub fn key_sequence_to_input_sequence(
    ks: &KeySequence,
    registry: &dyn InputCodecRegistry,
) -> Result<InputSequence, InputSequenceBridgeError> {
    // Verify TUI key codec is loaded (we don't need it for type-safe conversion
    // since encode_key_event is a free function, but this guards against
    // mis-configuration early).
    // Verify TUI key codec is loaded — guards against mis-configuration early.
    registry
        .get(reovim_codec_tui_input::KIND_KEY)
        .ok_or(InputSequenceBridgeError::TuiKeyCodecNotRegistered)?;

    let mut seq = InputSequence::new();
    for token in ks.as_slice() {
        let event = notation_to_key_event(token)
            .ok_or_else(|| InputSequenceBridgeError::UnknownToken(token.clone()))?;
        let payload = encode_key_event(&event);
        seq.push(payload);
    }
    Ok(seq)
}

/// Attempt to map a vim-notation token to a `KeyEvent`.
///
/// This covers the canonical tokens produced by `KeySequence::parse()`.
/// Returns `None` for tokens not yet mapped (will be extended as needed).
fn notation_to_key_event(token: &str) -> Option<KeyEvent> {
    use reovim_codec_tui_input::KeyCode;

    // Single printable character
    if token.chars().count() == 1 {
        let c = token.chars().next()?;
        return Some(KeyEvent::new(KeyCode::Char(c)));
    }

    // Angle-bracket notation
    if let Some(inner) = token.strip_prefix('<').and_then(|s| s.strip_suffix('>')) {
        return parse_bracket_notation(inner);
    }

    // Literal less-than stored as "<lt>" — handled as Char('<').
    if token == "<lt>" {
        return Some(KeyEvent::new(KeyCode::Char('<')));
    }
    if token == "<gt>" {
        return Some(KeyEvent::new(KeyCode::Char('>')));
    }

    None
}

fn parse_bracket_notation(inner: &str) -> Option<KeyEvent> {
    use reovim_codec_tui_input::{KeyCode, KeyEventKind, Modifiers};

    let mut remaining = inner;
    let mut mods = Modifiers::NONE;

    // Parse modifier prefixes.
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
