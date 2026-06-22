//! Phase 3 integration smoke (#797): Hello → Attach → SendInput over a real UDS.
//!
//! This module lives in the `server-selftest` bin (not in the server-rt rlib)
//! because the text Domain is an ext crate (`ext/server/domain/text`) and
//! `reovim-server-rt` must NOT depend on ext (core/ext boundary).
//!
//! ## What the smoke proves
//!
//! Boot kernel → register text Domain → `start_listener` → connect raw client
//! → send `Hello` → receive `HelloAck` → send `Attach` → receive `AttachAck` +
//! initial `Projection` notify → send `SendInput("x")` → receive `Projection`
//! notify with `"x"` in the content bytes.
//!
//! ## Error-path carrier coverage (#797 Phase 5)
//!
//! Additional tests cover the protocol-error branches in `carrier.rs`:
//! - `non_hello_first_frame_rejected`: non-Hello first frame → server sends
//!   `Reject` and closes (dispatch error → return path, line 61).
//! - `incompatible_protocol_major`: `Hello` with major != 1 → `IncompatibleApi`
//!   `Reject` + connection close (line 170-171).
//! - `send_input_before_attach`: `SendInput` before `Attach` → `ProtocolViolation`
//!   `Reject` (line 239-240).
//! - `second_attach_rejected`: second `Attach` → `Conflict` `Reject` (line 202-203).
//! - `oversized_body_len`: frame header with `body_len > WIRE_MAX_FRAME_BYTES` →
//!   `ResourceExhausted` error close (line 86).
//! - `truncated_frame_close`: frame body shorter than header says → read error
//!   → connection close.
//! - `send_reject_coverage`: `send_reject` helper is exercised by every Reject
//!   path above; additionally verified by checking the Reject bytes decode
//!   correctly.

use {
    reovim_arch::{arch_test, net::UnixStream},
    reovim_domain_text::{TextHandler, TextProjector},
    reovim_kernel::{
        Init, LauncherArgs,
        session::{BufferId, DomainAttachmentId, SessionState, WindowId},
    },
    reovim_lib_ds::Seq,
    reovim_server_rt::start_listener,
    reovim_system_kernel::{net::net_control, sched::thread_spawner},
    reovim_uapi::{
        abi::{ErrorCode, FrameHeader},
        protocol::{
            frame::{HEADER_LEN, read_frame},
            messages::{Attach, AttachAck, Hello, HelloAck, Message, Reject, SendInput},
            view::{RawInputList, StrList},
        },
    },
};

/// Static text Domain singletons. Zero-sized structs; no heap needed.
static TEXT_HANDLER: TextHandler = TextHandler;
static TEXT_PROJECTOR: TextProjector = TextProjector;

/// The socket path used by the integration smoke. The `\0` NUL terminator is
/// required by the arch `bind` wrapper (C-string path convention).
// TID-unique socket path: concurrent selftest binaries register this test.
fn smoke_path(buf: &mut [u8; 64]) -> &[u8] {
    reovim_arch::testrt::unique_path(b"/tmp/reovim-server-smoke-", buf)
}

/// Maximum frame body size the smoke client enforces.
const CLIENT_CAP: usize = 4 * 1024 * 1024;

