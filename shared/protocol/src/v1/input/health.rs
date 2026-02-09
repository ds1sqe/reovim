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
mod tests {
    use super::*;

    #[test]
    fn test_ping_event_minimal() {
        let ping = PingEvent::new();
        let json = serde_json::to_string(&ping).unwrap();
        assert_eq!(json, "{}");
    }

    #[test]
    fn test_ping_event_with_seq() {
        let ping = PingEvent::with_seq(42);
        let json = serde_json::to_string(&ping).unwrap();
        assert!(json.contains("\"seq\":42"));
        assert!(!json.contains("\"timestamp\""));
    }

    #[test]
    fn test_ping_event_with_timestamp() {
        let ping = PingEvent::with_timestamp(1_705_000_000_000);
        let json = serde_json::to_string(&ping).unwrap();
        assert!(json.contains("\"timestamp\":1705000000000"));
        assert!(!json.contains("\"seq\""));
    }

    #[test]
    fn test_pong_event_minimal() {
        let pong = PongEvent::new();
        let json = serde_json::to_string(&pong).unwrap();
        assert_eq!(json, "{}");
    }

    #[test]
    fn test_pong_from_ping() {
        let source = PingEvent {
            seq: Some(123),
            timestamp: Some(1_705_000_000_000),
        };
        let response = PongEvent::from_ping(&source);
        assert_eq!(response.seq, Some(123));
        assert_eq!(response.timestamp, Some(1_705_000_000_000));
    }

    #[test]
    fn test_ping_event_default() {
        let ping = PingEvent::default();
        assert!(ping.seq.is_none());
        assert!(ping.timestamp.is_none());
    }

    #[test]
    fn test_pong_event_default() {
        let pong = PongEvent::default();
        assert!(pong.seq.is_none());
        assert!(pong.timestamp.is_none());
    }

    #[test]
    fn test_pong_from_empty_ping() {
        let ping = PingEvent::new();
        let response = PongEvent::from_ping(&ping);
        assert!(response.seq.is_none());
        assert!(response.timestamp.is_none());
    }

    #[test]
    fn test_ping_event_roundtrip() {
        let ping = PingEvent {
            seq: Some(42),
            timestamp: Some(1_000_000),
        };
        let json = serde_json::to_string(&ping).unwrap();
        let decoded: PingEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.seq, Some(42));
        assert_eq!(decoded.timestamp, Some(1_000_000));
    }

    #[test]
    fn test_pong_event_with_seq() {
        let pong = PongEvent::with_seq(999);
        assert_eq!(pong.seq, Some(999));
        assert!(pong.timestamp.is_none());
    }

    #[test]
    fn test_pong_event_roundtrip() {
        let pong = PongEvent {
            seq: Some(10),
            timestamp: Some(5000),
        };
        let json = serde_json::to_string(&pong).unwrap();
        let decoded: PongEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, pong);
    }

    #[test]
    fn test_ping_pong_equality() {
        let a = PingEvent::with_seq(1);
        let b = PingEvent::with_seq(1);
        assert_eq!(a, b);

        let c = PingEvent::with_seq(2);
        assert_ne!(a, c);
    }
}
