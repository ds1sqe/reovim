//! ABI layout golden tests — Phase 3 (plan 05-uapi-foundation.md §Phase 3).
//!
//! For every type in the 6.3 / 6.2 / 6.4 catalog this file asserts:
//! - `size_of<T>()` against the spec-documented value.
//! - `align_of<T>()` against the spec-documented value.
//! - `offset_of!(T, field)` against the spec-documented table offset.
//!
//! Mechanism: `core::mem::offset_of!` (stable since Rust 1.77 / confirmed on
//! the installed nightly; gap-4 countdown pin).  No unsafe needed.
//!
//! The spec tables are the truth.  If an assert fails the code has drifted from
//! the spec — do NOT adjust the test; fix the catalog type.
//!
//! **Citation per type**: the spec section that owns the layout is noted in
//! each sub-section header.
//!
//! All assertions are plain `assert_eq!` over constants so rustc evaluates them
//! at compile time where possible.

use core::mem::{align_of, offset_of, size_of};

// ---------------------------------------------------------------------------
// 6.3 §2.1 — Versions + identifiers
// ---------------------------------------------------------------------------

/// `AbiVersion` (6.3 §2.1): four `u16` fields, natural `#[repr(C)]` layout.
///
/// Spec: size = 4 × 2 = 8 bytes, align = 2 bytes (all `u16` fields).
#[test]
fn abi_version_layout() {
    use reovim_uapi_abi::ids::AbiVersion;

    assert_eq!(size_of::<AbiVersion>(), 8, "6.3 §2.1: AbiVersion size");
    assert_eq!(align_of::<AbiVersion>(), 2, "6.3 §2.1: AbiVersion align");
    // Field offsets:
    assert_eq!(offset_of!(AbiVersion, major), 0, "AbiVersion.major offset");
    assert_eq!(offset_of!(AbiVersion, minor), 2, "AbiVersion.minor offset");
    assert_eq!(offset_of!(AbiVersion, patch), 4, "AbiVersion.patch offset");
    assert_eq!(offset_of!(AbiVersion, pad), 6, "AbiVersion.pad offset");
}

/// `Version` (6.3 §2.1 — ApiVersion): same layout as `AbiVersion`.
///
/// Spec: size = 8 bytes, align = 2 bytes.
#[test]
fn version_layout() {
    use reovim_uapi_abi::ids::Version;

    assert_eq!(size_of::<Version>(), 8, "6.3 §2.1: Version size");
    assert_eq!(align_of::<Version>(), 2, "6.3 §2.1: Version align");
    assert_eq!(offset_of!(Version, major), 0, "Version.major offset");
    assert_eq!(offset_of!(Version, minor), 2, "Version.minor offset");
    assert_eq!(offset_of!(Version, patch), 4, "Version.patch offset");
    assert_eq!(offset_of!(Version, pad), 6, "Version.pad offset");
}

/// `CdylibId` (6.3 §2.1 — `#[repr(transparent)]` over `NonZeroU32`).
///
/// Spec: size = 4 bytes, align = 4 bytes.
#[test]
fn cdylib_id_layout() {
    use reovim_uapi_abi::ids::CdylibId;

    assert_eq!(size_of::<CdylibId>(), 4, "6.3 §2.1: CdylibId size");
    assert_eq!(align_of::<CdylibId>(), 4, "6.3 §2.1: CdylibId align");
}

/// `DomainId` (6.3 §2.1 — `#[repr(transparent)]` over `NonZeroU32`).
///
/// Spec: size = 4 bytes, align = 4 bytes.
#[test]
fn domain_id_layout() {
    use reovim_uapi_abi::ids::DomainId;

    assert_eq!(size_of::<DomainId>(), 4, "6.3 §2.1: DomainId size");
    assert_eq!(align_of::<DomainId>(), 4, "6.3 §2.1: DomainId align");
}

/// `ClientId` (6.3 §2.1 — `#[repr(transparent)]` over `usize`).
///
/// Spec: size = `sizeof(usize)`, align = `alignof(usize)`.
/// On `x86_64`-linux: size = 8, align = 8.
#[test]
fn client_id_layout() {
    use reovim_uapi_abi::ids::ClientId;

    // transparent over usize
    assert_eq!(size_of::<ClientId>(), size_of::<usize>(), "6.3 §2.1: ClientId size");
    assert_eq!(align_of::<ClientId>(), align_of::<usize>(), "6.3 §2.1: ClientId align");
}

