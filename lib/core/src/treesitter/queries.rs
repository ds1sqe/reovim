//! Query file loading and caching

use std::collections::HashMap;

use tree_sitter::Query;

use super::grammar::LanguageId;

/// Embedded highlight queries for each language
mod embedded {
    pub const RUST_HIGHLIGHTS: &str = include_str!("queries/rust/highlights.scm");
    pub const C_HIGHLIGHTS: &str = include_str!("queries/c/highlights.scm");
    pub const JAVASCRIPT_HIGHLIGHTS: &str = include_str!("queries/javascript/highlights.scm");
    pub const PYTHON_HIGHLIGHTS: &str = include_str!("queries/python/highlights.scm");
    pub const JSON_HIGHLIGHTS: &str = include_str!("queries/json/highlights.scm");
    pub const TOML_HIGHLIGHTS: &str = include_str!("queries/toml/highlights.scm");
    pub const MARKDOWN_HIGHLIGHTS: &str = include_str!("queries/markdown/highlights.scm");
    pub const MARKDOWN_INLINE_HIGHLIGHTS: &str =
        include_str!("queries/markdown_inline/highlights.scm");

    // Text object queries for semantic text objects (daf, dic, etc.)
    pub const RUST_TEXTOBJECTS: &str = include_str!("queries/rust/textobjects.scm");
    pub const C_TEXTOBJECTS: &str = include_str!("queries/c/textobjects.scm");
    pub const JAVASCRIPT_TEXTOBJECTS: &str = include_str!("queries/javascript/textobjects.scm");
    pub const PYTHON_TEXTOBJECTS: &str = include_str!("queries/python/textobjects.scm");

    // Fold queries for code folding
    pub const RUST_FOLDS: &str = include_str!("queries/rust/folds.scm");
    pub const C_FOLDS: &str = include_str!("queries/c/folds.scm");
    pub const JAVASCRIPT_FOLDS: &str = include_str!("queries/javascript/folds.scm");
    pub const PYTHON_FOLDS: &str = include_str!("queries/python/folds.scm");

    // Decoration queries for visual rendering (concealment, icons, backgrounds)
    pub const MARKDOWN_DECORATIONS: &str = include_str!("queries/markdown/decorations.scm");
    pub const MARKDOWN_INLINE_DECORATIONS: &str =
        include_str!("queries/markdown_inline/decorations.scm");
}

/// Type of query
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QueryType {
    /// Syntax highlighting
    Highlights,
    /// Semantic text objects (function, class, parameter, etc.)
    TextObjects,
    /// Code folding regions
    Folds,
    /// Visual decorations (concealment, icons, backgrounds)
    Decorations,
}

/// Cache for compiled queries
pub struct QueryCache {
    queries: HashMap<(LanguageId, QueryType), Query>,
}

impl Default for QueryCache {
    fn default() -> Self {
        Self::new()
    }
}

impl QueryCache {
    /// Create a new empty query cache
    #[must_use]
    pub fn new() -> Self {
        Self {
            queries: HashMap::new(),
        }
    }

    /// Get a cached query (does not compile if not cached)
    #[must_use]
    pub fn get(&self, language_id: LanguageId, query_type: QueryType) -> Option<&Query> {
        self.queries.get(&(language_id, query_type))
    }

    /// Get or compile a query for the given language and type
    pub fn get_or_compile(
        &mut self,
        language_id: LanguageId,
        query_type: QueryType,
        ts_language: &tree_sitter::Language,
    ) -> Option<&Query> {
        let key = (language_id, query_type);

        // Return cached query if available
        if self.queries.contains_key(&key) {
            return self.queries.get(&key);
        }

        // Get the query source
        let source = Self::get_query_source(language_id, query_type)?;

        // Compile the query
        let query = match Query::new(ts_language, source) {
            Ok(q) => q,
            Err(e) => {
                tracing::error!(
                    "Failed to compile {:?} query for {:?}: {}",
                    query_type,
                    language_id,
                    e
                );
                return None;
            }
        };
        self.queries.insert(key, query);
        self.queries.get(&key)
    }

