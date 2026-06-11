//! Coverage-closure edge-case tests (#786 Phase 5 coverage-closure slice).
//!
//! Closes every uncovered region identified by the merged MC/DC report:
//!
//! - `messages.rs`: `raw_input_kind_from_u8` all 8 arms; `error_code_from_i32`
//!   all 26 arms; `Message::encode` `BufferTooSmall` guard; per-message
//!   `encode_body`/`decode` truncation sweep for every message with non-empty
//!   list fields; `put_str_list`/`decode_str_list`/`decode_domain_list`/
//!   `decode_carrier_list`/`decode_rawinput_list` with count > 0 payloads.
//! - `codec.rs`: `put_u16/u32/u64/i32/bool/bytes/str/u8_array8` short-buffer
//!   error arms; `get_u16/u32/u64/i32/u8_array8` truncation arms.
//! - `frame.rs`: `write_frame` cap-exceeded and short-output-buffer arms;
//!   `read_frame` body-shorter-than-declared arm.
//! - `state.rs`: Client-role handshake path; post-handshake closed-state
//!   re-entry; Notify with non-zero `correlation_id`; Response/Error direction
//!   in `on_known_frame`; client-side `on_unknown_frame` (SkipNotify).
//! - `view.rs`: multi-element iterators for all four list types; `is_empty`
//!   and `len` on non-empty lists; `IntoIterator for &List` shims.

use reovim_arch::arch_test;
use reovim_uapi_abi::{ErrorCode, FrameHeader, input::RawInputKind};
use reovim_uapi_protocol::{
    codec::{Decoder, Encoder},
    frame::{HEADER_LEN, read_frame, write_frame},
    messages::{
        Attach, AttachAck, AttachEventClientLeft, AttachEventCursor,
        AttachEventDomainTableDelta, AttachEventDiff, AttachEventFrame, AttachEventProjection,
        AttachEventServerDraining, AttachEventSessionDestroyed, AttachEventSessionPivot,
        ConfigDump, ConfigDumpResponse, ConfigValidate, ConfigValidateResponse, DebugDrive,
        DebugDriveAck, DebugRead, DebugReadEvent, DestroySession, Detach, Direction, Hello,
        HelloAck, InputAck, Message, PkgSync, PkgSyncDone, PkgSyncProgress, PkgVerify,
        PkgVerifyAck, Reject, RenameSession, SendInput, SwitchSession, error_code_from_i32,
        raw_input_kind_from_u8,
    },
    state::{Action, Phase, ProtocolState, RejectReason, Role, TagInfo},
    view::{CarrierList, DomainEntryList, RawInputList, StrList},
};

// ---------------------------------------------------------------------------
// Generic edge-sweep helper
// ---------------------------------------------------------------------------

/// Asserts that encoding `msg` into a buffer exactly one byte too short returns
/// `BufferTooSmall` and leaves the entire short buffer unmodified (`0xFF`
/// sentinel pattern, SP13: nothing written past failure).
fn check_encode_too_small<M: Message>(msg: &M) {
    let sz = msg.encoded_size();
    if sz == 0 {
        // Zero-size messages have no short-buffer case to close.
        return;
    }
    // Cap at 128 so we stay on the stack (all real messages fit well within
    // this; the largest test value is ~200 B but those use a separate buffer).
    let mut backing = [0xFFu8; 256];
    let short_len = sz - 1;
    let err = msg
        .encode(&mut backing[..short_len])
        .expect_err("BufferTooSmall expected");
    reovim_arch::testrt::check_eq(err, ErrorCode::BufferTooSmall);
    for b in &backing[..short_len] {
        reovim_arch::testrt::check(*b == 0xFF, "SP13: nothing written on short buffer");
    }
}

/// Calls `encode_body` directly on `msg` with a zero-length buffer, triggering
/// the first `put_*` inside `encode_body` to return `BufferTooSmall`.
///
/// This closes the `?`-operator error branches inside `encode_body` that
/// `Message::encode` normally pre-empts with its size guard.  These branches
/// are reachable only through a direct call.
fn check_encode_body_zero_buf<M: Message>(msg: &M) {
    // Full truncation sweep, both directions: for EVERY k < encoded_size,
    // a k-byte output must fail encode_body (the per-field put_* Err edges
    // the SP13 size guard pre-empts on the public path) and is checked by
    // the caller's decode sweep against the same boundaries.
    let size = msg.encoded_size();
    let mut scratch = [0u8; 512];
    let mut k = 0;
    while k < size {
        let err = msg
            .encode_body(&mut scratch[..k])
            .expect_err("encode_body on short buffer must fail");
        reovim_arch::testrt::check(
            err == ErrorCode::BufferTooSmall || err == ErrorCode::ResourceExhausted,
            "short encode_body fails with a sizing code",
        );
        k += 1;
    }
}

/// Decodes every strict prefix of `full[..size]`; each must fail (a cut at
/// any byte leaves some field short, driving that field's `?` Err edge).
fn check_decode_truncation_sweep<F>(full: &[u8], size: usize, decode_fails: F)
where
    F: Fn(&[u8]) -> bool,
{
    let mut k = 0;
    while k < size {
        reovim_arch::testrt::check(decode_fails(&full[..k]), "truncated decode fails");
        k += 1;
    }
}

// ---------------------------------------------------------------------------
// messages.rs: raw_input_kind_from_u8 — all 8 arms
// ---------------------------------------------------------------------------

arch_test!(msg_raw_input_kind_all_arms, {
    // All 7 valid discriminants.
    reovim_arch::testrt::check_eq(raw_input_kind_from_u8(1).unwrap(), RawInputKind::Key);
    reovim_arch::testrt::check_eq(raw_input_kind_from_u8(2).unwrap(), RawInputKind::Mouse);
    reovim_arch::testrt::check_eq(raw_input_kind_from_u8(3).unwrap(), RawInputKind::Text);
    reovim_arch::testrt::check_eq(raw_input_kind_from_u8(4).unwrap(), RawInputKind::Paste);
    reovim_arch::testrt::check_eq(raw_input_kind_from_u8(5).unwrap(), RawInputKind::Web);
    reovim_arch::testrt::check_eq(raw_input_kind_from_u8(6).unwrap(), RawInputKind::Trigger);
    reovim_arch::testrt::check_eq(raw_input_kind_from_u8(7).unwrap(), RawInputKind::Ime);
    // `_` arm: unknown discriminant.
    reovim_arch::testrt::check_eq(
        raw_input_kind_from_u8(0).unwrap_err(),
        ErrorCode::InvalidArgument,
    );
    reovim_arch::testrt::check_eq(
        raw_input_kind_from_u8(8).unwrap_err(),
        ErrorCode::InvalidArgument,
    );
    reovim_arch::testrt::check_eq(
        raw_input_kind_from_u8(255).unwrap_err(),
        ErrorCode::InvalidArgument,
    );
});

// ---------------------------------------------------------------------------
// messages.rs: error_code_from_i32 — all 26 arms + out-of-range `_` arm
// ---------------------------------------------------------------------------

