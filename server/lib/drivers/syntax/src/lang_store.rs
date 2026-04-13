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
#[path = "lang_store_tests.rs"]
mod tests;
