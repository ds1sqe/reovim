//! Syntax highlight categories and spans.
//!
//! This module defines the highlight types used by syntax drivers.
//! The [`HighlightGroup`] enum implements [`SyntaxHighlight`] trait
//! to provide category names for theme mapping.

use std::{fmt::Debug, hash::Hash, ops::Range};

// ============================================================================
// SyntaxHighlight Trait
// ============================================================================

/// Trait for syntax highlight categories.
///
/// This trait defines the interface for highlight types. Types implementing
/// this trait can be used by the syntax highlighting system.
///
/// # Example
///
/// ```
/// use reovim_driver_syntax::{SyntaxHighlight, HighlightGroup};
///
/// let group = HighlightGroup::Keyword;
/// assert_eq!(group.category(), "keyword");
/// ```
pub trait SyntaxHighlight: Debug + Copy + Eq + Hash + Send + Sync + 'static {
    /// Returns the category name for this highlight.
    ///
    /// Category names should be lowercase, dot-separated identifiers
    /// (e.g., "keyword", "function.builtin", "string.escape").
    fn category(&self) -> &'static str;
}

/// Syntax highlight categories.
///
/// This enum defines all supported highlight groups for syntax highlighting.
/// It implements [`SyntaxHighlight`] trait to provide category names that
/// themes can use for color mapping.
///
/// Uses `#[repr(u8)]` for compact storage.
///
/// # Example
///
/// ```
/// use reovim_driver_syntax::{HighlightGroup, SyntaxHighlight};
///
/// let group = HighlightGroup::Keyword;
/// assert_eq!(group.category(), "keyword");
/// assert!(group.is_keyword());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum HighlightGroup {
    // === Keywords (0-4) ===
    /// Generic keyword (if, else, for, while, return, etc.)
    Keyword = 0,
    /// Control flow keywords (if, else, match, loop)
    KeywordControl = 1,
    /// Operator keywords (and, or, not, in)
    KeywordOperator = 2,
    /// Function-related keywords (fn, def, function)
    KeywordFunction = 3,
    /// Type-related keywords (struct, class, enum, type)
    KeywordType = 4,

    // === Types (5-6) ===
    /// User-defined types
    Type = 5,
    /// Built-in types (int, str, bool)
    TypeBuiltin = 6,

    // === Functions (7-10) ===
    /// Function names
    Function = 7,
    /// Built-in functions (print, len)
    FunctionBuiltin = 8,
    /// Macro invocations
    FunctionMacro = 9,
    /// Method calls
    Method = 10,

    // === Variables (11-15) ===
    /// Generic variable
    Variable = 11,
    /// Built-in variables (self, super, this)
    VariableBuiltin = 12,
    /// Function parameters
    Parameter = 13,
    /// Struct/class fields
    Field = 14,
    /// Constants
    Constant = 15,

    // === Literals (16-20) ===
    /// String literals
    String = 16,
    /// Escape sequences in strings
    StringEscape = 17,
    /// Character literals
    Character = 18,
    /// Numeric literals
    Number = 19,
    /// Boolean literals (true, false)
    Boolean = 20,

    // === Comments (21-22) ===
    /// Regular comments
    Comment = 21,
    /// Documentation comments
    CommentDoc = 22,

    // === Punctuation (23-25) ===
    /// Generic punctuation
    Punctuation = 23,
    /// Brackets, braces, parentheses
    PunctuationBracket = 24,
    /// Commas, semicolons, colons
    PunctuationDelimiter = 25,

    // === Operators (26) ===
    /// Operators (+, -, *, /, =, etc.)
    Operator = 26,

    // === Diagnostics (27-30) ===
    /// Error highlights
    Error = 27,
    /// Warning highlights
    Warning = 28,
    /// Info highlights
    Info = 29,
    /// Hint highlights
    Hint = 30,

    // === Namespace/Module (31) ===
    /// Namespace or module names
    Namespace = 31,

    // === Constructor (32) ===
    /// Constructor calls/definitions
    Constructor = 32,

    // === Label (33) ===
    /// Labels (lifetimes in Rust, goto labels)
    Label = 33,

    // === Attribute (34) ===
    /// Attributes, decorators, annotations
    Attribute = 34,

    // === Tag (35) ===
    /// HTML/XML tags
    Tag = 35,

    // === Markup (36-44) ===
    /// Markup headings (# in markdown)
    MarkupHeading = 36,
    /// Bold text
    MarkupBold = 37,
    /// Italic text
    MarkupItalic = 38,
    /// Strikethrough text
    MarkupStrikethrough = 39,
    /// Links
    MarkupLink = 40,
    /// Link URLs
    MarkupLinkUrl = 41,
    /// List markers
    MarkupList = 42,
    /// Raw/code blocks
    MarkupRaw = 43,
    /// Inline code
    MarkupRawInline = 44,

    // === Embedded/Injection (45-46) ===
    /// Embedded language regions (injections)
    Embedded = 45,
    /// Special tokens
    Special = 46,

    // === Custom (255) ===
    /// Custom highlight (for extensions)
    Custom = 255,
}

