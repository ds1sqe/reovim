//! `Codec` implementations for TUI keyboard, mouse, and scroll events.
//!
//! Each codec encodes its typed vocabulary into opaque body bytes as required
//! by the `Codec` trait in `reovim-input-codec`.  The 8-byte header is managed
//! by the registry / `InputEvent` layer, NOT by these codec implementations.
//!
//! # Kind assignments
//!
//! | Kind   | Codec           | Vocab type    |
//! |--------|-----------------|---------------|
//! | 0x0001 | `TuiKeyCodec`   | `KeyEvent`    |
//! | 0x0002 | `TuiMouseCodec` | `MouseEvent`  |
//! | 0x0003 | `TuiScrollCodec`| `ScrollEvent` |
//!
//! # Body layouts (bytes AFTER the 8-byte header)
//!
//! ## `TuiKeyCodec` (KIND=0x0001) — body is 4 bytes
//!
//! ```text
//! The header (managed externally) carries:
//!   kind     = 0x0001 (u16 LE)
//!   flags    = InputFlags (u16 LE): PRESS | REPEAT | RELEASE
//!   context  = modifiers byte in bits 0-7 (u32 LE)
//!
//! Body bytes returned by encode() / passed to decode():
//!   [0..4]   keycode = u32 LE (see key_types::keycode_to_u32 table)
//! ```
//!
//! The full wire format (header+body) used when building an `InputEvent`:
//! ```text
//!   [0..2]   kind    (u16 LE)
//!   [2..4]   flags   (u16 LE)
//!   [4..8]   context (u32 LE)
//!   [8..12]  keycode (u32 LE)   <- body
//! ```
//!
//! ## `TuiMouseCodec` (KIND=0x0002) — body is 6 bytes
//!
//! ```text
//! Header (external):
//!   flags   = InputFlags::empty
//!   context = modifiers byte in bits 0-7 (u32 LE)
//!
//! Body bytes returned by encode() / passed to decode():
//!   [0]      tag    = event-kind discriminant (u8, see below)
//!   [1]      button = button id or 0           (u8)
//!   [2..4]   column = u16 LE
//!   [4..6]   row    = u16 LE
//!
//! tag/button encoding:
//!   tag=0 button=0   Moved
//!   tag=1 button=b   Down(b)
//!   tag=2 button=b   Up(b)
//!   tag=3 button=b   Drag(b)
//!   tag=4 button=0   ScrollUp
//!   tag=5 button=0   ScrollDown
//!   tag=6 button=0   ScrollLeft
//!   tag=7 button=0   ScrollRight
//!   button: Left=0, Right=1, Middle=2
//! ```
//!
//! Full wire format: header[0..8] + body[8..14].
//!
//! ## `TuiScrollCodec` (KIND=0x0003) — body is 10 bytes
//!
//! ```text
//! Header (external):
//!   flags   = InputFlags::empty
//!   context = modifiers byte in bits 0-7 (u32 LE)
//!
//! Body bytes returned by encode() / passed to decode():
//!   [0..2]   dx   = i16 LE (horizontal delta; positive = right)
//!   [2..4]   dy   = i16 LE (vertical delta; positive = down)
//!   [4..6]   x    = u16 LE (pointer column)
//!   [6..8]   y    = u16 LE (pointer row)
//!   [8..10]  _pad = u16 LE, reserved (written as 0)
//! ```
//!
//! Full wire format: header[0..8] + body[8..18].
//!
//! # Header assembly helper
//!
//! To produce a full `InputEvent` payload from encode() output, callers
//! assemble: `[kind_le, flags_le, context_le] + body`.
//! The `build_full_payload` helper function does this.

use {
    reovim_input_codec::{Codec, InputFlags, InputPayloadError},
    std::sync::Arc,
};

use crate::{
    KeyEvent, KeyEventKind, Modifiers, MouseEvent, ScrollEvent,
    key_types::{keycode_to_u32, u32_to_keycode},
    mouse_types::{bytes_to_mouse_kind, mouse_kind_to_bytes},
};

// ---------------------------------------------------------------------------
// Header assembly helper
// ---------------------------------------------------------------------------

