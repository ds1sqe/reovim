//! Completion item types.
//!
//! Defines the completion item produced by sources and the kind enum
//! for categorizing items in the popup display.

/// Completion item produced by a source.
#[derive(Debug, Clone)]
pub struct CompletionItem {
    /// Display label (shown in popup).
    pub label: String,
    /// Text to insert on confirm.
    pub insert_text: String,
    /// Category for icon/grouping.
    pub kind: CompletionKind,
    /// Short description (e.g., type signature).
    pub detail: Option<String>,
    /// Long documentation.
    pub documentation: Option<String>,
    /// Source that produced this item.
    pub source_id: &'static str,
    /// Whether `insert_text` uses snippet syntax.
    pub is_snippet: bool,
    /// Source-assigned priority for ordering within the same match score.
    /// Higher = preferred.
    pub sort_priority: u16,
}

/// Completion item kind for icon/category display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CompletionKind {
    /// Plain text.
    Text,
    /// Function.
    Function,
    /// Method.
    Method,
    /// Variable.
    Variable,
    /// Struct/record field.
    Field,
    /// Language keyword.
    Keyword,
    /// Snippet template.
    Snippet,
    /// Module or namespace.
    Module,
    /// Class.
    Class,
    /// Interface or trait.
    Interface,
    /// Property.
    Property,
    /// Constant value.
    Constant,
    /// Enum type.
    Enum,
    /// Enum member/variant.
    EnumMember,
    /// File path.
    File,
    /// Directory path.
    Folder,
    /// Type parameter or generic.
    TypeParameter,
}

impl CompletionKind {
    /// Short abbreviation for popup display.
    #[must_use]
    pub const fn abbreviation(self) -> &'static str {
        match self {
            Self::Text => "txt",
            Self::Function => "fn",
            Self::Method => "met",
            Self::Variable => "var",
            Self::Field => "fld",
            Self::Keyword => "kw",
            Self::Snippet => "snp",
            Self::Module => "mod",
            Self::Class => "cls",
            Self::Interface => "ifc",
            Self::Property => "prp",
            Self::Constant => "con",
            Self::Enum => "enm",
            Self::EnumMember => "emb",
            Self::File => "fil",
            Self::Folder => "dir",
            Self::TypeParameter => "typ",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_item() -> CompletionItem {
        CompletionItem {
            label: "test_func".to_string(),
            insert_text: "test_func()".to_string(),
            kind: CompletionKind::Function,
            detail: Some("fn test_func()".to_string()),
            documentation: None,
            source_id: "test",
            is_snippet: false,
            sort_priority: 100,
        }
    }

    #[test]
    fn item_debug() {
        let item = make_item();
        let debug = format!("{item:?}");
        assert!(debug.contains("test_func"));
        assert!(debug.contains("Function"));
    }

    #[test]
    fn item_clone() {
        let item = make_item();
        #[allow(clippy::redundant_clone)]
        let cloned = item.clone();
        assert_eq!(cloned.label, "test_func");
        assert_eq!(cloned.insert_text, "test_func()");
        assert_eq!(cloned.kind, CompletionKind::Function);
        assert_eq!(cloned.detail.as_deref(), Some("fn test_func()"));
        assert!(cloned.documentation.is_none());
        assert_eq!(cloned.source_id, "test");
        assert!(!cloned.is_snippet);
        assert_eq!(cloned.sort_priority, 100);
    }

    #[test]
    fn item_with_documentation() {
        let item = CompletionItem {
            documentation: Some("Detailed docs here".to_string()),
            ..make_item()
        };
        assert_eq!(item.documentation.as_deref(), Some("Detailed docs here"));
    }

    #[test]
    fn item_snippet_flag() {
        let item = CompletionItem {
            is_snippet: true,
            kind: CompletionKind::Snippet,
            ..make_item()
        };
        assert!(item.is_snippet);
        assert_eq!(item.kind, CompletionKind::Snippet);
    }

    #[test]
    fn kind_debug() {
        let debug = format!("{:?}", CompletionKind::Function);
        assert_eq!(debug, "Function");
    }

    #[test]
    fn kind_clone_copy() {
        let kind = CompletionKind::Method;
        let copied = kind;
        #[allow(clippy::clone_on_copy)]
        let cloned = kind.clone();
        assert_eq!(copied, cloned);
    }

    #[test]
    fn kind_eq() {
        assert_eq!(CompletionKind::Text, CompletionKind::Text);
        assert_ne!(CompletionKind::Text, CompletionKind::Function);
    }

    #[test]
    fn kind_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(CompletionKind::Function);
        set.insert(CompletionKind::Method);
        set.insert(CompletionKind::Function); // duplicate
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn abbreviation_all_variants() {
        let cases = [
            (CompletionKind::Text, "txt"),
            (CompletionKind::Function, "fn"),
            (CompletionKind::Method, "met"),
            (CompletionKind::Variable, "var"),
            (CompletionKind::Field, "fld"),
            (CompletionKind::Keyword, "kw"),
            (CompletionKind::Snippet, "snp"),
            (CompletionKind::Module, "mod"),
            (CompletionKind::Class, "cls"),
            (CompletionKind::Interface, "ifc"),
            (CompletionKind::Property, "prp"),
            (CompletionKind::Constant, "con"),
            (CompletionKind::Enum, "enm"),
            (CompletionKind::EnumMember, "emb"),
            (CompletionKind::File, "fil"),
            (CompletionKind::Folder, "dir"),
            (CompletionKind::TypeParameter, "typ"),
        ];
        for (kind, expected) in cases {
            assert_eq!(kind.abbreviation(), expected, "{kind:?} abbreviation mismatch");
        }
        // Ensure we covered all 17 variants
        assert_eq!(cases.len(), 17);
    }
}