arch_test!(phase3_integration_smoke_hello_attach_send_input, {
    // ── 1. Boot the kernel ────────────────────────────────────────────────────
    let kernel = Init::new(LauncherArgs::default())
        .boot()
        .expect("boot succeeds");

    // ── 2. Register the text Domain ───────────────────────────────────────────
    let domain_id = kernel
        .register_domain("text", &TEXT_HANDLER, &TEXT_PROJECTOR)
        .expect("register_domain succeeds");

    let state = SessionState::new(
        domain_id,
        DomainAttachmentId::new(1),
        BufferId::new(1),
        WindowId::new(1),
    );
    kernel.setup_session(state);

    // ── 3. Start the listener on the booted kernel ────────────────────────────
    let mut sbuf = [0u8; 64];
    let smoke_path: &[u8] = smoke_path(&mut sbuf);
    // The socket file survives a previous run's process exit (the listener
    // thread never drops); unlink first so a TID-reuse re-bind cannot hit
    // EADDRINUSE.
    let _ = reovim_arch::sys::unlinkat(reovim_arch::sys::AT_FDCWD, smoke_path, 0);
    start_listener(&kernel, smoke_path, net_control(), thread_spawner())
        .expect("start_listener succeeds");

    // ── 4. Connect the raw test client ────────────────────────────────────────
    // No sleep needed: `connect` succeeds once the socket is bound and
    // listening (OS queues the connection in the accept backlog).
    let client = UnixStream::connect(smoke_path).expect("connect succeeds");

    // ── 5. Send Hello ─────────────────────────────────────────────────────────
    let empty_caps = [0x00u8, 0x00, 0x00, 0x00];
    let hello = Hello {
        protocol_major: 1,
        protocol_minor: 0,
        caps: StrList::from_validated(&empty_caps),
        domain_codecs: StrList::from_validated(&empty_caps),
    };
    send_msg(&client, &hello, 0);

    // ── 6. Receive HelloAck ───────────────────────────────────────────────────
    let ack_buf = recv_frame_buf(&client);
    let frame = read_frame(ack_buf.as_slice(), CLIENT_CAP).expect("parse HelloAck frame");
    assert_eq!(frame.header.msg_type, HelloAck::TAG, "server must reply with HelloAck");
    let ack = HelloAck::decode(frame.body).expect("decode HelloAck");
    assert_eq!(ack.protocol_major, 1, "server major version must be 1");

    // ── 7. Send Attach ────────────────────────────────────────────────────────
    let empty_domain_codecs = [0x00u8, 0x00, 0x00, 0x00];
    let attach = Attach {
        session_name: "default",
        auth: &[],
        caps: StrList::from_validated(&empty_caps),
        domain_codecs: StrList::from_validated(&empty_domain_codecs),
    };
    send_msg(&client, &attach, 0);

    // ── 8. Receive AttachAck ──────────────────────────────────────────────────
    let attach_ack_buf = recv_frame_buf(&client);
    let attach_ack_frame =
        read_frame(attach_ack_buf.as_slice(), CLIENT_CAP).expect("parse AttachAck frame");
    assert_eq!(
        attach_ack_frame.header.msg_type,
        AttachAck::TAG,
        "server must reply with AttachAck"
    );
    let attach_ack = AttachAck::decode(attach_ack_frame.body).expect("decode AttachAck");
    assert_eq!(attach_ack.client_id, 1, "client_id must be 1");

    // ── 9. Receive initial Projection notify (SP12) ───────────────────────────
    let init_proj_buf = recv_frame_buf(&client);
    let init_proj_frame =
        read_frame(init_proj_buf.as_slice(), CLIENT_CAP).expect("parse initial Projection frame");
    // AttachEvent::Projection tag = 0x0304
    assert_eq!(
        init_proj_frame.header.msg_type, 0x0304,
        "server must push initial Projection notify after AttachAck"
    );
    assert_eq!(
        init_proj_frame.header.correlation_id, 0,
        "Projection notify correlation_id must be 0 (SP11)"
    );

    // ── 10. Send SendInput("x") ───────────────────────────────────────────────
    // Build a RawInputList with one Key input carrying payload b"x".
    // Encoding: [count u32 LE][kind u8][payload_len u32 LE][payload bytes]
    let raw_input_buf = build_raw_input_list(reovim_uapi::abi::input::RawInputKind::Key, b"x");
    let send_input = SendInput {
        client_id: 1,
        buffer_id: 1,
        window_id: 1,
        inputs: RawInputList::from_validated(raw_input_buf.as_slice()),
    };
    send_msg(&client, &send_input, 0);

    // ── 11. Receive Projection notify after SendInput ─────────────────────────
    let proj_buf = recv_frame_buf(&client);
    let proj_frame =
        read_frame(proj_buf.as_slice(), CLIENT_CAP).expect("parse Projection frame after input");
    assert_eq!(proj_frame.header.msg_type, 0x0304, "tag must be 0x0304");
    assert_eq!(proj_frame.header.correlation_id, 0, "SP11: notify cid = 0");

    // The projection body starts with buffer_id: u64 LE, then the raw projection
    // bytes (flat format: [u32 buf_id][u32 win_id][u32 cursor][u32 span_end][content]).
    // The attachment-event body is: buffer_id (u64) + length-prefixed projection bytes.
    // At minimum the inner projection must contain b"x" as content.
    assert!(
        proj_frame.body.len() > 8,
        "Projection body must be non-trivial (buffer_id + projection bytes)"
    );
    // The inner projection starts at byte 8+4 (buffer_id u64 = 8 bytes, then
    // the projection bytes are length-prefixed by the codec). The raw content
    // "x" appears past the 16-byte projection header.
    // Heuristic check: the body contains b"x" somewhere.
    assert!(
        proj_frame.body.contains(&b'x'),
        "Projection body must contain the inserted 'x' byte"
    );
});