/// `BufferId` (6.3 §2.1 — `#[repr(transparent)]` over `usize`).
#[test]
fn buffer_id_layout() {
    use reovim_uapi_abi::ids::BufferId;

    assert_eq!(size_of::<BufferId>(), size_of::<usize>(), "6.3 §2.1: BufferId size");
    assert_eq!(align_of::<BufferId>(), align_of::<usize>(), "6.3 §2.1: BufferId align");
}

/// `WindowId` (6.3 §2.1 — `#[repr(transparent)]` over `usize`).
#[test]
fn window_id_layout() {
    use reovim_uapi_abi::ids::WindowId;

    assert_eq!(size_of::<WindowId>(), size_of::<usize>(), "6.3 §2.1: WindowId size");
    assert_eq!(align_of::<WindowId>(), align_of::<usize>(), "6.3 §2.1: WindowId align");
}

/// `DomainAttachmentId` (6.3 §2.1 — `#[repr(transparent)]` over `u64`).
///
/// Spec: size = 8, align = 8.
#[test]
fn domain_attachment_id_layout() {
    use reovim_uapi_abi::ids::DomainAttachmentId;

    assert_eq!(size_of::<DomainAttachmentId>(), 8, "6.3 §2.1: DomainAttachmentId size");
    assert_eq!(align_of::<DomainAttachmentId>(), 8, "6.3 §2.1: DomainAttachmentId align");
}

/// `PendingAttachmentId` (6.3 §2.1 — `#[repr(transparent)]` over `u64`).
///
/// Spec: size = 8, align = 8.
#[test]
fn pending_attachment_id_layout() {
    use reovim_uapi_abi::ids::PendingAttachmentId;

    assert_eq!(size_of::<PendingAttachmentId>(), 8, "6.3 §2.1: PendingAttachmentId size");
    assert_eq!(align_of::<PendingAttachmentId>(), 8, "6.3 §2.1: PendingAttachmentId align");
}

/// `UndoGroupId` (6.3 §2.1 — `#[repr(transparent)]` over `u64`).
///
/// Spec: size = 8, align = 8.
#[test]
fn undo_group_id_layout() {
    use reovim_uapi_abi::ids::UndoGroupId;

    assert_eq!(size_of::<UndoGroupId>(), 8, "6.3 §2.1: UndoGroupId size");
    assert_eq!(align_of::<UndoGroupId>(), 8, "6.3 §2.1: UndoGroupId align");
}

/// `ServiceKey` (6.3 §2.1 — `#[repr(transparent)]` over `NonZeroU32`).
///
/// Spec: size = 4, align = 4.
#[test]
fn service_key_layout() {
    use reovim_uapi_abi::ids::ServiceKey;

    assert_eq!(size_of::<ServiceKey>(), 4, "6.3 §2.1: ServiceKey size");
    assert_eq!(align_of::<ServiceKey>(), 4, "6.3 §2.1: ServiceKey align");
}

// ---------------------------------------------------------------------------
// 6.3 §2.2 — Byte-slice types
// ---------------------------------------------------------------------------

/// `ByteSlice` (6.3 §2.2 — `*const u8` + `usize`).
///
/// Spec: size = 2×sizeof(usize), align = alignof(usize).
/// On `x86_64`-linux: size = 16, align = 8.
#[test]
fn byte_slice_layout() {
    use reovim_uapi_abi::slices::ByteSlice;

    let expected_size = 2 * size_of::<usize>();
    let expected_align = align_of::<usize>();
    assert_eq!(size_of::<ByteSlice>(), expected_size, "6.3 §2.2: ByteSlice size");
    assert_eq!(align_of::<ByteSlice>(), expected_align, "6.3 §2.2: ByteSlice align");
    assert_eq!(offset_of!(ByteSlice, data), 0, "ByteSlice.data offset");
    assert_eq!(offset_of!(ByteSlice, len), size_of::<usize>(), "ByteSlice.len offset");
}