arch_test!(msg_error_code_from_i32_all_arms, {
    reovim_arch::testrt::check_eq(error_code_from_i32(0), ErrorCode::Ok);
    // 1 (Generic) falls through to the `_` arm.
    reovim_arch::testrt::check_eq(error_code_from_i32(1), ErrorCode::Generic);
    reovim_arch::testrt::check_eq(error_code_from_i32(2), ErrorCode::IncompatibleAbi);
    reovim_arch::testrt::check_eq(error_code_from_i32(3), ErrorCode::IncompatibleApi);
    reovim_arch::testrt::check_eq(error_code_from_i32(4), ErrorCode::NotFound);
    reovim_arch::testrt::check_eq(error_code_from_i32(5), ErrorCode::Conflict);
    reovim_arch::testrt::check_eq(error_code_from_i32(6), ErrorCode::InvalidArgument);
    reovim_arch::testrt::check_eq(error_code_from_i32(7), ErrorCode::ResourceExhausted);
    reovim_arch::testrt::check_eq(error_code_from_i32(8), ErrorCode::ProtocolViolation);
    reovim_arch::testrt::check_eq(error_code_from_i32(9), ErrorCode::Busy);
    reovim_arch::testrt::check_eq(error_code_from_i32(10), ErrorCode::Stale);
    reovim_arch::testrt::check_eq(error_code_from_i32(11), ErrorCode::PermissionDenied);
    reovim_arch::testrt::check_eq(error_code_from_i32(12), ErrorCode::Panic);
    reovim_arch::testrt::check_eq(error_code_from_i32(13), ErrorCode::Cancelled);
    reovim_arch::testrt::check_eq(error_code_from_i32(14), ErrorCode::Timeout);
    reovim_arch::testrt::check_eq(error_code_from_i32(15), ErrorCode::SchemaInvalid);
    reovim_arch::testrt::check_eq(error_code_from_i32(16), ErrorCode::NamespaceConflict);
    reovim_arch::testrt::check_eq(error_code_from_i32(17), ErrorCode::IllegalTrustClass);
    reovim_arch::testrt::check_eq(error_code_from_i32(18), ErrorCode::ConfigSliceTooLarge);
    reovim_arch::testrt::check_eq(error_code_from_i32(19), ErrorCode::Utf8Invalid);
    reovim_arch::testrt::check_eq(error_code_from_i32(20), ErrorCode::IllegalProjectHostSection);
    reovim_arch::testrt::check_eq(error_code_from_i32(21), ErrorCode::IllegalKind);
    reovim_arch::testrt::check_eq(error_code_from_i32(22), ErrorCode::ShortVtable);
    reovim_arch::testrt::check_eq(error_code_from_i32(23), ErrorCode::CodecGone);
    reovim_arch::testrt::check_eq(error_code_from_i32(24), ErrorCode::NotActive);
    reovim_arch::testrt::check_eq(error_code_from_i32(25), ErrorCode::RollbackFailed);
    reovim_arch::testrt::check_eq(error_code_from_i32(26), ErrorCode::BufferTooSmall);
    // Out-of-range: degrades to Generic.
    reovim_arch::testrt::check_eq(error_code_from_i32(27), ErrorCode::Generic);
    reovim_arch::testrt::check_eq(error_code_from_i32(-1), ErrorCode::Generic);
    reovim_arch::testrt::check_eq(error_code_from_i32(9999), ErrorCode::Generic);
});

// ---------------------------------------------------------------------------
// messages.rs: encode_capped success path (line 161-162)
// ---------------------------------------------------------------------------

arch_test!(msg_encode_capped_success_path, {
    use reovim_uapi_protocol::messages::encode_capped;
    // encode_capped with a sufficient cap must succeed and return the same
    // byte count as encoded_size (line 162: `msg.encode(out)` branch).
    let msg = SwitchSession { session_name: "x" };
    let mut buf = [0u8; 64];
    let n = encode_capped(&msg, &mut buf, 4096).expect("encode_capped success");
    reovim_arch::testrt::check_eq(n, msg.encoded_size());
});

// ---------------------------------------------------------------------------
// messages.rs: Message::encode BufferTooSmall guard (trait provided body)
// ---------------------------------------------------------------------------

arch_test!(msg_encode_buffer_too_small_guard, {
    // SwitchSession: encoded_size = 4 + 3 = 7.  Encoding into a 6-byte buffer
    // must fail via the trait-provided encode() guard before encode_body runs.
    let msg = SwitchSession { session_name: "abc" };
    check_encode_too_small(&msg);

    // Reject: encoded_size = 4 + 4 + 5 = 13.
    let msg2 = Reject { code: ErrorCode::Ok, detail: "oops" };
    check_encode_too_small(&msg2);
});

// ---------------------------------------------------------------------------
// messages.rs: non-empty list encode/decode paths
// (closes decode_str_list loop body, put_str_list loop body,
//  decode_domain_list loop body, decode_carrier_list loop body,
//  decode_rawinput_list loop body, and the per-message encode_body lines)
// ---------------------------------------------------------------------------

/// Build a `StrList` backed by a pre-encoded buffer containing two strings.
macro_rules! str_list_two {
    ($backing:ident, $a:expr, $b:expr) => {{
        let mut enc = Encoder::new(&mut $backing);
        enc.put_u32(2).unwrap(); // count
        enc.put_str($a).unwrap();
        enc.put_str($b).unwrap();
        StrList::from_validated(&$backing)
    }};
}

arch_test!(msg_hello_with_nonempty_lists, {
    let mut caps_buf = [0u8; 64];
    let mut codecs_buf = [0u8; 64];
    let caps = str_list_two!(caps_buf, "render.cells", "render.dom");
    let codecs = str_list_two!(codecs_buf, "text.utf8", "text.xxd");

    let msg = Hello {
        protocol_major: 1,
        protocol_minor: 0,
        caps,
        domain_codecs: codecs,
    };
    // encode_body with non-empty caps list exercises put_str_list loop body
    // and decode_str_list loop body.
    let mut buf = [0u8; 4096];
    let n = msg.encode(&mut buf).expect("Hello encode");
    reovim_arch::testrt::check_eq(n, msg.encoded_size());
    let back = Hello::decode(&buf[..n]).expect("Hello decode");
    reovim_arch::testrt::check_eq(back.protocol_major, 1);
    reovim_arch::testrt::check_eq(back.caps.len(), 2);

    // Collect caps via iterator to exercise StrListIter::next on multi-element.
    let mut cap_names = [""; 4];
    let mut idx = 0usize;
    for s in back.caps.iter() {
        cap_names[idx] = s;
        idx += 1;
    }
    reovim_arch::testrt::check(cap_names[0] == "render.cells", "Hello caps[0]");
    reovim_arch::testrt::check(cap_names[1] == "render.dom", "Hello caps[1]");
    reovim_arch::testrt::check(!back.caps.is_empty(), "Hello caps not empty");

    // encode BufferTooSmall path for Hello with non-empty lists.
    check_encode_too_small(&msg);

    // Full edge sweep on the NON-EMPTY value: drives the list-element
    // probe/put loop Err edges that empty-list sweeps cannot reach.
    check_encode_body_zero_buf(&msg);
    check_decode_truncation_sweep(&buf, n, |b| Hello::decode(b).is_err());
});

