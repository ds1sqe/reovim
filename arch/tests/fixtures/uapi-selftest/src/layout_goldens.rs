//! ABI layout golden tests migrated to the no_std selftest runner (#786 Phase 5).
//!
//! Mirrors `uapi/abi/tests/layout_goldens.rs` — every test body is pure
//! `core::mem` assertions over `reovim_uapi_abi` types; no std needed.
//!
//! The libtest originals in `uapi/abi/tests/layout_goldens.rs` remain as the
//! bootstrap-state-1 mirror; these selftest registrations are the
//! flight-environment authority.
//!
//! Spec citations are preserved verbatim from the originals.

use core::mem::{align_of, offset_of, size_of};

use reovim_arch::arch_test;

// ---------------------------------------------------------------------------
// 6.3 §2.1 — Versions + identifiers
// ---------------------------------------------------------------------------

arch_test!(layout_abi_version, {
    use reovim_uapi_abi::ids::AbiVersion;
    reovim_arch::testrt::check_eq(size_of::<AbiVersion>(), 8);
    reovim_arch::testrt::check_eq(align_of::<AbiVersion>(), 2);
    reovim_arch::testrt::check_eq(offset_of!(AbiVersion, major), 0);
    reovim_arch::testrt::check_eq(offset_of!(AbiVersion, minor), 2);
    reovim_arch::testrt::check_eq(offset_of!(AbiVersion, patch), 4);
    reovim_arch::testrt::check_eq(offset_of!(AbiVersion, pad), 6);
});

arch_test!(layout_version, {
    use reovim_uapi_abi::ids::Version;
    reovim_arch::testrt::check_eq(size_of::<Version>(), 8);
    reovim_arch::testrt::check_eq(align_of::<Version>(), 2);
    reovim_arch::testrt::check_eq(offset_of!(Version, major), 0);
    reovim_arch::testrt::check_eq(offset_of!(Version, minor), 2);
    reovim_arch::testrt::check_eq(offset_of!(Version, patch), 4);
    reovim_arch::testrt::check_eq(offset_of!(Version, pad), 6);
});

arch_test!(layout_cdylib_id, {
    use reovim_uapi_abi::ids::CdylibId;
    reovim_arch::testrt::check_eq(size_of::<CdylibId>(), 4);
    reovim_arch::testrt::check_eq(align_of::<CdylibId>(), 4);
});

arch_test!(layout_domain_id, {
    use reovim_uapi_abi::ids::DomainId;
    reovim_arch::testrt::check_eq(size_of::<DomainId>(), 4);
    reovim_arch::testrt::check_eq(align_of::<DomainId>(), 4);
});

arch_test!(layout_client_id, {
    use reovim_uapi_abi::ids::ClientId;
    reovim_arch::testrt::check_eq(size_of::<ClientId>(), size_of::<usize>());
    reovim_arch::testrt::check_eq(align_of::<ClientId>(), align_of::<usize>());
});

arch_test!(layout_buffer_id, {
    use reovim_uapi_abi::ids::BufferId;
    reovim_arch::testrt::check_eq(size_of::<BufferId>(), size_of::<usize>());
    reovim_arch::testrt::check_eq(align_of::<BufferId>(), align_of::<usize>());
});

arch_test!(layout_window_id, {
    use reovim_uapi_abi::ids::WindowId;
    reovim_arch::testrt::check_eq(size_of::<WindowId>(), size_of::<usize>());
    reovim_arch::testrt::check_eq(align_of::<WindowId>(), align_of::<usize>());
});

arch_test!(layout_domain_attachment_id, {
    use reovim_uapi_abi::ids::DomainAttachmentId;
    reovim_arch::testrt::check_eq(size_of::<DomainAttachmentId>(), 8);
    reovim_arch::testrt::check_eq(align_of::<DomainAttachmentId>(), 8);
});

arch_test!(layout_pending_attachment_id, {
    use reovim_uapi_abi::ids::PendingAttachmentId;
    reovim_arch::testrt::check_eq(size_of::<PendingAttachmentId>(), 8);
    reovim_arch::testrt::check_eq(align_of::<PendingAttachmentId>(), 8);
});

