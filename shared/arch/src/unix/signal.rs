//! Unix signal handler implementation.
//!
//! Note: crossterm handles SIGWINCH through resize events in the event stream,
//! not through traditional signal handlers. This implementation provides the
//! trait interface but defers actual signal handling to the input source.

use crate::traits::{SignalHandler, TerminalSize};

/// Unix signal handler implementation.
///
/// On Unix with crossterm:
/// - Resize (SIGWINCH) is delivered via [`InputEvent::Resize`] through the input source
/// - Interrupt (SIGINT) is typically delivered as Ctrl+C key event in raw mode
/// - Suspend (SIGTSTP) is typically delivered as Ctrl+Z key event in raw mode
///
/// This struct provides a placeholder for future signal handling needs
/// or for platforms that require explicit signal registration.
#[allow(clippy::struct_field_names)]
pub struct UnixSignalHandler {
    // Handlers are stored but not used with crossterm since it handles
    // signals internally through the event stream
    #[allow(dead_code)]
    resize_handler: Option<Box<dyn Fn(TerminalSize) + Send + Sync>>,
    #[allow(dead_code)]
    interrupt_handler: Option<Box<dyn Fn() + Send + Sync>>,
    #[allow(dead_code)]
    suspend_handler: Option<Box<dyn Fn() + Send + Sync>>,
}

impl UnixSignalHandler {
    /// Create a new Unix signal handler.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            resize_handler: None,
            interrupt_handler: None,
            suspend_handler: None,
        }
    }
}

impl Default for UnixSignalHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl SignalHandler for UnixSignalHandler {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn on_resize(&mut self, handler: Box<dyn Fn(TerminalSize) + Send + Sync>) {
        // Note: With crossterm, resize events are delivered through the
        // event stream as InputEvent::Resize, not through signal handlers.
        // We store the handler for compatibility but it won't be called
        // in the current implementation.
        self.resize_handler = Some(handler);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn on_interrupt(&mut self, handler: Box<dyn Fn() + Send + Sync>) {
        // Note: In raw mode, Ctrl+C is delivered as a key event.
        // We store the handler for compatibility.
        self.interrupt_handler = Some(handler);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn on_suspend(&mut self, handler: Box<dyn Fn() + Send + Sync>) {
        // Note: In raw mode, Ctrl+Z is delivered as a key event.
        // We store the handler for compatibility.
        self.suspend_handler = Some(handler);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signal_handler_creation() {
        let _handler = UnixSignalHandler::new();
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_register_handlers() {
        let mut handler = UnixSignalHandler::new();

        handler.on_resize(Box::new(|_size| {
            // Resize callback
        }));

        handler.on_interrupt(Box::new(|| {
            // Interrupt callback
        }));

        handler.on_suspend(Box::new(|| {
            // Suspend callback
        }));

        // Handlers are registered (even if not actively used with crossterm)
    }

    #[test]
    fn test_signal_handler_default() {
        let _handler = UnixSignalHandler::default();
        // Verify Default implementation works
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_register_handlers_replaces_previous() {
        let mut handler = UnixSignalHandler::new();

        // Register first set
        handler.on_resize(Box::new(|_size| {}));
        handler.on_interrupt(Box::new(|| {}));
        handler.on_suspend(Box::new(|| {}));

        // Register second set (replaces first)
        handler.on_resize(Box::new(|_size| {}));
        handler.on_interrupt(Box::new(|| {}));
        handler.on_suspend(Box::new(|| {}));
        // Should not panic - handlers are simply replaced
    }
}