/// `ByteSliceMut` (6.3 §2.2 — `*mut u8` + `usize`).
///
/// Spec: same layout as `ByteSlice`.
#[test]
fn byte_slice_mut_layout() {
    use reovim_uapi_abi::slices::ByteSliceMut;

    let expected_size = 2 * size_of::<usize>();
    let expected_align = align_of::<usize>();
    assert_eq!(size_of::<ByteSliceMut>(), expected_size, "6.3 §2.2: ByteSliceMut size");
    assert_eq!(align_of::<ByteSliceMut>(), expected_align, "6.3 §2.2: ByteSliceMut align");
    assert_eq!(offset_of!(ByteSliceMut, data), 0, "ByteSliceMut.data offset");
    assert_eq!(offset_of!(ByteSliceMut, len), size_of::<usize>(), "ByteSliceMut.len offset");
}

/// `StrSlice` (6.3 §2.2 — identical wire layout to `ByteSlice`).
#[test]
fn str_slice_layout() {
    use reovim_uapi_abi::slices::{ByteSlice, StrSlice};

    // Must mirror ByteSlice exactly (6.3 §2.2).
    assert_eq!(size_of::<StrSlice>(), size_of::<ByteSlice>(), "6.3 §2.2: StrSlice size");
    assert_eq!(align_of::<StrSlice>(), align_of::<ByteSlice>(), "6.3 §2.2: StrSlice align");
    assert_eq!(offset_of!(StrSlice, data), 0, "StrSlice.data offset");
    assert_eq!(offset_of!(StrSlice, len), size_of::<usize>(), "StrSlice.len offset");
}

// ---------------------------------------------------------------------------
// 6.3 §2.3 — ErrorCode (`#[repr(i32)]`)
// ---------------------------------------------------------------------------

/// `ErrorCode` (6.3 §2.3 — `#[repr(i32)]`).
///
/// Spec: size = 4, align = 4; key discriminants per catalog.
#[test]
fn error_code_discriminants() {
    use reovim_uapi_abi::error::ErrorCode;

    // Size / align of the fieldless enum (repr(i32) → same as i32).
    assert_eq!(size_of::<ErrorCode>(), 4, "6.3 §2.3: ErrorCode size");
    assert_eq!(align_of::<ErrorCode>(), 4, "6.3 §2.3: ErrorCode align");
    // Catalog discriminant values (6.3 §2.3):
    assert_eq!(ErrorCode::Ok as i32, 0, "ErrorCode::Ok == 0");
    assert_eq!(ErrorCode::Generic as i32, 1, "ErrorCode::Generic == 1");
    assert_eq!(ErrorCode::IncompatibleAbi as i32, 2);
    assert_eq!(ErrorCode::IncompatibleApi as i32, 3);
    assert_eq!(ErrorCode::NotFound as i32, 4);
    assert_eq!(ErrorCode::Conflict as i32, 5);
    assert_eq!(ErrorCode::InvalidArgument as i32, 6);
    assert_eq!(ErrorCode::ResourceExhausted as i32, 7);
    assert_eq!(ErrorCode::ProtocolViolation as i32, 8);
    assert_eq!(ErrorCode::Busy as i32, 9);
    assert_eq!(ErrorCode::Stale as i32, 10);
    assert_eq!(ErrorCode::PermissionDenied as i32, 11);
    assert_eq!(ErrorCode::Panic as i32, 12);
    assert_eq!(ErrorCode::Cancelled as i32, 13);
    assert_eq!(ErrorCode::Timeout as i32, 14);
    assert_eq!(ErrorCode::SchemaInvalid as i32, 15);
    assert_eq!(ErrorCode::NamespaceConflict as i32, 16);
    assert_eq!(ErrorCode::IllegalTrustClass as i32, 17);
    assert_eq!(ErrorCode::ConfigSliceTooLarge as i32, 18);
    assert_eq!(ErrorCode::Utf8Invalid as i32, 19);
    assert_eq!(ErrorCode::IllegalProjectHostSection as i32, 20);
    assert_eq!(ErrorCode::IllegalKind as i32, 21);
    assert_eq!(ErrorCode::ShortVtable as i32, 22);
    assert_eq!(ErrorCode::CodecGone as i32, 23);
    assert_eq!(ErrorCode::NotActive as i32, 24);
    assert_eq!(ErrorCode::RollbackFailed as i32, 25);
    // SP13 key (7.3): BufferTooSmall = 26.
    assert_eq!(
        ErrorCode::BufferTooSmall as i32,
        26,
        "6.3 §2.3 + 7.3 SP13: BufferTooSmall == 26"
    );
}

