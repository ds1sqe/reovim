//! TOML language support for reovim
//!
//! This plugin provides TOML syntax highlighting via tree-sitter.
//!
//! # New API (Issue #206)
//!
//! Use `register()` to register with `TreeSitterDriverFactory`:
//!
//! ```ignore
//! use reovim_lang_toml::register;
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

/// Register TOML language with the driver factory.
///
/// This is the new API for Issue #206. Call this during plugin initialization
/// to register TOML language support with the `TreeSitterDriverFactory`.
pub fn register(factory: &TreeSitterDriverFactory) {
    factory.register(LanguageConfig {
        id: "toml",
        language: tree_sitter_toml_ng::LANGUAGE.into(),
        highlights_query: include_str!("queries/highlights.scm"),
        folds_query: None,
        injections_query: None,
        extensions: &["toml"],
    });
}

// ============================================================================
// Legacy API (kept for backward compatibility during migration)
// ============================================================================

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

#[cfg(test)]
mod tests {
    use {super::*, reovim_driver_syntax::SyntaxDriverFactory};

    #[test]
    fn test_toml_registration() {
        let factory = TreeSitterDriverFactory::new();
        register(&factory);

        assert!(factory.supports("toml"));
        assert_eq!(factory.language_count(), 1);
    }

    #[test]
    fn test_toml_driver_creation() {
        let factory = TreeSitterDriverFactory::new();
        register(&factory);

        let driver = factory.create("toml");
        assert!(driver.is_some());
        assert_eq!(driver.unwrap().language(), "toml");
    }

    #[test]
    fn test_toml_highlighting() {
        let factory = TreeSitterDriverFactory::new();
        register(&factory);

        let mut driver = factory.create("toml").unwrap();
        driver.parse(
            r#"[package]
name = "test"
version = "1.0""#,
        );

        assert!(driver.is_parsed());
        let highlights = driver.highlights(0..50);
        assert!(!highlights.is_empty());
    }

    #[test]
    fn test_toml_file_extensions() {
        let factory = TreeSitterDriverFactory::new();
        register(&factory);

        assert_eq!(factory.detect_language("Cargo.toml"), Some("toml".to_string()));
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
