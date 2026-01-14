//! Capture name to HighlightGroup mapping.
//!
//! This module provides `CaptureMapper` which maps tree-sitter capture names
//! (like `@keyword.control`) to `HighlightGroup` enum variants.
//!
//! # Hierarchical Fallback
//!
//! The mapper supports hierarchical fallback for dotted capture names:
//! - `keyword.control.conditional` -> try exact match first
//! - Falls back to `keyword.control`
//! - Falls back to `keyword`
//! - Finally returns `HighlightGroup::Custom` if no match

use std::collections::HashMap;

use reovim_driver_syntax::HighlightGroup;

/// Maps tree-sitter capture names to `HighlightGroup` variants.
///
/// Contains 120+ pre-defined mappings for common capture names across
/// languages. Supports hierarchical fallback for dotted names.
///
/// # Example
///
/// ```ignore
/// use reovim_plugin_treesitter::capture_mapper::CaptureMapper;
/// use reovim_driver_syntax::HighlightGroup;
///
/// let mapper = CaptureMapper::new();
///
/// // Exact match
/// assert_eq!(mapper.map("keyword"), HighlightGroup::Keyword);
///
/// // Hierarchical fallback
/// assert_eq!(mapper.map("keyword.control.conditional"), HighlightGroup::KeywordControl);
/// ```
pub struct CaptureMapper {
    mappings: HashMap<String, HighlightGroup>,
}

