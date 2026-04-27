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

    /// Nerd Font icon for popup display.
    #[must_use]
    pub const fn icon(self) -> &'static str {
        match self {
            Self::Text => "\u{f039a}",          // 󰎚 nf-md-format_text_variant
            Self::Function => "\u{f0295}",      // 󰊕 nf-md-function
            Self::Method => "\u{f01a7}",        // 󰆧 nf-md-cube_outline
            Self::Variable => "\u{f002b}",      // 󰀫 nf-md-alpha_v_box
            Self::Field => "\u{f0722}",         // 󰜢 nf-md-tag
            Self::Keyword => "\u{f030b}",       // 󰌋 nf-md-key
            Self::Snippet => "\u{f0a6b}",       // 󰩫 nf-md-snippet
            Self::Module => "\u{f0417}",        // 󰐗 nf-md-package_variant
            Self::Class => "\u{f0831}",         // 󰠱 nf-md-shape
            Self::Interface => "\u{f0730}",     // 󰜰 nf-md-transit_connection
            Self::Property => "\u{f05b7}",      // 󰖷 nf-md-wrench
            Self::Constant => "\u{f043f}",      // 󰐿 nf-md-pi
            Self::Enum => "\u{f0558}",          // 󰕘 nf-md-format_list_numbered
            Self::EnumMember => "\u{f055a}",    // 󰕚 nf-md-format_list_bulleted_type
            Self::File => "\u{f0219}",          // 󰈙 nf-md-file
            Self::Folder => "\u{f024b}",        // 󰉋 nf-md-folder
            Self::TypeParameter => "\u{f0284}", // 󰊄 nf-md-format_columns
        }
    }
}

#[cfg(test)]
#[path = "item_tests.rs"]
mod tests;
