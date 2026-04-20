//! Text-domain events.
//!
//! These events are emitted by the text provider runtime when
//! text-specific state transitions occur.  They carry text-domain
//! types directly.
//!
//! For pure byte-layer notifications without semantic context,
//! subscribe to `BufferBytesEdited` (kernel substrate event).
//!
//! # Tree-sitter note
//!
//! [`TextBufferModified`] keeps byte-range fields (`start_byte`,
//! `old_end_byte`, `new_end_byte`) so incremental parsers like
//! tree-sitter can correlate byte ranges with point ranges atomically
//! in one handler callback.

use reovim_kernel::api::v1::{Event, events::kernel::priority};

/// Re-export kernel identifiers so consumers don't need a direct kernel dep
/// just for `BufferId` / `WindowId`.
pub use reovim_kernel::api::v1::{BufferId, WindowId};

/// Re-export text types used in event payloads.
pub use reovim_domain_text::{TextEdit, TextPosition};

/// A buffer's text content was modified by a semantic text edit.
///
/// Keeps byte-range fields for tree-sitter's incremental parse
/// atomicity.  Pure byte consumers (byte undo log, network sync)
/// should subscribe to `BufferBytesEdited` instead.
#[derive(Debug, Clone)]
pub struct TextBufferModified {
    /// ID of the modified buffer.
    pub buffer_id: BufferId,
    /// The semantic text edit that occurred.
    pub edit: TextEdit,
    /// Byte offset where the edit begins.
    pub start_byte: usize,
    /// Byte offset of the old end (before edit).
    pub old_end_byte: usize,
    /// Byte offset of the new end (after edit).
    pub new_end_byte: usize,
}

impl Event for TextBufferModified {
    fn priority(&self) -> u32 {
        priority::NORMAL
    }
}

/// Cursor position changed within a text window.
///
/// Keyed on [`WindowId`] because text cursors are per-window (vim
/// convention).  Two windows on the same buffer have independent
/// cursors and each emits its own event.
#[derive(Debug, Clone, Copy)]
pub struct CursorMoved {
    /// Window where the cursor moved.
    pub window_id: WindowId,
    /// Buffer the cursor is in.
    pub buffer_id: BufferId,
    /// Previous cursor position.
    pub from: TextPosition,
    /// New cursor position.
    pub to: TextPosition,
}

impl Event for CursorMoved {
    fn priority(&self) -> u32 {
        priority::NORMAL
    }
}

/// Text viewport scrolled.
///
/// Keyed on [`WindowId`].  Line numbers are 0-indexed.
#[derive(Debug, Clone, Copy)]
pub struct ViewportScrolled {
    /// Window that scrolled.
    pub window_id: WindowId,
    /// Buffer being viewed.
    pub buffer_id: BufferId,
    /// First visible line (0-indexed).
    pub top_line: u32,
    /// Last visible line (0-indexed).
    pub bottom_line: u32,
}

impl Event for ViewportScrolled {
    fn priority(&self) -> u32 {
        priority::NORMAL
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
