//! Memory management subsystem.
//!
//! Linux equivalent: `mm/`
//!
//! This module provides kernel-owned identifiers, caching, and background
//! saturation support. Text-specific and byte-mapping structures have been
//! extracted:
//! - Position, Edit, Selection → `reovim-types-text`
//! - `LineIndex`, delimiter matching → `reovim-types-text` (#740)
//! - `FileMapping`, `PieceTree` → `reovim-driver-vfs` (#740)
//! - `Buffer`, `Rope` → `reovim-provider-text` (#740)
//! - `VirtualBuffer`, `HeapMapping` → `reovim-provider-text`
//! - `BufferSnapshot` → `reovim-provider-text` (#740)
//!
//! # Module Structure
//!
//! - [`buffer_id`]: Unique buffer identifiers with atomic generation
//! - [`cache`]: Line cache for viewport/render invalidation
//! - [`saturator`]: Background work coalescing for cache and analysis tasks

mod buffer_id;
mod cache;

mod saturator;
mod tab_id;
mod window_id;

#[cfg(test)]
mod tests;

pub use {
    buffer_id::BufferId,
    cache::LineCache,
    saturator::{
        RequestPriority, SaturationRequest, SaturatorConfig, SaturatorHandle, spawn_saturator,
    },
    tab_id::TabId,
    window_id::WindowId,
};

// Re-exports from reovim-types-text used by kernel internals.
// Position: delimiter, jumplist, mark, api/debug.
// Cursor: kernel mm tests only.
pub use reovim_types_text::Position;

#[cfg(test)]
pub use reovim_types_text::Cursor;
