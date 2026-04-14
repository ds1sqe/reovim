//! Syntax highlighting state per session.
//!
//! Core driver storage (`SyntaxSessionState`) lives in `reovim-driver-syntax`.
//! This module provides:
//! - Re-export of `SyntaxSessionState` for backward-compatible imports
//! - `SyntaxStreamState` for token update streaming to gRPC clients
//!
//! # Architecture
//!
//! ```text
//! Session ExtensionMap
//!   ├─ SyntaxSessionState (from reovim-driver-syntax)
//!   │    └─ HashMap<BufferId, Box<dyn SyntaxDriver>>
//!   └─ SyntaxStreamState (this module)
//!        └─ Vec<TokenSubscriber>
//! ```

// Re-export core syntax state from driver crate for backward compatibility.
pub use reovim_driver_text_syntax::SyntaxSessionState;

use {
    reovim_driver_text_syntax::SyntaxEdit,
    reovim_kernel::api::v1::BufferId,
    reovim_protocol::v2::{TokenSpan, TokenUpdate},
    reovim_subsys_session::SessionExtension,
    tokio::sync::mpsc,
};

/// Subscription handle for token update streams.
pub type TokenSubscriber = mpsc::Sender<TokenUpdate>;

/// Per-session token streaming state stored in `ExtensionMap`.
///
/// Manages subscriber channels for clients that want real-time token updates
/// via the `StreamTokens` gRPC endpoint.
///
/// # Separation from `SyntaxSessionState`
///
/// Core driver storage lives in `reovim-driver-syntax` (accessible to modules).
/// This type handles server-only streaming infrastructure that depends on
/// `tokio` and `reovim-protocol` (not available to modules).
#[derive(Default)]
pub struct SyntaxStreamState {
    /// Token update subscribers (streaming clients).
    subscribers: Vec<TokenSubscriber>,
}

impl SessionExtension for SyntaxStreamState {
    fn create() -> Self {
        Self::default()
    }
}

impl SyntaxStreamState {
    /// Create a new empty stream state.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Subscribe to token updates.
    ///
    /// Returns a receiver that will receive `TokenUpdate` messages when
    /// buffers are modified and re-tokenized.
    ///
    /// # Channel Size
    ///
    /// The channel has a buffer of 16 messages. If a client falls behind,
    /// older updates may be dropped.
    #[must_use]
    pub fn subscribe(&mut self) -> mpsc::Receiver<TokenUpdate> {
        let (tx, rx) = mpsc::channel(16);
        self.subscribers.push(tx);
        rx
    }

    /// Get the number of active subscribers.
    #[must_use]
    pub const fn subscriber_count(&self) -> usize {
        self.subscribers.len()
    }

    /// Check if there are no subscribers.
    #[must_use]
    pub const fn has_subscribers(&self) -> bool {
        !self.subscribers.is_empty()
    }

    /// Broadcast a token update to all subscribers.
    ///
    /// Removes disconnected subscribers automatically.
    pub fn broadcast(&mut self, update: &TokenUpdate) {
        self.subscribers
            .retain(|tx| tx.try_send(update.clone()).is_ok());
    }

