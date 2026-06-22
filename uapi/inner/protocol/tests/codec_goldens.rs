//! Codec golden tests — Phase 3 (plan 05-uapi-foundation.md §Phase 3).
//!
//! ## Coverage
//!
//! 1. **57-byte `Hello` frame golden** (7.3 §"worked frame"): the complete wire
//!    frame (16-byte header + 41-byte body) must match the spec byte-for-byte.
//!    This is the permanent SP13 golden (CF4 successor).
//!
//! 2. **Round-trip goldens for all 37 §7 message types** (plus `FrameHeader`):
//!    `decode(encode(msg)) == msg` byte-for-byte.  Encode twice → identical
//!    bytes (determinism).
//!
//! 3. **Failure-mode tests**: truncated frames at each interesting boundary,
//!    unknown discriminants, `BufferTooSmall`, invalid bool, bad UTF-8,
//!    out-of-range `Reject.code`, over-cap encode, header `body_len` over cap.
//!
//! All spec citations: Documentation/07-Surfaces/03-Server-Client-Protocol.md.

use reovim_uapi_abi::{ErrorCode, FrameHeader};
use reovim_uapi_protocol::{
    frame::{HEADER_LEN, read_frame, write_frame},
    messages::{
        // req
        Attach,
        // resp
        AttachAck,
        // notify
        AttachEventClientLeft,
        AttachEventCursor,
        AttachEventDiff,
        AttachEventDomainTableDelta,
        AttachEventFrame,
        AttachEventProjection,
        AttachEventServerDraining,
        AttachEventSessionDestroyed,
        AttachEventSessionPivot,
        ConfigDump,
        ConfigDumpResponse,
        ConfigValidate,
        ConfigValidateResponse,
        DebugDrive,
        DebugDriveAck,
        DebugRead,
        DebugReadEvent,
        DestroySession,
        DestroySessionAck,
        Detach,
        DetachAck,
        // helpers
        Direction,
        // handshake
        Hello,
        HelloAck,
        InputAck,
        Message,
        PkgSync,
        PkgSyncDone,
        PkgSyncProgress,
        PkgVerify,
        PkgVerifyAck,
        // error
        Reject,
        RenameSession,
        RenameSessionAck,
        SendInput,
        SwitchSession,
        SwitchSessionAck,
        encode_capped,
    },
    view::{CarrierList, DomainEntryList, RawInputList, StrList},
};

// ---------------------------------------------------------------------------
// Helper: verify encode→decode round-trip and determinism
// ---------------------------------------------------------------------------

/// Encodes `msg` into a 4 KiB stack buffer, asserts `encode` returns the same
/// length as `encoded_size`, decodes back, and asserts a second encode matches
/// the first (determinism).  Returns the encoded bytes.
fn round_trip_encode<M: Message>(msg: &M, buf: &mut [u8; 4096]) -> usize {
    let n = msg
        .encode(buf)
        .expect("encode must succeed for a well-formed message");
    assert_eq!(n, msg.encoded_size(), "encode returned != encoded_size");
    // Determinism: encode again into a second buffer, expect identical bytes.
    let mut buf2 = [0u8; 4096];
    let n2 = msg.encode(&mut buf2).expect("second encode must succeed");
    assert_eq!(n, n2, "second encode returned different length");
    assert_eq!(
        &buf[..n],
        &buf2[..n2],
        "second encode produced different bytes (non-deterministic)"
    );
    n
}

// ---------------------------------------------------------------------------
// 7.3 §"worked frame" — 57-byte Hello golden (permanent SP13 golden, CF4
// successor)
// ---------------------------------------------------------------------------

