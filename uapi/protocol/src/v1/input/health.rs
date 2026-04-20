//! Connection health event types for RPC protocol.
//!
//! Ping/pong events for connection health monitoring.

use serde::{Deserialize, Serialize};

/// Ping event (serializable).
///
/// Sent by either client or server to check connection health.
/// The receiver should respond with a matching [`PongEvent`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PingEvent {
    /// Optional sequence number for matching ping/pong pairs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seq: Option<u64>,
    /// Optional timestamp (milliseconds since epoch).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<u64>,
}

impl PingEvent {
    /// Create a new ping event without sequence.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            seq: None,
            timestamp: None,
        }
    }

    /// Create a ping event with sequence number.
    #[must_use]
    pub const fn with_seq(seq: u64) -> Self {
        Self {
            seq: Some(seq),
            timestamp: None,
        }
    }

    /// Create a ping event with timestamp.
    #[must_use]
    pub const fn with_timestamp(timestamp: u64) -> Self {
        Self {
            seq: None,
            timestamp: Some(timestamp),
        }
    }
}

impl Default for PingEvent {
    fn default() -> Self {
        Self::new()
    }
}

/// Pong event (serializable).
///
/// Response to a [`PingEvent`]. Should echo the same sequence number
/// if one was provided in the ping.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PongEvent {
    /// Sequence number echoed from the ping.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seq: Option<u64>,
    /// Original timestamp from the ping.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<u64>,
}

impl PongEvent {
    /// Create a new pong event without sequence.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            seq: None,
            timestamp: None,
        }
    }

    /// Create a pong event with sequence number.
    #[must_use]
    pub const fn with_seq(seq: u64) -> Self {
        Self {
            seq: Some(seq),
            timestamp: None,
        }
    }

    /// Create a pong from a ping (echoes seq and timestamp).
    #[must_use]
    pub const fn from_ping(ping: &PingEvent) -> Self {
        Self {
            seq: ping.seq,
            timestamp: ping.timestamp,
        }
    }
}

impl Default for PongEvent {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "health_tests.rs"]
mod tests;
