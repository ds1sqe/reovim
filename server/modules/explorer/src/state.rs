//! Per-client explorer state.
//!
//! Stored in the session's `ExtensionMap` as a `SessionExtension`.
//! Implements `TextInputSink` to receive character input when in
//! `Input` mode (for file creation, renaming, etc.).

use {
    reovim_driver_session::{SessionExtension, TextInputSink},
    std::{
        path::PathBuf,
        sync::{
            Mutex,
            atomic::{AtomicU64, Ordering},
        },
    },
};

use crate::tree::FileTree;

/// What the input buffer is being used for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExplorerInputMode {
    /// No input operation active.
    None,
    /// Creating a new file.
    CreateFile,
    /// Creating a new directory.
    CreateDir,
    /// Renaming an existing item.
    Rename,
    /// Confirming deletion (expects "y" or "n").
    ConfirmDelete,
}

impl ExplorerInputMode {
    /// Get a display label for the input prompt.
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::None => "",
            Self::CreateFile => "New file: ",
            Self::CreateDir => "New dir: ",
            Self::Rename => "Rename: ",
            Self::ConfirmDelete => "Delete? (y/n): ",
        }
    }
}

/// Per-client explorer state.
///
/// Tracks the explorer sidebar's visibility, cursor position, scroll
/// offset, and input mode. The `FileTree` requires VFS access to initialize,
/// so it starts as `None` and is populated on first toggle.
#[derive(Debug)]
pub struct ExplorerState {
    /// Whether the explorer sidebar is visible.
    pub active: bool,
    /// Index of the cursor in the flattened node list.
    pub cursor_index: usize,
    /// Scroll offset for the tree view.
    pub scroll_offset: usize,
    /// Number of visible rows in the tree area.
    pub visible_height: u16,
    /// Width of the sidebar in columns.
    pub width: u16,
    /// Whether to show hidden files (dotfiles).
    pub show_hidden: bool,
    /// Current input operation type.
    pub input_mode: ExplorerInputMode,
    /// Text buffer for input operations.
    pub input_buffer: String,
    /// Status message to display (e.g., error or confirmation).
    pub message: Option<String>,
    /// Root path for the explorer tree.
    pub root_path: PathBuf,
    /// The file tree (populated on first toggle via VFS).
    pub tree: Option<FileTree>,
    /// Cached visible node count (invalidated when tree structure changes).
    cached_visible_count: Mutex<Option<usize>>,
    /// Cached serialized nodes for bridge snapshot (invalidated with tree).
    cached_nodes_json: Mutex<Option<Vec<serde_json::Value>>>,
    /// Monotonic counter bumped on every tree-structure change.
    ///
    /// The bridge compares this with `snapshot_generation` to decide whether
    /// to include the full `"nodes"` array or emit a delta (metadata-only).
    tree_generation: AtomicU64,
    /// The `tree_generation` value at the time of the last snapshot that
    /// included the full `"nodes"` array. Starts at `u64::MAX` (sentinel)
    /// so the first snapshot always includes nodes.
    snapshot_generation: AtomicU64,
}

impl ExplorerState {
    /// Get the number of visible (flattened) nodes.
    ///
    /// Uses a cached value when available. The cache is invalidated by
    /// [`invalidate_tree_cache()`](Self::invalidate_tree_cache) when the
    /// tree structure changes.
    ///
    /// # Panics
    ///
    /// Panics if the internal cache mutex is poisoned.
    #[must_use]
    pub fn node_count(&self) -> usize {
        let mut guard = self.cached_visible_count.lock().unwrap();
        if let Some(count) = *guard {
            return count;
        }
        let count = self
            .tree
            .as_ref()
            .map_or(0, |t| t.flatten(self.show_hidden).len());
        *guard = Some(count);
        count
    }

