//! Lifecycle orchestration for the launcher.
//!
//! [`ShutdownCoord`] wraps a `broadcast` sender so the embedded and
//! subprocess paths share one "everyone stop" primitive. In embedded
//! mode the launcher clones a receiver into the server task; when the
//! client exits, the launcher fires the broadcast and the server task
//! awaits its receiver, calls [`reovim_server::Server::shutdown`], and
//! drains before returning.
//!
//! Subprocess-mode SIGINT forwarding lives in the sibling
//! [`subprocess_signals`] module; the separation keeps the embedded
//! composition free of platform-conditional signal plumbing.

use std::io;

use tokio::sync::broadcast;

/// Coordinate graceful shutdown across the embedded composition's
/// tokio tasks.
///
/// The channel is unit-typed (`broadcast::Sender<()>`); the signal is
/// "please stop" — not a value. Capacity 1 is enough because a late
/// subscriber that missed the signal can always observe the *closed*
/// state of its receiver, and the embedded flow never fires the
/// signal twice.
#[derive(Debug)]
pub struct ShutdownCoord {
    sender: broadcast::Sender<()>,
}

impl ShutdownCoord {
    /// Create a new coordinator with no subscribers.
    #[must_use]
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(1);
        Self { sender }
    }

    /// Subscribe a new receiver. The caller hands the receiver into
    /// the task it wants woken when shutdown fires.
    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<()> {
        self.sender.subscribe()
    }

    /// Notify every subscriber that shutdown is requested.
    ///
    /// # Errors
    ///
    /// Returns an I/O error when every receiver has already been
    /// dropped — no task remains to observe the signal.
    pub fn notify_server(&self) -> io::Result<()> {
        self.sender
            .send(())
            .map(|_| ())
            .map_err(|_| io::Error::other("shutdown: all subscribers dropped"))
    }
}

impl Default for ShutdownCoord {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "lifecycle_tests.rs"]
mod lifecycle_tests;
