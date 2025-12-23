//! Bash language support for reovim

use std::{any::TypeId, sync::Arc};

use {
    reovim_core::{
        event_bus::EventBus,
        plugin::{Plugin, PluginContext, PluginId, PluginStateRegistry},
    },
    reovim_plugin_treesitter::{LanguageSupport, RegisterLanguage, TreesitterPlugin},
};

/// Bash language support
pub struct BashLanguage;

impl LanguageSupport for BashLanguage {
    fn language_id(&self) -> &'static str {
        "bash"
    }

    fn file_extensions(&self) -> &'static [&'static str] {
        &["sh", "bash", "zsh", "bashrc", "zshrc", "profile"]
    }

    fn tree_sitter_language(&self) -> reovim_plugin_treesitter::Language {
        tree_sitter_bash::LANGUAGE.into()
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

/// Bash language plugin
pub struct BashPlugin;

impl Plugin for BashPlugin {
    fn id(&self) -> PluginId {
        PluginId::new("reovim:lang-bash")
    }

    fn name(&self) -> &'static str {
        "Bash Language"
    }

    fn description(&self) -> &'static str {
        "Bash language support with syntax highlighting and semantic analysis"
    }

    fn dependencies(&self) -> Vec<TypeId> {
        vec![TypeId::of::<TreesitterPlugin>()]
    }

    fn build(&self, _ctx: &mut PluginContext) {
        // No commands to register
    }

    fn subscribe(&self, bus: &EventBus, _state: Arc<PluginStateRegistry>) {
        bus.emit(RegisterLanguage {
            language: Arc::new(BashLanguage),
        });
    }
}