/// Assemble a full `InputEvent` payload from header fields and a body slice.
///
/// Used by callers that need to construct an `InputEvent` from typed events.
/// ```text
/// layout: [kind(u16 LE), flags(u16 LE), context(u32 LE), body...]
/// ```
#[must_use]
pub fn build_full_payload(kind: u16, flags: InputFlags, context: u32, body: &[u8]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(8 + body.len());
    buf.extend(kind.to_le_bytes());
    buf.extend(flags.bits().to_le_bytes());
    buf.extend(context.to_le_bytes());
    buf.extend(body);
    buf
}

// ---------------------------------------------------------------------------
// TuiKeyCodec
// ---------------------------------------------------------------------------

/// Codec for `KeyEvent` (KIND = 0x0001).
#[derive(Debug, Clone, Copy, Default)]
pub struct TuiKeyCodec;

/// Well-known kind value for TUI keyboard events.
pub const KIND_KEY: u16 = 0x0001;

impl Codec for TuiKeyCodec {
    fn kind(&self) -> u16 {
        KIND_KEY
    }

    fn encode(&self, value: &dyn std::any::Any) -> Result<Vec<u8>, InputPayloadError> {
        let event: &KeyEvent = value
            .downcast_ref()
            .ok_or(InputPayloadError::WrongType { expected: "KeyEvent" })?;
        Ok(encode_key_body(event))
    }

    fn decode(&self, body: &[u8]) -> Result<Box<dyn std::any::Any + Send>, InputPayloadError> {
        decode_key_body(body).map(|e| Box::new(e) as Box<dyn std::any::Any + Send>)
    }
}

/// Encode a `KeyEvent` to a 4-byte body (no header).
#[must_use]
pub fn encode_key_body(event: &KeyEvent) -> Vec<u8> {
    keycode_to_u32(&event.code).to_le_bytes().to_vec()
}

/// Encode a `KeyEvent` to a full payload (8-byte header + 4-byte body).
///
/// This is the convenience form used when constructing an `InputEvent`.
#[must_use]
pub fn encode_key_event(event: &KeyEvent) -> Vec<u8> {
    let flags = match event.kind {
        KeyEventKind::Press => InputFlags::PRESS,
        KeyEventKind::Repeat => InputFlags::REPEAT,
        KeyEventKind::Release => InputFlags::RELEASE,
    };
    let context = u32::from(event.modifiers.bits());
    let body = encode_key_body(event);
    build_full_payload(KIND_KEY, flags, context, &body)
}

/// Decode a `KeyEvent` from a 4-byte body slice (no header).
///
/// # Errors
///
/// Returns `InputPayloadError::TooShort` when `body.len() < 4`.
pub fn decode_key_body(body: &[u8]) -> Result<KeyEvent, InputPayloadError> {
    if body.len() < 4 {
        return Err(InputPayloadError::TooShort {
            got: body.len(),
            min: 4,
        });
    }
    let keycode_u32 = u32::from_le_bytes([body[0], body[1], body[2], body[3]]);
    let code = u32_to_keycode(keycode_u32);
    Ok(KeyEvent {
        code,
        modifiers: Modifiers::NONE,
        kind: KeyEventKind::Press,
    })
}

/// Decode a `KeyEvent` from a full payload (8-byte header + 4-byte body).
///
/// Reads modifier bits and event kind from the header fields.
///
/// # Errors
///
/// Returns `InputPayloadError::TooShort` when payload is shorter than 12 bytes.
pub fn decode_key_event(payload: &[u8]) -> Result<KeyEvent, InputPayloadError> {
    const REQUIRED: usize = 12; // 8 header + 4 body
    if payload.len() < REQUIRED {
        return Err(InputPayloadError::TooShort {
            got: payload.len(),
            min: REQUIRED,
        });
    }
    let flags = InputFlags::from_bits_truncate(u16::from_le_bytes([payload[2], payload[3]]));
    let context = u32::from_le_bytes([payload[4], payload[5], payload[6], payload[7]]);
    let body = &payload[8..];
    let keycode_u32 = u32::from_le_bytes([body[0], body[1], body[2], body[3]]);

    let event_kind = if flags.contains(InputFlags::PRESS) {
        KeyEventKind::Press
    } else if flags.contains(InputFlags::REPEAT) {
        KeyEventKind::Repeat
    } else {
        KeyEventKind::Release
    };

    #[allow(clippy::cast_possible_truncation)]
    let modifiers = Modifiers::from_bits_truncate(context as u8);
    let code = u32_to_keycode(keycode_u32);

    Ok(KeyEvent {
        code,
        modifiers,
        kind: event_kind,
    })
}

