//! Opaque input event envelope — domain-neutral input transport.
//!
//! `InputEvent` is the single input type crossing the server→driver boundary.
//! Subsys defines the envelope and header format. Codec crates outside subsys
//! define kind values and body layouts.
//!
//! Linux equivalent: `struct input_event` in `include/uapi/linux/input.h`.

use bitflags::bitflags;
use reovim_kernel::api::WindowId;

/// Minimum payload size: 8-byte header.
pub const INPUT_HEADER_SIZE: usize = 8;

/// Opaque input event envelope.
///
/// The payload is private — construction goes through `new()`, which validates
/// minimum length. Header accessor functions are guaranteed safe on a valid
/// `InputEvent`.
///
/// # Header layout (8 bytes, frozen forever)
///
/// ```text
///  0                   1                   2                   3
///  0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
/// |            kind (u16)         |           flags (u16)         |
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
/// |                        context (u32)                          |
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
/// |                     body (kind-specific)                      |
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
/// ```
#[derive(Debug, Clone)]
pub struct InputEvent {
    payload: Vec<u8>,
    pub window_id: Option<WindowId>,
    pub timestamp_ns: u64,
}

/// Diagnostic error for `InputEvent` construction.
/// `#[non_exhaustive]` allows adding variants without breaking callers.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputPayloadError {
    /// Payload shorter than the required header.
    TooShort { got: usize, min: usize },
}

impl std::fmt::Display for InputPayloadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooShort { got, min } => {
                write!(f, "input payload too short: got {got}, need at least {min}")
            }
        }
    }
}

impl std::error::Error for InputPayloadError {}

impl InputEvent {
    /// Construct with validation. Returns `Err` with diagnostics if payload
    /// is shorter than `INPUT_HEADER_SIZE`.
    pub fn new(
        payload: Vec<u8>,
        window_id: Option<WindowId>,
        timestamp_ns: u64,
    ) -> Result<Self, InputPayloadError> {
        if payload.len() < INPUT_HEADER_SIZE {
            return Err(InputPayloadError::TooShort {
                got: payload.len(),
                min: INPUT_HEADER_SIZE,
            });
        }
        Ok(Self {
            payload,
            window_id,
            timestamp_ns,
        })
    }

    /// Access the raw payload bytes (header + body).
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }
}

/// Extract the kind field (bytes 0-1, little-endian) from a payload.
///
/// # Panics
///
/// Panics if `payload.len() < INPUT_HEADER_SIZE`. Only call on a validated
/// `InputEvent`'s payload.
pub fn input_kind(payload: &[u8]) -> u16 {
    u16::from_le_bytes([payload[0], payload[1]])
}

/// Extract the flags field (bytes 2-3, little-endian) from a payload.
///
/// # Panics
///
/// Panics if `payload.len() < INPUT_HEADER_SIZE`.
pub fn input_flags(payload: &[u8]) -> InputFlags {
    InputFlags::from_bits_truncate(u16::from_le_bytes([payload[2], payload[3]]))
}

/// Extract the context field (bytes 4-7, little-endian) from a payload.
///
/// # Panics
///
/// Panics if `payload.len() < INPUT_HEADER_SIZE`.
pub fn input_context(payload: &[u8]) -> u32 {
    u32::from_le_bytes([payload[4], payload[5], payload[6], payload[7]])
}

bitflags! {
    /// Universal input lifecycle flags. Subsys defines the flag MEANINGS —
    /// kind values and body layouts live in codec crates.
    ///
    /// These are universal lifecycle phases that apply across all modalities:
    /// keyboard press/release, gesture begin/end, voice utterance start/stop.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct InputFlags: u16 {
        /// Key or gesture pressed/activated.
        const PRESS   = 1 << 0;
        /// Key or gesture released/deactivated.
        const RELEASE = 1 << 1;
        /// Key held down (auto-repeat).
        const REPEAT  = 1 << 2;
        /// Pointer/gesture dragging.
        const DRAG    = 1 << 3;
        /// Gesture/utterance/sequence beginning.
        const BEGIN   = 1 << 4;
        /// Gesture/utterance/sequence ending.
        const END     = 1 << 5;
    }
}
