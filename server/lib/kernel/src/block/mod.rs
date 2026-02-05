//! Block operations subsystem.
//!
//! Provides undo tree, change history, and atomic transactions.
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
//! - [`Transaction`]: Groups multiple edits for atomic undo/redo
//! - [`UndoTree`]: Branching undo history with cursor position tracking
//! - [`History`]: Change log with timestamps
//! - [`Snapshot`]: Buffer state capture for restore
//!
//! # Example
//!
//! ```
//! use reovim_kernel::api::v1::*;
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

mod history;
mod snapshot;
mod transaction;
mod undo;

#[cfg(test)]
mod tests;

pub use {
    history::{History, HistoryEntry},
    snapshot::Snapshot,
    transaction::Transaction,
    undo::{EditOrigin, UndoNode, UndoResult, UndoTree},
};