// ---------------------------------------------------------------------------
// TuiMouseCodec
// ---------------------------------------------------------------------------

/// Codec for `MouseEvent` (KIND = 0x0002).
#[derive(Debug, Clone, Copy, Default)]
pub struct TuiMouseCodec;

/// Well-known kind value for TUI mouse events.
pub const KIND_MOUSE: u16 = 0x0002;

impl Codec for TuiMouseCodec {
    fn kind(&self) -> u16 {
        KIND_MOUSE
    }

    fn encode(&self, value: &dyn std::any::Any) -> Result<Vec<u8>, InputPayloadError> {
        let event: &MouseEvent = value
            .downcast_ref()
            .ok_or(InputPayloadError::WrongType { expected: "MouseEvent" })?;
        Ok(encode_mouse_body(event))
    }

    fn decode(&self, body: &[u8]) -> Result<Box<dyn std::any::Any + Send>, InputPayloadError> {
        decode_mouse_body(body).map(|e| Box::new(e) as Box<dyn std::any::Any + Send>)
    }
}

/// Encode a `MouseEvent` to a 6-byte body (no header).
#[must_use]
pub fn encode_mouse_body(event: &MouseEvent) -> Vec<u8> {
    let (tag, button) = mouse_kind_to_bytes(&event.kind);
    let mut buf = Vec::with_capacity(6);
    buf.push(tag);
    buf.push(button);
    buf.extend(event.column.to_le_bytes());
    buf.extend(event.row.to_le_bytes());
    buf
}

/// Encode a `MouseEvent` to a full payload (8-byte header + 6-byte body).
#[must_use]
pub fn encode_mouse_event(event: &MouseEvent) -> Vec<u8> {
    let context = u32::from(event.modifiers.bits());
    let body = encode_mouse_body(event);
    build_full_payload(KIND_MOUSE, InputFlags::empty(), context, &body)
}

/// Decode a `MouseEvent` from a 6-byte body slice (no header).
///
/// # Errors
///
/// - [`InputPayloadError::TooShort`] when `body.len() < 6`.
/// - [`InputPayloadError::InvalidData`] when the tag/button combination
///   does not identify a known `MouseEventKind`.
pub fn decode_mouse_body(body: &[u8]) -> Result<MouseEvent, InputPayloadError> {
    if body.len() < 6 {
        return Err(InputPayloadError::TooShort {
            got: body.len(),
            min: 6,
        });
    }
    let tag = body[0];
    let button = body[1];
    let column = u16::from_le_bytes([body[2], body[3]]);
    let row = u16::from_le_bytes([body[4], body[5]]);
    let kind = bytes_to_mouse_kind(tag, button).ok_or(InputPayloadError::InvalidData {
        reason: "unknown mouse tag/button combination",
    })?;
    Ok(MouseEvent {
        kind,
        column,
        row,
        modifiers: Modifiers::NONE,
    })
}

/// Decode a `MouseEvent` from a full payload (8-byte header + 6-byte body).
///
/// # Errors
///
/// Returns `InputPayloadError::TooShort` when payload is shorter than 14 bytes.
pub fn decode_mouse_event(payload: &[u8]) -> Result<MouseEvent, InputPayloadError> {
    const REQUIRED: usize = 14; // 8 header + 6 body
    if payload.len() < REQUIRED {
        return Err(InputPayloadError::TooShort {
            got: payload.len(),
            min: REQUIRED,
        });
    }
    let context = u32::from_le_bytes([payload[4], payload[5], payload[6], payload[7]]);
    #[allow(clippy::cast_possible_truncation)]
    let modifiers = Modifiers::from_bits_truncate(context as u8);
    let body = &payload[8..];
    let mut event = decode_mouse_body(body)?;
    event.modifiers = modifiers;
    Ok(event)
}

// ---------------------------------------------------------------------------
// TuiScrollCodec
// ---------------------------------------------------------------------------

