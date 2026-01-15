//! Per-client viewport state.
//!
//! Each client has independent viewport state allowing different terminal
//! dimensions, active buffers, and cursor positions per buffer.
//!
//! # Buffer Close Cleanup
//!
//! When a buffer is closed, clients viewing that buffer should have their
//! `active_buffer` cleared. Use [`clear_viewports_for_closed_buffer`] to
//! handle this cleanup.

use std::collections::HashMap;

use reovim_kernel::api::v1::{BufferId, Position};

use crate::session::Session;

/// Per-client viewport state.
///
/// Each client has independent viewport state allowing:
/// - Different terminal dimensions
/// - Different active buffers
/// - Independent cursor positions per buffer
///
/// # Cursor Semantics
///
/// Each client maintains **independent cursor positions per buffer**. This enables
/// multi-client editing of the same file without cursor conflicts, matching
/// traditional multi-terminal text editor behavior (like vim in separate tmux panes).
///
/// # Lock Hierarchy Safety
///
/// `ClientViewport` is at **Level 2** in the lock hierarchy. Always drop viewport
/// locks before acquiring session locks to prevent deadlocks:
///
/// ```ignore
/// // SAFE: viewport → session
/// let buffer_id = {
///     let vp = client.viewport().read().await;
///     vp.active_buffer
/// }; // dropped
/// session.with_state(|state| { ... }).await;
/// ```
#[derive(Debug, Clone)]
pub struct ClientViewport {
    /// Terminal width in columns.
    pub terminal_width: u16,

    /// Terminal height in rows.
    pub terminal_height: u16,

    /// Active buffer for this client.
    pub active_buffer: Option<BufferId>,

    /// Cursor positions per buffer (restored on buffer switch).
    pub buffer_cursors: HashMap<BufferId, Position>,

    /// Scroll top line (for future window implementation).
    pub scroll_top: usize,
}

impl Default for ClientViewport {
    fn default() -> Self {
        Self {
            terminal_width: 80,  // VT100 default
            terminal_height: 24, // VT100 default
            active_buffer: None,
            buffer_cursors: HashMap::new(),
            scroll_top: 0,
        }
    }
}

impl ClientViewport {
    /// Create a new viewport with custom dimensions.
    #[must_use]
    pub fn with_size(width: u16, height: u16) -> Self {
        Self {
            terminal_width: width,
            terminal_height: height,
            ..Default::default()
        }
    }

    /// Get cursor position for a buffer.
    ///
    /// Returns the saved cursor position for the buffer, or `Position::default()` (0, 0)
    /// if the buffer hasn't been visited yet.
    #[must_use]
    pub fn cursor_for_buffer(&self, buffer_id: BufferId) -> Position {
        self.buffer_cursors
            .get(&buffer_id)
            .copied()
            .unwrap_or_default()
    }

    /// Update cursor position for a buffer.
    ///
    /// Saves the cursor position so it can be restored when switching back to this buffer.
    pub fn set_cursor_for_buffer(&mut self, buffer_id: BufferId, pos: Position) {
        self.buffer_cursors.insert(buffer_id, pos);
    }
}