arch_test!(msg_hello_ack_with_nonempty_caps, {
    let mut caps_buf = [0u8; 64];
    let caps = str_list_two!(caps_buf, "render.cells", "debug");
    let msg = HelloAck {
        protocol_major: 1,
        protocol_minor: 0,
        granted_caps: caps,
        server_name: "reovim",
    };
    let mut buf = [0u8; 4096];
    let n = msg.encode(&mut buf).expect("HelloAck encode");
    let back = HelloAck::decode(&buf[..n]).expect("HelloAck decode");
    reovim_arch::testrt::check_eq(back.granted_caps.len(), 2);
    check_encode_too_small(&msg);

    // Full edge sweep on the NON-EMPTY value: drives the list-element
    // probe/put loop Err edges that empty-list sweeps cannot reach.
    check_encode_body_zero_buf(&msg);
    check_decode_truncation_sweep(&buf, n, |b| HelloAck::decode(b).is_err());
});

arch_test!(msg_attach_with_nonempty_lists, {
    let mut caps_buf = [0u8; 64];
    let mut codecs_buf = [0u8; 64];
    let caps = str_list_two!(caps_buf, "cap-a", "cap-b");
    let codecs = str_list_two!(codecs_buf, "codec-x", "codec-y");
    let msg = Attach {
        session_name: "main",
        auth: b"tok",
        caps,
        domain_codecs: codecs,
    };
    let mut buf = [0u8; 4096];
    let n = msg.encode(&mut buf).expect("Attach encode");
    let back = Attach::decode(&buf[..n]).expect("Attach decode");
    reovim_arch::testrt::check_eq(back.caps.len(), 2);
    check_encode_too_small(&msg);

    // Full edge sweep on the NON-EMPTY value: drives the list-element
    // probe/put loop Err edges that empty-list sweeps cannot reach.
    check_encode_body_zero_buf(&msg);
    check_decode_truncation_sweep(&buf, n, |b| Attach::decode(b).is_err());
});

arch_test!(msg_pkg_sync_with_nonempty_targets, {
    let mut targets_buf = [0u8; 64];
    let targets = str_list_two!(targets_buf, "mod-vim", "mod-lsp");
    let msg = PkgSync { targets, dry_run: true };
    let mut buf = [0u8; 4096];
    let n = msg.encode(&mut buf).expect("PkgSync encode");
    let back = PkgSync::decode(&buf[..n]).expect("PkgSync decode");
    reovim_arch::testrt::check_eq(back.targets.len(), 2);
    reovim_arch::testrt::check(back.dry_run, "PkgSync.dry_run");
    check_encode_too_small(&msg);

    // Full edge sweep on the NON-EMPTY value: drives the list-element
    // probe/put loop Err edges that empty-list sweeps cannot reach.
    check_encode_body_zero_buf(&msg);
    check_decode_truncation_sweep(&buf, n, |b| PkgSync::decode(b).is_err());
});

// Build an `AttachAck` body with one non-empty `DomainEntry` and verify
// all encode/decode paths exercise the domain list loop bodies.
arch_test!(msg_attach_ack_with_domain_entry, {
    // Wire layout for AttachAck: client_id(u64) + domain_list
    // domain_list wire: count(u32) + [str + u32 + bool + bool] per entry
    let mut body_buf = [0u8; 128];
    let used = {
        let mut enc = Encoder::new(&mut body_buf);
        enc.put_u64(99).unwrap(); // client_id
        enc.put_u32(1).unwrap(); // domain_table count = 1
        enc.put_str("text").unwrap(); // domain_name
        enc.put_u32(42).unwrap(); // inner_id
        enc.put_bool(true).unwrap(); // has_display
        enc.put_bool(false).unwrap(); // has_semantic
        enc.position()
    };
    let back = AttachAck::decode(&body_buf[..used]).expect("AttachAck decode");
    reovim_arch::testrt::check_eq(back.client_id, 99);
    reovim_arch::testrt::check_eq(back.domain_table.len(), 1);
    reovim_arch::testrt::check(!back.domain_table.is_empty(), "domain_table not empty");

    // Iterate to exercise DomainEntryIter::next on a non-empty list.
    let mut count = 0usize;
    for e in back.domain_table.iter() {
        reovim_arch::testrt::check(e.domain_name == "text", "domain_name");
        reovim_arch::testrt::check_eq(e.inner_id, 42);
        reovim_arch::testrt::check(e.has_display, "has_display");
        reovim_arch::testrt::check(!e.has_semantic, "has_semantic");
        count += 1;
    }
    reovim_arch::testrt::check_eq(count, 1);

    // IntoIterator for &DomainEntryList.
    let _ = (&back.domain_table).into_iter().count();

    // Round-trip via encode so encode_body with domain entries is covered
    // (exercises put_domain_list loop body and domain_table_size iterator).
    let mut enc_buf = [0u8; 256];
    let n = back.encode(&mut enc_buf).expect("AttachAck encode");
    let back2 = AttachAck::decode(&enc_buf[..n]).expect("AttachAck re-decode");
    reovim_arch::testrt::check_eq(back2.domain_table.len(), 1);

    check_encode_too_small(&back);

    // Full edge sweep on the NON-EMPTY value: drives the list-element
    // probe/put loop Err edges that empty-list sweeps cannot reach.
    check_encode_body_zero_buf(&back);
    check_decode_truncation_sweep(&enc_buf, n, |b| AttachAck::decode(b).is_err());
});

arch_test!(msg_attach_event_domain_table_delta_with_entries, {
    // Build a body with 1 added + 1 removed domain entry.
    let mut buf = [0u8; 256];
    let used = {
        let mut enc = Encoder::new(&mut buf);
        // added: count=1, one entry
        enc.put_u32(1).unwrap();
        enc.put_str("domain-a").unwrap();
        enc.put_u32(1).unwrap();
        enc.put_bool(true).unwrap();
        enc.put_bool(true).unwrap();
        // removed: count=1, one entry
        enc.put_u32(1).unwrap();
        enc.put_str("domain-b").unwrap();
        enc.put_u32(2).unwrap();
        enc.put_bool(false).unwrap();
        enc.put_bool(false).unwrap();
        enc.position()
    };
    let back =
        AttachEventDomainTableDelta::decode(&buf[..used]).expect("DomainTableDelta decode");
    reovim_arch::testrt::check_eq(back.added.len(), 1);
    reovim_arch::testrt::check_eq(back.removed.len(), 1);

    // Round-trip to exercise encode_body with non-empty domain lists.
    let mut enc_buf = [0u8; 256];
    let n = back.encode(&mut enc_buf).expect("DomainTableDelta encode");
    let back2 =
        AttachEventDomainTableDelta::decode(&enc_buf[..n]).expect("DomainTableDelta re-decode");
    reovim_arch::testrt::check_eq(back2.added.len(), 1);

    check_encode_too_small(&back);

    // Full edge sweep on the NON-EMPTY value: drives the list-element
    // probe/put loop Err edges that empty-list sweeps cannot reach.
    check_encode_body_zero_buf(&back);
    check_decode_truncation_sweep(&enc_buf, n, |b| AttachEventDomainTableDelta::decode(b).is_err());
});