// ---------------------------------------------------------------------------
// 6.3 §2.4 — LogLevel (`#[repr(u8)]`)
// ---------------------------------------------------------------------------

/// `LogLevel` (6.3 §2.4 — `#[repr(u8)]`).
///
/// Spec: size = 1, align = 1.
#[test]
fn log_level_discriminants() {
    use reovim_uapi_abi::error::LogLevel;

    assert_eq!(size_of::<LogLevel>(), 1, "6.3 §2.4: LogLevel size");
    assert_eq!(align_of::<LogLevel>(), 1, "6.3 §2.4: LogLevel align");
    assert_eq!(LogLevel::Unknown as u8, 0);
    assert_eq!(LogLevel::Trace as u8, 1);
    assert_eq!(LogLevel::Debug as u8, 2);
    assert_eq!(LogLevel::Info as u8, 3);
    assert_eq!(LogLevel::Warn as u8, 4);
    assert_eq!(LogLevel::Error as u8, 5);
}

// ---------------------------------------------------------------------------
// 6.3 §3 — VtableHeader
// ---------------------------------------------------------------------------

/// `VtableHeader` (6.3 §3).
///
/// Spec field layout (natural #[repr(C)], 64-bit):
/// | `offset` | `size` | `field` |
/// |--------|------|-------|
/// | 0 | 8 | `abi (AbiVersion: 4×u16)` |
/// | 8 | 8 | `api (Version: 4×u16)` |
/// | 16 | 8 | `size_of_self (usize)` |
/// | 24 | 1 | `kind (ManifestKind: u8) → but padded to align(u32)` |
/// | `(25+3 pad)` | 4 | `flags (u32)` |
///
/// On `x86_64` (`usize`=8): size = 32, align = 8.
#[test]
fn vtable_header_layout() {
    use reovim_uapi_abi::vtable::VtableHeader;

    // On a 64-bit target: abi(8) + api(8) + size_of_self(8) + kind(1 + 3 pad) + flags(4) = 32.
    assert_eq!(size_of::<VtableHeader>(), 32, "6.3 §3: VtableHeader size (64-bit)");
    assert_eq!(align_of::<VtableHeader>(), 8, "6.3 §3: VtableHeader align");
    // Field offsets:
    assert_eq!(offset_of!(VtableHeader, abi), 0, "VtableHeader.abi offset");
    assert_eq!(offset_of!(VtableHeader, api), 8, "VtableHeader.api offset");
    assert_eq!(offset_of!(VtableHeader, size_of_self), 16, "VtableHeader.size_of_self offset");
    assert_eq!(offset_of!(VtableHeader, kind), 24, "VtableHeader.kind offset");
    assert_eq!(offset_of!(VtableHeader, flags), 28, "VtableHeader.flags offset");
}

/// `ManifestKind` (6.3 §3 — `#[repr(u8)]`).
///
/// Spec: size = 1, align = 1; discriminant values.
#[test]
fn manifest_kind_discriminants() {
    use reovim_uapi_abi::vtable::ManifestKind;

    assert_eq!(size_of::<ManifestKind>(), 1, "6.3 §3: ManifestKind size");
    assert_eq!(align_of::<ManifestKind>(), 1, "6.3 §3: ManifestKind align");
    assert_eq!(ManifestKind::Unknown as u8, 0);
    assert_eq!(ManifestKind::ModuleServer as u8, 1);
    assert_eq!(ManifestKind::DriverServer as u8, 2);
    assert_eq!(ManifestKind::ModuleClient as u8, 3);
    assert_eq!(ManifestKind::DriverClient as u8, 4);
    assert_eq!(ManifestKind::CapabilityClient as u8, 5);
    assert_eq!(ManifestKind::DomainServer as u8, 6);
    assert_eq!(ManifestKind::ProviderServer as u8, 7);
    assert_eq!(ManifestKind::StreamScheme as u8, 8);
}

// ---------------------------------------------------------------------------
// 6.3 §4 / 6.4 — Config types
// ---------------------------------------------------------------------------

