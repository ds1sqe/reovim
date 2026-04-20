//! Formatter registry — maps filetypes to formatter providers.

use std::collections::HashMap;

use {crate::provider::FormatterProvider, reovim_kernel::api::v1::Service};

/// Registry of formatters keyed by filetype.
///
/// Stores one formatter per filetype (e.g., "rust" -> rustfmt, "python" -> black).
pub struct FormatterRegistry {
    formatters: HashMap<String, Box<dyn FormatterProvider>>,
}

impl FormatterRegistry {
    /// Create an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            formatters: HashMap::new(),
        }
    }

    /// Register a formatter for a filetype.
    ///
    /// Replaces any existing formatter for the same filetype.
    pub fn register(&mut self, filetype: impl Into<String>, provider: Box<dyn FormatterProvider>) {
        self.formatters.insert(filetype.into(), provider);
    }

    /// Get the formatter for a filetype.
    #[must_use]
    pub fn get(&self, filetype: &str) -> Option<&dyn FormatterProvider> {
        self.formatters.get(filetype).map(AsRef::as_ref)
    }

    /// Check if a formatter is registered for a filetype.
    #[must_use]
    pub fn has(&self, filetype: &str) -> bool {
        self.formatters.contains_key(filetype)
    }

    /// Number of registered formatters.
    #[must_use]
    pub fn len(&self) -> usize {
        self.formatters.len()
    }

    /// Whether the registry is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.formatters.is_empty()
    }
}

impl Service for FormatterRegistry {}

impl Default for FormatterRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "registry_tests.rs"]
mod tests;
