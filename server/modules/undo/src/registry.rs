//! Per-buffer undo tree storage with persistence.
//!
//! The `UndoRegistry` maintains separate undo trees for each buffer,
//! enabling per-buffer undo/redo operations while keeping undo history
//! isolated between buffers.
//!
//! # Design Philosophy
//!
//! This follows the mechanism/policy separation:
//! - **Mechanism**: `UndoProvider` trait (in driver)
//! - **Policy**: `UndoRegistry` (this module) with persistence

use std::path::{Path, PathBuf};

use {
    reovim_arch::sync::RwLock,
    reovim_driver_undo::{UndoPersistError, UndoProvider},
    reovim_driver_vfs::VfsDriver,
    reovim_kernel::api::v1::{
        BufferId, Edit, EditOrigin, Position, UndoResult, UndoTree, transform_position,
    },
    reovim_protocol::v1::undo::{UndoFileError, UndoFileFormat, from_undo_tree, to_undo_tree},
    std::collections::HashMap,
};

/// Default undo directory under the data directory.
const UNDO_SUBDIR: &str = "undo";

/// File extension for undo files.
const UNDO_EXTENSION: &str = ".undo";

/// Registry for per-buffer undo trees with integrated persistence.
///
/// Each buffer gets its own `UndoTree`, allowing independent undo/redo
/// histories. Trees are created lazily on first edit or undo operation.
///
/// Persistence is handled internally - the registry knows where to store
/// undo files and how to serialize/deserialize them.
///
/// # Thread Safety
///
/// Uses interior mutability via `RwLock` to allow the `UndoProvider` trait
/// methods to take `&self` while still allowing modification.
///
/// # Example
///
/// ```ignore
/// let registry = UndoRegistry::with_data_dir(Path::new("/home/user/.local/share/reovim"));
///
/// // Record an edit for buffer 1
/// registry.record(buffer_id, vec![edit], cursor_before, cursor_after);
///
/// // Undo the last change
/// if let Some(result) = registry.undo(buffer_id) {
///     apply_edits(&mut buffer, &result.edits);
///     // Cursor restored via per-client window (see #471)
///     // window.cursor = result.cursor.into();
/// }
///
/// // Persist to disk
/// registry.persist(buffer_id, "/path/to/file.rs", &vfs)?;
/// ```
/// Key for per-client undo cursor tracking.
type ClientBufferKey = (BufferId, usize);

#[derive(Debug)]
pub struct UndoRegistry {
    trees: RwLock<HashMap<BufferId, UndoTree>>,
    /// Base directory for undo files (e.g., `~/.local/share/reovim/undo/`).
    undo_dir: PathBuf,
    /// Active batches for insert mode undo grouping.
    batches: RwLock<HashMap<BufferId, PendingBatch>>,
    /// Per-client cursor positions in the undo tree (#471).
    ///
    /// Maps `(buffer_id, client_id)` to the node index representing that
    /// client's current position in the undo tree. This enables per-client
    /// undo where each client only undoes their own changes.
    client_cursors: RwLock<HashMap<ClientBufferKey, usize>>,
}

/// Pending batch of edits for insert mode undo grouping.
#[derive(Debug)]
struct PendingBatch {
    /// Accumulated edits in order.
    edits: Vec<Edit>,
    /// Cursor position at batch start.
    cursor_before: Position,
    /// Client origin for per-client undo (#471).
    /// Set when `record_for_client` contributes to the batch.
    origin: Option<EditOrigin>,
}

impl Default for UndoRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl UndoRegistry {
    /// Create a new empty undo registry with default data directory.
    ///
    /// Uses platform-specific data directory (e.g., `~/.local/share/reovim/`).
    #[must_use]
    pub fn new() -> Self {
        let data_dir = reovim_arch::dirs::data_local_dir()
            .map_or_else(|| PathBuf::from(".reovim"), |d| d.join("reovim"));
        Self::with_data_dir(&data_dir)
    }

    /// Create a new undo registry with a specific data directory.
    ///
    /// The undo directory will be `{data_dir}/undo/`.
    #[must_use]
    pub fn with_data_dir(data_dir: &Path) -> Self {
        Self {
            trees: RwLock::new(HashMap::new()),
            undo_dir: data_dir.join(UNDO_SUBDIR),
            batches: RwLock::new(HashMap::new()),
            client_cursors: RwLock::new(HashMap::new()),
        }
    }

    /// Get a read-only reference to the undo tree for a buffer.
    ///
    /// Returns `None` if no undo history exists for the buffer.
    /// Use this for visualization features like `:undotree` panel.
    ///
    /// Note: This returns a clone of the tree for thread safety.
    #[must_use]
    pub fn get_tree_cloned(&self, buffer_id: BufferId) -> Option<UndoTree> {
        self.trees.read().get(&buffer_id).cloned()
    }

    /// Set the undo tree for a buffer directly.
    ///
    /// Used when loading from disk or restoring from backup.
    pub fn set_tree(&self, buffer_id: BufferId, tree: UndoTree) {
        self.trees.write().insert(buffer_id, tree);
    }

    /// Get the undo directory path.
    #[must_use]
    pub fn undo_dir(&self) -> &Path {
        &self.undo_dir
    }

    /// Get the undo file path for a buffer's file path.
    #[must_use]
    pub fn undo_file_path(&self, buffer_path: &str) -> PathBuf {
        let encoded = encode_path_component(buffer_path);
        self.undo_dir.join(format!("{encoded}{UNDO_EXTENSION}"))
    }

    /// Ensure the undo directory exists.
    fn ensure_dir(&self, vfs: &dyn VfsDriver) -> Result<(), UndoPersistError> {
        if !vfs.exists(&self.undo_dir) {
            vfs.create_dir_all(&self.undo_dir)
                .map_err(|e| UndoPersistError::Io(format!("Failed to create undo dir: {e}")))?;
        }
        Ok(())
    }
}

impl UndoProvider for UndoRegistry {
    fn undo(&self, buffer_id: BufferId) -> Option<UndoResult> {
        self.trees.write().get_mut(&buffer_id)?.undo()
    }

    fn redo(&self, buffer_id: BufferId) -> Option<UndoResult> {
        self.trees.write().get_mut(&buffer_id)?.redo()
    }

    fn redo_branch(&self, buffer_id: BufferId, branch_idx: usize) -> Option<UndoResult> {
        self.trees
            .write()
            .get_mut(&buffer_id)?
            .redo_branch(branch_idx)
    }

    fn record(
        &self,
        buffer_id: BufferId,
        edits: Vec<Edit>,
        cursor_before: Position,
        cursor_after: Position,
    ) {
        // Check if batching is active for this buffer
        let mut batches = self.batches.write();
        if let Some(batch) = batches.get_mut(&buffer_id) {
            // Accumulate edits into the batch
            batch.edits.extend(edits);
            // cursor_before is set at batch start, cursor_after will be set at end
            return;
        }
        drop(batches);

        // No active batch - record immediately
        self.trees
            .write()
            .entry(buffer_id)
            .or_default()
            .push(edits, cursor_before, cursor_after);
    }

    fn has_history(&self, buffer_id: BufferId) -> bool {
        self.trees.read().contains_key(&buffer_id)
    }

    fn remove(&self, buffer_id: BufferId) {
        self.trees.write().remove(&buffer_id);
    }

    fn buffer_count(&self) -> usize {
        self.trees.read().len()
    }

    fn get_tree(&self, buffer_id: BufferId) -> Option<UndoTree> {
        self.get_tree_cloned(buffer_id)
    }

    fn begin_batch(&self, buffer_id: BufferId, cursor_before: Position) {
        let mut batches = self.batches.write();
        batches.insert(
            buffer_id,
            PendingBatch {
                edits: Vec::new(),
                cursor_before,
                origin: None, // Will be set when record_for_client contributes
            },
        );
    }

