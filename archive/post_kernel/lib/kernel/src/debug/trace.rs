//! Kernel tracing infrastructure.
//!
//! Linux equivalent: `kernel/trace/`
//!
//! Provides trace points for debugging without external dependencies.
//! The driver layer can bridge to `tracing`/`tokio-tracing` if needed.

use std::sync::{
    OnceLock,
    atomic::{AtomicBool, Ordering},
};

/// A trace point that can be enabled/disabled at runtime.
///
/// Linux equivalent: `struct tracepoint`
///
/// # Example
///
/// ```ignore
/// use reovim_kernel::debug::TracePoint;
///
/// static TP_BUFFER_CREATE: TracePoint = TracePoint::new("buffer_create", "mm");
///
/// if TP_BUFFER_CREATE.is_enabled() {
///     // emit trace event
/// }
/// ```
pub struct TracePoint {
    /// Name of the trace point.
    pub name: &'static str,
    /// Module where the trace point is defined.
    pub module: &'static str,
    /// Whether the trace point is enabled.
    enabled: AtomicBool,
}

impl TracePoint {
    /// Create a new trace point (const for static initialization).
    #[must_use]
    pub const fn new(name: &'static str, module: &'static str) -> Self {
        Self {
            name,
            module,
            enabled: AtomicBool::new(false),
        }
    }

    /// Check if the trace point is enabled.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    /// Enable the trace point.
    pub fn enable(&self) {
        self.enabled.store(true, Ordering::Relaxed);
    }

    /// Disable the trace point.
    pub fn disable(&self) {
        self.enabled.store(false, Ordering::Relaxed);
    }
}

/// Trace event data.
#[derive(Debug, Clone)]
pub struct TraceEvent {
    /// Timestamp in nanoseconds since Unix epoch.
    pub timestamp_ns: u64,
    /// Name of the trace point that emitted this event.
    pub tracepoint: &'static str,
    /// Module where the trace point is defined.
    pub module: &'static str,
    /// Event message.
    pub message: String,
}

/// Trace sink trait - driver layer implements this.
///
/// Allows the driver layer to capture trace events and forward them
/// to an external tracing system (e.g., `tracing` crate).
pub trait TraceSink: Send + Sync {
    /// Emit a trace event.
    fn emit(&self, event: TraceEvent);
}

/// Global trace sink (set by driver layer).
static TRACE_SINK: OnceLock<Box<dyn TraceSink>> = OnceLock::new();

/// Set the global trace sink.
///
/// Can only be called once; subsequent calls are ignored.
pub fn set_trace_sink(sink: Box<dyn TraceSink>) {
    let _ = TRACE_SINK.set(sink);
}

/// Emit a trace event to the global sink.
///
/// If no sink is set, this is a no-op.
#[allow(clippy::cast_possible_truncation)] // Timestamp truncation acceptable
pub fn emit_trace(tracepoint: &'static str, module: &'static str, message: String) {
    if let Some(sink) = TRACE_SINK.get() {
        sink.emit(TraceEvent {
            timestamp_ns: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos() as u64),
            tracepoint,
            module,
            message,
        });
    }
}

/// Define a trace point.
///
/// # Example
///
/// ```ignore
/// define_tracepoint!(TP_BUFFER_CREATE, "mm");
///
/// trace_event!(TP_BUFFER_CREATE, "Created buffer {}", id);
/// ```
#[macro_export]
macro_rules! define_tracepoint {
    ($name:ident, $module:expr) => {
        static $name: $crate::debug::TracePoint =
            $crate::debug::TracePoint::new(stringify!($name), $module);
    };
}

/// Emit a trace event if the trace point is enabled.
///
/// # Example
///
/// ```ignore
/// trace_event!(TP_BUFFER_CREATE, "Created buffer with id={}", buffer_id);
/// ```
#[macro_export]
macro_rules! trace_event {
    ($tp:expr, $($arg:tt)*) => {
        if $tp.is_enabled() {
            $crate::debug::emit_trace($tp.name, $tp.module, format!($($arg)*));
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tracepoint_enable_disable() {
        let tp = TracePoint::new("test", "test_module");
        assert!(!tp.is_enabled());

        tp.enable();
        assert!(tp.is_enabled());

        tp.disable();
        assert!(!tp.is_enabled());
    }

    #[test]
    fn test_trace_event_creation() {
        let event = TraceEvent {
            timestamp_ns: 12345,
            tracepoint: "test_tp",
            module: "test_mod",
            message: "test message".to_string(),
        };

        assert_eq!(event.tracepoint, "test_tp");
        assert_eq!(event.module, "test_mod");
        assert_eq!(event.message, "test message");
    }

    #[test]
    fn test_emit_trace_no_sink() {
        // Should not panic when no sink is set
        emit_trace("test", "test_mod", "test message".to_string());
    }
}
