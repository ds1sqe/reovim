//! Codec golden tests migrated to the no_std selftest runner (#786 Phase 5).
//!
//! Mirrors `uapi/protocol/tests/codec_goldens.rs`.  All std conveniences
//! (`Vec`, `format!`) are replaced with fixed stack buffers and
//! `testrt::check`/`check_eq`.
//!
//! The libtest originals in `uapi/protocol/tests/codec_goldens.rs` remain
//! as the bootstrap-state-1 mirror; these selftest registrations are the
//! flight-environment authority.
//!
//! Spec citations are preserved verbatim from the originals.

use reovim_arch::arch_test;
use reovim_uapi_abi::{ErrorCode, FrameHeader};
use reovim_uapi_protocol::{
    frame::{HEADER_LEN, read_frame, write_frame},
    messages::{
        Attach,
        AttachAck,
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
        Direction,
        Hello,
        HelloAck,
        InputAck,
        Message,
        PkgSync,
        PkgSyncDone,
        PkgSyncProgress,
        PkgVerify,
        PkgVerifyAck,
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
// Helper: encode→decode round-trip (no_std; uses a caller stack buffer)
// ---------------------------------------------------------------------------

/// Encodes `msg` into a 4 KiB stack buffer, asserts `encode` returned the
/// same length as `encoded_size`, re-encodes into a second buffer and checks
/// byte-for-byte determinism.  Returns the encoded byte count.
fn round_trip_encode<M: Message>(msg: &M, buf: &mut [u8; 4096]) -> usize {
    let n = msg
        .encode(buf)
        .expect("encode must succeed for a well-formed message");
    reovim_arch::testrt::check_eq(n, msg.encoded_size());
    // Determinism check.
    let mut buf2 = [0u8; 4096];
    let n2 = msg.encode(&mut buf2).expect("second encode must succeed");
    reovim_arch::testrt::check_eq(n, n2);
    reovim_arch::testrt::check(&buf[..n] == &buf2[..n2], "encode is deterministic");
    n
}

// ---------------------------------------------------------------------------
// 7.3 §"worked frame" — 57-byte Hello golden (permanent SP13 golden, CF4
// successor)
// ---------------------------------------------------------------------------

arch_test!(hello_57_byte_frame_golden, {
    // Build the Hello caps/codecs list buffers (no alloc: pre-encode into
    // fixed-size scratch buffers).
    let caps_buf = {
        let mut b = [0u8; 32];
        let mut enc = reovim_uapi_protocol::codec::Encoder::new(&mut b);
        enc.put_u32(1).unwrap();
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
    reovim_arch::testrt::check_eq(body_len, 41);

    // Expected body bytes (7.3 §"worked frame"):
    #[rustfmt::skip]
    let expected_body: [u8; 41] = [
        0x01, 0x00,
        0x00, 0x00,
        0x01, 0x00, 0x00, 0x00,
        0x0C, 0x00, 0x00, 0x00,
        0x72, 0x65, 0x6E, 0x64, 0x65, 0x72, 0x2E, 0x63, 0x65, 0x6C, 0x6C, 0x73,
        0x01, 0x00, 0x00, 0x00,
        0x09, 0x00, 0x00, 0x00,
        0x74, 0x65, 0x78, 0x74, 0x2E, 0x75, 0x74, 0x66, 0x38,
    ];
    reovim_arch::testrt::check(
        &body_buf[..41] == &expected_body,
        "7.3 worked-frame: Hello body bytes",
    );

    // Write the full 57-byte frame.
    let hdr = FrameHeader {
        body_len: 0,
        msg_type: Hello::TAG,
        flags: 0,
        correlation_id: 0,
    };
    let mut frame_buf = [0u8; 256];
    let total = write_frame(&mut frame_buf, hdr, &body_buf[..41], 4096).expect("write_frame");
    reovim_arch::testrt::check_eq(total, 57);

    // Expected full 57-byte golden.
    #[rustfmt::skip]
    let expected_frame: [u8; 57] = [
        0x29, 0x00, 0x00, 0x00,
        0x01, 0x00,
        0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x01, 0x00,
        0x00, 0x00,
        0x01, 0x00, 0x00, 0x00,
        0x0C, 0x00, 0x00, 0x00,
        0x72, 0x65, 0x6E, 0x64, 0x65, 0x72, 0x2E, 0x63, 0x65, 0x6C, 0x6C, 0x73,
        0x01, 0x00, 0x00, 0x00,
        0x09, 0x00, 0x00, 0x00,
        0x74, 0x65, 0x78, 0x74, 0x2E, 0x75, 0x74, 0x66, 0x38,
    ];
    reovim_arch::testrt::check(
        &frame_buf[..57] == &expected_frame,
        "7.3 worked-frame: 57-byte golden (SP13)",
    );

    // Round-trip: read_frame + Hello::decode must recover original fields.
    let fv = read_frame(&frame_buf[..57], 4096).expect("read_frame");
    reovim_arch::testrt::check_eq(fv.header.body_len, 41);
    reovim_arch::testrt::check_eq(fv.header.msg_type, Hello::TAG);
    reovim_arch::testrt::check_eq(fv.header.flags, 0);
    reovim_arch::testrt::check_eq(fv.header.correlation_id, 0);
    let back = Hello::decode(fv.body).expect("Hello::decode");
    reovim_arch::testrt::check_eq(back.protocol_major, 1);
    reovim_arch::testrt::check_eq(back.protocol_minor, 0);

    // Collect caps to a fixed array instead of Vec (no_std).
    let mut caps_arr = [""; 8];
    let mut caps_len = 0usize;
    for s in back.caps.iter() {
        caps_arr[caps_len] = s;
        caps_len += 1;
    }
    reovim_arch::testrt::check_eq(caps_len, 1);
    reovim_arch::testrt::check(caps_arr[0] == "render.cells", "Hello.caps[0]");

    let mut codecs_arr = [""; 8];
    let mut codecs_len = 0usize;
    for s in back.domain_codecs.iter() {
        codecs_arr[codecs_len] = s;
        codecs_len += 1;
    }
    reovim_arch::testrt::check_eq(codecs_len, 1);
    reovim_arch::testrt::check(codecs_arr[0] == "text.utf8", "Hello.domain_codecs[0]");
});

// ---------------------------------------------------------------------------
// Round-trip goldens for all 37 §7 message types
// ---------------------------------------------------------------------------

// ── Handshake ──────────────────────────────────────────────────────────────

arch_test!(round_trip_hello, {
    let empty = [0u8; 4];
    let msg = Hello {
        protocol_major: 2,
        protocol_minor: 1,
        caps: StrList::from_validated(&empty),
        domain_codecs: StrList::from_validated(&empty),
    };
    let mut buf = [0u8; 4096];
    let n = round_trip_encode(&msg, &mut buf);
    let back = Hello::decode(&buf[..n]).expect("Hello decode");
    reovim_arch::testrt::check_eq((back.protocol_major, back.protocol_minor), (2, 1));
    reovim_arch::testrt::check_eq(Hello::TAG, 0x0001_u16);
    reovim_arch::testrt::check_eq(Hello::DIRECTION, Direction::Handshake);
});

arch_test!(round_trip_hello_ack, {
    let empty = [0u8; 4];
    let msg = HelloAck {
        protocol_major: 1,
        protocol_minor: 0,
        granted_caps: StrList::from_validated(&empty),
        server_name: "reovim-test",
    };
    let mut buf = [0u8; 4096];
    let n = round_trip_encode(&msg, &mut buf);
    let back = HelloAck::decode(&buf[..n]).expect("HelloAck decode");
    reovim_arch::testrt::check(back.server_name == "reovim-test", "HelloAck.server_name");
    reovim_arch::testrt::check_eq(HelloAck::TAG, 0x0002_u16);
    reovim_arch::testrt::check_eq(HelloAck::DIRECTION, Direction::Handshake);
});

// ── Requests ────────────────────────────────────────────────────────────────

arch_test!(round_trip_attach, {
    let empty = [0u8; 4];
    let msg = Attach {
        session_name: "main",
        auth: b"secret",
        caps: StrList::from_validated(&empty),
        domain_codecs: StrList::from_validated(&empty),
    };
    let mut buf = [0u8; 4096];
    let n = round_trip_encode(&msg, &mut buf);
    let back = Attach::decode(&buf[..n]).expect("Attach decode");
    reovim_arch::testrt::check(back.session_name == "main", "Attach.session_name");
    reovim_arch::testrt::check(back.auth == b"secret", "Attach.auth");
    reovim_arch::testrt::check_eq(Attach::TAG, 0x0100_u16);
    reovim_arch::testrt::check_eq(Attach::DIRECTION, Direction::Request);
});

arch_test!(round_trip_detach, {
    let msg = Detach { reason: "test-done" };
    let mut buf = [0u8; 4096];
    let n = round_trip_encode(&msg, &mut buf);
    let back = Detach::decode(&buf[..n]).expect("Detach decode");
    reovim_arch::testrt::check(back.reason == "test-done", "Detach.reason");
    reovim_arch::testrt::check_eq(Detach::TAG, 0x0101_u16);
    reovim_arch::testrt::check_eq(Detach::DIRECTION, Direction::Request);
});

arch_test!(round_trip_send_input_empty, {
    let empty = [0u8; 4];
    let msg = SendInput {
        client_id: 3,
        buffer_id: 7,
        window_id: 42,
        inputs: RawInputList::from_validated(&empty),
    };
    let mut buf = [0u8; 4096];
    let n = round_trip_encode(&msg, &mut buf);
    let back = SendInput::decode(&buf[..n]).expect("SendInput decode");
    reovim_arch::testrt::check_eq((back.client_id, back.buffer_id, back.window_id), (3, 7, 42));
    reovim_arch::testrt::check(back.inputs.is_empty(), "SendInput.inputs empty");
    reovim_arch::testrt::check_eq(SendInput::TAG, 0x0102_u16);
    reovim_arch::testrt::check_eq(SendInput::DIRECTION, Direction::Request);
});

arch_test!(round_trip_switch_session, {
    let msg = SwitchSession { session_name: "other" };
    let mut buf = [0u8; 4096];
    let n = round_trip_encode(&msg, &mut buf);
    let back = SwitchSession::decode(&buf[..n]).expect("SwitchSession decode");
    reovim_arch::testrt::check(back.session_name == "other", "SwitchSession.session_name");
    reovim_arch::testrt::check_eq(SwitchSession::TAG, 0x0103_u16);
    reovim_arch::testrt::check_eq(SwitchSession::DIRECTION, Direction::Request);
});

arch_test!(round_trip_destroy_session, {
    let msg = DestroySession { session_name: "old", force: true };
    let mut buf = [0u8; 4096];
    let n = round_trip_encode(&msg, &mut buf);
    let back = DestroySession::decode(&buf[..n]).expect("DestroySession decode");
    reovim_arch::testrt::check(back.session_name == "old", "DestroySession.session_name");
    reovim_arch::testrt::check(back.force, "DestroySession.force");
    reovim_arch::testrt::check_eq(DestroySession::TAG, 0x0104_u16);
    reovim_arch::testrt::check_eq(DestroySession::DIRECTION, Direction::Request);
});

arch_test!(round_trip_rename_session, {
    let msg = RenameSession { old_name: "a", new_name: "b" };
    let mut buf = [0u8; 4096];
    let n = round_trip_encode(&msg, &mut buf);
    let back = RenameSession::decode(&buf[..n]).expect("RenameSession decode");
    reovim_arch::testrt::check((back.old_name, back.new_name) == ("a", "b"), "RenameSession names");
    reovim_arch::testrt::check_eq(RenameSession::TAG, 0x0105_u16);
    reovim_arch::testrt::check_eq(RenameSession::DIRECTION, Direction::Request);
});

arch_test!(round_trip_debug_read, {
    let msg = DebugRead { op_kind: 99, payload: b"probe", subscribe: true };
    let mut buf = [0u8; 4096];
    let n = round_trip_encode(&msg, &mut buf);
    let back = DebugRead::decode(&buf[..n]).expect("DebugRead decode");
    reovim_arch::testrt::check_eq(back.op_kind, 99);
    reovim_arch::testrt::check(back.payload == b"probe", "DebugRead.payload");
    reovim_arch::testrt::check(back.subscribe, "DebugRead.subscribe");
    reovim_arch::testrt::check_eq(DebugRead::TAG, 0x0106_u16);
    reovim_arch::testrt::check_eq(DebugRead::DIRECTION, Direction::Request);
});

arch_test!(round_trip_debug_drive, {
    let msg = DebugDrive { op_kind: 5, payload: b"cmd" };
    let mut buf = [0u8; 4096];
    let n = round_trip_encode(&msg, &mut buf);
    let back = DebugDrive::decode(&buf[..n]).expect("DebugDrive decode");
    reovim_arch::testrt::check_eq(back.op_kind, 5);
    reovim_arch::testrt::check(back.payload == b"cmd", "DebugDrive.payload");
    reovim_arch::testrt::check_eq(DebugDrive::TAG, 0x0107_u16);
    reovim_arch::testrt::check_eq(DebugDrive::DIRECTION, Direction::Request);
});

arch_test!(round_trip_pkg_sync, {
    let empty = [0u8; 4];
    let msg = PkgSync { targets: StrList::from_validated(&empty), dry_run: false };
    let mut buf = [0u8; 4096];
    let n = round_trip_encode(&msg, &mut buf);
    let back = PkgSync::decode(&buf[..n]).expect("PkgSync decode");
    reovim_arch::testrt::check(!back.dry_run, "PkgSync.dry_run false");
    reovim_arch::testrt::check(back.targets.is_empty(), "PkgSync.targets empty");
    reovim_arch::testrt::check_eq(PkgSync::TAG, 0x0108_u16);
    reovim_arch::testrt::check_eq(PkgSync::DIRECTION, Direction::Request);
});

arch_test!(round_trip_pkg_verify, {
    let empty = [0u8; 4];
    let msg = PkgVerify { targets: StrList::from_validated(&empty) };
    let mut buf = [0u8; 4096];
    let n = round_trip_encode(&msg, &mut buf);
    let back = PkgVerify::decode(&buf[..n]).expect("PkgVerify decode");
    reovim_arch::testrt::check(back.targets.is_empty(), "PkgVerify.targets empty");
    reovim_arch::testrt::check_eq(PkgVerify::TAG, 0x0109_u16);
    reovim_arch::testrt::check_eq(PkgVerify::DIRECTION, Direction::Request);
});

arch_test!(round_trip_config_dump, {
    let msg = ConfigDump { namespace: "ui", show_secrets: false };
    let mut buf = [0u8; 4096];
    let n = round_trip_encode(&msg, &mut buf);
    let back = ConfigDump::decode(&buf[..n]).expect("ConfigDump decode");
    reovim_arch::testrt::check(back.namespace == "ui", "ConfigDump.namespace");
    reovim_arch::testrt::check(!back.show_secrets, "ConfigDump.show_secrets false");
    reovim_arch::testrt::check_eq(ConfigDump::TAG, 0x010A_u16);
    reovim_arch::testrt::check_eq(ConfigDump::DIRECTION, Direction::Request);
});

arch_test!(round_trip_config_validate, {
    let msg = ConfigValidate { toml: b"[x]\ny=1", namespace: "mod" };
    let mut buf = [0u8; 4096];
    let n = round_trip_encode(&msg, &mut buf);
    let back = ConfigValidate::decode(&buf[..n]).expect("ConfigValidate decode");
    reovim_arch::testrt::check(back.toml == b"[x]\ny=1", "ConfigValidate.toml");
    reovim_arch::testrt::check(back.namespace == "mod", "ConfigValidate.namespace");
    reovim_arch::testrt::check_eq(ConfigValidate::TAG, 0x010B_u16);
    reovim_arch::testrt::check_eq(ConfigValidate::DIRECTION, Direction::Request);
});

// ── Responses ───────────────────────────────────────────────────────────────

arch_test!(round_trip_attach_ack, {
    let empty = [0u8; 4];
    let msg = AttachAck {
        client_id: 11,
        domain_table: DomainEntryList::from_validated(&empty),
    };
    let mut buf = [0u8; 4096];
    let n = round_trip_encode(&msg, &mut buf);
    let back = AttachAck::decode(&buf[..n]).expect("AttachAck decode");
    reovim_arch::testrt::check_eq(back.client_id, 11);
    reovim_arch::testrt::check(back.domain_table.is_empty(), "AttachAck.domain_table empty");
    reovim_arch::testrt::check_eq(AttachAck::TAG, 0x0200_u16);
    reovim_arch::testrt::check_eq(AttachAck::DIRECTION, Direction::Response);
});

arch_test!(round_trip_detach_ack, {
    let msg = DetachAck;
    reovim_arch::testrt::check_eq(msg.encoded_size(), 0);
    let mut buf = [0u8; 4096];
    let n = msg.encode(&mut buf).expect("DetachAck encode");
    reovim_arch::testrt::check_eq(n, 0);
    let _ = DetachAck::decode(&buf[..n]).expect("DetachAck decode");
    reovim_arch::testrt::check_eq(DetachAck::TAG, 0x0201_u16);
    reovim_arch::testrt::check_eq(DetachAck::DIRECTION, Direction::Response);
});

arch_test!(round_trip_input_ack, {
    let msg = InputAck { accepted: 1000, dropped: 5 };
    let mut buf = [0u8; 4096];
    let n = round_trip_encode(&msg, &mut buf);
    let back = InputAck::decode(&buf[..n]).expect("InputAck decode");
    reovim_arch::testrt::check_eq((back.accepted, back.dropped), (1000, 5));
    reovim_arch::testrt::check_eq(InputAck::TAG, 0x0202_u16);
    reovim_arch::testrt::check_eq(InputAck::DIRECTION, Direction::Response);
});

arch_test!(round_trip_switch_session_ack, {
    let msg = SwitchSessionAck;
    reovim_arch::testrt::check_eq(msg.encoded_size(), 0);
    let mut buf = [0u8; 4096];
    let n = msg.encode(&mut buf).expect("SwitchSessionAck encode");
    reovim_arch::testrt::check_eq(n, 0);
    reovim_arch::testrt::check_eq(SwitchSessionAck::TAG, 0x0203_u16);
    reovim_arch::testrt::check_eq(SwitchSessionAck::DIRECTION, Direction::Response);
});

arch_test!(round_trip_destroy_session_ack, {
    let msg = DestroySessionAck;
    reovim_arch::testrt::check_eq(msg.encoded_size(), 0);
    let mut buf = [0u8; 4096];
    let n = msg.encode(&mut buf).expect("DestroySessionAck encode");
    reovim_arch::testrt::check_eq(n, 0);
    reovim_arch::testrt::check_eq(DestroySessionAck::TAG, 0x0204_u16);
    reovim_arch::testrt::check_eq(DestroySessionAck::DIRECTION, Direction::Response);
});

arch_test!(round_trip_rename_session_ack, {
    let msg = RenameSessionAck;
    reovim_arch::testrt::check_eq(msg.encoded_size(), 0);
    let mut buf = [0u8; 4096];
    let n = msg.encode(&mut buf).expect("RenameSessionAck encode");
    reovim_arch::testrt::check_eq(n, 0);
    reovim_arch::testrt::check_eq(RenameSessionAck::TAG, 0x0205_u16);
    reovim_arch::testrt::check_eq(RenameSessionAck::DIRECTION, Direction::Response);
});

arch_test!(round_trip_debug_read_event, {
    let msg = DebugReadEvent { op_kind: 3, payload: b"data", is_final: true };
    let mut buf = [0u8; 4096];
    let n = round_trip_encode(&msg, &mut buf);
    let back = DebugReadEvent::decode(&buf[..n]).expect("DebugReadEvent decode");
    reovim_arch::testrt::check_eq(back.op_kind, 3);
    reovim_arch::testrt::check(back.payload == b"data", "DebugReadEvent.payload");
    reovim_arch::testrt::check(back.is_final, "DebugReadEvent.is_final");
    reovim_arch::testrt::check_eq(DebugReadEvent::TAG, 0x0206_u16);
    reovim_arch::testrt::check_eq(DebugReadEvent::DIRECTION, Direction::Response);
});

arch_test!(round_trip_debug_drive_ack, {
    let msg = DebugDriveAck { op_kind: 4, result: ErrorCode::Ok, payload: b"ok" };
    let mut buf = [0u8; 4096];
    let n = round_trip_encode(&msg, &mut buf);
    let back = DebugDriveAck::decode(&buf[..n]).expect("DebugDriveAck decode");
    reovim_arch::testrt::check_eq(back.op_kind, 4);
    reovim_arch::testrt::check_eq(back.result, ErrorCode::Ok);
    reovim_arch::testrt::check(back.payload == b"ok", "DebugDriveAck.payload");
    reovim_arch::testrt::check_eq(DebugDriveAck::TAG, 0x0207_u16);
    reovim_arch::testrt::check_eq(DebugDriveAck::DIRECTION, Direction::Response);
});

arch_test!(round_trip_pkg_sync_progress, {
    let msg = PkgSyncProgress { target: "mod-vim", done: 3, total: 10 };
    let mut buf = [0u8; 4096];
    let n = round_trip_encode(&msg, &mut buf);
    let back = PkgSyncProgress::decode(&buf[..n]).expect("PkgSyncProgress decode");
    reovim_arch::testrt::check((back.target, back.done, back.total) == ("mod-vim", 3, 10), "PkgSyncProgress fields");
    reovim_arch::testrt::check_eq(PkgSyncProgress::TAG, 0x0208_u16);
    reovim_arch::testrt::check_eq(PkgSyncProgress::DIRECTION, Direction::Response);
});

arch_test!(round_trip_pkg_sync_done, {
    let msg = PkgSyncDone { result: ErrorCode::Ok, summary: "done" };
    let mut buf = [0u8; 4096];
    let n = round_trip_encode(&msg, &mut buf);
    let back = PkgSyncDone::decode(&buf[..n]).expect("PkgSyncDone decode");
    reovim_arch::testrt::check_eq(back.result, ErrorCode::Ok);
    reovim_arch::testrt::check(back.summary == "done", "PkgSyncDone.summary");
    reovim_arch::testrt::check_eq(PkgSyncDone::TAG, 0x0209_u16);
    reovim_arch::testrt::check_eq(PkgSyncDone::DIRECTION, Direction::Response);
});

arch_test!(round_trip_pkg_verify_ack, {
    let msg = PkgVerifyAck { result: ErrorCode::NotFound, report: b"missing" };
    let mut buf = [0u8; 4096];
    let n = round_trip_encode(&msg, &mut buf);
    let back = PkgVerifyAck::decode(&buf[..n]).expect("PkgVerifyAck decode");
    reovim_arch::testrt::check_eq(back.result, ErrorCode::NotFound);
    reovim_arch::testrt::check(back.report == b"missing", "PkgVerifyAck.report");
    reovim_arch::testrt::check_eq(PkgVerifyAck::TAG, 0x020A_u16);
    reovim_arch::testrt::check_eq(PkgVerifyAck::DIRECTION, Direction::Response);
});

arch_test!(round_trip_config_dump_response, {
    let msg = ConfigDumpResponse { toml: b"[section]\nkey = 1" };
    let mut buf = [0u8; 4096];
    let n = round_trip_encode(&msg, &mut buf);
    let back = ConfigDumpResponse::decode(&buf[..n]).expect("ConfigDumpResponse decode");
    reovim_arch::testrt::check(back.toml == b"[section]\nkey = 1", "ConfigDumpResponse.toml");
    reovim_arch::testrt::check_eq(ConfigDumpResponse::TAG, 0x020B_u16);
    reovim_arch::testrt::check_eq(ConfigDumpResponse::DIRECTION, Direction::Response);
});

arch_test!(round_trip_config_validate_response, {
    let msg = ConfigValidateResponse { result: ErrorCode::SchemaInvalid, detail: "bad key" };
    let mut buf = [0u8; 4096];
    let n = round_trip_encode(&msg, &mut buf);
    let back = ConfigValidateResponse::decode(&buf[..n]).expect("ConfigValidateResponse decode");
    reovim_arch::testrt::check_eq(back.result, ErrorCode::SchemaInvalid);
    reovim_arch::testrt::check(back.detail == "bad key", "ConfigValidateResponse.detail");
    reovim_arch::testrt::check_eq(ConfigValidateResponse::TAG, 0x020C_u16);
    reovim_arch::testrt::check_eq(ConfigValidateResponse::DIRECTION, Direction::Response);
});

// ── Notifications ────────────────────────────────────────────────────────────

arch_test!(round_trip_attach_event_frame, {
    let msg = AttachEventFrame { buffer_id: 1, window_id: 2, frame: b"pixels" };
    let mut buf = [0u8; 4096];
    let n = round_trip_encode(&msg, &mut buf);
    let back = AttachEventFrame::decode(&buf[..n]).expect("AttachEventFrame decode");
    reovim_arch::testrt::check_eq((back.buffer_id, back.window_id), (1, 2));
    reovim_arch::testrt::check(back.frame == b"pixels", "AttachEventFrame.frame");
    reovim_arch::testrt::check_eq(AttachEventFrame::TAG, 0x0301_u16);
    reovim_arch::testrt::check_eq(AttachEventFrame::DIRECTION, Direction::Notify);
});

arch_test!(round_trip_attach_event_diff, {
    let msg = AttachEventDiff { buffer_id: 5, window_id: 6, diff: b"delta" };
    let mut buf = [0u8; 4096];
    let n = round_trip_encode(&msg, &mut buf);
    let back = AttachEventDiff::decode(&buf[..n]).expect("AttachEventDiff decode");
    reovim_arch::testrt::check_eq((back.buffer_id, back.window_id), (5, 6));
    reovim_arch::testrt::check(back.diff == b"delta", "AttachEventDiff.diff");
    reovim_arch::testrt::check_eq(AttachEventDiff::TAG, 0x0302_u16);
    reovim_arch::testrt::check_eq(AttachEventDiff::DIRECTION, Direction::Notify);
});

arch_test!(round_trip_attach_event_cursor_empty, {
    let empty = [0u8; 4];
    let msg = AttachEventCursor {
        buffer_id: 0,
        window_id: 0,
        cursors: CarrierList::from_validated(&empty),
    };
    let mut buf = [0u8; 4096];
    let n = round_trip_encode(&msg, &mut buf);
    let back = AttachEventCursor::decode(&buf[..n]).expect("AttachEventCursor decode");
    reovim_arch::testrt::check(back.cursors.is_empty(), "CR3: empty cursor set");
    reovim_arch::testrt::check_eq(AttachEventCursor::TAG, 0x0303_u16);
    reovim_arch::testrt::check_eq(AttachEventCursor::DIRECTION, Direction::Notify);
});

arch_test!(round_trip_attach_event_projection, {
    let msg = AttachEventProjection { buffer_id: 8, projection: b"proj-bytes" };
    let mut buf = [0u8; 4096];
    let n = round_trip_encode(&msg, &mut buf);
    let back = AttachEventProjection::decode(&buf[..n]).expect("AttachEventProjection decode");
    reovim_arch::testrt::check_eq(back.buffer_id, 8);
    reovim_arch::testrt::check(back.projection == b"proj-bytes", "AttachEventProjection.projection");
    reovim_arch::testrt::check_eq(AttachEventProjection::TAG, 0x0304_u16);
    reovim_arch::testrt::check_eq(AttachEventProjection::DIRECTION, Direction::Notify);
});

arch_test!(round_trip_attach_event_domain_table_delta, {
    let empty = [0u8; 4];
    let msg = AttachEventDomainTableDelta {
        added: DomainEntryList::from_validated(&empty),
        removed: DomainEntryList::from_validated(&empty),
    };
    let mut buf = [0u8; 4096];
    let n = round_trip_encode(&msg, &mut buf);
    let back = AttachEventDomainTableDelta::decode(&buf[..n])
        .expect("AttachEventDomainTableDelta decode");
    reovim_arch::testrt::check(
        back.added.is_empty() && back.removed.is_empty(),
        "AttachEventDomainTableDelta empty lists",
    );
    reovim_arch::testrt::check_eq(AttachEventDomainTableDelta::TAG, 0x0305_u16);
    reovim_arch::testrt::check_eq(AttachEventDomainTableDelta::DIRECTION, Direction::Notify);
});

arch_test!(round_trip_attach_event_session_pivot, {
    let msg = AttachEventSessionPivot { session_name: "pivot" };
    let mut buf = [0u8; 4096];
    let n = round_trip_encode(&msg, &mut buf);
    let back = AttachEventSessionPivot::decode(&buf[..n]).expect("AttachEventSessionPivot decode");
    reovim_arch::testrt::check(back.session_name == "pivot", "AttachEventSessionPivot.session_name");
    reovim_arch::testrt::check_eq(AttachEventSessionPivot::TAG, 0x0306_u16);
    reovim_arch::testrt::check_eq(AttachEventSessionPivot::DIRECTION, Direction::Notify);
});

arch_test!(round_trip_attach_event_session_destroyed, {
    let msg = AttachEventSessionDestroyed { session_name: "gone" };
    let mut buf = [0u8; 4096];
    let n = round_trip_encode(&msg, &mut buf);
    let back = AttachEventSessionDestroyed::decode(&buf[..n])
        .expect("AttachEventSessionDestroyed decode");
    reovim_arch::testrt::check(back.session_name == "gone", "AttachEventSessionDestroyed.session_name");
    reovim_arch::testrt::check_eq(AttachEventSessionDestroyed::TAG, 0x0307_u16);
    reovim_arch::testrt::check_eq(AttachEventSessionDestroyed::DIRECTION, Direction::Notify);
});

arch_test!(round_trip_attach_event_client_left, {
    let msg = AttachEventClientLeft { client_id: 99, reason: "connection-closed" };
    let mut buf = [0u8; 4096];
    let n = round_trip_encode(&msg, &mut buf);
    let back = AttachEventClientLeft::decode(&buf[..n]).expect("AttachEventClientLeft decode");
    reovim_arch::testrt::check(
        (back.client_id, back.reason) == (99, "connection-closed"),
        "AttachEventClientLeft fields",
    );
    reovim_arch::testrt::check_eq(AttachEventClientLeft::TAG, 0x0308_u16);
    reovim_arch::testrt::check_eq(AttachEventClientLeft::DIRECTION, Direction::Notify);
});

arch_test!(round_trip_attach_event_server_draining, {
    let msg = AttachEventServerDraining { grace_ms: 5000 };
    let mut buf = [0u8; 4096];
    let n = round_trip_encode(&msg, &mut buf);
    let back =
        AttachEventServerDraining::decode(&buf[..n]).expect("AttachEventServerDraining decode");
    reovim_arch::testrt::check_eq(back.grace_ms, 5000);
    reovim_arch::testrt::check_eq(AttachEventServerDraining::TAG, 0x0309_u16);
    reovim_arch::testrt::check_eq(AttachEventServerDraining::DIRECTION, Direction::Notify);
});

// ── Error ────────────────────────────────────────────────────────────────────

arch_test!(round_trip_reject, {
    let msg = Reject { code: ErrorCode::ProtocolViolation, detail: "bad frame" };
    let mut buf = [0u8; 4096];
    let n = round_trip_encode(&msg, &mut buf);
    let back = Reject::decode(&buf[..n]).expect("Reject decode");
    reovim_arch::testrt::check_eq(back.code, ErrorCode::ProtocolViolation);
    reovim_arch::testrt::check(back.detail == "bad frame", "Reject.detail");
    reovim_arch::testrt::check_eq(Reject::TAG, 0xFF00_u16);
    reovim_arch::testrt::check_eq(Reject::DIRECTION, Direction::Error);
});

// ---------------------------------------------------------------------------
// Failure-mode tests
// ---------------------------------------------------------------------------

arch_test!(fail_buffer_too_small_nothing_written, {
    // SP13: encode into a buffer exactly 1 byte too short → BufferTooSmall,
    // nothing written.  No alloc: use a fixed 64-byte array and only use
    // the first (sz - 1) bytes as the slice.
    let msg = Detach { reason: "bye" };
    let sz = msg.encoded_size();
    reovim_arch::testrt::check(sz > 0, "Detach.encoded_size > 0");
    // sz is small (≤ 64 for short reason strings); use a fixed buffer and
    // truncate.  The assertion holds for any sz ≤ 63.
    let mut buf = [0xFFu8; 64];
    let short = &mut buf[..sz - 1];
    let err = msg.encode(short).expect_err("BufferTooSmall expected");
    reovim_arch::testrt::check_eq(err, ErrorCode::BufferTooSmall);
    // Verify nothing was written (all bytes still 0xFF).
    for b in short.iter() {
        reovim_arch::testrt::check(*b == 0xFF, "SP13: nothing written past failure");
    }
});

arch_test!(fail_bool_non_01_invalid_argument, {
    let body: &[u8] = &[
        0x01, 0x00, 0x00, 0x00, b'x', // session_name = "x"
        0x02,                          // force = 2 (invalid bool)
    ];
    let err = DestroySession::decode(body).expect_err("bool=2 must fail");
    reovim_arch::testrt::check_eq(err, ErrorCode::InvalidArgument);
});

arch_test!(fail_str_non_utf8, {
    let body: &[u8] = &[
        0x02, 0x00, 0x00, 0x00, // len = 2
        0xFF, 0xFE,              // invalid UTF-8
    ];
    let err = Detach::decode(body).expect_err("bad UTF-8 must fail");
    reovim_arch::testrt::check_eq(err, ErrorCode::Utf8Invalid);
});

arch_test!(fail_truncated_header, {
    let buf = [0u8; 10]; // < HEADER_LEN = 16
    let err = read_frame(&buf, 4096).expect_err("short header must fail");
    reovim_arch::testrt::check_eq(err, ErrorCode::ProtocolViolation);
});

arch_test!(fail_truncated_body, {
    let mut buf = [0u8; HEADER_LEN + 2]; // body only 2 of 4 bytes
    buf[0] = 4; // body_len = 4 LE
    let err = read_frame(&buf, 4096).expect_err("short body must fail");
    reovim_arch::testrt::check_eq(err, ErrorCode::ProtocolViolation);
});

arch_test!(fail_body_len_over_cap, {
    let mut buf = [0u8; HEADER_LEN];
    buf[0] = 0xE8; // body_len = 1000 LE
    buf[1] = 0x03;
    let err = read_frame(&buf, 8).expect_err("over-cap body_len must fail");
    reovim_arch::testrt::check_eq(err, ErrorCode::ProtocolViolation);
});

arch_test!(fail_encode_capped_over_cap, {
    let msg = Detach { reason: "too-long-for-tiny-cap" };
    let mut buf = [0u8; 4096];
    let err = encode_capped(&msg, &mut buf, 2).expect_err("over-cap encode must fail");
    reovim_arch::testrt::check_eq(err, ErrorCode::ResourceExhausted);
});

arch_test!(fail_reject_out_of_range_code_degrades_to_generic, {
    let body: &[u8] = &[
        0x0F, 0x27, 0x00, 0x00, // code = 9999 LE i32
        0x00, 0x00, 0x00, 0x00, // detail = "" (len = 0)
    ];
    let back = Reject::decode(body).expect("Reject with out-of-range code must not fail");
    reovim_arch::testrt::check_eq(back.code, ErrorCode::Generic);
});

arch_test!(fail_truncated_mid_list_count, {
    // Hello: major(2) + minor(2) + partial count (2 bytes of 4)
    let body: &[u8] = &[0x01, 0x00, 0x00, 0x00, 0x01, 0x00];
    let err = Hello::decode(body).expect_err("truncated list count must fail");
    reovim_arch::testrt::check_eq(err, ErrorCode::ProtocolViolation);
});

arch_test!(fail_truncated_mid_string_payload, {
    // Detach reason: len=5 but only 2 bytes follow
    let body: &[u8] = &[0x05, 0x00, 0x00, 0x00, b'h', b'i'];
    let err = Detach::decode(body).expect_err("truncated string payload must fail");
    reovim_arch::testrt::check_eq(err, ErrorCode::ProtocolViolation);
});

arch_test!(fail_compressed_flag_rejected_by_state_machine, {
    use reovim_uapi_protocol::{
        frame::FLAG_COMPRESSED,
        state::{Action, ProtocolState, Role, TagInfo},
    };
    let mut sm = ProtocolState::new(Role::Server);
    // First frame: Hello (handshake), no flags → Accept.
    let hello_hdr = FrameHeader {
        body_len: 0,
        msg_type: Hello::TAG,
        flags: 0,
        correlation_id: 0,
    };
    reovim_arch::testrt::check_eq(
        sm.on_frame(hello_hdr, TagInfo::Known(Direction::Handshake)),
        Action::Accept,
    );
    // Req with COMPRESSED flag set → Reject.
    let req_hdr = FrameHeader {
        body_len: 0,
        msg_type: 0x0100,
        flags: FLAG_COMPRESSED,
        correlation_id: 1,
    };
    let action = sm.on_frame(req_hdr, TagInfo::Known(Direction::Request));
    reovim_arch::testrt::check(
        matches!(action, Action::Reject(_)),
        "COMPRESSED flag in req → Reject",
    );
});

// ---------------------------------------------------------------------------
// State machine failure modes
// ---------------------------------------------------------------------------

arch_test!(fail_state_machine_first_frame_not_hello, {
    use reovim_uapi_protocol::state::{Action, ProtocolState, Role, TagInfo};
    let mut sm = ProtocolState::new(Role::Server);
    let req_hdr = FrameHeader {
        body_len: 0,
        msg_type: 0x0100,
        flags: 0,
        correlation_id: 1,
    };
    let action = sm.on_frame(req_hdr, TagInfo::Known(Direction::Request));
    reovim_arch::testrt::check(
        matches!(action, Action::Reject(_)),
        "SP9.1: non-Hello first frame → Reject",
    );
});

arch_test!(pass_state_machine_unknown_notify_skipped, {
    use reovim_uapi_protocol::state::{Action, ProtocolState, Role, TagInfo};
    let mut sm = ProtocolState::new(Role::Client);
    let ack_hdr = FrameHeader {
        body_len: 0,
        msg_type: HelloAck::TAG,
        flags: 0,
        correlation_id: 0,
    };
    reovim_arch::testrt::check_eq(
        sm.on_frame(ack_hdr, TagInfo::Known(Direction::Handshake)),
        Action::Accept,
    );
    let unknown_hdr = FrameHeader {
        body_len: 4,
        msg_type: 0x03FF,
        flags: 0,
        correlation_id: 0,
    };
    let action = sm.on_frame(unknown_hdr, TagInfo::Unknown);
    reovim_arch::testrt::check_eq(action, Action::SkipNotify);
});

arch_test!(fail_state_machine_unknown_req_rejected, {
    use reovim_uapi_protocol::state::{Action, ProtocolState, Role, TagInfo};
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
    reovim_arch::testrt::check(
        matches!(action, Action::Reject(_)),
        "§10.3: unknown req tag → Reject",
    );
});