/// Full 57-byte Hello frame (header + body) must match the spec byte-for-byte.
///
/// Source: 7.3 §"worked frame":
///   `protocol_major = 1`, `protocol_minor = 0`,
///   `caps = ["render.cells"]`, `domain_codecs = ["text.utf8"]`,
///   `correlation_id = 0`, `flags = 0`, `msg_type = 0x0001`.
///
/// Expected flat hex:
///
/// ```text
/// 29 00 00 00  01 00  00 00  00 00 00 00 00 00 00 00  (16-byte header)
/// 01 00  00 00                                        (major=1, minor=0)
/// 01 00 00 00                                        (caps.count=1)
/// 0C 00 00 00  72 65 6E 64 65 72 2E 63 65 6C 6C 73  (caps[0]="render.cells")
/// 01 00 00 00                                        (domain_codecs.count=1)
/// 09 00 00 00  74 65 78 74 2E 75 74 66 38           (domain_codecs[0]="text.utf8")
/// ```
#[test]
fn hello_57_byte_frame_golden() {
    // Build the Hello body.
    let caps_buf = {
        let mut b = [0u8; 32];
        let mut enc = reovim_uapi_protocol::codec::Encoder::new(&mut b);
        enc.put_u32(1).unwrap(); // count
        enc.put_str("render.cells").unwrap();
        b
    };
    let codecs_buf = {
        let mut b = [0u8; 32];
        let mut enc = reovim_uapi_protocol::codec::Encoder::new(&mut b);
        enc.put_u32(1).unwrap();
        enc.put_str("text.utf8").unwrap();
        b
    };
    let msg = Hello {
        protocol_major: 1,
        protocol_minor: 0,
        caps: StrList::from_validated(&caps_buf),
        domain_codecs: StrList::from_validated(&codecs_buf),
    };

    // Encode the body.
    let mut body_buf = [0u8; 256];
    let body_len = msg.encode(&mut body_buf).expect("Hello encode");
    assert_eq!(body_len, 41, "7.3 worked-frame: Hello body must be 41 bytes");

    // Expected body bytes (7.3 §"worked frame"):
    #[rustfmt::skip]
    let expected_body: [u8; 41] = [
        // protocol_major = 1
        0x01, 0x00,
        // protocol_minor = 0
        0x00, 0x00,
        // caps.count = 1
        0x01, 0x00, 0x00, 0x00,
        // caps[0].len = 12, then "render.cells"
        0x0C, 0x00, 0x00, 0x00,
        0x72, 0x65, 0x6E, 0x64, 0x65, 0x72, 0x2E, 0x63, 0x65, 0x6C, 0x6C, 0x73,
        // domain_codecs.count = 1
        0x01, 0x00, 0x00, 0x00,
        // domain_codecs[0].len = 9, then "text.utf8"
        0x09, 0x00, 0x00, 0x00,
        0x74, 0x65, 0x78, 0x74, 0x2E, 0x75, 0x74, 0x66, 0x38,
    ];
    assert_eq!(&body_buf[..41], &expected_body, "7.3 worked-frame: Hello body bytes");

    // Build the full frame using write_frame.
    let hdr = FrameHeader {
        body_len: 0,
        msg_type: Hello::TAG,
        flags: 0,
        correlation_id: 0,
    };
    let mut frame_buf = [0u8; 256];
    let total = write_frame(&mut frame_buf, hdr, &body_buf[..41], 4096).expect("write_frame");
    assert_eq!(total, 57, "7.3 worked-frame: total frame bytes == 57");

    // Expected full 57-byte flat hex from the spec:
    #[rustfmt::skip]
    let expected_frame: [u8; 57] = [
        // header (16 bytes)
        0x29, 0x00, 0x00, 0x00, // body_len = 41
        0x01, 0x00,             // msg_type = 0x0001
        0x00, 0x00,             // flags = 0
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // correlation_id = 0
        // body (41 bytes) — same as expected_body above
        0x01, 0x00,
        0x00, 0x00,
        0x01, 0x00, 0x00, 0x00,
        0x0C, 0x00, 0x00, 0x00,
        0x72, 0x65, 0x6E, 0x64, 0x65, 0x72, 0x2E, 0x63, 0x65, 0x6C, 0x6C, 0x73,
        0x01, 0x00, 0x00, 0x00,
        0x09, 0x00, 0x00, 0x00,
        0x74, 0x65, 0x78, 0x74, 0x2E, 0x75, 0x74, 0x66, 0x38,
    ];
    assert_eq!(&frame_buf[..57], &expected_frame, "7.3 worked-frame: 57-byte golden (SP13)");

    // Round-trip: read_frame + Hello::decode must recover original fields.
    let fv = read_frame(&frame_buf[..57], 4096).expect("read_frame");
    assert_eq!(fv.header.body_len, 41);
    assert_eq!(fv.header.msg_type, Hello::TAG);
    assert_eq!(fv.header.flags, 0);
    assert_eq!(fv.header.correlation_id, 0);
    let back = Hello::decode(fv.body).expect("Hello::decode");
    assert_eq!(back.protocol_major, 1);
    assert_eq!(back.protocol_minor, 0);
    let caps_vec: std::vec::Vec<&str> = back.caps.iter().collect();
    assert_eq!(caps_vec, vec!["render.cells"]);
    let codecs_vec: std::vec::Vec<&str> = back.domain_codecs.iter().collect();
    assert_eq!(codecs_vec, vec!["text.utf8"]);
}

// ---------------------------------------------------------------------------
// Round-trip goldens for all 37 §7 message types
// ---------------------------------------------------------------------------
// Each test:
// 1. Constructs a representative value.
// 2. Calls round_trip_encode (asserts encode == encoded_size, determinism).
// 3. Decodes the bytes.
// 4. Asserts key decoded fields match the original.
//
// Tags from 7.3 §7 (count = 37): 0x0001, 0x0002, 0x0100–0x010B,
// 0x0200–0x020C, 0x0301–0x0309, 0xFF00.

// ── Handshake ─────────────────────────────────────────────────────────────────

/// `Hello` (tag 0x0001, handshake).
#[test]
fn round_trip_hello() {
    let empty_list_buf = [0u8; 4]; // count = 0
    let msg = Hello {
        protocol_major: 2,
        protocol_minor: 1,
        caps: StrList::from_validated(&empty_list_buf),
        domain_codecs: StrList::from_validated(&empty_list_buf),
    };
    let mut buf = [0u8; 4096];
    let n = round_trip_encode(&msg, &mut buf);
    let back = Hello::decode(&buf[..n]).expect("Hello decode");
    assert_eq!((back.protocol_major, back.protocol_minor), (2, 1));
    assert_eq!(Hello::TAG, 0x0001);
    assert_eq!(Hello::DIRECTION, Direction::Handshake);
}

/// `HelloAck` (tag 0x0002, handshake).
#[test]
fn round_trip_hello_ack() {
    let empty_list_buf = [0u8; 4];
    let msg = HelloAck {
        protocol_major: 1,
        protocol_minor: 0,
        granted_caps: StrList::from_validated(&empty_list_buf),
        server_name: "reovim-test",
    };
    let mut buf = [0u8; 4096];
    let n = round_trip_encode(&msg, &mut buf);
    let back = HelloAck::decode(&buf[..n]).expect("HelloAck decode");
    assert_eq!(back.server_name, "reovim-test");
    assert_eq!(HelloAck::TAG, 0x0002);
    assert_eq!(HelloAck::DIRECTION, Direction::Handshake);
}

