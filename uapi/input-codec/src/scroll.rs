//! Scroll input codec — encode/decode scroll events into `InputEvent` payload.
//!
//! Kind: `KIND_SCROLL = 0x0003`
//!
//! Body layout (after 8-byte header):
//! - bytes 0-1: dx (i16, little-endian, horizontal scroll)
//! - bytes 2-3: dy (i16, little-endian, vertical scroll)
//! - bytes 4-5: x position (u16, little-endian)
//! - bytes 6-7: y position (u16, little-endian)

use reovim_subsys_input::{InputFlags, input_event::INPUT_HEADER_SIZE};

/// Well-known kind for scroll input.
pub const KIND_SCROLL: u16 = 0x0003;

/// Scroll event data decoded from an `InputEvent` payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScrollEvent {
    pub dx: i16,
    pub dy: i16,
    pub x: u16,
    pub y: u16,
}

/// Encode a `ScrollEvent` into an `InputEvent` payload.
pub fn encode(event: &ScrollEvent) -> Vec<u8> {
    let mut buf = Vec::with_capacity(INPUT_HEADER_SIZE + 8);
    buf.extend(KIND_SCROLL.to_le_bytes()); // header: kind
    buf.extend(InputFlags::empty().bits().to_le_bytes()); // header: flags
    buf.extend(0u32.to_le_bytes()); // header: context
    buf.extend(event.dx.to_le_bytes()); // body: dx
    buf.extend(event.dy.to_le_bytes()); // body: dy
    buf.extend(event.x.to_le_bytes()); // body: x
    buf.extend(event.y.to_le_bytes()); // body: y
    buf
}

/// Decode a `ScrollEvent` from an `InputEvent` payload.
pub fn decode(payload: &[u8]) -> Option<ScrollEvent> {
    if payload.len() < INPUT_HEADER_SIZE + 8 {
        return None;
    }

    let kind = u16::from_le_bytes([payload[0], payload[1]]);
    if kind != KIND_SCROLL {
        return None;
    }

    let dx = i16::from_le_bytes([payload[8], payload[9]]);
    let dy = i16::from_le_bytes([payload[10], payload[11]]);
    let x = u16::from_le_bytes([payload[12], payload[13]]);
    let y = u16::from_le_bytes([payload[14], payload[15]]);

    Some(ScrollEvent { dx, dy, x, y })
}