arch_test!(layout_undo_group_id, {
    use reovim_uapi_abi::ids::UndoGroupId;
    reovim_arch::testrt::check_eq(size_of::<UndoGroupId>(), 8);
    reovim_arch::testrt::check_eq(align_of::<UndoGroupId>(), 8);
});

arch_test!(layout_service_key, {
    use reovim_uapi_abi::ids::ServiceKey;
    reovim_arch::testrt::check_eq(size_of::<ServiceKey>(), 4);
    reovim_arch::testrt::check_eq(align_of::<ServiceKey>(), 4);
});

// ---------------------------------------------------------------------------
// 6.3 §2.2 — Byte-slice types
// ---------------------------------------------------------------------------

arch_test!(layout_byte_slice, {
    use reovim_uapi_abi::slices::ByteSlice;
    let expected_size = 2 * size_of::<usize>();
    let expected_align = align_of::<usize>();
    reovim_arch::testrt::check_eq(size_of::<ByteSlice>(), expected_size);
    reovim_arch::testrt::check_eq(align_of::<ByteSlice>(), expected_align);
    reovim_arch::testrt::check_eq(offset_of!(ByteSlice, data), 0);
    reovim_arch::testrt::check_eq(offset_of!(ByteSlice, len), size_of::<usize>());
});

arch_test!(layout_byte_slice_mut, {
    use reovim_uapi_abi::slices::ByteSliceMut;
    let expected_size = 2 * size_of::<usize>();
    let expected_align = align_of::<usize>();
    reovim_arch::testrt::check_eq(size_of::<ByteSliceMut>(), expected_size);
    reovim_arch::testrt::check_eq(align_of::<ByteSliceMut>(), expected_align);
    reovim_arch::testrt::check_eq(offset_of!(ByteSliceMut, data), 0);
    reovim_arch::testrt::check_eq(offset_of!(ByteSliceMut, len), size_of::<usize>());
});

arch_test!(layout_str_slice, {
    use reovim_uapi_abi::slices::{ByteSlice, StrSlice};
    reovim_arch::testrt::check_eq(size_of::<StrSlice>(), size_of::<ByteSlice>());
    reovim_arch::testrt::check_eq(align_of::<StrSlice>(), align_of::<ByteSlice>());
    reovim_arch::testrt::check_eq(offset_of!(StrSlice, data), 0);
    reovim_arch::testrt::check_eq(offset_of!(StrSlice, len), size_of::<usize>());
});

// ---------------------------------------------------------------------------
// 6.3 §2.3 — ErrorCode discriminants
// ---------------------------------------------------------------------------

arch_test!(layout_error_code_discriminants, {
    use reovim_uapi_abi::error::ErrorCode;
    reovim_arch::testrt::check_eq(size_of::<ErrorCode>(), 4);
    reovim_arch::testrt::check_eq(align_of::<ErrorCode>(), 4);
    reovim_arch::testrt::check_eq(ErrorCode::Ok as i32, 0);
    reovim_arch::testrt::check_eq(ErrorCode::Generic as i32, 1);
    reovim_arch::testrt::check_eq(ErrorCode::IncompatibleAbi as i32, 2);
    reovim_arch::testrt::check_eq(ErrorCode::IncompatibleApi as i32, 3);
    reovim_arch::testrt::check_eq(ErrorCode::NotFound as i32, 4);
    reovim_arch::testrt::check_eq(ErrorCode::Conflict as i32, 5);
    reovim_arch::testrt::check_eq(ErrorCode::InvalidArgument as i32, 6);
    reovim_arch::testrt::check_eq(ErrorCode::ResourceExhausted as i32, 7);
    reovim_arch::testrt::check_eq(ErrorCode::ProtocolViolation as i32, 8);
    reovim_arch::testrt::check_eq(ErrorCode::Busy as i32, 9);
    reovim_arch::testrt::check_eq(ErrorCode::Stale as i32, 10);
    reovim_arch::testrt::check_eq(ErrorCode::PermissionDenied as i32, 11);
    reovim_arch::testrt::check_eq(ErrorCode::Panic as i32, 12);
    reovim_arch::testrt::check_eq(ErrorCode::Cancelled as i32, 13);
    reovim_arch::testrt::check_eq(ErrorCode::Timeout as i32, 14);
    reovim_arch::testrt::check_eq(ErrorCode::SchemaInvalid as i32, 15);
    reovim_arch::testrt::check_eq(ErrorCode::NamespaceConflict as i32, 16);
    reovim_arch::testrt::check_eq(ErrorCode::IllegalTrustClass as i32, 17);
    reovim_arch::testrt::check_eq(ErrorCode::ConfigSliceTooLarge as i32, 18);
    reovim_arch::testrt::check_eq(ErrorCode::Utf8Invalid as i32, 19);
    reovim_arch::testrt::check_eq(ErrorCode::IllegalProjectHostSection as i32, 20);
    reovim_arch::testrt::check_eq(ErrorCode::IllegalKind as i32, 21);
    reovim_arch::testrt::check_eq(ErrorCode::ShortVtable as i32, 22);
    reovim_arch::testrt::check_eq(ErrorCode::CodecGone as i32, 23);
    reovim_arch::testrt::check_eq(ErrorCode::NotActive as i32, 24);
    reovim_arch::testrt::check_eq(ErrorCode::RollbackFailed as i32, 25);
    // SP13 key: BufferTooSmall = 26.
    reovim_arch::testrt::check_eq(ErrorCode::BufferTooSmall as i32, 26);
});

