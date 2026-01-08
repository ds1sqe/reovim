//! Prioritized event sender for dual-channel architecture
//!
//! This module provides a wrapper around the dual-channel event system that
//! separates high-priority events (user input) from low-priority events
//! (background tasks like render signals).

use tokio::sync::mpsc;

use crate::event::RuntimeEvent;

/// Wrapper for dual-channel event sending with priority support.
///
/// High-priority channel is used for user-facing events that need immediate
/// processing (keystrokes, mode changes, text input). Low-priority channel
/// is used for background events (render signals, syntax updates, plugin events).
#[derive(Clone)]
pub struct PrioritizedEventSender {
    hi: mpsc::Sender<RuntimeEvent>,
    lo: mpsc::Sender<RuntimeEvent>,
}

impl PrioritizedEventSender {
    /// Create a new prioritized sender from high and low priority channels.
    #[must_use]
    pub const fn new(hi: mpsc::Sender<RuntimeEvent>, lo: mpsc::Sender<RuntimeEvent>) -> Self {
        Self { hi, lo }
    }

    /// Send a high-priority event (user input, mode changes).
    ///
    /// Use this for events that need immediate processing to maintain
    /// responsive user experience (<16ms latency target).
    ///
    /// # Errors
    ///
    /// Returns an error if the channel is full or closed.
    #[allow(clippy::result_large_err)]
    pub fn send_hi(
        &self,
        event: RuntimeEvent,
    ) -> Result<(), mpsc::error::TrySendError<RuntimeEvent>> {
        self.hi.try_send(event)
    }

    /// Send a low-priority event (render signals, background tasks).
    ///
    /// Use this for events that can be batched or delayed without
    /// affecting user-perceived responsiveness.
    ///
    /// # Errors
    ///
    /// Returns an error if the channel is full or closed.
    #[allow(clippy::result_large_err)]
    pub fn send_lo(
        &self,
        event: RuntimeEvent,
    ) -> Result<(), mpsc::error::TrySendError<RuntimeEvent>> {
        self.lo.try_send(event)
    }

    /// Clone just the high-priority sender (for user input paths).
    ///
    /// Use this when a component only sends high-priority events
    /// (e.g., `Dispatcher`, `TerminateHandler`).
    #[must_use]
    pub fn hi_sender(&self) -> mpsc::Sender<RuntimeEvent> {
        self.hi.clone()
    }

    /// Clone just the low-priority sender (for background paths).
    ///
    /// Use this when a component only sends low-priority events
    /// (e.g., `AnimationController`, `Saturator`, `RuntimeContext`).
    #[must_use]
    pub fn lo_sender(&self) -> mpsc::Sender<RuntimeEvent> {
        self.lo.clone()
    }
}