arch_test!(msg_attach_event_cursor_with_carrier, {
    // Build a body: buffer_id(u64) + window_id(u64) + carrier_list (count=1)
    let mut buf = [0u8; 256];
    let used = {
        let mut enc = Encoder::new(&mut buf);
        enc.put_u64(3).unwrap(); // buffer_id
        enc.put_u64(7).unwrap(); // window_id
        enc.put_u32(1).unwrap(); // cursors count
        enc.put_u8_array8(&[1, 2, 3, 4, 5, 6, 7, 8]).unwrap(); // header
        enc.put_bytes(b"cdata").unwrap(); // content
        enc.position()
    };
    let back = AttachEventCursor::decode(&buf[..used]).expect("AttachEventCursor decode");
    reovim_arch::testrt::check_eq(back.cursors.len(), 1);
    reovim_arch::testrt::check(!back.cursors.is_empty(), "cursors not empty");

    // Iterate to exercise CarrierIter::next on non-empty list.
    for c in back.cursors.iter() {
        reovim_arch::testrt::check_eq(c.header, [1, 2, 3, 4, 5, 6, 7, 8]);
        reovim_arch::testrt::check(c.content == b"cdata", "carrier content");
    }

    // IntoIterator for &CarrierList.
    let _ = (&back.cursors).into_iter().count();

    // encode_body with non-empty carrier list.
    let mut enc_buf = [0u8; 256];
    let n = back.encode(&mut enc_buf).expect("AttachEventCursor encode");
    let back2 = AttachEventCursor::decode(&enc_buf[..n]).expect("AttachEventCursor re-decode");
    reovim_arch::testrt::check_eq(back2.cursors.len(), 1);

    check_encode_too_small(&back);

    // Full edge sweep on the NON-EMPTY value: drives the list-element
    // probe/put loop Err edges that empty-list sweeps cannot reach.
    check_encode_body_zero_buf(&back);
    check_decode_truncation_sweep(&enc_buf, n, |b| AttachEventCursor::decode(b).is_err());
});

arch_test!(msg_send_input_with_rawinput_item, {
    // Build a SendInput body with one RawInput item (kind=Key, payload=2 bytes).
    let mut buf = [0u8; 256];
    let used = {
        let mut enc = Encoder::new(&mut buf);
        enc.put_u64(1).unwrap(); // client_id
        enc.put_u64(2).unwrap(); // buffer_id
        enc.put_u64(3).unwrap(); // window_id
        enc.put_u32(1).unwrap(); // inputs count
        enc.put_u8(1).unwrap(); // kind = Key
        enc.put_bytes(&[0x10, 0x20]).unwrap(); // payload
        enc.position()
    };
    let back = SendInput::decode(&buf[..used]).expect("SendInput decode");
    reovim_arch::testrt::check_eq(back.inputs.len(), 1);
    reovim_arch::testrt::check(!back.inputs.is_empty(), "inputs not empty");

    // Iterate to exercise RawInputIter::next.
    for item in back.inputs.iter() {
        reovim_arch::testrt::check_eq(item.kind, RawInputKind::Key);
        reovim_arch::testrt::check(item.payload == &[0x10, 0x20], "input payload");
    }

    // IntoIterator for &RawInputList.
    let _ = (&back.inputs).into_iter().count();

    // encode_body with non-empty inputs (exercises decode_rawinput_list loop).
    let mut enc_buf = [0u8; 256];
    let n = back.encode(&mut enc_buf).expect("SendInput encode");
    let back2 = SendInput::decode(&enc_buf[..n]).expect("SendInput re-decode");
    reovim_arch::testrt::check_eq(back2.inputs.len(), 1);

    check_encode_too_small(&back);

    // Full edge sweep on the NON-EMPTY value: drives the list-element
    // probe/put loop Err edges that empty-list sweeps cannot reach.
    check_encode_body_zero_buf(&back);
    check_decode_truncation_sweep(&enc_buf, n, |b| SendInput::decode(b).is_err());
});

// ---------------------------------------------------------------------------
// view.rs: is_empty / len on non-empty StrList, RawInputList, CarrierList
// (some accessors are only reachable through the non-empty path above, but
//  keep explicit checks here so the coverage region is unambiguous)
// ---------------------------------------------------------------------------

arch_test!(view_str_list_nonempty_accessors, {
    let mut buf = [0u8; 64];
    let list = str_list_two!(buf, "hello", "world");
    reovim_arch::testrt::check_eq(list.len(), 2);
    reovim_arch::testrt::check(!list.is_empty(), "StrList not empty");

    // IntoIterator for StrList (by value) and &StrList.
    let mut count = 0usize;
    for _s in list {
        count += 1;
    }
    reovim_arch::testrt::check_eq(count, 2);

    let mut buf2 = [0u8; 64];
    let list2 = str_list_two!(buf2, "a", "b");
    let mut count2 = 0usize;
    for _s in &list2 {
        count2 += 1;
    }
    reovim_arch::testrt::check_eq(count2, 2);
});

arch_test!(view_raw_input_list_nonempty_accessors, {
    // count=2 payload
    let mut buf = [0u8; 64];
    {
        let mut enc = Encoder::new(&mut buf);
        enc.put_u32(2).unwrap();
        enc.put_u8(1).unwrap(); // Key
        enc.put_bytes(b"k").unwrap();
        enc.put_u8(3).unwrap(); // Text
        enc.put_bytes(b"t").unwrap();
    }
    let list = RawInputList::from_validated(&buf);
    reovim_arch::testrt::check_eq(list.len(), 2);
    reovim_arch::testrt::check(!list.is_empty(), "RawInputList not empty");
    reovim_arch::testrt::check_eq(list.iter().count(), 2);

    // IntoIterator for &RawInputList and by value.
    let _ = list.into_iter().count();

    // Exact size_hint contract on this view's iterator (telemetry ask):
    // (left, Some(left)) before and after one step.
    let mut sh = list.iter();
    reovim_arch::testrt::check_eq(sh.size_hint(), (2usize, Some(2usize)));
    let _ = sh.next();
    reovim_arch::testrt::check_eq(sh.size_hint(), (2usize - 1, Some(2usize - 1)));
});

