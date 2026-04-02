//! Memory management subsystem.
//!
//! Linux equivalent: `mm/`
//!
//! This module provides buffer storage, identifier types, and virtual buffer
//! abstractions. Text-specific types (Position, Edit, Selection) have been
//! extracted to `reovim-types-text` as part of #740.
//!
//! # Module Structure
//!
//! - [`buffer_id`]: Unique buffer identifiers with atomic generation
//! - [`buffer`]: Core buffer data structure with line-based storage
//! - [`virtual_buffer`]: Memory-mapped large file support
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
// Large file offsets are u64 but Rust indexing uses usize.
// Truncation is lossless on our 64-bit-only target.
#[allow(clippy::cast_possible_truncation)]
mod virtual_buffer;

// Re-export Rope for snapshot types in block/. Not exposed via api::v1
// because the mm module itself is private.
pub use rope::Rope;
mod saturator;
mod snapshot;
mod tab_id;
mod window_id;

#[cfg(test)]
mod tests;

pub use piece_table::{Piece, PieceMetrics, PieceSource, PieceTree};
pub use file_mapping::FileMapping;
pub use {
    line_index::{InvalidUtf8, LineIndex},
    virtual_buffer::{HeapMapping, VirtualBuffer, VirtualSnapshot},
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
// Position: delimiter, jumplist, mark. Edit: block/history, block/transaction.
// Cursor: kernel tests. Will be removed when these modules are extracted (Phases 2-3).
pub use reovim_types_text::{Cursor, Edit, Position};
