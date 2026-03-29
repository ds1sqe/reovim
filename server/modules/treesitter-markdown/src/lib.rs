#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Markdown syntax highlighting module for reovim.
//!
//! This module provides Markdown language support for syntax highlighting
//! using tree-sitter-md and the `reovim-driver-syntax-treesitter` driver.
//!
//! # Architecture
//!
//! ```text
//! reovim-driver-syntax              (trait definitions + SyntaxFactoryStore)
//!         ^
//!         |
//! reovim-driver-syntax-treesitter   (generic tree-sitter driver)
//!         ^
//!         |
//! reovim-module-treesitter-markdown (THIS CRATE - Module + Markdown grammar)
//! ```
//!
//! # Self-Registration Pattern
//!
//! This module follows the self-registration pattern (like `RustSyntaxFactory`):
//! - Implements `Module` trait
//! - Registers `MarkdownSyntaxFactory` into `SyntaxFactoryStore` during `init()`
//! - Added to `DefaultsModule::create_modules()` for auto-loading
//!
//! # Example
//!
//! ```
//! use reovim_module_treesitter_markdown::MarkdownSyntaxFactory;
//! use reovim_driver_syntax::SyntaxDriverFactory;
//!
//! let factory = MarkdownSyntaxFactory::new();
//! let mut driver = factory.create("markdown").expect("Markdown is supported");
//!
//! driver.parse("# Hello World\n\nSome text.");
//! let highlights = driver.highlights(0..100);
//!
//! assert!(!highlights.is_empty());
//! ```

mod list;
mod table;

use std::sync::Arc;

use {
    reovim_driver_syntax::{
        BracketConfig, BracketConfigStore, DecorationRule, HighlightCategory, LanguageInfo,
        LanguageInfoStore, SyntaxDriver, SyntaxDriverFactory, SyntaxFactoryStore,
    },
    reovim_driver_syntax_treesitter::{Language, Query, TreeSitterDriver},
    reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version},
};

/// Markdown highlights query (embedded from queries/highlights.scm)
const MARKDOWN_HIGHLIGHTS_QUERY: &str = include_str!("queries/highlights.scm");

/// Markdown injections query (embedded from queries/injections.scm)
const MARKDOWN_INJECTIONS_QUERY: &str = include_str!("queries/injections.scm");

/// Markdown decorations query (embedded from queries/decorations.scm)
const MARKDOWN_DECORATIONS_QUERY: &str = include_str!("queries/decorations.scm");

/// Markdown inline decorations query (embedded from `queries_inline/decorations.scm`)
const MARKDOWN_INLINE_DECORATIONS_QUERY: &str = include_str!("queries_inline/decorations.scm");

/// Markdown context query (embedded from queries/context.scm)
const MARKDOWN_CONTEXT_QUERY: &str = include_str!("queries/context.scm");

/// Factory for creating Markdown syntax drivers.
///
/// This factory creates `TreeSitterDriver` instances configured for
/// Markdown syntax highlighting using the tree-sitter-md grammar.
pub struct MarkdownSyntaxFactory {
    /// Pre-compiled highlights query
    highlight_query: Arc<Query>,
    /// Pre-compiled injections query (for code block language detection)
    injections_query: Arc<Query>,
    /// Pre-compiled decorations query (conceal, background, virtual text)
    decoration_query: Arc<Query>,
    /// Declarative rules mapping capture names to annotation kinds
    decoration_rules: Vec<DecorationRule>,
    /// Inline language grammar (for emphasis, bold, links, etc.)
    inline_language: Language,
    /// Pre-compiled inline decorations query
    inline_decoration_query: Arc<Query>,
    /// Inline decoration rules
    inline_decoration_rules: Vec<DecorationRule>,
    /// Pre-compiled table query for `TableDecorationProvider`
    table_query: Arc<Query>,
    /// Pre-compiled list marker query for `ListDecorationProvider`
    list_query: Arc<Query>,
    /// Pre-compiled context query (scope hierarchy)
    context_query: Arc<Query>,
}