impl CaptureMapper {
    /// Create a new capture mapper with all standard mappings.
    #[must_use]
    pub fn new() -> Self {
        let mut mappings = HashMap::with_capacity(150);

        // ===== KEYWORDS (30+ variants) =====
        mappings.insert("keyword".into(), HighlightGroup::Keyword);
        mappings.insert("keyword.control".into(), HighlightGroup::KeywordControl);
        mappings.insert("keyword.control.conditional".into(), HighlightGroup::KeywordControl);
        mappings.insert("keyword.control.repeat".into(), HighlightGroup::KeywordControl);
        mappings.insert("keyword.control.return".into(), HighlightGroup::KeywordControl);
        mappings.insert("keyword.control.exception".into(), HighlightGroup::KeywordControl);
        mappings.insert("keyword.control.import".into(), HighlightGroup::KeywordControl);
        mappings.insert("keyword.control.flow".into(), HighlightGroup::KeywordControl);
        mappings.insert("keyword.operator".into(), HighlightGroup::KeywordOperator);
        mappings.insert("keyword.function".into(), HighlightGroup::KeywordFunction);
        mappings.insert("keyword.type".into(), HighlightGroup::KeywordType);
        mappings.insert("keyword.storage".into(), HighlightGroup::Keyword);
        mappings.insert("keyword.storage.type".into(), HighlightGroup::KeywordType);
        mappings.insert("keyword.storage.modifier".into(), HighlightGroup::Keyword);
        mappings.insert("keyword.storage.modifier.mut".into(), HighlightGroup::Keyword);
        mappings.insert("keyword.storage.modifier.ref".into(), HighlightGroup::Keyword);
        mappings.insert("keyword.async".into(), HighlightGroup::Keyword);
        mappings.insert("keyword.async.block".into(), HighlightGroup::Keyword);
        mappings.insert("keyword.coroutine".into(), HighlightGroup::Keyword);
        mappings.insert("keyword.import".into(), HighlightGroup::KeywordControl);
        mappings.insert("keyword.export".into(), HighlightGroup::KeywordControl);
        mappings.insert("keyword.return".into(), HighlightGroup::KeywordControl);
        mappings.insert("keyword.conditional".into(), HighlightGroup::KeywordControl);
        mappings.insert("keyword.repeat".into(), HighlightGroup::KeywordControl);
        mappings.insert("keyword.exception".into(), HighlightGroup::KeywordControl);
        mappings.insert("keyword.directive".into(), HighlightGroup::Keyword);

        // ===== TYPES (20+ variants) =====
        mappings.insert("type".into(), HighlightGroup::Type);
        mappings.insert("type.builtin".into(), HighlightGroup::TypeBuiltin);
        mappings.insert("type.definition".into(), HighlightGroup::Type);
        mappings.insert("type.qualifier".into(), HighlightGroup::Type);
        mappings.insert("type.array".into(), HighlightGroup::Type);
        mappings.insert("type.pointer".into(), HighlightGroup::Type);
        mappings.insert("type.reference".into(), HighlightGroup::Type);
        mappings.insert("type.reference.mut".into(), HighlightGroup::Type);
        mappings.insert("type.slice".into(), HighlightGroup::Type);
        mappings.insert("type.tuple".into(), HighlightGroup::Type);
        mappings.insert("type.enum".into(), HighlightGroup::Type);
        mappings.insert("type.enum.variant".into(), HighlightGroup::Type);
        mappings.insert("type.struct".into(), HighlightGroup::Type);
        mappings.insert("type.class".into(), HighlightGroup::Type);
        mappings.insert("type.interface".into(), HighlightGroup::Type);
        mappings.insert("type.trait".into(), HighlightGroup::Type);
        mappings.insert("type.parameter".into(), HighlightGroup::Type);
        mappings.insert("type.super".into(), HighlightGroup::Type);

        // ===== FUNCTIONS (15+ variants) =====
        mappings.insert("function".into(), HighlightGroup::Function);
        mappings.insert("function.builtin".into(), HighlightGroup::FunctionBuiltin);
        mappings.insert("function.call".into(), HighlightGroup::Function);
        mappings.insert("function.macro".into(), HighlightGroup::FunctionMacro);
        mappings.insert("function.method".into(), HighlightGroup::Method);
        mappings.insert("function.method.call".into(), HighlightGroup::Method);
        mappings.insert("function.special".into(), HighlightGroup::Function);
        mappings.insert("method".into(), HighlightGroup::Method);
        mappings.insert("method.call".into(), HighlightGroup::Method);
        mappings.insert("method.builtin".into(), HighlightGroup::FunctionBuiltin);
        mappings.insert("macro".into(), HighlightGroup::FunctionMacro);
        mappings.insert("macro.call".into(), HighlightGroup::FunctionMacro);

        // ===== VARIABLES (15+ variants) =====
        mappings.insert("variable".into(), HighlightGroup::Variable);
        mappings.insert("variable.builtin".into(), HighlightGroup::VariableBuiltin);
        mappings.insert("variable.parameter".into(), HighlightGroup::Parameter);
        mappings.insert("variable.other".into(), HighlightGroup::Variable);
        mappings.insert("variable.other.member".into(), HighlightGroup::Field);
        mappings.insert("variable.member".into(), HighlightGroup::Field);
        mappings.insert("variable.special".into(), HighlightGroup::Variable);
        mappings.insert("parameter".into(), HighlightGroup::Parameter);
        mappings.insert("property".into(), HighlightGroup::Field);
        mappings.insert("property.definition".into(), HighlightGroup::Field);
        mappings.insert("field".into(), HighlightGroup::Field);
        mappings.insert("constant".into(), HighlightGroup::Constant);
        mappings.insert("constant.builtin".into(), HighlightGroup::Constant);
        mappings.insert("constant.builtin.boolean".into(), HighlightGroup::Boolean);

        // ===== LITERALS (15+ variants) =====
        mappings.insert("string".into(), HighlightGroup::String);
        mappings.insert("string.escape".into(), HighlightGroup::StringEscape);
        mappings.insert("string.special".into(), HighlightGroup::String);
        mappings.insert("string.special.path".into(), HighlightGroup::String);
        mappings.insert("string.special.symbol".into(), HighlightGroup::String);
        mappings.insert("string.special.url".into(), HighlightGroup::String);
        mappings.insert("string.regexp".into(), HighlightGroup::String);
        mappings.insert("string.documentation".into(), HighlightGroup::CommentDoc);
        mappings.insert("character".into(), HighlightGroup::Character);
        mappings.insert("character.special".into(), HighlightGroup::StringEscape);
        mappings.insert("number".into(), HighlightGroup::Number);
        mappings.insert("number.float".into(), HighlightGroup::Number);
        mappings.insert("float".into(), HighlightGroup::Number);
        mappings.insert("boolean".into(), HighlightGroup::Boolean);

        // ===== COMMENTS (10+ variants) =====
        mappings.insert("comment".into(), HighlightGroup::Comment);
        mappings.insert("comment.documentation".into(), HighlightGroup::CommentDoc);
        mappings.insert("comment.line".into(), HighlightGroup::Comment);
        mappings.insert("comment.block".into(), HighlightGroup::Comment);
        mappings.insert("comment.block.documentation".into(), HighlightGroup::CommentDoc);
        mappings.insert("comment.todo".into(), HighlightGroup::Comment);
        mappings.insert("comment.note".into(), HighlightGroup::Comment);
        mappings.insert("comment.warning".into(), HighlightGroup::Comment);

        // ===== PUNCTUATION (15+ variants) =====
        mappings.insert("punctuation".into(), HighlightGroup::Punctuation);
        mappings.insert("punctuation.bracket".into(), HighlightGroup::PunctuationBracket);
        mappings.insert("punctuation.bracket.angle".into(), HighlightGroup::PunctuationBracket);
        mappings.insert("punctuation.bracket.round".into(), HighlightGroup::PunctuationBracket);
        mappings.insert("punctuation.bracket.square".into(), HighlightGroup::PunctuationBracket);
        mappings.insert("punctuation.bracket.curly".into(), HighlightGroup::PunctuationBracket);
        mappings.insert("punctuation.delimiter".into(), HighlightGroup::PunctuationDelimiter);
        mappings.insert("punctuation.special".into(), HighlightGroup::Punctuation);
        mappings.insert("punctuation.separator".into(), HighlightGroup::PunctuationDelimiter);

        // ===== OPERATORS (15+ variants) =====
        mappings.insert("operator".into(), HighlightGroup::Operator);
        mappings.insert("operator.binary".into(), HighlightGroup::Operator);
        mappings.insert("operator.unary".into(), HighlightGroup::Operator);
        mappings.insert("operator.logical".into(), HighlightGroup::Operator);
        mappings.insert("operator.comparison".into(), HighlightGroup::Operator);
        mappings.insert("operator.assignment".into(), HighlightGroup::Operator);
        mappings.insert("operator.arithmetic".into(), HighlightGroup::Operator);
        mappings.insert("operator.bitwise".into(), HighlightGroup::Operator);
        mappings.insert("operator.deref".into(), HighlightGroup::Operator);
        mappings.insert("operator.borrow".into(), HighlightGroup::Operator);

        // ===== NEW CATEGORIES (require extended HighlightGroup) =====
        mappings.insert("namespace".into(), HighlightGroup::Namespace);
        mappings.insert("module".into(), HighlightGroup::Namespace);
        mappings.insert("constructor".into(), HighlightGroup::Constructor);
        mappings.insert("label".into(), HighlightGroup::Label);
        mappings.insert("attribute".into(), HighlightGroup::Attribute);
        mappings.insert("attribute.builtin".into(), HighlightGroup::Attribute);
        mappings.insert("embedded".into(), HighlightGroup::Embedded);
        mappings.insert("special".into(), HighlightGroup::Special);
        mappings.insert("tag".into(), HighlightGroup::Tag);
        mappings.insert("tag.builtin".into(), HighlightGroup::Tag);
        mappings.insert("tag.attribute".into(), HighlightGroup::Attribute);
        mappings.insert("tag.delimiter".into(), HighlightGroup::Punctuation);

        // ===== RUST-SPECIFIC PATTERNS =====
        mappings.insert("pattern".into(), HighlightGroup::Keyword);
        mappings.insert("pattern.or".into(), HighlightGroup::Keyword);
        mappings.insert("pattern.tuple".into(), HighlightGroup::Keyword);
        mappings.insert("pattern.struct".into(), HighlightGroup::Keyword);
        mappings.insert("pattern.enum.variant".into(), HighlightGroup::Keyword);
        mappings.insert("lifetime".into(), HighlightGroup::Label);

        // ===== MARKDOWN MARKUP =====
        mappings.insert("markup".into(), HighlightGroup::Special);
        mappings.insert("markup.heading".into(), HighlightGroup::MarkupHeading);
        mappings.insert("markup.heading.1".into(), HighlightGroup::MarkupHeading);
        mappings.insert("markup.heading.2".into(), HighlightGroup::MarkupHeading);
        mappings.insert("markup.heading.3".into(), HighlightGroup::MarkupHeading);
        mappings.insert("markup.heading.4".into(), HighlightGroup::MarkupHeading);
        mappings.insert("markup.heading.5".into(), HighlightGroup::MarkupHeading);
        mappings.insert("markup.heading.6".into(), HighlightGroup::MarkupHeading);
        mappings.insert("markup.bold".into(), HighlightGroup::MarkupBold);
        mappings.insert("markup.italic".into(), HighlightGroup::MarkupItalic);
        mappings.insert("markup.strikethrough".into(), HighlightGroup::MarkupStrikethrough);
        mappings.insert("markup.link".into(), HighlightGroup::MarkupLink);
        mappings.insert("markup.link.text".into(), HighlightGroup::MarkupLink);
        mappings.insert("markup.link.url".into(), HighlightGroup::MarkupLinkUrl);
        mappings.insert("markup.link.label".into(), HighlightGroup::MarkupLink);
        mappings.insert("markup.list".into(), HighlightGroup::MarkupList);
        mappings.insert("markup.list.numbered".into(), HighlightGroup::MarkupList);
        mappings.insert("markup.list.unnumbered".into(), HighlightGroup::MarkupList);
        mappings.insert("markup.raw".into(), HighlightGroup::MarkupRaw);
        mappings.insert("markup.raw.block".into(), HighlightGroup::MarkupRaw);
        mappings.insert("markup.raw.inline".into(), HighlightGroup::MarkupRawInline);
        mappings.insert("markup.quote".into(), HighlightGroup::Comment);

        // ===== DIFF/PATCH =====
        mappings.insert("diff.plus".into(), HighlightGroup::String);
        mappings.insert("diff.minus".into(), HighlightGroup::Error);
        mappings.insert("diff.delta".into(), HighlightGroup::Warning);

        // ===== DIAGNOSTICS =====
        mappings.insert("error".into(), HighlightGroup::Error);
        mappings.insert("warning".into(), HighlightGroup::Warning);
        mappings.insert("info".into(), HighlightGroup::Info);
        mappings.insert("hint".into(), HighlightGroup::Hint);
        mappings.insert("diagnostic".into(), HighlightGroup::Info);
        mappings.insert("diagnostic.error".into(), HighlightGroup::Error);
        mappings.insert("diagnostic.warning".into(), HighlightGroup::Warning);
        mappings.insert("diagnostic.info".into(), HighlightGroup::Info);
        mappings.insert("diagnostic.hint".into(), HighlightGroup::Hint);

        Self { mappings }
    }

