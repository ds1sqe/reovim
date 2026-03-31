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
//! // Edit the buffer at explicit position (cursor is per-window, not per-buffer)
//! buf.insert_at(Position::new(0, 5), "!");
//! assert_eq!(buf.line(0), Some("Hello!"));
//!
//! // Create edit for undo tracking (edits are self-contained)
//! let edit = Edit::insert(Position::new(0, 5), "!");
//! assert!(edit.is_insert());
//! assert_eq!(edit.text(), "!");
//!
//! // Get inverse for undo
//! let undo = edit.inverse();
//! assert!(undo.is_delete());
//! ```

mod buffer;
mod buffer_id;
mod cache;
mod delimiter;
mod edit;
mod position;
mod line_index;
mod piece_table;
mod rope;
// Large file offsets are u64 but Rust indexing uses usize.
// Truncation is lossless on our 64-bit-only target.
#[allow(clippy::cast_possible_truncation)]
mod virtual_buffer;

// Re-export Rope for snapshot types in block/. Not exposed via api::v1
// because the mm module itself is private.
pub use rope::Rope;
mod saturator;
mod selection;
mod snapshot;
mod tab_id;
mod window_id;
mod word;

#[cfg(test)]
mod tests;

pub use line_index::{InvalidUtf8, LineIndex};
#[allow(unused_imports)]
pub use piece_table::{Piece, PieceMetrics, PieceSource, PieceTree};
pub use virtual_buffer::{FileMapping, HeapMapping, VirtualBuffer, VirtualSnapshot};

pub use {
    buffer::Buffer,
    buffer_id::BufferId,
    cache::LineCache,
    delimiter::{find_delimiter_pair, find_matching_delimiter},
    edit::{Edit, TextDimensions, delete_end, text_dimensions, transform_position},
    position::{Cursor, Position},
    saturator::{
        RequestPriority, SaturationRequest, SaturatorConfig, SaturatorHandle, spawn_saturator,
    },
    selection::{Selection, SelectionMode},
    snapshot::BufferSnapshot,
    tab_id::TabId,
    window_id::WindowId,
    word::{
        CharKind, WordType, char_kind, next_word_end, next_word_start, word_bounds, word_end,
        word_start,
    },
};
