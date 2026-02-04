//! Debug infrastructure for post-mortem analysis.
//!
//! This module provides server-level debugging tools:
//! - Ring buffer for capturing events for crash analysis
//! - Composite logger for dual output (ring buffer + tracing)
//! - Panic integration for crash reports

mod composite_logger;
mod ring_buffer;

pub use {
    composite_logger::{COMPOSITE_LOGGER, CompositeLogger},
    ring_buffer::{
        AlreadyInitialized, BufferStats, DEFAULT_CAPACITY, DebugRingBuffer, LogEntry, LogEntryView,
        MAX_MESSAGE_LEN, debug_ring, init_debug_ring, init_debug_ring_with_capacity,
        try_debug_ring,
    },
};
