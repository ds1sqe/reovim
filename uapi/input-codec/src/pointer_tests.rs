//! Round-trip tests for pointer codec.

use {
    crate::pointer::{KIND_POINTER, PointerEvent, decode, encode},
    reovim_subsys_input::{InputFlags, input_event::INPUT_HEADER_SIZE, input_kind},
};

#[test]
fn round_trip_click() {
    let event = PointerEvent {
        x: 100,
        y: 50,
        button_mask: 1,
        flags: InputFlags::PRESS,
    };
    let payload = encode(&event);
    assert_eq!(input_kind(&payload), KIND_POINTER);

    let decoded = decode(&payload).unwrap();
    assert_eq!(decoded.x, 100);
    assert_eq!(decoded.y, 50);
    assert_eq!(decoded.button_mask, 1);
    assert_eq!(decoded.flags, InputFlags::PRESS);
}

#[test]
fn round_trip_drag() {
    let event = PointerEvent {
        x: 200,
        y: 300,
        button_mask: 1,
        flags: InputFlags::DRAG,
    };
    let payload = encode(&event);
    let decoded = decode(&payload).unwrap();
    assert_eq!(decoded.flags, InputFlags::DRAG);
}

#[test]
fn round_trip_release() {
    let event = PointerEvent {
        x: 0,
        y: 0,
        button_mask: 0,
        flags: InputFlags::RELEASE,
    };
    let payload = encode(&event);
    let decoded = decode(&payload).unwrap();
    assert_eq!(decoded.flags, InputFlags::RELEASE);
    assert_eq!(decoded.button_mask, 0);
}

#[test]
fn decode_wrong_kind_returns_none() {
    let mut payload = vec![0u8; INPUT_HEADER_SIZE + 4];
    payload[0] = 0x01; // KIND_KEY
    assert!(decode(&payload).is_none());
}

#[test]
fn decode_short_payload_returns_none() {
    let payload = vec![0u8; INPUT_HEADER_SIZE]; // no body
    assert!(decode(&payload).is_none());
}
