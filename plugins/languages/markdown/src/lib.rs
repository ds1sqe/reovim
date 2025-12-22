//! Markdown language support for reovim

#[cfg(test)]
mod tests;

use std::{any::TypeId, sync::Arc};

use {
    reovim_core::{
        event_bus::EventBus,
        plugin::{Plugin, PluginContext, PluginId, PluginStateRegistry},
    },
    reovim_plugin_treesitter::{LanguageSupport, RegisterLanguage, TreesitterPlugin},
};

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

    fn subscribe(&self, bus: &EventBus, _state: Arc<PluginStateRegistry>) {
        // Register both markdown and markdown_inline languages
        bus.emit(RegisterLanguage {
            language: Arc::new(MarkdownLanguage),
        });
        bus.emit(RegisterLanguage {
            language: Arc::new(MarkdownInlineLanguage),
        });
    }
}