// ---------------------------------------------------------------------------
// 6.3 §2.4 — LogLevel
// ---------------------------------------------------------------------------

arch_test!(layout_log_level_discriminants, {
    use reovim_uapi_abi::error::LogLevel;
    reovim_arch::testrt::check_eq(size_of::<LogLevel>(), 1);
    reovim_arch::testrt::check_eq(align_of::<LogLevel>(), 1);
    reovim_arch::testrt::check_eq(LogLevel::Unknown as u8, 0);
    reovim_arch::testrt::check_eq(LogLevel::Trace as u8, 1);
    reovim_arch::testrt::check_eq(LogLevel::Debug as u8, 2);
    reovim_arch::testrt::check_eq(LogLevel::Info as u8, 3);
    reovim_arch::testrt::check_eq(LogLevel::Warn as u8, 4);
    reovim_arch::testrt::check_eq(LogLevel::Error as u8, 5);
});

// ---------------------------------------------------------------------------
// 6.3 §3 — VtableHeader + ManifestKind
// ---------------------------------------------------------------------------

arch_test!(layout_vtable_header, {
    use reovim_uapi_abi::vtable::VtableHeader;
    reovim_arch::testrt::check_eq(size_of::<VtableHeader>(), 32);
    reovim_arch::testrt::check_eq(align_of::<VtableHeader>(), 8);
    reovim_arch::testrt::check_eq(offset_of!(VtableHeader, abi), 0);
    reovim_arch::testrt::check_eq(offset_of!(VtableHeader, api), 8);
    reovim_arch::testrt::check_eq(offset_of!(VtableHeader, size_of_self), 16);
    reovim_arch::testrt::check_eq(offset_of!(VtableHeader, kind), 24);
    reovim_arch::testrt::check_eq(offset_of!(VtableHeader, flags), 28);
});

arch_test!(layout_manifest_kind_discriminants, {
    use reovim_uapi_abi::vtable::ManifestKind;
    reovim_arch::testrt::check_eq(size_of::<ManifestKind>(), 1);
    reovim_arch::testrt::check_eq(align_of::<ManifestKind>(), 1);
    reovim_arch::testrt::check_eq(ManifestKind::Unknown as u8, 0);
    reovim_arch::testrt::check_eq(ManifestKind::ModuleServer as u8, 1);
    reovim_arch::testrt::check_eq(ManifestKind::DriverServer as u8, 2);
    reovim_arch::testrt::check_eq(ManifestKind::ModuleClient as u8, 3);
    reovim_arch::testrt::check_eq(ManifestKind::DriverClient as u8, 4);
    reovim_arch::testrt::check_eq(ManifestKind::CapabilityClient as u8, 5);
    reovim_arch::testrt::check_eq(ManifestKind::DomainServer as u8, 6);
    reovim_arch::testrt::check_eq(ManifestKind::ProviderServer as u8, 7);
    reovim_arch::testrt::check_eq(ManifestKind::StreamScheme as u8, 8);
});

// ---------------------------------------------------------------------------
// 6.3 §4 / 6.4 — Config types
// ---------------------------------------------------------------------------