/// `ConfigValueKind` (6.3 §4 — `#[repr(u8)]`).
#[test]
fn config_value_kind_discriminants() {
    use reovim_uapi_abi::config::ConfigValueKind;

    assert_eq!(size_of::<ConfigValueKind>(), 1, "6.3 §4: ConfigValueKind size");
    assert_eq!(align_of::<ConfigValueKind>(), 1, "6.3 §4: ConfigValueKind align");
    assert_eq!(ConfigValueKind::Unknown as u8, 0);
    assert_eq!(ConfigValueKind::Bool as u8, 1);
    assert_eq!(ConfigValueKind::U32 as u8, 2);
    assert_eq!(ConfigValueKind::I32 as u8, 3);
    assert_eq!(ConfigValueKind::U64 as u8, 4);
    assert_eq!(ConfigValueKind::I64 as u8, 5);
    assert_eq!(ConfigValueKind::F64 as u8, 6);
    assert_eq!(ConfigValueKind::String as u8, 7);
    assert_eq!(ConfigValueKind::Path as u8, 8);
    assert_eq!(ConfigValueKind::Enum as u8, 9);
    assert_eq!(ConfigValueKind::CanonicalToml as u8, 10);
}

/// `ConfigSource` (6.3 §4 — `#[repr(u8)]`).
#[test]
fn config_source_discriminants() {
    use reovim_uapi_abi::config::ConfigSource;

    assert_eq!(size_of::<ConfigSource>(), 1, "6.3 §4: ConfigSource size");
    assert_eq!(align_of::<ConfigSource>(), 1, "6.3 §4: ConfigSource align");
    assert_eq!(ConfigSource::Default as u8, 0);
    assert_eq!(ConfigSource::System as u8, 1);
    assert_eq!(ConfigSource::User as u8, 2);
    assert_eq!(ConfigSource::Project as u8, 3);
    assert_eq!(ConfigSource::Env as u8, 4);
    assert_eq!(ConfigSource::Cli as u8, 5);
    assert_eq!(ConfigSource::Force as u8, 6);
}

/// `ConfigValue` (6.3 §4 — `#[repr(C)]` union).
///
/// Spec: the widest member is `f64`/`u64`/`i64`/`ByteSlice`.
/// On `x86_64`: `ByteSlice` = 16, so size = 16, align = 8.
#[test]
fn config_value_layout() {
    use reovim_uapi_abi::config::ConfigValue;

    // Widest member is ByteSlice (2×usize = 16 on 64-bit).
    assert_eq!(size_of::<ConfigValue>(), 16, "6.3 §4: ConfigValue size (64-bit)");
    assert_eq!(align_of::<ConfigValue>(), 8, "6.3 §4: ConfigValue align (64-bit)");
}

// ---------------------------------------------------------------------------
// 6.3 §5 — Coordination carriers
// ---------------------------------------------------------------------------

/// `PositionHeader` (6.3 §5 — `[u8; 8]`).
///
/// Spec: size = 8, align = 1.
#[test]
fn position_header_layout() {
    use reovim_uapi_abi::coordination::PositionHeader;

    assert_eq!(size_of::<PositionHeader>(), 8, "6.3 §5: PositionHeader size");
    assert_eq!(align_of::<PositionHeader>(), 1, "6.3 §5: PositionHeader align");
    assert_eq!(offset_of!(PositionHeader, bytes), 0, "PositionHeader.bytes offset");
}

/// `CursorHeader` (6.3 §5 — `[u8; 8]`).
///
/// Spec: size = 8, align = 1.
#[test]
fn cursor_header_layout() {
    use reovim_uapi_abi::coordination::CursorHeader;

    assert_eq!(size_of::<CursorHeader>(), 8, "6.3 §5: CursorHeader size");
    assert_eq!(align_of::<CursorHeader>(), 1, "6.3 §5: CursorHeader align");
    assert_eq!(offset_of!(CursorHeader, bytes), 0, "CursorHeader.bytes offset");
}

/// `PositionCarrierWire` (6.3 §5 — header(8) + ByteSlice(2×usize)).
///
/// On `x86_64`: header(8) + `ByteSlice`(16) = 24 bytes, align = 8.
#[test]
fn position_carrier_wire_layout() {
    use reovim_uapi_abi::coordination::PositionCarrierWire;

    assert_eq!(size_of::<PositionCarrierWire>(), 24, "6.3 §5: PositionCarrierWire size");
    assert_eq!(align_of::<PositionCarrierWire>(), 8, "6.3 §5: PositionCarrierWire align");
    assert_eq!(offset_of!(PositionCarrierWire, header), 0, "PositionCarrierWire.header offset");
    assert_eq!(
        offset_of!(PositionCarrierWire, content),
        8,
        "PositionCarrierWire.content offset"
    );
}

