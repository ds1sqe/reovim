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
    // Future: Indents
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
        let query = Query::new(ts_language, source).ok()?;
        self.queries.insert(key, query);
        self.queries.get(&key)
    }

    /// Get the raw query source for a language and type
    const fn get_query_source(language_id: LanguageId, query_type: QueryType) -> Option<&'static str> {
        match (language_id, query_type) {
            // Highlight queries
            (LanguageId::Rust, QueryType::Highlights) => Some(embedded::RUST_HIGHLIGHTS),
            (LanguageId::C, QueryType::Highlights) => Some(embedded::C_HIGHLIGHTS),
            (LanguageId::JavaScript, QueryType::Highlights) => Some(embedded::JAVASCRIPT_HIGHLIGHTS),
            (LanguageId::Python, QueryType::Highlights) => Some(embedded::PYTHON_HIGHLIGHTS),
            (LanguageId::Json, QueryType::Highlights) => Some(embedded::JSON_HIGHLIGHTS),
            (LanguageId::Toml, QueryType::Highlights) => Some(embedded::TOML_HIGHLIGHTS),
            (LanguageId::Markdown, QueryType::Highlights) => Some(embedded::MARKDOWN_HIGHLIGHTS),

            // Text object queries (for semantic text objects like daf, dic)
            (LanguageId::Rust, QueryType::TextObjects) => Some(embedded::RUST_TEXTOBJECTS),
            (LanguageId::C, QueryType::TextObjects) => Some(embedded::C_TEXTOBJECTS),
            (LanguageId::JavaScript, QueryType::TextObjects) => Some(embedded::JAVASCRIPT_TEXTOBJECTS),
            (LanguageId::Python, QueryType::TextObjects) => Some(embedded::PYTHON_TEXTOBJECTS),

            // Fold queries (for code folding)
            (LanguageId::Rust, QueryType::Folds) => Some(embedded::RUST_FOLDS),
            (LanguageId::C, QueryType::Folds) => Some(embedded::C_FOLDS),
            (LanguageId::JavaScript, QueryType::Folds) => Some(embedded::JAVASCRIPT_FOLDS),
            (LanguageId::Python, QueryType::Folds) => Some(embedded::PYTHON_FOLDS),

            // Languages without text object or fold queries, or unknown language
            (
                LanguageId::Json | LanguageId::Toml | LanguageId::Markdown,
                QueryType::TextObjects | QueryType::Folds,
            )
            | (LanguageId::Unknown, _) => None,
        }
    }
}
