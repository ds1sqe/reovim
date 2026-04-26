//! Async edit-subscription trait, separated from [`crate::Buffer`].
//!
//! Lives on its own trait to avoid coupling the closed-tier `Buffer`
//! trait object to a concrete tokio type. Wiring this trait requires no
//! `Buffer` changes; add it when async consumers (LSP, file-watcher,
//! debug surfaces) are introduced.

use {reovim_kernel::api::v1::ByteEdit, tokio::sync::broadcast};

/// Async subscription to per-buffer byte edits.
///
/// Implemented alongside [`crate::Buffer`] by drivers that support the
/// async delivery channel. The `broadcast::Receiver` uses lag-drop
/// semantics (slow subscribers do not back up fast publishers) matching
/// the session notification channel in `server/lib/server/`.
///
/// # Delivery contract
///
/// - Subscribers observe edits in `apply_edit` source-order. The driver
///   sends to the broadcast channel AFTER mutating canonical bytes and
///   AFTER synchronous codec fan-out has returned, so a subscriber that
///   reads `read_bytes` immediately upon wake sees the bytes the edit
///   produced.
/// - Edits are NOT replayed to a fresh subscriber. A subscriber that
///   needs the buffer's current state at subscription time must
///   `read_bytes` independently after subscribing.
/// - `RecvError::Lagged(n)` indicates the host's runtime has dropped
///   `n` consecutive edits because the receiver fell behind the
///   driver's broadcast capacity. Recovery is the consumer's choice
///   (typically: re-snapshot via `read_bytes`, then resume from the
///   next live edit).
pub trait BufferSubscribable: Send + Sync {
    /// Subscribe to the buffer's byte-edit broadcast stream.
    ///
    /// Each call returns a new independent receiver. The channel
    /// capacity is driver-defined; receivers that fall behind may
    /// observe `RecvError::Lagged`.
    fn subscribe_edits(&self) -> broadcast::Receiver<ByteEdit>;
}
