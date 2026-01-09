//! Markdown language support for reovim

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
    reovim_plugin_treesitter::{LanguageSupport, RegisterLanguage, TreesitterPlugin},
};

use factory::MarkdownDecorationFactory;

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

    fn build(&self, _ctx: &mut PluginContext) {
        // No commands to register
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
