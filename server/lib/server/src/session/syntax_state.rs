//! Syntax highlighting state per session.
//!
//! # Architecture (#753 E6)
//!
//! Syntax state management has been removed from the server crate.
//! Syntax drivers are domain-owned — the text domain driver manages
//! `SyntaxSessionState` internally and routes token updates through
//! post-dispatch hooks.
//!
//! `SyntaxStreamState` (token subscriber management) remains as it
//! depends on server-side infrastructure (tokio channels, gRPC protocol).

use {
    reovim_protocol::v2::TokenUpdate,
    reovim_subsys_session::SessionExtension,
    tokio::sync::mpsc,
};

/// Subscription handle for token update streams.
pub type TokenSubscriber = mpsc::Sender<TokenUpdate>;

/// Per-session token streaming state stored in `ExtensionMap`.
///
/// Manages subscriber channels for clients that want real-time token updates
/// via the `StreamTokens` gRPC endpoint.
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
}

// SyntaxSessionState re-export and notify_edit/send_full_refresh/build_token_update:
// REMOVED (#753 E6). These used driver-text-syntax types (SyntaxSessionState,
// SyntaxEdit). Syntax driver management is now domain-owned.

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
