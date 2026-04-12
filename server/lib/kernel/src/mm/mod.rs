//! Memory management subsystem.
//!
//! Linux equivalent: `mm/`
//!
//! This module provides kernel-owned identifiers and background
//! saturation support. Text-specific and byte-mapping structures have been
//! extracted:
//! - Position, Edit, Selection → `reovim-domain-text`
//! - `LineIndex`, delimiter matching → `reovim-domain-text` (#740)
//! - `FileMapping`, `PieceTree` → `reovim-driver-vfs` (#740)
//! - `Buffer`, `Rope` → `reovim-provider-text` (#740)
//! - `VirtualBuffer`, `HeapMapping` → `reovim-provider-text`
//! - `BufferSnapshot` → `reovim-provider-text` (#740)
//!
//! # Module Structure
//!
//! - [`buffer_id`]: Unique buffer identifiers with atomic generation
//! - [`saturator`]: Background work coalescing for cache and analysis tasks

mod buffer_id;

mod saturator;
mod tab_id;
mod window_id;

#[cfg(test)]
mod tests;

pub use {
    buffer_id::BufferId,
    saturator::{
        RequestPriority, SaturationRequest, SaturatorConfig, SaturatorHandle, spawn_saturator,
    },
    tab_id::TabId,
    window_id::WindowId,
};

// Position re-export removed (#740) — no longer used by kernel internals.
// Consumers should import directly from reovim_domain_text.

#[cfg(test)]
pub use reovim_domain_text::Cursor;