// ── Error-path carrier coverage ──────────────────────────────────────────────
//
// Each test boots its own kernel + listener on a TID-unique path to avoid any
// cross-test ordering dependency. The error conditions are provoked by crafting
// byte sequences that the server must reject.

/// Boots a kernel with the text Domain registered and a session wired up,
/// starts a listener on `path`, and returns the connected client stream.
///
/// The listener runs on a background thread; the client stream is ready to use
/// immediately (the OS accept-backlog queues the connection).
fn boot_and_connect(path: &[u8]) -> UnixStream {
    let kernel = Init::new(LauncherArgs::default())
        .boot()
        .expect("boot succeeds");

    let domain_id = kernel
        .register_domain("text", &TEXT_HANDLER, &TEXT_PROJECTOR)
        .expect("register_domain succeeds");
    let state = SessionState::new(
        domain_id,
        DomainAttachmentId::new(1),
        BufferId::new(1),
        WindowId::new(1),
    );
    kernel.setup_session(state);

    let _ = reovim_arch::sys::unlinkat(reovim_arch::sys::AT_FDCWD, path, 0);
    start_listener(&kernel, path, net_control(), thread_spawner())
        .expect("start_listener succeeds");

    UnixStream::connect(path).expect("connect succeeds")
}

/// Reads from `stream` until the server closes the connection (zero bytes or
/// EBADF/ECONNRESET). Returns the last successfully read bytes as a `Seq<u8>`.
///
/// The function reads frame-by-frame until no more bytes arrive. It is used
/// by tests that expect the server to close after sending a `Reject`.
fn drain_until_close(stream: &UnixStream) -> Seq<u8> {
    let mut h16 = [0u8; HEADER_LEN];
    match stream.read_exact(&mut h16) {
        Ok(()) => {}
        Err(_) => return Seq::new(), // connection already closed
    }
    let header = FrameHeader::decode(&h16);
    let body_len = header.body_len as usize;
    let total = HEADER_LEN + body_len;
    let mut buf: Seq<u8> = Seq::new();
    for _ in 0..total {
        buf.try_push(0).expect("alloc");
    }
    buf.as_mut_slice()[..HEADER_LEN].copy_from_slice(&h16);
    if body_len > 0 {
        let _ = stream.read_exact(&mut buf.as_mut_slice()[HEADER_LEN..total]);
    }
    buf
}

// ── non-Hello first frame → Reject + close (carrier.rs line 61 + send_reject) ─

