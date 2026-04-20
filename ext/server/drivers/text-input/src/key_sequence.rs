//! Key sequence and token conversion for the text-input driver.
//!
//! `KeySequence` and `ToKeyToken` moved here from `reovim-subsys-input-contracts`
//! as part of the Plan-14 I.6 reshape.  These types are text-input-specific:
//! they carry vim-notation string tokens and are not domain-neutral.
//!
//! Domain-neutral lookup primitives (`LookupState<C>`, `LookupResult<C>`, etc.)
//! live in `reovim-subsys-input`.

use std::fmt;

/// Conversion hook for key-sequence tokens.
///
/// Contract crates stay untyped; typed input crates implement this trait to
/// adapt their own key representations into canonical notation tokens.
pub trait ToKeyToken {
    /// Convert the value into a canonical key token string.
    fn to_key_token(&self) -> String;
}

impl ToKeyToken for String {
    fn to_key_token(&self) -> String {
        self.clone()
    }
}

impl ToKeyToken for &str {
    fn to_key_token(&self) -> String {
        (*self).to_owned()
    }
}

impl ToKeyToken for reovim_codec_tui_input::KeyEvent {
    fn to_key_token(&self) -> String {
        use reovim_codec_tui_input::{KeyCode, Modifiers};

        let has_mod = self.modifiers != Modifiers::NONE;

        // Convert the base key code to its canonical notation name.
        let base = match &self.code {
            KeyCode::Char(c) => {
                if !has_mod {
                    return c.to_string();
                }
                c.to_string()
            }
            KeyCode::Escape => "Esc".to_owned(),
            KeyCode::Enter => "Enter".to_owned(),
            KeyCode::Tab => "Tab".to_owned(),
            KeyCode::BackTab => "S-Tab".to_owned(),
            KeyCode::Backspace => "BS".to_owned(),
            KeyCode::Delete => "Del".to_owned(),
            KeyCode::Insert => "Insert".to_owned(),
            KeyCode::Up => "Up".to_owned(),
            KeyCode::Down => "Down".to_owned(),
            KeyCode::Left => "Left".to_owned(),
            KeyCode::Right => "Right".to_owned(),
            KeyCode::Home => "Home".to_owned(),
            KeyCode::End => "End".to_owned(),
            KeyCode::PageUp => "PageUp".to_owned(),
            KeyCode::PageDown => "PageDown".to_owned(),
            KeyCode::F(n) => format!("F{n}"),
            KeyCode::Null => "@".to_owned(),
            KeyCode::CapsLock => "CapsLock".to_owned(),
            KeyCode::ScrollLock => "ScrollLock".to_owned(),
            KeyCode::NumLock => "NumLock".to_owned(),
            KeyCode::PrintScreen => "PrintScreen".to_owned(),
            KeyCode::Pause => "Pause".to_owned(),
            KeyCode::Menu => "Menu".to_owned(),
            KeyCode::KeypadBegin => "KP5".to_owned(),
            KeyCode::MediaPlay => "MediaPlay".to_owned(),
            KeyCode::MediaPause => "MediaPause".to_owned(),
            KeyCode::MediaPlayPause => "MediaPlayPause".to_owned(),
            KeyCode::MediaStop => "MediaStop".to_owned(),
            KeyCode::MediaReverse => "MediaReverse".to_owned(),
            KeyCode::MediaFastForward => "MediaFastForward".to_owned(),
            KeyCode::MediaRewind => "MediaRewind".to_owned(),
            KeyCode::MediaNext => "MediaNext".to_owned(),
            KeyCode::MediaPrevious => "MediaPrevious".to_owned(),
            KeyCode::MediaRecord => "MediaRecord".to_owned(),
            KeyCode::MediaLowerVolume => "MediaLowerVolume".to_owned(),
            KeyCode::MediaRaiseVolume => "MediaRaiseVolume".to_owned(),
            KeyCode::MediaMuteVolume => "MediaMuteVolume".to_owned(),
            KeyCode::LeftShift => "LeftShift".to_owned(),
            KeyCode::RightShift => "RightShift".to_owned(),
            KeyCode::LeftCtrl => "LeftCtrl".to_owned(),
            KeyCode::RightCtrl => "RightCtrl".to_owned(),
            KeyCode::LeftAlt => "LeftAlt".to_owned(),
            KeyCode::RightAlt => "RightAlt".to_owned(),
            KeyCode::LeftSuper => "LeftSuper".to_owned(),
            KeyCode::RightSuper => "RightSuper".to_owned(),
            KeyCode::LeftHyper => "LeftHyper".to_owned(),
            KeyCode::RightHyper => "RightHyper".to_owned(),
            KeyCode::LeftMeta => "LeftMeta".to_owned(),
            KeyCode::RightMeta => "RightMeta".to_owned(),
            KeyCode::IsoLevel3Shift => "IsoLevel3Shift".to_owned(),
            KeyCode::IsoLevel5Shift => "IsoLevel5Shift".to_owned(),
        };

        if !has_mod {
            // No modifiers: wrap in angle brackets for special keys.
            return match &self.code {
                KeyCode::Char(_) => base,
                _ => format!("<{base}>"),
            };
        }

        // With modifiers: always use angle-bracket form.
        let mut token = String::from("<");
        if self.modifiers.contains(Modifiers::CTRL) {
            token.push_str("C-");
        }
        if self.modifiers.contains(Modifiers::ALT) {
            token.push_str("A-");
        }
        if self.modifiers.contains(Modifiers::SHIFT) {
            token.push_str("S-");
        }
        token.push_str(&base);
        token.push('>');
        token
    }
}