arch_test!(view_domain_entry_list_nonempty_accessors, {
    let mut buf = [0u8; 64];
    {
        let mut enc = Encoder::new(&mut buf);
        enc.put_u32(1).unwrap();
        enc.put_str("d").unwrap();
        enc.put_u32(0).unwrap();
        enc.put_bool(false).unwrap();
        enc.put_bool(true).unwrap();
    }
    let list = DomainEntryList::from_validated(&buf);
    reovim_arch::testrt::check_eq(list.len(), 1);
    reovim_arch::testrt::check(!list.is_empty(), "DomainEntryList not empty");
    let _ = list.into_iter().count();

    // Exact size_hint contract on this view's iterator (telemetry ask):
    // (left, Some(left)) before and after one step.
    let mut sh = list.iter();
    reovim_arch::testrt::check_eq(sh.size_hint(), (1usize, Some(1usize)));
    let _ = sh.next();
    reovim_arch::testrt::check_eq(sh.size_hint(), (1usize - 1, Some(1usize - 1)));
});

arch_test!(view_carrier_list_nonempty_accessors, {
    let mut buf = [0u8; 64];
    {
        let mut enc = Encoder::new(&mut buf);
        enc.put_u32(1).unwrap();
        enc.put_u8_array8(&[0u8; 8]).unwrap();
        enc.put_bytes(b"x").unwrap();
    }
    let list = CarrierList::from_validated(&buf);
    reovim_arch::testrt::check_eq(list.len(), 1);
    reovim_arch::testrt::check(!list.is_empty(), "CarrierList not empty");
    let _ = list.into_iter().count();

    // Exact size_hint contract on this view's iterator (telemetry ask):
    // (left, Some(left)) before and after one step.
    let mut sh = list.iter();
    reovim_arch::testrt::check_eq(sh.size_hint(), (1usize, Some(1usize)));
    let _ = sh.next();
    reovim_arch::testrt::check_eq(sh.size_hint(), (1usize - 1, Some(1usize - 1)));
});

// ---------------------------------------------------------------------------
// codec.rs: put_* short-buffer error arms (all primitive widths)
// ---------------------------------------------------------------------------

arch_test!(codec_decoder_accessors, {
    // Decoder::is_empty() — both arms.
    reovim_arch::testrt::check(Decoder::new(&[]).is_empty(), "empty decoder is_empty");
    reovim_arch::testrt::check(!Decoder::new(&[1u8]).is_empty(), "non-empty decoder not empty");

    // get_bool false arm (value 0).
    reovim_arch::testrt::check_eq(Decoder::new(&[0u8]).get_bool().unwrap(), false);
    // get_bool true arm (value 1) — exercised in existing tests; cover explicitly.
    reovim_arch::testrt::check_eq(Decoder::new(&[1u8]).get_bool().unwrap(), true);

    // get_u8_array8 success path (lines 552-555).
    let arr = Decoder::new(&[1u8, 2, 3, 4, 5, 6, 7, 8]).get_u8_array8().unwrap();
    reovim_arch::testrt::check_eq(arr, [1u8, 2, 3, 4, 5, 6, 7, 8]);
});

arch_test!(codec_put_primitives_short_buffer, {
    // put_u16: 1-byte buffer
    let mut b1 = [0u8; 1];
    reovim_arch::testrt::check_eq(
        Encoder::new(&mut b1).put_u16(1).unwrap_err(),
        ErrorCode::BufferTooSmall,
    );
    // put_u32: 3-byte buffer
    let mut b3 = [0u8; 3];
    reovim_arch::testrt::check_eq(
        Encoder::new(&mut b3).put_u32(1).unwrap_err(),
        ErrorCode::BufferTooSmall,
    );
    // put_u64: 7-byte buffer
    let mut b7 = [0u8; 7];
    reovim_arch::testrt::check_eq(
        Encoder::new(&mut b7).put_u64(1).unwrap_err(),
        ErrorCode::BufferTooSmall,
    );
    // put_i32: 3-byte buffer
    let mut b3b = [0u8; 3];
    reovim_arch::testrt::check_eq(
        Encoder::new(&mut b3b).put_i32(-1).unwrap_err(),
        ErrorCode::BufferTooSmall,
    );
    // put_bool: 0-byte buffer
    let mut b0: [u8; 0] = [];
    reovim_arch::testrt::check_eq(
        Encoder::new(&mut b0).put_bool(true).unwrap_err(),
        ErrorCode::BufferTooSmall,
    );
    // put_bytes: buffer too small for the prefix
    let mut b3c = [0u8; 3];
    reovim_arch::testrt::check_eq(
        Encoder::new(&mut b3c).put_bytes(b"x").unwrap_err(),
        ErrorCode::BufferTooSmall,
    );
    // put_str: same path as put_bytes
    let mut b3d = [0u8; 3];
    reovim_arch::testrt::check_eq(
        Encoder::new(&mut b3d).put_str("x").unwrap_err(),
        ErrorCode::BufferTooSmall,
    );
    // put_u8_array8: 7-byte buffer
    let mut b7b = [0u8; 7];
    reovim_arch::testrt::check_eq(
        Encoder::new(&mut b7b).put_u8_array8(&[0u8; 8]).unwrap_err(),
        ErrorCode::BufferTooSmall,
    );
});

arch_test!(codec_put_bytes_payload_too_short, {
    // put_bytes: prefix fits (4 bytes) but payload does not (need 5, only 7 total
    // so only 3 bytes remain after the prefix).
    let mut b7 = [0u8; 7];
    reovim_arch::testrt::check_eq(
        Encoder::new(&mut b7).put_bytes(b"hello12").unwrap_err(),
        ErrorCode::BufferTooSmall,
    );
});

// ---------------------------------------------------------------------------
// codec.rs: get_* truncation arms (all primitive widths)
// ---------------------------------------------------------------------------

arch_test!(codec_get_primitives_truncated, {
    // get_u8: empty buffer
    reovim_arch::testrt::check_eq(
        Decoder::new(&[]).get_u8().unwrap_err(),
        ErrorCode::ProtocolViolation,
    );
    // get_u16: only 1 byte
    reovim_arch::testrt::check_eq(
        Decoder::new(&[0u8; 1]).get_u16().unwrap_err(),
        ErrorCode::ProtocolViolation,
    );
    // get_u32: only 3 bytes
    reovim_arch::testrt::check_eq(
        Decoder::new(&[0u8; 3]).get_u32().unwrap_err(),
        ErrorCode::ProtocolViolation,
    );
    // get_u64: only 7 bytes
    reovim_arch::testrt::check_eq(
        Decoder::new(&[0u8; 7]).get_u64().unwrap_err(),
        ErrorCode::ProtocolViolation,
    );
    // get_i32: only 3 bytes
    reovim_arch::testrt::check_eq(
        Decoder::new(&[0u8; 3]).get_i32().unwrap_err(),
        ErrorCode::ProtocolViolation,
    );
    // get_u8_array8: only 7 bytes
    reovim_arch::testrt::check_eq(
        Decoder::new(&[0u8; 7]).get_u8_array8().unwrap_err(),
        ErrorCode::ProtocolViolation,
    );
    // get_bytes: prefix itself truncated (only 3 of 4 prefix bytes)
    reovim_arch::testrt::check_eq(
        Decoder::new(&[0x00u8; 3]).get_bytes().unwrap_err(),
        ErrorCode::ProtocolViolation,
    );
    // get_bytes: prefix present but payload truncated (len=5, only 1 payload byte)
    let truncated_bytes = [0x05, 0x00, 0x00, 0x00, b'a']; // len=5, only 1 byte
    reovim_arch::testrt::check_eq(
        Decoder::new(&truncated_bytes).get_bytes().unwrap_err(),
        ErrorCode::ProtocolViolation,
    );
    // get_str: length prefix present but payload truncated
    let truncated_str = [0x03, 0x00, 0x00, 0x00, b'h']; // len=3, only 1 byte
    reovim_arch::testrt::check_eq(
        Decoder::new(&truncated_str).get_str().unwrap_err(),
        ErrorCode::ProtocolViolation,
    );
});

