use std::fmt;

/// Conversion hook for contract-owned key-sequence tokens.
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

/// Driver-free sequence of canonical key-notation tokens.
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

    /// Borrow the canonical tokens.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_sequence_basics_and_prefixes() {
        let mut seq = KeySequence::new();
        assert!(seq.is_empty());
        assert_eq!(seq.len(), 0);

        seq.push("g");
        seq.push("g");
        assert_eq!(seq.as_slice(), ["g".to_owned(), "g".to_owned()].as_slice());
        assert_eq!(seq.keys(), ["g".to_owned(), "g".to_owned()].as_slice());
        assert_eq!(seq.as_string(), "gg");
        assert_eq!(format!("{seq}"), "gg");

        let prefix = KeySequence::from_keys(&["g"]);
        let other = KeySequence::from_keys(&["d"]);
        assert!(seq.starts_with(&prefix));
        assert!(!seq.starts_with(&other));

        seq.clear();
        assert!(seq.is_empty());
    }

    #[test]
    fn parse_normalizes_plain_special_and_modified_tokens() {
        assert_eq!(KeySequence::parse("gg").unwrap().as_slice(), ["g", "g"]);
        assert_eq!(KeySequence::parse("<Esc>").unwrap().as_slice(), ["<Esc>"]);
        assert_eq!(KeySequence::parse("<Escape>").unwrap().as_slice(), ["<Esc>"]);
        assert_eq!(KeySequence::parse("<CR>").unwrap().as_slice(), ["<Enter>"]);
        assert_eq!(KeySequence::parse("<Space>").unwrap().as_slice(), [" "]);
        assert_eq!(KeySequence::parse("<lt>").unwrap().as_slice(), ["<lt>"]);
        assert_eq!(KeySequence::parse("<gt>").unwrap().as_slice(), ["<gt>"]);
        assert_eq!(KeySequence::parse("<C-w>h").unwrap().as_slice(), ["<C-w>", "h"]);
        assert_eq!(KeySequence::parse("<M-x>").unwrap().as_slice(), ["<A-x>"]);
        assert_eq!(KeySequence::parse("<S-Tab>").unwrap().as_slice(), ["<S-Tab>"]);
        assert_eq!(KeySequence::parse("<C-A-S-x>").unwrap().as_slice(), ["<C-A-S-x>"]);
        assert_eq!(KeySequence::parse("🎉").unwrap().as_slice(), ["🎉"]);
        assert_eq!(KeySequence::parse("<F12>").unwrap().as_slice(), ["<F12>"]);
    }

    #[test]
    fn parse_rejects_invalid_notation() {
        assert!(KeySequence::parse("").is_none());
        assert!(KeySequence::parse("<Ctrl").is_none());
        assert!(KeySequence::parse("<Unknown>").is_none());
        assert!(KeySequence::parse("<F13>").is_none());
    }

    #[test]
    fn string_inputs_are_accepted_via_to_key_token() {
        let owned = String::from("<C-w>");
        let borrowed = "h";
        let seq = KeySequence::from_keys(&[owned.as_str(), borrowed]);
        assert_eq!(seq.as_string(), "<C-w>h");
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn debug_mentions_type_name() {
        assert!(format!("{:?}", KeySequence::default()).contains("KeySequence"));
    }
}