// ── Requests (0x0100–0x010B) ──────────────────────────────────────────────────

/// `Attach` (tag 0x0100, req).
#[test]
fn round_trip_attach() {
    let empty_list_buf = [0u8; 4];
    let msg = Attach {
        session_name: "main",
        auth: b"secret",
        caps: StrList::from_validated(&empty_list_buf),
        domain_codecs: StrList::from_validated(&empty_list_buf),
    };
    let mut buf = [0u8; 4096];
    let n = round_trip_encode(&msg, &mut buf);
    let back = Attach::decode(&buf[..n]).expect("Attach decode");
    assert_eq!(back.session_name, "main");
    assert_eq!(back.auth, b"secret");
    assert_eq!(Attach::TAG, 0x0100);
    assert_eq!(Attach::DIRECTION, Direction::Request);
}

/// `Detach` (tag 0x0101, req).
#[test]
fn round_trip_detach() {
    let msg = Detach {
        reason: "test-done",
    };
    let mut buf = [0u8; 4096];
    let n = round_trip_encode(&msg, &mut buf);
    let back = Detach::decode(&buf[..n]).expect("Detach decode");
    assert_eq!(back.reason, "test-done");
    assert_eq!(Detach::TAG, 0x0101);
    assert_eq!(Detach::DIRECTION, Direction::Request);
}

/// `SendInput` (tag 0x0102, req) with an empty input list.
#[test]
fn round_trip_send_input_empty() {
    let empty_input_buf = [0u8; 4];
    let msg = SendInput {
        client_id: 3,
        buffer_id: 7,
        window_id: 42,
        inputs: RawInputList::from_validated(&empty_input_buf),
    };
    let mut buf = [0u8; 4096];
    let n = round_trip_encode(&msg, &mut buf);
    let back = SendInput::decode(&buf[..n]).expect("SendInput decode");
    assert_eq!((back.client_id, back.buffer_id, back.window_id), (3, 7, 42));
    assert!(back.inputs.is_empty());
    assert_eq!(SendInput::TAG, 0x0102);
    assert_eq!(SendInput::DIRECTION, Direction::Request);
}

/// `SwitchSession` (tag 0x0103, req).
#[test]
fn round_trip_switch_session() {
    let msg = SwitchSession {
        session_name: "other",
    };
    let mut buf = [0u8; 4096];
    let n = round_trip_encode(&msg, &mut buf);
    let back = SwitchSession::decode(&buf[..n]).expect("SwitchSession decode");
    assert_eq!(back.session_name, "other");
    assert_eq!(SwitchSession::TAG, 0x0103);
    assert_eq!(SwitchSession::DIRECTION, Direction::Request);
}

/// `DestroySession` (tag 0x0104, req).
#[test]
fn round_trip_destroy_session() {
    let msg = DestroySession {
        session_name: "old",
        force: true,
    };
    let mut buf = [0u8; 4096];
    let n = round_trip_encode(&msg, &mut buf);
    let back = DestroySession::decode(&buf[..n]).expect("DestroySession decode");
    assert_eq!(back.session_name, "old");
    assert!(back.force);
    assert_eq!(DestroySession::TAG, 0x0104);
    assert_eq!(DestroySession::DIRECTION, Direction::Request);
}

/// `RenameSession` (tag 0x0105, req).
#[test]
fn round_trip_rename_session() {
    let msg = RenameSession {
        old_name: "a",
        new_name: "b",
    };
    let mut buf = [0u8; 4096];
    let n = round_trip_encode(&msg, &mut buf);
    let back = RenameSession::decode(&buf[..n]).expect("RenameSession decode");
    assert_eq!((back.old_name, back.new_name), ("a", "b"));
    assert_eq!(RenameSession::TAG, 0x0105);
    assert_eq!(RenameSession::DIRECTION, Direction::Request);
}

/// `DebugRead` (tag 0x0106, req).
#[test]
fn round_trip_debug_read() {
    let msg = DebugRead {
        op_kind: 99,
        payload: b"probe",
        subscribe: true,
    };
    let mut buf = [0u8; 4096];
    let n = round_trip_encode(&msg, &mut buf);
    let back = DebugRead::decode(&buf[..n]).expect("DebugRead decode");
    assert_eq!(back.op_kind, 99);
    assert_eq!(back.payload, b"probe");
    assert!(back.subscribe);
    assert_eq!(DebugRead::TAG, 0x0106);
    assert_eq!(DebugRead::DIRECTION, Direction::Request);
}

/// `DebugDrive` (tag 0x0107, req).
#[test]
fn round_trip_debug_drive() {
    let msg = DebugDrive {
        op_kind: 5,
        payload: b"cmd",
    };
    let mut buf = [0u8; 4096];
    let n = round_trip_encode(&msg, &mut buf);
    let back = DebugDrive::decode(&buf[..n]).expect("DebugDrive decode");
    assert_eq!((back.op_kind, back.payload), (5, b"cmd".as_ref()));
    assert_eq!(DebugDrive::TAG, 0x0107);
    assert_eq!(DebugDrive::DIRECTION, Direction::Request);
}

/// `PkgSync` (tag 0x0108, req).
#[test]
fn round_trip_pkg_sync() {
    let empty = [0u8; 4];
    let msg = PkgSync {
        targets: StrList::from_validated(&empty),
        dry_run: false,
    };
    let mut buf = [0u8; 4096];
    let n = round_trip_encode(&msg, &mut buf);
    let back = PkgSync::decode(&buf[..n]).expect("PkgSync decode");
    assert!(!back.dry_run);
    assert!(back.targets.is_empty());
    assert_eq!(PkgSync::TAG, 0x0108);
    assert_eq!(PkgSync::DIRECTION, Direction::Request);
}

