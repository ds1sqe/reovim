//! Carrier loop: per-connection framed read → decode → dispatch → encode →
//! write (SP9.1, SP10, SP11, SP12, §7).
//!
//! [`run_connection`] is the single entry point for a connection thread. It
//! reads frames from the stream, runs them through the `ConnState` + protocol
//! machine, dispatches `Hello`/`Attach`/`SendInput`, writes responses, and
//! pushes `AttachEvent::Projection` notifies after each `SendInput` dispatch.
//!
//! The loop terminates on any I/O error, protocol violation, or client-side
//! disconnect (zero-byte read).

use {
    reovim_editor_core::EditorCore,
    reovim_lib_ds::{Seq, Shared},
    reovim_uapi::{
        abi::{ErrorCode, FrameHeader},
        net::UnixStream,
        protocol::{
            frame::{HEADER_LEN, read_frame},
            messages::{Attach, AttachAck, Direction, Hello, HelloAck, Message, Reject, SendInput},
            state::{Action, RejectReason, TagInfo},
            view::{DomainEntryList, StrList},
        },
    },
};

use crate::{
    conn::{ConnPhase, ConnState},
    error::RuntimeError,
    notify::{WIRE_MAX_FRAME_BYTES, push_projection},
};

// ── Protocol constants ────────────────────────────────────────────────────────

/// The protocol major version this runtime speaks.
const PROTO_MAJOR: u16 = 1;
/// The protocol minor version this runtime speaks.
const PROTO_MINOR: u16 = 0;

/// Pre-encoded empty `StrList` body (count = 0, 4 bytes).
const EMPTY_STR_LIST: [u8; 4] = [0x00, 0x00, 0x00, 0x00];
/// Pre-encoded empty `DomainEntryList` body (count = 0, 4 bytes).
const EMPTY_DOMAIN_LIST: [u8; 4] = [0x00, 0x00, 0x00, 0x00];

// ── run_connection ────────────────────────────────────────────────────────────

/// Runs the framed carrier loop on `stream`, dispatching each frame through
/// `editor_core`, until the connection closes or an unrecoverable error occurs.
///
/// This is the per-connection thread body called by the listener.
///
/// ```rust,no_run
/// // no_run: requires a live arch runtime + UDS connection.
/// ```
pub fn run_connection(stream: &UnixStream, editor_core: &Shared<EditorCore>) {
    let mut state = ConnState::new();
    loop {
        match recv_frame(stream) {
            Ok(buf) => {
                if dispatch_frame(stream, &mut state, editor_core, &buf).is_err() {
                    return; // protocol error or I/O failure → close
                }
            }
            Err(_) => return, // I/O error or client disconnect → close
        }
    }
}

// ── recv_frame ────────────────────────────────────────────────────────────────

/// Reads exactly one frame from `stream`: 16-byte header then `body_len`
/// body bytes. Returns the raw frame buffer (header ++ body) on success.
///
/// Returns `Err` on any I/O error or SP10 cap violation.
fn recv_frame(stream: &UnixStream) -> Result<Seq<u8>, RuntimeError> {
    // Step 1: read the 16-byte header prefix.
    let mut h16 = [0u8; HEADER_LEN];
    stream
        .read_exact(&mut h16)
        .map_err(|e| RuntimeError::Io(e.code()))?;

    // Step 2: parse body_len from the header.
    let header = FrameHeader::decode(&h16);
    let body_len = header.body_len as usize;
    if body_len > WIRE_MAX_FRAME_BYTES {
        return Err(RuntimeError::Protocol(ErrorCode::ResourceExhausted as i32));
    }

    // Step 3: allocate a buffer for the full frame (header + body).
    let total = HEADER_LEN + body_len;
    let mut buf = alloc_zeroed(total)?;

    // Copy the header we already read into the front.
    buf.as_mut_slice()[..HEADER_LEN].copy_from_slice(&h16);

    // Step 4: read the body directly into the buffer after the header.
    if body_len > 0 {
        stream
            .read_exact(&mut buf.as_mut_slice()[HEADER_LEN..total])
            .map_err(|e| RuntimeError::Io(e.code()))?;
    }

    Ok(buf)
}

// ── dispatch_frame ────────────────────────────────────────────────────────────

