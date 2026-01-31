//! Bash language support for reovim
//!
//! This plugin provides Bash/Shell syntax highlighting via tree-sitter.
//!
//! # New API (Issue #206)
//!
//! Use `register()` to register with `TreeSitterDriverFactory`:
//!
//! ```ignore
//! use reovim_lang_bash::register;
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

/// Register Bash language with the driver factory.
///
/// This is the new API for Issue #206. Call this during plugin initialization
/// to register Bash language support with the `TreeSitterDriverFactory`.
pub fn register(factory: &TreeSitterDriverFactory) {
    factory.register(LanguageConfig {
        id: "bash",
        language: tree_sitter_bash::LANGUAGE.into(),
        highlights_query: include_str!("queries/highlights.scm"),
        folds_query: Some(include_str!("queries/folds.scm")),
        injections_query: None,
        extensions: &["sh", "bash", "zsh", "bashrc", "zshrc", "profile"],
    });
}

// ============================================================================
// Legacy API (kept for backward compatibility during migration)
// ============================================================================

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

#[cfg(test)]
mod tests {
    use {super::*, reovim_driver_syntax::SyntaxDriverFactory};

    #[test]
    fn test_bash_registration() {
        let factory = TreeSitterDriverFactory::new();
        register(&factory);

        assert!(factory.supports("bash"));
        assert_eq!(factory.language_count(), 1);
    }

    #[test]
    fn test_bash_driver_creation() {
        let factory = TreeSitterDriverFactory::new();
        register(&factory);

        let driver = factory.create("bash");
        assert!(driver.is_some());
        assert_eq!(driver.unwrap().language(), "bash");
    }

    #[test]
    fn test_bash_highlighting() {
        let factory = TreeSitterDriverFactory::new();
        register(&factory);

        let mut driver = factory.create("bash").unwrap();
        driver.parse("#!/bin/bash\necho \"Hello World\"");

        assert!(driver.is_parsed());
        let highlights = driver.highlights(0..35);
        assert!(!highlights.is_empty());
    }

    #[test]
    fn test_bash_file_extensions() {
        let factory = TreeSitterDriverFactory::new();
        register(&factory);

        assert_eq!(factory.detect_language("script.sh"), Some("bash".to_string()));
        assert_eq!(factory.detect_language("script.bash"), Some("bash".to_string()));
        assert_eq!(factory.detect_language(".bashrc"), Some("bash".to_string()));
    }
}
