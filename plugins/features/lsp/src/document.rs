//! Document state tracking for LSP synchronization.
//!
//! Tracks per-buffer state needed for LSP document lifecycle:
//! - URI mapping (`buffer_id` -> file:// URI)
//! - Version numbers (increment on each change)
//! - Pending sync timestamps (for debouncing)

use std::{
    collections::HashMap,
    path::PathBuf,
    time::{Duration, Instant},
};

use reovim_lsp::{Uri, uri_from_path};

/// State for a single document being synchronized with the LSP server.
#[derive(Debug)]
pub struct DocumentState {
    /// File path.
    pub path: PathBuf,
    /// Document URI (file:// format).
    pub uri: Uri,
    /// Language ID for the document (e.g., "rust").
    pub language_id: String,
    /// Document version (increments on each change).
    pub version: i32,
    /// Whether didOpen has been sent.
    pub opened: bool,
}

impl DocumentState {
    /// Create a new document state.
    #[must_use]
    pub fn new(path: PathBuf, language_id: String) -> Self {
        let uri = uri_from_path(&path);
        Self {
            path,
            uri,
            language_id,
            version: 0,
            opened: false,
        }
    }

    /// Increment version and return the new version.
    pub const fn increment_version(&mut self) -> i32 {
        self.version += 1;
        self.version
    }
}

/// Manages document state for all open buffers.
#[derive(Debug)]
pub struct DocumentManager {
    /// Map from `buffer_id` to document state.
    documents: HashMap<usize, DocumentState>,
    /// Pending sync requests (`buffer_id` -> timestamp).
    pending_syncs: HashMap<usize, Instant>,
    /// Debounce duration for syncing (default: 50ms like treesitter).
    debounce_duration: Duration,
}

impl DocumentManager {
    /// Default debounce duration (50ms, same as treesitter).
    pub const DEFAULT_DEBOUNCE_MS: u64 = 50;

    /// Create a new document manager.
    #[must_use]
    pub fn new() -> Self {
        Self {
            documents: HashMap::new(),
            pending_syncs: HashMap::new(),
            debounce_duration: Duration::from_millis(Self::DEFAULT_DEBOUNCE_MS),
        }
    }

    /// Register a new document for a buffer.
    ///
    /// Returns the document state if this is a supported language.
    pub fn open_document(&mut self, buffer_id: usize, path: PathBuf) -> Option<&DocumentState> {
        // Detect language from file extension
        let language_id = detect_language(&path)?;

        let state = DocumentState::new(path, language_id);
        self.documents.insert(buffer_id, state);
        self.documents.get(&buffer_id)
    }

    /// Mark a document as opened (didOpen sent).
    pub fn mark_opened(&mut self, buffer_id: usize) {
        if let Some(doc) = self.documents.get_mut(&buffer_id) {
            doc.opened = true;
        }
    }

    /// Schedule a sync for the given buffer (debounced).
    ///
    /// Returns the new version number if the document exists.
    pub fn schedule_sync(&mut self, buffer_id: usize) -> Option<i32> {
        let doc = self.documents.get_mut(&buffer_id)?;

        // Increment version
        let version = doc.increment_version();

        // Schedule sync with current timestamp
        self.pending_syncs.insert(buffer_id, Instant::now());

        Some(version)
    }

    /// Schedule an immediate sync for the given buffer (no debouncing).
    ///
    /// This is used for the initial didOpen sync which should happen immediately.
    /// Returns the new version number if the document exists.
    pub fn schedule_immediate_sync(&mut self, buffer_id: usize) -> Option<i32> {
        let doc = self.documents.get_mut(&buffer_id)?;

        // Increment version
        let version = doc.increment_version();

        // Schedule sync with a timestamp far enough in the past to bypass debounce
        // Use checked_sub to handle edge cases, falling back to current time
        let sync_time = Instant::now()
            .checked_sub(self.debounce_duration)
            .unwrap_or_else(Instant::now);
        self.pending_syncs.insert(buffer_id, sync_time);

        Some(version)
    }

