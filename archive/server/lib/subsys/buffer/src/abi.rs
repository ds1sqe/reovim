//! C-stable ABI types for the buffer server driver.
//!
//! These types are the runtime-loading counterpart to the
//! [`crate::driver::BufferDriver`] trait. A cdylib driver exports a
//! static vtable under the symbol `REOVIM_BUFFER_DRIVER_VTABLE`
//! (spelled in the driver's codegen, not re-exported here). The host
//! loader (`server/lib/subsys/driver-loader/`) reads the vtable
//! through this module.
//!
//! Binary contract: `docs/architecture/driver-abi-v1.md`.

#![allow(unsafe_code)] // conversion helpers contain unsafe blocks

use {
    reovim_kernel::api::v1::{ByteEdit, Version},
    std::ffi::{c_char, c_int, c_void},
};

// ── Version constants ────────────────────────────────────────────────────────

/// ABI version epoch for the buffer driver vtable layout.
///
/// Hosts reject cdylibs whose vtable reports a different value. Any
/// field reorder, add, or remove bumps this epoch. Orthogonal to
/// [`REOVIM_BUFFER_DRIVER_API_VERSION`], which tracks
/// backwards-compatible semantics.
pub const REOVIM_BUFFER_DRIVER_ABI_VERSION: u32 = 1;

/// API version of the buffer driver contract.
///
/// Semver: host accepts a driver with the same major and
/// `minor >= host.minor`.
pub const REOVIM_BUFFER_DRIVER_API_VERSION: Version = Version::new(1, 0, 0);

// ── Probe ────────────────────────────────────────────────────────────────────

/// Static probe metadata returned by the driver before construction.
///
/// `kind` and `name` are zero-padded UTF-8 byte arrays. The host
/// treats a zero-byte `kind` as "non-driver, skip". The loader
/// accepts a driver only when `kind == b"buffer"`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct BufferDriverProbe {
    /// Driver kind: `b"buffer\0...\0"` (must equal `"buffer"`
    /// for loader acceptance).
    pub kind: [u8; 64],
    /// Human-readable driver name (e.g.
    /// `b"reovim-driver-text-buffer\0...\0"`).
    pub name: [u8; 128],
}

impl BufferDriverProbe {
    /// Build a probe value from `kind` and `name` strings.
    ///
    /// Both strings are truncated if they exceed the array capacity;
    /// the last byte of each array stays `0` as a nul terminator.
    #[must_use]
    pub fn new(kind: &str, name: &str) -> Self {
        let mut k = [0u8; 64];
        let mut n = [0u8; 128];
        let kb = kind.as_bytes();
        let nb = name.as_bytes();
        let klen = kb.len().min(63);
        let nlen = nb.len().min(127);
        k[..klen].copy_from_slice(&kb[..klen]);
        n[..nlen].copy_from_slice(&nb[..nlen]);
        Self { kind: k, name: n }
    }
}

// ── FFI edit / codec types ───────────────────────────────────────────────────

/// FFI image of [`reovim_kernel::api::v1::ByteEdit`].
///
/// `old_ptr` and `new_ptr` point into host-pinned `Vec<u8>` allocations
/// that must remain alive for the duration of each `notify` trampoline
/// call. Drivers must copy the bytes before returning if they need to
/// retain them.
#[repr(C)]
pub struct FfiByteEdit {
    /// Byte offset (u64, not usize, for stable cross-platform width).
    pub offset: u64,
    /// Pointer to old bytes; host-pinned for the notify call duration.
    pub old_ptr: *const u8,
    /// Length of the old-bytes slice.
    pub old_len: usize,
    /// Pointer to new bytes; host-pinned for the notify call duration.
    pub new_ptr: *const u8,
    /// Length of the new-bytes slice.
    pub new_len: usize,
}

/// FFI codec-slot descriptor passed across the cdylib boundary.
///
/// Allocator ownership: `handle` is codec-allocated and codec-freed
/// via `destroy_handle`. `name_ptr` is host-pinned for the slot
/// lifetime (the codec must not free it). Matches ABI v1 §6.1
/// allocator hygiene.
#[repr(C)]
pub struct FfiCodecAttachment {
    /// Codec name: host-pinned null-terminated string; valid for the
    /// entire slot lifetime.
    pub name_ptr: *const c_char,
    /// Byte length of the name string (no trailing nul included).
    pub name_len: usize,
    /// Codec-allocated opaque handle; freed by `destroy_handle` below.
    pub handle: *mut c_void,
    /// Codec-side notify trampoline.
    ///
    /// Called synchronously by the buffer inside `apply_edit` for each
    /// attached slot. Returns `0` on success, `-1` on error with
    /// `*out_err` set (driver-allocated; freed by the host via
    /// `destroy_error_string`), `-2` if a panic was caught.
    pub notify: unsafe extern "C" fn(
        handle: *mut c_void,
        edit: *const FfiByteEdit,
        out_err: *mut *mut c_char,
    ) -> c_int,
    /// Codec-side destructor; invoked by the buffer on `detach_codec`
    /// and on buffer destruction.
    pub destroy_handle: unsafe extern "C" fn(handle: *mut c_void),
}