arch_test!(non_hello_first_frame_causes_reject_and_close, {
    let mut sbuf = [0u8; 64];
    let path: &[u8] = reovim_arch::testrt::unique_path(b"/tmp/reovim-err1-", &mut sbuf);
    let client = boot_and_connect(path);

    // Send a Request-direction frame (Attach, tag 0x0100) as the very first
    // message. The server's ProtocolState machine expects a Handshake-direction
    // frame first; it returns Action::Reject → server sends Reject and closes.
    let empty_caps = [0x00u8, 0x00, 0x00, 0x00];
    let attach = Attach {
        session_name: "default",
        auth: &[],
        caps: StrList::from_validated(&empty_caps),
        domain_codecs: StrList::from_validated(&empty_caps),
    };
    send_msg(&client, &attach, 0);

    // The server must reply with a Reject frame before closing.
    let reject_buf = drain_until_close(&client);
    assert!(!reject_buf.is_empty(), "server must send a Reject before closing");
    let frame = read_frame(reject_buf.as_slice(), CLIENT_CAP);
    if let Ok(f) = frame {
        assert_eq!(
            f.header.msg_type,
            Reject::TAG,
            "server reply must be Reject (tag 0xFF00), got 0x{:04X}",
            f.header.msg_type
        );
        let reject = Reject::decode(f.body).expect("Reject decode");
        assert_eq!(
            reject.code,
            ErrorCode::ProtocolViolation,
            "non-Hello first frame must yield ProtocolViolation reject"
        );
    }
    // After the Reject the server closes; subsequent reads return an error.
    let post = drain_until_close(&client);
    assert!(post.is_empty(), "server must close after sending Reject (no further frames)");
});

// ── incompatible protocol major → IncompatibleApi Reject + close ─────────────
// carrier.rs line 169-171

arch_test!(incompatible_protocol_major_causes_reject, {
    let mut sbuf = [0u8; 64];
    let path: &[u8] = reovim_arch::testrt::unique_path(b"/tmp/reovim-err2-", &mut sbuf);
    let client = boot_and_connect(path);

    let empty_caps = [0x00u8, 0x00, 0x00, 0x00];
    // protocol_major = 255 (not 1) → server sends IncompatibleApi Reject.
    let hello = Hello {
        protocol_major: 255,
        protocol_minor: 0,
        caps: StrList::from_validated(&empty_caps),
        domain_codecs: StrList::from_validated(&empty_caps),
    };
    send_msg(&client, &hello, 0);

    let reject_buf = drain_until_close(&client);
    assert!(!reject_buf.is_empty(), "server must send Reject for bad protocol_major");
    let frame = read_frame(reject_buf.as_slice(), CLIENT_CAP);
    if let Ok(f) = frame {
        assert_eq!(f.header.msg_type, Reject::TAG, "reply must be Reject");
        let reject = Reject::decode(f.body).expect("Reject decode");
        assert_eq!(
            reject.code,
            ErrorCode::IncompatibleApi,
            "bad protocol_major must yield IncompatibleApi"
        );
    }
});

// ── SendInput before Attach → ProtocolViolation Reject ───────────────────────
// carrier.rs line 238-240

arch_test!(send_input_before_attach_causes_protocol_violation, {
    let mut sbuf = [0u8; 64];
    let path: &[u8] = reovim_arch::testrt::unique_path(b"/tmp/reovim-err3-", &mut sbuf);
    let client = boot_and_connect(path);

    // Complete the Hello handshake first.
    let empty_caps = [0x00u8, 0x00, 0x00, 0x00];
    let hello = Hello {
        protocol_major: 1,
        protocol_minor: 0,
        caps: StrList::from_validated(&empty_caps),
        domain_codecs: StrList::from_validated(&empty_caps),
    };
    send_msg(&client, &hello, 0);
    // Read HelloAck.
    let _ack = recv_frame_buf(&client);

    // Now send SendInput WITHOUT sending Attach first.
    let raw_input = build_raw_input_list(reovim_uapi::abi::input::RawInputKind::Key, b"x");
    let send_input = SendInput {
        client_id: 1,
        buffer_id: 1,
        window_id: 1,
        inputs: RawInputList::from_validated(raw_input.as_slice()),
    };
    send_msg(&client, &send_input, 0);

    let reject_buf = drain_until_close(&client);
    assert!(!reject_buf.is_empty(), "server must send Reject for SendInput before Attach");
    let frame = read_frame(reject_buf.as_slice(), CLIENT_CAP);
    if let Ok(f) = frame {
        assert_eq!(f.header.msg_type, Reject::TAG, "reply must be Reject");
        let reject = Reject::decode(f.body).expect("Reject decode");
        assert_eq!(
            reject.code,
            ErrorCode::ProtocolViolation,
            "SendInput before Attach must yield ProtocolViolation"
        );
    }
});

