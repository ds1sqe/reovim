//! 6.3 §12 — Wire frame header.
//!
//! **Scope note (6.3 §12):** this layout crosses the *wire* boundary, not
//! the *cdylib* boundary.  It is never passed across an `extern "C"` cdylib
//! call; it is the fixed prefix of every frame on the server-client stream
//! socket (7.3 §3).  It is registered in the catalog so the catalog remains
//! the single layout authority.
//!
//! The on-wire form is the four fields in declared order, each
//! **little-endian**; total 16 bytes.  The struct itself is never
//! `transmute`d — the encoder/decoder read and write field by field.
//!
//! | offset | size | field | wire encoding |
//! |---|---|---|---|
//! | 0 | 4 | `body_len` | little-endian |
//! | 4 | 2 | `msg_type` | little-endian |
//! | 6 | 2 | `flags` | little-endian |
//! | 8 | 8 | `correlation_id` | little-endian |

/// Fixed 16-byte prefix of every wire frame (6.3 §12, 7.3 §3).
///
/// Size 16, align 8 (natural).  Field semantics and flag-bit assignments are
/// defined in 7.3 §3; this catalog owns only the layout.
///
/// ```rust
/// use reovim_uapi_abi::frame::FrameHeader;
/// use core::mem;
///
/// assert_eq!(mem::size_of::<FrameHeader>(), 16);
/// assert_eq!(mem::align_of::<FrameHeader>(), 8);
/// ```
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameHeader {
    /// Length of the frame body in bytes (little-endian on the wire).
    pub body_len: u32,
    /// Message type tag (little-endian on the wire); matches `TAG` in the
    /// message struct.
    pub msg_type: u16,
    /// Frame flags (little-endian on the wire); bits defined in 7.3 §3.
    pub flags: u16,
    /// Correlation identifier (little-endian on the wire); see 7.3 §4.
    pub correlation_id: u64,
}

impl FrameHeader {
    /// Serializes the header into a caller-provided 16-byte buffer,
    /// little-endian field order (6.3 §12 wire encoding).
    ///
    /// ```rust
    /// use reovim_uapi_abi::frame::FrameHeader;
    ///
    /// let hdr = FrameHeader { body_len: 0x0A, msg_type: 1, flags: 0, correlation_id: 0 };
    /// let mut buf = [0u8; 16];
    /// hdr.encode(&mut buf);
    /// assert_eq!(buf[0], 0x0A); // body_len LE byte 0
    /// assert_eq!(buf[4], 1);    // msg_type LE byte 0
    /// ```
    pub fn encode(&self, buf: &mut [u8; 16]) {
        buf[0..4].copy_from_slice(&self.body_len.to_le_bytes());
        buf[4..6].copy_from_slice(&self.msg_type.to_le_bytes());
        buf[6..8].copy_from_slice(&self.flags.to_le_bytes());
        buf[8..16].copy_from_slice(&self.correlation_id.to_le_bytes());
    }

    /// Deserializes a header from a 16-byte buffer, assuming little-endian
    /// field order (6.3 §12 wire encoding).
    ///
    /// ```rust
    /// use reovim_uapi_abi::frame::FrameHeader;
    ///
    /// let mut buf = [0u8; 16];
    /// buf[0] = 41; // body_len
    /// buf[4] = 1;  // msg_type
    /// let hdr = FrameHeader::decode(&buf);
    /// assert_eq!(hdr.body_len, 41);
    /// assert_eq!(hdr.msg_type, 1);
    /// ```
    #[must_use]
    pub const fn decode(buf: &[u8; 16]) -> Self {
        Self {
            body_len: u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]),
            msg_type: u16::from_le_bytes([buf[4], buf[5]]),
            flags: u16::from_le_bytes([buf[6], buf[7]]),
            correlation_id: u64::from_le_bytes([
                buf[8], buf[9], buf[10], buf[11], buf[12], buf[13], buf[14], buf[15],
            ]),
        }
    }
}