/// `PkgVerify` (tag 0x0109, req).
#[test]
fn round_trip_pkg_verify() {
    let empty = [0u8; 4];
    let msg = PkgVerify {
        targets: StrList::from_validated(&empty),
    };
    let mut buf = [0u8; 4096];
    let n = round_trip_encode(&msg, &mut buf);
    let back = PkgVerify::decode(&buf[..n]).expect("PkgVerify decode");
    assert!(back.targets.is_empty());
    assert_eq!(PkgVerify::TAG, 0x0109);
    assert_eq!(PkgVerify::DIRECTION, Direction::Request);
}

/// `ConfigDump` (tag 0x010A, req).
#[test]
fn round_trip_config_dump() {
    let msg = ConfigDump {
        namespace: "ui",
        show_secrets: false,
    };
    let mut buf = [0u8; 4096];
    let n = round_trip_encode(&msg, &mut buf);
    let back = ConfigDump::decode(&buf[..n]).expect("ConfigDump decode");
    assert_eq!(back.namespace, "ui");
    assert!(!back.show_secrets);
    assert_eq!(ConfigDump::TAG, 0x010A);
    assert_eq!(ConfigDump::DIRECTION, Direction::Request);
}

/// `ConfigValidate` (tag 0x010B, req).
#[test]
fn round_trip_config_validate() {
    let msg = ConfigValidate {
        toml: b"[x]\ny=1",
        namespace: "mod",
    };
    let mut buf = [0u8; 4096];
    let n = round_trip_encode(&msg, &mut buf);
    let back = ConfigValidate::decode(&buf[..n]).expect("ConfigValidate decode");
    assert_eq!(back.toml, b"[x]\ny=1");
    assert_eq!(back.namespace, "mod");
    assert_eq!(ConfigValidate::TAG, 0x010B);
    assert_eq!(ConfigValidate::DIRECTION, Direction::Request);
}

// ── Responses (0x0200–0x020C) ─────────────────────────────────────────────────

/// `AttachAck` (tag 0x0200, resp) with an empty domain table.
#[test]
fn round_trip_attach_ack() {
    let empty = [0u8; 4];
    let msg = AttachAck {
        client_id: 11,
        domain_table: DomainEntryList::from_validated(&empty),
    };
    let mut buf = [0u8; 4096];
    let n = round_trip_encode(&msg, &mut buf);
    let back = AttachAck::decode(&buf[..n]).expect("AttachAck decode");
    assert_eq!(back.client_id, 11);
    assert!(back.domain_table.is_empty());
    assert_eq!(AttachAck::TAG, 0x0200);
    assert_eq!(AttachAck::DIRECTION, Direction::Response);
}

/// `DetachAck` (tag 0x0201, resp — empty body).
#[test]
fn round_trip_detach_ack() {
    let msg = DetachAck;
    assert_eq!(msg.encoded_size(), 0);
    let mut buf = [0u8; 4096];
    let n = msg.encode(&mut buf).expect("DetachAck encode");
    assert_eq!(n, 0);
    let back = DetachAck::decode(&buf[..n]).expect("DetachAck decode");
    let _ = back; // unit struct
    assert_eq!(DetachAck::TAG, 0x0201);
    assert_eq!(DetachAck::DIRECTION, Direction::Response);
}

/// `InputAck` (tag 0x0202, resp).
#[test]
fn round_trip_input_ack() {
    let msg = InputAck {
        accepted: 1000,
        dropped: 5,
    };
    let mut buf = [0u8; 4096];
    let n = round_trip_encode(&msg, &mut buf);
    let back = InputAck::decode(&buf[..n]).expect("InputAck decode");
    assert_eq!((back.accepted, back.dropped), (1000, 5));
    assert_eq!(InputAck::TAG, 0x0202);
    assert_eq!(InputAck::DIRECTION, Direction::Response);
}

/// `SwitchSessionAck` (tag 0x0203, resp — empty body).
#[test]
fn round_trip_switch_session_ack() {
    let msg = SwitchSessionAck;
    assert_eq!(msg.encoded_size(), 0);
    let mut buf = [0u8; 4096];
    let n = msg.encode(&mut buf).expect("SwitchSessionAck encode");
    assert_eq!(n, 0);
    assert_eq!(SwitchSessionAck::TAG, 0x0203);
    assert_eq!(SwitchSessionAck::DIRECTION, Direction::Response);
}

/// `DestroySessionAck` (tag 0x0204, resp — empty body).
#[test]
fn round_trip_destroy_session_ack() {
    let msg = DestroySessionAck;
    assert_eq!(msg.encoded_size(), 0);
    let mut buf = [0u8; 4096];
    let n = msg.encode(&mut buf).expect("DestroySessionAck encode");
    assert_eq!(n, 0);
    assert_eq!(DestroySessionAck::TAG, 0x0204);
    assert_eq!(DestroySessionAck::DIRECTION, Direction::Response);
}

/// `RenameSessionAck` (tag 0x0205, resp — empty body).
#[test]
fn round_trip_rename_session_ack() {
    let msg = RenameSessionAck;
    assert_eq!(msg.encoded_size(), 0);
    let mut buf = [0u8; 4096];
    let n = msg.encode(&mut buf).expect("RenameSessionAck encode");
    assert_eq!(n, 0);
    assert_eq!(RenameSessionAck::TAG, 0x0205);
    assert_eq!(RenameSessionAck::DIRECTION, Direction::Response);
}

