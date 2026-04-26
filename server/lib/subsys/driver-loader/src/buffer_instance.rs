//! Per-buffer wrapper that routes [`Buffer`] trait methods through the
//! cdylib's [`BufferVTable`] per-instance slots.
//!
//! Every `LoadedBufferInstance` carries an `Arc<BufferLibraryGuard>`
//! cloned from the [`crate::buffer::LoadedBuffer`] that produced it.
//! That clone keeps both the cdylib mapping and the driver instance
//! alive for as long as any buffer instance lives — so the vtable
//! function pointers and the driver-allocated heap blocks the
//! per-buffer slots reach into remain valid.
//!
//! `Drop` calls `vtable.buffer_destroy(handle)`. The
//! `BufferLibraryGuard`'s own `Drop` runs `vtable.destroy(driver
//! instance)` and unmaps the cdylib only after the last clone — driver
//! wrapper plus every buffer instance — is gone.

use {
    crate::buffer::BufferLibraryGuard,
    reovim_content_codec::ByteNotifiable,
    reovim_kernel::api::v1::{BufferId, ByteEdit},
    reovim_subsys_buffer::{
        Buffer, BufferError, CodecAttachmentId,
        abi::{FfiByteEdit, byte_edit_to_ffi},
    },
    std::{
        ffi::{c_char, c_void},
        ops::Range,
        ptr,
        sync::Arc,
    },
};

/// Buffer instance produced by [`crate::buffer::LoadedBuffer`].
pub struct LoadedBufferInstance {
    handle: *mut c_void,
    library: Arc<BufferLibraryGuard>,
}

// SAFETY: every per-buffer trampoline catches unwinds and the user
// `Buffer` impl in the cdylib is required by the `Buffer` trait to be
// `Send + Sync`. `library` is already `Send + Sync` (see
// `BufferLibraryGuard`).
unsafe impl Send for LoadedBufferInstance {}
unsafe impl Sync for LoadedBufferInstance {}

impl LoadedBufferInstance {
    /// Wrap a raw FFI buffer handle alongside a shared guard.
    pub(crate) const fn from_raw(handle: *mut c_void, library: Arc<BufferLibraryGuard>) -> Self {
        Self { handle, library }
    }
}

impl Drop for LoadedBufferInstance {
    fn drop(&mut self) {
        // SAFETY: `buffer_destroy` was validated at load time and the
        // cdylib is still mapped (this struct holds an `Arc` clone of
        // `BufferLibraryGuard`, which is dropped after this method
        // returns and only unmaps the library on the final clone).
        unsafe { (self.library.vtable.buffer_destroy)(self.handle) };
    }
}

#[allow(clippy::cast_possible_truncation)] // usize ↔ u64 is sound on 64-bit
impl Buffer for LoadedBufferInstance {
    fn id(&self) -> BufferId {
        // SAFETY: `buffer_id` was validated at load time. `self.handle`
        // is the FFI buffer handle returned by the driver; it is alive
        // because we hold an `Arc` clone of `BufferLibraryGuard`.
        let raw = unsafe { (self.library.vtable.buffer_id)(self.handle) };
        BufferId::from_raw(raw as usize)
    }

    fn file_path(&self) -> Option<String> {
        let mut out_ptr: *mut c_char = ptr::null_mut();
        let mut out_len: usize = 0;
        // SAFETY: `buffer_file_path` was validated at load time.
        let rc = unsafe {
            (self.library.vtable.buffer_file_path)(self.handle, &raw mut out_ptr, &raw mut out_len)
        };
        if rc != 0 || out_ptr.is_null() {
            // No path / error path: the macro guarantees out_ptr is
            // null when there is no path, so nothing to free.
            return None;
        }
        // SAFETY: the driver allocated `out_len` bytes at `out_ptr`;
        // we copy them out before handing the allocation back via
        // `destroy_file_path`.
        let bytes = unsafe { std::slice::from_raw_parts(out_ptr.cast::<u8>(), out_len) };
        let s = String::from_utf8_lossy(bytes).into_owned();
        // SAFETY: `out_ptr` is the driver-allocated buffer from this
        // call; the destructor matches the allocator.
        unsafe { (self.library.vtable.destroy_file_path)(out_ptr, out_len) };
        Some(s)
    }

    fn set_file_path(&self, path: Option<String>) -> Result<(), BufferError> {
        let (ptr_, len) = path
            .as_deref()
            .map_or((ptr::null::<c_char>(), 0usize), |p| (p.as_ptr().cast::<c_char>(), p.len()));
        let mut err_ptr: *mut c_char = ptr::null_mut();
        // SAFETY: `buffer_set_file_path` was validated at load time;
        // the path bytes are pinned for this call.
        let rc = unsafe {
            (self.library.vtable.buffer_set_file_path)(self.handle, ptr_, len, &raw mut err_ptr)
        };
        rc_to_unit(rc, err_ptr, &self.library)
    }

