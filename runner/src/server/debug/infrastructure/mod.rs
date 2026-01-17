//! Debug infrastructure components.
//!
//! This module provides infrastructure for debug features:
//! - Uptime tracking
//! - Handler metrics (Phase 3)
//! - Log buffer (Phase 4)
//! - Log buffer tracing layer (Phase 4 enhancement)
//! - Log subscription for real-time streaming (#332)
//! - Log bridge for async notification broadcasting (#332)
//! - Dynamic log level control (#324)

pub mod log_bridge;
pub mod log_buffer;
pub mod log_buffer_layer;
pub mod log_level;
pub mod log_subscriber;
pub mod metrics;
pub mod uptime;

pub use {
    log_bridge::{dropped_count, init_log_bridge, spawn_drain_task, try_send_log},
    log_buffer::{LogEntry, LogRingBuffer, current_log_level, log_buffer},
    log_buffer_layer::LogBufferLayer,
    log_level::{LogLevelError, get_current_level, init_level_handle, set_log_level},
    log_subscriber::{
        LogSubscribers, LogSubscription, SubscriptionId, entry_to_payload, log_subscribers,
    },
    metrics::{
        HandlerMetricsSnapshot, RequestTimer, increment_total_requests, snapshot_handler_metrics,
        total_requests,
    },
    uptime::{init_server_start, start_time_iso, uptime, uptime_human, uptime_seconds},
};