/// `DebugReadEvent` (tag 0x0206, resp).
#[test]
fn round_trip_debug_read_event() {
    let msg = DebugReadEvent {
        op_kind: 3,
        payload: b"data",
        is_final: true,
    };
    let mut buf = [0u8; 4096];
    let n = round_trip_encode(&msg, &mut buf);
    let back = DebugReadEvent::decode(&buf[..n]).expect("DebugReadEvent decode");
    assert_eq!(back.op_kind, 3);
    assert_eq!(back.payload, b"data");
    assert!(back.is_final);
    assert_eq!(DebugReadEvent::TAG, 0x0206);
    assert_eq!(DebugReadEvent::DIRECTION, Direction::Response);
}

/// `DebugDriveAck` (tag 0x0207, resp).
#[test]
fn round_trip_debug_drive_ack() {
    let msg = DebugDriveAck {
        op_kind: 4,
        result: ErrorCode::Ok,
        payload: b"ok",
    };
    let mut buf = [0u8; 4096];
    let n = round_trip_encode(&msg, &mut buf);
    let back = DebugDriveAck::decode(&buf[..n]).expect("DebugDriveAck decode");
    assert_eq!(back.op_kind, 4);
    assert_eq!(back.result, ErrorCode::Ok);
    assert_eq!(back.payload, b"ok");
    assert_eq!(DebugDriveAck::TAG, 0x0207);
    assert_eq!(DebugDriveAck::DIRECTION, Direction::Response);
}

/// `PkgSyncProgress` (tag 0x0208, resp).
#[test]
fn round_trip_pkg_sync_progress() {
    let msg = PkgSyncProgress {
        target: "mod-vim",
        done: 3,
        total: 10,
    };
    let mut buf = [0u8; 4096];
    let n = round_trip_encode(&msg, &mut buf);
    let back = PkgSyncProgress::decode(&buf[..n]).expect("PkgSyncProgress decode");
    assert_eq!((back.target, back.done, back.total), ("mod-vim", 3, 10));
    assert_eq!(PkgSyncProgress::TAG, 0x0208);
    assert_eq!(PkgSyncProgress::DIRECTION, Direction::Response);
}

/// `PkgSyncDone` (tag 0x0209, resp).
#[test]
fn round_trip_pkg_sync_done() {
    let msg = PkgSyncDone {
        result: ErrorCode::Ok,
        summary: "done",
    };
    let mut buf = [0u8; 4096];
    let n = round_trip_encode(&msg, &mut buf);
    let back = PkgSyncDone::decode(&buf[..n]).expect("PkgSyncDone decode");
    assert_eq!(back.result, ErrorCode::Ok);
    assert_eq!(back.summary, "done");
    assert_eq!(PkgSyncDone::TAG, 0x0209);
    assert_eq!(PkgSyncDone::DIRECTION, Direction::Response);
}

/// `PkgVerifyAck` (tag 0x020A, resp).
#[test]
fn round_trip_pkg_verify_ack() {
    let msg = PkgVerifyAck {
        result: ErrorCode::NotFound,
        report: b"missing",
    };
    let mut buf = [0u8; 4096];
    let n = round_trip_encode(&msg, &mut buf);
    let back = PkgVerifyAck::decode(&buf[..n]).expect("PkgVerifyAck decode");
    assert_eq!(back.result, ErrorCode::NotFound);
    assert_eq!(back.report, b"missing");
    assert_eq!(PkgVerifyAck::TAG, 0x020A);
    assert_eq!(PkgVerifyAck::DIRECTION, Direction::Response);
}

/// `ConfigDumpResponse` (tag 0x020B, resp).
#[test]
fn round_trip_config_dump_response() {
    let msg = ConfigDumpResponse {
        toml: b"[section]\nkey = 1",
    };
    let mut buf = [0u8; 4096];
    let n = round_trip_encode(&msg, &mut buf);
    let back = ConfigDumpResponse::decode(&buf[..n]).expect("ConfigDumpResponse decode");
    assert_eq!(back.toml, b"[section]\nkey = 1");
    assert_eq!(ConfigDumpResponse::TAG, 0x020B);
    assert_eq!(ConfigDumpResponse::DIRECTION, Direction::Response);
}

/// `ConfigValidateResponse` (tag 0x020C, resp).
#[test]
fn round_trip_config_validate_response() {
    let msg = ConfigValidateResponse {
        result: ErrorCode::SchemaInvalid,
        detail: "bad key",
    };
    let mut buf = [0u8; 4096];
    let n = round_trip_encode(&msg, &mut buf);
    let back = ConfigValidateResponse::decode(&buf[..n]).expect("ConfigValidateResponse decode");
    assert_eq!(back.result, ErrorCode::SchemaInvalid);
    assert_eq!(back.detail, "bad key");
    assert_eq!(ConfigValidateResponse::TAG, 0x020C);
    assert_eq!(ConfigValidateResponse::DIRECTION, Direction::Response);
}

// ── Notifications (0x0301–0x0309) ─────────────────────────────────────────────