    fn end_batch(&self, buffer_id: BufferId, cursor_after: Position) {
        let batch = {
            let mut batches = self.batches.write();
            batches.remove(&buffer_id)
        };

        // If there was an active batch with edits, commit them
        if let Some(batch) = batch
            && !batch.edits.is_empty()
        {
            let mut trees = self.trees.write();
            let tree = trees.entry(buffer_id).or_default();

            // #471: Use origin if set (for per-client undo)
            if let Some(origin) = batch.origin {
                tree.push_with_origin(batch.edits, batch.cursor_before, cursor_after, origin);

                // Update client's cursor to new position
                if let EditOrigin::Client(client_id) = origin {
                    let key = (buffer_id, client_id);
                    let new_idx = tree.current_index();
                    drop(trees);
                    self.client_cursors.write().insert(key, new_idx);
                }
            } else {
                tree.push(batch.edits, batch.cursor_before, cursor_after);
            }
        }
    }

    fn is_batching(&self, buffer_id: BufferId) -> bool {
        self.batches.read().contains_key(&buffer_id)
    }

    fn persist(
        &self,
        buffer_id: BufferId,
        buffer_path: &str,
        vfs: &dyn VfsDriver,
    ) -> Result<(), UndoPersistError> {
        // Get a clone of the tree to release the read lock early
        let tree = {
            let trees = self.trees.read();
            match trees.get(&buffer_id) {
                Some(t) => t.clone(),
                None => {
                    // No undo history for this buffer, nothing to persist
                    return Ok(());
                }
            }
        };

        // Ensure directory exists
        self.ensure_dir(vfs)?;

        // Convert to serializable format
        let serializable = from_undo_tree(&tree);
        let format = UndoFileFormat::new(buffer_path.to_string(), serializable);

        // Serialize to bytes
        let bytes = format
            .to_bytes()
            .map_err(|e| UndoPersistError::Serialize(e.to_string()))?;

        // Write to file
        let undo_path = self.undo_file_path(buffer_path);
        vfs.write(&undo_path, &bytes)
            .map_err(|e| UndoPersistError::Io(e.to_string()))?;

        tracing::debug!("Persisted undo tree for '{}' ({} bytes)", buffer_path, bytes.len());

        Ok(())
    }

    fn load(
        &self,
        buffer_id: BufferId,
        buffer_path: &str,
        vfs: &dyn VfsDriver,
    ) -> Result<bool, UndoPersistError> {
        let undo_path = self.undo_file_path(buffer_path);

        if !vfs.exists(&undo_path) {
            return Ok(false);
        }

        // Read file
        let bytes = vfs
            .read(&undo_path)
            .map_err(|e| UndoPersistError::Io(e.to_string()))?;

        // Deserialize
        let format = UndoFileFormat::from_bytes(&bytes).map_err(|e| match e {
            UndoFileError::TooShort => UndoPersistError::Deserialize("File too short".to_string()),
            UndoFileError::InvalidMagic => {
                UndoPersistError::Deserialize("Invalid magic bytes".to_string())
            }
            UndoFileError::Deserialize(e) => UndoPersistError::Deserialize(e.to_string()),
            UndoFileError::Io(e) => UndoPersistError::Io(e.to_string()),
        })?;

        // Verify path matches (log warning if mismatch)
        if format.original_path != buffer_path {
            tracing::warn!(
                "Undo file path mismatch: expected '{}', found '{}'",
                buffer_path,
                format.original_path
            );
        }

        // Convert back to kernel type
        let tree = to_undo_tree(&format.tree);

        tracing::debug!("Loaded undo tree for '{}' ({} nodes)", buffer_path, tree.node_count());

        // Store in registry
        self.trees.write().insert(buffer_id, tree);

        Ok(true)
    }

    // ========================================================================
    // Multi-Client Undo Methods (#471)
    // ========================================================================

    fn undo_for_client(&self, buffer_id: BufferId, client_id: usize) -> Option<UndoResult> {
        let key = (buffer_id, client_id);
        let trees = self.trees.read();
        let tree = trees.get(&buffer_id)?;

        // Get client's current cursor position, or use tree's current position
        let mut cursor_pos = {
            let cursors = self.client_cursors.read();
            cursors
                .get(&key)
                .copied()
                .unwrap_or_else(|| tree.current_index())
        };

        // Navigate backwards from cursor position, looking for a node made by this client
        let target_origin = EditOrigin::Client(client_id);

        loop {
            let node = tree.node(cursor_pos)?;

            // Can't undo past root
            let parent_idx = node.parent()?;

            // Check if this node was made by the client
            if node.origin() == target_origin {
                // Found target node — compute raw inverse edits
                let inverse_edits: Vec<Edit> =
                    node.edits().iter().rev().map(Edit::inverse).collect();
                let cursor = node.cursor_before();

                // Transform through intervening edits (OT-lite, #495)
                let (edits, cursor) = match tree.edits_since(cursor_pos) {
                    Some(intervening) if !intervening.is_empty() => {
                        transform_through_intervening(&intervening, inverse_edits, cursor)
                    }
                    Some(_) => (inverse_edits, cursor),
                    None => {
                        // Target is not ancestor of current (pruned tree?)
                        // Graceful degradation: use raw inverse
                        tracing::warn!(
                            "edits_since({}) returned None for buffer {:?}, using raw inverse",
                            cursor_pos,
                            buffer_id,
                        );
                        (inverse_edits, cursor)
                    }
                };

                let result = UndoResult { edits, cursor };

                // Update client's cursor to parent
                drop(trees);
                self.client_cursors.write().insert(key, parent_idx);

                return Some(result);
            }

            // Move to parent and continue looking
            cursor_pos = parent_idx;
        }
    }

    fn redo_for_client(&self, buffer_id: BufferId, client_id: usize) -> Option<UndoResult> {
        let key = (buffer_id, client_id);
        let trees = self.trees.read();
        let tree = trees.get(&buffer_id)?;

        // Get client's current cursor position
        let cursor_pos = {
            let cursors = self.client_cursors.read();
            cursors
                .get(&key)
                .copied()
                .unwrap_or_else(|| tree.current_index())
        };

        let target_origin = EditOrigin::Client(client_id);

        // Look for a child node made by this client
        // Try to find a path forward that leads to a node made by this client
        let result = find_redo_for_client(tree, cursor_pos, target_origin);

        if let Some((node_idx, edits, cursor_after)) = result {
            drop(trees);
            self.client_cursors.write().insert(key, node_idx);

            return Some(UndoResult {
                edits,
                cursor: cursor_after,
            });
        }

        None
    }

    fn record_for_client(
        &self,
        buffer_id: BufferId,
        client_id: usize,
        edits: Vec<Edit>,
        cursor_before: Position,
        cursor_after: Position,
    ) {
        if edits.is_empty() {
            return;
        }

        let key = (buffer_id, client_id);
        let origin = EditOrigin::Client(client_id);

        // Check if batching is active for this buffer
        let mut batches = self.batches.write();
        if let Some(batch) = batches.get_mut(&buffer_id) {
            // Accumulate edits into the batch and set origin (#471)
            batch.edits.extend(edits);
            // Set origin if not already set (first contributor wins)
            if batch.origin.is_none() {
                batch.origin = Some(origin);
            }
            return;
        }
        drop(batches);

        // No active batch - record with origin
        let mut trees = self.trees.write();
        let tree = trees.entry(buffer_id).or_default();
        tree.push_with_origin(edits, cursor_before, cursor_after, origin);
        let new_idx = tree.current_index();
        drop(trees);

        // Update client's cursor to new position
        self.client_cursors.write().insert(key, new_idx);
    }