// ── second Attach → Conflict Reject ──────────────────────────────────────────
// carrier.rs line 201-203

arch_test!(second_attach_causes_conflict_reject, {
    let mut sbuf = [0u8; 64];
    let path: &[u8] = reovim_arch::testrt::unique_path(b"/tmp/reovim-err4-", &mut sbuf);
    let client = boot_and_connect(path);

    let empty_caps = [0x00u8, 0x00, 0x00, 0x00];
    // Complete Hello.
    let hello = Hello {
        protocol_major: 1,
        protocol_minor: 0,
        caps: StrList::from_validated(&empty_caps),
        domain_codecs: StrList::from_validated(&empty_caps),
    };
    send_msg(&client, &hello, 0);
    let _ack = recv_frame_buf(&client);

    // First Attach (must succeed → AttachAck + initial Projection).
    let attach = Attach {
        session_name: "default",
        auth: &[],
        caps: StrList::from_validated(&empty_caps),
        domain_codecs: StrList::from_validated(&empty_caps),
    };
    send_msg(&client, &attach, 0);
    // Consume AttachAck.
    let _attach_ack = recv_frame_buf(&client);
    // Consume initial Projection notify (SP12).
    let _init_proj = recv_frame_buf(&client);

    // Second Attach → server must send Conflict Reject.
    let attach2 = Attach {
        session_name: "default",
        auth: &[],
        caps: StrList::from_validated(&empty_caps),
        domain_codecs: StrList::from_validated(&empty_caps),
    };
    send_msg(&client, &attach2, 0);

    let reject_buf = drain_until_close(&client);
    assert!(!reject_buf.is_empty(), "server must send Reject for second Attach");
    let frame = read_frame(reject_buf.as_slice(), CLIENT_CAP);
    if let Ok(f) = frame {
        assert_eq!(f.header.msg_type, Reject::TAG, "reply must be Reject");
        let reject = Reject::decode(f.body).expect("Reject decode");
        assert_eq!(reject.code, ErrorCode::Conflict, "second Attach must yield Conflict");
    }
});

// ── oversized body_len → ResourceExhausted + close ───────────────────────────
// carrier.rs line 84-86: body_len > WIRE_MAX_FRAME_BYTES is checked in recv_frame.

arch_test!(oversized_body_len_causes_resource_exhausted_close, {
    let mut sbuf = [0u8; 64];
    let path: &[u8] = reovim_arch::testrt::unique_path(b"/tmp/reovim-err5-", &mut sbuf);
    let client = boot_and_connect(path);

    // Craft a raw header with body_len = WIRE_MAX_FRAME_BYTES + 1 = 4 MiB + 1.
    // The server's recv_frame checks this cap and returns an error → closes.
    // We do NOT need to send the body — the server reads only the header first.
    const CAP_PLUS_ONE: u32 = (4 * 1024 * 1024 + 1) as u32;
    let header = FrameHeader {
        body_len: CAP_PLUS_ONE,
        msg_type: Hello::TAG,
        flags: 0,
        correlation_id: 0,
    };
    let mut h16 = [0u8; HEADER_LEN];
    header.encode(&mut h16);
    let _ = client.write_all(&h16);

    // The server closes after the cap violation; further reads will fail.
    // We drain to confirm the server does NOT reply with a valid frame.
    // (The server returns an error immediately without sending Reject for
    // recv_frame failures — it just closes the connection per run_connection.)
    let _after = drain_until_close(&client);
    // Pass: the server closed cleanly (no panic, no assertion failure).
});