arch_test!(layout_config_value_kind_discriminants, {
    use reovim_uapi_abi::config::ConfigValueKind;
    reovim_arch::testrt::check_eq(size_of::<ConfigValueKind>(), 1);
    reovim_arch::testrt::check_eq(align_of::<ConfigValueKind>(), 1);
    reovim_arch::testrt::check_eq(ConfigValueKind::Unknown as u8, 0);
    reovim_arch::testrt::check_eq(ConfigValueKind::Bool as u8, 1);
    reovim_arch::testrt::check_eq(ConfigValueKind::U32 as u8, 2);
    reovim_arch::testrt::check_eq(ConfigValueKind::I32 as u8, 3);
    reovim_arch::testrt::check_eq(ConfigValueKind::U64 as u8, 4);
    reovim_arch::testrt::check_eq(ConfigValueKind::I64 as u8, 5);
    reovim_arch::testrt::check_eq(ConfigValueKind::F64 as u8, 6);
    reovim_arch::testrt::check_eq(ConfigValueKind::String as u8, 7);
    reovim_arch::testrt::check_eq(ConfigValueKind::Path as u8, 8);
    reovim_arch::testrt::check_eq(ConfigValueKind::Enum as u8, 9);
    reovim_arch::testrt::check_eq(ConfigValueKind::CanonicalToml as u8, 10);
});

arch_test!(layout_config_source_discriminants, {
    use reovim_uapi_abi::config::ConfigSource;
    reovim_arch::testrt::check_eq(size_of::<ConfigSource>(), 1);
    reovim_arch::testrt::check_eq(align_of::<ConfigSource>(), 1);
    reovim_arch::testrt::check_eq(ConfigSource::Default as u8, 0);
    reovim_arch::testrt::check_eq(ConfigSource::System as u8, 1);
    reovim_arch::testrt::check_eq(ConfigSource::User as u8, 2);
    reovim_arch::testrt::check_eq(ConfigSource::Project as u8, 3);
    reovim_arch::testrt::check_eq(ConfigSource::Env as u8, 4);
    reovim_arch::testrt::check_eq(ConfigSource::Cli as u8, 5);
    reovim_arch::testrt::check_eq(ConfigSource::Force as u8, 6);
});

arch_test!(layout_config_value, {
    use reovim_uapi_abi::config::ConfigValue;
    // Widest member is ByteSlice (2×usize = 16 on 64-bit).
    reovim_arch::testrt::check_eq(size_of::<ConfigValue>(), 16);
    reovim_arch::testrt::check_eq(align_of::<ConfigValue>(), 8);
});

// ---------------------------------------------------------------------------
// 6.3 §5 — Coordination carriers
// ---------------------------------------------------------------------------

arch_test!(layout_position_header, {
    use reovim_uapi_abi::coordination::PositionHeader;
    reovim_arch::testrt::check_eq(size_of::<PositionHeader>(), 8);
    reovim_arch::testrt::check_eq(align_of::<PositionHeader>(), 1);
    reovim_arch::testrt::check_eq(offset_of!(PositionHeader, bytes), 0);
});

arch_test!(layout_cursor_header, {
    use reovim_uapi_abi::coordination::CursorHeader;
    reovim_arch::testrt::check_eq(size_of::<CursorHeader>(), 8);
    reovim_arch::testrt::check_eq(align_of::<CursorHeader>(), 1);
    reovim_arch::testrt::check_eq(offset_of!(CursorHeader, bytes), 0);
});

arch_test!(layout_position_carrier_wire, {
    use reovim_uapi_abi::coordination::PositionCarrierWire;
    reovim_arch::testrt::check_eq(size_of::<PositionCarrierWire>(), 24);
    reovim_arch::testrt::check_eq(align_of::<PositionCarrierWire>(), 8);
    reovim_arch::testrt::check_eq(offset_of!(PositionCarrierWire, header), 0);
    reovim_arch::testrt::check_eq(offset_of!(PositionCarrierWire, content), 8);
});