    fn init_client(&self, buffer_id: BufferId, client_id: usize) {
        let key = (buffer_id, client_id);

        // Initialize client's cursor to tree's current position
        let cursor_pos = {
            let trees = self.trees.read();
            trees.get(&buffer_id).map_or(0, UndoTree::current_index)
        };

        self.client_cursors.write().insert(key, cursor_pos);
    }

    fn remove_client(&self, buffer_id: BufferId, client_id: usize) {
        let key = (buffer_id, client_id);
        self.client_cursors.write().remove(&key);
    }
}

/// Transform inverse edits and cursor through intervening edits.
///
/// This is the core OT-lite algorithm (#495): given raw inverse edits from
/// an undo target and the edits that happened since that target, produce
/// adjusted inverse edits that apply correctly to the current buffer state.
fn transform_through_intervening(
    intervening: &[&Edit],
    mut inverse_edits: Vec<Edit>,
    mut cursor: Position,
) -> (Vec<Edit>, Position) {
    for ie in intervening {
        inverse_edits = inverse_edits.iter().map(|inv| inv.transform(ie)).collect();
        cursor = transform_position(cursor, ie);
    }
    (inverse_edits, cursor)
}

/// Find the next node to redo for a specific client.
///
/// This does a depth-first search of the tree from the given position,
/// looking for a child node made by the specified client.
fn find_redo_for_client(
    tree: &UndoTree,
    from: usize,
    target_origin: EditOrigin,
) -> Option<(usize, Vec<Edit>, Position)> {
    let node = tree.node(from)?;

    // Check each child
    for &child_idx in node.children() {
        if let Some(child_node) = tree.node(child_idx) {
            if child_node.origin() == target_origin {
                // Found a direct child made by this client
                return Some((child_idx, child_node.edits().to_vec(), child_node.cursor_after()));
            }

            // Recursively search children
            if let Some(found) = find_redo_for_client(tree, child_idx, target_origin) {
                return Some(found);
            }
        }
    }

    None
}

/// Percent-encode a file path for use as a filename.
///
/// Encodes characters that are not safe for filenames:
/// - `/` -> `%2F`
/// - `\` -> `%5C`
/// - `:` -> `%3A`
/// - `%` -> `%25` (escape the escape character)
/// - `<`, `>`, `"`, `|`, `?`, `*` -> encoded (Windows reserved)
#[must_use]
pub fn encode_path_component(path: &str) -> String {
    let mut encoded = String::with_capacity(path.len() * 2);

    for c in path.chars() {
        match c {
            '%' => encoded.push_str("%25"),
            '/' => encoded.push_str("%2F"),
            '\\' => encoded.push_str("%5C"),
            ':' => encoded.push_str("%3A"),
            '<' => encoded.push_str("%3C"),
            '>' => encoded.push_str("%3E"),
            '"' => encoded.push_str("%22"),
            '|' => encoded.push_str("%7C"),
            '?' => encoded.push_str("%3F"),
            '*' => encoded.push_str("%2A"),
            _ => encoded.push(c),
        }
    }

    encoded
}