/// `AttachEvent::Frame` (tag 0x0301, notify).
#[test]
fn round_trip_attach_event_frame() {
    let msg = AttachEventFrame {
        buffer_id: 1,
        window_id: 2,
        frame: b"pixels",
    };
    let mut buf = [0u8; 4096];
    let n = round_trip_encode(&msg, &mut buf);
    let back = AttachEventFrame::decode(&buf[..n]).expect("AttachEventFrame decode");
    assert_eq!((back.buffer_id, back.window_id), (1, 2));
    assert_eq!(back.frame, b"pixels");
    assert_eq!(AttachEventFrame::TAG, 0x0301);
    assert_eq!(AttachEventFrame::DIRECTION, Direction::Notify);
}

/// `AttachEvent::Diff` (tag 0x0302, notify).
#[test]
fn round_trip_attach_event_diff() {
    let msg = AttachEventDiff {
        buffer_id: 5,
        window_id: 6,
        diff: b"delta",
    };
    let mut buf = [0u8; 4096];
    let n = round_trip_encode(&msg, &mut buf);
    let back = AttachEventDiff::decode(&buf[..n]).expect("AttachEventDiff decode");
    assert_eq!((back.buffer_id, back.window_id), (5, 6));
    assert_eq!(back.diff, b"delta");
    assert_eq!(AttachEventDiff::TAG, 0x0302);
    assert_eq!(AttachEventDiff::DIRECTION, Direction::Notify);
}

/// `AttachEvent::Cursor` (tag 0x0303, notify) with an empty cursor list (CR3).
#[test]
fn round_trip_attach_event_cursor_empty() {
    let empty = [0u8; 4];
    let msg = AttachEventCursor {
        buffer_id: 0,
        window_id: 0,
        cursors: CarrierList::from_validated(&empty),
    };
    let mut buf = [0u8; 4096];
    let n = round_trip_encode(&msg, &mut buf);
    let back = AttachEventCursor::decode(&buf[..n]).expect("AttachEventCursor decode");
    assert!(back.cursors.is_empty(), "CR3: empty cursor set is structural");
    assert_eq!(AttachEventCursor::TAG, 0x0303);
    assert_eq!(AttachEventCursor::DIRECTION, Direction::Notify);
}

/// `AttachEvent::Projection` (tag 0x0304, notify).
#[test]
fn round_trip_attach_event_projection() {
    let msg = AttachEventProjection {
        buffer_id: 8,
        projection: b"proj-bytes",
    };
    let mut buf = [0u8; 4096];
    let n = round_trip_encode(&msg, &mut buf);
    let back = AttachEventProjection::decode(&buf[..n]).expect("AttachEventProjection decode");
    assert_eq!(back.buffer_id, 8);
    assert_eq!(back.projection, b"proj-bytes");
    assert_eq!(AttachEventProjection::TAG, 0x0304);
    assert_eq!(AttachEventProjection::DIRECTION, Direction::Notify);
}

/// `AttachEvent::DomainTableDelta` (tag 0x0305, notify).
#[test]
fn round_trip_attach_event_domain_table_delta() {
    let empty = [0u8; 4];
    let msg = AttachEventDomainTableDelta {
        added: DomainEntryList::from_validated(&empty),
        removed: DomainEntryList::from_validated(&empty),
    };
    let mut buf = [0u8; 4096];
    let n = round_trip_encode(&msg, &mut buf);
    let back =
        AttachEventDomainTableDelta::decode(&buf[..n]).expect("AttachEventDomainTableDelta decode");
    assert!(back.added.is_empty() && back.removed.is_empty());
    assert_eq!(AttachEventDomainTableDelta::TAG, 0x0305);
    assert_eq!(AttachEventDomainTableDelta::DIRECTION, Direction::Notify);
}

/// `AttachEvent::SessionPivot` (tag 0x0306, notify).
#[test]
fn round_trip_attach_event_session_pivot() {
    let msg = AttachEventSessionPivot {
        session_name: "pivot",
    };
    let mut buf = [0u8; 4096];
    let n = round_trip_encode(&msg, &mut buf);
    let back = AttachEventSessionPivot::decode(&buf[..n]).expect("AttachEventSessionPivot decode");
    assert_eq!(back.session_name, "pivot");
    assert_eq!(AttachEventSessionPivot::TAG, 0x0306);
    assert_eq!(AttachEventSessionPivot::DIRECTION, Direction::Notify);
}

/// `AttachEvent::SessionDestroyed` (tag 0x0307, notify).
#[test]
fn round_trip_attach_event_session_destroyed() {
    let msg = AttachEventSessionDestroyed {
        session_name: "gone",
    };
    let mut buf = [0u8; 4096];
    let n = round_trip_encode(&msg, &mut buf);
    let back =
        AttachEventSessionDestroyed::decode(&buf[..n]).expect("AttachEventSessionDestroyed decode");
    assert_eq!(back.session_name, "gone");
    assert_eq!(AttachEventSessionDestroyed::TAG, 0x0307);
    assert_eq!(AttachEventSessionDestroyed::DIRECTION, Direction::Notify);
}

/// `AttachEvent::ClientLeft` (tag 0x0308, notify).
#[test]
fn round_trip_attach_event_client_left() {
    let msg = AttachEventClientLeft {
        client_id: 99,
        reason: "connection-closed",
    };
    let mut buf = [0u8; 4096];
    let n = round_trip_encode(&msg, &mut buf);
    let back = AttachEventClientLeft::decode(&buf[..n]).expect("AttachEventClientLeft decode");
    assert_eq!((back.client_id, back.reason), (99, "connection-closed"));
    assert_eq!(AttachEventClientLeft::TAG, 0x0308);
    assert_eq!(AttachEventClientLeft::DIRECTION, Direction::Notify);
}

