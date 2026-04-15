//! Round-trip tests for scroll codec.

use crate::scroll::{decode, encode, ScrollEvent, KIND_SCROLL};
use reovim_subsys_input::{input_event::INPUT_HEADER_SIZE, input_kind};

#[test]
fn round_trip_scroll_down() {
    let event = ScrollEvent {
        dx: 0,
        dy: -3,
        x: 50,
        y: 100,
    };
    let payload = encode(&event);
    assert_eq!(input_kind(&payload), KIND_SCROLL);

    let decoded = decode(&payload).unwrap();
    assert_eq!(decoded.dx, 0);
    assert_eq!(decoded.dy, -3);
    assert_eq!(decoded.x, 50);
    assert_eq!(decoded.y, 100);
}

#[test]
fn round_trip_scroll_horizontal() {
    let event = ScrollEvent {
        dx: 5,
        dy: 0,
        x: 200,
        y: 300,
    };
    let payload = encode(&event);
    let decoded = decode(&payload).unwrap();
    assert_eq!(decoded.dx, 5);
    assert_eq!(decoded.dy, 0);
}

#[test]
fn round_trip_negative_scroll() {
    let event = ScrollEvent {
        dx: -10,
        dy: -20,
        x: 0,
        y: 0,
    };
    let payload = encode(&event);
    let decoded = decode(&payload).unwrap();
    assert_eq!(decoded.dx, -10);
    assert_eq!(decoded.dy, -20);
}

#[test]
fn decode_wrong_kind_returns_none() {
    let mut payload = vec![0u8; INPUT_HEADER_SIZE + 8];
    payload[0] = 0x01; // KIND_KEY
    assert!(decode(&payload).is_none());
}

#[test]
fn decode_short_payload_returns_none() {
    let payload = vec![0u8; INPUT_HEADER_SIZE + 4]; // need 8 body bytes
    assert!(decode(&payload).is_none());
}
