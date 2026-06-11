//! Projection notify push — encodes a `Projection` and sends it as an
//! `AttachEvent::Projection` (tag `0x0304`, notify, SP12, §7.4).
//!
//! After each successful `SendInput` → kernel dispatch the server pushes the
//! resulting `Projection` to the attached client with `correlation_id == 0`
//! (SP11, notify direction). The push model is used (§5.3 walking-skeleton
//! push default).

use {
    reovim_arch::{ds::Seq, net::UnixStream},
    reovim_subsys_domain::projection::Projection,
    reovim_uapi_abi::{ErrorCode, FrameHeader},
    reovim_uapi_protocol::{
        frame::HEADER_LEN,
        messages::{AttachEventProjection, Message},
    },
};

use crate::error::RuntimeError;

/// Maximum frame body size this runtime enforces (SP10, §10.4).
///
/// 4 MiB is a generous cap adequate for the walking skeleton's small text
/// buffers. The config-driven limit arrives with the config service.
///
/// See [`push_projection`] for usage.
///
/// ```rust
/// use reovim_server_rt::notify::WIRE_MAX_FRAME_BYTES;
///
/// assert_eq!(WIRE_MAX_FRAME_BYTES, 4 * 1024 * 1024);
/// ```
pub const WIRE_MAX_FRAME_BYTES: usize = 4 * 1024 * 1024;

/// Refuses a body that exceeds the SP10 frame cap.
///
/// Shared by every encode-and-send path (`push_projection`, the carrier's
/// `send_message`), so the cap arm is tested once for all of them.
///
/// # Errors
///
/// Returns `RuntimeError::Protocol(ResourceExhausted)` when `body_len`
/// exceeds [`WIRE_MAX_FRAME_BYTES`].
pub(crate) const fn ensure_frame_cap(body_len: usize) -> Result<(), RuntimeError> {
    if body_len > WIRE_MAX_FRAME_BYTES {
        return Err(RuntimeError::Protocol(ErrorCode::ResourceExhausted as i32));
    }
    Ok(())
}

/// Allocates a `Seq<u8>` of `n` zero bytes, returning `Err(Alloc)` on failure.
fn alloc_zeroed(n: usize) -> Result<Seq<u8>, RuntimeError> {
    let mut buf: Seq<u8> = Seq::new();
    for _ in 0..n {
        buf.try_push(0).map_err(|_| RuntimeError::Alloc)?;
    }
    Ok(buf)
}

/// Encodes `projection` and sends it as an `AttachEvent::Projection` notify
/// frame on `stream`.
///
/// The frame carries `correlation_id == 0` (SP11 — notifications are
/// uncorrelated). The header and body are written in two `write_all` calls to
/// avoid a combined allocation that would require a `write_frame` target buffer
/// wider than the available mutable slice path.
///
/// # Errors
///
/// Returns [`RuntimeError::Protocol`] when the encoded frame exceeds the SP10
/// cap, [`RuntimeError::Alloc`] on allocation failure, and
/// [`RuntimeError::Io`] on a write failure.
///
/// ```rust,no_run
/// // no_run: requires a live arch runtime + UDS connection.
/// ```
pub fn push_projection(stream: &UnixStream, projection: &Projection) -> Result<(), RuntimeError> {
    // Encode the Projection into the flat wire representation.
    let proj_bytes = projection.encode().map_err(|_| RuntimeError::Alloc)?;

    let msg = AttachEventProjection {
        buffer_id: u64::from(projection.buffer_id.as_u32()),
        projection: proj_bytes.as_slice(),
    };

    let body_len = msg.encoded_size();
    ensure_frame_cap(body_len)?;

    let body_len_u32 = u32::try_from(body_len)
        .map_err(|_| RuntimeError::Protocol(ErrorCode::ResourceExhausted as i32))?;

    // Build and write the 16-byte header (SP10 fixed prefix).
    let header = FrameHeader {
        body_len: body_len_u32,
        msg_type: AttachEventProjection::TAG,
        flags: 0,
        correlation_id: 0, // SP11: notify carries 0
    };
    let mut h16 = [0u8; HEADER_LEN];
    header.encode(&mut h16);
    stream
        .write_all(&h16)
        .map_err(|e| RuntimeError::Io(e.code()))?;

    // Encode and write the body.
    let mut body_buf = alloc_zeroed(body_len)?;
    msg.encode(body_buf.as_mut_slice())
        .map_err(|e| RuntimeError::Protocol(e as i32))?;
    stream
        .write_all(body_buf.as_slice())
        .map_err(|e| RuntimeError::Io(e.code()))
}

// L12 layout: tests in sibling notify_tests.rs, declared in lib.rs.