/// Clear `active_buffer` for all clients viewing a closed buffer.
///
/// When a buffer is closed (deleted), clients viewing that buffer should
/// have their `active_buffer` set to `None`. This prevents stale buffer
/// references in client viewports.
///
/// # Note
///
/// The `buffer_cursors` entry for the closed buffer is intentionally
/// preserved. This allows cursor position restoration if the buffer
/// is later re-opened (e.g., via undo or reload).
///
/// # Example
///
/// ```ignore
/// // In a buffer/close handler:
/// clear_viewports_for_closed_buffer(&ctx.session, closed_buffer_id).await;
/// ```
pub async fn clear_viewports_for_closed_buffer(session: &Session, closed_buffer_id: BufferId) {
    for client in session.clients().iter() {
        let needs_clear = {
            let viewport = client.viewport().read().await;
            viewport.active_buffer == Some(closed_buffer_id)
        }; // Read lock dropped

        if needs_clear {
            let mut viewport = client.viewport().write().await;
            viewport.active_buffer = None;
            drop(viewport); // Explicitly drop write lock before logging

            tracing::debug!(
                "Cleared active buffer for client {:?} after buffer {:?} closed",
                client.id(),
                closed_buffer_id
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_viewport_default() {
        let vp = ClientViewport::default();

        // VT100 defaults
        assert_eq!(vp.terminal_width, 80);
        assert_eq!(vp.terminal_height, 24);
        assert_eq!(vp.active_buffer, None);
        assert!(vp.buffer_cursors.is_empty());
        assert_eq!(vp.scroll_top, 0);
    }

    #[test]
    fn test_viewport_with_size() {
        let vp = ClientViewport::with_size(200, 50);

        assert_eq!(vp.terminal_width, 200);
        assert_eq!(vp.terminal_height, 50);
        // Other fields should be default
        assert_eq!(vp.active_buffer, None);
        assert!(vp.buffer_cursors.is_empty());
    }

    #[test]
    fn test_viewport_buffer_cursors() {
        let mut vp = ClientViewport::default();
        let buf1 = BufferId::from_raw(1);
        let buf2 = BufferId::from_raw(2);

        // Set cursor for buffer 1
        vp.set_cursor_for_buffer(
            buf1,
            Position {
                line: 10,
                column: 5,
            },
        );
        assert_eq!(
            vp.cursor_for_buffer(buf1),
            Position {
                line: 10,
                column: 5
            }
        );

        // Buffer 2 should have default cursor
        assert_eq!(vp.cursor_for_buffer(buf2), Position::default());

        // Update buffer 1 cursor
        vp.set_cursor_for_buffer(
            buf1,
            Position {
                line: 20,
                column: 15,
            },
        );
        assert_eq!(
            vp.cursor_for_buffer(buf1),
            Position {
                line: 20,
                column: 15
            }
        );

        // Set cursor for buffer 2
        vp.set_cursor_for_buffer(buf2, Position { line: 5, column: 3 });
        assert_eq!(vp.cursor_for_buffer(buf2), Position { line: 5, column: 3 });

        // Buffer 1 should still have its cursor
        assert_eq!(
            vp.cursor_for_buffer(buf1),
            Position {
                line: 20,
                column: 15
            }
        );
    }

    #[test]
    fn test_viewport_cursor_for_buffer_default() {
        let vp = ClientViewport::default();
        let buf = BufferId::from_raw(999);

        // Non-existent buffer should return default position (0, 0)
        assert_eq!(vp.cursor_for_buffer(buf), Position::default());
        assert_eq!(vp.cursor_for_buffer(buf), Position { line: 0, column: 0 });
    }

    #[test]
    fn test_viewport_cursor_preserved_after_clear_active() {
        let mut vp = ClientViewport::default();
        let buf = BufferId::from_raw(1);

        // Set cursor and active buffer
        vp.set_cursor_for_buffer(
            buf,
            Position {
                line: 50,
                column: 25,
            },
        );
        vp.active_buffer = Some(buf);

        // Clear active buffer (simulating buffer close)
        vp.active_buffer = None;

        // Cursor position should still be preserved (for undo/reload)
        assert_eq!(
            vp.cursor_for_buffer(buf),
            Position {
                line: 50,
                column: 25
            }
        );
    }
}

/// Tests for `clear_viewports_for_closed_buffer` function.
///
/// These tests verify buffer close cleanup behavior.
#[cfg(test)]
mod clear_viewports_tests {
    use {
        super::*,
        crate::{
            server::{client::Client, transport::TransportWriter},
            session::{ClientId, Session, SessionId},
        },
        reovim_kernel::api::v1::{KernelContext, ModeId, ModuleId},
        std::sync::Arc,
    };

    fn test_session() -> Arc<Session> {
        Session::new(
            SessionId::new("test"),
            KernelContext::default(),
            ModeId::new(ModuleId::new("test"), "normal"),
        )
    }

    fn test_client(session_id: SessionId, client_id: ClientId) -> Arc<Client> {
        Client::new(client_id, session_id, TransportWriter::from_stdio())
    }

    #[tokio::test]
    async fn test_clear_viewports_single_client_viewing() {
        let session = test_session();
        let client_id = ClientId::new(1);
        let client = test_client(session.id().clone(), client_id);

        // Register client with session
        session.clients().insert(&client);

        let buffer_id = BufferId::from_raw(42);

        // Set client's active buffer
        {
            let mut viewport = client.viewport().write().await;
            viewport.active_buffer = Some(buffer_id);
            viewport.set_cursor_for_buffer(
                buffer_id,
                Position {
                    line: 10,
                    column: 5,
                },
            );
        }

        // Close the buffer
        clear_viewports_for_closed_buffer(&session, buffer_id).await;

        // Verify active_buffer is cleared and cursor is preserved
        let (active_buf, saved_cursor) = {
            let viewport = client.viewport().read().await;
            (viewport.active_buffer, viewport.cursor_for_buffer(buffer_id))
        };
        assert_eq!(active_buf, None, "Active buffer should be cleared");
        assert_eq!(
            saved_cursor,
            Position {
                line: 10,
                column: 5
            },
            "Cursor position should be preserved after buffer close"
        );
    }

    #[tokio::test]
    async fn test_clear_viewports_multiple_clients_viewing_same_buffer() {
        let session = test_session();
        let buffer_id = BufferId::from_raw(42);

        // Create three clients all viewing the same buffer
        let client1 = test_client(session.id().clone(), ClientId::new(1));
        let client2 = test_client(session.id().clone(), ClientId::new(2));
        let client3 = test_client(session.id().clone(), ClientId::new(3));

        // Register all clients
        session.clients().insert(&client1);
        session.clients().insert(&client2);
        session.clients().insert(&client3);

        // Set all clients to view the same buffer
        for client in [&client1, &client2, &client3] {
            let mut viewport = client.viewport().write().await;
            viewport.active_buffer = Some(buffer_id);
        }

        // Close the buffer
        clear_viewports_for_closed_buffer(&session, buffer_id).await;

        // Verify all clients have active_buffer cleared
        for (i, client) in [&client1, &client2, &client3].iter().enumerate() {
            let active = {
                let viewport = client.viewport().read().await;
                viewport.active_buffer
            };
            assert_eq!(active, None, "Client {} should have active_buffer cleared", i + 1);
        }
    }

    #[tokio::test]
    async fn test_clear_viewports_client_not_viewing_buffer() {
        let session = test_session();
        let closed_buffer = BufferId::from_raw(42);
        let other_buffer = BufferId::from_raw(99);

        let client = test_client(session.id().clone(), ClientId::new(1));
        session.clients().insert(&client);

        // Client is viewing a different buffer
        {
            let mut viewport = client.viewport().write().await;
            viewport.active_buffer = Some(other_buffer);
        }

        // Close the buffer that client is NOT viewing
        clear_viewports_for_closed_buffer(&session, closed_buffer).await;

        // Verify client's active buffer is unchanged
        let active = {
            let viewport = client.viewport().read().await;
            viewport.active_buffer
        };
        assert_eq!(
            active,
            Some(other_buffer),
            "Client viewing different buffer should not be affected"
        );
    }

    #[tokio::test]
    async fn test_clear_viewports_mixed_clients() {
        let session = test_session();
        let closed_buffer = BufferId::from_raw(42);
        let other_buffer = BufferId::from_raw(99);

        // Client 1 viewing closed_buffer
        let client1 = test_client(session.id().clone(), ClientId::new(1));
        // Client 2 viewing other_buffer
        let client2 = test_client(session.id().clone(), ClientId::new(2));
        // Client 3 viewing closed_buffer
        let client3 = test_client(session.id().clone(), ClientId::new(3));
        // Client 4 with no active buffer
        let client4 = test_client(session.id().clone(), ClientId::new(4));

        // Register all clients
        session.clients().insert(&client1);
        session.clients().insert(&client2);
        session.clients().insert(&client3);
        session.clients().insert(&client4);

        // Set active buffers
        {
            let mut vp1 = client1.viewport().write().await;
            vp1.active_buffer = Some(closed_buffer);
        }
        {
            let mut vp2 = client2.viewport().write().await;
            vp2.active_buffer = Some(other_buffer);
        }
        {
            let mut vp3 = client3.viewport().write().await;
            vp3.active_buffer = Some(closed_buffer);
        }
        // client4 has no active buffer (None by default)

        // Close the buffer
        clear_viewports_for_closed_buffer(&session, closed_buffer).await;

        // Verify results
        let active1 = { client1.viewport().read().await.active_buffer };
        let active2 = { client2.viewport().read().await.active_buffer };
        let active3 = { client3.viewport().read().await.active_buffer };
        let active4 = { client4.viewport().read().await.active_buffer };

        assert_eq!(active1, None, "Client 1 should be cleared");
        assert_eq!(active2, Some(other_buffer), "Client 2 should be unchanged");
        assert_eq!(active3, None, "Client 3 should be cleared");
        assert_eq!(active4, None, "Client 4 should remain None");
    }

    #[tokio::test]
    async fn test_clear_viewports_empty_session() {
        let session = test_session();
        let buffer_id = BufferId::from_raw(42);

        // Should not panic with no clients
        clear_viewports_for_closed_buffer(&session, buffer_id).await;
    }
}