/// `CursorCarrierWire` (6.3 §5 — same layout as `PositionCarrierWire`).
#[test]
fn cursor_carrier_wire_layout() {
    use reovim_uapi_abi::coordination::CursorCarrierWire;

    assert_eq!(size_of::<CursorCarrierWire>(), 24, "6.3 §5: CursorCarrierWire size");
    assert_eq!(align_of::<CursorCarrierWire>(), 8, "6.3 §5: CursorCarrierWire align");
    assert_eq!(offset_of!(CursorCarrierWire, header), 0, "CursorCarrierWire.header offset");
    assert_eq!(offset_of!(CursorCarrierWire, content), 8, "CursorCarrierWire.content offset");
}

// ---------------------------------------------------------------------------
// 6.3 §7 — RawInput + per-kind payload types
// ---------------------------------------------------------------------------

/// `RawInputKind` (6.3 §7 — `#[repr(u8)]`).
#[test]
fn raw_input_kind_discriminants() {
    use reovim_uapi_abi::input::RawInputKind;

    assert_eq!(size_of::<RawInputKind>(), 1, "6.3 §7: RawInputKind size");
    assert_eq!(align_of::<RawInputKind>(), 1, "6.3 §7: RawInputKind align");
    assert_eq!(RawInputKind::Key as u8, 1);
    assert_eq!(RawInputKind::Mouse as u8, 2);
    assert_eq!(RawInputKind::Text as u8, 3);
    assert_eq!(RawInputKind::Paste as u8, 4);
    assert_eq!(RawInputKind::Web as u8, 5);
    assert_eq!(RawInputKind::Trigger as u8, 6);
    assert_eq!(RawInputKind::Ime as u8, 7);
}

/// `KeyEvent` (6.3 §7.1): size 24, align 8.
///
/// Spec field table:
/// | `offset` | `size` | `field` |
/// |--------|------|-------|
/// | 0 | 4 | `keycode (u32)` |
/// | 4 | 1 | `mods (u8)` |
/// | 5 | 1 | `action (u8)` |
/// | 6 | 2 | `pad ([u8;2])` |
/// | 8 | 8 | `timestamp_ns (u64)` |
/// | 16 | 8 | `utf8 ([u8;8])` |
#[test]
fn key_event_layout() {
    use reovim_uapi_abi::input::KeyEvent;

    assert_eq!(size_of::<KeyEvent>(), 24, "6.3 §7.1: KeyEvent size");
    assert_eq!(align_of::<KeyEvent>(), 8, "6.3 §7.1: KeyEvent align");
    assert_eq!(offset_of!(KeyEvent, keycode), 0, "KeyEvent.keycode offset");
    assert_eq!(offset_of!(KeyEvent, mods), 4, "KeyEvent.mods offset");
    assert_eq!(offset_of!(KeyEvent, action), 5, "KeyEvent.action offset");
    assert_eq!(offset_of!(KeyEvent, pad), 6, "KeyEvent.pad offset");
    assert_eq!(offset_of!(KeyEvent, timestamp_ns), 8, "KeyEvent.timestamp_ns offset");
    assert_eq!(offset_of!(KeyEvent, utf8), 16, "KeyEvent.utf8 offset");
}