// ---------------------------------------------------------------------------
// frame.rs: write_frame cap-exceeded and short-output-buffer arms
// ---------------------------------------------------------------------------

arch_test!(frame_write_frame_cap_exceeded, {
    // body.len() > cap → ResourceExhausted (line 91-92 in frame.rs)
    let hdr = FrameHeader { body_len: 0, msg_type: 1, flags: 0, correlation_id: 0 };
    let mut out = [0u8; 256];
    reovim_arch::testrt::check_eq(
        write_frame(&mut out, hdr, b"large", 2).unwrap_err(),
        ErrorCode::ResourceExhausted,
    );
});

arch_test!(frame_write_frame_short_output_buffer, {
    // out.len() < HEADER_LEN + body.len() → BufferTooSmall (lines 97-99)
    let hdr = FrameHeader { body_len: 0, msg_type: 1, flags: 0, correlation_id: 0 };
    // body is 3 bytes but output buffer is only HEADER_LEN + 2 bytes.
    let mut out = [0u8; HEADER_LEN + 2];
    reovim_arch::testrt::check_eq(
        write_frame(&mut out, hdr, b"xyz", 4096).unwrap_err(),
        ErrorCode::BufferTooSmall,
    );
});

arch_test!(frame_read_frame_body_shorter_than_declared, {
    // Header declares body_len=10 but buffer only has 5 body bytes (line 147-149).
    let mut buf = [0u8; HEADER_LEN + 5];
    // Encode body_len=10 in the first 4 bytes (LE u32).
    buf[0] = 10;
    buf[1] = 0;
    buf[2] = 0;
    buf[3] = 0;
    reovim_arch::testrt::check_eq(
        read_frame(&buf, 4096).unwrap_err(),
        ErrorCode::ProtocolViolation,
    );
});

// ---------------------------------------------------------------------------
// state.rs: uncovered paths
// ---------------------------------------------------------------------------

arch_test!(state_client_role_handshake_path, {
    // Client role: first frame must be HelloAck (not Hello).
    let mut sm = ProtocolState::new(Role::Client);
    reovim_arch::testrt::check_eq(sm.phase(), Phase::AwaitingHandshake);

    let ack_hdr = FrameHeader {
        body_len: 0,
        msg_type: HelloAck::TAG,
        flags: 0,
        correlation_id: 0,
    };
    // TagInfo::Known(Handshake) — the on_handshake_frame Client arm.
    reovim_arch::testrt::check_eq(
        sm.on_frame(ack_hdr, TagInfo::Known(Direction::Handshake)),
        Action::Accept,
    );
    reovim_arch::testrt::check_eq(sm.phase(), Phase::Established);
});

arch_test!(state_client_wrong_handshake_tag, {
    // Client role receives Hello (not HelloAck) → Reject.
    let mut sm = ProtocolState::new(Role::Client);
    let hello_hdr = FrameHeader {
        body_len: 0,
        msg_type: Hello::TAG,
        flags: 0,
        correlation_id: 0,
    };
    let action = sm.on_frame(hello_hdr, TagInfo::Known(Direction::Handshake));
    reovim_arch::testrt::check(
        matches!(action, Action::Reject(RejectReason::HandshakeExpected)),
        "Client: Hello → HandshakeExpected",
    );
    reovim_arch::testrt::check_eq(sm.phase(), Phase::Closed);
});

arch_test!(state_closed_state_rejects_all, {
    // After a terminal reject, Phase::Closed; any further frame → Reject.
    let mut sm = ProtocolState::new(Role::Server);
    // Trigger close by sending non-handshake first.
    let req_hdr = FrameHeader {
        body_len: 0,
        msg_type: 0x0100,
        flags: 0,
        correlation_id: 1,
    };
    sm.on_frame(req_hdr, TagInfo::Known(Direction::Request));
    reovim_arch::testrt::check_eq(sm.phase(), Phase::Closed);
    // Next frame on closed state must still reject.
    let any_hdr = FrameHeader {
        body_len: 0,
        msg_type: Hello::TAG,
        flags: 0,
        correlation_id: 0,
    };
    let action = sm.on_frame(any_hdr, TagInfo::Known(Direction::Handshake));
    reovim_arch::testrt::check(
        matches!(action, Action::Reject(_)),
        "Closed state: any frame → Reject",
    );
});

arch_test!(state_notify_nonzero_correlation_rejected, {
    // In Established state: Notify with non-zero correlation_id → Reject.
    let mut sm = ProtocolState::new(Role::Server);
    let hello_hdr = FrameHeader {
        body_len: 0,
        msg_type: Hello::TAG,
        flags: 0,
        correlation_id: 0,
    };
    sm.on_frame(hello_hdr, TagInfo::Known(Direction::Handshake));
    let notify_hdr = FrameHeader {
        body_len: 0,
        msg_type: 0x0300,
        flags: 0,
        correlation_id: 99, // non-zero: violation SP11
    };
    let action = sm.on_frame(notify_hdr, TagInfo::Known(Direction::Notify));
    reovim_arch::testrt::check(
        matches!(action, Action::Reject(RejectReason::CorrelationViolation)),
        "Notify with non-zero correlation → CorrelationViolation",
    );
});

arch_test!(state_notify_zero_correlation_accepted, {
    // In Established state: Notify with zero correlation_id → Accept.
    let mut sm = ProtocolState::new(Role::Server);
    let hello_hdr = FrameHeader {
        body_len: 0,
        msg_type: Hello::TAG,
        flags: 0,
        correlation_id: 0,
    };
    sm.on_frame(hello_hdr, TagInfo::Known(Direction::Handshake));
    let notify_hdr = FrameHeader {
        body_len: 0,
        msg_type: 0x0300,
        flags: 0,
        correlation_id: 0,
    };
    reovim_arch::testrt::check_eq(
        sm.on_frame(notify_hdr, TagInfo::Known(Direction::Notify)),
        Action::Accept,
    );
});