/// A driver-allocated `(id, name)` pair returned by `buffer_list_codecs`.
///
/// `ptr` points to a driver-allocated C string; the entire array is
/// freed by calling the `destroy_codec_name_array` vtable slot.
#[repr(C)]
pub struct FfiCodecName {
    /// Driver-allocated string pointer; freed by `destroy_codec_name_array`.
    pub ptr: *mut c_char,
    /// Byte length of the string (no trailing nul included).
    pub len: usize,
}

// ── VTable ───────────────────────────────────────────────────────────────────

/// Vtable exported by a text-buffer cdylib driver under the symbol
/// `REOVIM_BUFFER_DRIVER_VTABLE`.
///
/// Field order is load-bearing for the binary contract; reordering
/// requires bumping [`REOVIM_BUFFER_DRIVER_ABI_VERSION`]. The
/// host validates `abi_version`, `api_version`, and `size_of_self`
/// before invoking any other slot.
#[repr(C)]
pub struct BufferVTable {
    /// ABI epoch — must equal [`REOVIM_BUFFER_DRIVER_ABI_VERSION`]
    /// on the host.
    pub abi_version: u32,
    /// Semver API version — host accepts same major + `minor >= host.minor`.
    pub api_version: Version,
    /// Size of this struct as observed by the driver build. Host
    /// rejects mismatches (forward-compat guard).
    pub size_of_self: usize,

    // ── Lifecycle ───────────────────────────────────────────────────────
    /// Return static probe metadata without constructing the driver.
    pub probe: unsafe extern "C" fn() -> BufferDriverProbe,
    /// Construct a driver instance.
    ///
    /// On success writes the instance pointer into `out_instance` and
    /// returns `0`. On error writes a driver-allocated C-string into
    /// `out_err` (host frees via `destroy_error_string`) and returns
    /// `-1`. `-2` indicates a caught panic.
    pub construct:
        unsafe extern "C" fn(out_instance: *mut *mut c_void, out_err: *mut *mut c_char) -> c_int,

    // ── BufferDriver factory slots ──────────────────────────────────────
    /// Create a buffer pre-populated with `bytes_ptr[0..bytes_len]`.
    ///
    /// `path_ptr`/`path_len` name an optional file association; pass
    /// `null` / `0` for anonymous buffers. On success: writes the
    /// buffer handle into `out_buffer`, the allocated `BufferId` into
    /// `out_id`, returns `0`.
    pub create_buffer: unsafe extern "C" fn(
        instance: *mut c_void,
        bytes_ptr: *const u8,
        bytes_len: usize,
        path_ptr: *const c_char,
        path_len: usize,
        out_buffer: *mut *mut c_void,
        out_id: *mut u64,
        out_err: *mut *mut c_char,
    ) -> c_int,
    /// Open the file at `path_ptr[0..path_len]` and return a buffer.
    pub open_buffer: unsafe extern "C" fn(
        instance: *mut c_void,
        path_ptr: *const c_char,
        path_len: usize,
        out_buffer: *mut *mut c_void,
        out_id: *mut u64,
        out_err: *mut *mut c_char,
    ) -> c_int,
    /// List all live buffer ids.
    ///
    /// Allocates an array of `*out_count` `u64` values into
    /// `*out_ids`. The host frees the array via `destroy_id_list`.
    pub list_buffers: unsafe extern "C" fn(
        instance: *mut c_void,
        out_ids: *mut *mut u64,
        out_count: *mut usize,
        out_err: *mut *mut c_char,
    ) -> c_int,
    /// Retrieve the buffer handle for `id`.
    ///
    /// Returns `0` and writes the handle into `out_buffer` on success;
    /// returns `-1` (buffer not found) with `*out_buffer = null`.
    pub get_buffer: unsafe extern "C" fn(
        instance: *mut c_void,
        id: u64,
        out_buffer: *mut *mut c_void,
        out_err: *mut *mut c_char,
    ) -> c_int,
    /// Release the buffer with `id` from the driver registry.
    pub close_buffer:
        unsafe extern "C" fn(instance: *mut c_void, id: u64, out_err: *mut *mut c_char) -> c_int,
    /// Free a `u64[]` array allocated by `list_buffers`.
    pub destroy_id_list: unsafe extern "C" fn(ids_ptr: *mut u64, count: usize),

