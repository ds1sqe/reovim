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
pub use reovim_driver_syntax::SyntaxSessionState;

use {
    reovim_driver_session::SessionExtension,
    reovim_driver_syntax::SyntaxEdit,
    reovim_kernel::api::v1::BufferId,
    reovim_protocol::v2::{TokenSpan, TokenUpdate},
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

        // Re-acquire immutable reference after mutable borrow ended
        let Some(driver) = syntax.get(buffer_id) else {
            return;
        };
        let highlights = driver.highlights(start_byte..end_byte);

        // Convert to TokenSpan
        let tokens: Vec<TokenSpan> = highlights
            .into_iter()
            .map(|span| TokenSpan {
                start_byte: span.start_byte as u32,
                end_byte: span.end_byte as u32,
                category: span.category.to_string(),
                kind: None,
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

        // Get all highlights
        let highlights = driver.highlights(0..usize::MAX);

        // Convert to TokenSpan
        let tokens: Vec<TokenSpan> = highlights
            .into_iter()
            .map(|span| TokenSpan {
                start_byte: span.start_byte as u32,
                end_byte: span.end_byte as u32,
                category: span.category.to_string(),
                kind: None,
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
    let highlights = driver.highlights(0..usize::MAX);

    let tokens: Vec<TokenSpan> = highlights
        .into_iter()
        .map(|span| TokenSpan {
            start_byte: span.start_byte as u32,
            end_byte: span.end_byte as u32,
            category: span.category.to_string(),
            kind: None,
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
mod tests {
    use super::*;

    use std::{ops::Range, sync::Arc};

    use reovim_driver_syntax::{
        Annotation, HighlightCategory, SyntaxDriver, SyntaxDriverFactory, SyntaxEdit,
    };

    /// A minimal test driver for unit tests.
    struct TestDriver {
        language: String,
        parsed: bool,
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl TestDriver {
        fn new(language: &str) -> Self {
            Self {
                language: language.to_string(),
                parsed: false,
            }
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl SyntaxDriver for TestDriver {
        fn language(&self) -> &str {
            &self.language
        }

        fn parse(&mut self, _content: &str) {
            self.parsed = true;
        }

        fn update(&mut self, _content: &str, _edit: &SyntaxEdit) {
            // No-op
        }

        fn highlights(&self, byte_range: Range<usize>) -> Vec<Annotation> {
            if self.parsed {
                vec![Annotation::highlight(
                    byte_range.start,
                    byte_range.end,
                    HighlightCategory::new("comment"),
                )]
            } else {
                Vec::new()
            }
        }

        fn is_parsed(&self) -> bool {
            self.parsed
        }
    }

    /// A minimal test factory.
    struct TestFactory;

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl SyntaxDriverFactory for TestFactory {
        fn create(&self, language_id: &str) -> Option<Box<dyn SyntaxDriver>> {
            if language_id == "rust" {
                Some(Box::new(TestDriver::new("rust")))
            } else {
                None
            }
        }

        fn supported_languages(&self) -> Vec<&str> {
            vec!["rust"]
        }

        fn supports(&self, language_id: &str) -> bool {
            language_id == "rust"
        }
    }

    fn buffer_id(n: usize) -> BufferId {
        BufferId::from_raw(n)
    }

    // ========================================================================
    // SyntaxStreamState tests
    // ========================================================================

    #[test]
    fn test_stream_state_new() {
        let state = SyntaxStreamState::new();
        assert_eq!(state.subscriber_count(), 0);
        assert!(!state.has_subscribers());
    }

    #[test]
    fn test_stream_state_session_extension() {
        let state = SyntaxStreamState::create();
        assert_eq!(state.subscriber_count(), 0);
    }

    #[test]
    fn test_subscribe() {
        let mut state = SyntaxStreamState::new();
        assert_eq!(state.subscriber_count(), 0);

        let _rx1 = state.subscribe();
        assert_eq!(state.subscriber_count(), 1);
        assert!(state.has_subscribers());

        let _rx2 = state.subscribe();
        assert_eq!(state.subscriber_count(), 2);
    }

    #[test]
    fn test_broadcast() {
        let mut state = SyntaxStreamState::new();
        let mut rx = state.subscribe();

        let update = TokenUpdate {
            buffer_id: 1,
            tokens: vec![],
            start_line: 0,
            end_line: 0,
            full_refresh: false,
            layer: "syntax".into(),
            priority: 0,
        };

        state.broadcast(&update);

        let received = rx.try_recv().expect("Should receive update");
        assert_eq!(received.buffer_id, 1);
        assert_eq!(received.layer, "syntax");
        assert_eq!(received.priority, 0);
    }

    #[test]
    fn test_broadcast_removes_disconnected() {
        let mut state = SyntaxStreamState::new();
        let rx = state.subscribe();
        assert_eq!(state.subscriber_count(), 1);

        // Drop the receiver to disconnect
        drop(rx);

        let update = TokenUpdate {
            buffer_id: 1,
            tokens: vec![],
            start_line: 0,
            end_line: 0,
            full_refresh: false,
            layer: "syntax".into(),
            priority: 0,
        };

        state.broadcast(&update);

        // Disconnected subscriber should be removed
        assert_eq!(state.subscriber_count(), 0);
    }

    #[tokio::test]
    async fn test_notify_edit_with_subscriber() {
        let mut syntax = SyntaxSessionState::new();
        let mut stream = SyntaxStreamState::new();
        let id = buffer_id(1);

        // Set up a driver
        syntax.set(id, Box::new(TestDriver::new("rust")));
        syntax.get_mut(id).unwrap().parse("fn main() {}");

        // Subscribe
        let mut rx = stream.subscribe();
        assert_eq!(stream.subscriber_count(), 1);

        // Create a simple edit
        let edit = SyntaxEdit::insert(0, 0, 0, 3, 0, 3);

        // Notify edit
        stream.notify_edit(&mut syntax, id, "fn main() {}", &edit, 0, 0);

        // Should receive an update
        let update = rx.try_recv().expect("Should receive update");
        assert_eq!(update.buffer_id, 1);
        assert!(!update.full_refresh);
        assert_eq!(update.layer, "syntax");
        assert_eq!(update.priority, 0);
    }

    #[test]
    fn test_notify_edit_no_driver() {
        let mut syntax = SyntaxSessionState::new();
        let mut stream = SyntaxStreamState::new();
        let id = buffer_id(1);

        // No driver set
        let edit = SyntaxEdit::insert(0, 0, 0, 3, 0, 3);

        // Should not panic
        stream.notify_edit(&mut syntax, id, "hello", &edit, 0, 0);
    }

    #[test]
    fn test_send_full_refresh() {
        let mut syntax = SyntaxSessionState::new();
        let mut stream = SyntaxStreamState::new();
        let id = buffer_id(1);

        // Set up a driver
        syntax.set(id, Box::new(TestDriver::new("rust")));
        syntax.get_mut(id).unwrap().parse("fn main() {}");

        // Subscribe
        let mut rx = stream.subscribe();

        // Send full refresh
        stream.send_full_refresh(&syntax, id, 10);

        // Should receive a full refresh update
        let update = rx.try_recv().expect("Should receive update");
        assert_eq!(update.buffer_id, 1);
        assert!(update.full_refresh);
        assert_eq!(update.end_line, 9); // total_lines - 1
        assert_eq!(update.layer, "syntax");
        assert_eq!(update.priority, 0);
    }

    #[test]
    fn test_notify_edit_no_subscribers_skips_extraction() {
        let mut syntax = SyntaxSessionState::new();
        let mut stream = SyntaxStreamState::new();
        let id = buffer_id(1);

        syntax.set(id, Box::new(TestDriver::new("rust")));

        // No subscribers: notify_edit should return early after driver.update()
        let edit = SyntaxEdit {
            start_byte: 0,
            old_end_byte: 0,
            new_end_byte: 5,
            start_row: 0,
            start_col: 0,
            old_end_row: 0,
            old_end_col: 0,
            new_end_row: 0,
            new_end_col: 5,
        };
        stream.notify_edit(&mut syntax, id, "hello", &edit, 0, 0);
        // No panic, no subscribers to receive
    }

    #[test]
    fn test_send_full_refresh_no_driver_returns_early() {
        let syntax = SyntaxSessionState::new();
        let mut stream = SyntaxStreamState::new();
        let _rx = stream.subscribe(); // Has subscriber but no driver
        let unknown = buffer_id(999);

        // Should return early (no driver)
        stream.send_full_refresh(&syntax, unknown, 10);
        // No panic
    }

    #[test]
    fn test_send_full_refresh_no_subscribers_returns_early() {
        let mut syntax = SyntaxSessionState::new();
        let stream = SyntaxStreamState::new();
        let id = buffer_id(1);
        syntax.set(id, Box::new(TestDriver::new("rust")));

        // Has driver but no subscribers: returns early
        // Note: we need &mut self for send_full_refresh, use a mutable binding
        let mut stream = stream;
        stream.send_full_refresh(&syntax, id, 10);
        // No panic
    }

    #[test]
    fn test_debug_impl() {
        let mut state = SyntaxStreamState::new();
        let _rx = state.subscribe();

        let debug = format!("{state:?}");
        assert!(debug.contains("SyntaxStreamState"));
        assert!(debug.contains("subscriber_count"));
    }

    // ========================================================================
    // SyntaxSessionState re-export sanity test
    // ========================================================================

    // ========================================================================
    // build_token_update tests
    // ========================================================================

    #[test]
    fn test_build_token_update_with_driver() {
        let mut syntax = SyntaxSessionState::new();
        let id = buffer_id(1);

        syntax.set(id, Box::new(TestDriver::new("rust")));
        syntax.get_mut(id).unwrap().parse("fn main() {}");

        let update = build_token_update(&syntax, id, 10, true);
        assert!(update.is_some());

        let update = update.unwrap();
        assert_eq!(update.buffer_id, 1);
        assert!(update.full_refresh);
        assert_eq!(update.start_line, 0);
        assert_eq!(update.end_line, 9);
        assert!(!update.tokens.is_empty());
        assert_eq!(update.layer, "syntax");
        assert_eq!(update.priority, 0);
    }

    #[test]
    fn test_build_token_update_no_driver() {
        let syntax = SyntaxSessionState::new();
        let id = buffer_id(1);

        let update = build_token_update(&syntax, id, 10, true);
        assert!(update.is_none());
    }

    #[test]
    fn test_build_token_update_incremental() {
        let mut syntax = SyntaxSessionState::new();
        let id = buffer_id(1);

        syntax.set(id, Box::new(TestDriver::new("rust")));
        syntax.get_mut(id).unwrap().parse("fn main() {}");

        let update = build_token_update(&syntax, id, 5, false).unwrap();
        assert!(!update.full_refresh);
        assert_eq!(update.end_line, 4);
    }

    #[test]
    fn test_syntax_session_state_reexport() {
        // Verify re-export works: SyntaxSessionState accessible from this module
        let mut state = SyntaxSessionState::new();
        let id = buffer_id(1);

        state.set_factory(Arc::new(TestFactory));
        assert!(state.ensure_driver(id, "rust", "fn main() {}"));
        assert!(state.get(id).is_some());
    }
}