/// `MouseEvent` (6.3 §7.2): size 32, align 8.
///
/// Spec field table:
/// | `offset` | `size` | `field` |
/// |--------|------|-------|
/// | 0 | 1 | `action (u8)` |
/// | 1 | 1 | `button (u8)` |
/// | 2 | 1 | `mods (u8)` |
/// | 3 | 1 | `flags (u8)` |
/// | 4 | 4 | `col (i32)` |
/// | 8 | 4 | `row (i32)` |
/// | 12 | 4 | `px_x (i32)` |
/// | 16 | 4 | `px_y (i32)` |
/// | 20 | 2 | `scroll_x (i16)` |
/// | 22 | 2 | `scroll_y (i16)` |
/// | 24 | 8 | `timestamp_ns (u64)` |
#[test]
fn mouse_event_layout() {
    use reovim_uapi_abi::input::MouseEvent;

    assert_eq!(size_of::<MouseEvent>(), 32, "6.3 §7.2: MouseEvent size");
    assert_eq!(align_of::<MouseEvent>(), 8, "6.3 §7.2: MouseEvent align");
    assert_eq!(offset_of!(MouseEvent, action), 0, "MouseEvent.action offset");
    assert_eq!(offset_of!(MouseEvent, button), 1, "MouseEvent.button offset");
    assert_eq!(offset_of!(MouseEvent, mods), 2, "MouseEvent.mods offset");
    assert_eq!(offset_of!(MouseEvent, flags), 3, "MouseEvent.flags offset");
    assert_eq!(offset_of!(MouseEvent, col), 4, "MouseEvent.col offset");
    assert_eq!(offset_of!(MouseEvent, row), 8, "MouseEvent.row offset");
    assert_eq!(offset_of!(MouseEvent, px_x), 12, "MouseEvent.px_x offset");
    assert_eq!(offset_of!(MouseEvent, px_y), 16, "MouseEvent.px_y offset");
    assert_eq!(offset_of!(MouseEvent, scroll_x), 20, "MouseEvent.scroll_x offset");
    assert_eq!(offset_of!(MouseEvent, scroll_y), 22, "MouseEvent.scroll_y offset");
    assert_eq!(offset_of!(MouseEvent, timestamp_ns), 24, "MouseEvent.timestamp_ns offset");
}

/// `TriggerEvent` (6.3 §7.4): size 16, align 8.
///
/// Spec field table:
/// | `offset` | `size` | `field` |
/// |--------|------|-------|
/// | 0 | 4 | `trigger_id (u32)` |
/// | 4 | 4 | `pad (u32)` |
/// | 8 | 8 | `timestamp_ns (u64)` |
#[test]
fn trigger_event_layout() {
    use reovim_uapi_abi::input::TriggerEvent;

    assert_eq!(size_of::<TriggerEvent>(), 16, "6.3 §7.4: TriggerEvent size");
    assert_eq!(align_of::<TriggerEvent>(), 8, "6.3 §7.4: TriggerEvent align");
    assert_eq!(offset_of!(TriggerEvent, trigger_id), 0, "TriggerEvent.trigger_id offset");
    assert_eq!(offset_of!(TriggerEvent, pad), 4, "TriggerEvent.pad offset");
    assert_eq!(offset_of!(TriggerEvent, timestamp_ns), 8, "TriggerEvent.timestamp_ns offset");
}

/// `ImeEvent` (6.3 §7.5): 16-byte header, align 8.
///
/// Spec field table:
/// | `offset` | `size` | `field` |
/// |--------|------|-------|
/// | 0 | 1 | `phase (u8)` |
/// | 1 | 3 | `pad ([u8;3])` |
/// | 4 | 4 | `caret_byte (u32)` |
/// | 8 | 8 | `timestamp_ns (u64)` |
#[test]
fn ime_event_layout() {
    use reovim_uapi_abi::input::ImeEvent;

    assert_eq!(size_of::<ImeEvent>(), 16, "6.3 §7.5: ImeEvent size");
    assert_eq!(align_of::<ImeEvent>(), 8, "6.3 §7.5: ImeEvent align");
    assert_eq!(offset_of!(ImeEvent, phase), 0, "ImeEvent.phase offset");
    assert_eq!(offset_of!(ImeEvent, pad), 1, "ImeEvent.pad offset");
    assert_eq!(offset_of!(ImeEvent, caret_byte), 4, "ImeEvent.caret_byte offset");
    assert_eq!(offset_of!(ImeEvent, timestamp_ns), 8, "ImeEvent.timestamp_ns offset");
}

// ---------------------------------------------------------------------------
// 6.3 §8 — Stream substrate
// ---------------------------------------------------------------------------

/// `StreamId` (6.3 §8 — `#[repr(transparent)]` over `u64`).
#[test]
fn stream_id_layout() {
    use reovim_uapi_abi::stream::StreamId;

    assert_eq!(size_of::<StreamId>(), 8, "6.3 §8: StreamId size");
    assert_eq!(align_of::<StreamId>(), 8, "6.3 §8: StreamId align");
}

/// `StreamState` (6.3 §8 — `#[repr(u8)]`).
#[test]
fn stream_state_discriminants() {
    use reovim_uapi_abi::stream::StreamState;

    assert_eq!(size_of::<StreamState>(), 1, "6.3 §8: StreamState size");
    assert_eq!(align_of::<StreamState>(), 1, "6.3 §8: StreamState align");
    assert_eq!(StreamState::Init as u8, 0);
    assert_eq!(StreamState::Running as u8, 1);
    assert_eq!(StreamState::BackpressureBlocked as u8, 2);
    assert_eq!(StreamState::Stale as u8, 3);
    assert_eq!(StreamState::Draining as u8, 4);
    assert_eq!(StreamState::Closed as u8, 5);
}