    // ── Buffer (per-instance) slots ─────────────────────────────────────
    /// Return the buffer's `BufferId` as a raw `u64`.
    pub buffer_id: unsafe extern "C" fn(buffer: *mut c_void) -> u64,
    /// Return the total byte count of the buffer.
    pub buffer_size: unsafe extern "C" fn(buffer: *mut c_void) -> usize,
    /// Return `1` if the buffer has unsaved modifications, `0` otherwise.
    pub buffer_is_modified: unsafe extern "C" fn(buffer: *mut c_void) -> u8,
    /// Return the file path associated with this buffer.
    ///
    /// Writes a driver-allocated heap string into `*out_ptr` /
    /// `*out_len`. Returns `-1` (no path) with `*out_ptr = null`.
    /// Host frees via `destroy_file_path`.
    pub buffer_file_path: unsafe extern "C" fn(
        buffer: *mut c_void,
        out_ptr: *mut *mut c_char,
        out_len: *mut usize,
    ) -> c_int,
    /// Replace the file path associated with this buffer.
    ///
    /// Pass `path_ptr = null`, `path_len = 0` to dissociate any
    /// existing path.
    pub buffer_set_file_path: unsafe extern "C" fn(
        buffer: *mut c_void,
        path_ptr: *const c_char,
        path_len: usize,
        out_err: *mut *mut c_char,
    ) -> c_int,
    /// Read bytes at `[start, end)` from the buffer.
    ///
    /// Allocates a `u8[]` into `*out_ptr` / `*out_len`. Host frees via
    /// `destroy_byte_buffer`.
    pub buffer_read_bytes: unsafe extern "C" fn(
        buffer: *mut c_void,
        start: usize,
        end: usize,
        out_ptr: *mut *mut u8,
        out_len: *mut usize,
        out_err: *mut *mut c_char,
    ) -> c_int,
    /// Apply a byte-level edit and fan out to all attached codecs.
    pub buffer_apply_edit: unsafe extern "C" fn(
        buffer: *mut c_void,
        edit: *const FfiByteEdit,
        out_err: *mut *mut c_char,
    ) -> c_int,
    /// Write the full buffer contents via `write_cb`.
    ///
    /// `write_cb` is called zero or more times with consecutive byte
    /// chunks. Returns `0` on success, `-1` if `write_cb` ever returns
    /// a non-zero status, `-2` if a panic was caught.
    pub buffer_write_to: unsafe extern "C" fn(
        buffer: *mut c_void,
        write_cb: unsafe extern "C" fn(ctx: *mut c_void, bytes: *const u8, len: usize) -> c_int,
        ctx: *mut c_void,
        out_err: *mut *mut c_char,
    ) -> c_int,

    // ── Multi-attachment slots (M8 surface) ─────────────────────────────
    /// Attach a codec slot to the buffer.
    ///
    /// The driver calls `attachment.notify` with the current content to
    /// warm-start the codec index before returning the slot id in
    /// `*out_id`.
    pub buffer_attach_codec: unsafe extern "C" fn(
        buffer: *mut c_void,
        attachment: *const FfiCodecAttachment,
        out_id: *mut u32,
        out_err: *mut *mut c_char,
    ) -> c_int,
    /// Detach and return the codec slot at `id`.
    ///
    /// Writes the original `FfiCodecAttachment` into `*out_attachment`
    /// so the host can invoke `destroy_handle` on the codec's behalf.
    pub buffer_detach_codec: unsafe extern "C" fn(
        buffer: *mut c_void,
        id: u32,
        out_attachment: *mut FfiCodecAttachment,
        out_err: *mut *mut c_char,
    ) -> c_int,
    /// List all live codec slots.
    ///
    /// Allocates parallel arrays of `*out_count` slot ids (`u32[]`)
    /// and name entries (`FfiCodecName[]`) into `*out_ids` and
    /// `*out_names`. Host frees both via `destroy_codec_name_array`.
    pub buffer_list_codecs: unsafe extern "C" fn(
        buffer: *mut c_void,
        out_ids: *mut *mut u32,
        out_names: *mut *mut FfiCodecName,
        out_count: *mut usize,
        out_err: *mut *mut c_char,
    ) -> c_int,