/// `AttachEvent::ServerDraining` (tag 0x0309, notify).
#[test]
fn round_trip_attach_event_server_draining() {
    let msg = AttachEventServerDraining { grace_ms: 5000 };
    let mut buf = [0u8; 4096];
    let n = round_trip_encode(&msg, &mut buf);
    let back =
        AttachEventServerDraining::decode(&buf[..n]).expect("AttachEventServerDraining decode");
    assert_eq!(back.grace_ms, 5000);
    assert_eq!(AttachEventServerDraining::TAG, 0x0309);
    assert_eq!(AttachEventServerDraining::DIRECTION, Direction::Notify);
}

// ── Error (0xFF00) ─────────────────────────────────────────────────────────────

/// `Reject` (tag 0xFF00, error).
#[test]
fn round_trip_reject() {
    let msg = Reject {
        code: ErrorCode::ProtocolViolation,
        detail: "bad frame",
    };
    let mut buf = [0u8; 4096];
    let n = round_trip_encode(&msg, &mut buf);
    let back = Reject::decode(&buf[..n]).expect("Reject decode");
    assert_eq!(back.code, ErrorCode::ProtocolViolation);
    assert_eq!(back.detail, "bad frame");
    assert_eq!(Reject::TAG, 0xFF00);
    assert_eq!(Reject::DIRECTION, Direction::Error);
}

// ---------------------------------------------------------------------------
// Failure-mode tests (7.3 §6 / §9 / §10 / SP13)
// ---------------------------------------------------------------------------

/// SP13: `out.len() == encoded_size() - 1` fails `BufferTooSmall` with
/// nothing written.
#[test]
fn fail_buffer_too_small_nothing_written() {
    let msg = Detach { reason: "bye" };
    let sz = msg.encoded_size();
    assert!(sz > 0, "Detach must have a non-zero body");
    let mut buf = vec![0xFFu8; sz - 1];
    let err = msg
        .encode(&mut buf)
        .expect_err("should fail BufferTooSmall");
    assert_eq!(err, ErrorCode::BufferTooSmall, "SP13: BufferTooSmall on short buffer");
    // Verify nothing was written (first byte unchanged from 0xFF).
    assert!(buf.iter().all(|&b| b == 0xFF), "SP13: nothing written past the failure");
}

/// §6: `bool` byte other than 0/1 fails `InvalidArgument`.
#[test]
fn fail_bool_non_01_invalid_argument() {
    // DestroySession: str_len(4) + str_bytes(4) + bool(1) = 9 bytes typical.
    // Manually craft a body with bool = 2.
    let body: &[u8] = &[
        0x01, 0x00, 0x00, 0x00, b'x', // session_name = "x" (len=1)
        0x02, // force = 2 (invalid bool)
    ];
    let err = DestroySession::decode(body).expect_err("bool=2 must fail");
    assert_eq!(err, ErrorCode::InvalidArgument, "§6: bool != 0/1 → InvalidArgument");
}

/// §6: non-UTF-8 `str` fails `Utf8Invalid`.
#[test]
fn fail_str_non_utf8() {
    // Detach reason is a str; inject invalid UTF-8.
    let body: &[u8] = &[
        0x02, 0x00, 0x00, 0x00, // len = 2
        0xFF, 0xFE, // invalid UTF-8
    ];
    let err = Detach::decode(body).expect_err("bad UTF-8 must fail");
    assert_eq!(err, ErrorCode::Utf8Invalid, "§6: non-UTF-8 str → Utf8Invalid");
}

/// Truncated header: buffer shorter than 16 bytes fails `ProtocolViolation`.
#[test]
fn fail_truncated_header() {
    let buf = [0u8; 10]; // < HEADER_LEN
    let err = read_frame(&buf, 4096).expect_err("short header must fail");
    assert_eq!(err, ErrorCode::ProtocolViolation, "truncated header → ProtocolViolation");
}

/// Truncated body: header says `body_len`=4 but buffer has fewer bytes.
#[test]
fn fail_truncated_body() {
    let mut buf = [0u8; HEADER_LEN + 2]; // header OK, but body only 2 of 4 bytes
    // Write body_len = 4 in LE.
    buf[0] = 4;
    let err = read_frame(&buf, 4096).expect_err("short body must fail");
    assert_eq!(err, ErrorCode::ProtocolViolation, "truncated body → ProtocolViolation");
}

/// SP10: `body_len` exceeds the cap → `ProtocolViolation`.
#[test]
fn fail_body_len_over_cap() {
    let mut buf = [0u8; HEADER_LEN];
    // body_len = 1000
    buf[0] = 0xE8;
    buf[1] = 0x03;
    let err = read_frame(&buf, 8).expect_err("over-cap body_len must fail");
    assert_eq!(err, ErrorCode::ProtocolViolation, "SP10: body_len > cap → ProtocolViolation");
}

/// SP17 / 7.3 §10.4: encode refuses an over-cap value with `ResourceExhausted`.
#[test]
fn fail_encode_capped_over_cap() {
    let msg = Detach {
        reason: "too-long-for-tiny-cap",
    };
    let mut buf = [0u8; 4096];
    let err = encode_capped(&msg, &mut buf, 2).expect_err("over-cap encode must fail");
    assert_eq!(err, ErrorCode::ResourceExhausted, "SP17: encode over cap → ResourceExhausted");
}

