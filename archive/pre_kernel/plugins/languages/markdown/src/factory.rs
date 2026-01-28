//! Markdown decoration factory
//!
//! Implements `DecorationFactory` trait from core to create decoration
//! providers for markdown files.

use {
    reovim_core::decoration::{DecorationFactory, DecorationProvider, SharedDecorationFactory},
    std::sync::Arc,
};

use crate::decorator::MarkdownDecorator;

/// Factory for creating markdown decoration providers
pub struct MarkdownDecorationFactory;

impl MarkdownDecorationFactory {
    /// Create a new markdown decoration factory
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Create as a shared factory (for registration with PluginStateRegistry)
    #[must_use]
    pub fn shared() -> SharedDecorationFactory {
        Arc::new(Self::new())
    }
}

impl Default for MarkdownDecorationFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl DecorationFactory for MarkdownDecorationFactory {
    fn create_provider(
        &self,
        file_path: &str,
        _content: &str,
    ) -> Option<Box<dyn DecorationProvider>> {
        if !self.supports_file(file_path) {
            return None;
        }

        MarkdownDecorator::new().map(|d| Box::new(d) as Box<dyn DecorationProvider>)
    }

    fn supports_file(&self, file_path: &str) -> bool {
        let path = std::path::Path::new(file_path);
        path.extension()
            .is_some_and(|ext| matches!(ext.to_str(), Some("md" | "markdown" | "mkd")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_factory_supports_markdown_files() {
        let factory = MarkdownDecorationFactory::new();

        assert!(factory.supports_file("README.md"));
        assert!(factory.supports_file("docs/guide.markdown"));
        assert!(factory.supports_file("/path/to/file.mkd"));
        assert!(!factory.supports_file("main.rs"));
        assert!(!factory.supports_file("config.json"));
    }

    #[test]
    fn test_factory_creates_provider() {
        let factory = MarkdownDecorationFactory::new();

        let provider = factory.create_provider("test.md", "# Hello");
        assert!(provider.is_some());

        let provider = factory.create_provider("test.rs", "fn main() {}");
        assert!(provider.is_none());
    }
}
