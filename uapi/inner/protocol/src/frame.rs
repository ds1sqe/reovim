//! Frame assembly: the 16-byte header (6.3 §12 / 7.3 §3) plus a framing
//! helper that validates `body_len` against the SP10 cap.
//!
//! The header layout and field-by-field codec live in `reovim-uapi-abi`
//! ([`FrameHeader`]); this module adds the protocol-level framing rules:
//! a reader reads 16 header bytes, then `body_len` body bytes, and a declared
//! `body_len` exceeding `wire-max-frame-bytes` is a protocol violation
//! (SP10).  The cap is supplied by the caller — the protocol stays
//! config-agnostic (L11: it never reads config).

use reovim_uapi_abi::{ErrorCode, FrameHeader};

/// Size of the fixed frame header prefix, in bytes (6.3 §12, 7.3 §3).
pub const HEADER_LEN: usize = 16;

/// Frame flag bit: the body is a fragment (`MORE`); a later frame with the
/// same `(msg_type, correlation_id)` and `MORE` clear completes it (7.3 §3).
pub const FLAG_MORE: u16 = 1 << 0;

/// Frame flag bit: the body is compressed (`COMPRESSED`); reserved, v1 sets 0
/// and rejects 1 (7.3 §3).
pub const FLAG_COMPRESSED: u16 = 1 << 1;

/// Mask of the flag bits a v1 frame may carry: `MORE` only.
///
/// `COMPRESSED` is reserved — v1 sets it to 0 and REJECTS 1 (7.3 §3) — so
/// it is named in the catalog above but absent from the accept mask; any
/// bit outside this mask is a protocol violation.
pub const FLAG_V1_ACCEPT_MASK: u16 = FLAG_MORE;

/// A framed view over a byte buffer: the parsed header plus the borrowed body
/// slice (SP17).  Produced by [`read_frame`].
///
/// # Examples
///
/// ```rust
/// use reovim_uapi_protocol::frame::{read_frame, HEADER_LEN};
/// use reovim_uapi_abi::FrameHeader;
///
/// let hdr = FrameHeader { body_len: 2, msg_type: 1, flags: 0, correlation_id: 0 };
/// let mut buf = [0u8; HEADER_LEN + 2];
/// let mut h16 = [0u8; HEADER_LEN];
/// hdr.encode(&mut h16);
/// buf[..HEADER_LEN].copy_from_slice(&h16);
/// buf[HEADER_LEN] = 0xAA;
/// buf[HEADER_LEN + 1] = 0xBB;
///
/// let frame = read_frame(&buf, 4096).unwrap();
/// assert_eq!(frame.header.msg_type, 1);
/// assert_eq!(frame.body, &[0xAA, 0xBB]);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameView<'a> {
    /// The parsed 16-byte frame header.
    pub header: FrameHeader,
    /// The borrowed body bytes (`header.body_len` bytes).
    pub body: &'a [u8],
}

/// Writes a header + body into `out`, returning the total bytes written.
///
/// The header's `body_len` is set from `body.len()`.  A short output buffer
/// fails [`ErrorCode::BufferTooSmall`] with nothing relied upon; a `body`
/// exceeding `cap` (`wire-max-frame-bytes`) fails
/// [`ErrorCode::ResourceExhausted`] before writing (SP10/SP17 codec-boundary
/// cap enforcement).
///
/// # Examples
///
/// ```rust
/// use reovim_uapi_protocol::frame::write_frame;
/// use reovim_uapi_abi::FrameHeader;
///
/// let hdr = FrameHeader { body_len: 0, msg_type: 1, flags: 0, correlation_id: 0 };
/// let mut buf = [0u8; 32];
/// let n = write_frame(&mut buf, hdr, &[0xAB], 4096).unwrap();
/// assert_eq!(n, 17);
/// assert_eq!(buf[16], 0xAB);
/// ```
///
/// # Errors
///
/// Fails with [`ErrorCode::BufferTooSmall`] / [`ErrorCode::ProtocolViolation`] / [`ErrorCode::ResourceExhausted`] under the conditions described above.
pub fn write_frame(
    out: &mut [u8],
    mut header: FrameHeader,
    body: &[u8],
    cap: usize,
) -> Result<usize, ErrorCode> {
    if body.len() > cap {
        return Err(ErrorCode::ResourceExhausted);
    }
    let body_len = u32::try_from(body.len()).map_err(|_| ErrorCode::ResourceExhausted)?;
    header.body_len = body_len;
    // `body.len() <= u32::MAX` (the try_from above succeeded), so adding the
    // 16-byte header cannot wrap a usize on the only target (x86_64).
    let total = HEADER_LEN + body.len();
    if out.len() < total {
        return Err(ErrorCode::BufferTooSmall);
    }
    let mut h16 = [0u8; HEADER_LEN];
    header.encode(&mut h16);
    out[..HEADER_LEN].copy_from_slice(&h16);
    out[HEADER_LEN..total].copy_from_slice(body);
    Ok(total)
}

/// Parses a frame from `buf`: reads the 16-byte header, validates `body_len`
/// against `cap` (`wire-max-frame-bytes`), and returns the header plus the
/// borrowed body (SP10, SP17).
///
/// - `buf.len() < 16` fails [`ErrorCode::ProtocolViolation`] (a partial
///   header is not yet a frame; the caller buffers more bytes first).
/// - `body_len > cap` fails [`ErrorCode::ProtocolViolation`] (terminal: SP10
///   closes the connection).
/// - `buf` shorter than `16 + body_len` fails [`ErrorCode::ProtocolViolation`]
///   (the caller has not yet read the full body).
///
/// # Examples
///
/// ```rust
/// use reovim_uapi_protocol::frame::read_frame;
/// use reovim_uapi_abi::ErrorCode;
///
/// // body_len declares 1000 but cap is 8 → protocol violation
/// let mut buf = [0u8; 16];
/// buf[0] = 0xE8; buf[1] = 0x03; // body_len = 1000
/// assert_eq!(read_frame(&buf, 8).unwrap_err(), ErrorCode::ProtocolViolation);
/// ```
///
/// # Errors
///
/// Fails with [`ErrorCode::ProtocolViolation`] under the conditions described above.
pub fn read_frame(buf: &[u8], cap: usize) -> Result<FrameView<'_>, ErrorCode> {
    if buf.len() < HEADER_LEN {
        return Err(ErrorCode::ProtocolViolation);
    }
    let mut h16 = [0u8; HEADER_LEN];
    h16.copy_from_slice(&buf[..HEADER_LEN]);
    let header = FrameHeader::decode(&h16);
    let body_len = header.body_len as usize;
    if body_len > cap {
        return Err(ErrorCode::ProtocolViolation);
    }
    // `body_len` was decoded from a `u32` (<= u32::MAX), so adding the
    // 16-byte header cannot wrap a usize on the only target (x86_64).
    let end = HEADER_LEN + body_len;
    if buf.len() < end {
        return Err(ErrorCode::ProtocolViolation);
    }
    Ok(FrameView {
        header,
        body: &buf[HEADER_LEN..end],
    })
}
