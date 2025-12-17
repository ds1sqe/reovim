//! Text object types for operator commands (di(, da{, daf, dic, diw, etc.)
//!
//! Supports:
//! - Delimiter-based text objects (di(, da{)
//! - Word text objects (iw, aw, iW, aW)
//! - Semantic text objects based on treesitter (daf, dic)

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

/// Word types for word text objects
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WordType {
    /// Small word - alphanumeric + underscore (iw/aw)
    /// Sequence of [a-zA-Z0-9_] or sequence of other non-whitespace
    Word,
    /// Big WORD - any non-whitespace characters (iW/aW)
    /// Sequence of non-whitespace characters
    BigWord,
}

impl WordType {
    /// Check if a character is part of a word (for small word)
    #[must_use]
    pub const fn is_word_char(c: char) -> bool {
        c.is_ascii_alphanumeric() || c == '_'
    }

    /// Check if a character matches this word type
    #[must_use]
    pub const fn matches(&self, c: char) -> bool {
        match self {
            Self::Word => Self::is_word_char(c),
            Self::BigWord => !c.is_whitespace(),
        }
    }

    /// Parse a character into a word type
    #[must_use]
    pub const fn from_char(c: char) -> Option<Self> {
        match c {
            'w' => Some(Self::Word),
            'W' => Some(Self::BigWord),
            _ => None,
        }
    }
}

/// Word text object specification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WordTextObject {
    pub scope: TextObjectScope,
    pub word_type: WordType,
}

impl WordTextObject {
    /// Create a new word text object
    #[must_use]
    pub const fn new(scope: TextObjectScope, word_type: WordType) -> Self {
        Self { scope, word_type }
    }

    /// Parse a two-character sequence into a word text object (e.g., "iw", "aW")
    #[must_use]
    pub const fn from_keys(scope_char: char, word_char: char) -> Option<Self> {
        let scope = match scope_char {
            'i' => TextObjectScope::Inner,
            'a' => TextObjectScope::Around,
            _ => return None,
        };

        let Some(word_type) = WordType::from_char(word_char) else {
            return None;
        };

        Some(Self::new(scope, word_type))
    }
}

/// Semantic text object types based on treesitter AST
///
/// These text objects use treesitter queries to find language constructs
/// like functions, classes, parameters, etc.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemanticTextObject {
    /// Function body/definition (key: 'f')
    /// `daf` = delete around function, `dif` = delete inner function
    Function,
    /// Class/struct/impl block (key: 'c')
    /// `dac` = delete around class, `dic` = delete inner class
    Class,
    /// Function parameter/argument (key: 'a')
    /// `daa` = delete around argument, `dia` = delete inner argument
    Parameter,
    /// Conditional block - if/match/switch (key: 'o' for "conditional")
    /// `dao` = delete around conditional, `dio` = delete inner conditional
    Conditional,
    /// Loop block - for/while/loop (key: 'l')
    /// `dal` = delete around loop, `dil` = delete inner loop
    Loop,
    /// Comment (key: '/')
    /// `da/` = delete around comment, `di/` = delete inner comment
    Comment,
    /// Generic block (key: 'B' - capital B to differentiate from 'b' for brackets in some contexts)
    /// `daB` = delete around block, `diB` = delete inner block
    Block,
}

impl SemanticTextObject {
    /// Parse a character into a semantic text object
    #[must_use]
    pub const fn from_char(c: char) -> Option<Self> {
        match c {
            'f' => Some(Self::Function),
            'c' => Some(Self::Class),
            'a' => Some(Self::Parameter),
            'o' => Some(Self::Conditional),
            'l' => Some(Self::Loop),
            '/' => Some(Self::Comment),
            'B' => Some(Self::Block),
            _ => None,
        }
    }

    /// Get the query capture name for this text object
    #[must_use]
    pub const fn capture_name(&self) -> &'static str {
        match self {
            Self::Function => "function",
            Self::Class => "class",
            Self::Parameter => "parameter",
            Self::Conditional => "conditional",
            Self::Loop => "loop",
            Self::Comment => "comment",
            Self::Block => "block",
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

/// A semantic text object specification (treesitter-based)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SemanticTextObjectSpec {
    pub scope: TextObjectScope,
    pub kind: SemanticTextObject,
}

impl SemanticTextObjectSpec {
    /// Create a new semantic text object specification
    #[must_use]
    pub const fn new(scope: TextObjectScope, kind: SemanticTextObject) -> Self {
        Self { scope, kind }
    }

    /// Parse a two-character sequence into a semantic text object (e.g., "if", "ac")
    #[must_use]
    pub fn from_keys(scope_char: char, kind_char: char) -> Option<Self> {
        let scope = match scope_char {
            'i' => TextObjectScope::Inner,
            'a' => TextObjectScope::Around,
            _ => return None,
        };

        let kind = SemanticTextObject::from_char(kind_char)?;
        Some(Self::new(scope, kind))
    }

    /// Get the full capture name for the query (e.g., "function.inner", "class.outer")
    #[must_use]
    pub fn capture_name(&self) -> String {
        let scope_str = match self.scope {
            TextObjectScope::Inner => "inner",
            TextObjectScope::Around => "outer",
        };
        format!("{}.{}", self.kind.capture_name(), scope_str)
    }
}

/// Unified text object kind - delimiter-based, word-based, or semantic
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextObjectKind {
    /// Delimiter-based text object (di(, da{)
    Delimiter(TextObject),
    /// Word-based text object (iw, aw, iW, aW)
    Word(WordTextObject),
    /// Semantic text object based on treesitter (daf, dic)
    Semantic(SemanticTextObjectSpec),
}

impl TextObjectKind {
    /// Try to parse a two-character sequence into any text object
    ///
    /// Priority: delimiter -> word -> semantic
    #[must_use]
    pub fn from_keys(scope_char: char, kind_char: char) -> Option<Self> {
        // Try delimiter first
        if let Some(text_obj) = TextObject::from_keys(scope_char, kind_char) {
            return Some(Self::Delimiter(text_obj));
        }

        // Try word text objects (iw, aw, iW, aW)
        if let Some(word_obj) = WordTextObject::from_keys(scope_char, kind_char) {
            return Some(Self::Word(word_obj));
        }

        // Try semantic
        if let Some(semantic) = SemanticTextObjectSpec::from_keys(scope_char, kind_char) {
            return Some(Self::Semantic(semantic));
        }

        None
    }

    /// Get the scope of this text object
    #[must_use]
    pub const fn scope(&self) -> TextObjectScope {
        match self {
            Self::Delimiter(obj) => obj.scope,
            Self::Word(obj) => obj.scope,
            Self::Semantic(obj) => obj.scope,
        }
    }
}
