//! Domain-neutral window managed by `SessionRuntime`.
//!
//! [`Window`] tracks which buffer is in which window, the viewport, and
//! caches opaque cursor handles from the domain driver.
//!
//! # Cursor Model
//!
//! Window caches `Vec<Box<dyn Cursor>>` as opaque handles from the domain.
//! The domain driver is the **source of truth** for cursor state — it
//! computes cursors during `dispatch_key`. The mechanism refreshes the
//! cache by calling `domain.cursors(client_id, window_id)` whenever
//! `ChangeSet.cursor_moved` is set.
//!
//! This design:
//! - **Fast gRPC access**: server reads cursors from Window directly
//! - **No domain call per gRPC tick**: only re-queries on `cursor_moved`
//! - **Auto cleanup**: closing a Window drops cursor handles — no
//!   `on_window_closed` notification needed on the domain
//! - **Domain-neutral**: server never interprets cursor content

use {
    reovim_kernel::api::v1::{BufferId, WindowId},
    reovim_subsys_coordination::Cursor,
};

use super::Viewport;

/// Domain-neutral window managed by the session mechanism.
///
/// The server creates, arranges, and destroys windows. Each window references
/// a buffer (which belongs to a domain) and caches domain-neutral cursor
/// handles.
#[derive(Debug)]
pub struct Window {
    id: WindowId,
    buffer_id: BufferId,
    viewport: Viewport,
    domain_id: u32,
    cursors: Vec<Box<dyn Cursor>>,
}

impl Window {
    /// Create a new window with an initial cursor.
    #[must_use]
    pub fn new(
        id: WindowId,
        buffer_id: BufferId,
        viewport: Viewport,
        domain_id: u32,
        initial_cursor: Box<dyn Cursor>,
    ) -> Self {
        Self {
            id,
            buffer_id,
            viewport,
            domain_id,
            cursors: vec![initial_cursor],
        }
    }

    /// Window identifier.
    #[must_use]
    pub const fn id(&self) -> WindowId {
        self.id
    }

    /// Buffer displayed in this window.
    #[must_use]
    pub const fn buffer_id(&self) -> BufferId {
        self.buffer_id
    }

    /// Current viewport.
    #[must_use]
    pub const fn viewport(&self) -> &Viewport {
        &self.viewport
    }

    /// Mutable viewport access (for scroll updates).
    pub const fn viewport_mut(&mut self) -> &mut Viewport {
        &mut self.viewport
    }

    /// Domain ID of the buffer in this window.
    #[must_use]
    pub const fn domain_id(&self) -> u32 {
        self.domain_id
    }

    /// Cached cursors (opaque `dyn Cursor` handles from the domain).
    ///
    /// These are refreshed when `ChangeSet.cursor_moved` is set.
    #[must_use]
    pub fn cursors(&self) -> &[Box<dyn Cursor>] {
        &self.cursors
    }

    /// Update cached cursors after domain reports `cursor_moved`.
    pub fn set_cursors(&mut self, cursors: Vec<Box<dyn Cursor>>) {
        self.cursors = cursors;
    }

    /// Primary cursor (convention: index 0).
    ///
    /// Returns `None` if no cursors are cached (shouldn't happen in practice).
    #[must_use]
    pub fn primary_cursor(&self) -> Option<&dyn Cursor> {
        self.cursors.first().map(|c| &**c)
    }

    /// Change the buffer displayed in this window.
    ///
    /// The `domain_id` may change if the new buffer belongs to a different
    /// domain. Caller must also update cursors via [`set_cursors`](Self::set_cursors).
    pub const fn set_buffer(&mut self, buffer_id: BufferId, domain_id: u32) {
        self.buffer_id = buffer_id;
        self.domain_id = domain_id;
    }
}
