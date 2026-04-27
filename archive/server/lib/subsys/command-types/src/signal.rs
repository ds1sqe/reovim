//! Runtime signals for per-client lifecycle events.
//!
//! Like Unix signals: commands communicate lifecycle events through
//! the runtime, not through return values. See issue #547.

/// Per-client lifecycle signal.
///
/// Commands push signals onto `SessionRuntime`'s signal queue during
/// execution. The server drains the queue after command execution
/// completes and acts on the signals (server policy).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeSignal {
    /// Disconnect this client.
    ///
    /// Server disconnects the client. Server keeps running.
    /// In integrated mode: server checks unsaved buffers before
    /// allowing the disconnect (server-side, since server owns buffers).
    Quit,
}

#[cfg(test)]
#[path = "signal_tests.rs"]
mod tests;