/// Decode a percent-encoded path component.
///
/// Reverses the encoding done by [`encode_path_component`].
#[must_use]
pub fn decode_path_component(encoded: &str) -> String {
    let mut decoded = String::with_capacity(encoded.len());
    let mut chars = encoded.chars();

    while let Some(c) = chars.next() {
        if c == '%' {
            // Read two hex digits
            let hex: String = chars.by_ref().take(2).collect();
            if hex.len() == 2
                && let Ok(byte) = u8::from_str_radix(&hex, 16)
            {
                decoded.push(byte as char);
                continue;
            }
            // Invalid escape sequence, keep as-is
            decoded.push('%');
            decoded.push_str(&hex);
        } else {
            decoded.push(c);
        }
    }

    decoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_registry_empty() {
        let registry = UndoRegistry::new();
        assert_eq!(registry.buffer_count(), 0);
    }

    #[test]
    fn test_record_stores_edit_in_correct_buffer() {
        let registry = UndoRegistry::new();
        let buffer1 = BufferId::from_raw(1);
        let buffer2 = BufferId::from_raw(2);

        let edit = Edit::insert(Position::new(0, 0), "hello");

        registry.record(buffer1, vec![edit], Position::new(0, 0), Position::new(0, 5));

        assert!(registry.has_history(buffer1));
        // buffer2 should not have history yet
        assert!(!registry.has_history(buffer2));
    }

    #[test]
    fn test_undo_returns_edit() {
        let registry = UndoRegistry::new();
        let buffer_id = BufferId::from_raw(1);

        let edit = Edit::insert(Position::new(0, 0), "hello");
        registry.record(buffer_id, vec![edit], Position::new(0, 0), Position::new(0, 5));

        let result = registry.undo(buffer_id);
        assert!(result.is_some());

        let undo_result = result.unwrap();
        assert_eq!(undo_result.cursor, Position::new(0, 0));
    }

    #[test]
    fn test_redo_after_undo() {
        let registry = UndoRegistry::new();
        let buffer_id = BufferId::from_raw(1);

        let edit = Edit::insert(Position::new(0, 0), "hello");
        registry.record(buffer_id, vec![edit], Position::new(0, 0), Position::new(0, 5));

        // Undo first
        let _undo_result = registry.undo(buffer_id);

        // Now redo should work
        let redo_result = registry.redo(buffer_id);
        assert!(redo_result.is_some());

        let result = redo_result.unwrap();
        assert_eq!(result.cursor, Position::new(0, 5));
    }

    #[test]
    fn test_multiple_buffers_isolated() {
        let registry = UndoRegistry::new();
        let buffer1 = BufferId::from_raw(1);
        let buffer2 = BufferId::from_raw(2);

        // Record edits to both buffers
        let edit1 = Edit::insert(Position::new(0, 0), "hello");
        let edit2 = Edit::insert(Position::new(0, 0), "world");

        registry.record(buffer1, vec![edit1], Position::new(0, 0), Position::new(0, 5));
        registry.record(buffer2, vec![edit2], Position::new(0, 0), Position::new(0, 5));

        // Undo buffer1
        let result1 = registry.undo(buffer1);
        assert!(result1.is_some());

        // buffer2 should still have history to undo
        let result2 = registry.undo(buffer2);
        assert!(result2.is_some());
    }

    #[test]
    fn test_undo_nonexistent_buffer_returns_none() {
        let registry = UndoRegistry::new();
        let buffer_id = BufferId::from_raw(999);

        let result = registry.undo(buffer_id);
        assert!(result.is_none());
    }

    #[test]
    fn test_remove_clears_history() {
        let registry = UndoRegistry::new();
        let buffer_id = BufferId::from_raw(1);

        // Add some history
        let edit = Edit::insert(Position::new(0, 0), "hello");
        registry.record(buffer_id, vec![edit], Position::new(0, 0), Position::new(0, 5));

        assert!(registry.has_history(buffer_id));

        // Remove
        registry.remove(buffer_id);

        assert!(!registry.has_history(buffer_id));
        assert_eq!(registry.buffer_count(), 0);
    }

    #[test]
    fn test_get_tree_existing_buffer() {
        let registry = UndoRegistry::new();
        let buffer_id = BufferId::from_raw(1);

        // Record an edit to create the tree
        let edit = Edit::insert(Position::new(0, 0), "hello");
        registry.record(buffer_id, vec![edit], Position::new(0, 0), Position::new(0, 5));

        // get_tree should return Some
        let tree = registry.get_tree(buffer_id);
        assert!(tree.is_some());

        // Verify we can read tree properties
        let tree = tree.unwrap();
        assert_eq!(tree.node_count(), 2); // root + 1 edit
    }

    #[test]
    fn test_get_tree_nonexistent_buffer() {
        let registry = UndoRegistry::new();
        let buffer_id = BufferId::from_raw(999);

        // get_tree should return None for nonexistent buffer
        let tree = registry.get_tree(buffer_id);
        assert!(tree.is_none());
    }

    #[test]
    fn test_encode_path_component_unix() {
        assert_eq!(encode_path_component("/home/user/file.rs"), "%2Fhome%2Fuser%2Ffile.rs");
    }

    #[test]
    fn test_encode_path_component_windows() {
        assert_eq!(
            encode_path_component("C:\\Users\\Name\\file.txt"),
            "C%3A%5CUsers%5CName%5Cfile.txt"
        );
    }

    #[test]
    fn test_encode_decode_roundtrip() {
        let paths = [
            "/home/user/project/src/main.rs",
            "C:\\Users\\Name\\Documents\\file.txt",
            "/tmp/test%file.txt",
            "/path/with spaces/file.rs",
            "/special<>|?*chars.txt",
        ];

        for path in paths {
            let encoded = encode_path_component(path);
            let decoded = decode_path_component(&encoded);
            assert_eq!(path, decoded, "Round-trip failed for: {path}");
        }
    }

    #[test]
    fn test_undo_file_path() {
        let registry = UndoRegistry::with_data_dir(Path::new("/home/user/.local/share/reovim"));
        let undo_path = registry.undo_file_path("/home/user/project/main.rs");
        assert_eq!(
            undo_path.to_str().unwrap(),
            "/home/user/.local/share/reovim/undo/%2Fhome%2Fuser%2Fproject%2Fmain.rs.undo"
        );
    }

    #[test]
    fn test_undo_dir() {
        let registry = UndoRegistry::with_data_dir(Path::new("/data"));
        assert_eq!(registry.undo_dir(), Path::new("/data/undo"));
    }

    // ========================================================================
    // Multi-Client Undo Tests (#471)
    // ========================================================================

    #[test]
    fn test_record_for_client_tags_origin() {
        let registry = UndoRegistry::new();
        let buffer_id = BufferId::from_raw(1);
        let client_id = 42_usize;

        let edit = Edit::insert(Position::new(0, 0), "hello");
        registry.record_for_client(
            buffer_id,
            client_id,
            vec![edit],
            Position::new(0, 0),
            Position::new(0, 5),
        );

        // Verify the edit was tagged with the correct origin
        let tree = registry.get_tree(buffer_id).expect("tree should exist");
        let current = tree.current_node();
        assert_eq!(current.origin(), EditOrigin::Client(client_id));
    }

    #[test]
    fn test_undo_for_client_only_undoes_own_edits() {
        let registry = UndoRegistry::new();
        let buffer_id = BufferId::from_raw(1);
        let client_a = 1_usize;
        let client_b = 2_usize;

        // Client A makes edit 1
        let edit1 = Edit::insert(Position::new(0, 0), "AAA");
        registry.record_for_client(
            buffer_id,
            client_a,
            vec![edit1],
            Position::new(0, 0),
            Position::new(0, 3),
        );

        // Client B makes edit 2
        let edit2 = Edit::insert(Position::new(0, 3), "BBB");
        registry.record_for_client(
            buffer_id,
            client_b,
            vec![edit2],
            Position::new(0, 3),
            Position::new(0, 6),
        );

        // Client A makes edit 3
        let edit3 = Edit::insert(Position::new(0, 6), "CCC");
        registry.record_for_client(
            buffer_id,
            client_a,
            vec![edit3],
            Position::new(0, 6),
            Position::new(0, 9),
        );

        // Client A undoes - should undo their edit 3, skipping B's edit 2
        let result = registry.undo_for_client(buffer_id, client_a);
        assert!(result.is_some(), "Client A should be able to undo");

        let undo_result = result.unwrap();
        // Cursor should restore to position before edit 3
        assert_eq!(undo_result.cursor, Position::new(0, 6));
        // Should have inverse edit (delete "CCC")
        assert_eq!(undo_result.edits.len(), 1);
        assert!(matches!(&undo_result.edits[0], Edit::Delete { text, .. } if text == "CCC"));
    }

    #[test]
    fn test_undo_for_client_skips_other_clients() {
        let registry = UndoRegistry::new();
        let buffer_id = BufferId::from_raw(1);
        let client_a = 1_usize;
        let client_b = 2_usize;

        // Client A makes an edit
        let edit1 = Edit::insert(Position::new(0, 0), "AAA");
        registry.record_for_client(
            buffer_id,
            client_a,
            vec![edit1],
            Position::new(0, 0),
            Position::new(0, 3),
        );

        // Client B makes two edits
        let edit2 = Edit::insert(Position::new(0, 3), "BBB");
        registry.record_for_client(
            buffer_id,
            client_b,
            vec![edit2],
            Position::new(0, 3),
            Position::new(0, 6),
        );
        let edit3 = Edit::insert(Position::new(0, 6), "CCC");
        registry.record_for_client(
            buffer_id,
            client_b,
            vec![edit3],
            Position::new(0, 6),
            Position::new(0, 9),
        );

        // Client A undoes - should undo their edit 1, skipping B's edits
        let result = registry.undo_for_client(buffer_id, client_a);
        assert!(result.is_some());

        let undo_result = result.unwrap();
        // Should undo "AAA", not "BBB" or "CCC"
        assert_eq!(undo_result.edits.len(), 1);
        assert!(matches!(&undo_result.edits[0], Edit::Delete { text, .. } if text == "AAA"));
    }

    #[test]
    fn test_undo_for_client_returns_none_when_nothing_to_undo() {
        let registry = UndoRegistry::new();
        let buffer_id = BufferId::from_raw(1);
        let client_a = 1_usize;
        let client_b = 2_usize;

        // Only Client B makes edits
        let edit = Edit::insert(Position::new(0, 0), "BBB");
        registry.record_for_client(
            buffer_id,
            client_b,
            vec![edit],
            Position::new(0, 0),
            Position::new(0, 3),
        );

        // Client A tries to undo - should return None (nothing to undo)
        let result = registry.undo_for_client(buffer_id, client_a);
        assert!(result.is_none(), "Client A has no edits to undo");
    }

    #[test]
    fn test_init_client_sets_cursor() {
        let registry = UndoRegistry::new();
        let buffer_id = BufferId::from_raw(1);
        let client_id = 42_usize;

        // Make some edits first
        let edit = Edit::insert(Position::new(0, 0), "hello");
        registry.record(buffer_id, vec![edit], Position::new(0, 0), Position::new(0, 5));

        // Initialize client
        registry.init_client(buffer_id, client_id);

        // Client cursor should be set
        assert!(
            registry
                .client_cursors
                .read()
                .contains_key(&(buffer_id, client_id))
        );
    }

    #[test]
    fn test_remove_client_cleans_up_cursor() {
        let registry = UndoRegistry::new();
        let buffer_id = BufferId::from_raw(1);
        let client_id = 42_usize;

        // Make an edit and init client
        let edit = Edit::insert(Position::new(0, 0), "hello");
        registry.record_for_client(
            buffer_id,
            client_id,
            vec![edit],
            Position::new(0, 0),
            Position::new(0, 5),
        );

        // Client cursor should exist
        assert!(
            registry
                .client_cursors
                .read()
                .contains_key(&(buffer_id, client_id))
        );

        // Remove client
        registry.remove_client(buffer_id, client_id);

        // Client cursor should be gone
        assert!(
            !registry
                .client_cursors
                .read()
                .contains_key(&(buffer_id, client_id))
        );
    }

    // ========================================================================
    // OT-Lite Transformation Tests (#495)
    // ========================================================================

    #[test]
    fn test_undo_for_client_transforms_same_line_insert() {
        let registry = UndoRegistry::new();
        let buffer_id = BufferId::from_raw(1);
        let client_a = 1_usize;
        let client_b = 2_usize;

        // Client A inserts "AAA" at (0,5)
        registry.record_for_client(
            buffer_id,
            client_a,
            vec![Edit::insert(Position::new(0, 5), "AAA")],
            Position::new(0, 5),
            Position::new(0, 8),
        );

        // Client B inserts "BBB" at (0,0) — shifts A's text right by 3
        registry.record_for_client(
            buffer_id,
            client_b,
            vec![Edit::insert(Position::new(0, 0), "BBB")],
            Position::new(0, 0),
            Position::new(0, 3),
        );

        // Client A undoes — inverse Delete "AAA" should be at (0,8), not (0,5)
        let result = registry.undo_for_client(buffer_id, client_a).unwrap();
        assert_eq!(result.edits.len(), 1);
        assert!(result.edits[0].is_delete());
        assert_eq!(result.edits[0].text(), "AAA");
        assert_eq!(result.edits[0].position(), Position::new(0, 8));
    }

    #[test]
    fn test_undo_for_client_transforms_multiline_insert() {
        let registry = UndoRegistry::new();
        let buffer_id = BufferId::from_raw(1);
        let client_a = 1_usize;
        let client_b = 2_usize;

        // Client A inserts "AAA" at (0,0)
        registry.record_for_client(
            buffer_id,
            client_a,
            vec![Edit::insert(Position::new(0, 0), "AAA")],
            Position::new(0, 0),
            Position::new(0, 3),
        );

        // Client B inserts a multiline text "BB\nCC" at (0,3)
        registry.record_for_client(
            buffer_id,
            client_b,
            vec![Edit::insert(Position::new(0, 3), "BB\nCC")],
            Position::new(0, 3),
            Position::new(1, 2),
        );

        // Client A undoes — inverse Delete "AAA" at (0,0) is before B's insert (0,3),
        // so no shift needed. Position stays (0,0).
        let result = registry.undo_for_client(buffer_id, client_a).unwrap();
        assert_eq!(result.edits[0].position(), Position::new(0, 0));
        assert_eq!(result.edits[0].text(), "AAA");
    }

    #[test]
    fn test_undo_for_client_no_intervening_edits() {
        let registry = UndoRegistry::new();
        let buffer_id = BufferId::from_raw(1);
        let client_a = 1_usize;

        // Client A makes a single edit — no other clients
        registry.record_for_client(
            buffer_id,
            client_a,
            vec![Edit::insert(Position::new(0, 0), "AAA")],
            Position::new(0, 0),
            Position::new(0, 3),
        );

        // Undo with no intervening edits — should work exactly like Phase 2
        let result = registry.undo_for_client(buffer_id, client_a).unwrap();
        assert_eq!(result.edits.len(), 1);
        assert!(result.edits[0].is_delete());
        assert_eq!(result.edits[0].text(), "AAA");
        assert_eq!(result.edits[0].position(), Position::new(0, 0));
        assert_eq!(result.cursor, Position::new(0, 0));
    }

    #[test]
    fn test_undo_for_client_transforms_delete() {
        let registry = UndoRegistry::new();
        let buffer_id = BufferId::from_raw(1);
        let client_a = 1_usize;
        let client_b = 2_usize;

        // Client A inserts "AAA" at (0,5)
        registry.record_for_client(
            buffer_id,
            client_a,
            vec![Edit::insert(Position::new(0, 5), "AAA")],
            Position::new(0, 5),
            Position::new(0, 8),
        );

        // Client B deletes "XX" at (0,0) — shifts A's text left by 2
        registry.record_for_client(
            buffer_id,
            client_b,
            vec![Edit::delete(Position::new(0, 0), "XX")],
            Position::new(0, 0),
            Position::new(0, 0),
        );

        // Client A undoes — inverse Delete "AAA" should be at (0,3), not (0,5)
        let result = registry.undo_for_client(buffer_id, client_a).unwrap();
        assert_eq!(result.edits[0].position(), Position::new(0, 3));
        assert_eq!(result.edits[0].text(), "AAA");
    }

    #[test]
    fn test_undo_for_client_multiple_intervening_edits() {
        let registry = UndoRegistry::new();
        let buffer_id = BufferId::from_raw(1);
        let client_a = 1_usize;
        let client_b = 2_usize;
        let client_c = 3_usize;

        // Client A inserts "AAA" at (0,0)
        registry.record_for_client(
            buffer_id,
            client_a,
            vec![Edit::insert(Position::new(0, 0), "AAA")],
            Position::new(0, 0),
            Position::new(0, 3),
        );

        // Client B inserts "B1" at (0,0) — shifts A's text right by 2
        registry.record_for_client(
            buffer_id,
            client_b,
            vec![Edit::insert(Position::new(0, 0), "B1")],
            Position::new(0, 0),
            Position::new(0, 2),
        );

        // Client C inserts "C1" at (0,0) — shifts A's text right by another 2
        registry.record_for_client(
            buffer_id,
            client_c,
            vec![Edit::insert(Position::new(0, 0), "C1")],
            Position::new(0, 0),
            Position::new(0, 2),
        );

        // Client A undoes — inverse Delete "AAA" at (0,0) → (0,4) after two shifts
        let result = registry.undo_for_client(buffer_id, client_a).unwrap();
        assert_eq!(result.edits[0].position(), Position::new(0, 4));
        assert_eq!(result.edits[0].text(), "AAA");
    }

    #[test]
    fn test_undo_for_client_cursor_position_transformed() {
        let registry = UndoRegistry::new();
        let buffer_id = BufferId::from_raw(1);
        let client_a = 1_usize;
        let client_b = 2_usize;

        // Client A inserts "AAA" at (0,5), cursor_before=(0,5)
        registry.record_for_client(
            buffer_id,
            client_a,
            vec![Edit::insert(Position::new(0, 5), "AAA")],
            Position::new(0, 5),
            Position::new(0, 8),
        );

        // Client B inserts "BBB" at (0,0) — shifts cursor right by 3
        registry.record_for_client(
            buffer_id,
            client_b,
            vec![Edit::insert(Position::new(0, 0), "BBB")],
            Position::new(0, 0),
            Position::new(0, 3),
        );

        // Client A undoes — cursor should be transformed from (0,5) to (0,8)
        let result = registry.undo_for_client(buffer_id, client_a).unwrap();
        assert_eq!(result.cursor, Position::new(0, 8));
    }

    #[test]
    fn test_undo_for_client_batch_edits_in_intervening_node() {
        let registry = UndoRegistry::new();
        let buffer_id = BufferId::from_raw(1);
        let client_a = 1_usize;
        let client_b = 2_usize;

        // Client A inserts "X" at (0,5)
        registry.record_for_client(
            buffer_id,
            client_a,
            vec![Edit::insert(Position::new(0, 5), "X")],
            Position::new(0, 5),
            Position::new(0, 6),
        );

        // Client B batches two edits at (0,0) and (0,2):
        // First "AA" at (0,0), then "BB" at (0,2)
        // Total shift: +4
        registry.record_for_client(
            buffer_id,
            client_b,
            vec![
                Edit::insert(Position::new(0, 0), "AA"),
                Edit::insert(Position::new(0, 2), "BB"),
            ],
            Position::new(0, 0),
            Position::new(0, 4),
        );

        // Client A undoes — inverse Delete "X" at (0,5) → (0,9) after both batch shifts
        let result = registry.undo_for_client(buffer_id, client_a).unwrap();
        assert_eq!(result.edits[0].position(), Position::new(0, 9));
        assert_eq!(result.edits[0].text(), "X");
    }

    /// Simulate the exact insert-mode batching scenario from the integration test.
    ///
    /// Client 0 types "iAAAA<Esc>" → batch of 4 char inserts at (0,0)-(0,3)
    /// Client 1 types "0iBBBB<Esc>" → batch of 4 char inserts at (0,0)-(0,3)
    /// Client 0 undoes → should remove AAAA (now at col 4-7), not BBBB (at col 0-3)
    #[test]
    fn test_undo_for_client_batched_insert_mode_scenario() {
        let registry = UndoRegistry::new();
        let buffer_id = BufferId::from_raw(1);

        // Client 0 enters insert mode: begin_batch
        registry.begin_batch(buffer_id, Position::new(0, 0));

        // Client 0 types 'A' 4 times
        for col in 0..4_usize {
            registry.record_for_client(
                buffer_id,
                0,
                vec![Edit::insert(Position::new(0, col), "A")],
                Position::new(0, col),
                Position::new(0, col + 1),
            );
        }

        // Client 0 exits insert mode: end_batch
        registry.end_batch(buffer_id, Position::new(0, 4));

        // Client 1 enters insert mode: begin_batch
        registry.begin_batch(buffer_id, Position::new(0, 0));

        // Client 1 types 'B' 4 times (at position 0, before AAAA)
        for col in 0..4_usize {
            registry.record_for_client(
                buffer_id,
                1,
                vec![Edit::insert(Position::new(0, col), "B")],
                Position::new(0, col),
                Position::new(0, col + 1),
            );
        }

        // Client 1 exits insert mode: end_batch
        registry.end_batch(buffer_id, Position::new(0, 4));

        // Client 0 undoes — OT-lite should transform the inverse edits
        // from col 0-3 to col 4-7 (shifted by Client 1's 4 char inserts)
        let result = registry.undo_for_client(buffer_id, 0);
        assert!(result.is_some(), "undo_for_client should return Some");

        let result = result.unwrap();

        // All inverse edits should be Delete operations targeting the shifted positions
        assert_eq!(result.edits.len(), 4, "Should have 4 inverse edits");
        for edit in &result.edits {
            assert!(edit.is_delete(), "Each inverse edit should be Delete: {edit:?}");
            assert_eq!(edit.text(), "A", "Each delete should remove 'A': {edit:?}");
        }

        // After OT transformation, the positions should be shifted right by 4
        // Original inverse: Delete at (0,3), (0,2), (0,1), (0,0)
        // After transforming through 4 inserts at (0,0)-(0,3):
        // Should be Delete at (0,7), (0,6), (0,5), (0,4)
        let positions: Vec<usize> = result.edits.iter().map(|e| e.position().column).collect();
        assert!(
            positions.iter().all(|&col| col >= 4),
            "All delete positions should be >= 4 (shifted past BBBB), got: {positions:?}"
        );
    }

    #[test]
    fn test_redo_for_client_restores_own_edits() {
        let registry = UndoRegistry::new();
        let buffer_id = BufferId::from_raw(1);
        let client_a = 1_usize;

        // Client A makes an edit
        let edit = Edit::insert(Position::new(0, 0), "AAA");
        registry.record_for_client(
            buffer_id,
            client_a,
            vec![edit],
            Position::new(0, 0),
            Position::new(0, 3),
        );

        // Client A undoes
        let undo_result = registry.undo_for_client(buffer_id, client_a);
        assert!(undo_result.is_some());

        // Client A redoes
        let redo_result = registry.redo_for_client(buffer_id, client_a);
        assert!(redo_result.is_some());

        let result = redo_result.unwrap();
        assert_eq!(result.cursor, Position::new(0, 3));
        assert_eq!(result.edits.len(), 1);
        assert!(matches!(&result.edits[0], Edit::Insert { text, .. } if text == "AAA"));
    }

    // ========================================================================
    // Additional Coverage Tests
    // ========================================================================

    #[test]
    fn test_default_registry() {
        let registry = UndoRegistry::default();
        assert_eq!(registry.buffer_count(), 0);
    }

    #[test]
    fn test_with_data_dir() {
        let registry = UndoRegistry::with_data_dir(Path::new("/custom/path"));
        assert_eq!(registry.undo_dir(), Path::new("/custom/path/undo"));
    }

    #[test]
    fn test_set_tree() {
        let registry = UndoRegistry::new();
        let buffer_id = BufferId::from_raw(1);

        // No tree initially
        assert!(registry.get_tree_cloned(buffer_id).is_none());

        // Set a tree directly
        let tree = UndoTree::default();
        registry.set_tree(buffer_id, tree);

        // Now it should exist
        assert!(registry.get_tree_cloned(buffer_id).is_some());
        assert!(registry.has_history(buffer_id));
    }

    #[test]
    fn test_redo_nonexistent_buffer_returns_none() {
        let registry = UndoRegistry::new();
        let buffer_id = BufferId::from_raw(999);
        assert!(registry.redo(buffer_id).is_none());
    }

    #[test]
    fn test_redo_without_undo_returns_none() {
        let registry = UndoRegistry::new();
        let buffer_id = BufferId::from_raw(1);

        let edit = Edit::insert(Position::new(0, 0), "hello");
        registry.record(buffer_id, vec![edit], Position::new(0, 0), Position::new(0, 5));

        // Redo without prior undo should return None
        assert!(registry.redo(buffer_id).is_none());
    }

    #[test]
    fn test_redo_branch_nonexistent_buffer() {
        let registry = UndoRegistry::new();
        let buffer_id = BufferId::from_raw(999);
        assert!(registry.redo_branch(buffer_id, 0).is_none());
    }

    #[test]
    fn test_remove_nonexistent_buffer_is_noop() {
        let registry = UndoRegistry::new();
        let buffer_id = BufferId::from_raw(999);
        // Should not panic
        registry.remove(buffer_id);
        assert_eq!(registry.buffer_count(), 0);
    }

    #[test]
    fn test_begin_end_batch_empty() {
        let registry = UndoRegistry::new();
        let buffer_id = BufferId::from_raw(1);

        // Begin batch with no edits
        registry.begin_batch(buffer_id, Position::new(0, 0));
        assert!(registry.is_batching(buffer_id));

        // End batch with no edits - should not create a tree entry
        registry.end_batch(buffer_id, Position::new(0, 0));
        assert!(!registry.is_batching(buffer_id));
        assert!(!registry.has_history(buffer_id));
    }

    #[test]
    fn test_begin_end_batch_with_edits() {
        let registry = UndoRegistry::new();
        let buffer_id = BufferId::from_raw(1);

        registry.begin_batch(buffer_id, Position::new(0, 0));

        // Record during batch - should accumulate
        let edit = Edit::insert(Position::new(0, 0), "hello");
        registry.record(buffer_id, vec![edit], Position::new(0, 0), Position::new(0, 5));

        // During batch, tree should not have entries yet
        // (record accumulates into batch instead of directly into tree)
        assert!(!registry.has_history(buffer_id));

        // End batch
        registry.end_batch(buffer_id, Position::new(0, 5));

        // Now tree should exist with the edit
        assert!(registry.has_history(buffer_id));
    }

    #[test]
    fn test_is_batching_false_by_default() {
        let registry = UndoRegistry::new();
        let buffer_id = BufferId::from_raw(1);
        assert!(!registry.is_batching(buffer_id));
    }

    #[test]
    fn test_record_for_client_empty_edits_is_noop() {
        let registry = UndoRegistry::new();
        let buffer_id = BufferId::from_raw(1);

        registry.record_for_client(buffer_id, 1, vec![], Position::new(0, 0), Position::new(0, 0));

        assert!(!registry.has_history(buffer_id));
    }

    #[test]
    fn test_init_client_no_tree() {
        let registry = UndoRegistry::new();
        let buffer_id = BufferId::from_raw(1);
        let client_id = 1_usize;

        // Init client before any edits - should set cursor to 0
        registry.init_client(buffer_id, client_id);
        assert!(
            registry
                .client_cursors
                .read()
                .contains_key(&(buffer_id, client_id))
        );
    }

    #[test]
    fn test_undo_for_client_nonexistent_buffer() {
        let registry = UndoRegistry::new();
        let result = registry.undo_for_client(BufferId::from_raw(999), 1);
        assert!(result.is_none());
    }

    #[test]
    fn test_redo_for_client_nonexistent_buffer() {
        let registry = UndoRegistry::new();
        let result = registry.redo_for_client(BufferId::from_raw(999), 1);
        assert!(result.is_none());
    }

    #[test]
    fn test_redo_for_client_no_child() {
        let registry = UndoRegistry::new();
        let buffer_id = BufferId::from_raw(1);
        let client_a = 1_usize;

        // Client A makes an edit (no undo, so no redo target)
        let edit = Edit::insert(Position::new(0, 0), "AAA");
        registry.record_for_client(
            buffer_id,
            client_a,
            vec![edit],
            Position::new(0, 0),
            Position::new(0, 3),
        );

        // Without undo, redo should return None
        let result = registry.redo_for_client(buffer_id, client_a);
        assert!(result.is_none());
    }

    #[test]
    fn test_encode_path_component_percent() {
        assert_eq!(encode_path_component("100%done"), "100%25done");
    }

    #[test]
    fn test_encode_path_component_all_special_chars() {
        let encoded = encode_path_component(r#"/<>"|?*:\test"#);
        assert!(encoded.contains("%2F"));
        assert!(encoded.contains("%3C"));
        assert!(encoded.contains("%3E"));
        assert!(encoded.contains("%22"));
        assert!(encoded.contains("%7C"));
        assert!(encoded.contains("%3F"));
        assert!(encoded.contains("%2A"));
        assert!(encoded.contains("%3A"));
        assert!(encoded.contains("%5C"));
    }

    #[test]
    fn test_encode_path_component_no_special_chars() {
        assert_eq!(encode_path_component("hello.rs"), "hello.rs");
    }

    #[test]
    fn test_decode_path_component_invalid_hex() {
        // Invalid hex after % - should keep as-is
        let decoded = decode_path_component("%ZZ");
        assert_eq!(decoded, "%ZZ");
    }

    #[test]
    fn test_decode_path_component_truncated_percent() {
        // Only one char after % instead of two
        let decoded = decode_path_component("%2");
        assert_eq!(decoded, "%2");
    }

    #[test]
    fn test_decode_path_component_plain_text() {
        assert_eq!(decode_path_component("hello.rs"), "hello.rs");
    }

    #[test]
    fn test_undo_file_path_windows() {
        let registry = UndoRegistry::with_data_dir(Path::new("/data"));
        let undo_path = registry.undo_file_path("C:\\Users\\file.txt");
        let path_str = undo_path.to_str().unwrap();
        assert!(path_str.starts_with("/data/undo/"));
        assert!(
            Path::new(path_str)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("undo"))
        );
        assert!(path_str.contains("%3A"));
        assert!(path_str.contains("%5C"));
    }

    #[test]
    fn test_multiple_undo_redo_cycles() {
        let registry = UndoRegistry::new();
        let buffer_id = BufferId::from_raw(1);

        // Record two edits
        let edit1 = Edit::insert(Position::new(0, 0), "hello");
        registry.record(buffer_id, vec![edit1], Position::new(0, 0), Position::new(0, 5));

        let edit2 = Edit::insert(Position::new(0, 5), " world");
        registry.record(buffer_id, vec![edit2], Position::new(0, 5), Position::new(0, 11));

        // Undo twice
        let r1 = registry.undo(buffer_id);
        assert!(r1.is_some());
        let r2 = registry.undo(buffer_id);
        assert!(r2.is_some());

        // Undo a third time should fail (at root)
        let r3 = registry.undo(buffer_id);
        assert!(r3.is_none());

        // Redo twice
        let r4 = registry.redo(buffer_id);
        assert!(r4.is_some());
        let r5 = registry.redo(buffer_id);
        assert!(r5.is_some());

        // Redo a third time should fail (at latest)
        let r6 = registry.redo(buffer_id);
        assert!(r6.is_none());
    }

    #[test]
    fn test_buffer_count_tracks_correctly() {
        let registry = UndoRegistry::new();
        assert_eq!(registry.buffer_count(), 0);

        let b1 = BufferId::from_raw(1);
        let b2 = BufferId::from_raw(2);

        registry.record(
            b1,
            vec![Edit::insert(Position::new(0, 0), "a")],
            Position::new(0, 0),
            Position::new(0, 1),
        );
        assert_eq!(registry.buffer_count(), 1);

        registry.record(
            b2,
            vec![Edit::insert(Position::new(0, 0), "b")],
            Position::new(0, 0),
            Position::new(0, 1),
        );
        assert_eq!(registry.buffer_count(), 2);

        registry.remove(b1);
        assert_eq!(registry.buffer_count(), 1);

        registry.remove(b2);
        assert_eq!(registry.buffer_count(), 0);
    }

    #[test]
    fn test_record_for_client_during_batch_sets_origin() {
        let registry = UndoRegistry::new();
        let buffer_id = BufferId::from_raw(1);
        let client_id = 42_usize;

        registry.begin_batch(buffer_id, Position::new(0, 0));

        registry.record_for_client(
            buffer_id,
            client_id,
            vec![Edit::insert(Position::new(0, 0), "a")],
            Position::new(0, 0),
            Position::new(0, 1),
        );

        registry.end_batch(buffer_id, Position::new(0, 1));

        // Verify origin was set on the batch
        let tree = registry.get_tree(buffer_id).expect("tree should exist");
        let current = tree.current_node();
        assert_eq!(current.origin(), EditOrigin::Client(client_id));
    }

    // ========================================================================
    // Persistence Tests (persist/load via MockVfs)
    // ========================================================================

    #[test]
    fn test_persist_and_load_roundtrip() {
        use reovim_driver_vfs::MockVfs;

        let registry = UndoRegistry::with_data_dir(Path::new("/test-data"));
        let buffer_id = BufferId::from_raw(1);
        let buffer_path = "/home/user/file.rs";

        // Record some edits
        let edit = Edit::insert(Position::new(0, 0), "hello");
        registry.record(buffer_id, vec![edit], Position::new(0, 0), Position::new(0, 5));

        let vfs = MockVfs::new();

        // Persist the undo tree
        let result = registry.persist(buffer_id, buffer_path, &vfs);
        assert!(result.is_ok(), "persist should succeed");

        // Verify directory was created and file was written
        let undo_path = registry.undo_file_path(buffer_path);
        assert!(vfs.exists(&undo_path), "undo file should exist after persist");

        // Load into a new registry
        let registry2 = UndoRegistry::with_data_dir(Path::new("/test-data"));
        let buffer_id2 = BufferId::from_raw(2);
        let loaded = registry2.load(buffer_id2, buffer_path, &vfs);
        assert!(loaded.is_ok());
        assert!(loaded.unwrap(), "load should return true for existing file");
        assert!(registry2.has_history(buffer_id2));
    }

    #[test]
    fn test_persist_no_history_is_noop() {
        use reovim_driver_vfs::MockVfs;

        let registry = UndoRegistry::with_data_dir(Path::new("/test-data"));
        let buffer_id = BufferId::from_raw(99);
        let vfs = MockVfs::new();

        // Persist with no history - should succeed without writing
        let result = registry.persist(buffer_id, "/some/file.rs", &vfs);
        assert!(result.is_ok());
        assert!(vfs.write_calls().is_empty(), "no writes should occur for empty history");
    }

    #[test]
    fn test_load_nonexistent_file_returns_false() {
        use reovim_driver_vfs::MockVfs;

        let registry = UndoRegistry::with_data_dir(Path::new("/test-data"));
        let buffer_id = BufferId::from_raw(1);
        let vfs = MockVfs::new();

        let loaded = registry.load(buffer_id, "/no/such/file.rs", &vfs);
        assert!(loaded.is_ok());
        assert!(!loaded.unwrap(), "load should return false for nonexistent file");
    }

    #[test]
    fn test_load_invalid_data_returns_error() {
        use reovim_driver_vfs::MockVfs;

        let registry = UndoRegistry::with_data_dir(Path::new("/test-data"));
        let buffer_id = BufferId::from_raw(1);
        let vfs = MockVfs::new();

        // Write garbage data to the undo file
        let undo_path = registry.undo_file_path("/file.rs");
        vfs.add_file(&undo_path, b"not a valid undo file");

        let loaded = registry.load(buffer_id, "/file.rs", &vfs);
        assert!(loaded.is_err(), "load of invalid data should return error");
    }

    #[test]
    fn test_persist_with_io_error() {
        use reovim_driver_vfs::{MockErrorKind, MockVfs};

        let registry = UndoRegistry::with_data_dir(Path::new("/test-data"));
        let buffer_id = BufferId::from_raw(1);
        let buffer_path = "/file.rs";

        let edit = Edit::insert(Position::new(0, 0), "hello");
        registry.record(buffer_id, vec![edit], Position::new(0, 0), Position::new(0, 5));

        let vfs = MockVfs::new();
        // Make the undo file path fail on write
        let undo_path = registry.undo_file_path(buffer_path);
        vfs.set_error(&undo_path, MockErrorKind::PermissionDenied);

        let result = registry.persist(buffer_id, buffer_path, &vfs);
        assert!(result.is_err(), "persist should fail on write error");
    }

    #[test]
    fn test_load_with_io_error() {
        use reovim_driver_vfs::{MockErrorKind, MockVfs};

        let registry = UndoRegistry::with_data_dir(Path::new("/test-data"));
        let buffer_id = BufferId::from_raw(1);
        let buffer_path = "/file.rs";
        let vfs = MockVfs::new();

        // Add file but make read fail
        let undo_path = registry.undo_file_path(buffer_path);
        vfs.add_file(&undo_path, b"dummy");
        vfs.set_error(&undo_path, MockErrorKind::PermissionDenied);

        let loaded = registry.load(buffer_id, buffer_path, &vfs);
        assert!(loaded.is_err(), "load should fail on read error");
    }

    #[test]
    fn test_end_batch_without_begin_is_noop() {
        let registry = UndoRegistry::new();
        let buffer_id = BufferId::from_raw(1);

        // end_batch with no active batch should be a no-op
        registry.end_batch(buffer_id, Position::new(0, 0));
        assert!(!registry.has_history(buffer_id));
    }

    #[test]
    fn test_end_batch_with_client_origin_updates_cursor() {
        let registry = UndoRegistry::new();
        let buffer_id = BufferId::from_raw(1);
        let client_id = 7_usize;

        registry.begin_batch(buffer_id, Position::new(0, 0));

        // Record via client so origin gets set
        registry.record_for_client(
            buffer_id,
            client_id,
            vec![Edit::insert(Position::new(0, 0), "abc")],
            Position::new(0, 0),
            Position::new(0, 3),
        );

        registry.end_batch(buffer_id, Position::new(0, 3));

        // Verify client cursor was updated
        let key = (buffer_id, client_id);
        let contains = registry.client_cursors.read().contains_key(&key);
        assert!(contains, "client cursor should be set after batch end");
    }

    #[test]
    fn test_record_for_client_during_batch_first_origin_wins() {
        let registry = UndoRegistry::new();
        let buffer_id = BufferId::from_raw(1);

        registry.begin_batch(buffer_id, Position::new(0, 0));

        // First client contributes
        registry.record_for_client(
            buffer_id,
            1,
            vec![Edit::insert(Position::new(0, 0), "a")],
            Position::new(0, 0),
            Position::new(0, 1),
        );

        // Second client contributes to same batch
        registry.record_for_client(
            buffer_id,
            2,
            vec![Edit::insert(Position::new(0, 1), "b")],
            Position::new(0, 1),
            Position::new(0, 2),
        );

        registry.end_batch(buffer_id, Position::new(0, 2));

        // First contributor's origin should win
        let tree = registry.get_tree(buffer_id).expect("tree should exist");
        let current = tree.current_node();
        assert_eq!(current.origin(), EditOrigin::Client(1));
    }

    #[test]
    fn test_redo_for_client_finds_recursive_child() {
        let registry = UndoRegistry::new();
        let buffer_id = BufferId::from_raw(1);
        let client_a = 1_usize;
        let client_b = 2_usize;

        // Client A makes edit, then client B, then client A again
        registry.record_for_client(
            buffer_id,
            client_a,
            vec![Edit::insert(Position::new(0, 0), "A1")],
            Position::new(0, 0),
            Position::new(0, 2),
        );
        registry.record_for_client(
            buffer_id,
            client_b,
            vec![Edit::insert(Position::new(0, 2), "B1")],
            Position::new(0, 2),
            Position::new(0, 4),
        );
        registry.record_for_client(
            buffer_id,
            client_a,
            vec![Edit::insert(Position::new(0, 4), "A2")],
            Position::new(0, 4),
            Position::new(0, 6),
        );

        // Undo A2 and A1
        let r1 = registry.undo_for_client(buffer_id, client_a);
        assert!(r1.is_some(), "first undo should succeed");
        let r2 = registry.undo_for_client(buffer_id, client_a);
        assert!(r2.is_some(), "second undo should succeed");

        // Redo should find A1 (not B1, skipping over B1)
        let redo = registry.redo_for_client(buffer_id, client_a);
        assert!(redo.is_some(), "redo should find client A's edit");
    }

    #[test]
    fn test_persist_load_path_mismatch_still_loads() {
        use reovim_driver_vfs::MockVfs;

        let registry = UndoRegistry::with_data_dir(Path::new("/test-data"));
        let buffer_id = BufferId::from_raw(1);
        let original_path = "/home/user/original.rs";

        let edit = Edit::insert(Position::new(0, 0), "test");
        registry.record(buffer_id, vec![edit], Position::new(0, 0), Position::new(0, 4));

        let vfs = MockVfs::new();
        registry.persist(buffer_id, original_path, &vfs).unwrap();

        // Load using a different buffer path but same undo file
        // We manually move the undo file to simulate path mismatch
        let undo_path = registry.undo_file_path(original_path);
        let undo_bytes = vfs.read(&undo_path).unwrap();

        let different_path = "/home/user/moved.rs";
        let different_undo_path = registry.undo_file_path(different_path);
        vfs.add_file(&different_undo_path, &undo_bytes);

        let registry2 = UndoRegistry::with_data_dir(Path::new("/test-data"));
        let buffer_id2 = BufferId::from_raw(2);
        // This will trigger the path mismatch warning but should still load
        let loaded = registry2.load(buffer_id2, different_path, &vfs);
        assert!(loaded.is_ok());
        assert!(loaded.unwrap(), "load should succeed despite path mismatch");
        assert!(registry2.has_history(buffer_id2));
    }

    #[test]
    fn test_ensure_dir_already_exists() {
        use reovim_driver_vfs::MockVfs;

        let registry = UndoRegistry::with_data_dir(Path::new("/test-data"));
        let buffer_id = BufferId::from_raw(1);
        let buffer_path = "/file.rs";

        let edit = Edit::insert(Position::new(0, 0), "x");
        registry.record(buffer_id, vec![edit], Position::new(0, 0), Position::new(0, 1));

        let vfs = MockVfs::new();
        // Pre-create the undo directory
        vfs.add_dir("/test-data/undo");

        let result = registry.persist(buffer_id, buffer_path, &vfs);
        assert!(result.is_ok());
    }

    #[test]
    fn test_transform_through_intervening_empty() {
        // Test the helper directly with empty intervening edits
        let inverse_edits = vec![Edit::delete(Position::new(0, 5), "AAA")];
        let cursor = Position::new(0, 5);
        let (edits, pos) = transform_through_intervening(&[], inverse_edits, cursor);
        assert_eq!(edits.len(), 1);
        assert_eq!(pos, Position::new(0, 5));
    }
}