    // ── Async subscription bridge ────────────────────────────────────────
    /// Register an async byte-edit subscriber.
    ///
    /// `subscriber_cb` will be invoked on each edit until the
    /// subscription is cancelled via `buffer_unsubscribe_edits`.
    /// `subscriber_destroy` is called when the subscription is
    /// cancelled or the buffer is destroyed.
    pub buffer_subscribe_edits: unsafe extern "C" fn(
        buffer: *mut c_void,
        subscriber_handle: *mut c_void,
        subscriber_cb: unsafe extern "C" fn(handle: *mut c_void, edit: *const FfiByteEdit) -> c_int,
        subscriber_destroy: unsafe extern "C" fn(handle: *mut c_void),
        out_subscription_id: *mut u32,
        out_err: *mut *mut c_char,
    ) -> c_int,
    /// Cancel an async byte-edit subscription.
    pub buffer_unsubscribe_edits: unsafe extern "C" fn(
        buffer: *mut c_void,
        subscription_id: u32,
        out_err: *mut *mut c_char,
    ) -> c_int,

    // ── Buffer destruction ──────────────────────────────────────────────
    /// Destroy a buffer handle returned by `create_buffer` or
    /// `open_buffer`. Detaches all attached codec slots (calling each
    /// `destroy_handle`) and invokes all active `subscriber_destroy`
    /// callbacks before freeing internal state.
    pub buffer_destroy: unsafe extern "C" fn(buffer: *mut c_void),

    // ── Driver destruction ──────────────────────────────────────────────
    /// Destroy the driver instance. Called once after all buffers have
    /// been destroyed and before the cdylib is unloaded.
    pub destroy: unsafe extern "C" fn(instance: *mut c_void),
    /// Free a C-string allocated by any driver error path.
    pub destroy_error_string: unsafe extern "C" fn(ptr: *mut c_char),
    /// Free a `u8[]` allocated by `buffer_read_bytes`.
    pub destroy_byte_buffer: unsafe extern "C" fn(ptr: *mut u8, len: usize),
    /// Free a file-path string allocated by `buffer_file_path`.
    ///
    /// The driver allocates file paths on the heap (not as static
    /// literals — the path can change at runtime) and the host must
    /// call this slot after copying the bytes into a host-owned buffer.
    pub destroy_file_path: unsafe extern "C" fn(ptr: *mut c_char, len: usize),
    /// Free the parallel id/name arrays allocated by `buffer_list_codecs`.
    pub destroy_codec_name_array:
        unsafe extern "C" fn(ids_ptr: *mut u32, names_ptr: *mut FfiCodecName, count: usize),
}

// ── Conversion helpers ───────────────────────────────────────────────────────

/// Convert a host-side [`ByteEdit`] to its FFI image.
///
/// The returned `FfiByteEdit` holds raw pointers into `edit.old_bytes`
/// and `edit.new_bytes`. The caller must ensure `edit` outlives any
/// use of the returned value.
#[must_use]
pub const fn byte_edit_to_ffi(edit: &ByteEdit) -> FfiByteEdit {
    #[allow(clippy::cast_possible_truncation)] // usize → u64: no truncation on 64-bit targets
    let offset = edit.offset as u64;
    FfiByteEdit {
        offset,
        old_ptr: edit.old_bytes.as_ptr(),
        old_len: edit.old_bytes.len(),
        new_ptr: edit.new_bytes.as_ptr(),
        new_len: edit.new_bytes.len(),
    }
}

/// Reconstruct a [`ByteEdit`] from its FFI image.
///
/// # Safety
///
/// Both `ffi.old_ptr[0..ffi.old_len]` and `ffi.new_ptr[0..ffi.new_len]`
/// must be valid, readable byte slices for the duration of this call.
/// Null pointers are only safe when the corresponding `len` is `0`.
#[must_use]
pub unsafe fn byte_edit_from_ffi(ffi: &FfiByteEdit) -> ByteEdit {
    let old_bytes = if ffi.old_len == 0 {
        Vec::new()
    } else {
        // SAFETY: caller guarantees validity; see function safety contract.
        unsafe { std::slice::from_raw_parts(ffi.old_ptr, ffi.old_len).to_vec() }
    };
    let new_bytes = if ffi.new_len == 0 {
        Vec::new()
    } else {
        // SAFETY: caller guarantees validity; see function safety contract.
        unsafe { std::slice::from_raw_parts(ffi.new_ptr, ffi.new_len).to_vec() }
    };
    #[allow(clippy::cast_possible_truncation)] // u64 → usize: no truncation on 64-bit targets
    let offset = ffi.offset as usize;
    ByteEdit {
        offset,
        old_bytes,
        new_bytes,
    }
}

#[cfg(test)]
#[path = "abi_tests.rs"]
mod tests;