/// `StreamHandleInfo` (6.3 §8).
///
/// On `x86_64`: `StreamId`(8) + `StreamState`(1 + 7 pad) + `bytes_in_flight`(8) + `bytes_buffered`(8) = 32.
#[test]
fn stream_handle_info_layout() {
    use reovim_uapi_abi::stream::StreamHandleInfo;

    assert_eq!(size_of::<StreamHandleInfo>(), 32, "6.3 §8: StreamHandleInfo size");
    assert_eq!(align_of::<StreamHandleInfo>(), 8, "6.3 §8: StreamHandleInfo align");
    assert_eq!(offset_of!(StreamHandleInfo, id), 0, "StreamHandleInfo.id offset");
    assert_eq!(offset_of!(StreamHandleInfo, state), 8, "StreamHandleInfo.state offset");
    assert_eq!(
        offset_of!(StreamHandleInfo, bytes_in_flight),
        16,
        "StreamHandleInfo.bytes_in_flight offset"
    );
    assert_eq!(
        offset_of!(StreamHandleInfo, bytes_buffered),
        24,
        "StreamHandleInfo.bytes_buffered offset"
    );
}

// ---------------------------------------------------------------------------
// 6.3 §12 — FrameHeader (wire boundary)
// ---------------------------------------------------------------------------

/// `FrameHeader` (6.3 §12 / 7.3 §3): size 16, align 8.
///
/// Spec field table:
/// | `offset` | `size` | `field` |
/// |--------|------|-------|
/// | 0 | 4 | `body_len (u32)` |
/// | 4 | 2 | `msg_type (u16)` |
/// | 6 | 2 | `flags (u16)` |
/// | 8 | 8 | `correlation_id (u64)` |
#[test]
fn frame_header_layout() {
    use reovim_uapi_abi::frame::FrameHeader;

    assert_eq!(size_of::<FrameHeader>(), 16, "6.3 §12: FrameHeader size");
    assert_eq!(align_of::<FrameHeader>(), 8, "6.3 §12: FrameHeader align");
    assert_eq!(offset_of!(FrameHeader, body_len), 0, "FrameHeader.body_len offset");
    assert_eq!(offset_of!(FrameHeader, msg_type), 4, "FrameHeader.msg_type offset");
    assert_eq!(offset_of!(FrameHeader, flags), 6, "FrameHeader.flags offset");
    assert_eq!(offset_of!(FrameHeader, correlation_id), 8, "FrameHeader.correlation_id offset");
}

/// `FrameHeader` wire-bytes golden: encode a known header and assert the exact
/// 16-byte little-endian form.
///
/// Golden source: 7.3 §"worked frame" — `body_len`=41=0x29, `msg_type`=0x0001,
/// `flags`=0, `correlation_id`=0.
#[test]
fn frame_header_wire_bytes_golden() {
    use reovim_uapi_abi::frame::FrameHeader;

    let hdr = FrameHeader {
        body_len: 41,
        msg_type: 0x0001,
        flags: 0,
        correlation_id: 0,
    };
    let mut buf = [0u8; 16];
    hdr.encode(&mut buf);

    // Expected per 7.3 §"worked frame":
    // offset 0x00: 29 00 00 00  (body_len = 41 LE)
    // offset 0x04: 01 00        (msg_type = 0x0001 LE)
    // offset 0x06: 00 00        (flags = 0)
    // offset 0x08: 00 00 00 00 00 00 00 00  (correlation_id = 0)
    let expected: [u8; 16] = [
        0x29, 0x00, 0x00, 0x00, // body_len = 41
        0x01, 0x00, // msg_type = 0x0001
        0x00, 0x00, // flags = 0
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // correlation_id = 0
    ];
    assert_eq!(buf, expected, "6.3 §12 wire golden: FrameHeader 16-byte LE encoding");

    // Round-trip: decode back gives the same header.
    let back = FrameHeader::decode(&buf);
    assert_eq!(back, hdr, "6.3 §12: FrameHeader decode round-trip");
}
