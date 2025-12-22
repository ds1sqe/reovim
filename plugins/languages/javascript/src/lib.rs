//! JavaScript language support for reovim

use std::{any::TypeId, sync::Arc};

use {
    reovim_core::{
        event_bus::EventBus,
        plugin::{Plugin, PluginContext, PluginId, PluginStateRegistry},
    },
    reovim_plugin_treesitter::{LanguageSupport, RegisterLanguage, TreesitterPlugin},
};

/// JavaScript language support
pub struct JavaScriptLanguage;

impl LanguageSupport for JavaScriptLanguage {
    fn language_id(&self) -> &'static str {
        "javascript"
    }

    fn file_extensions(&self) -> &'static [&'static str] {
        &["js", "jsx", "mjs", "cjs"]
    }

    fn tree_sitter_language(&self) -> reovim_plugin_treesitter::Language {
        tree_sitter_javascript::LANGUAGE.into()
    }

    fn highlights_query(&self) -> &'static str {
        include_str!("queries/highlights.scm")
    }

    fn folds_query(&self) -> Option<&'static str> {
        Some(include_str!("queries/folds.scm"))
    }

    fn textobjects_query(&self) -> Option<&'static str> {
        Some(include_str!("queries/textobjects.scm"))
    }
}

/// JavaScript language plugin
pub struct JavaScriptPlugin;

impl Plugin for JavaScriptPlugin {
    fn id(&self) -> PluginId {
        PluginId::new("reovim:lang-javascript")
    }

    fn name(&self) -> &'static str {
        "JavaScript Language"
    }

    fn description(&self) -> &'static str {
        "JavaScript language support with syntax highlighting and semantic analysis"
    }

    fn dependencies(&self) -> Vec<TypeId> {
        vec![TypeId::of::<TreesitterPlugin>()]
    }

    fn build(&self, _ctx: &mut PluginContext) {
        // No commands to register
    }

    fn subscribe(&self, bus: &EventBus, _state: Arc<PluginStateRegistry>) {
        // Register this language with treesitter
        bus.emit(RegisterLanguage {
            language: Arc::new(JavaScriptLanguage),
        });
    }
}