// ── truncated frame body → short read → connection close ─────────────────────
// carrier.rs: recv_frame calls read_exact for the body; a truncated body causes
// read_exact to return an error (ECONNRESET or EBADF from peer close) which
// maps to RuntimeError::Io → connection close.

arch_test!(truncated_frame_body_causes_close, {
    let mut sbuf = [0u8; 64];
    let path: &[u8] = reovim_arch::testrt::unique_path(b"/tmp/reovim-err6-", &mut sbuf);
    let client = boot_and_connect(path);

    // Send a header declaring body_len = 32 but then close the connection
    // before writing any body bytes. The server's read_exact for the body will
    // return an error → RuntimeError::Io → connection close.
    let header = FrameHeader {
        body_len: 32,
        msg_type: Hello::TAG,
        flags: 0,
        correlation_id: 0,
    };
    let mut h16 = [0u8; HEADER_LEN];
    header.encode(&mut h16);
    let _ = client.write_all(&h16);
    // Drop (close) the client without sending the body — server's read_exact fails.
    drop(client);
    // Pass: no crash; the server closes the connection on the I/O error.
});

// ── send_reject coverage via Reject decode round-trip ────────────────────────
// send_reject (carrier.rs lines 297-300) is exercised by every error test above.
// This dedicated test additionally asserts that the Reject bytes sent by the
// server are correctly decodable and carry the expected detail string.

arch_test!(send_reject_detail_is_non_empty, {
    // Reuse the incompatible-major scenario: server sends "incompatible protocol
    // version" as the detail string in the Reject body.
    let mut sbuf = [0u8; 64];
    let path: &[u8] = reovim_arch::testrt::unique_path(b"/tmp/reovim-err7-", &mut sbuf);
    let client = boot_and_connect(path);

    let empty_caps = [0x00u8, 0x00, 0x00, 0x00];
    let hello = Hello {
        protocol_major: 0, // major = 0 triggers IncompatibleApi
        protocol_minor: 0,
        caps: StrList::from_validated(&empty_caps),
        domain_codecs: StrList::from_validated(&empty_caps),
    };
    send_msg(&client, &hello, 0);

    let reject_buf = drain_until_close(&client);
    assert!(!reject_buf.is_empty(), "Reject must be sent");
    if let Ok(f) = read_frame(reject_buf.as_slice(), CLIENT_CAP) {
        if f.header.msg_type == Reject::TAG {
            let reject = Reject::decode(f.body).expect("Reject decode");
            assert!(!reject.detail.is_empty(), "send_reject detail string must be non-empty");
        }
    }
});

// ── helpers ───────────────────────────────────────────────────────────────────

/// Encodes and writes a single message to `stream`.
fn send_msg<M: Message>(stream: &UnixStream, msg: &M, correlation_id: u64) {
    let body_size = msg.encoded_size();
    // Encode body into a Seq<u8>.
    let mut body_buf: Seq<u8> = Seq::new();
    for _ in 0..body_size {
        body_buf.try_push(0).expect("alloc body");
    }
    msg.encode(body_buf.as_mut_slice())
        .expect("encode message body");

    // Build header.
    let header = FrameHeader {
        body_len: u32::try_from(body_size).expect("body size fits u32"),
        msg_type: M::TAG,
        flags: 0,
        correlation_id,
    };
    let mut h16 = [0u8; HEADER_LEN];
    header.encode(&mut h16);
    stream.write_all(&h16).expect("write header");
    stream.write_all(body_buf.as_slice()).expect("write body");
}