arch_test!(state_response_and_error_directions_accepted, {
    // In Established state: Response and Error direction frames → Accept.
    let mut sm = ProtocolState::new(Role::Server);
    let hello_hdr = FrameHeader {
        body_len: 0,
        msg_type: Hello::TAG,
        flags: 0,
        correlation_id: 0,
    };
    sm.on_frame(hello_hdr, TagInfo::Known(Direction::Handshake));

    let resp_hdr = FrameHeader {
        body_len: 0,
        msg_type: 0x0200,
        flags: 0,
        correlation_id: 5,
    };
    reovim_arch::testrt::check_eq(
        sm.on_frame(resp_hdr, TagInfo::Known(Direction::Response)),
        Action::Accept,
    );

    let err_hdr = FrameHeader {
        body_len: 0,
        msg_type: 0xFF00,
        flags: 0,
        correlation_id: 0,
    };
    reovim_arch::testrt::check_eq(
        sm.on_frame(err_hdr, TagInfo::Known(Direction::Error)),
        Action::Accept,
    );
});

arch_test!(state_unexpected_handshake_after_established, {
    // In Established state: known Handshake direction → UnexpectedHandshake.
    let mut sm = ProtocolState::new(Role::Server);
    let hello_hdr = FrameHeader {
        body_len: 0,
        msg_type: Hello::TAG,
        flags: 0,
        correlation_id: 0,
    };
    sm.on_frame(hello_hdr, TagInfo::Known(Direction::Handshake));
    // Second handshake frame.
    let second_hello = FrameHeader {
        body_len: 0,
        msg_type: Hello::TAG,
        flags: 0,
        correlation_id: 0,
    };
    let action = sm.on_frame(second_hello, TagInfo::Known(Direction::Handshake));
    reovim_arch::testrt::check(
        matches!(action, Action::Reject(RejectReason::UnexpectedHandshake)),
        "Second handshake → UnexpectedHandshake",
    );
});

arch_test!(state_correlation_violation_on_handshake, {
    // Server AwaitingHandshake: Hello with non-zero correlation_id → CorrelationViolation.
    let mut sm = ProtocolState::new(Role::Server);
    let hello_hdr = FrameHeader {
        body_len: 0,
        msg_type: Hello::TAG,
        flags: 0,
        correlation_id: 7, // non-zero: violation
    };
    let action = sm.on_frame(hello_hdr, TagInfo::Known(Direction::Handshake));
    reovim_arch::testrt::check(
        matches!(action, Action::Reject(RejectReason::CorrelationViolation)),
        "Hello with non-zero correlation_id → CorrelationViolation",
    );
});

// ---------------------------------------------------------------------------
// Batch: encode_body zero-buffer + truncated decode for ALL non-trivial msgs
// ---------------------------------------------------------------------------
//
// Each message's `encode_body` has `?` operators whose error branches are only
// reachable by calling `encode_body` directly with a too-short buffer (the
// trait-provided `encode` guard pre-empts these in normal use).  Similarly,
// each `decode` has `?` branches reachable only when the input is truncated.
//
// This single test closes ALL those branches in one pass.
//
// Uses `check_encode_body_zero_buf` (zero-len buffer → first put_* fails) and
// direct truncated-decode calls (empty slice → first get_* fails).