/// Codec for `ScrollEvent` (KIND = 0x0003).
#[derive(Debug, Clone, Copy, Default)]
pub struct TuiScrollCodec;

/// Well-known kind value for TUI scroll events.
pub const KIND_SCROLL: u16 = 0x0003;

impl Codec for TuiScrollCodec {
    fn kind(&self) -> u16 {
        KIND_SCROLL
    }

    fn encode(&self, value: &dyn std::any::Any) -> Result<Vec<u8>, InputPayloadError> {
        let event: &ScrollEvent = value
            .downcast_ref()
            .ok_or(InputPayloadError::WrongType { expected: "ScrollEvent" })?;
        Ok(encode_scroll_body(event))
    }

    fn decode(&self, body: &[u8]) -> Result<Box<dyn std::any::Any + Send>, InputPayloadError> {
        decode_scroll_body(body).map(|e| Box::new(e) as Box<dyn std::any::Any + Send>)
    }
}

/// Encode a `ScrollEvent` to a 10-byte body (no header).
#[must_use]
pub fn encode_scroll_body(event: &ScrollEvent) -> Vec<u8> {
    let mut buf = Vec::with_capacity(10);
    buf.extend(event.dx.to_le_bytes());
    buf.extend(event.dy.to_le_bytes());
    buf.extend(event.x.to_le_bytes());
    buf.extend(event.y.to_le_bytes());
    buf.extend(0u16.to_le_bytes()); // padding / reserved
    buf
}

/// Encode a `ScrollEvent` to a full payload (8-byte header + 10-byte body).
#[must_use]
pub fn encode_scroll_event(event: &ScrollEvent) -> Vec<u8> {
    let context = u32::from(event.modifiers.bits());
    let body = encode_scroll_body(event);
    build_full_payload(KIND_SCROLL, InputFlags::empty(), context, &body)
}

/// Decode a `ScrollEvent` from a 10-byte body slice (no header).
///
/// # Errors
///
/// Returns `InputPayloadError::TooShort` when `body.len() < 10`.
pub fn decode_scroll_body(body: &[u8]) -> Result<ScrollEvent, InputPayloadError> {
    if body.len() < 10 {
        return Err(InputPayloadError::TooShort {
            got: body.len(),
            min: 10,
        });
    }
    let dx = i16::from_le_bytes([body[0], body[1]]);
    let dy = i16::from_le_bytes([body[2], body[3]]);
    let x = u16::from_le_bytes([body[4], body[5]]);
    let y = u16::from_le_bytes([body[6], body[7]]);
    Ok(ScrollEvent {
        dx,
        dy,
        x,
        y,
        modifiers: Modifiers::NONE,
    })
}

/// Decode a `ScrollEvent` from a full payload (8-byte header + 10-byte body).
///
/// # Errors
///
/// Returns `InputPayloadError::TooShort` when payload is shorter than 18 bytes.
pub fn decode_scroll_event(payload: &[u8]) -> Result<ScrollEvent, InputPayloadError> {
    const REQUIRED: usize = 18; // 8 header + 10 body
    if payload.len() < REQUIRED {
        return Err(InputPayloadError::TooShort {
            got: payload.len(),
            min: REQUIRED,
        });
    }
    let context = u32::from_le_bytes([payload[4], payload[5], payload[6], payload[7]]);
    #[allow(clippy::cast_possible_truncation)]
    let modifiers = Modifiers::from_bits_truncate(context as u8);
    let body = &payload[8..];
    let mut event = decode_scroll_body(body)?;
    event.modifiers = modifiers;
    Ok(event)
}

/// Create an `Arc<TuiKeyCodec>` for registry insertion.
#[must_use]
pub fn tui_key_codec() -> Arc<TuiKeyCodec> {
    Arc::new(TuiKeyCodec)
}

/// Create an `Arc<TuiMouseCodec>` for registry insertion.
#[must_use]
pub fn tui_mouse_codec() -> Arc<TuiMouseCodec> {
    Arc::new(TuiMouseCodec)
}

/// Create an `Arc<TuiScrollCodec>` for registry insertion.
#[must_use]
pub fn tui_scroll_codec() -> Arc<TuiScrollCodec> {
    Arc::new(TuiScrollCodec)
}