    /// Get buffer IDs that are ready for syncing (debounce elapsed).
    #[must_use]
    pub fn get_ready_syncs(&mut self) -> Vec<usize> {
        let now = Instant::now();

        let ready: Vec<usize> = self
            .pending_syncs
            .iter()
            .filter(|(_, time)| now.duration_since(**time) >= self.debounce_duration)
            .map(|(id, _)| *id)
            .collect();

        // Remove synced entries
        for id in &ready {
            self.pending_syncs.remove(id);
        }

        ready
    }

    /// Check if there are any pending syncs.
    #[must_use]
    pub fn has_pending_syncs(&self) -> bool {
        !self.pending_syncs.is_empty()
    }

    /// Get document state for a buffer.
    #[must_use]
    pub fn get(&self, buffer_id: usize) -> Option<&DocumentState> {
        self.documents.get(&buffer_id)
    }

    /// Remove a document when buffer is closed.
    pub fn close_document(&mut self, buffer_id: usize) -> Option<DocumentState> {
        self.pending_syncs.remove(&buffer_id);
        self.documents.remove(&buffer_id)
    }

    /// Check if a buffer has a tracked document.
    #[must_use]
    pub fn has_document(&self, buffer_id: usize) -> bool {
        self.documents.contains_key(&buffer_id)
    }

    /// Get all buffer IDs with tracked documents.
    #[must_use]
    pub fn buffer_ids(&self) -> Vec<usize> {
        self.documents.keys().copied().collect()
    }
}

impl Default for DocumentManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Detect language ID from file path.
///
/// Currently only supports Rust. Returns `None` for unsupported languages.
#[must_use]
fn detect_language(path: &std::path::Path) -> Option<String> {
    let extension = path.extension()?.to_str()?;

    match extension {
        "rs" => Some("rust".to_string()),
        // Future: add more languages here
        // "ts" | "tsx" => Some("typescript".to_string()),
        // "js" | "jsx" => Some("javascript".to_string()),
        // "py" => Some("python".to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use {super::*, std::thread::sleep};

    #[test]
    fn test_open_document_rust() {
        let mut manager = DocumentManager::new();
        let path = PathBuf::from("/home/user/project/src/main.rs");

        let doc = manager.open_document(1, path);
        assert!(doc.is_some());

        let doc = doc.unwrap();
        assert_eq!(doc.language_id, "rust");
        assert_eq!(doc.version, 0);
        assert!(!doc.opened);
    }

    #[test]
    fn test_open_document_unsupported() {
        let mut manager = DocumentManager::new();
        let path = PathBuf::from("/home/user/project/README.md");

        let doc = manager.open_document(1, path);
        assert!(doc.is_none());
    }

    #[test]
    fn test_schedule_sync_increments_version() {
        let mut manager = DocumentManager::new();
        let path = PathBuf::from("/home/user/project/src/main.rs");

        manager.open_document(1, path);

        let v1 = manager.schedule_sync(1);
        assert_eq!(v1, Some(1));

        let v2 = manager.schedule_sync(1);
        assert_eq!(v2, Some(2));
    }

    #[test]
    fn test_debounce() {
        let mut manager = DocumentManager::new();
        manager.debounce_duration = Duration::from_millis(10);

        let path = PathBuf::from("/home/user/project/src/main.rs");
        manager.open_document(1, path);
        manager.schedule_sync(1);

        // Should not be ready immediately
        let ready = manager.get_ready_syncs();
        assert!(ready.is_empty());

        // Wait for debounce
        sleep(Duration::from_millis(15));

        // Now should be ready
        let ready = manager.get_ready_syncs();
        assert_eq!(ready, vec![1]);

        // Should be removed after getting ready
        let ready = manager.get_ready_syncs();
        assert!(ready.is_empty());
    }

    #[test]
    fn test_close_document() {
        let mut manager = DocumentManager::new();
        let path = PathBuf::from("/home/user/project/src/main.rs");

        manager.open_document(1, path);
        manager.schedule_sync(1);

        assert!(manager.has_document(1));
        assert!(manager.has_pending_syncs());

        let closed = manager.close_document(1);
        assert!(closed.is_some());
        assert!(!manager.has_document(1));
        assert!(!manager.has_pending_syncs());
    }
}
