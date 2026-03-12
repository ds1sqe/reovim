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
#[path = "item_tests.rs"]
mod tests;