impl MarkdownSyntaxFactory {
    /// Create a new Markdown syntax factory.
    ///
    /// Pre-compiles the highlights and injections queries for efficiency.
    ///
    /// # Panics
    ///
    /// Panics if the embedded queries fail to compile.
    /// This should never happen with correctly bundled queries.
    #[must_use]
    pub fn new() -> Self {
        let language: Language = tree_sitter_md::LANGUAGE.into();
        let inline_language: Language = tree_sitter_md::INLINE_LANGUAGE.into();

        let highlight_query = Query::new(&language, MARKDOWN_HIGHLIGHTS_QUERY)
            .expect("Failed to compile Markdown highlights query");

        let injections_query = Query::new(&language, MARKDOWN_INJECTIONS_QUERY)
            .expect("Failed to compile Markdown injections query");

        let decoration_query = Query::new(&language, MARKDOWN_DECORATIONS_QUERY)
            .expect("Failed to compile Markdown decorations query");

        let inline_decoration_query =
            Query::new(&inline_language, MARKDOWN_INLINE_DECORATIONS_QUERY)
                .expect("Failed to compile Markdown inline decorations query");

        let table_query = Query::new(&language, "(pipe_table) @table")
            .expect("Failed to compile Markdown table query");

        let list_query = Query::new(
            &language,
            "(list_marker_minus) @marker (list_marker_plus) @marker (list_marker_star) @marker",
        )
        .expect("Failed to compile Markdown list query");

        let context_query = Query::new(&language, MARKDOWN_CONTEXT_QUERY)
            .expect("Failed to compile Markdown context query");

        Self {
            highlight_query: Arc::new(highlight_query),
            injections_query: Arc::new(injections_query),
            decoration_query: Arc::new(decoration_query),
            decoration_rules: markdown_decoration_rules(),
            inline_language,
            inline_decoration_query: Arc::new(inline_decoration_query),
            inline_decoration_rules: markdown_inline_decoration_rules(),
            table_query: Arc::new(table_query),
            list_query: Arc::new(list_query),
            context_query: Arc::new(context_query),
        }
    }
}

impl Default for MarkdownSyntaxFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl SyntaxDriverFactory for MarkdownSyntaxFactory {
    fn create(&self, language_id: &str) -> Option<Box<dyn SyntaxDriver>> {
        // Only support "markdown" language ID
        if language_id != "markdown" {
            return None;
        }

        let language: Language = tree_sitter_md::LANGUAGE.into();

        TreeSitterDriver::builder("markdown", &language, self.highlight_query.clone())
            .context_query(self.context_query.clone())
            .injections_query(self.injections_query.clone())
            .decoration(self.decoration_query.clone(), self.decoration_rules.clone())
            .inline_decoration(
                &self.inline_language,
                self.inline_decoration_query.clone(),
                self.inline_decoration_rules.clone(),
            )
            .decoration_provider(Box::new(table::TableDecorationProvider::new(
                self.table_query.clone(),
            )))
            .decoration_provider(Box::new(list::ListDecorationProvider::new(
                self.list_query.clone(),
            )))
            .build()
            .map(|d| Box::new(d) as Box<dyn SyntaxDriver>)
    }

    fn supported_languages(&self) -> Vec<&str> {
        vec!["markdown"]
    }

    fn supports(&self, language_id: &str) -> bool {
        language_id == "markdown"
    }
}

// ============================================================================
// Decoration Rules
// ============================================================================

/// Build the declarative decoration rules for Markdown.
///
/// Each rule maps a capture name from `decorations.scm` to a semantic
/// `HighlightCategory`. No rendering concerns — the client decides visuals.
fn markdown_decoration_rules() -> Vec<DecorationRule> {
    vec![
        // Heading markers
        DecorationRule {
            capture_name: "heading.1.marker".into(),
            category: HighlightCategory::new("markup.heading.1"),
        },
        DecorationRule {
            capture_name: "heading.2.marker".into(),
            category: HighlightCategory::new("markup.heading.2"),
        },
        DecorationRule {
            capture_name: "heading.3.marker".into(),
            category: HighlightCategory::new("markup.heading.3"),
        },
        DecorationRule {
            capture_name: "heading.4.marker".into(),
            category: HighlightCategory::new("markup.heading.4"),
        },
        DecorationRule {
            capture_name: "heading.5.marker".into(),
            category: HighlightCategory::new("markup.heading.5"),
        },
        DecorationRule {
            capture_name: "heading.6.marker".into(),
            category: HighlightCategory::new("markup.heading.6"),
        },
        // Checkboxes
        DecorationRule {
            capture_name: "checkbox.unchecked".into(),
            category: HighlightCategory::new("markup.list.checkbox"),
        },
        DecorationRule {
            capture_name: "checkbox.checked".into(),
            category: HighlightCategory::new("markup.list.checkbox.checked"),
        },
        // Code blocks
        DecorationRule {
            capture_name: "code_block".into(),
            category: HighlightCategory::new("markup.raw.block"),
        },
        // Blockquote markers
        DecorationRule {
            capture_name: "blockquote.marker".into(),
            category: HighlightCategory::new("markup.quote.marker"),
        },
        // Horizontal rules
        DecorationRule {
            capture_name: "horizontal_rule".into(),
            category: HighlightCategory::new("markup.horizontal_rule"),
        },
    ]
}

