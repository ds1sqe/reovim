//! Tests for InputEvent opaque envelope and header codec.

use super::input_event::*;

#[test]
fn new_rejects_short_payload() {
    let result = InputEvent::new(vec![0; 4], None, 0);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(
        err,
        InputPayloadError::TooShort {
            got: 4,
            min: INPUT_HEADER_SIZE
        }
    );
}

#[test]
fn new_accepts_exact_header() {
    let result = InputEvent::new(vec![0; INPUT_HEADER_SIZE], None, 0);
    assert!(result.is_ok());
}

#[test]
fn new_accepts_header_plus_body() {
    let result = InputEvent::new(vec![0; INPUT_HEADER_SIZE + 16], None, 0);
    assert!(result.is_ok());
    assert_eq!(result.unwrap().payload().len(), INPUT_HEADER_SIZE + 16);
}

#[test]
fn input_kind_extracts_correctly() {
    let mut payload = vec![0u8; INPUT_HEADER_SIZE];
    // kind = 0x0001 (little-endian)
    payload[0] = 0x01;
    payload[1] = 0x00;
    assert_eq!(input_kind(&payload), 1);

    // kind = 0x0100
    payload[0] = 0x00;
    payload[1] = 0x01;
    assert_eq!(input_kind(&payload), 256);
}

#[test]
fn input_flags_extracts_correctly() {
    let mut payload = vec![0u8; INPUT_HEADER_SIZE];
    // flags = PRESS | REPEAT
    let flags = InputFlags::PRESS | InputFlags::REPEAT;
    let bytes = flags.bits().to_le_bytes();
    payload[2] = bytes[0];
    payload[3] = bytes[1];
    assert_eq!(input_flags(&payload), InputFlags::PRESS | InputFlags::REPEAT);
}

#[test]
fn input_context_extracts_correctly() {
    let mut payload = vec![0u8; INPUT_HEADER_SIZE];
    // context = 0x12345678
    let ctx: u32 = 0x1234_5678;
    let bytes = ctx.to_le_bytes();
    payload[4] = bytes[0];
    payload[5] = bytes[1];
    payload[6] = bytes[2];
    payload[7] = bytes[3];
    assert_eq!(input_context(&payload), 0x1234_5678);
}

#[test]
fn round_trip_header() {
    let kind: u16 = 0x0042;
    let flags = InputFlags::PRESS | InputFlags::DRAG;
    let context: u32 = 0xDEAD_BEEF;

    let mut payload = vec![0u8; INPUT_HEADER_SIZE + 4];
    payload[0..2].copy_from_slice(&kind.to_le_bytes());
    payload[2..4].copy_from_slice(&flags.bits().to_le_bytes());
    payload[4..8].copy_from_slice(&context.to_le_bytes());

    let event = InputEvent::new(payload, None, 12345).unwrap();
    let p = event.payload();
    assert_eq!(input_kind(p), 0x0042);
    assert_eq!(input_flags(p), InputFlags::PRESS | InputFlags::DRAG);
    assert_eq!(input_context(p), 0xDEAD_BEEF);
    assert_eq!(event.timestamp_ns, 12345);
}

#[test]
fn window_id_preserved() {
    use reovim_kernel::api::WindowId;
    let wid = WindowId::from_raw(42);
    let payload = vec![0u8; INPUT_HEADER_SIZE];
    let event = InputEvent::new(payload, Some(wid), 0).unwrap();
    assert_eq!(event.window_id, Some(WindowId::from_raw(42)));
}

#[test]
fn flags_truncates_unknown_bits() {
    let mut payload = vec![0u8; INPUT_HEADER_SIZE];
    // Set all bits in flags field
    payload[2] = 0xFF;
    payload[3] = 0xFF;
    let flags = input_flags(&payload);
    // Only known bits should be set
    assert!(flags.contains(InputFlags::PRESS));
    assert!(flags.contains(InputFlags::RELEASE));
    assert!(flags.contains(InputFlags::REPEAT));
    assert!(flags.contains(InputFlags::DRAG));
    assert!(flags.contains(InputFlags::BEGIN));
    assert!(flags.contains(InputFlags::END));
}

#[test]
fn payload_error_display() {
    let err = InputPayloadError::TooShort { got: 3, min: 8 };
    assert_eq!(err.to_string(), "input payload too short: got 3, need at least 8");
}

#[test]
fn payload_error_display_wrong_type() {
    let err = InputPayloadError::WrongType {
        expected: "KeyEvent",
    };
    assert_eq!(err.to_string(), "codec encode: wrong input type, expected KeyEvent");
}

#[test]
fn payload_error_display_invalid_data() {
    let err = InputPayloadError::InvalidData {
        reason: "unknown mouse tag",
    };
    assert_eq!(err.to_string(), "input payload invalid: unknown mouse tag");
}

#[test]
fn empty_payload_rejected() {
    let result = InputEvent::new(vec![], None, 0);
    assert!(result.is_err());
}
