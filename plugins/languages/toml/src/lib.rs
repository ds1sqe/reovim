//! TOML language support for reovim

use std::{any::TypeId, sync::Arc};

use {
    reovim_core::{
        event_bus::EventBus,
        plugin::{Plugin, PluginContext, PluginId, PluginStateRegistry},
    },
    reovim_plugin_treesitter::{LanguageSupport, RegisterLanguage, TreesitterPlugin},
};

/// TOML language support
pub struct TomlLanguage;

impl LanguageSupport for TomlLanguage {
    fn language_id(&self) -> &'static str {
        "toml"
    }

    fn file_extensions(&self) -> &'static [&'static str] {
        &["toml"]
    }

    fn tree_sitter_language(&self) -> reovim_plugin_treesitter::Language {
        tree_sitter_toml_ng::LANGUAGE.into()
    }

    fn highlights_query(&self) -> &'static str {
        include_str!("queries/highlights.scm")
    }
}

/// TOML language plugin
pub struct TomlPlugin;

impl Plugin for TomlPlugin {
    fn id(&self) -> PluginId {
        PluginId::new("reovim:lang-toml")
    }

    fn name(&self) -> &'static str {
        "TOML Language"
    }

    fn description(&self) -> &'static str {
        "TOML language support with syntax highlighting"
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
            language: Arc::new(TomlLanguage),
        });
    }
}