arch_test!(msg_all_encode_body_and_decode_truncation, {
    // Hello/HelloAck/Attach/SendInput/AttachEventCursor/
    // AttachEventDomainTableDelta are absent here on purpose: their
    // dedicated non-empty-list tests above run the same full sweep on
    // richer values.
    let empty: [u8; 4] = [0u8; 4]; // valid empty list encoding (count = 0)

    // ── Handshake ────────────────────────────────────────────────────────────

    // ── Requests ─────────────────────────────────────────────────────────────
    {
        let msg = Detach { reason: "r" };
        check_encode_body_zero_buf(&msg);
        let mut full = [0u8; 512];
        let n = msg.encode(&mut full).expect("full encode");
        check_decode_truncation_sweep(&full, n, |b| Detach::decode(b).is_err());
    }
    {
        let msg = SwitchSession { session_name: "s" };
        check_encode_body_zero_buf(&msg);
        let mut full = [0u8; 512];
        let n = msg.encode(&mut full).expect("full encode");
        check_decode_truncation_sweep(&full, n, |b| SwitchSession::decode(b).is_err());
    }
    {
        let msg = DestroySession { session_name: "s", force: false };
        check_encode_body_zero_buf(&msg);
        let mut full = [0u8; 512];
        let n = msg.encode(&mut full).expect("full encode");
        check_decode_truncation_sweep(&full, n, |b| DestroySession::decode(b).is_err());
    }
    {
        let msg = RenameSession { old_name: "a", new_name: "b" };
        check_encode_body_zero_buf(&msg);
        let mut full = [0u8; 512];
        let n = msg.encode(&mut full).expect("full encode");
        check_decode_truncation_sweep(&full, n, |b| RenameSession::decode(b).is_err());
    }
    {
        let msg = DebugRead { op_kind: 1, payload: b"", subscribe: false };
        check_encode_body_zero_buf(&msg);
        let mut full = [0u8; 512];
        let n = msg.encode(&mut full).expect("full encode");
        check_decode_truncation_sweep(&full, n, |b| DebugRead::decode(b).is_err());
    }
    {
        let msg = DebugDrive { op_kind: 1, payload: b"" };
        check_encode_body_zero_buf(&msg);
        let mut full = [0u8; 512];
        let n = msg.encode(&mut full).expect("full encode");
        check_decode_truncation_sweep(&full, n, |b| DebugDrive::decode(b).is_err());
    }
    {
        let msg = PkgSync { targets: StrList::from_validated(&empty), dry_run: false };
        check_encode_body_zero_buf(&msg);
        let mut full = [0u8; 512];
        let n = msg.encode(&mut full).expect("full encode");
        check_decode_truncation_sweep(&full, n, |b| PkgSync::decode(b).is_err());
    }
    {
        let msg = PkgVerify { targets: StrList::from_validated(&empty) };
        check_encode_body_zero_buf(&msg);
        let mut full = [0u8; 512];
        let n = msg.encode(&mut full).expect("full encode");
        check_decode_truncation_sweep(&full, n, |b| PkgVerify::decode(b).is_err());
    }
    {
        let msg = ConfigDump { namespace: "ns", show_secrets: false };
        check_encode_body_zero_buf(&msg);
        let mut full = [0u8; 512];
        let n = msg.encode(&mut full).expect("full encode");
        check_decode_truncation_sweep(&full, n, |b| ConfigDump::decode(b).is_err());
    }
    {
        let msg = ConfigValidate { toml: b"", namespace: "ns" };
        check_encode_body_zero_buf(&msg);
        let mut full = [0u8; 512];
        let n = msg.encode(&mut full).expect("full encode");
        check_decode_truncation_sweep(&full, n, |b| ConfigValidate::decode(b).is_err());
    }

    // ── Responses ─────────────────────────────────────────────────────────────
    {
        let msg_body = {
            let mut b = [0u8; 16];
            let mut enc = Encoder::new(&mut b);
            enc.put_u64(1).unwrap();
            enc.put_u32(0).unwrap();
            let n = enc.position();
            (b, n)
        };
        let ack = AttachAck::decode(&msg_body.0[..msg_body.1]).expect("AttachAck decode");
        check_encode_body_zero_buf(&ack);
        let _ = AttachAck::decode(&[]).expect_err("AttachAck truncated");
        }
    {
        let msg = InputAck { accepted: 1, dropped: 0 };
        check_encode_body_zero_buf(&msg);
        let mut full = [0u8; 512];
        let n = msg.encode(&mut full).expect("full encode");
        check_decode_truncation_sweep(&full, n, |b| InputAck::decode(b).is_err());
    }
    {
        let msg = DebugReadEvent { op_kind: 1, payload: b"", is_final: false };
        check_encode_body_zero_buf(&msg);
        let mut full = [0u8; 512];
        let n = msg.encode(&mut full).expect("full encode");
        check_decode_truncation_sweep(&full, n, |b| DebugReadEvent::decode(b).is_err());
    }
    {
        let msg = DebugDriveAck { op_kind: 1, result: ErrorCode::Ok, payload: b"" };
        check_encode_body_zero_buf(&msg);
        let mut full = [0u8; 512];
        let n = msg.encode(&mut full).expect("full encode");
        check_decode_truncation_sweep(&full, n, |b| DebugDriveAck::decode(b).is_err());
    }
    {
        let msg = PkgSyncProgress { target: "t", done: 0, total: 1 };
        check_encode_body_zero_buf(&msg);
        let mut full = [0u8; 512];
        let n = msg.encode(&mut full).expect("full encode");
        check_decode_truncation_sweep(&full, n, |b| PkgSyncProgress::decode(b).is_err());
    }
    {
        let msg = PkgSyncDone { result: ErrorCode::Ok, summary: "ok" };
        check_encode_body_zero_buf(&msg);
        let mut full = [0u8; 512];
        let n = msg.encode(&mut full).expect("full encode");
        check_decode_truncation_sweep(&full, n, |b| PkgSyncDone::decode(b).is_err());
    }
    {
        let msg = PkgVerifyAck { result: ErrorCode::Ok, report: b"" };
        check_encode_body_zero_buf(&msg);
        let mut full = [0u8; 512];
        let n = msg.encode(&mut full).expect("full encode");
        check_decode_truncation_sweep(&full, n, |b| PkgVerifyAck::decode(b).is_err());
    }
    {
        let msg = ConfigDumpResponse { toml: b"" };
        check_encode_body_zero_buf(&msg);
        let mut full = [0u8; 512];
        let n = msg.encode(&mut full).expect("full encode");
        check_decode_truncation_sweep(&full, n, |b| ConfigDumpResponse::decode(b).is_err());
    }
    {
        let msg = ConfigValidateResponse { result: ErrorCode::Ok, detail: "ok" };
        check_encode_body_zero_buf(&msg);
        let mut full = [0u8; 512];
        let n = msg.encode(&mut full).expect("full encode");
        check_decode_truncation_sweep(&full, n, |b| ConfigValidateResponse::decode(b).is_err());
    }

    // ── Notifications ──────────────────────────────────────────────────────────
    {
        let msg = AttachEventFrame { buffer_id: 0, window_id: 0, frame: b"" };
        check_encode_body_zero_buf(&msg);
        let mut full = [0u8; 512];
        let n = msg.encode(&mut full).expect("full encode");
        check_decode_truncation_sweep(&full, n, |b| AttachEventFrame::decode(b).is_err());
    }
    {
        let msg = AttachEventDiff { buffer_id: 0, window_id: 0, diff: b"" };
        check_encode_body_zero_buf(&msg);
        let mut full = [0u8; 512];
        let n = msg.encode(&mut full).expect("full encode");
        check_decode_truncation_sweep(&full, n, |b| AttachEventDiff::decode(b).is_err());
    }
    {
        let msg = AttachEventProjection { buffer_id: 0, projection: b"" };
        check_encode_body_zero_buf(&msg);
        let mut full = [0u8; 512];
        let n = msg.encode(&mut full).expect("full encode");
        check_decode_truncation_sweep(&full, n, |b| AttachEventProjection::decode(b).is_err());
    }
    {
        let msg = AttachEventSessionPivot { session_name: "s" };
        check_encode_body_zero_buf(&msg);
        let mut full = [0u8; 512];
        let n = msg.encode(&mut full).expect("full encode");
        check_decode_truncation_sweep(&full, n, |b| AttachEventSessionPivot::decode(b).is_err());
    }
    {
        let msg = AttachEventSessionDestroyed { session_name: "s" };
        check_encode_body_zero_buf(&msg);
        let mut full = [0u8; 512];
        let n = msg.encode(&mut full).expect("full encode");
        check_decode_truncation_sweep(&full, n, |b| AttachEventSessionDestroyed::decode(b).is_err());
    }
    {
        let msg = AttachEventClientLeft { client_id: 0, reason: "r" };
        check_encode_body_zero_buf(&msg);
        let mut full = [0u8; 512];
        let n = msg.encode(&mut full).expect("full encode");
        check_decode_truncation_sweep(&full, n, |b| AttachEventClientLeft::decode(b).is_err());
    }
    {
        let msg = AttachEventServerDraining { grace_ms: 1000 };
        check_encode_body_zero_buf(&msg);
        let mut full = [0u8; 512];
        let n = msg.encode(&mut full).expect("full encode");
        check_decode_truncation_sweep(&full, n, |b| AttachEventServerDraining::decode(b).is_err());
    }

    // ── Error ──────────────────────────────────────────────────────────────────
    {
        let msg = Reject { code: ErrorCode::Ok, detail: "e" };
        check_encode_body_zero_buf(&msg);
        let mut full = [0u8; 512];
        let n = msg.encode(&mut full).expect("full encode");
        check_decode_truncation_sweep(&full, n, |b| Reject::decode(b).is_err());
    }
});

// Established-phase request with correlation id 0 is accepted: §7.5 allows
// uncorrelated SendInput, so the machine cannot reject id-0 requests.
arch_test!(state_request_zero_correlation_accepted, {
    use reovim_uapi_abi::FrameHeader;
    use reovim_uapi_protocol::{
        messages::{Direction, Hello},
        state::{Action, ProtocolState, Role, TagInfo},
    };
    let mut sm = ProtocolState::new(Role::Server);
    let hello = FrameHeader { body_len: 0, msg_type: Hello::TAG, flags: 0, correlation_id: 0 };
    reovim_arch::testrt::check_eq(sm.on_frame(hello, TagInfo::Known(Direction::Handshake)), Action::Accept);
    let req = FrameHeader { body_len: 0, msg_type: 0x0100, flags: 0, correlation_id: 0 };
    reovim_arch::testrt::check_eq(sm.on_frame(req, TagInfo::Known(Direction::Request)), Action::Accept);
});

// The iterator size_hint contract on every list view: exact (left, Some(left))
// before and during traversal.
arch_test!(view_iterators_report_exact_size_hint, {
    let mut caps_buf = [0u8; 64];
    let caps = str_list_two!(caps_buf, "a", "b");
    let mut it = caps.iter();
    reovim_arch::testrt::check_eq(it.size_hint(), (2, Some(2usize)));
    let _ = it.next();
    reovim_arch::testrt::check_eq(it.size_hint(), (1, Some(1usize)));
});
