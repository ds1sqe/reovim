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
    crate::conversion::{from_undo_tree, to_undo_tree},
    reovim_arch::sync::RwLock,
    reovim_domain_text::{Edit, EditOrigin, Position, UndoResult, UndoTree, transform_position},
    reovim_driver_text_undo::{UndoPersistError, UndoProvider},
    reovim_kernel::api::v1::BufferId,
    reovim_protocol::v1::undo::{UndoFileError, UndoFileFormat},
    reovim_subsys_vfs::VfsDriver,
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

/// Collapse a batch of accumulated edits into a minimal set (#554).
///
/// Insert mode records individual character edits with absolute positions.
/// When `UndoTree::undo()` reverses and inverts these, the positions become
/// invalid because each delete changes the buffer length. Collapsing into a
/// single bulk edit makes undo inversion trivially correct.
///
/// Strategy: simulate the edits against a local string buffer starting at the
/// batch anchor position. The result is a single `Edit::Insert` with the net
/// inserted text, or an empty vec if all edits cancel out.
fn collapse_batch_edits(edits: &[Edit], anchor: Position) -> Vec<Edit> {
    if edits.len() <= 1 {
        return edits.to_vec();
    }

    // Simulate the batch edits against a virtual buffer.
    // We track a string that represents the net change starting at `anchor`.
    // Each edit's position is relative to the real buffer, so we translate
    // to an offset within our virtual string.
    let mut net_text = String::new();
    let anchor_line = anchor.line;
    let anchor_col = anchor.column;

    for edit in edits {
        match edit {
            Edit::Insert { position, text } => {
                // Compute offset within net_text.
                // For single-line inserts within the same line as anchor,
                // offset = position.column - anchor_col (adjusted for prior edits).
                // For multiline support, we count characters.
                let offset = char_offset_in_net(
                    &net_text,
                    anchor_line,
                    anchor_col,
                    position.line,
                    position.column,
                );
                // Clamp offset to valid range
                let char_len = net_text.chars().count();
                let offset = offset.min(char_len);
                // Convert char offset to byte offset for string insertion
                let byte_offset = net_text
                    .char_indices()
                    .nth(offset)
                    .map_or(net_text.len(), |(i, _)| i);
                net_text.insert_str(byte_offset, text);
            }
            Edit::Delete { position, text } => {
                let offset = char_offset_in_net(
                    &net_text,
                    anchor_line,
                    anchor_col,
                    position.line,
                    position.column,
                );
                let char_len = net_text.chars().count();
                let offset = offset.min(char_len);
                let del_chars = text.chars().count();
                let end_offset = (offset + del_chars).min(char_len);

                // Convert char offsets to byte offsets
                let byte_start = net_text
                    .char_indices()
                    .nth(offset)
                    .map_or(net_text.len(), |(i, _)| i);
                let byte_end = net_text
                    .char_indices()
                    .nth(end_offset)
                    .map_or(net_text.len(), |(i, _)| i);

                net_text.replace_range(byte_start..byte_end, "");
            }
        }
    }

    if net_text.is_empty() {
        return Vec::new();
    }

    vec![Edit::insert(anchor, net_text)]
}

/// Compute the character offset within the net string for a given buffer position.
///
/// The net string starts at `(anchor_line, anchor_col)`. Given a target position
/// `(line, col)`, compute how many characters into the net string that maps to.
fn char_offset_in_net(
    net_text: &str,
    anchor_line: usize,
    anchor_col: usize,
    target_line: usize,
    target_col: usize,
) -> usize {
    if target_line == anchor_line {
        // Same line: offset is just column difference
        target_col.saturating_sub(anchor_col)
    } else {
        // Different line: count characters through newlines in net_text
        let line_diff = target_line - anchor_line;
        let mut lines_seen = 0;
        let mut char_count = 0;
        for ch in net_text.chars() {
            if lines_seen == line_diff {
                return char_count + target_col;
            }
            char_count += 1;
            if ch == '\n' {
                lines_seen += 1;
            }
        }
        // If we didn't find enough newlines, append at end
        char_count + target_col
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

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn end_batch(&self, buffer_id: BufferId, cursor_after: Position) {
        let batch = {
            let mut batches = self.batches.write();
            batches.remove(&buffer_id)
        };

        // If there was an active batch with edits, commit them
        if let Some(batch) = batch
            && !batch.edits.is_empty()
        {
            // #554: Collapse accumulated edits into a single edit.
            //
            // Insert mode records character-by-character edits with advancing
            // absolute positions. When UndoTree::undo() reverses and inverts
            // these, the Delete positions become invalid as the buffer shrinks.
            // Collapsing into a single Insert makes inversion trivially correct.
            let edits = collapse_batch_edits(&batch.edits, batch.cursor_before);

            let mut trees = self.trees.write();
            let tree = trees.entry(buffer_id).or_default();

            // #471: Use origin if set (for per-client undo)
            if let Some(origin) = batch.origin {
                tree.push_with_origin(edits, batch.cursor_before, cursor_after, origin);

                // Update client's cursor to new position
                if let EditOrigin::Client(client_id) = origin {
                    let key = (buffer_id, client_id);
                    let new_idx = tree.current_index();
                    drop(trees);
                    self.client_cursors.write().insert(key, new_idx);
                }
            } else {
                tree.push(edits, batch.cursor_before, cursor_after);
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

    #[cfg_attr(coverage_nightly, coverage(off))]
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

    #[cfg_attr(coverage_nightly, coverage(off))]
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
#[cfg_attr(coverage_nightly, coverage(off))]
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
#[path = "registry_tests.rs"]
mod tests;
