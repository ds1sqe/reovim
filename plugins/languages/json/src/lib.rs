//! JSON language support for reovim
//!
//! This plugin provides JSON syntax highlighting via tree-sitter.
//!
//! # New API (Issue #206)
//!
//! Use `register()` to register with `TreeSitterDriverFactory`:
//!
//! ```ignore
//! use reovim_lang_json::register;
//! register(&factory);
//! ```

use std::{any::TypeId, sync::Arc};

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

// ============================================================================
// New API (Issue #206)
// ============================================================================

/// Register JSON language with the driver factory.
///
/// This is the new API for Issue #206. Call this during plugin initialization
/// to register JSON language support with the `TreeSitterDriverFactory`.
pub fn register(factory: &TreeSitterDriverFactory) {
    factory.register(LanguageConfig {
        id: "json",
        language: tree_sitter_json::LANGUAGE.into(),
        highlights_query: include_str!("queries/highlights.scm"),
        folds_query: None,
        injections_query: None,
        extensions: &["json", "jsonc"],
    });
}

// ============================================================================
// Legacy API (kept for backward compatibility during migration)
// ============================================================================

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

#[cfg(test)]
mod tests {
    use {super::*, reovim_driver_syntax::SyntaxDriverFactory};

    #[test]
    fn test_json_registration() {
        let factory = TreeSitterDriverFactory::new();
        register(&factory);

        assert!(factory.supports("json"));
        assert_eq!(factory.language_count(), 1);
    }

    #[test]
    fn test_json_driver_creation() {
        let factory = TreeSitterDriverFactory::new();
        register(&factory);

        let driver = factory.create("json");
        assert!(driver.is_some());

        let driver = driver.unwrap();
        assert_eq!(driver.language(), "json");
    }

    #[test]
    fn test_json_highlighting() {
        let factory = TreeSitterDriverFactory::new();
        register(&factory);

        let mut driver = factory.create("json").unwrap();
        driver.parse(r#"{"key": "value", "num": 42}"#);

        assert!(driver.is_parsed());
        let highlights = driver.highlights(0..27);

        // Should have highlights for strings and numbers
        assert!(!highlights.is_empty());
    }

    #[test]
    fn test_json_file_extensions() {
        let factory = TreeSitterDriverFactory::new();
        register(&factory);

        assert_eq!(factory.detect_language("test.json"), Some("json".to_string()));
        assert_eq!(factory.detect_language("test.jsonc"), Some("json".to_string()));
    }
}
