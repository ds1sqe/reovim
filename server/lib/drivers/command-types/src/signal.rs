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
mod tests {
    use super::*;

    #[test]
    fn test_runtime_signal_quit_exists() {
        let signal = RuntimeSignal::Quit;
        assert_eq!(signal, RuntimeSignal::Quit);
    }

    #[test]
    fn test_runtime_signal_debug_format() {
        let debug = format!("{:?}", RuntimeSignal::Quit);
        assert_eq!(debug, "Quit");
    }

    #[test]
    fn test_runtime_signal_clone() {
        let signal = RuntimeSignal::Quit;
        let cloned = signal.clone();
        assert_eq!(signal, cloned);
    }

    #[test]
    fn test_runtime_signal_eq() {
        assert_eq!(RuntimeSignal::Quit, RuntimeSignal::Quit);
    }

    #[test]
    fn test_runtime_signal_exhaustive_match() {
        // Ensures adding a new variant requires updating this test
        let signal = RuntimeSignal::Quit;
        match signal {
            RuntimeSignal::Quit => {}
        }
    }
}
