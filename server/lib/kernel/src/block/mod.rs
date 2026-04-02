//! Block operations subsystem.
//!
//! Provides buffer state capture and restore.
//! Linux equivalent: Block device operations, journaling.
//!
//! # Design Principle
//!
//! Kernel provides mechanisms (pure Rust), drivers provide policies
//! (serialization). Snapshot provides accessors; file I/O is handled
//! by the driver layer.
//!
//! # Components
//!
//! - [`Snapshot`]: Buffer state capture for restore
//!
//! Text-specific block operations have been extracted to `reovim-types-text`:
//! - `Transaction`: Groups multiple edits for atomic undo/redo
//! - `UndoTree`: Branching undo history with cursor position tracking
//! - `History`: Change log with timestamps
//!
//! # Example
//!
//! ```
//! use reovim_kernel::api::v1::*;
//! use reovim_types_text::{Edit, Position, UndoTree};
//!
//! // Create an undo tree
//! let mut tree = UndoTree::new();
//!
//! // Record an edit with cursor positions
//! let edit = Edit::insert(Position::new(0, 0), "Hello");
//! tree.push(
//!     vec![edit],
//!     Position::new(0, 0),  // cursor before
//!     Position::new(0, 5),  // cursor after
//! );
//!
//! // Undo returns edits to apply and cursor to restore
//! if let Some(result) = tree.undo() {
//!     assert_eq!(result.cursor, Position::new(0, 0));
//! }
//! ```

mod snapshot;

#[cfg(test)]
mod tests;

pub use snapshot::{Snapshot, SnapshotMismatch};

// Re-exports from reovim-types-text for backward compat within kernel.
// Used by block/tests/ which imports via `super::*`.
#[allow(unused_imports)]
pub use reovim_types_text::{
    EditOrigin, History, HistoryEntry, Transaction, UndoNode, UndoResult, UndoTree,
};
