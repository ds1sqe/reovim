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
