//! C language support for reovim
//!
//! This plugin provides C syntax highlighting via tree-sitter.
//!
//! # New API (Issue #206)
//!
//! Use `register()` to register with `TreeSitterDriverFactory`:
//!
//! ```ignore
//! use reovim_lang_c::register;
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

/// Register C language with the driver factory.
///
/// This is the new API for Issue #206. Call this during plugin initialization
/// to register C language support with the `TreeSitterDriverFactory`.
pub fn register(factory: &TreeSitterDriverFactory) {
    factory.register(LanguageConfig {
        id: "c",
        language: tree_sitter_c::LANGUAGE.into(),
        highlights_query: include_str!("queries/highlights.scm"),
        folds_query: Some(include_str!("queries/folds.scm")),
        injections_query: None,
        extensions: &["c", "h"],
    });
}

// ============================================================================
// Legacy API (kept for backward compatibility during migration)
// ============================================================================

/// C language support
pub struct CLanguage;

impl LanguageSupport for CLanguage {
    fn language_id(&self) -> &'static str {
        "c"
    }

    fn file_extensions(&self) -> &'static [&'static str] {
        &["c", "h"]
    }

    fn tree_sitter_language(&self) -> reovim_plugin_treesitter::Language {
        tree_sitter_c::LANGUAGE.into()
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

/// C language plugin
pub struct CPlugin;

impl Plugin for CPlugin {
    fn id(&self) -> PluginId {
        PluginId::new("reovim:lang-c")
    }

    fn name(&self) -> &'static str {
        "C Language"
    }

    fn description(&self) -> &'static str {
        "C language support with syntax highlighting and semantic analysis"
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
            language: Arc::new(CLanguage),
        });
    }
}

#[cfg(test)]
mod tests {
    use {super::*, reovim_driver_syntax::SyntaxDriverFactory};

    #[test]
    fn test_c_registration() {
        let factory = TreeSitterDriverFactory::new();
        register(&factory);

        assert!(factory.supports("c"));
        assert_eq!(factory.language_count(), 1);
    }

    #[test]
    fn test_c_driver_creation() {
        let factory = TreeSitterDriverFactory::new();
        register(&factory);

        let driver = factory.create("c");
        assert!(driver.is_some());
        assert_eq!(driver.unwrap().language(), "c");
    }

    #[test]
    fn test_c_highlighting() {
        let factory = TreeSitterDriverFactory::new();
        register(&factory);

        let mut driver = factory.create("c").unwrap();
        driver.parse("int main() { return 0; }");

        assert!(driver.is_parsed());
        let highlights = driver.highlights(0..24);
        assert!(!highlights.is_empty());
    }

    #[test]
    fn test_c_file_extensions() {
        let factory = TreeSitterDriverFactory::new();
        register(&factory);

        assert_eq!(factory.detect_language("main.c"), Some("c".to_string()));
        assert_eq!(factory.detect_language("header.h"), Some("c".to_string()));
    }
}
