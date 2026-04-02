//! Memory management subsystem.
//!
//! Linux equivalent: `mm/`
//!
//! This module provides buffer storage, identifier types, and byte-level
//! data structures. Text-specific types have been extracted:
//! - Position, Edit, Selection → `reovim-types-text`
//! - `VirtualBuffer`, `HeapMapping` → `reovim-provider-text`
//!
//! # Module Structure
//!
//! - [`buffer_id`]: Unique buffer identifiers with atomic generation
//! - [`buffer`]: Core buffer data structure with line-based storage
//! - [`piece_table`]: Byte-only B-tree for large file editing
//! - [`file_mapping`]: Generic byte-access abstraction for zero-copy access
//! - [`delimiter`]: Bracket/delimiter matching
//!
//! # Example
//!
//! ```
//! use reovim_kernel::api::v1::Buffer;
//! use reovim_types_text::{Position, Edit};
//!
//! let mut buf = Buffer::from_string("Hello\nWorld");
//! assert_eq!(buf.line_count(), 2);
//! assert_eq!(buf.line(0), Some("Hello"));
//!
//! buf.insert_at(Position::new(0, 5), "!");
//! assert_eq!(buf.line(0), Some("Hello!"));
//!
//! let edit = Edit::insert(Position::new(0, 5), "!");
//! assert!(edit.is_insert());
//! ```

mod buffer;
mod buffer_id;
mod cache;
mod delimiter;
mod file_mapping;
mod line_index;
mod piece_table;
mod rope;

mod saturator;
mod snapshot;
mod tab_id;
mod window_id;

#[cfg(test)]
mod tests;

pub use {
    file_mapping::FileMapping,
    line_index::{InvalidUtf8, LineIndex},
    piece_table::{Piece, PieceMetrics, PieceSource, PieceTree},
};

pub use {
    buffer::Buffer,
    buffer_id::BufferId,
    cache::LineCache,
    delimiter::{find_delimiter_pair, find_matching_delimiter},
    saturator::{
        RequestPriority, SaturationRequest, SaturatorConfig, SaturatorHandle, spawn_saturator,
    },
    snapshot::BufferSnapshot,
    tab_id::TabId,
    window_id::WindowId,
};

// Re-exports from reovim-types-text used by kernel internals.
// Position: delimiter, jumplist, mark, api/debug.
// Cursor: kernel tests only (re-exported via api/v1 for external use).
pub use reovim_types_text::{Cursor, Position};
