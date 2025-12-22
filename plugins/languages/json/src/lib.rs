//! JSON language support for reovim

use std::{any::TypeId, sync::Arc};

use {
    reovim_core::{
        event_bus::EventBus,
        plugin::{Plugin, PluginContext, PluginId, PluginStateRegistry},
    },
    reovim_plugin_treesitter::{LanguageSupport, RegisterLanguage, TreesitterPlugin},
};

/// JSON language support
pub struct JsonLanguage;

impl LanguageSupport for JsonLanguage {
    fn language_id(&self) -> &'static str {
        "json"
    }

    fn file_extensions(&self) -> &'static [&'static str] {
        &["json", "jsonc"]
    }

    fn tree_sitter_language(&self) -> reovim_plugin_treesitter::Language {
        tree_sitter_json::LANGUAGE.into()
    }

    fn highlights_query(&self) -> &'static str {
        include_str!("queries/highlights.scm")
    }
}

/// JSON language plugin
pub struct JsonPlugin;

impl Plugin for JsonPlugin {
    fn id(&self) -> PluginId {
        PluginId::new("reovim:lang-json")
    }

    fn name(&self) -> &'static str {
        "JSON Language"
    }

    fn description(&self) -> &'static str {
        "JSON language support with syntax highlighting"
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
            language: Arc::new(JsonLanguage),
        });
    }
}
