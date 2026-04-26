//! Shape and round-trip tests for the FFI ABI types.

#![allow(unsafe_code)] // round-trip tests dereference host-pinned pointers

use super::*;

// ── Size / alignment ─────────────────────────────────────────────────────────

#[test]
fn ffi_byte_edit_size_and_align() {
    // offset(u64=8) + old_ptr(*const u8=8) + old_len(usize=8) +
    // new_ptr(*const u8=8) + new_len(usize=8) = 40; no padding on 64-bit.
    assert_eq!(std::mem::size_of::<FfiByteEdit>(), 40);
    assert_eq!(std::mem::align_of::<FfiByteEdit>(), std::mem::align_of::<*const u8>());
}

#[test]
fn ffi_codec_attachment_size_and_align() {
    // name_ptr(8) + name_len(8) + handle(8) + notify(8) + destroy_handle(8)
    // = 40; all pointer-sized on 64-bit.
    assert_eq!(std::mem::size_of::<FfiCodecAttachment>(), 40);
    assert_eq!(std::mem::align_of::<FfiCodecAttachment>(), std::mem::align_of::<*const ()>());
}

#[test]
fn ffi_codec_name_size_and_align() {
    // ptr(8) + len(8) = 16.
    assert_eq!(std::mem::size_of::<FfiCodecName>(), 16);
    assert_eq!(std::mem::align_of::<FfiCodecName>(), std::mem::align_of::<*const ()>());
}

#[test]
fn probe_size_is_192_bytes() {
    // kind[u8;64] + name[u8;128] = 192; all u8, no padding.
    assert_eq!(std::mem::size_of::<BufferDriverProbe>(), 192);
    assert_eq!(std::mem::align_of::<BufferDriverProbe>(), 1);
}

#[test]
fn vtable_aligns_to_pointer() {
    assert_eq!(std::mem::align_of::<BufferVTable>(), std::mem::align_of::<*const ()>());
}

#[test]
fn vtable_is_non_empty() {
    // Sanity guard against accidental field removal collapsing the
    // struct. Exact size depends on platform pointer width and padding;
    // we assert a lower bound (header + 27 fn-pointer slots).
    let min_size = std::mem::size_of::<u32>()                          // abi_version
        + std::mem::size_of::<reovim_kernel::api::v1::Version>()       // api_version
        + std::mem::size_of::<usize>()                                 // size_of_self
        + 27 * std::mem::size_of::<*const ()>(); // 27 fn-pointer slots
    assert!(
        std::mem::size_of::<BufferVTable>() >= min_size,
        "vtable size shrank below minimum field-sum (got {}, want >= {})",
        std::mem::size_of::<BufferVTable>(),
        min_size,
    );
}

// ── Field offsets ────────────────────────────────────────────────────────────

#[test]
fn ffi_byte_edit_field_offsets() {
    assert_eq!(std::mem::offset_of!(FfiByteEdit, offset), 0);
    assert_eq!(std::mem::offset_of!(FfiByteEdit, old_ptr), 8);
    assert_eq!(std::mem::offset_of!(FfiByteEdit, old_len), 16);
    assert_eq!(std::mem::offset_of!(FfiByteEdit, new_ptr), 24);
    assert_eq!(std::mem::offset_of!(FfiByteEdit, new_len), 32);
}

#[test]
fn ffi_codec_attachment_field_offsets() {
    assert_eq!(std::mem::offset_of!(FfiCodecAttachment, name_ptr), 0);
    assert_eq!(std::mem::offset_of!(FfiCodecAttachment, name_len), 8);
    assert_eq!(std::mem::offset_of!(FfiCodecAttachment, handle), 16);
    assert_eq!(std::mem::offset_of!(FfiCodecAttachment, notify), 24);
    assert_eq!(std::mem::offset_of!(FfiCodecAttachment, destroy_handle), 32);
}

#[test]
fn ffi_codec_name_field_offsets() {
    assert_eq!(std::mem::offset_of!(FfiCodecName, ptr), 0);
    assert_eq!(std::mem::offset_of!(FfiCodecName, len), 8);
}

#[test]
fn probe_field_offsets() {
    assert_eq!(std::mem::offset_of!(BufferDriverProbe, kind), 0);
    assert_eq!(std::mem::offset_of!(BufferDriverProbe, name), 64);
}

// ── Probe::new ───────────────────────────────────────────────────────────────

#[test]
fn probe_new_normal_strings_zero_padded() {
    let p = BufferDriverProbe::new("buffer", "reovim-driver-text-buffer");
    assert_eq!(&p.kind[..6], b"buffer");
    assert_eq!(p.kind[6], 0, "kind zero-padded after content");
    assert_eq!(&p.name[..25], b"reovim-driver-text-buffer");
    assert_eq!(p.name[25], 0, "name zero-padded after content");
}

#[test]
fn probe_new_truncates_oversized_strings() {
    let kind = "x".repeat(100);
    let name = "y".repeat(200);
    let p = BufferDriverProbe::new(&kind, &name);
    assert_eq!(p.kind[63], 0, "kind: last byte stays nul");
    assert_eq!(p.name[127], 0, "name: last byte stays nul");
    assert_eq!(p.kind[..63], [b'x'; 63]);
    assert_eq!(p.name[..127], [b'y'; 127]);
}

// ── byte_edit round-trip ─────────────────────────────────────────────────────

#[test]
fn byte_edit_round_trip() {
    use reovim_kernel::api::v1::ByteEdit;

    let original = ByteEdit {
        offset: 10,
        old_bytes: b"abc".to_vec(),
        new_bytes: b"xyz".to_vec(),
    };

    let ffi = byte_edit_to_ffi(&original);
    assert_eq!(ffi.offset, 10);
    assert_eq!(ffi.old_len, 3);
    assert_eq!(ffi.new_len, 3);

    // SAFETY: `ffi` holds pointers into `original`'s `Vec<u8>` fields,
    // which remain allocated for the duration of this test.
    let recovered = unsafe { byte_edit_from_ffi(&ffi) };
    assert_eq!(recovered.offset, original.offset);
    assert_eq!(recovered.old_bytes, original.old_bytes);
    assert_eq!(recovered.new_bytes, original.new_bytes);
}

#[test]
fn byte_edit_round_trip_empty_slices() {
    use reovim_kernel::api::v1::ByteEdit;

    let insert = ByteEdit::insert(0, b"hello");
    let ffi = byte_edit_to_ffi(&insert);
    // SAFETY: `ffi` points into `insert`'s Vecs which are live.
    let recovered = unsafe { byte_edit_from_ffi(&ffi) };
    assert_eq!(recovered, insert);

    let delete = ByteEdit::delete(5, b"abc");
    let ffi2 = byte_edit_to_ffi(&delete);
    // SAFETY: same as above.
    let recovered2 = unsafe { byte_edit_from_ffi(&ffi2) };
    assert_eq!(recovered2, delete);
}

// ── Version constants ────────────────────────────────────────────────────────

#[test]
fn abi_version_constant() {
    assert_eq!(REOVIM_BUFFER_DRIVER_ABI_VERSION, 1);
}

#[test]
fn api_version_constant() {
    assert_eq!(REOVIM_BUFFER_DRIVER_API_VERSION.major, 1);
    assert_eq!(REOVIM_BUFFER_DRIVER_API_VERSION.minor, 0);
    assert_eq!(REOVIM_BUFFER_DRIVER_API_VERSION.patch, 0);
}
