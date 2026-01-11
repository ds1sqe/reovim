//! Memory management subsystem.
//!
//! Linux equivalent: `mm/`
//!
//! This module provides the foundational data structures for text editing:
//! buffer storage, position types, and edit operations. It serves as the
//! foundation upon which all other kernel modules are built.
//!
//! # Module Structure
//!
//! - [`buffer_id`]: Unique buffer identifiers with atomic generation
//! - [`position`]: Position and cursor types for text navigation
//! - [`edit`]: Edit operations for undo/redo support
//! - [`buffer`]: Core buffer data structure with line-based storage
//!
//! # Design Philosophy
//!
//! This module follows the Linux kernel's memory management principles:
//! - Simple, efficient data structures
//! - Clear ownership semantics
//! - No external dependencies (pure Rust)
//! - Extensible foundation for advanced features
//!
//! # Example
//!
//! ```
//! use reovim_kernel::api::v1::*;
//!
//! // Create a buffer from text
//! let mut buf = Buffer::from_string("Hello\nWorld");
//! assert_eq!(buf.line_count(), 2);
//! assert_eq!(buf.line(0), Some("Hello"));
//!
//! // Edit the buffer
//! buf.set_position(Position::new(0, 5));
//! let edit = buf.insert("!");
//! assert_eq!(buf.line(0), Some("Hello!"));
//!
//! // Edits are self-contained for undo
//! assert!(edit.is_insert());
//! assert_eq!(edit.text(), "!");
//!
//! // Get inverse for undo
//! let undo = edit.inverse();
//! assert!(undo.is_delete());
//! ```

mod buffer;
mod buffer_id;
mod edit;
mod position;

#[cfg(test)]
mod tests;

pub use {
    buffer::Buffer,
    buffer_id::BufferId,
    edit::Edit,
    position::{Cursor, Position},
};