    /// Map a capture name to a `HighlightGroup`.
    ///
    /// Uses hierarchical fallback for dotted capture names:
    /// 1. Try exact match
    /// 2. Strip last segment and try again
    /// 3. Repeat until no dots remain
    /// 4. Return `HighlightGroup::Custom` if no match
    ///
    /// # Arguments
    ///
    /// * `capture_name` - The tree-sitter capture name (without `@` prefix)
    ///
    /// # Returns
    ///
    /// The matching `HighlightGroup` or `Custom` if no match found.
    #[must_use]
    pub fn map(&self, capture_name: &str) -> HighlightGroup {
        // Try exact match first
        if let Some(&group) = self.mappings.get(capture_name) {
            return group;
        }

        // Hierarchical fallback: "keyword.control.conditional" -> "keyword.control" -> "keyword"
        let mut name = capture_name;
        while let Some(dot_pos) = name.rfind('.') {
            name = &name[..dot_pos];
            if let Some(&group) = self.mappings.get(name) {
                return group;
            }
        }

        // No match found
        HighlightGroup::Custom
    }

    /// Get the number of mappings.
    #[must_use]
    pub fn len(&self) -> usize {
        self.mappings.len()
    }

    /// Check if the mapper has no mappings.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.mappings.is_empty()
    }

    /// Check if a capture name has an exact mapping.
    #[must_use]
    pub fn has_exact(&self, capture_name: &str) -> bool {
        self.mappings.contains_key(capture_name)
    }

    /// Check if a capture name has any mapping (including fallback).
    #[must_use]
    pub fn has_mapping(&self, capture_name: &str) -> bool {
        self.map(capture_name) != HighlightGroup::Custom
    }
}