impl SyntaxHighlight for HighlightGroup {
    fn category(&self) -> &'static str {
        match self {
            Self::Keyword => "keyword",
            Self::KeywordControl => "keyword.control",
            Self::KeywordOperator => "keyword.operator",
            Self::KeywordFunction => "keyword.function",
            Self::KeywordType => "keyword.type",
            Self::Type => "type",
            Self::TypeBuiltin => "type.builtin",
            Self::Function => "function",
            Self::FunctionBuiltin => "function.builtin",
            Self::FunctionMacro => "function.macro",
            Self::Method => "function.method",
            Self::Variable => "variable",
            Self::VariableBuiltin => "variable.builtin",
            Self::Parameter => "variable.parameter",
            Self::Field => "variable.field",
            Self::Constant => "constant",
            Self::String => "string",
            Self::StringEscape => "string.escape",
            Self::Character => "character",
            Self::Number => "number",
            Self::Boolean => "boolean",
            Self::Comment => "comment",
            Self::CommentDoc => "comment.doc",
            Self::Punctuation => "punctuation",
            Self::PunctuationBracket => "punctuation.bracket",
            Self::PunctuationDelimiter => "punctuation.delimiter",
            Self::Operator => "operator",
            Self::Error => "diagnostic.error",
            Self::Warning => "diagnostic.warning",
            Self::Info => "diagnostic.info",
            Self::Hint => "diagnostic.hint",
            Self::Namespace => "namespace",
            Self::Constructor => "constructor",
            Self::Label => "label",
            Self::Attribute => "attribute",
            Self::Tag => "tag",
            Self::MarkupHeading => "markup.heading",
            Self::MarkupBold => "markup.bold",
            Self::MarkupItalic => "markup.italic",
            Self::MarkupStrikethrough => "markup.strikethrough",
            Self::MarkupLink => "markup.link",
            Self::MarkupLinkUrl => "markup.link.url",
            Self::MarkupList => "markup.list",
            Self::MarkupRaw => "markup.raw",
            Self::MarkupRawInline => "markup.raw.inline",
            Self::Embedded => "embedded",
            Self::Special => "special",
            Self::Custom => "custom",
        }
    }
}

impl HighlightGroup {
    /// Check if this is a keyword category.
    #[must_use]
    pub const fn is_keyword(self) -> bool {
        matches!(
            self,
            Self::Keyword
                | Self::KeywordControl
                | Self::KeywordOperator
                | Self::KeywordFunction
                | Self::KeywordType
        )
    }

    /// Check if this is a type category.
    #[must_use]
    pub const fn is_type(self) -> bool {
        matches!(self, Self::Type | Self::TypeBuiltin)
    }

