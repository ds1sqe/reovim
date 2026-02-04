//! Markdown language support for reovim
//!
//! This plugin provides Markdown syntax highlighting and decorations via tree-sitter.
//!
//! # New API (Issue #206)
//!
//! Use `register()` to register with `TreeSitterDriverFactory`:
//!
//! ```ignore
//! use reovim_lang_markdown::register;
//! register(&factory);
//! ```
//!
//! This registers both `markdown` (block) and `markdown_inline` languages.

// markdown/mod.rs removed - dead code (obsolete LanguageRenderer implementation)

mod config;

pub mod decorator;
pub mod factory;
pub mod stage;

#[cfg(test)]
mod tests;

use std::{any::TypeId, sync::Arc};

pub use config::MarkdownConfig;

use {
    reovim_core::{
        event_bus::EventBus,
        plugin::{Plugin, PluginContext, PluginId, PluginStateRegistry},
    },
    reovim_plugin_treesitter::{
        LanguageConfig, LanguageSupport, RegisterLanguage, TreeSitterDriverFactory,
        TreesitterPlugin,
    },
};

use {factory::MarkdownDecorationFactory, stage::MarkdownTableBorderStage};

// ============================================================================
// New API (Issue #206)
// ============================================================================

/// Register Markdown languages with the driver factory.
///
/// This is the new API for Issue #206. Call this during plugin initialization
/// to register Markdown language support with the `TreeSitterDriverFactory`.
///
/// This registers two languages:
/// - `markdown`: Block-level grammar for document structure
/// - `markdown_inline`: Inline grammar for inline formatting (injected)
pub fn register(factory: &TreeSitterDriverFactory) {
    // Block grammar (document structure)
    factory.register(LanguageConfig {
        id: "markdown",
        language: tree_sitter_md::LANGUAGE.into(),
        highlights_query: include_str!("queries/highlights.scm"),
        folds_query: None,
        injections_query: Some(include_str!("queries/injections.scm")),
        extensions: &["md", "markdown"],
    });

    // Inline grammar (inline formatting) - injected by markdown
    factory.register(LanguageConfig {
        id: "markdown_inline",
        language: tree_sitter_md::INLINE_LANGUAGE.into(),
        highlights_query: include_str!("queries_inline/highlights.scm"),
        folds_query: None,
        injections_query: None,
        extensions: &[], // No file extensions - injected only
    });
}

// ============================================================================
// Legacy API (kept for backward compatibility during migration)
// ============================================================================

/// Markdown language support
pub struct MarkdownLanguage;

impl LanguageSupport for MarkdownLanguage {
    fn language_id(&self) -> &'static str {
        "markdown"
    }

    fn file_extensions(&self) -> &'static [&'static str] {
        &["md", "markdown"]
    }

    fn tree_sitter_language(&self) -> reovim_plugin_treesitter::Language {
        tree_sitter_md::LANGUAGE.into()
    }

    fn highlights_query(&self) -> &'static str {
        include_str!("queries/highlights.scm")
    }

    fn decorations_query(&self) -> Option<&'static str> {
        Some(include_str!("queries/decorations.scm"))
    }

    fn injections_query(&self) -> Option<&'static str> {
        Some(include_str!("queries/injections.scm"))
    }

    fn context_query(&self) -> Option<&'static str> {
        Some(include_str!("queries/context.scm"))
    }
}

/// Markdown inline language support
pub struct MarkdownInlineLanguage;

impl LanguageSupport for MarkdownInlineLanguage {
    fn language_id(&self) -> &'static str {
        "markdown_inline"
    }

    fn file_extensions(&self) -> &'static [&'static str] {
        // No file extensions - this is injected by markdown
        &[]
    }

    fn tree_sitter_language(&self) -> reovim_plugin_treesitter::Language {
        tree_sitter_md::INLINE_LANGUAGE.into()
    }

    fn highlights_query(&self) -> &'static str {
        include_str!("queries_inline/highlights.scm")
    }

    fn decorations_query(&self) -> Option<&'static str> {
        Some(include_str!("queries_inline/decorations.scm"))
    }
}

/// Markdown language plugin
pub struct MarkdownPlugin;

impl Plugin for MarkdownPlugin {
    fn id(&self) -> PluginId {
        PluginId::new("reovim:lang-markdown")
    }

    fn name(&self) -> &'static str {
        "Markdown Language"
    }

    fn description(&self) -> &'static str {
        "Markdown language support with syntax highlighting and decorations"
    }

    fn dependencies(&self) -> Vec<TypeId> {
        vec![TypeId::of::<TreesitterPlugin>()]
    }

    fn build(&self, ctx: &mut PluginContext) {
        // Register render stage for table borders (virtual lines)
        ctx.register_render_stage(Arc::new(MarkdownTableBorderStage::new()));
    }

    fn init_state(&self, registry: &PluginStateRegistry) {
        // Register the decoration factory for markdown files
        registry.set_decoration_factory(MarkdownDecorationFactory::shared());
    }

    fn subscribe(&self, bus: &EventBus, _state: Arc<PluginStateRegistry>) {
        // Register both markdown and markdown_inline languages
        // Context is now provided via TreesitterContextProvider using context_query()
        bus.emit(RegisterLanguage {
            language: Arc::new(MarkdownLanguage),
        });
        bus.emit(RegisterLanguage {
            language: Arc::new(MarkdownInlineLanguage),
        });
    }
}

#[cfg(test)]
mod register_tests {
    use {super::*, reovim_driver_syntax::SyntaxDriverFactory};

    #[test]
    fn test_markdown_registration() {
        let factory = TreeSitterDriverFactory::new();
        register(&factory);

        // Should register both markdown and markdown_inline
        assert!(factory.supports("markdown"));
        assert!(factory.supports("markdown_inline"));
        assert_eq!(factory.language_count(), 2);
    }

    #[test]
    fn test_markdown_driver_creation() {
        let factory = TreeSitterDriverFactory::new();
        register(&factory);

        let driver = factory.create("markdown");
        assert!(driver.is_some());
        assert_eq!(driver.unwrap().language(), "markdown");
    }

    #[test]
    fn test_markdown_inline_driver_creation() {
        let factory = TreeSitterDriverFactory::new();
        register(&factory);

        let driver = factory.create("markdown_inline");
        assert!(driver.is_some());
        assert_eq!(driver.unwrap().language(), "markdown_inline");
    }

    #[test]
    fn test_markdown_highlighting() {
        let factory = TreeSitterDriverFactory::new();
        register(&factory);

        let mut driver = factory.create("markdown").unwrap();
        driver.parse("# Hello World\n\nThis is **bold** text.");

        assert!(driver.is_parsed());
        let highlights = driver.highlights(0..40);
        assert!(!highlights.is_empty());
    }

    #[test]
    fn test_markdown_file_extensions() {
        let factory = TreeSitterDriverFactory::new();
        register(&factory);

        assert_eq!(factory.detect_language("README.md"), Some("markdown".to_string()));
        assert_eq!(factory.detect_language("doc.markdown"), Some("markdown".to_string()));
    }

    #[test]
    fn test_markdown_inline_no_extensions() {
        let factory = TreeSitterDriverFactory::new();
        register(&factory);

        // markdown_inline should not be detected from file extensions
        // (it's only used via injection)
        let extensions = factory.extensions_for("markdown_inline");
        assert!(extensions.is_empty());
    }
}
