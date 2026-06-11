//! UDS client carrier: connect, framed handshake, notify read loop.
//!
//! This module owns the byte-transport side of the client. The protocol
//! machine state is driven by the `ProtocolState` from `uapi/protocol` (L11:
//! the carrier lives here, not in `uapi/protocol`; that crate stays sans-IO
//! pure).
//!
//! ## Handshake sequence
//!
//! 1. `connect(path)` → `UnixStream`.
//! 2. Send `Hello{proto_major=1, proto_minor=0, caps=[], domain_codecs=[]}`.
//! 3. Read `HelloAck` — check `protocol_major == 1`; any other reply → error.
//! 4. Send `Attach{session_name="default", auth=[], caps=[], domain_codecs=[]}`.
//! 5. Read `AttachAck{client_id, ...}` — store `client_id`.
//! 6. Read initial `AttachEvent::Projection` notify (SP12).
//! 7. Enter the notify/input loop (`recv_notify_frame`).

use {
    reovim_arch::{
        ds::Seq,
        net::{Errno, UnixStream},
    },
    reovim_uapi_abi::FrameHeader,
    reovim_uapi_protocol::{
        frame::{HEADER_LEN, read_frame},
        messages::{Attach, AttachAck, AttachEventProjection, Hello, HelloAck, Message, SendInput},
        view::{RawInputList, StrList},
    },
};

/// Maximum frame body size the client accepts (SP10 cap).
const CLIENT_CAP: usize = 4 * 1024 * 1024;

/// Why a carrier operation was rejected.
///
/// ```rust
/// use reovim_platform_tui::carrier::CarrierError;
///
/// assert_ne!(CarrierError::Connect, CarrierError::Protocol);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CarrierError {
    /// The socket connect failed.
    Connect,
    /// A frame could not be sent.
    Send,
    /// A frame could not be received.
    Recv,
    /// The server replied with an unexpected message type.
    Protocol,
    /// A local allocation failed.
    Alloc,
}

/// Connected client state after a successful handshake.
///
/// ```no_run
/// // no_run: requires a live arch runtime + server.
/// ```
pub struct ClientConn {
    pub stream: UnixStream,
    pub client_id: u64,
}

// ── helpers ───────────────────────────────────────────────────────────────────

/// Allocates a zeroed `Seq<u8>` of `n` bytes.
fn alloc_zeroed(n: usize) -> Result<Seq<u8>, CarrierError> {
    let mut buf: Seq<u8> = Seq::new();
    for _ in 0..n {
        buf.try_push(0).map_err(|_| CarrierError::Alloc)?;
    }
    Ok(buf)
}

/// Encodes `msg` and writes it to `stream`.
pub(crate) fn send_msg<M: Message>(
    stream: &UnixStream,
    msg: &M,
    correlation_id: u64,
) -> Result<(), CarrierError> {
    let body_size = msg.encoded_size();
    let mut body_buf = alloc_zeroed(body_size)?;
    msg.encode(body_buf.as_mut_slice())
        .map_err(|_| CarrierError::Send)?;

    let header = FrameHeader {
        body_len: u32::try_from(body_size).map_err(|_| CarrierError::Send)?,
        msg_type: M::TAG,
        flags: 0,
        correlation_id,
    };
    let mut h16 = [0u8; HEADER_LEN];
    header.encode(&mut h16);
    stream.write_all(&h16).map_err(|_| CarrierError::Send)?;
    stream
        .write_all(body_buf.as_slice())
        .map_err(|_| CarrierError::Send)
}

