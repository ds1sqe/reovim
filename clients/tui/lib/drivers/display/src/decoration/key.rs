//! Decoration provider key - typed key for decoration provider lookup.

use std::sync::Arc;

use reovim_kernel::api::v1::ServiceKey;

/// Typed key for per-buffer decoration provider lookup.
///
/// Used with `DecorationProviderFactory` to create per-buffer providers.
/// For global providers that take `buffer_id`, use `DecorationSourceKey`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DecorationProviderKey {
    /// Syntax highlighting provider (per-buffer).
    Syntax,
    /// Diagnostic markers (LSP, per-buffer).
    Diagnostic,
}

impl ServiceKey for DecorationProviderKey {
    fn service_name() -> &'static str {
        "DecorationProvider"
    }
}

/// String-based key for global buffer decoration source lookup.
///
/// Modules define their own keys using a namespace convention:
/// - `"pair.rainbow"` (from pair module)
/// - `"search.match"` (from search module)
/// - `"visual.selection"` (from editor/vim module)
///
/// # Architecture
///
/// This is purely **mechanism** - the driver defines the key type but has
/// zero knowledge of which modules exist or what keys they register.
///
/// ```text
/// clients/tui/lib/drivers/display/   -> DecorationSourceKey (mechanism)
/// server/modules/pair/               -> "pair.rainbow" key (policy)
/// server/modules/search/             -> "search.match" key (policy)
/// ```
///
/// # Example
///
/// ```ignore
/// use reovim_driver_display::DecorationSourceKey;
///
/// // Module defines its key
/// const RAINBOW_KEY: &str = "pair.rainbow";
///
/// // In init():
/// let key = DecorationSourceKey::new(RAINBOW_KEY);
/// registry.register(key, state.clone());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DecorationSourceKey(Arc<str>);

impl DecorationSourceKey {
    /// Create a new decoration source key.
    ///
    /// # Convention
    ///
    /// Use format: `<module>.<feature>` (e.g., `"pair.rainbow"`, `"search.match"`)
    #[must_use]
    pub fn new(key: impl Into<Arc<str>>) -> Self {
        Self(key.into())
    }

    /// Get the key as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&'static str> for DecorationSourceKey {
    fn from(s: &'static str) -> Self {
        Self::new(s)
    }
}

impl From<String> for DecorationSourceKey {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

impl ServiceKey for DecorationSourceKey {
    fn service_name() -> &'static str {
        "BufferDecorationSource"
    }
}

impl std::fmt::Display for DecorationSourceKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
#[path = "key_tests.rs"]
mod tests;
