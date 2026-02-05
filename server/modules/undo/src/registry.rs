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
    reovim_kernel::api::v1::{BufferId, Edit, Position, UndoResult, UndoTree},
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
#[derive(Debug)]
pub struct UndoRegistry {
    trees: RwLock<HashMap<BufferId, UndoTree>>,
    /// Base directory for undo files (e.g., `~/.local/share/reovim/undo/`).
    undo_dir: PathBuf,
    /// Active batches for insert mode undo grouping.
    batches: RwLock<HashMap<BufferId, PendingBatch>>,
}

/// Pending batch of edits for insert mode undo grouping.
#[derive(Debug)]
struct PendingBatch {
    /// Accumulated edits in order.
    edits: Vec<Edit>,
    /// Cursor position at batch start.
    cursor_before: Position,
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
            self.trees.write().entry(buffer_id).or_default().push(
                batch.edits,
                batch.cursor_before,
                cursor_after,
            );
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
}
