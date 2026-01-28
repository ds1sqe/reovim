//! Tracing layer for capturing logs to the ring buffer.
//!
//! This layer captures tracing events and pushes them to the global
//! `LogRingBuffer` for later retrieval via `debug/log_tail`.
//! Also sends entries to the log bridge for real-time notification streaming.

use {
    tracing::{
        Event, Subscriber,
        field::{Field, Visit},
    },
    tracing_subscriber::layer::{Context, Layer},
};

use super::{
    log_bridge::try_send_log,
    log_buffer::{LogEntry, log_buffer},
};

// ============================================================================
// Message Visitor
// ============================================================================

/// Visitor to extract the message field from tracing events.
struct MessageVisitor {
    /// The extracted message.
    message: String,
}

impl MessageVisitor {
    /// Create a new message visitor.
    const fn new() -> Self {
        Self {
            message: String::new(),
        }
    }
}

impl Visit for MessageVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message = format!("{value:?}");
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.message = value.to_string();
        }
    }
}

// ============================================================================
// Log Buffer Layer
// ============================================================================

/// A tracing layer that captures log events to the ring buffer.
///
/// This layer extracts the level, target, and message from each tracing
/// event and pushes it to the global `LogRingBuffer`. Events can then
/// be retrieved via `debug/log_tail`.
///
/// # Example
///
/// ```rust,ignore
/// use tracing_subscriber::prelude::*;
///
/// let subscriber = tracing_subscriber::registry()
///     .with(LogBufferLayer);
///
/// tracing::subscriber::set_global_default(subscriber).unwrap();
/// ```
pub struct LogBufferLayer;

impl<S> Layer<S> for LogBufferLayer
where
    S: Subscriber,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let metadata = event.metadata();
        let level = metadata.level().to_string();
        let target = metadata.target().to_string();

        // Extract the message field from the event
        let mut visitor = MessageVisitor::new();
        event.record(&mut visitor);

        // Create the log entry
        let entry = LogEntry::new(&level, &target, &visitor.message);

        // Push to the global log buffer (for debug/log_tail)
        log_buffer().push(entry.clone());

        // Send to bridge for real-time notifications (non-blocking)
        try_send_log(entry);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_visitor_new() {
        let visitor = MessageVisitor::new();

        // Verify new visitor has empty message
        assert!(visitor.message.is_empty());
    }

    #[test]
    fn test_log_buffer_layer_send_sync() {
        // Verify LogBufferLayer is Send + Sync (required for tracing layers)
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<LogBufferLayer>();
    }
}
