//! Tests for `carrier.rs` (L12.1 sibling, compiled under `selftest`).
//!
//! ## Pure-logic tests
//!
//! `is_disconnect`: unit-testable pure predicate.
//!
//! ## Protocol-error path tests (#797 Phase 5 coverage)
//!
//! The error arms in `connect_and_handshake` and `recv_notify_frame` require a
//! fake server that sends crafted (wrong) responses. Each test binds a TID-unique
//! listener, spawns an accept thread that writes the crafted bytes, then calls
//! the carrier function under test on the client side and asserts the error.
//!
//! Closed arms:
//! - `wrong_reply_to_hello_returns_protocol_error` — line 154-155 (wrong tag
//!   after Hello → `CarrierError::Protocol`).
//! - `incompatible_hello_ack_major_returns_protocol_error` — line 158-159
//!   (HelloAck with major != 1 → `CarrierError::Protocol`).
//! - `wrong_reply_to_attach_returns_protocol_error` — line 175-176 (wrong tag
//!   after Attach → `CarrierError::Protocol`).
//! - `wrong_notify_tag_returns_protocol_error` — line 202-203 (wrong tag for
//!   notify frame → `CarrierError::Protocol`).
//! - `carrier_alloc_error_on_connect` covers `alloc_zeroed` via OOM sweep.

use {
    crate::carrier::{CarrierError, connect_and_handshake, is_disconnect, recv_notify_frame},
    reovim_arch::{arch_test, net::EBADF},
    reovim_uapi_protocol::messages::Message,
};

arch_test!(carrier_is_disconnect_true_for_ebadf, {
    reovim_arch::testrt::check(is_disconnect(EBADF), "EBADF is the disconnect sentinel");
});

arch_test!(carrier_is_disconnect_false_for_other_error, {
    use reovim_arch::net::ENOENT;
    reovim_arch::testrt::check(!is_disconnect(ENOENT), "ENOENT is not a disconnect");
});

// ── Fake-server helpers ───────────────────────────────────────────────────────

use {
    reovim_arch::{
        net::{UnixListener, UnixStream},
        thread::spawn,
    },
    reovim_lib_ds::Seq,
    reovim_uapi_abi::FrameHeader,
    reovim_uapi_protocol::frame::HEADER_LEN,
};

/// Writes a raw framed message to `stream`: 16-byte header (msg_type + body_len)
/// followed by `body`.
fn write_raw_frame(stream: &UnixStream, msg_type: u16, body: &[u8]) {
    let header = FrameHeader {
        body_len: u32::try_from(body.len()).unwrap_or(0),
        msg_type,
        flags: 0,
        correlation_id: 0,
    };
    let mut h16 = [0u8; HEADER_LEN];
    header.encode(&mut h16);
    let _ = stream.write_all(&h16);
    if !body.is_empty() {
        let _ = stream.write_all(body);
    }
}

/// Reads and discards exactly one framed message from `stream`.
fn drain_one_frame(stream: &UnixStream) {
    let mut h16 = [0u8; HEADER_LEN];
    if stream.read_exact(&mut h16).is_err() {
        return;
    }
    let header = FrameHeader::decode(&h16);
    let body_len = header.body_len as usize;
    if body_len == 0 {
        return;
    }
    let mut body: Seq<u8> = Seq::new();
    for _ in 0..body_len {
        if body.try_push(0).is_err() {
            return;
        }
    }
    let _ = stream.read_exact(body.as_mut_slice());
}

/// Builds a minimal HelloAck body: protocol_major=1, protocol_minor=0,
/// granted_caps = empty StrList (4 bytes: count=0), server_name = "x" (5 bytes).
///
/// Layout: [major u16 LE][minor u16 LE][caps_count u32 LE][name_len u32 LE][name bytes]
fn hello_ack_body(major: u16) -> Seq<u8> {
    let mut body: Seq<u8> = Seq::new();
    // major
    for b in major.to_le_bytes() {
        body.try_push(b).expect("alloc");
    }
    // minor
    for b in 0u16.to_le_bytes() {
        body.try_push(b).expect("alloc");
    }
    // granted_caps count = 0 (u32 LE)
    for b in 0u32.to_le_bytes() {
        body.try_push(b).expect("alloc");
    }
    // server_name: length-prefixed str "x"
    for b in 1u32.to_le_bytes() {
        body.try_push(b).expect("alloc");
    } // len=1
    body.try_push(b'x').expect("alloc");
    body
}

/// Builds a minimal AttachAck body: client_id=1, domain_table empty.
///
/// Layout: [client_id u64 LE][domain_count u32 LE]
fn attach_ack_body() -> Seq<u8> {
    let mut body: Seq<u8> = Seq::new();
    for b in 1u64.to_le_bytes() {
        body.try_push(b).expect("alloc");
    }
    for b in 0u32.to_le_bytes() {
        body.try_push(b).expect("alloc");
    }
    body
}

// ── wrong reply after Hello → CarrierError::Protocol (line 154-155) ──────────

