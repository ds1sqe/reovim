//! Undo/redo action types for command results.
//!
//! Provides action intents for undo/redo operations and undotree visualization.

/// Undo/redo action intent returned by commands.
///
/// Commands return this to request undo/redo operations. The runner
/// handles the actual undo tree manipulation, maintaining separation
/// of concerns between command (policy) and runner (mechanism).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UndoAction {
    /// Request to undo the specified number of changes.
    Undo { count: usize },
    /// Request to redo the specified number of changes.
    Redo { count: usize },
}

/// Undotree visualization action intent returned by commands.
///
/// Commands return this to request undotree operations. The runner
/// handles the actual panel creation, navigation, and tree traversal,
/// maintaining separation of concerns between command (policy) and
/// runner (mechanism).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UndotreeAction {
    /// Toggle undotree panel for the specified buffer.
    Toggle { buffer_id: usize },
    /// Close undotree panel.
    Close,
    /// Navigate to a specific node in the undotree.
    GotoNode { node_index: usize },
    /// Go to the currently selected node.
    GotoSelected,
    /// Move selection up (toward parent).
    MoveUp,
    /// Move selection down (toward child).
    MoveDown,
    /// Preview diff of the currently selected node.
    PreviewDiff,
    /// Clear/dismiss the diff preview.
    ClearPreview,
}