    fn is_modified(&self) -> bool {
        // SAFETY: `buffer_is_modified` was validated at load time.
        let raw = unsafe { (self.library.vtable.buffer_is_modified)(self.handle) };
        raw != 0
    }

    fn size(&self) -> usize {
        // SAFETY: `buffer_size` was validated at load time.
        unsafe { (self.library.vtable.buffer_size)(self.handle) }
    }

    fn read_bytes(&self, range: Range<usize>) -> Result<Vec<u8>, BufferError> {
        let mut out_ptr: *mut u8 = ptr::null_mut();
        let mut out_len: usize = 0;
        let mut err_ptr: *mut c_char = ptr::null_mut();
        // SAFETY: `buffer_read_bytes` was validated at load time.
        let rc = unsafe {
            (self.library.vtable.buffer_read_bytes)(
                self.handle,
                range.start,
                range.end,
                &raw mut out_ptr,
                &raw mut out_len,
                &raw mut err_ptr,
            )
        };
        match crate::rc::classify_rc(rc, err_ptr, self.library.vtable) {
            crate::rc::RcOutcome::Ok => {
                let bytes = if out_ptr.is_null() || out_len == 0 {
                    Vec::new()
                } else {
                    // SAFETY: driver-allocated buffer; copy then free.
                    let slice = unsafe { std::slice::from_raw_parts(out_ptr, out_len) };
                    slice.to_vec()
                };
                if !out_ptr.is_null() {
                    // SAFETY: matched destructor for the same call.
                    unsafe { (self.library.vtable.destroy_byte_buffer)(out_ptr, out_len) };
                }
                Ok(bytes)
            }
            other => Err(crate::buffer::rc_to_buffer_error(other)),
        }
    }

    fn apply_edit(&self, edit: ByteEdit) -> Result<(), BufferError> {
        // `byte_edit_to_ffi` borrows from `edit`'s `Vec<u8>` buffers;
        // we keep `edit` on this stack frame until the FFI call returns.
        let ffi: FfiByteEdit = byte_edit_to_ffi(&edit);
        let mut err_ptr: *mut c_char = ptr::null_mut();
        // SAFETY: `buffer_apply_edit` was validated at load time. The
        // byte slices inside `ffi` are pinned by `edit` for the whole
        // FFI dispatch.
        let rc = unsafe {
            (self.library.vtable.buffer_apply_edit)(self.handle, &raw const ffi, &raw mut err_ptr)
        };
        // Keep `edit` alive across the call — explicit `drop` documents
        // the lifetime requirement.
        drop(edit);
        rc_to_unit(rc, err_ptr, &self.library)
    }

    fn write_to(&self, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
        // Bridge `vtable.buffer_write_to` to the host writer through a
        // C callback. The cdylib invokes `write_cb` zero or more times
        // with chunks of bytes and we forward each chunk to `writer`.
        // Errors from `writer` propagate via the `last_err` slot in the
        // context; we surface them after the FFI call returns.
        struct Ctx<'a> {
            writer: &'a mut dyn std::io::Write,
            last_err: Option<std::io::Error>,
        }
        unsafe extern "C" fn write_cb(
            ctx: *mut c_void,
            bytes: *const u8,
            len: usize,
        ) -> std::ffi::c_int {
            if ctx.is_null() {
                return -1;
            }
            // SAFETY: `ctx` was passed in by the host (this method)
            // and points at a `Ctx<'_>` on the host stack alive for
            // the duration of the FFI call. The byte slice is
            // cdylib-pinned for this call.
            let ctx = unsafe { &mut *ctx.cast::<Ctx<'_>>() };
            let slice: &[u8] = if len == 0 || bytes.is_null() {
                &[]
            } else {
                // SAFETY: cdylib pinned for this call per the contract.
                unsafe { std::slice::from_raw_parts(bytes, len) }
            };
            match ctx.writer.write_all(slice) {
                Ok(()) => 0,
                Err(e) => {
                    ctx.last_err = Some(e);
                    -1
                }
            }
        }

        let mut ctx = Ctx {
            writer,
            last_err: None,
        };
        let ctx_ptr: *mut c_void = (&raw mut ctx).cast();
        let mut err_ptr: *mut c_char = ptr::null_mut();

        // SAFETY: `buffer_write_to` was validated at load time. `ctx`
        // outlives the FFI call (it is on this stack frame).
        let rc = unsafe {
            (self.library.vtable.buffer_write_to)(self.handle, write_cb, ctx_ptr, &raw mut err_ptr)
        };
        match crate::rc::classify_rc(rc, err_ptr, self.library.vtable) {
            crate::rc::RcOutcome::Ok => Ok(()),
            crate::rc::RcOutcome::Panicked => {
                Err(std::io::Error::other("driver panicked at FFI boundary"))
            }
            crate::rc::RcOutcome::Error(msg) => Err(ctx
                .last_err
                .take()
                .unwrap_or_else(|| std::io::Error::other(msg))),
        }
    }

