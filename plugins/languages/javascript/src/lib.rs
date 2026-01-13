//! JavaScript language support for reovim
//!
//! This plugin provides JavaScript syntax highlighting via tree-sitter.
//!
//! # New API (Issue #206)
//!
//! Use `register()` to register with `TreeSitterDriverFactory`:
//!
//! ```ignore
//! use reovim_lang_javascript::register;
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

/// Register JavaScript language with the driver factory.
///
/// This is the new API for Issue #206. Call this during plugin initialization
/// to register JavaScript language support with the `TreeSitterDriverFactory`.
pub fn register(factory: &TreeSitterDriverFactory) {
    factory.register(LanguageConfig {
        id: "javascript",
        language: tree_sitter_javascript::LANGUAGE.into(),
        highlights_query: include_str!("queries/highlights.scm"),
        folds_query: Some(include_str!("queries/folds.scm")),
        injections_query: None,
        extensions: &["js", "jsx", "mjs", "cjs"],
    });
}

// ============================================================================
// Legacy API (kept for backward compatibility during migration)
// ============================================================================

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

#[cfg(test)]
mod tests {
    use {super::*, reovim_driver_syntax::SyntaxDriverFactory};

    #[test]
    fn test_javascript_registration() {
        let factory = TreeSitterDriverFactory::new();
        register(&factory);

        assert!(factory.supports("javascript"));
        assert_eq!(factory.language_count(), 1);
    }

    #[test]
    fn test_javascript_driver_creation() {
        let factory = TreeSitterDriverFactory::new();
        register(&factory);

        let driver = factory.create("javascript");
        assert!(driver.is_some());
        assert_eq!(driver.unwrap().language(), "javascript");
    }

    #[test]
    fn test_javascript_highlighting() {
        let factory = TreeSitterDriverFactory::new();
        register(&factory);

        let mut driver = factory.create("javascript").unwrap();
        driver.parse("const x = 42; function foo() { return x; }");

        assert!(driver.is_parsed());
        let highlights = driver.highlights(0..43);
        assert!(!highlights.is_empty());
    }

    #[test]
    fn test_javascript_file_extensions() {
        let factory = TreeSitterDriverFactory::new();
        register(&factory);

        assert_eq!(factory.detect_language("app.js"), Some("javascript".to_string()));
        assert_eq!(factory.detect_language("app.jsx"), Some("javascript".to_string()));
        assert_eq!(factory.detect_language("app.mjs"), Some("javascript".to_string()));
        assert_eq!(factory.detect_language("app.cjs"), Some("javascript".to_string()));
    }
}