/// Reads one framed message from `stream`. Returns the raw buffer (header ++ body).
fn recv_frame_buf(stream: &UnixStream) -> Result<Seq<u8>, CarrierError> {
    let mut h16 = [0u8; HEADER_LEN];
    stream
        .read_exact(&mut h16)
        .map_err(|_| CarrierError::Recv)?;
    let header = FrameHeader::decode(&h16);
    let body_len = header.body_len as usize;

    let total = HEADER_LEN + body_len;
    let mut buf = alloc_zeroed(total)?;
    buf.as_mut_slice()[..HEADER_LEN].copy_from_slice(&h16);
    if body_len > 0 {
        stream
            .read_exact(&mut buf.as_mut_slice()[HEADER_LEN..total])
            .map_err(|_| CarrierError::Recv)?;
    }
    Ok(buf)
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Empty pre-encoded cap/codec lists (count = 0, 4 bytes).
const EMPTY_LIST: [u8; 4] = [0x00, 0x00, 0x00, 0x00];

/// Connects to the server at `path` and completes the Hello→Attach handshake.
///
/// On success returns a [`ClientConn`] with the connected stream and assigned
/// `client_id`. The caller must then read the initial `AttachEvent::Projection`
/// notify via [`recv_notify_frame`].
///
/// # Errors
///
/// Returns [`CarrierError`] if the connect, send, or handshake protocol fails.
///
/// ```no_run
/// // no_run: requires a live arch runtime + server.
/// ```
pub fn connect_and_handshake(path: &[u8]) -> Result<ClientConn, CarrierError> {
    let stream = UnixStream::connect(path).map_err(|_| CarrierError::Connect)?;

    // ── Hello ─────────────────────────────────────────────────────────────────
    let hello = Hello {
        protocol_major: 1,
        protocol_minor: 0,
        caps: StrList::from_validated(&EMPTY_LIST),
        domain_codecs: StrList::from_validated(&EMPTY_LIST),
    };
    send_msg(&stream, &hello, 0)?;

    // ── HelloAck ──────────────────────────────────────────────────────────────
    let ack_buf = recv_frame_buf(&stream)?;
    let frame = read_frame(ack_buf.as_slice(), CLIENT_CAP).map_err(|_| CarrierError::Protocol)?;
    if frame.header.msg_type != HelloAck::TAG {
        return Err(CarrierError::Protocol);
    }
    let ack = HelloAck::decode(frame.body).map_err(|_| CarrierError::Protocol)?;
    if ack.protocol_major != 1 {
        return Err(CarrierError::Protocol);
    }

    // ── Attach ────────────────────────────────────────────────────────────────
    let attach = Attach {
        session_name: "default",
        auth: &[],
        caps: StrList::from_validated(&EMPTY_LIST),
        domain_codecs: StrList::from_validated(&EMPTY_LIST),
    };
    send_msg(&stream, &attach, 0)?;

    // ── AttachAck ─────────────────────────────────────────────────────────────
    let attach_buf = recv_frame_buf(&stream)?;
    let attach_frame =
        read_frame(attach_buf.as_slice(), CLIENT_CAP).map_err(|_| CarrierError::Protocol)?;
    if attach_frame.header.msg_type != AttachAck::TAG {
        return Err(CarrierError::Protocol);
    }
    let attach_ack = AttachAck::decode(attach_frame.body).map_err(|_| CarrierError::Protocol)?;

    Ok(ClientConn {
        stream,
        client_id: attach_ack.client_id,
    })
}

/// Reads one notify frame from the stream and extracts the projection bytes.
///
/// Blocks until a frame arrives. Returns `Ok(buf)` where `buf` is the raw
/// `projection` bytes from an `AttachEvent::Projection` body. Returns `Err` on
/// any I/O or protocol error.
///
/// ```no_run
/// // no_run: requires a live arch runtime + server.
/// ```
/// # Errors
///
/// Returns [`CarrierError`] when the read fails, the peer disconnects, or
/// the frame header is malformed/oversized.
pub fn recv_notify_frame(stream: &UnixStream) -> Result<Seq<u8>, CarrierError> {
    let buf = recv_frame_buf(stream)?;
    let frame = read_frame(buf.as_slice(), CLIENT_CAP).map_err(|_| CarrierError::Protocol)?;
    if frame.header.msg_type != AttachEventProjection::TAG {
        return Err(CarrierError::Protocol);
    }
    let event = AttachEventProjection::decode(frame.body).map_err(|_| CarrierError::Protocol)?;
    // Copy the projection bytes into an owned Seq<u8>.
    let mut proj = alloc_zeroed(event.projection.len())?;
    proj.as_mut_slice().copy_from_slice(event.projection);
    Ok(proj)
}

/// Sends one `SendInput` frame carrying `raw_input_bytes`.
///
/// `raw_input_bytes` must be a pre-encoded `RawInputList` byte sequence.
///
/// ```no_run
/// // no_run: requires a live arch runtime + server.
/// ```
/// # Errors
///
/// Returns [`CarrierError`] when encoding fails or the write does not
/// complete (peer gone, fd error).
pub fn send_input(
    stream: &UnixStream,
    client_id: u64,
    raw_input_bytes: &[u8],
) -> Result<(), CarrierError> {
    let send_input = SendInput {
        client_id,
        buffer_id: 1,
        window_id: 1,
        inputs: RawInputList::from_validated(raw_input_bytes),
    };
    send_msg(stream, &send_input, 0)
}

/// Returns `true` when `errno` signals a clean peer-closed disconnect
/// (EBADF is the arch convention on a zero-byte `read_exact`).
///
/// ```rust
/// use reovim_arch::net::EBADF;
/// use reovim_platform_tui::carrier::is_disconnect;
///
/// assert!(is_disconnect(EBADF));
/// ```
#[must_use]
pub fn is_disconnect(e: Errno) -> bool {
    e == reovim_arch::net::EBADF
}

// L12 layout: tests in sibling carrier_tests.rs, declared in lib.rs.