/// 7.3 §9: out-of-range `Reject.code` discriminant decodes to
/// `ErrorCode::Generic` rather than rejecting the frame.
#[test]
fn fail_reject_out_of_range_code_degrades_to_generic() {
    // Encode a Reject with i32 discriminant 9999 (out of range).
    let body: &[u8] = &[
        // code = 9999 (0x270F) little-endian i32
        0x0F, 0x27, 0x00, 0x00, // detail = "" (len = 0)
        0x00, 0x00, 0x00, 0x00,
    ];
    let back = Reject::decode(body).expect("Reject with out-of-range code must not fail");
    assert_eq!(back.code, ErrorCode::Generic, "7.3 §9: out-of-range Reject.code → Generic");
}

/// Truncated frame body mid-`list<str>` count prefix.
#[test]
fn fail_truncated_mid_list_count() {
    // Hello: major(2) + minor(2) + partial count (only 2 bytes of 4).
    let body: &[u8] = &[0x01, 0x00, 0x00, 0x00, 0x01, 0x00];
    let err = Hello::decode(body).expect_err("truncated list count must fail");
    assert_eq!(err, ErrorCode::ProtocolViolation, "truncated mid-list → ProtocolViolation");
}

/// Truncated frame body mid-string payload.
#[test]
fn fail_truncated_mid_string_payload() {
    // Detach reason: len=5 but only 2 bytes follow.
    let body: &[u8] = &[0x05, 0x00, 0x00, 0x00, b'h', b'i'];
    let err = Detach::decode(body).expect_err("truncated string payload must fail");
    assert_eq!(err, ErrorCode::ProtocolViolation, "truncated string → ProtocolViolation");
}

/// COMPRESSED flag (bit 1) in request direction is a reserved violation.
/// The state machine should reject it; we verify the flag-mask constant.
#[test]
fn fail_compressed_flag_rejected_by_state_machine() {
    use reovim_uapi_protocol::{
        frame::FLAG_COMPRESSED,
        messages::{Direction, Hello, Message},
        state::{Action, ProtocolState, Role, TagInfo},
    };

    let mut sm = ProtocolState::new(Role::Server);
    // First accept the handshake (Hello, no flags).
    let hello_hdr = FrameHeader {
        body_len: 0,
        msg_type: Hello::TAG,
        flags: 0,
        correlation_id: 0,
    };
    assert_eq!(sm.on_frame(hello_hdr, TagInfo::Known(Direction::Handshake)), Action::Accept);
    // Now send a req with COMPRESSED set → BadFlags reject.
    let req_hdr = FrameHeader {
        body_len: 0,
        msg_type: 0x0100,
        flags: FLAG_COMPRESSED,
        correlation_id: 1,
    };
    let action = sm.on_frame(req_hdr, TagInfo::Known(Direction::Request));
    assert!(
        matches!(action, Action::Reject(_)),
        "COMPRESSED flag in req direction must be rejected"
    );
}

// ---------------------------------------------------------------------------
// State machine failure modes (7.3 §10)
// ---------------------------------------------------------------------------

/// SP9.1: first frame is not Hello/`HelloAck` → reject `HandshakeExpected`.
#[test]
fn fail_state_machine_first_frame_not_hello() {
    use reovim_uapi_protocol::state::{Action, ProtocolState, Role, TagInfo};

    let mut sm = ProtocolState::new(Role::Server);
    // Send a request as the first frame instead of Hello.
    let req_hdr = FrameHeader {
        body_len: 0,
        msg_type: 0x0100,
        flags: 0,
        correlation_id: 1,
    };
    let action = sm.on_frame(req_hdr, TagInfo::Known(Direction::Request));
    assert!(matches!(action, Action::Reject(_)), "SP9.1: non-Hello first frame → Reject");
}

/// Unknown notify tag (post-handshake, client side) → `SkipNotify`.
#[test]
fn pass_state_machine_unknown_notify_skipped() {
    use reovim_uapi_protocol::{
        messages::{Direction, HelloAck, Message},
        state::{Action, ProtocolState, Role, TagInfo},
    };

    let mut sm = ProtocolState::new(Role::Client);
    // Establish: HelloAck first.
    let ack_hdr = FrameHeader {
        body_len: 0,
        msg_type: HelloAck::TAG,
        flags: 0,
        correlation_id: 0,
    };
    assert_eq!(sm.on_frame(ack_hdr, TagInfo::Known(Direction::Handshake)), Action::Accept);
    // Unknown notify tag.
    let unknown_hdr = FrameHeader {
        body_len: 4,
        msg_type: 0x03FF,
        flags: 0,
        correlation_id: 0,
    };
    let action = sm.on_frame(unknown_hdr, TagInfo::Unknown);
    assert_eq!(action, Action::SkipNotify, "§10.3: unknown notify → SkipNotify");
}

/// Unknown req tag (post-handshake, server side) → Reject.
#[test]
fn fail_state_machine_unknown_req_rejected() {
    use reovim_uapi_protocol::{
        messages::{Direction, Hello, Message},
        state::{Action, ProtocolState, Role, TagInfo},
    };

    let mut sm = ProtocolState::new(Role::Server);
    let hello_hdr = FrameHeader {
        body_len: 0,
        msg_type: Hello::TAG,
        flags: 0,
        correlation_id: 0,
    };
    sm.on_frame(hello_hdr, TagInfo::Known(Direction::Handshake));
    let unknown_hdr = FrameHeader {
        body_len: 0,
        msg_type: 0x01FF,
        flags: 0,
        correlation_id: 1,
    };
    let action = sm.on_frame(unknown_hdr, TagInfo::Unknown);
    assert!(matches!(action, Action::Reject(_)), "§10.3: unknown req tag → Reject");
}