    fn attach_codec(
        &self,
        _name: &str,
        _codec: Box<dyn ByteNotifiable>,
    ) -> Result<CodecAttachmentId, BufferError> {
        // The host-side codec FFI adapter is symmetric to the cdylib
        // adapter (`__ReovimBufferFfiCodecAdapter` in the driver macro)
        // but lives in the OPPOSITE allocator domain. Building it
        // requires pinning the `Box<dyn ByteNotifiable>` plus a stable
        // C function-pointer pair (`notify` + `destroy_handle`) plus a
        // host-pinned name string for the slot lifetime. Deferred per
        // `tmp/deferral-draft-sp03-codec-ffi-adapter.md`. Returning a
        // structured error rather than silently succeeding so any
        // caller that exercises this slot in SP03 gets a clear signal.
        Err(BufferError::Driver(
            "attach_codec: host-side FFI adapter not yet implemented".to_owned(),
        ))
    }

    fn detach_codec(&self, _id: CodecAttachmentId) -> Result<Box<dyn ByteNotifiable>, BufferError> {
        // Symmetric to `attach_codec`. Deferred per
        // `tmp/deferral-draft-sp03-codec-ffi-adapter.md`.
        Err(BufferError::Driver(
            "detach_codec: host-side FFI adapter not yet implemented".to_owned(),
        ))
    }

    fn list_codecs(&self) -> Vec<(CodecAttachmentId, String)> {
        let mut out_ids: *mut u32 = ptr::null_mut();
        let mut out_names: *mut reovim_subsys_buffer::abi::FfiCodecName = ptr::null_mut();
        let mut count: usize = 0;
        let mut err_ptr: *mut c_char = ptr::null_mut();
        // SAFETY: `buffer_list_codecs` was validated at load time.
        let rc = unsafe {
            (self.library.vtable.buffer_list_codecs)(
                self.handle,
                &raw mut out_ids,
                &raw mut out_names,
                &raw mut count,
                &raw mut err_ptr,
            )
        };
        // Best-effort: errors collapse to an empty list. Round-trip the
        // error string through the destructor to avoid leaks.
        if !matches!(
            crate::rc::classify_rc(rc, err_ptr, self.library.vtable),
            crate::rc::RcOutcome::Ok
        ) {
            return Vec::new();
        }
        let mut entries: Vec<(CodecAttachmentId, String)> = Vec::with_capacity(count);
        if !out_ids.is_null() && !out_names.is_null() && count != 0 {
            // SAFETY: parallel arrays of length `count` allocated by
            // the driver's `buffer_list_codecs` slot.
            let ids_slice = unsafe { std::slice::from_raw_parts(out_ids, count) };
            let names_slice = unsafe { std::slice::from_raw_parts(out_names, count) };
            for (raw_id, name_entry) in ids_slice.iter().zip(names_slice.iter()) {
                let name = if name_entry.ptr.is_null() || name_entry.len == 0 {
                    String::new()
                } else {
                    // SAFETY: driver-pinned name bytes for the duration
                    // of the array's lifetime; copied before destructor.
                    let bytes = unsafe {
                        std::slice::from_raw_parts(name_entry.ptr.cast::<u8>(), name_entry.len)
                    };
                    String::from_utf8_lossy(bytes).into_owned()
                };
                if let Some(nz) = std::num::NonZeroU32::new(*raw_id) {
                    entries.push((CodecAttachmentId(nz), name));
                }
            }
        }
        // SAFETY: matched destructor for the same call's allocations.
        unsafe {
            (self.library.vtable.destroy_codec_name_array)(out_ids, out_names, count);
        }
        entries
    }
}

/// Translate an FFI `c_int` + error-string out-param into
/// `Result<(), BufferError>`. Round-trips any driver-allocated error
/// string through the vtable's `destroy_error_string` slot.
fn rc_to_unit(
    rc: std::ffi::c_int,
    err_ptr: *mut c_char,
    library: &BufferLibraryGuard,
) -> Result<(), BufferError> {
    match crate::rc::classify_rc(rc, err_ptr, library.vtable) {
        crate::rc::RcOutcome::Ok => Ok(()),
        other => Err(crate::buffer::rc_to_buffer_error(other)),
    }
}