/// Build the declarative inline decoration rules for Markdown.
///
/// These rules target the inline grammar (`INLINE_LANGUAGE`) for emphasis,
/// bold, code spans, strikethrough, and links.
fn markdown_inline_decoration_rules() -> Vec<DecorationRule> {
    vec![
        // Code span delimiters (backticks)
        DecorationRule {
            capture_name: "code_span.delimiter".into(),
            category: HighlightCategory::new("markup.raw.delimiter"),
        },
        // Code span content
        DecorationRule {
            capture_name: "code_span".into(),
            category: HighlightCategory::new("markup.raw.inline"),
        },
        // Emphasis (styled by client as italic)
        DecorationRule {
            capture_name: "emphasis".into(),
            category: HighlightCategory::new("markup.italic"),
        },
        // Strong emphasis (styled by client as bold)
        DecorationRule {
            capture_name: "strong".into(),
            category: HighlightCategory::new("markup.bold"),
        },
        // Strikethrough (styled by client as strikethrough)
        DecorationRule {
            capture_name: "strikethrough".into(),
            category: HighlightCategory::new("markup.strikethrough"),
        },
        // Link destination (URL)
        DecorationRule {
            capture_name: "link.destination".into(),
            category: HighlightCategory::new("markup.link.url"),
        },
    ]
}

// ============================================================================
// Module Implementation (Self-Registration Pattern)
// ============================================================================

/// Treesitter Markdown syntax module.
///
/// Follows the self-registration pattern (like `TreesitterRustModule`):
/// - Implements `Module` trait
/// - Registers `MarkdownSyntaxFactory` into `SyntaxFactoryStore` during `init()`
/// - Added to `DefaultsModule::create_modules()` for auto-loading
pub struct TreesitterMarkdownModule;

impl TreesitterMarkdownModule {
    /// Create a new Treesitter Markdown module.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for TreesitterMarkdownModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for TreesitterMarkdownModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("treesitter-markdown")
    }

    fn name(&self) -> &'static str {
        "Treesitter Markdown"
    }

    fn version(&self) -> Version {
        Version::new(0, 9, 0)
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        let factory = Arc::new(MarkdownSyntaxFactory::new());

        // Register as SyntaxDriverFactory (for creating Markdown drivers and injection children)
        let syntax_store = ctx.services.get_or_create::<SyntaxFactoryStore>();
        syntax_store.add(factory);

        // Register language metadata for detection
        let lang_store = ctx.services.get_or_create::<LanguageInfoStore>();
        lang_store.add(
            LanguageInfo::new("markdown", "Markdown").with_extensions(["md", "markdown", "mdx"]),
        );

        // Register bracket config for Markdown
        // Note: {} not used in prose
        let bracket_store = ctx.services.get_or_create::<BracketConfigStore>();
        bracket_store.add(
            BracketConfig::new("markdown")
                .with_rainbow([('(', ')'), ('[', ']')])
                .with_autopair([('(', ')'), ('[', ']'), ('"', '"'), ('`', '`')])
                .with_highlight([('(', ')'), ('[', ']')]),
        );

        tracing::info!(
            "TreesitterMarkdownModule: registered Markdown syntax and injection factories"
        );
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        tracing::info!("TreesitterMarkdownModule: exiting");
        Ok(())
    }

    fn provides(&self) -> &[&'static str] {
        &[reovim_capabilities::SYNTAX_HIGHLIGHTING]
    }
}

// Generate FFI entry points for dynamic loading (only when building standalone cdylib)
#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(TreesitterMarkdownModule);

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
