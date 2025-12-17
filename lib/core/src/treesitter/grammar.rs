//! Grammar loading and language detection

use std::collections::HashMap;
use std::path::Path;

use tree_sitter::Language;

/// Language identifier for treesitter grammars
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LanguageId {
    Rust,
    C,
    JavaScript,
    Python,
    Json,
    Toml,
    Markdown,
    Unknown,
}

impl LanguageId {
    /// Get the language name as a string
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::C => "c",
            Self::JavaScript => "javascript",
            Self::Python => "python",
            Self::Json => "json",
            Self::Toml => "toml",
            Self::Markdown => "markdown",
            Self::Unknown => "unknown",
        }
    }
}

/// A bundled tree-sitter grammar
pub struct BundledGrammar {
    id: LanguageId,
    language: Language,
    extensions: &'static [&'static str],
}

impl BundledGrammar {
    /// Create a new bundled grammar
    const fn new(id: LanguageId, language: Language, extensions: &'static [&'static str]) -> Self {
        Self {
            id,
            language,
            extensions,
        }
    }

    /// Get the language identifier
    #[must_use]
    pub const fn id(&self) -> LanguageId {
        self.id
    }

    /// Get the tree-sitter Language
    #[must_use]
    pub const fn language(&self) -> &Language {
        &self.language
    }

    /// Get supported file extensions
    #[must_use]
    pub const fn extensions(&self) -> &'static [&'static str] {
        self.extensions
    }

    /// Create Rust grammar
    #[must_use]
    pub fn rust() -> Self {
        Self::new(
            LanguageId::Rust,
            tree_sitter_rust::LANGUAGE.into(),
            &["rs"],
        )
    }

    /// Create C grammar
    #[must_use]
    pub fn c() -> Self {
        Self::new(LanguageId::C, tree_sitter_c::LANGUAGE.into(), &["c", "h"])
    }

    /// Create JavaScript grammar
    #[must_use]
    pub fn javascript() -> Self {
        Self::new(
            LanguageId::JavaScript,
            tree_sitter_javascript::LANGUAGE.into(),
            &["js", "mjs", "cjs", "jsx"],
        )
    }

    /// Create Python grammar
    #[must_use]
    pub fn python() -> Self {
        Self::new(
            LanguageId::Python,
            tree_sitter_python::LANGUAGE.into(),
            &["py", "pyi"],
        )
    }

    /// Create JSON grammar
    #[must_use]
    pub fn json() -> Self {
        Self::new(
            LanguageId::Json,
            tree_sitter_json::LANGUAGE.into(),
            &["json"],
        )
    }

    /// Create TOML grammar
    #[must_use]
    pub fn toml() -> Self {
        Self::new(
            LanguageId::Toml,
            tree_sitter_toml_ng::LANGUAGE.into(),
            &["toml"],
        )
    }

    /// Create Markdown grammar
    #[must_use]
    pub fn markdown() -> Self {
        Self::new(
            LanguageId::Markdown,
            tree_sitter_md::LANGUAGE.into(),
            &["md", "markdown"],
        )
    }
}

/// Registry of available grammars with language detection
pub struct GrammarRegistry {
    grammars: HashMap<LanguageId, BundledGrammar>,
    extension_map: HashMap<&'static str, LanguageId>,
}

impl Default for GrammarRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl GrammarRegistry {
    /// Create a new registry with all bundled grammars
    #[must_use]
    pub fn new() -> Self {
        let mut registry = Self {
            grammars: HashMap::new(),
            extension_map: HashMap::new(),
        };

        // Register all bundled grammars
        registry.register(BundledGrammar::rust());
        registry.register(BundledGrammar::c());
        registry.register(BundledGrammar::javascript());
        registry.register(BundledGrammar::python());
        registry.register(BundledGrammar::json());
        registry.register(BundledGrammar::toml());
        registry.register(BundledGrammar::markdown());

        registry
    }

    /// Register a bundled grammar
    fn register(&mut self, grammar: BundledGrammar) {
        let id = grammar.id();
        for ext in grammar.extensions() {
            self.extension_map.insert(ext, id);
        }
        self.grammars.insert(id, grammar);
    }

    /// Detect language from file path based on extension
    #[must_use]
    pub fn detect_language(&self, path: &str) -> LanguageId {
        Path::new(path)
            .extension()
            .and_then(|e| e.to_str())
            .and_then(|ext| self.extension_map.get(ext))
            .copied()
            .unwrap_or(LanguageId::Unknown)
    }

    /// Get the grammar for a language
    #[must_use]
    pub fn get(&self, id: LanguageId) -> Option<&BundledGrammar> {
        self.grammars.get(&id)
    }

    /// Get the tree-sitter Language for a language ID
    #[must_use]
    pub fn get_language(&self, id: LanguageId) -> Option<Language> {
        self.grammars.get(&id).map(|g| g.language().clone())
    }
}
