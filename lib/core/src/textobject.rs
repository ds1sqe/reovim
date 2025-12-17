//! Text object types for operator commands (di(, da{, etc.)

/// Delimiter types for text objects
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delimiter {
    /// Parentheses ()
    Paren,
    /// Square brackets []
    Bracket,
    /// Curly braces {}
    Brace,
    /// Angle brackets <>
    Angle,
    /// Double quotes ""
    DoubleQuote,
    /// Single quotes ''
    SingleQuote,
    /// Backticks
    Backtick,
}

impl Delimiter {
    /// Get the opening and closing characters for this delimiter
    #[must_use]
    pub const fn chars(&self) -> (char, char) {
        match self {
            Self::Paren => ('(', ')'),
            Self::Bracket => ('[', ']'),
            Self::Brace => ('{', '}'),
            Self::Angle => ('<', '>'),
            Self::DoubleQuote => ('"', '"'),
            Self::SingleQuote => ('\'', '\''),
            Self::Backtick => ('`', '`'),
        }
    }

    /// Check if this delimiter has symmetric open/close chars (quotes)
    #[must_use]
    pub const fn is_symmetric(&self) -> bool {
        matches!(
            self,
            Self::DoubleQuote | Self::SingleQuote | Self::Backtick
        )
    }

    /// Parse a character into a delimiter
    #[must_use]
    pub const fn from_char(c: char) -> Option<Self> {
        match c {
            '(' | ')' => Some(Self::Paren),
            '[' | ']' => Some(Self::Bracket),
            '{' | '}' => Some(Self::Brace),
            '<' | '>' => Some(Self::Angle),
            '"' => Some(Self::DoubleQuote),
            '\'' => Some(Self::SingleQuote),
            '`' => Some(Self::Backtick),
            _ => None,
        }
    }
}

/// Text object scope (inner vs around)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextObjectScope {
    /// Inner - contents only, excludes delimiters (`di(`)
    Inner,
    /// Around - includes delimiters (`da(`)
    Around,
}

/// A complete text object specification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextObject {
    pub scope: TextObjectScope,
    pub delimiter: Delimiter,
}

impl TextObject {
    /// Create a new text object
    #[must_use]
    pub const fn new(scope: TextObjectScope, delimiter: Delimiter) -> Self {
        Self { scope, delimiter }
    }

    /// Parse a two-character sequence into a text object (e.g., "i(", "a{")
    #[must_use]
    pub fn from_keys(scope_char: char, delim_char: char) -> Option<Self> {
        let scope = match scope_char {
            'i' => TextObjectScope::Inner,
            'a' => TextObjectScope::Around,
            _ => return None,
        };

        let delimiter = Delimiter::from_char(delim_char)?;
        Some(Self::new(scope, delimiter))
    }
}