arch_test!(layout_cursor_carrier_wire, {
    use reovim_uapi_abi::coordination::CursorCarrierWire;
    reovim_arch::testrt::check_eq(size_of::<CursorCarrierWire>(), 24);
    reovim_arch::testrt::check_eq(align_of::<CursorCarrierWire>(), 8);
    reovim_arch::testrt::check_eq(offset_of!(CursorCarrierWire, header), 0);
    reovim_arch::testrt::check_eq(offset_of!(CursorCarrierWire, content), 8);
});

// ---------------------------------------------------------------------------
// 6.3 §7 — RawInput + per-kind payloads
// ---------------------------------------------------------------------------

arch_test!(layout_raw_input_kind_discriminants, {
    use reovim_uapi_abi::input::RawInputKind;
    reovim_arch::testrt::check_eq(size_of::<RawInputKind>(), 1);
    reovim_arch::testrt::check_eq(align_of::<RawInputKind>(), 1);
    reovim_arch::testrt::check_eq(RawInputKind::Key as u8, 1);
    reovim_arch::testrt::check_eq(RawInputKind::Mouse as u8, 2);
    reovim_arch::testrt::check_eq(RawInputKind::Text as u8, 3);
    reovim_arch::testrt::check_eq(RawInputKind::Paste as u8, 4);
    reovim_arch::testrt::check_eq(RawInputKind::Web as u8, 5);
    reovim_arch::testrt::check_eq(RawInputKind::Trigger as u8, 6);
    reovim_arch::testrt::check_eq(RawInputKind::Ime as u8, 7);
});

arch_test!(layout_key_event, {
    use reovim_uapi_abi::input::KeyEvent;
    reovim_arch::testrt::check_eq(size_of::<KeyEvent>(), 24);
    reovim_arch::testrt::check_eq(align_of::<KeyEvent>(), 8);
    reovim_arch::testrt::check_eq(offset_of!(KeyEvent, keycode), 0);
    reovim_arch::testrt::check_eq(offset_of!(KeyEvent, mods), 4);
    reovim_arch::testrt::check_eq(offset_of!(KeyEvent, action), 5);
    reovim_arch::testrt::check_eq(offset_of!(KeyEvent, pad), 6);
    reovim_arch::testrt::check_eq(offset_of!(KeyEvent, timestamp_ns), 8);
    reovim_arch::testrt::check_eq(offset_of!(KeyEvent, utf8), 16);
});

arch_test!(layout_mouse_event, {
    use reovim_uapi_abi::input::MouseEvent;
    reovim_arch::testrt::check_eq(size_of::<MouseEvent>(), 32);
    reovim_arch::testrt::check_eq(align_of::<MouseEvent>(), 8);
    reovim_arch::testrt::check_eq(offset_of!(MouseEvent, action), 0);
    reovim_arch::testrt::check_eq(offset_of!(MouseEvent, button), 1);
    reovim_arch::testrt::check_eq(offset_of!(MouseEvent, mods), 2);
    reovim_arch::testrt::check_eq(offset_of!(MouseEvent, flags), 3);
    reovim_arch::testrt::check_eq(offset_of!(MouseEvent, col), 4);
    reovim_arch::testrt::check_eq(offset_of!(MouseEvent, row), 8);
    reovim_arch::testrt::check_eq(offset_of!(MouseEvent, px_x), 12);
    reovim_arch::testrt::check_eq(offset_of!(MouseEvent, px_y), 16);
    reovim_arch::testrt::check_eq(offset_of!(MouseEvent, scroll_x), 20);
    reovim_arch::testrt::check_eq(offset_of!(MouseEvent, scroll_y), 22);
    reovim_arch::testrt::check_eq(offset_of!(MouseEvent, timestamp_ns), 24);
});

arch_test!(layout_trigger_event, {
    use reovim_uapi_abi::input::TriggerEvent;
    reovim_arch::testrt::check_eq(size_of::<TriggerEvent>(), 16);
    reovim_arch::testrt::check_eq(align_of::<TriggerEvent>(), 8);
    reovim_arch::testrt::check_eq(offset_of!(TriggerEvent, trigger_id), 0);
    reovim_arch::testrt::check_eq(offset_of!(TriggerEvent, pad), 4);
    reovim_arch::testrt::check_eq(offset_of!(TriggerEvent, timestamp_ns), 8);
});