/// Parses and dispatches one received frame buffer. Writes any response to
/// `stream`. Returns `Err` if the connection should close.
fn dispatch_frame(
    stream: &UnixStream,
    state: &mut ConnState,
    editor_core: &Shared<EditorCore>,
    buf: &Seq<u8>,
) -> Result<(), RuntimeError> {
    let frame = read_frame(buf.as_slice(), WIRE_MAX_FRAME_BYTES)
        .map_err(|e| RuntimeError::Protocol(e as i32))?;

    // Classify the tag so ProtocolState can enforce SP9.1 / SP11.
    let tag_info = classify_tag(frame.header.msg_type);

    // Feed the header + tag to the protocol state machine.
    match state.protocol.on_frame(frame.header, tag_info) {
        Action::Accept => {}
        Action::SkipNotify => {
            // Unknown notify from client — forward-compat skip, stay open.
            return Ok(());
        }
        Action::Reject(reason) => {
            // Terminal protocol violation: send Reject and close.
            let _ = send_reject(stream, reject_reason_to_error(reason), "protocol violation");
            return Err(RuntimeError::Protocol(ErrorCode::ProtocolViolation as i32));
        }
    }

    // Advance application-level state if the protocol handshake just
    // completed (phase transitions from AwaitingHandshake to Established on
    // Hello acceptance).
    if frame.header.msg_type == Hello::TAG && state.phase() == ConnPhase::Handshake {
        handle_hello(stream, state, frame.body)?;
        return Ok(());
    }

    match frame.header.msg_type {
        Attach::TAG => handle_attach(stream, state, editor_core, frame.body),
        SendInput::TAG => handle_send_input(stream, state, editor_core, frame.body),
        _ => {
            // Unknown request accepted by the state machine → ignore
            // (server-role unknown requests are rejected by the state
            // machine before reaching here; this branch is unreachable in
            // practice but must be exhaustive).
            Ok(())
        }
    }
}

// ── handle_hello ─────────────────────────────────────────────────────────────

/// Processes a `Hello` frame: decodes it, validates the protocol version,
/// replies with `HelloAck`, and advances `state` to `Ready`.
fn handle_hello(
    stream: &UnixStream,
    state: &mut ConnState,
    body: &[u8],
) -> Result<(), RuntimeError> {
    let hello = Hello::decode(body).map_err(|e| RuntimeError::Protocol(e as i32))?;

    // Version check: we speak major=1 only.
    if hello.protocol_major != PROTO_MAJOR {
        let _ = send_reject(stream, ErrorCode::IncompatibleApi, "incompatible protocol version");
        return Err(RuntimeError::Protocol(ErrorCode::IncompatibleApi as i32));
    }

    // Reply with HelloAck.
    let granted_caps = StrList::from_validated(&EMPTY_STR_LIST);
    let ack = HelloAck {
        protocol_major: PROTO_MAJOR,
        protocol_minor: PROTO_MINOR,
        granted_caps,
        server_name: "reovim",
    };
    send_message(stream, &ack, 0)?;

    state.set_ready();
    Ok(())
}

// ── handle_attach ────────────────────────────────────────────────────────────

/// Processes an `Attach` frame: decodes it, enforces SP1 (one attach per
/// connection), replies with `AttachAck`, and pushes the initial `Projection`.
fn handle_attach(
    stream: &UnixStream,
    state: &mut ConnState,
    editor_core: &Shared<EditorCore>,
    body: &[u8],
) -> Result<(), RuntimeError> {
    let _attach = Attach::decode(body).map_err(|e| RuntimeError::Protocol(e as i32))?;

    // SP1: second Attach on one connection is a Conflict.
    if !state.set_attached() {
        let _ = send_reject(stream, ErrorCode::Conflict, "already attached (SP1)");
        return Err(RuntimeError::AlreadyAttached);
    }

    // Reply with AttachAck — client_id=1, empty domain table (skeleton).
    let domain_table = DomainEntryList::from_validated(&EMPTY_DOMAIN_LIST);
    let ack = AttachAck {
        client_id: 1,
        domain_table,
    };
    // AttachAck is a Response; echo the request's correlation_id.
    // SendInput uses correlation_id == 0 (uncorrelated hot path); Attach
    // uses the same uncorrelated path in the walking skeleton.
    send_message(stream, &ack, 0)?;

    // Push the initial Projection (SP12: after AttachAck the notify stream
    // is active). The initial state is whatever the session currently holds.
    let projection = editor_core
        .dispatch_input(&[])
        .map_err(|_| RuntimeError::Dispatch)?;
    push_projection(stream, &projection)
}

// ── handle_send_input ────────────────────────────────────────────────────────

