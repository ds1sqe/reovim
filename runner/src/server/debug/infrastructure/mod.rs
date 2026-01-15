//! Debug infrastructure components.
//!
//! This module provides infrastructure for debug features:
//! - Uptime tracking
//! - Handler metrics (Phase 3)
//! - Log buffer (Phase 4)
//! - Log buffer tracing layer (Phase 4 enhancement)

pub mod log_buffer;
pub mod log_buffer_layer;
pub mod metrics;
pub mod uptime;

pub use {
    log_buffer::{LogEntry, LogRingBuffer, current_log_level, log_buffer},
    log_buffer_layer::LogBufferLayer,
    metrics::{
        HandlerMetricsSnapshot, RequestTimer, increment_total_requests, snapshot_handler_metrics,
        total_requests,
    },
    uptime::{init_server_start, start_time_iso, uptime, uptime_human, uptime_seconds},
};