impl Default for CaptureMapper {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mapper_exact_match() {
        let mapper = CaptureMapper::new();

        assert_eq!(mapper.map("keyword"), HighlightGroup::Keyword);
        assert_eq!(mapper.map("type"), HighlightGroup::Type);
        assert_eq!(mapper.map("function"), HighlightGroup::Function);
        assert_eq!(mapper.map("string"), HighlightGroup::String);
        assert_eq!(mapper.map("comment"), HighlightGroup::Comment);
    }

    #[test]
    fn test_mapper_hierarchical_fallback() {
        let mapper = CaptureMapper::new();

        // Exact match for keyword.control
        assert_eq!(mapper.map("keyword.control"), HighlightGroup::KeywordControl);

        // Fallback from keyword.control.conditional
        assert_eq!(mapper.map("keyword.control.conditional"), HighlightGroup::KeywordControl);

        // Deep fallback: unknown.nested.path -> unknown.nested -> unknown -> Custom
        assert_eq!(mapper.map("keyword.unknown.nested.path"), HighlightGroup::Keyword);

        // Complete unknown falls back to Custom
        assert_eq!(mapper.map("completely_unknown"), HighlightGroup::Custom);
    }

    #[test]
    fn test_mapper_new_categories() {
        let mapper = CaptureMapper::new();

        // New categories added in Issue #206
        assert_eq!(mapper.map("namespace"), HighlightGroup::Namespace);
        assert_eq!(mapper.map("constructor"), HighlightGroup::Constructor);
        assert_eq!(mapper.map("label"), HighlightGroup::Label);
        assert_eq!(mapper.map("attribute"), HighlightGroup::Attribute);
        assert_eq!(mapper.map("tag"), HighlightGroup::Tag);
    }