    /// Invalidate cached tree data (node count and serialized nodes).
    ///
    /// Call this whenever the tree structure changes: expand, collapse,
    /// toggle hidden, refresh, or any file operation (create/rename/delete).
    /// Also bumps `tree_generation` so the bridge knows to send full nodes
    /// in the next snapshot.
    ///
    /// # Panics
    ///
    /// Panics if the internal cache mutex is poisoned.
    pub fn invalidate_tree_cache(&self) {
        *self.cached_visible_count.lock().unwrap() = None;
        *self.cached_nodes_json.lock().unwrap() = None;
        self.tree_generation.fetch_add(1, Ordering::Relaxed);
    }

    /// Current tree generation counter.
    ///
    /// Bumped by [`invalidate_tree_cache()`](Self::invalidate_tree_cache).
    #[must_use]
    pub fn tree_generation(&self) -> u64 {
        self.tree_generation.load(Ordering::Relaxed)
    }

    /// Generation value of the last snapshot that included full nodes.
    #[must_use]
    pub fn snapshot_generation(&self) -> u64 {
        self.snapshot_generation.load(Ordering::Relaxed)
    }

    /// Record that a full-nodes snapshot was emitted at the given generation.
    pub fn set_snapshot_generation(&self, generation: u64) {
        self.snapshot_generation
            .store(generation, Ordering::Relaxed);
    }

    /// Reset snapshot generation so the next snapshot sends full nodes.
    ///
    /// Called when the explorer deactivates — the client discards its
    /// nodes on `active: false`, so the next activation must be a full
    /// snapshot, not a delta.
    pub fn reset_snapshot_generation(&self) {
        self.snapshot_generation.store(u64::MAX, Ordering::Relaxed);
    }

    /// Get the cached serialized nodes, if available.
    ///
    /// # Panics
    ///
    /// Panics if the internal cache mutex is poisoned.
    #[must_use]
    pub fn cached_nodes_json(&self) -> Option<Vec<serde_json::Value>> {
        self.cached_nodes_json.lock().unwrap().clone()
    }

    /// Store serialized nodes in the cache.
    ///
    /// # Panics
    ///
    /// Panics if the internal cache mutex is poisoned.
    pub fn set_cached_nodes_json(&self, nodes: Vec<serde_json::Value>) {
        *self.cached_nodes_json.lock().unwrap() = Some(nodes);
    }

    /// Clamp cursor to valid range and adjust scroll to keep cursor visible.
    pub fn update_scroll(&mut self) {
        let count = self.node_count();
        self.update_scroll_with_count(count);
    }

    /// Clamp cursor and adjust scroll using a pre-computed node count.
    ///
    /// Avoids a redundant `node_count()` call when the caller already
    /// has the count.
    pub const fn update_scroll_with_count(&mut self, count: usize) {
        if count == 0 {
            self.cursor_index = 0;
            self.scroll_offset = 0;
            return;
        }

        // Clamp cursor
        if self.cursor_index >= count {
            self.cursor_index = count - 1;
        }

        // Keep cursor within the visible window
        let height = self.visible_height as usize;
        if height == 0 {
            return;
        }

        if self.cursor_index < self.scroll_offset {
            self.scroll_offset = self.cursor_index;
        } else if self.cursor_index >= self.scroll_offset + height {
            self.scroll_offset = self.cursor_index - height + 1;
        }
    }
}

impl SessionExtension for ExplorerState {
    fn create() -> Self {
        Self {
            active: false,
            cursor_index: 0,
            scroll_offset: 0,
            visible_height: 24,
            width: 30,
            show_hidden: false,
            input_mode: ExplorerInputMode::None,
            input_buffer: String::new(),
            message: None,
            root_path: PathBuf::new(),
            tree: None,
            cached_visible_count: Mutex::new(None),
            cached_nodes_json: Mutex::new(None),
            tree_generation: AtomicU64::new(0),
            snapshot_generation: AtomicU64::new(u64::MAX),
        }
    }

    fn as_text_input_sink(&mut self) -> Option<&mut dyn TextInputSink> {
        if self.active && self.input_mode != ExplorerInputMode::None {
            Some(self)
        } else {
            None
        }
    }
}

impl TextInputSink for ExplorerState {
    fn insert_char(&mut self, ch: char) {
        self.input_buffer.push(ch);
    }
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