arch_test!(layout_ime_event, {
    use reovim_uapi_abi::input::ImeEvent;
    reovim_arch::testrt::check_eq(size_of::<ImeEvent>(), 16);
    reovim_arch::testrt::check_eq(align_of::<ImeEvent>(), 8);
    reovim_arch::testrt::check_eq(offset_of!(ImeEvent, phase), 0);
    reovim_arch::testrt::check_eq(offset_of!(ImeEvent, pad), 1);
    reovim_arch::testrt::check_eq(offset_of!(ImeEvent, caret_byte), 4);
    reovim_arch::testrt::check_eq(offset_of!(ImeEvent, timestamp_ns), 8);
});

// ---------------------------------------------------------------------------
// 6.3 §8 — Stream substrate
// ---------------------------------------------------------------------------

arch_test!(layout_stream_id, {
    use reovim_uapi_abi::stream::StreamId;
    reovim_arch::testrt::check_eq(size_of::<StreamId>(), 8);
    reovim_arch::testrt::check_eq(align_of::<StreamId>(), 8);
});

arch_test!(layout_stream_state_discriminants, {
    use reovim_uapi_abi::stream::StreamState;
    reovim_arch::testrt::check_eq(size_of::<StreamState>(), 1);
    reovim_arch::testrt::check_eq(align_of::<StreamState>(), 1);
    reovim_arch::testrt::check_eq(StreamState::Init as u8, 0);
    reovim_arch::testrt::check_eq(StreamState::Running as u8, 1);
    reovim_arch::testrt::check_eq(StreamState::BackpressureBlocked as u8, 2);
    reovim_arch::testrt::check_eq(StreamState::Stale as u8, 3);
    reovim_arch::testrt::check_eq(StreamState::Draining as u8, 4);
    reovim_arch::testrt::check_eq(StreamState::Closed as u8, 5);
});

arch_test!(layout_stream_handle_info, {
    use reovim_uapi_abi::stream::StreamHandleInfo;
    reovim_arch::testrt::check_eq(size_of::<StreamHandleInfo>(), 32);
    reovim_arch::testrt::check_eq(align_of::<StreamHandleInfo>(), 8);
    reovim_arch::testrt::check_eq(offset_of!(StreamHandleInfo, id), 0);
    reovim_arch::testrt::check_eq(offset_of!(StreamHandleInfo, state), 8);
    reovim_arch::testrt::check_eq(offset_of!(StreamHandleInfo, bytes_in_flight), 16);
    reovim_arch::testrt::check_eq(offset_of!(StreamHandleInfo, bytes_buffered), 24);
});

// ---------------------------------------------------------------------------
// 6.3 §12 — FrameHeader
// ---------------------------------------------------------------------------

arch_test!(layout_frame_header, {
    use reovim_uapi_abi::frame::FrameHeader;
    reovim_arch::testrt::check_eq(size_of::<FrameHeader>(), 16);
    reovim_arch::testrt::check_eq(align_of::<FrameHeader>(), 8);
    reovim_arch::testrt::check_eq(offset_of!(FrameHeader, body_len), 0);
    reovim_arch::testrt::check_eq(offset_of!(FrameHeader, msg_type), 4);
    reovim_arch::testrt::check_eq(offset_of!(FrameHeader, flags), 6);
    reovim_arch::testrt::check_eq(offset_of!(FrameHeader, correlation_id), 8);
});

arch_test!(layout_frame_header_wire_bytes_golden, {
    use reovim_uapi_abi::frame::FrameHeader;
    let hdr = FrameHeader {
        body_len: 41,
        msg_type: 0x0001,
        flags: 0,
        correlation_id: 0,
    };
    let mut buf = [0u8; 16];
    hdr.encode(&mut buf);
    // Expected per 7.3 §"worked frame" (spec-exact golden):
    let expected: [u8; 16] = [
        0x29, 0x00, 0x00, 0x00, // body_len = 41 LE
        0x01, 0x00,             // msg_type = 0x0001 LE
        0x00, 0x00,             // flags = 0
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // correlation_id = 0
    ];
    reovim_arch::testrt::check(buf == expected, "6.3 §12 wire golden: FrameHeader 16-byte LE");
    // Round-trip decode.
    let back = FrameHeader::decode(&buf);
    reovim_arch::testrt::check(back == hdr, "6.3 §12: FrameHeader round-trip");
});