    /// Check if this is a function category.
    #[must_use]
    pub const fn is_function(self) -> bool {
        matches!(
            self,
            Self::Function | Self::FunctionBuiltin | Self::FunctionMacro | Self::Method
        )
    }

    /// Check if this is a variable category.
    #[must_use]
    pub const fn is_variable(self) -> bool {
        matches!(
            self,
            Self::Variable | Self::VariableBuiltin | Self::Parameter | Self::Field | Self::Constant
        )
    }

    /// Check if this is a literal category.
    #[must_use]
    pub const fn is_literal(self) -> bool {
        matches!(
            self,
            Self::String | Self::StringEscape | Self::Character | Self::Number | Self::Boolean
        )
    }

    /// Check if this is a comment category.
    #[must_use]
    pub const fn is_comment(self) -> bool {
        matches!(self, Self::Comment | Self::CommentDoc)
    }

    /// Check if this is a punctuation category.
    #[must_use]
    pub const fn is_punctuation(self) -> bool {
        matches!(self, Self::Punctuation | Self::PunctuationBracket | Self::PunctuationDelimiter)
    }

    /// Check if this is a diagnostic category.
    #[must_use]
    pub const fn is_diagnostic(self) -> bool {
        matches!(self, Self::Error | Self::Warning | Self::Info | Self::Hint)
    }

    /// Check if this is a markup category.
    #[must_use]
    pub const fn is_markup(self) -> bool {
        matches!(
            self,
            Self::MarkupHeading
                | Self::MarkupBold
                | Self::MarkupItalic
                | Self::MarkupStrikethrough
                | Self::MarkupLink
                | Self::MarkupLinkUrl
                | Self::MarkupList
                | Self::MarkupRaw
                | Self::MarkupRawInline
        )
    }
}

/// A highlighted byte range in source code.
///
/// Uses byte offsets for efficient incremental updates.
/// Byte offsets align with tree-sitter's native representation.
///
/// # Example
///
/// ```
/// use reovim_driver_syntax::{HighlightSpan, HighlightGroup};
///
/// let span = HighlightSpan::new(0, 5, HighlightGroup::Keyword);
/// assert_eq!(span.len(), 5);
/// assert!(!span.is_empty());
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighlightSpan {
    /// Start byte offset (inclusive).
    pub start_byte: usize,
    /// End byte offset (exclusive).
    pub end_byte: usize,
    /// Highlight category.
    pub group: HighlightGroup,
}

impl HighlightSpan {
    /// Create a new highlight span.
    #[must_use]
    pub const fn new(start_byte: usize, end_byte: usize, group: HighlightGroup) -> Self {
        Self {
            start_byte,
            end_byte,
            group,
        }
    }