    #[test]
    fn test_mapper_markup() {
        let mapper = CaptureMapper::new();

        assert_eq!(mapper.map("markup.heading"), HighlightGroup::MarkupHeading);
        assert_eq!(mapper.map("markup.heading.1"), HighlightGroup::MarkupHeading);
        assert_eq!(mapper.map("markup.bold"), HighlightGroup::MarkupBold);
        assert_eq!(mapper.map("markup.italic"), HighlightGroup::MarkupItalic);
        assert_eq!(mapper.map("markup.link"), HighlightGroup::MarkupLink);
        assert_eq!(mapper.map("markup.raw"), HighlightGroup::MarkupRaw);
        assert_eq!(mapper.map("markup.raw.inline"), HighlightGroup::MarkupRawInline);
    }

    #[test]
    fn test_mapper_rust_specific() {
        let mapper = CaptureMapper::new();

        // Rust-specific captures
        assert_eq!(mapper.map("lifetime"), HighlightGroup::Label);
        assert_eq!(mapper.map("pattern"), HighlightGroup::Keyword);
        assert_eq!(mapper.map("function.macro"), HighlightGroup::FunctionMacro);
    }

    #[test]
    fn test_mapper_count() {
        let mapper = CaptureMapper::new();

        // Should have 120+ mappings
        assert!(mapper.len() >= 120, "Expected 120+ mappings, got {}", mapper.len());
    }

    #[test]
    fn test_mapper_has_mapping() {
        let mapper = CaptureMapper::new();

        assert!(mapper.has_exact("keyword"));
        assert!(mapper.has_exact("keyword.control"));
        assert!(!mapper.has_exact("keyword.unknown.deep"));

        // has_mapping includes fallback
        assert!(mapper.has_mapping("keyword.unknown.deep"));
        assert!(!mapper.has_mapping("completely_unknown"));
    }

    #[test]
    fn test_capture_coverage_common() {
        let mapper = CaptureMapper::new();

        // Common captures should all map to non-Custom
        let common_captures = [
            "keyword",
            "keyword.control",
            "keyword.function",
            "type",
            "type.builtin",
            "function",
            "function.macro",
            "variable",
            "parameter",
            "string",
            "string.escape",
            "comment",
            "comment.documentation",
            "operator",
            "punctuation",
            "punctuation.bracket",
            "number",
            "boolean",
            "constant",
        ];

        for capture in common_captures {
            assert_ne!(
                mapper.map(capture),
                HighlightGroup::Custom,
                "Capture '{}' should not map to Custom",
                capture
            );
        }
    }
}