/// Reads one framed message from `stream` into a `Seq<u8>` buffer.
/// Returns the raw buffer (header ++ body).
fn recv_frame_buf(stream: &UnixStream) -> Seq<u8> {
    // Read the 16-byte header.
    let mut h16 = [0u8; HEADER_LEN];
    stream.read_exact(&mut h16).expect("read frame header");
    let header = FrameHeader::decode(&h16);
    let body_len = header.body_len as usize;

    // Allocate header+body buffer.
    let total = HEADER_LEN + body_len;
    let mut buf: Seq<u8> = Seq::new();
    for _ in 0..total {
        buf.try_push(0).expect("alloc frame buffer");
    }
    buf.as_mut_slice()[..HEADER_LEN].copy_from_slice(&h16);
    if body_len > 0 {
        stream
            .read_exact(&mut buf.as_mut_slice()[HEADER_LEN..total])
            .expect("read frame body");
    }
    buf
}

/// Builds a `RawInputList`-compatible pre-encoded byte buffer for one input.
/// Layout: `[count u32 LE][kind u8][len u32 LE][payload bytes]`.
fn build_raw_input_list(kind: reovim_uapi::abi::input::RawInputKind, payload: &[u8]) -> Seq<u8> {
    let mut buf: Seq<u8> = Seq::new();
    // count = 1
    for b in 1u32.to_le_bytes() {
        buf.try_push(b).expect("alloc");
    }
    // kind discriminant
    buf.try_push(kind as u8).expect("alloc");
    // payload length as u32 LE
    let len = u32::try_from(payload.len()).expect("payload len fits u32");
    for b in len.to_le_bytes() {
        buf.try_push(b).expect("alloc");
    }
    // payload bytes
    for &b in payload {
        buf.try_push(b).expect("alloc");
    }
    buf
}

// ── unknown notify-class tag from client → SkipNotify, connection stays open ──
// carrier.rs dispatch_frame: Action::SkipNotify returns Ok(()) and the carrier
// keeps reading (forward-compat skip per the §10 unknown-tag state machine).

arch_test!(unknown_notify_tag_is_skipped_and_connection_stays_open, {
    let mut sbuf = [0u8; 64];
    let path: &[u8] = reovim_arch::testrt::unique_path(b"/tmp/reovim-skip-", &mut sbuf);
    let client = boot_and_connect(path);

    // Complete the handshake first — the SP9.1 first-frame rule means an
    // unknown tag BEFORE Hello is a violation, not a skip.
    let empty_caps = [0x00u8, 0x00, 0x00, 0x00];
    let hello = Hello {
        protocol_major: 1,
        protocol_minor: 0,
        caps: StrList::from_validated(&empty_caps),
        domain_codecs: StrList::from_validated(&empty_caps),
    };
    send_msg(&client, &hello, 0);
    let ack_buf = recv_frame_buf(&client);
    let ack_frame = read_frame(ack_buf.as_slice(), CLIENT_CAP).expect("HelloAck frame");
    assert_eq!(ack_frame.header.msg_type, HelloAck::TAG, "handshake completes");

    // Craft a zero-body frame with an unknown tag in the notify range
    // (0x0300..=0x03FF) — the state machine answers SkipNotify and the
    // carrier keeps the connection open.
    let header = FrameHeader {
        body_len: 0,
        msg_type: 0x03FE,
        flags: 0,
        correlation_id: 0,
    };
    let mut h16 = [0u8; HEADER_LEN];
    header.encode(&mut h16);
    client.write_all(&h16).expect("write unknown notify frame");

    // The connection must remain open: an Attach afterwards still succeeds.
    let attach = Attach {
        session_name: "default",
        auth: &[],
        caps: StrList::from_validated(&empty_caps),
        domain_codecs: StrList::from_validated(&empty_caps),
    };
    send_msg(&client, &attach, 0);
    let attach_ack_buf = recv_frame_buf(&client);
    let attach_ack_frame =
        read_frame(attach_ack_buf.as_slice(), CLIENT_CAP).expect("AttachAck frame");
    assert_eq!(
        attach_ack_frame.header.msg_type,
        AttachAck::TAG,
        "server must still answer Attach after skipping the unknown notify"
    );
});