    /// Notify subscribers of a buffer edit.
    ///
    /// This method:
    /// 1. Updates the syntax driver incrementally via `driver.update()`
    /// 2. Gets updated tokens for the affected region
    /// 3. Broadcasts `TokenUpdate` to all subscribers
    ///
    /// # Arguments
    ///
    /// * `syntax` - The syntax session state containing drivers
    /// * `buffer_id` - The buffer that was modified
    /// * `content` - The full buffer content after the edit
    /// * `edit` - The edit description for incremental parsing
    /// * `start_line` - First line affected by the edit (for `TokenUpdate`)
    /// * `end_line` - Last line affected by the edit (for `TokenUpdate`)
    ///
    /// # Panics
    ///
    /// Panics if the syntax driver for `buffer_id` is removed between the
    /// mutable `update()` call and the immutable re-acquisition.  This cannot
    /// happen in practice because no code path removes drivers in between.
    #[allow(clippy::cast_possible_truncation)]
    pub fn notify_edit(
        &mut self,
        syntax: &mut SyntaxSessionState,
        buffer_id: BufferId,
        content: &str,
        edit: &SyntaxEdit,
        start_line: u64,
        end_line: u64,
    ) {
        // Get the driver for this buffer
        let Some(driver) = syntax.get_mut(buffer_id) else {
            return; // No driver for this buffer
        };

        // Update driver incrementally
        driver.update(content, edit);

        // If no subscribers, skip token extraction
        if self.subscribers.is_empty() {
            return;
        }

        // Get tokens for the affected region (with some context)
        // Use byte range from the edit, with padding for context
        let start_byte = edit.start_byte.saturating_sub(100);
        let end_byte = (edit.new_end_byte + 100).min(content.len());

        // Re-acquire immutable reference after mutable borrow ended.
        // The driver cannot vanish between the mutable update() and this
        // immutable re-acquisition — nothing removes it in between.
        let driver = syntax
            .get(buffer_id)
            .expect("driver present: just used for update()");
        let mut highlights = driver.highlights(start_byte..end_byte);
        highlights.extend(driver.decorations(start_byte..end_byte));

        // Convert to TokenSpan
        let tokens: Vec<TokenSpan> = highlights
            .into_iter()
            .map(|span| TokenSpan {
                start_byte: span.start_byte as u32,
                end_byte: span.end_byte as u32,
                category: span.category.to_string(),
            })
            .collect();

        // Build the update message
        let update = TokenUpdate {
            buffer_id: buffer_id.as_usize() as u64,
            tokens,
            start_line,
            end_line,
            full_refresh: false,
            layer: "syntax".into(),
            priority: 0,
        };

        // Broadcast to subscribers (remove disconnected ones)
        self.subscribers
            .retain(|tx| tx.try_send(update.clone()).is_ok());
    }

    /// Send a full token refresh for a buffer.
    ///
    /// Call this when a new subscriber connects or when a buffer's language changes.
    #[allow(clippy::cast_possible_truncation)]
    pub fn send_full_refresh(
        &mut self,
        syntax: &SyntaxSessionState,
        buffer_id: BufferId,
        total_lines: u64,
    ) {
        let Some(driver) = syntax.get(buffer_id) else {
            return;
        };

        if self.subscribers.is_empty() {
            return;
        }

        // Get all highlights and decorations
        let mut highlights = driver.highlights(0..usize::MAX);
        highlights.extend(driver.decorations(0..usize::MAX));

        // Convert to TokenSpan
        let tokens: Vec<TokenSpan> = highlights
            .into_iter()
            .map(|span| TokenSpan {
                start_byte: span.start_byte as u32,
                end_byte: span.end_byte as u32,
                category: span.category.to_string(),
            })
            .collect();

        let update = TokenUpdate {
            buffer_id: buffer_id.as_usize() as u64,
            tokens,
            start_line: 0,
            end_line: total_lines.saturating_sub(1),
            full_refresh: true,
            layer: "syntax".into(),
            priority: 0,
        };

        // Broadcast to subscribers
        self.subscribers
            .retain(|tx| tx.try_send(update.clone()).is_ok());
    }
}

/// Build a `TokenUpdate` from a syntax driver's current highlights.
///
/// This is a standalone function to avoid double-borrow issues when
/// both `SyntaxSessionState` and `SyntaxStreamState` are in the same
/// `ExtensionMap`. Call this after updating the driver, then pass the
/// result to `SyntaxStreamState::broadcast()`.
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub fn build_token_update(
    syntax: &SyntaxSessionState,
    buffer_id: BufferId,
    total_lines: u64,
    full_refresh: bool,
) -> Option<TokenUpdate> {
    let driver = syntax.get(buffer_id)?;
    let mut highlights = driver.highlights(0..usize::MAX);
    highlights.extend(driver.decorations(0..usize::MAX));

    let tokens: Vec<TokenSpan> = highlights
        .into_iter()
        .map(|span| TokenSpan {
            start_byte: span.start_byte as u32,
            end_byte: span.end_byte as u32,
            category: span.category.to_string(),
        })
        .collect();

    Some(TokenUpdate {
        buffer_id: buffer_id.as_usize() as u64,
        tokens,
        start_line: 0,
        end_line: total_lines.saturating_sub(1),
        full_refresh,
        layer: "syntax".into(),
        priority: 0,
    })
}

impl std::fmt::Debug for SyntaxStreamState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SyntaxStreamState")
            .field("subscriber_count", &self.subscribers.len())
            .finish()
    }
}

#[cfg(test)]
#[path = "syntax_state_tests.rs"]
mod tests;
