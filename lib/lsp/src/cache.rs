//! Lock-free diagnostic cache using `ArcSwap`.
//!
//! This cache follows the same pattern as reovim's treesitter saturator:
//! - Saturator thread writes via atomic swap (~O(1))
//! - Render thread reads via lock-free load (~6µs)
//!
//! This ensures the render thread never blocks on diagnostic updates.

use std::{collections::HashMap, sync::Arc};

use {
    arc_swap::ArcSwap,
    lsp_types::{Diagnostic, Uri},
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
}

impl Default for DiagnosticCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        lsp_types::{DiagnosticSeverity, Position, Range},
    };

    fn make_uri(path: &str) -> Uri {
        path.parse().expect("test URI should parse")
    }

    fn make_diagnostic(message: &str, severity: DiagnosticSeverity) -> Diagnostic {
        Diagnostic {
            range: Range::new(Position::new(0, 0), Position::new(0, 10)),
            severity: Some(severity),
            message: message.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn test_store_and_get() {
        let cache = DiagnosticCache::new();
        let uri = make_uri("file:///test.rs");
        let diagnostics = vec![make_diagnostic("test error", DiagnosticSeverity::ERROR)];

        cache.store(&uri, Some(1), diagnostics);

        let result = cache.get(&uri).unwrap();
        assert_eq!(result.version, Some(1));
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].message, "test error");
    }

    #[test]
    fn test_get_nonexistent() {
        let cache = DiagnosticCache::new();
        let uri = make_uri("file:///nonexistent.rs");

        assert!(cache.get(&uri).is_none());
    }

    #[test]
    fn test_remove() {
        let cache = DiagnosticCache::new();
        let uri = make_uri("file:///test.rs");
        let diagnostics = vec![make_diagnostic("error", DiagnosticSeverity::ERROR)];

        cache.store(&uri, None, diagnostics);
        assert!(cache.has(&uri));

        cache.remove(&uri);
        assert!(!cache.has(&uri));
    }

    #[test]
    fn test_clear() {
        let cache = DiagnosticCache::new();

        for i in 0..5 {
            let uri = make_uri(&format!("file:///test{i}.rs"));
            cache.store(&uri, None, vec![]);
        }

        assert_eq!(cache.len(), 5);

        cache.clear();
        assert!(cache.is_empty());
    }

    #[test]
    fn test_update_replaces() {
        let cache = DiagnosticCache::new();
        let uri = make_uri("file:///test.rs");

        cache.store(&uri, Some(1), vec![make_diagnostic("old", DiagnosticSeverity::ERROR)]);

        cache.store(&uri, Some(2), vec![make_diagnostic("new", DiagnosticSeverity::WARNING)]);

        let result = cache.get(&uri).unwrap();
        assert_eq!(result.version, Some(2));
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].message, "new");
    }

    #[test]
    fn test_get_all() {
        let cache = DiagnosticCache::new();

        let uri1 = make_uri("file:///a.rs");
        let uri2 = make_uri("file:///b.rs");

        cache.store(&uri1, None, vec![]);
        cache.store(&uri2, None, vec![]);

        let all = cache.get_all();
        assert_eq!(all.len(), 2);
        assert!(all.contains_key(uri1.as_str()));
        assert!(all.contains_key(uri2.as_str()));
    }
}
