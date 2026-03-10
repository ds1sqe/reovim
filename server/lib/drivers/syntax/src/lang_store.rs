//! Language info store for module self-registration.
//!
//! This module provides [`LanguageInfoStore`], a registry where syntax modules
//! can register their [`LanguageInfo`] metadata during `init()`. This follows
//! the same pattern as [`SyntaxFactoryStore`], [`ModeInfoStore`], etc.
//!
//! # Self-Registration Pattern
//!
//! ```ignore
//! // In treesitter-rust module's init():
//! impl Module for TreesitterRustModule {
//!     fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
//!         let lang_store = ctx.services.get_or_create::<LanguageInfoStore>();
//!         lang_store.add(
//!             LanguageInfo::new("rust", "Rust")
//!                 .with_extensions(["rs"])
//!                 .with_mime_types(["text/x-rust"])
//!                 .with_comments(CommentTokens::with_block("//", "/*", "*/"))
//!         );
//!         ProbeResult::Success
//!     }
//! }
//! ```

use {parking_lot::RwLock, reovim_kernel::api::v1::Service};

use crate::LanguageInfo;

/// Store for language info registered by modules during init.
///
/// Modules register their `LanguageInfo` here, and bootstrap extracts them
/// to build a concrete `LanguageRegistry` after module initialization.
#[derive(Default)]
pub struct LanguageInfoStore {
    entries: RwLock<Vec<LanguageInfo>>,
}

impl LanguageInfoStore {
    /// Create a new empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add language info to the store.
    ///
    /// Called by syntax modules during `init()`.
    pub fn add(&self, info: LanguageInfo) {
        self.entries.write().push(info);
    }

    /// Take all registered language info entries.
    ///
    /// Called by bootstrap after all modules have initialized.
    /// This drains the store, so subsequent calls return empty vec.
    pub fn take_all(&self) -> Vec<LanguageInfo> {
        std::mem::take(&mut *self.entries.write())
    }

    /// Get the number of registered entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.read().len()
    }

    /// Check if no entries are registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.read().is_empty()
    }
}

impl Service for LanguageInfoStore {}

impl std::fmt::Debug for LanguageInfoStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LanguageInfoStore")
            .field("count", &self.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::CommentTokens;

    #[test]
    fn test_store_new_empty() {
        let store = LanguageInfoStore::new();
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
    }

    #[test]
    fn test_store_default_empty() {
        let store = LanguageInfoStore::default();
        assert!(store.is_empty());
    }

    #[test]
    fn test_store_add_and_len() {
        let store = LanguageInfoStore::new();

        store.add(LanguageInfo::new("rust", "Rust").with_extensions(["rs"]));
        assert_eq!(store.len(), 1);
        assert!(!store.is_empty());

        store.add(LanguageInfo::new("markdown", "Markdown").with_extensions(["md"]));
        assert_eq!(store.len(), 2);
    }

    #[test]
    fn test_store_take_all_drains() {
        let store = LanguageInfoStore::new();
        store.add(
            LanguageInfo::new("rust", "Rust")
                .with_extensions(["rs"])
                .with_comments(CommentTokens::with_block("//", "/*", "*/")),
        );
        store.add(LanguageInfo::new("markdown", "Markdown").with_extensions(["md", "markdown"]));

        let entries = store.take_all();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].id, "rust");
        assert_eq!(entries[1].id, "markdown");

        // Store should now be empty
        assert!(store.is_empty());
        assert!(store.take_all().is_empty());
    }

    #[test]
    fn test_store_take_empty() {
        let store = LanguageInfoStore::new();
        let entries = store.take_all();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_store_service_impl() {
        fn accepts_service(_: &dyn reovim_kernel::api::v1::Service) {}
        let store = LanguageInfoStore::new();
        accepts_service(&store);
    }

    #[test]
    fn test_store_debug() {
        let store = LanguageInfoStore::new();
        store.add(LanguageInfo::new("rust", "Rust"));

        let debug = format!("{store:?}");
        assert!(debug.contains("LanguageInfoStore"));
        assert!(debug.contains("count"));
    }

    #[test]
    fn test_store_debug_empty() {
        let store = LanguageInfoStore::new();
        let debug = format!("{store:?}");
        assert!(debug.contains("LanguageInfoStore"));
        assert!(debug.contains('0'));
    }

    #[test]
    fn test_store_take_then_add_again() {
        let store = LanguageInfoStore::new();
        store.add(LanguageInfo::new("rust", "Rust"));

        let taken = store.take_all();
        assert_eq!(taken.len(), 1);
        assert!(store.is_empty());

        store.add(LanguageInfo::new("python", "Python"));
        assert_eq!(store.len(), 1);

        let taken2 = store.take_all();
        assert_eq!(taken2.len(), 1);
        assert_eq!(taken2[0].id, "python");
    }

    #[test]
    fn test_store_preserves_full_language_info() {
        let store = LanguageInfoStore::new();
        store.add(
            LanguageInfo::new("rust", "Rust")
                .with_extensions(["rs"])
                .with_mime_types(["text/x-rust"])
                .with_comments(CommentTokens::with_block("//", "/*", "*/")),
        );

        let entries = store.take_all();
        let rust = &entries[0];

        assert_eq!(rust.id, "rust");
        assert_eq!(rust.name, "Rust");
        assert!(rust.matches_extension("rs"));
        assert!(rust.matches_mime("text/x-rust"));
        assert!(rust.has_comments());
        assert!(rust.comment_tokens.has_line_comment());
        assert!(rust.comment_tokens.has_block_comment());
    }
}