    /// Get the length in bytes.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.end_byte - self.start_byte
    }

    /// Check if the span is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.start_byte == self.end_byte
    }

    /// Check if this span overlaps with a byte range.
    #[must_use]
    pub const fn overlaps(&self, range: &Range<usize>) -> bool {
        self.start_byte < range.end && self.end_byte > range.start
    }

    /// Check if this span contains a byte offset.
    #[must_use]
    pub const fn contains(&self, byte: usize) -> bool {
        self.start_byte <= byte && byte < self.end_byte
    }

    /// Get this span as a byte range.
    #[must_use]
    pub const fn byte_range(&self) -> Range<usize> {
        self.start_byte..self.end_byte
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_highlight_group_implements_syntax_highlight() {
        assert_eq!(HighlightGroup::Keyword.category(), "keyword");
        assert_eq!(HighlightGroup::KeywordControl.category(), "keyword.control");
        assert_eq!(HighlightGroup::Function.category(), "function");
        assert_eq!(HighlightGroup::String.category(), "string");
        assert_eq!(HighlightGroup::Comment.category(), "comment");
        assert_eq!(HighlightGroup::Error.category(), "diagnostic.error");
        assert_eq!(HighlightGroup::Custom.category(), "custom");

        // New categories (Issue #206)
        assert_eq!(HighlightGroup::Namespace.category(), "namespace");
        assert_eq!(HighlightGroup::Constructor.category(), "constructor");
        assert_eq!(HighlightGroup::Label.category(), "label");
        assert_eq!(HighlightGroup::Attribute.category(), "attribute");
        assert_eq!(HighlightGroup::Tag.category(), "tag");
        assert_eq!(HighlightGroup::MarkupHeading.category(), "markup.heading");
        assert_eq!(HighlightGroup::MarkupBold.category(), "markup.bold");
        assert_eq!(HighlightGroup::MarkupItalic.category(), "markup.italic");
        assert_eq!(HighlightGroup::MarkupStrikethrough.category(), "markup.strikethrough");
        assert_eq!(HighlightGroup::MarkupLink.category(), "markup.link");
        assert_eq!(HighlightGroup::MarkupLinkUrl.category(), "markup.link.url");
        assert_eq!(HighlightGroup::MarkupList.category(), "markup.list");
        assert_eq!(HighlightGroup::MarkupRaw.category(), "markup.raw");
        assert_eq!(HighlightGroup::MarkupRawInline.category(), "markup.raw.inline");
        assert_eq!(HighlightGroup::Embedded.category(), "embedded");
        assert_eq!(HighlightGroup::Special.category(), "special");
    }

    #[test]
    fn test_highlight_group_is_keyword() {
        assert!(HighlightGroup::Keyword.is_keyword());
        assert!(HighlightGroup::KeywordControl.is_keyword());
        assert!(HighlightGroup::KeywordOperator.is_keyword());
        assert!(HighlightGroup::KeywordFunction.is_keyword());
        assert!(HighlightGroup::KeywordType.is_keyword());
        assert!(!HighlightGroup::Function.is_keyword());
        assert!(!HighlightGroup::String.is_keyword());
    }

    #[test]
    fn test_highlight_group_is_type() {
        assert!(HighlightGroup::Type.is_type());
        assert!(HighlightGroup::TypeBuiltin.is_type());
        assert!(!HighlightGroup::Keyword.is_type());
    }

    #[test]
    fn test_highlight_group_is_function() {
        assert!(HighlightGroup::Function.is_function());
        assert!(HighlightGroup::FunctionBuiltin.is_function());
        assert!(HighlightGroup::FunctionMacro.is_function());
        assert!(HighlightGroup::Method.is_function());
        assert!(!HighlightGroup::Variable.is_function());
    }

    #[test]
    fn test_highlight_group_is_variable() {
        assert!(HighlightGroup::Variable.is_variable());
        assert!(HighlightGroup::VariableBuiltin.is_variable());
        assert!(HighlightGroup::Parameter.is_variable());
        assert!(HighlightGroup::Field.is_variable());
        assert!(HighlightGroup::Constant.is_variable());
        assert!(!HighlightGroup::Function.is_variable());
    }

    #[test]
    fn test_highlight_group_is_literal() {
        assert!(HighlightGroup::String.is_literal());
        assert!(HighlightGroup::StringEscape.is_literal());
        assert!(HighlightGroup::Character.is_literal());
        assert!(HighlightGroup::Number.is_literal());
        assert!(HighlightGroup::Boolean.is_literal());
        assert!(!HighlightGroup::Comment.is_literal());
    }

    #[test]
    fn test_highlight_group_is_comment() {
        assert!(HighlightGroup::Comment.is_comment());
        assert!(HighlightGroup::CommentDoc.is_comment());
        assert!(!HighlightGroup::String.is_comment());
    }

    #[test]
    fn test_highlight_group_is_punctuation() {
        assert!(HighlightGroup::Punctuation.is_punctuation());
        assert!(HighlightGroup::PunctuationBracket.is_punctuation());
        assert!(HighlightGroup::PunctuationDelimiter.is_punctuation());
        assert!(!HighlightGroup::Operator.is_punctuation());
    }

    #[test]
    fn test_highlight_group_is_diagnostic() {
        assert!(HighlightGroup::Error.is_diagnostic());
        assert!(HighlightGroup::Warning.is_diagnostic());
        assert!(HighlightGroup::Info.is_diagnostic());
        assert!(HighlightGroup::Hint.is_diagnostic());
        assert!(!HighlightGroup::Keyword.is_diagnostic());
    }

    #[test]
    fn test_highlight_group_is_markup() {
        assert!(HighlightGroup::MarkupHeading.is_markup());
        assert!(HighlightGroup::MarkupBold.is_markup());
        assert!(HighlightGroup::MarkupItalic.is_markup());
        assert!(HighlightGroup::MarkupStrikethrough.is_markup());
        assert!(HighlightGroup::MarkupLink.is_markup());
        assert!(HighlightGroup::MarkupLinkUrl.is_markup());
        assert!(HighlightGroup::MarkupList.is_markup());
        assert!(HighlightGroup::MarkupRaw.is_markup());
        assert!(HighlightGroup::MarkupRawInline.is_markup());
        assert!(!HighlightGroup::Keyword.is_markup());
        assert!(!HighlightGroup::String.is_markup());
    }

    #[test]
    fn test_highlight_span_new() {
        let span = HighlightSpan::new(10, 20, HighlightGroup::Keyword);
        assert_eq!(span.start_byte, 10);
        assert_eq!(span.end_byte, 20);
        assert_eq!(span.group, HighlightGroup::Keyword);
    }

    #[test]
    fn test_highlight_span_len() {
        let span = HighlightSpan::new(10, 20, HighlightGroup::Keyword);
        assert_eq!(span.len(), 10);

        let empty = HighlightSpan::new(5, 5, HighlightGroup::Comment);
        assert_eq!(empty.len(), 0);
    }

    #[test]
    fn test_highlight_span_is_empty() {
        let span = HighlightSpan::new(10, 20, HighlightGroup::Keyword);
        assert!(!span.is_empty());

        let empty = HighlightSpan::new(5, 5, HighlightGroup::Comment);
        assert!(empty.is_empty());
    }

    #[test]
    fn test_highlight_span_overlaps() {
        let span = HighlightSpan::new(10, 20, HighlightGroup::Keyword);

        // Overlapping ranges
        assert!(span.overlaps(&(5..15)));
        assert!(span.overlaps(&(15..25)));
        assert!(span.overlaps(&(12..18)));
        assert!(span.overlaps(&(5..25)));
        assert!(span.overlaps(&(10..20)));

        // Non-overlapping ranges
        assert!(!span.overlaps(&(0..10)));
        assert!(!span.overlaps(&(20..30)));
        assert!(!span.overlaps(&(0..5)));
    }

    #[test]
    fn test_highlight_span_contains() {
        let span = HighlightSpan::new(10, 20, HighlightGroup::Keyword);

        assert!(span.contains(10));
        assert!(span.contains(15));
        assert!(span.contains(19));
        assert!(!span.contains(9));
        assert!(!span.contains(20));
        assert!(!span.contains(25));
    }

    #[test]
    fn test_highlight_span_byte_range() {
        let span = HighlightSpan::new(10, 20, HighlightGroup::Keyword);
        assert_eq!(span.byte_range(), 10..20);
    }

    #[test]
    fn test_highlight_group_all_categories_covered() {
        // Ensure every variant has a non-empty category string
        let all_groups = [
            HighlightGroup::Keyword,
            HighlightGroup::KeywordControl,
            HighlightGroup::KeywordOperator,
            HighlightGroup::KeywordFunction,
            HighlightGroup::KeywordType,
            HighlightGroup::Type,
            HighlightGroup::TypeBuiltin,
            HighlightGroup::Function,
            HighlightGroup::FunctionBuiltin,
            HighlightGroup::FunctionMacro,
            HighlightGroup::Method,
            HighlightGroup::Variable,
            HighlightGroup::VariableBuiltin,
            HighlightGroup::Parameter,
            HighlightGroup::Field,
            HighlightGroup::Constant,
            HighlightGroup::String,
            HighlightGroup::StringEscape,
            HighlightGroup::Character,
            HighlightGroup::Number,
            HighlightGroup::Boolean,
            HighlightGroup::Comment,
            HighlightGroup::CommentDoc,
            HighlightGroup::Punctuation,
            HighlightGroup::PunctuationBracket,
            HighlightGroup::PunctuationDelimiter,
            HighlightGroup::Operator,
            HighlightGroup::Error,
            HighlightGroup::Warning,
            HighlightGroup::Info,
            HighlightGroup::Hint,
            HighlightGroup::Namespace,
            HighlightGroup::Constructor,
            HighlightGroup::Label,
            HighlightGroup::Attribute,
            HighlightGroup::Tag,
            HighlightGroup::MarkupHeading,
            HighlightGroup::MarkupBold,
            HighlightGroup::MarkupItalic,
            HighlightGroup::MarkupStrikethrough,
            HighlightGroup::MarkupLink,
            HighlightGroup::MarkupLinkUrl,
            HighlightGroup::MarkupList,
            HighlightGroup::MarkupRaw,
            HighlightGroup::MarkupRawInline,
            HighlightGroup::Embedded,
            HighlightGroup::Special,
            HighlightGroup::Custom,
        ];

        for group in all_groups {
            let category = group.category();
            assert!(!category.is_empty(), "Category for {group:?} should not be empty");
        }
    }

    #[test]
    fn test_highlight_group_classification_non_overlapping() {
        // Keyword groups should NOT match other classification methods
        assert!(!HighlightGroup::Keyword.is_type());
        assert!(!HighlightGroup::Keyword.is_function());
        assert!(!HighlightGroup::Keyword.is_variable());
        assert!(!HighlightGroup::Keyword.is_literal());
        assert!(!HighlightGroup::Keyword.is_comment());
        assert!(!HighlightGroup::Keyword.is_punctuation());
        assert!(!HighlightGroup::Keyword.is_diagnostic());
        assert!(!HighlightGroup::Keyword.is_markup());

        // Operator should not match any classification
        assert!(!HighlightGroup::Operator.is_keyword());
        assert!(!HighlightGroup::Operator.is_type());
        assert!(!HighlightGroup::Operator.is_function());
        assert!(!HighlightGroup::Operator.is_variable());
        assert!(!HighlightGroup::Operator.is_literal());
        assert!(!HighlightGroup::Operator.is_comment());
        assert!(!HighlightGroup::Operator.is_punctuation());
        assert!(!HighlightGroup::Operator.is_diagnostic());
        assert!(!HighlightGroup::Operator.is_markup());
    }

    #[test]
    fn test_highlight_span_equality() {
        let span1 = HighlightSpan::new(10, 20, HighlightGroup::Keyword);
        let span2 = HighlightSpan::new(10, 20, HighlightGroup::Keyword);
        let span3 = HighlightSpan::new(10, 20, HighlightGroup::Function);
        let span4 = HighlightSpan::new(10, 25, HighlightGroup::Keyword);

        assert_eq!(span1, span2);
        assert_ne!(span1, span3);
        assert_ne!(span1, span4);
    }

    #[test]
    fn test_highlight_span_clone() {
        let span = HighlightSpan::new(5, 15, HighlightGroup::String);
        let cloned = span.clone();
        assert_eq!(span, cloned);
    }

    #[test]
    fn test_highlight_span_debug() {
        let span = HighlightSpan::new(0, 5, HighlightGroup::Comment);
        let debug = format!("{span:?}");
        assert!(debug.contains("HighlightSpan"));
        assert!(debug.contains("Comment"));
    }
}