    /// Get the raw query source for a language and type
    const fn get_query_source(
        language_id: LanguageId,
        query_type: QueryType,
    ) -> Option<&'static str> {
        match (language_id, query_type) {
            // Highlight queries
            (LanguageId::Rust, QueryType::Highlights) => Some(embedded::RUST_HIGHLIGHTS),
            (LanguageId::C, QueryType::Highlights) => Some(embedded::C_HIGHLIGHTS),
            (LanguageId::JavaScript, QueryType::Highlights) => {
                Some(embedded::JAVASCRIPT_HIGHLIGHTS)
            }
            (LanguageId::Python, QueryType::Highlights) => Some(embedded::PYTHON_HIGHLIGHTS),
            (LanguageId::Json, QueryType::Highlights) => Some(embedded::JSON_HIGHLIGHTS),
            (LanguageId::Toml, QueryType::Highlights) => Some(embedded::TOML_HIGHLIGHTS),
            (LanguageId::Markdown, QueryType::Highlights) => Some(embedded::MARKDOWN_HIGHLIGHTS),

            // Text object queries (for semantic text objects like daf, dic)
            (LanguageId::Rust, QueryType::TextObjects) => Some(embedded::RUST_TEXTOBJECTS),
            (LanguageId::C, QueryType::TextObjects) => Some(embedded::C_TEXTOBJECTS),
            (LanguageId::JavaScript, QueryType::TextObjects) => {
                Some(embedded::JAVASCRIPT_TEXTOBJECTS)
            }
            (LanguageId::Python, QueryType::TextObjects) => Some(embedded::PYTHON_TEXTOBJECTS),

            // Fold queries (for code folding)
            (LanguageId::Rust, QueryType::Folds) => Some(embedded::RUST_FOLDS),
            (LanguageId::C, QueryType::Folds) => Some(embedded::C_FOLDS),
            (LanguageId::JavaScript, QueryType::Folds) => Some(embedded::JAVASCRIPT_FOLDS),
            (LanguageId::Python, QueryType::Folds) => Some(embedded::PYTHON_FOLDS),

            // Decoration queries (for visual rendering)
            (LanguageId::Markdown, QueryType::Decorations) => Some(embedded::MARKDOWN_DECORATIONS),

            // Markdown inline queries
            (LanguageId::MarkdownInline, QueryType::Highlights) => {
                Some(embedded::MARKDOWN_INLINE_HIGHLIGHTS)
            }
            (LanguageId::MarkdownInline, QueryType::Decorations) => {
                Some(embedded::MARKDOWN_INLINE_DECORATIONS)
            }

            // Languages without specific queries
            (
                LanguageId::Json
                | LanguageId::Toml
                | LanguageId::Markdown
                | LanguageId::MarkdownInline,
                QueryType::TextObjects | QueryType::Folds,
            )
            | (
                LanguageId::Rust
                | LanguageId::C
                | LanguageId::JavaScript
                | LanguageId::Python
                | LanguageId::Json
                | LanguageId::Toml,
                QueryType::Decorations,
            )
            | (LanguageId::Unknown, _) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use {super::*, tree_sitter::Language};

    fn get_rust_language() -> Language {
        tree_sitter_rust::LANGUAGE.into()
    }

    #[test]
    fn test_rust_highlights_query_compiles() {
        let lang = get_rust_language();
        let source = embedded::RUST_HIGHLIGHTS;
        let result = Query::new(&lang, source);
        assert!(result.is_ok(), "Rust highlights query failed to compile: {:?}", result.err());
    }

    #[test]
    fn test_rust_textobjects_query_compiles() {
        let lang = get_rust_language();
        let source = embedded::RUST_TEXTOBJECTS;
        let result = Query::new(&lang, source);
        assert!(result.is_ok(), "Rust textobjects query failed to compile: {:?}", result.err());
    }

    #[test]
    fn test_rust_folds_query_compiles() {
        let lang = get_rust_language();
        let source = embedded::RUST_FOLDS;
        let result = Query::new(&lang, source);
        assert!(result.is_ok(), "Rust folds query failed to compile: {:?}", result.err());
    }

    fn get_markdown_language() -> Language {
        tree_sitter_md::LANGUAGE.into()
    }

    fn get_markdown_inline_language() -> Language {
        tree_sitter_md::INLINE_LANGUAGE.into()
    }

    #[test]
    fn test_markdown_decorations_query_compiles() {
        let lang = get_markdown_language();
        let source = embedded::MARKDOWN_DECORATIONS;
        let result = Query::new(&lang, source);
        assert!(
            result.is_ok(),
            "Markdown decorations query failed to compile: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_markdown_inline_highlights_query_compiles() {
        let lang = get_markdown_inline_language();
        let source = embedded::MARKDOWN_INLINE_HIGHLIGHTS;
        let result = Query::new(&lang, source);
        assert!(
            result.is_ok(),
            "Markdown inline highlights query failed to compile: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_markdown_inline_decorations_query_compiles() {
        let lang = get_markdown_inline_language();
        let source = embedded::MARKDOWN_INLINE_DECORATIONS;
        let result = Query::new(&lang, source);
        assert!(
            result.is_ok(),
            "Markdown inline decorations query failed to compile: {:?}",
            result.err()
        );
    }
}

#[cfg(test)]
mod integration_tests {
    use crate::treesitter::TreesitterManager;

    #[test]
    fn test_rust_file_highlighting() {
        let rust_code = r#"
use std::collections::HashMap;

const MAX_SIZE: usize = 100;

pub struct MyStruct {
    pub name: String,
}

impl MyStruct {
    pub fn new(name: &str) -> Self {
        Self { name: name.to_string() }
    }
}

fn main() {
    let s = MyStruct::new("test");
    println!("Hello!");
}
"#;

        let mut manager = TreesitterManager::new();
        let lang_id = manager.init_buffer(0, Some("test.rs"));

        assert_eq!(lang_id, crate::treesitter::LanguageId::Rust, "Should detect Rust language");
        assert!(manager.has_parser(0), "Should have parser for buffer");

        let highlights = manager.parse_and_highlight(0, rust_code, 0, 20);

        assert!(!highlights.is_empty(), "Should generate highlights for Rust code");
    }
}