/// Driver-owned sequence of canonical key-notation tokens.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct KeySequence(Vec<String>);

impl KeySequence {
    /// Create a new empty key sequence.
    #[must_use]
    pub const fn new() -> Self {
        Self(Vec::new())
    }

    /// Create a key sequence from canonical token producers.
    #[must_use]
    pub fn from_keys<T: ToKeyToken>(keys: &[T]) -> Self {
        Self(keys.iter().map(ToKeyToken::to_key_token).collect())
    }

    /// Push a token-producing value into the sequence.
    pub fn push<T: ToKeyToken>(&mut self, key: T) {
        self.0.push(key.to_key_token());
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

    /// Get the number of tokens in the sequence.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.0.len()
    }

    /// Borrow the canonical tokens.
    #[must_use]
    pub fn as_slice(&self) -> &[String] {
        &self.0
    }

    /// Borrow the canonical tokens (alias for `as_slice`).
    #[must_use]
    pub fn keys(&self) -> &[String] {
        &self.0
    }

    /// Render the sequence back to a single notation string.
    #[must_use]
    pub fn as_string(&self) -> String {
        self.0.concat()
    }

    /// Check if this sequence starts with another sequence.
    #[must_use]
    pub fn starts_with(&self, prefix: &Self) -> bool {
        self.0.starts_with(&prefix.0)
    }

    /// Parse canonical key notation into normalized tokens.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        if s.is_empty() {
            return None;
        }

        let mut tokens = Vec::new();
        let mut chars = s.chars().peekable();

        while let Some(c) = chars.next() {
            if c == '<' {
                let mut spec = String::new();
                let mut closed = false;
                for next in chars.by_ref() {
                    if next == '>' {
                        closed = true;
                        break;
                    }
                    spec.push(next);
                }

                if !closed {
                    return None;
                }

                tokens.push(parse_special(&spec)?);
            } else {
                tokens.push(c.to_string());
            }
        }

        Some(Self(tokens))
    }
}

impl fmt::Display for KeySequence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.as_string())
    }
}

fn parse_special(spec: &str) -> Option<String> {
    let mut remaining = spec;
    let mut ctrl = false;
    let mut alt = false;
    let mut shift = false;

    loop {
        if let Some(rest) = remaining.strip_prefix("C-") {
            ctrl = true;
            remaining = rest;
        } else if let Some(rest) = remaining.strip_prefix("A-") {
            alt = true;
            remaining = rest;
        } else if let Some(rest) = remaining.strip_prefix("M-") {
            alt = true;
            remaining = rest;
        } else if let Some(rest) = remaining.strip_prefix("S-") {
            shift = true;
            remaining = rest;
        } else {
            break;
        }
    }

    let lowered = remaining.to_lowercase();
    let base = match lowered.as_str() {
        "esc" | "escape" => "Esc".to_owned(),
        "enter" | "cr" | "return" => "Enter".to_owned(),
        "tab" => "Tab".to_owned(),
        "space" => "Space".to_owned(),
        "bs" | "backspace" => "BS".to_owned(),
        "del" | "delete" => "Del".to_owned(),
        "up" => "Up".to_owned(),
        "down" => "Down".to_owned(),
        "left" => "Left".to_owned(),
        "right" => "Right".to_owned(),
        "home" => "Home".to_owned(),
        "end" => "End".to_owned(),
        "pageup" => "PageUp".to_owned(),
        "pagedown" => "PageDown".to_owned(),
        "lt" => "lt".to_owned(),
        "gt" => "gt".to_owned(),
        _ => {
            if let Some(function) = parse_function_key(&lowered) {
                function
            } else if lowered.chars().count() == 1 {
                lowered
            } else {
                return None;
            }
        }
    };

    let has_modifiers = ctrl || alt || shift;
    if !has_modifiers {
        return Some(match base.as_str() {
            "Space" => " ".to_owned(),
            "lt" => "<lt>".to_owned(),
            "gt" => "<gt>".to_owned(),
            _ if base.chars().count() == 1 => base,
            _ => format!("<{base}>"),
        });
    }

    let mut token = String::from("<");
    if ctrl {
        token.push_str("C-");
    }
    if alt {
        token.push_str("A-");
    }
    if shift {
        token.push_str("S-");
    }

    token.push_str(&base);
    token.push('>');
    Some(token)
}

fn parse_function_key(spec: &str) -> Option<String> {
    let digits = spec.strip_prefix('f')?;
    let value = digits.parse::<u8>().ok()?;
    if (1..=12).contains(&value) {
        Some(format!("F{value}"))
    } else {
        None
    }
}

