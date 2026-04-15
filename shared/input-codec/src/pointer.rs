//! Pointer (mouse) input codec — encode/decode pointer events into `InputEvent` payload.
//!
//! Kind: `KIND_POINTER = 0x0002`
//!
//! Body layout (after 8-byte header):
//! - bytes 0-1: x position (u16, little-endian)
//! - bytes 2-3: y position (u16, little-endian)
//!
//! Context bits 0-3: button mask

use reovim_subsys_input::{InputFlags, input_event::INPUT_HEADER_SIZE};

/// Well-known kind for pointer/mouse input.
pub const KIND_POINTER: u16 = 0x0002;

/// Pointer event data decoded from an `InputEvent` payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PointerEvent {
    pub x: u16,
    pub y: u16,
    pub button_mask: u8,
    pub flags: InputFlags,
}

/// Encode a `PointerEvent` into an `InputEvent` payload.
pub fn encode(event: &PointerEvent) -> Vec<u8> {
    let context = u32::from(event.button_mask);

    let mut buf = Vec::with_capacity(INPUT_HEADER_SIZE + 4);
    buf.extend(KIND_POINTER.to_le_bytes()); // header: kind
    buf.extend(event.flags.bits().to_le_bytes()); // header: flags
    buf.extend(context.to_le_bytes()); // header: context
    buf.extend(event.x.to_le_bytes()); // body: x
    buf.extend(event.y.to_le_bytes()); // body: y
    buf
}

/// Decode a `PointerEvent` from an `InputEvent` payload.
pub fn decode(payload: &[u8]) -> Option<PointerEvent> {
    if payload.len() < INPUT_HEADER_SIZE + 4 {
        return None;
    }

    let kind = u16::from_le_bytes([payload[0], payload[1]]);
    if kind != KIND_POINTER {
        return None;
    }

    let flags = InputFlags::from_bits_truncate(u16::from_le_bytes([payload[2], payload[3]]));
    let context = u32::from_le_bytes([payload[4], payload[5], payload[6], payload[7]]);

    let x = u16::from_le_bytes([payload[8], payload[9]]);
    let y = u16::from_le_bytes([payload[10], payload[11]]);

    #[allow(clippy::cast_possible_truncation)]
    let button_mask = context as u8;

    Some(PointerEvent {
        x,
        y,
        button_mask,
        flags,
    })
}
