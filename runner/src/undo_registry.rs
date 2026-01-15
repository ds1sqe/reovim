//! Per-buffer undo tree storage.
//!
//! The `UndoRegistry` maintains separate undo trees for each buffer,
//! enabling per-buffer undo/redo operations while keeping undo history
//! isolated between buffers.
//!
//! # Design Philosophy
//!
//! This follows the runner-side callback pattern where:
//! - Commands (policy) declare WHAT they want via `CommandResult::UndoAction`
//! - Runner (mechanism) decides HOW via `UndoRegistry`
//!
//! This keeps the kernel's `UndoTree` as a pure mechanism while allowing
//! the runner to manage per-buffer undo trees.

use {
    reovim_kernel::api::v1::{BufferId, Edit, Position, UndoResult, UndoTree},
    std::collections::HashMap,
};

/// Registry for per-buffer undo trees.
///
/// Each buffer gets its own `UndoTree`, allowing independent undo/redo
/// histories. Trees are created lazily on first edit or undo operation.
///
/// # Example
///
/// ```ignore
/// let mut registry = UndoRegistry::new();
///
/// // Record an edit for buffer 1
/// registry.record(buffer_id, vec![edit], cursor_before, cursor_after);
///
/// // Undo the last change
/// if let Some(result) = registry.undo(buffer_id) {
///     apply_edits(&mut buffer, &result.edits);
///     buffer.set_position(result.cursor);
/// }
/// ```
#[derive(Debug, Default)]
pub struct UndoRegistry {
    trees: HashMap<BufferId, UndoTree>,
}

impl UndoRegistry {
    /// Create a new empty undo registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Get or create the undo tree for a buffer.
    ///
    /// Creates a new tree if one doesn't exist for the buffer.
    pub fn get_or_create(&mut self, buffer_id: BufferId) -> &mut UndoTree {
        self.trees.entry(buffer_id).or_default()
    }

    /// Get a read-only reference to the undo tree for a buffer.
    ///
    /// Returns `None` if no undo history exists for the buffer.
    /// Use this for visualization features like `:undotree` panel.
    #[must_use]
    pub fn get_tree(&self, buffer_id: BufferId) -> Option<&UndoTree> {
        self.trees.get(&buffer_id)
    }

    /// Undo the last change for a buffer.
    ///
    /// Returns the edits to apply and cursor position, or `None` if
    /// there's nothing to undo (at the root of the undo tree).
    pub fn undo(&mut self, buffer_id: BufferId) -> Option<UndoResult> {
        self.trees.get_mut(&buffer_id)?.undo()
    }

    /// Redo the last undone change for a buffer.
    ///
    /// Returns the edits to apply and cursor position, or `None` if
    /// there's nothing to redo.
    pub fn redo(&mut self, buffer_id: BufferId) -> Option<UndoResult> {
        self.trees.get_mut(&buffer_id)?.redo()
    }

    /// Record an edit transaction for a buffer.
    ///
    /// This should be called after making edits to a buffer so they can
    /// be undone later.
    pub fn record(
        &mut self,
        buffer_id: BufferId,
        edits: Vec<Edit>,
        cursor_before: Position,
        cursor_after: Position,
    ) {
        self.get_or_create(buffer_id)
            .push(edits, cursor_before, cursor_after);
    }

    /// Check if a buffer has any undo history.
    #[must_use]
    pub fn has_history(&self, buffer_id: BufferId) -> bool {
        self.trees.contains_key(&buffer_id)
    }

    /// Remove the undo tree for a buffer.
    ///
    /// Called when a buffer is closed to free memory.
    pub fn remove(&mut self, buffer_id: BufferId) {
        self.trees.remove(&buffer_id);
    }

    /// Get the number of buffers with undo history.
    #[must_use]
    pub fn buffer_count(&self) -> usize {
        self.trees.len()
    }
}

#[cfg(test)]
mod tests {
    use {super::*, reovim_kernel::api::v1::Edit};

    #[test]
    fn test_new_registry_empty() {
        let registry = UndoRegistry::new();
        assert_eq!(registry.buffer_count(), 0);
    }

    #[test]
    fn test_get_or_create_first_access_creates_tree() {
        let mut registry = UndoRegistry::new();
        let buffer_id = BufferId::from_raw(1);

        let _tree = registry.get_or_create(buffer_id);
        assert!(registry.has_history(buffer_id));
        assert_eq!(registry.buffer_count(), 1);
    }

    #[test]
    fn test_get_or_create_subsequent_returns_same() {
        let mut registry = UndoRegistry::new();
        let buffer_id = BufferId::from_raw(1);

        // First access creates
        registry.get_or_create(buffer_id);
        assert_eq!(registry.buffer_count(), 1);

        // Second access returns existing
        registry.get_or_create(buffer_id);
        assert_eq!(registry.buffer_count(), 1);
    }

    #[test]
    fn test_record_stores_edit_in_correct_buffer() {
        let mut registry = UndoRegistry::new();
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
        let mut registry = UndoRegistry::new();
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
        let mut registry = UndoRegistry::new();
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
        let mut registry = UndoRegistry::new();
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
    fn test_undo_empty_tree_returns_none() {
        let mut registry = UndoRegistry::new();
        let buffer_id = BufferId::from_raw(1);

        // Create tree but don't add any edits
        registry.get_or_create(buffer_id);

        // Undo should return None (at root)
        let result = registry.undo(buffer_id);
        assert!(result.is_none());
    }

    #[test]
    fn test_redo_empty_tree_returns_none() {
        let mut registry = UndoRegistry::new();
        let buffer_id = BufferId::from_raw(1);

        // Create tree but don't add any edits
        registry.get_or_create(buffer_id);

        // Redo should return None
        let result = registry.redo(buffer_id);
        assert!(result.is_none());
    }

    #[test]
    fn test_undo_nonexistent_buffer_returns_none() {
        let mut registry = UndoRegistry::new();
        let buffer_id = BufferId::from_raw(999);

        let result = registry.undo(buffer_id);
        assert!(result.is_none());
    }

    #[test]
    fn test_remove_clears_history() {
        let mut registry = UndoRegistry::new();
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
        let mut registry = UndoRegistry::new();
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
}
