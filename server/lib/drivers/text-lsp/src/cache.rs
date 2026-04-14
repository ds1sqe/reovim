//! Lock-free diagnostic cache using `ArcSwap`.
//!
//! This cache follows the same pattern as reovim's treesitter saturator:
//! - Saturator thread writes via atomic swap (~O(1))
//! - Render thread reads via lock-free load (~6µs)
//!
//! This ensures the render thread never blocks on diagnostic updates.

use std::{collections::HashMap, sync::Arc, time::Instant};

use {
    lsp_types::{Diagnostic, Uri},
    reovim_kernel::api::v1::ArcSwap,
};

/// Per-buffer diagnostic data.
#[derive(Debug, Clone, Default)]
pub struct BufferDiagnostics {
    /// Document version when diagnostics were computed.
    /// `None` if version was not provided by the server.
    pub version: Option<i32>,
    /// List of diagnostics for this buffer.
    pub diagnostics: Vec<Diagnostic>,
}

/// Internal cache storage.
/// Uses String as key instead of Uri because Uri has interior mutability.
#[derive(Debug, Default)]
struct CacheData {
    /// Map from document URI string to diagnostics.
    entries: HashMap<String, BufferDiagnostics>,
    /// When the cache was last updated.
    last_updated: Option<Instant>,
}

/// Lock-free diagnostic cache.
///
/// Uses `ArcSwap` for atomic pointer swaps, enabling:
/// - Lock-free reads from the render thread
/// - Atomic updates from the saturator thread
///
/// # Thread Safety
///
/// This cache is designed for the following access pattern:
/// - **Saturator thread**: Calls `store()` to update diagnostics
/// - **Render thread**: Calls `get()` to read diagnostics
///
/// Both operations are lock-free and never block.
#[derive(Debug)]
pub struct DiagnosticCache {
    /// Atomic pointer to current cache data.
    current: ArcSwap<CacheData>,
}

impl DiagnosticCache {
    /// Create a new empty cache.
    #[must_use]
    pub fn new() -> Self {
        Self {
            current: ArcSwap::from_pointee(CacheData::default()),
        }
    }

    /// Store diagnostics for a document.
    ///
    /// This is an atomic operation that replaces all diagnostics for the given URI.
    /// Called by the saturator thread when receiving `publishDiagnostics`.
    pub fn store(&self, uri: &Uri, version: Option<i32>, diagnostics: Vec<Diagnostic>) {
        // Load current data
        let old = self.current.load();

        // Create new data with updated entry
        let mut new_entries = old.entries.clone();
        new_entries.insert(
            uri.as_str().to_string(),
            BufferDiagnostics {
                version,
                diagnostics,
            },
        );

        // Atomic swap
        self.current.store(Arc::new(CacheData {
            entries: new_entries,
            last_updated: Some(Instant::now()),
        }));
    }

    /// Get diagnostics for a document.
    ///
    /// This is a lock-free read (~6µs) that never blocks.
    /// Called by the render thread when drawing diagnostic markers.
    #[must_use]
    pub fn get(&self, uri: &Uri) -> Option<BufferDiagnostics> {
        let data = self.current.load();
        data.entries.get(uri.as_str()).cloned()
    }

    /// Get all diagnostics.
    ///
    /// Returns a snapshot of all diagnostics in the cache.
    #[must_use]
    pub fn get_all(&self) -> HashMap<String, BufferDiagnostics> {
        let data = self.current.load();
        data.entries.clone()
    }

    /// Remove diagnostics for a document.
    ///
    /// Called when a buffer is closed.
    pub fn remove(&self, uri: &Uri) {
        let old = self.current.load();

        let mut new_entries = old.entries.clone();
        new_entries.remove(uri.as_str());

        self.current.store(Arc::new(CacheData {
            entries: new_entries,
            last_updated: old.last_updated,
        }));
    }

    /// Clear all diagnostics.
    ///
    /// Called when the language server shuts down or crashes.
    pub fn clear(&self) {
        self.current.store(Arc::new(CacheData::default()));
    }

    /// Check if there are any diagnostics for a document.
    #[must_use]
    pub fn has(&self, uri: &Uri) -> bool {
        let data = self.current.load();
        data.entries.contains_key(uri.as_str())
    }

    /// Get the count of documents with diagnostics.
    #[must_use]
    pub fn len(&self) -> usize {
        let data = self.current.load();
        data.entries.len()
    }

    /// Check if the cache is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get when the cache was last updated.
    #[must_use]
    pub fn last_updated(&self) -> Option<Instant> {
        self.current.load().last_updated
    }
}

impl Default for DiagnosticCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "cache_tests.rs"]
mod tests;