/// Processes a `SendInput` frame: decodes it, enforces the attached guard,
/// dispatches through the editor core, and pushes the resulting `Projection`.
fn handle_send_input(
    stream: &UnixStream,
    state: &ConnState,
    editor_core: &Shared<EditorCore>,
    body: &[u8],
) -> Result<(), RuntimeError> {
    let msg = SendInput::decode(body).map_err(|e| RuntimeError::Protocol(e as i32))?;

    // Guard: SendInput before Attach is a protocol violation.
    if !state.is_attached() {
        let _ = send_reject(stream, ErrorCode::ProtocolViolation, "SendInput before Attach");
        return Err(RuntimeError::Protocol(ErrorCode::ProtocolViolation as i32));
    }

    // Flatten the RawInput batch into a single byte slice for dispatch.
    // The walking skeleton passes the first input's payload; an empty
    // batch dispatches an empty slice.
    let payload: &[u8] = msg.inputs.iter().next().map_or(&[], |item| item.payload);

    let projection = editor_core
        .dispatch_input(payload)
        .map_err(|_| RuntimeError::Dispatch)?;

    // SP12: push the resulting Projection as an AttachEvent::Projection notify.
    push_projection(stream, &projection)
}

// ── send_message ─────────────────────────────────────────────────────────────

/// Encodes `msg` into a heap buffer and writes it as a complete framed message.
fn send_message<M: Message>(
    stream: &UnixStream,
    msg: &M,
    correlation_id: u64,
) -> Result<(), RuntimeError> {
    let body_len = msg.encoded_size();
    crate::notify::ensure_frame_cap(body_len)?;
    let body_len_u32 = u32::try_from(body_len)
        .map_err(|_| RuntimeError::Protocol(ErrorCode::ResourceExhausted as i32))?;

    // Write the 16-byte header.
    let header = FrameHeader {
        body_len: body_len_u32,
        msg_type: M::TAG,
        flags: 0,
        correlation_id,
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

// ── send_reject ───────────────────────────────────────────────────────────────

/// Sends a `Reject` frame. Errors are discarded (terminal path — the
/// connection is closing regardless).
fn send_reject(stream: &UnixStream, code: ErrorCode, detail: &str) -> Result<(), RuntimeError> {
    let msg = Reject { code, detail };
    send_message(stream, &msg, 0)
}

// ── classify_tag ─────────────────────────────────────────────────────────────

/// Maps a `msg_type` to the `TagInfo` required by `ProtocolState::on_frame`.
///
/// Tag ranges follow §7 of the spec:
/// - `0x0001..=0x00FF` — Handshake/lifecycle
/// - `0x0100..=0x01FF` — Request (client → server)
/// - `0x0200..=0x02FF` — Response (server → client)
/// - `0x0300..=0x03FF` — Notify (server → client)
/// - `0xFF00..=0xFFFF` — Error/Reject
#[must_use]
pub(crate) const fn classify_tag(tag: u16) -> TagInfo {
    match tag {
        0x0001..=0x00FF => TagInfo::Known(Direction::Handshake),
        0x0100..=0x01FF => TagInfo::Known(Direction::Request),
        0x0200..=0x02FF => TagInfo::Known(Direction::Response),
        0x0300..=0x03FF => TagInfo::Known(Direction::Notify),
        0xFF00..=0xFFFF => TagInfo::Known(Direction::Error),
        _ => TagInfo::Unknown,
    }
}

// ── reject_reason_to_error ────────────────────────────────────────────────────

/// Maps a [`RejectReason`] to the `ErrorCode` sent in the `Reject` frame.
pub(crate) const fn reject_reason_to_error(reason: RejectReason) -> ErrorCode {
    match reason {
        RejectReason::HandshakeExpected
        | RejectReason::UnexpectedHandshake
        | RejectReason::UnknownRequest
        | RejectReason::CorrelationViolation
        | RejectReason::BadFlags => ErrorCode::ProtocolViolation,
    }
}

// ── alloc_zeroed ─────────────────────────────────────────────────────────────

/// Allocates a `Seq<u8>` of `n` zero bytes; `Err(Alloc)` on failure.
fn alloc_zeroed(n: usize) -> Result<Seq<u8>, RuntimeError> {
    let mut buf: Seq<u8> = Seq::new();
    for _ in 0..n {
        buf.try_push(0).map_err(|_| RuntimeError::Alloc)?;
    }
    Ok(buf)
}

// L12 layout: tests in sibling carrier_tests.rs, declared in lib.rs.