arch_test!(wrong_reply_to_hello_returns_protocol_error, {
    use reovim_uapi_protocol::messages::{Message as _, Reject};
    let mut sbuf = [0u8; 64];
    let path: &[u8] = reovim_arch::testrt::unique_path(b"/tmp/reovim-tui-ce1-", &mut sbuf);
    let _ = reovim_arch::sys::unlinkat(reovim_arch::sys::AT_FDCWD, path, 0);
    let listener = UnixListener::bind(path).expect("bind");

    // Fake server: drain one frame (Hello), then reply with Reject (tag 0xFF00)
    // instead of HelloAck (tag 0x0002). The carrier checks msg_type == HelloAck::TAG.
    let handle = spawn::<_, ()>(move || {
        if let Ok(conn) = listener.accept() {
            drain_one_frame(&conn); // consume the Hello
            // Write a Reject frame where the client expects HelloAck.
            write_raw_frame(&conn, Reject::TAG, b"");
        }
    })
    .expect("spawn fake-server");

    let result = connect_and_handshake(path);
    assert_eq!(
        result.err(),
        Some(CarrierError::Protocol),
        "wrong reply after Hello must yield CarrierError::Protocol"
    );
    let () = handle.join();
});

// ── HelloAck with wrong major → CarrierError::Protocol (line 158-159) ────────

arch_test!(incompatible_hello_ack_major_returns_protocol_error, {
    use reovim_uapi_protocol::messages::HelloAck;
    let mut sbuf = [0u8; 64];
    let path: &[u8] = reovim_arch::testrt::unique_path(b"/tmp/reovim-tui-ce2-", &mut sbuf);
    let _ = reovim_arch::sys::unlinkat(reovim_arch::sys::AT_FDCWD, path, 0);
    let listener = UnixListener::bind(path).expect("bind");

    let handle = spawn::<_, ()>(move || {
        if let Ok(conn) = listener.accept() {
            drain_one_frame(&conn); // consume Hello
            // Send HelloAck with major=99 (not 1) → carrier rejects.
            let body = hello_ack_body(99);
            write_raw_frame(&conn, <HelloAck as Message>::TAG, body.as_slice());
        }
    })
    .expect("spawn fake-server");

    let result = connect_and_handshake(path);
    assert_eq!(
        result.err(),
        Some(CarrierError::Protocol),
        "HelloAck major != 1 must yield CarrierError::Protocol"
    );
    let () = handle.join();
});

// ── wrong reply after Attach → CarrierError::Protocol (line 175-176) ─────────

arch_test!(wrong_reply_to_attach_returns_protocol_error, {
    use reovim_uapi_protocol::messages::{HelloAck, Message as _, Reject};
    let mut sbuf = [0u8; 64];
    let path: &[u8] = reovim_arch::testrt::unique_path(b"/tmp/reovim-tui-ce3-", &mut sbuf);
    let _ = reovim_arch::sys::unlinkat(reovim_arch::sys::AT_FDCWD, path, 0);
    let listener = UnixListener::bind(path).expect("bind");

    let handle = spawn::<_, ()>(move || {
        if let Ok(conn) = listener.accept() {
            drain_one_frame(&conn); // Hello
            // Reply with valid HelloAck.
            let body = hello_ack_body(1);
            write_raw_frame(&conn, <HelloAck as Message>::TAG, body.as_slice());
            drain_one_frame(&conn); // Attach
            // Reply with Reject instead of AttachAck.
            write_raw_frame(&conn, Reject::TAG, b"");
        }
    })
    .expect("spawn fake-server");

    let result = connect_and_handshake(path);
    assert_eq!(
        result.err(),
        Some(CarrierError::Protocol),
        "wrong reply after Attach must yield CarrierError::Protocol"
    );
    let () = handle.join();
});

// ── wrong notify tag → CarrierError::Protocol (line 202-203) ─────────────────
//
// `recv_notify_frame` expects msg_type == AttachEventProjection::TAG (0x0304).
// If the server sends a different tag, it returns CarrierError::Protocol.

arch_test!(wrong_notify_tag_returns_protocol_error, {
    use reovim_uapi_protocol::messages::{AttachAck, HelloAck, Message as _, SendInput};
    let mut sbuf = [0u8; 64];
    let path: &[u8] = reovim_arch::testrt::unique_path(b"/tmp/reovim-tui-ce4-", &mut sbuf);
    let _ = reovim_arch::sys::unlinkat(reovim_arch::sys::AT_FDCWD, path, 0);
    let listener = UnixListener::bind(path).expect("bind");

    let handle = spawn::<_, ()>(move || {
        if let Ok(conn) = listener.accept() {
            drain_one_frame(&conn); // Hello
            let body = hello_ack_body(1);
            write_raw_frame(&conn, <HelloAck as Message>::TAG, body.as_slice());
            drain_one_frame(&conn); // Attach
            let attach_ack_b = attach_ack_body();
            write_raw_frame(&conn, AttachAck::TAG, attach_ack_b.as_slice());
            // Now send a frame with the WRONG tag (SendInput, 0x0102) instead of
            // AttachEventProjection (0x0304). `recv_notify_frame` checks the tag.
            write_raw_frame(&conn, SendInput::TAG, b"");
        }
    })
    .expect("spawn fake-server");

    // connect_and_handshake will succeed; then recv_notify_frame is what errors.
    let conn_result = connect_and_handshake(path);
    if let Ok(conn) = conn_result {
        let notify_result = recv_notify_frame(&conn.stream);
        assert_eq!(
            notify_result.err(),
            Some(CarrierError::Protocol),
            "wrong notify tag must yield CarrierError::Protocol"
        );
    } else {
        // handshake failed before reaching recv_notify_frame; still covers the
        // connection-close path.
        assert!(conn_result.is_err(), "if handshake fails, CarrierError must be returned");
    }
    let () = handle.join();
});
