//! Codegen for `declare_buffer_driver!`.
//!
//! Expands to a `#[repr(C)]` static vtable exported under the symbol
//! `REOVIM_BUFFER_DRIVER_VTABLE`, plus the per-slot
//! `unsafe extern "C"` trampoline bodies that route through the
//! `BufferDriver` and `Buffer` traits in `reovim-subsys-buffer`.
//!
//! Every trampoline body is wrapped in
//! `catch_unwind(AssertUnwindSafe(|| ...))` so panics never unwind
//! across the C ABI.
//!
//! See `docs/architecture/driver-abi-v1.md` for the binary contract.
//!
//! # Required driver type contract
//!
//! The user driver type passed to `declare_buffer_driver!(MyDriver)`
//! must:
//!
//! 1. Implement `::reovim_subsys_buffer::BufferDriver` (the factory trait).
//! 2. Provide an inherent
//!    `pub fn probe() -> ::reovim_subsys_buffer::abi::BufferDriverProbe`.
//! 3. Provide an inherent
//!    `pub fn construct() -> Result<Self, ::reovim_subsys_buffer::BufferError>`.
//!
//! These are inherent methods, not a separate trait, so the driver
//! crate can implement them in idiomatic Rust without dragging a
//! lifecycle trait into subsys-buffer.
//!
//! # Buffer handle representation
//!
//! Each FFI buffer pointer (`*mut c_void` returned by `create_buffer`,
//! `open_buffer`, `get_buffer`) is a `*mut Arc<dyn Buffer>` — an
//! `Arc<dyn Buffer>` heap-allocated via `Box::new` then leaked through
//! `Box::into_raw`. The `buffer_destroy` slot reconstructs the `Box`
//! and drops it (releasing the `Arc` refcount); the buffer itself
//! lives as long as any other `Arc` clone inside the driver.
//!
//! # Codec adapter side table
//!
//! `FfiCodecAttachment` enters via `buffer_attach_codec` and must be
//! returned to the host on `buffer_detach_codec` so the host can run
//! its own `destroy_handle`. The macro-generated trampolines maintain
//! a process-global side table keyed by `(buffer_ptr_addr, slot_id)`
//! that stores the attachment fields between attach and detach. On
//! detach the trampoline `mem::forget`s the returned adapter `Box` to
//! suppress the cdylib-side `Drop` (which would call `destroy_handle`
//! a second time) and writes the original attachment into
//! `*out_attachment`. Buffer destruction sweeps the side table for any
//! still-live entries belonging to that buffer.

use {
    proc_macro::TokenStream,
    quote::quote,
    syn::{Ident, parse_macro_input},
};

#[allow(clippy::too_many_lines)] // FFI codegen emits 27 trampolines + helpers
pub fn expand(input: TokenStream) -> TokenStream {
    let driver_type: Ident = parse_macro_input!(input as Ident);

    let expanded = quote! {
        // Main driver vtable exported under the canonical symbol name.
        #[unsafe(no_mangle)]
        pub static REOVIM_BUFFER_DRIVER_VTABLE:
            ::reovim_subsys_buffer::abi::BufferVTable =
            ::reovim_subsys_buffer::abi::BufferVTable {
                abi_version:
                    ::reovim_subsys_buffer::abi::REOVIM_BUFFER_DRIVER_ABI_VERSION,
                api_version:
                    ::reovim_subsys_buffer::abi::REOVIM_BUFFER_DRIVER_API_VERSION,
                size_of_self: ::std::mem::size_of::<
                    ::reovim_subsys_buffer::abi::BufferVTable,
                >(),
                probe: __reovim_buffer_probe,
                construct: __reovim_buffer_construct,
                create_buffer: __reovim_buffer_create_buffer,
                open_buffer: __reovim_buffer_open_buffer,
                list_buffers: __reovim_buffer_list_buffers,
                get_buffer: __reovim_buffer_get_buffer,
                close_buffer: __reovim_buffer_close_buffer,
                destroy_id_list: __reovim_buffer_destroy_id_list,
                buffer_id: __reovim_buffer_buffer_id,
                buffer_size: __reovim_buffer_buffer_size,
                buffer_is_modified: __reovim_buffer_buffer_is_modified,
                buffer_file_path: __reovim_buffer_buffer_file_path,
                buffer_set_file_path: __reovim_buffer_buffer_set_file_path,
                buffer_read_bytes: __reovim_buffer_buffer_read_bytes,
                buffer_apply_edit: __reovim_buffer_buffer_apply_edit,
                buffer_write_to: __reovim_buffer_buffer_write_to,
                buffer_attach_codec: __reovim_buffer_buffer_attach_codec,
                buffer_detach_codec: __reovim_buffer_buffer_detach_codec,
                buffer_list_codecs: __reovim_buffer_buffer_list_codecs,
                buffer_subscribe_edits: __reovim_buffer_buffer_subscribe_edits,
                buffer_unsubscribe_edits: __reovim_buffer_buffer_unsubscribe_edits,
                buffer_destroy: __reovim_buffer_buffer_destroy,
                destroy: __reovim_buffer_destroy,
                destroy_error_string: __reovim_buffer_destroy_error_string,
                destroy_byte_buffer: __reovim_buffer_destroy_byte_buffer,
                destroy_file_path: __reovim_buffer_destroy_file_path,
                destroy_codec_name_array:
                    __reovim_buffer_destroy_codec_name_array,
            };

        // ────────────────────────────────────────────────────────────
        // Codec attachment side table.
        //
        // Maps `(buffer_ptr_addr, slot_id)` to the original
        // `FfiCodecAttachment` fields the host passed to
        // `buffer_attach_codec`. `buffer_detach_codec` reads this map
        // to rebuild the attachment for the host before the
        // adapter `Box` is `mem::forget`-ed (suppressing the cdylib
        // side `destroy_handle` call so the host owns destruction).
        //
        // Stored fields are copied verbatim from the FFI value; the
        // raw pointers inside (`name_ptr`, `handle`) belong to the
        // host / codec and the side table never frees them.
        // ────────────────────────────────────────────────────────────

        #[doc(hidden)]
        #[derive(Clone, Copy)]
        struct __ReovimBufferAttachmentRecord {
            name_ptr: *const ::std::ffi::c_char,
            name_len: usize,
            handle: *mut ::std::ffi::c_void,
            notify: unsafe extern "C" fn(
                handle: *mut ::std::ffi::c_void,
                edit: *const ::reovim_subsys_buffer::abi::FfiByteEdit,
                out_err: *mut *mut ::std::ffi::c_char,
            ) -> ::std::ffi::c_int,
            destroy_handle: unsafe extern "C" fn(handle: *mut ::std::ffi::c_void),
        }

        // SAFETY: the raw pointer fields are codec / host-owned and
        // are never dereferenced by the side table itself; the
        // cdylib only stores them so it can copy them back to the
        // host on detach. Send + Sync is therefore sound.
        #[allow(unsafe_code)]
        unsafe impl ::std::marker::Send for __ReovimBufferAttachmentRecord {}
        #[allow(unsafe_code)]
        unsafe impl ::std::marker::Sync for __ReovimBufferAttachmentRecord {}

        #[doc(hidden)]
        fn __reovim_buffer_attachment_table()
            -> &'static ::std::sync::RwLock<
                ::std::collections::HashMap<
                    (usize, u32),
                    __ReovimBufferAttachmentRecord,
                >,
            >
        {
            static TABLE: ::std::sync::OnceLock<
                ::std::sync::RwLock<
                    ::std::collections::HashMap<
                        (usize, u32),
                        __ReovimBufferAttachmentRecord,
                    >,
                >,
            > = ::std::sync::OnceLock::new();
            TABLE.get_or_init(|| {
                ::std::sync::RwLock::new(::std::collections::HashMap::new())
            })
        }

        // ────────────────────────────────────────────────────────────
        // FfiCodecAdapter — host-side `ByteNotifiable` impl that
        // forwards `notify` calls back through the FFI function
        // pointer the host installed at attach time.
        // ────────────────────────────────────────────────────────────

        #[doc(hidden)]
        struct __ReovimBufferFfiCodecAdapter {
            handle: *mut ::std::ffi::c_void,
            notify: unsafe extern "C" fn(
                handle: *mut ::std::ffi::c_void,
                edit: *const ::reovim_subsys_buffer::abi::FfiByteEdit,
                out_err: *mut *mut ::std::ffi::c_char,
            ) -> ::std::ffi::c_int,
            destroy_handle: unsafe extern "C" fn(handle: *mut ::std::ffi::c_void),
        }

        // SAFETY: the codec handle is host-allocated and the host
        // contract guarantees its function pointers are safe to
        // call from any thread; codecs are required to be `Send +
        // Sync`. Host-managed lifetime; cdylib only forwards.
        #[allow(unsafe_code)]
        unsafe impl ::std::marker::Send for __ReovimBufferFfiCodecAdapter {}
        #[allow(unsafe_code)]
        unsafe impl ::std::marker::Sync for __ReovimBufferFfiCodecAdapter {}

        impl ::std::ops::Drop for __ReovimBufferFfiCodecAdapter {
            fn drop(&mut self) {
                // SAFETY: `destroy_handle` is the codec-supplied
                // destructor. It runs only when the cdylib drops
                // the adapter `Box` — that is, on buffer destruction
                // or on detach paths that did NOT mem::forget the
                // adapter. The detach trampoline mem::forgets the
                // adapter and writes the attachment back to the
                // host so the host can invoke `destroy_handle`
                // itself.
                unsafe { (self.destroy_handle)(self.handle); }
            }
        }

        impl ::reovim_content_codec::ByteNotifiable
            for __ReovimBufferFfiCodecAdapter
        {
            fn build(&mut self, _raw: &[u8]) {
                // FFI codecs do not expose a `build` slot in
                // `FfiCodecAttachment`. Hosts that attach a codec
                // through the cdylib path are expected to issue an
                // explicit synthetic edit if they need warm-start
                // semantics; the cdylib forwards `notify` only.
            }

            fn notify(&mut self, edit: &::reovim_kernel::api::v1::ByteEdit) {
                let ffi = ::reovim_subsys_buffer::abi::byte_edit_to_ffi(edit);
                let mut err_ptr: *mut ::std::ffi::c_char = ::std::ptr::null_mut();
                // SAFETY: `self.handle` was supplied by the host at
                // attach time and is alive for the slot lifetime.
                // `&ffi` lives for the duration of this call. The
                // host contract guarantees `notify` may be invoked
                // synchronously from inside `apply_edit`.
                let rc = unsafe {
                    (self.notify)(self.handle, &ffi as *const _, &mut err_ptr)
                };
                if rc != 0 {
                    // The host allocated `err_ptr`; we have no FFI
                    // slot to free it from the cdylib side, so this
                    // is best-effort logging only. The host's slot
                    // poisoning policy applies.
                    if !err_ptr.is_null() {
                        // SAFETY: `err_ptr` is host-allocated via
                        // `CString::into_raw` per the codec contract;
                        // we leak it (no cdylib-side destructor is
                        // available across this boundary).
                        let _ = err_ptr;
                    }
                }
            }
        }

        // ────────────────────────────────────────────────────────────
        // CallbackWriter — `std::io::Write` adapter that forwards
        // each write into the host-supplied callback.
        // ────────────────────────────────────────────────────────────

        #[doc(hidden)]
        struct __ReovimBufferCallbackWriter {
            ctx: *mut ::std::ffi::c_void,
            cb: unsafe extern "C" fn(
                ctx: *mut ::std::ffi::c_void,
                bytes: *const u8,
                len: usize,
            ) -> ::std::ffi::c_int,
        }

        impl ::std::io::Write for __ReovimBufferCallbackWriter {
            fn write(&mut self, buf: &[u8]) -> ::std::io::Result<usize> {
                // SAFETY: `self.ctx` is the host-supplied opaque
                // value; the callback contract guarantees safe
                // invocation. `buf` is borrowed for the duration
                // of this call.
                let rc = unsafe {
                    (self.cb)(self.ctx, buf.as_ptr(), buf.len())
                };
                if rc == 0 {
                    ::std::result::Result::Ok(buf.len())
                } else {
                    ::std::result::Result::Err(::std::io::Error::other(
                        "host write callback returned non-zero status",
                    ))
                }
            }

            fn flush(&mut self) -> ::std::io::Result<()> {
                ::std::result::Result::Ok(())
            }
        }

        // ────────────────────────────────────────────────────────────
        // Helpers.
        // ────────────────────────────────────────────────────────────

        #[doc(hidden)]
        fn __reovim_buffer_write_err(
            out_err: *mut *mut ::std::ffi::c_char,
            msg: ::std::string::String,
        ) {
            if out_err.is_null() {
                return;
            }
            let cs = match ::std::ffi::CString::new(msg) {
                ::std::result::Result::Ok(cs) => cs,
                ::std::result::Result::Err(_) => ::std::ffi::CString::new(
                    "buffer error (interior nul)",
                ).unwrap(),
            };
            // SAFETY: `out_err` is a host-provided writable slot.
            unsafe { *out_err = cs.into_raw(); }
        }

        #[doc(hidden)]
        // # Safety
        //
        // `path_ptr[0..path_len]` must be a host-pinned readable byte
        // slice, or `path_ptr` must be null with `path_len == 0`.
        unsafe fn __reovim_buffer_path_from_ffi(
            path_ptr: *const ::std::ffi::c_char,
            path_len: usize,
        ) -> ::std::option::Option<::std::string::String> {
            if path_ptr.is_null() || path_len == 0 {
                return ::std::option::Option::None;
            }
            // SAFETY: caller's contract.
            let bytes: &[u8] = unsafe {
                ::std::slice::from_raw_parts(path_ptr.cast::<u8>(), path_len)
            };
            ::std::option::Option::Some(
                ::std::string::String::from_utf8_lossy(bytes).into_owned(),
            )
        }

        #[doc(hidden)]
        fn __reovim_buffer_box_arc_buffer(
            arc: ::std::sync::Arc<dyn ::reovim_subsys_buffer::Buffer>,
        ) -> *mut ::std::ffi::c_void {
            let boxed: ::std::boxed::Box<
                ::std::sync::Arc<dyn ::reovim_subsys_buffer::Buffer>,
            > = ::std::boxed::Box::new(arc);
            ::std::boxed::Box::into_raw(boxed).cast()
        }

        // ────────────────────────────────────────────────────────────
        // Lifecycle trampolines.
        // ────────────────────────────────────────────────────────────

        #[doc(hidden)]
        unsafe extern "C" fn __reovim_buffer_probe()
            -> ::reovim_subsys_buffer::abi::BufferDriverProbe
        {
            ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                <#driver_type>::probe()
            }))
            .unwrap_or_else(|_|
                ::reovim_subsys_buffer::abi::BufferDriverProbe::new("", "")
            )
        }

        #[doc(hidden)]
        unsafe extern "C" fn __reovim_buffer_construct(
            out_instance: *mut *mut ::std::ffi::c_void,
            out_err: *mut *mut ::std::ffi::c_char,
        ) -> ::std::ffi::c_int {
            let result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                <#driver_type>::construct()
            }));
            match result {
                ::std::result::Result::Ok(::std::result::Result::Ok(driver)) => {
                    let boxed = ::std::boxed::Box::new(driver);
                    // SAFETY: out_instance is host-provided writable.
                    unsafe { *out_instance = ::std::boxed::Box::into_raw(boxed).cast(); }
                    0
                }
                ::std::result::Result::Ok(::std::result::Result::Err(e)) => {
                    __reovim_buffer_write_err(out_err, ::std::format!("{}", e));
                    -1
                }
                ::std::result::Result::Err(_) => -2,
            }
        }

        // ────────────────────────────────────────────────────────────
        // BufferDriver factory trampolines.
        // ────────────────────────────────────────────────────────────

        #[doc(hidden)]
        unsafe extern "C" fn __reovim_buffer_create_buffer(
            instance: *mut ::std::ffi::c_void,
            bytes_ptr: *const u8,
            bytes_len: usize,
            path_ptr: *const ::std::ffi::c_char,
            path_len: usize,
            out_buffer: *mut *mut ::std::ffi::c_void,
            out_id: *mut u64,
            out_err: *mut *mut ::std::ffi::c_char,
        ) -> ::std::ffi::c_int {
            let result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                if instance.is_null() {
                    return ::std::result::Result::Err(
                        ::reovim_subsys_buffer::BufferError::Driver(
                            ::std::string::String::from(
                                "null instance in create_buffer",
                            ),
                        ),
                    );
                }
                // SAFETY: instance came from Box::into_raw in construct;
                // we borrow rather than consume so destroy can free it.
                let driver = unsafe {
                    &*(instance as *const #driver_type)
                };
                let bytes: &[u8] = if bytes_len == 0 {
                    &[]
                } else {
                    // SAFETY: host-pinned for the call.
                    unsafe {
                        ::std::slice::from_raw_parts(bytes_ptr, bytes_len)
                    }
                };
                // SAFETY: path bytes are host-pinned per the FFI contract.
                let path = unsafe {
                    __reovim_buffer_path_from_ffi(path_ptr, path_len)
                };
                <#driver_type as ::reovim_subsys_buffer::BufferDriver>
                    ::create_buffer(driver, bytes, path)
            }));
            match result {
                ::std::result::Result::Ok(::std::result::Result::Ok(arc)) => {
                    let id = arc.id().as_usize() as u64;
                    let raw = __reovim_buffer_box_arc_buffer(arc);
                    // SAFETY: out_buffer / out_id are host-provided writable.
                    unsafe {
                        *out_buffer = raw;
                        *out_id = id;
                    }
                    0
                }
                ::std::result::Result::Ok(::std::result::Result::Err(e)) => {
                    __reovim_buffer_write_err(out_err, ::std::format!("{}", e));
                    -1
                }
                ::std::result::Result::Err(_) => -2,
            }
        }

        #[doc(hidden)]
        unsafe extern "C" fn __reovim_buffer_open_buffer(
            instance: *mut ::std::ffi::c_void,
            path_ptr: *const ::std::ffi::c_char,
            path_len: usize,
            out_buffer: *mut *mut ::std::ffi::c_void,
            out_id: *mut u64,
            out_err: *mut *mut ::std::ffi::c_char,
        ) -> ::std::ffi::c_int {
            let result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                if instance.is_null() {
                    return ::std::result::Result::Err(
                        ::reovim_subsys_buffer::BufferError::Driver(
                            ::std::string::String::from(
                                "null instance in open_buffer",
                            ),
                        ),
                    );
                }
                // SAFETY: instance came from Box::into_raw in construct.
                let driver = unsafe {
                    &*(instance as *const #driver_type)
                };
                // SAFETY: path bytes are host-pinned per the FFI contract.
                let path = unsafe {
                    __reovim_buffer_path_from_ffi(path_ptr, path_len)
                }
                    .ok_or_else(|| ::reovim_subsys_buffer::BufferError::Io(
                        ::std::string::String::from(
                            "open_buffer requires non-empty path",
                        ),
                    ))?;
                <#driver_type as ::reovim_subsys_buffer::BufferDriver>
                    ::open_buffer(driver, &path)
            }));
            match result {
                ::std::result::Result::Ok(::std::result::Result::Ok(arc)) => {
                    let id = arc.id().as_usize() as u64;
                    let raw = __reovim_buffer_box_arc_buffer(arc);
                    // SAFETY: out_buffer / out_id are host-provided writable.
                    unsafe {
                        *out_buffer = raw;
                        *out_id = id;
                    }
                    0
                }
                ::std::result::Result::Ok(::std::result::Result::Err(e)) => {
                    __reovim_buffer_write_err(out_err, ::std::format!("{}", e));
                    -1
                }
                ::std::result::Result::Err(_) => -2,
            }
        }

        #[doc(hidden)]
        unsafe extern "C" fn __reovim_buffer_list_buffers(
            instance: *mut ::std::ffi::c_void,
            out_ids: *mut *mut u64,
            out_count: *mut usize,
            out_err: *mut *mut ::std::ffi::c_char,
        ) -> ::std::ffi::c_int {
            let result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                if instance.is_null() {
                    return ::std::result::Result::Err(
                        ::std::string::String::from(
                            "null instance in list_buffers",
                        ),
                    );
                }
                // SAFETY: instance came from Box::into_raw in construct.
                let driver = unsafe {
                    &*(instance as *const #driver_type)
                };
                let ids = <#driver_type as ::reovim_subsys_buffer::BufferDriver>
                    ::list_buffers(driver);
                ::std::result::Result::Ok(ids)
            }));
            match result {
                ::std::result::Result::Ok(::std::result::Result::Ok(ids)) => {
                    let raw_ids: ::std::vec::Vec<u64> = ids.into_iter()
                        .map(|id| id.as_usize() as u64)
                        .collect();
                    let count = raw_ids.len();
                    // Convert to `Box<[u64]>` so the boxed-slice
                    // allocator layout matches what
                    // `destroy_id_list` reconstructs (slice::len ==
                    // allocated capacity).
                    let boxed: ::std::boxed::Box<[u64]> =
                        raw_ids.into_boxed_slice();
                    let ptr = ::std::boxed::Box::into_raw(boxed).cast::<u64>();
                    // SAFETY: out_ids / out_count are host-provided writable.
                    unsafe {
                        *out_ids = ptr;
                        *out_count = count;
                    }
                    0
                }
                ::std::result::Result::Ok(::std::result::Result::Err(e)) => {
                    __reovim_buffer_write_err(out_err, e);
                    -1
                }
                ::std::result::Result::Err(_) => -2,
            }
        }

        #[doc(hidden)]
        unsafe extern "C" fn __reovim_buffer_get_buffer(
            instance: *mut ::std::ffi::c_void,
            id: u64,
            out_buffer: *mut *mut ::std::ffi::c_void,
            out_err: *mut *mut ::std::ffi::c_char,
        ) -> ::std::ffi::c_int {
            let result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                if instance.is_null() {
                    return ::std::result::Result::Err(
                        ::std::string::String::from(
                            "null instance in get_buffer",
                        ),
                    );
                }
                // SAFETY: instance came from Box::into_raw in construct.
                let driver = unsafe {
                    &*(instance as *const #driver_type)
                };
                let id_typed =
                    ::reovim_kernel::api::v1::BufferId::from_raw(id as usize);
                ::std::result::Result::Ok(
                    <#driver_type as ::reovim_subsys_buffer::BufferDriver>
                        ::get_buffer(driver, id_typed),
                )
            }));
            match result {
                ::std::result::Result::Ok(::std::result::Result::Ok(::std::option::Option::Some(arc))) => {
                    let raw = __reovim_buffer_box_arc_buffer(arc);
                    // SAFETY: out_buffer is host-provided writable.
                    unsafe { *out_buffer = raw; }
                    0
                }
                ::std::result::Result::Ok(::std::result::Result::Ok(::std::option::Option::None)) => {
                    // SAFETY: out_buffer is host-provided writable.
                    unsafe { *out_buffer = ::std::ptr::null_mut(); }
                    -1
                }
                ::std::result::Result::Ok(::std::result::Result::Err(e)) => {
                    __reovim_buffer_write_err(out_err, e);
                    -1
                }
                ::std::result::Result::Err(_) => -2,
            }
        }

        #[doc(hidden)]
        unsafe extern "C" fn __reovim_buffer_close_buffer(
            instance: *mut ::std::ffi::c_void,
            id: u64,
            out_err: *mut *mut ::std::ffi::c_char,
        ) -> ::std::ffi::c_int {
            let result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                if instance.is_null() {
                    return ::std::result::Result::Err(
                        ::reovim_subsys_buffer::BufferError::Driver(
                            ::std::string::String::from(
                                "null instance in close_buffer",
                            ),
                        ),
                    );
                }
                // SAFETY: instance came from Box::into_raw in construct.
                let driver = unsafe {
                    &*(instance as *const #driver_type)
                };
                let id_typed =
                    ::reovim_kernel::api::v1::BufferId::from_raw(id as usize);
                <#driver_type as ::reovim_subsys_buffer::BufferDriver>
                    ::close_buffer(driver, id_typed)
            }));
            match result {
                ::std::result::Result::Ok(::std::result::Result::Ok(())) => 0,
                ::std::result::Result::Ok(::std::result::Result::Err(e)) => {
                    __reovim_buffer_write_err(out_err, ::std::format!("{}", e));
                    -1
                }
                ::std::result::Result::Err(_) => -2,
            }
        }

        #[doc(hidden)]
        unsafe extern "C" fn __reovim_buffer_destroy_id_list(
            ids_ptr: *mut u64,
            count: usize,
        ) {
            if ids_ptr.is_null() || count == 0 {
                return;
            }
            let _ = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                // SAFETY: `ids_ptr` was produced by
                // `Box::<[u64]>::into_raw` in `list_buffers`;
                // reconstructing the boxed slice with the matching
                // length returns the allocation to the driver
                // allocator.
                unsafe {
                    let slice_ptr: *mut [u64] =
                        ::std::ptr::slice_from_raw_parts_mut(ids_ptr, count);
                    ::std::mem::drop(::std::boxed::Box::from_raw(slice_ptr));
                }
            }));
        }

        // ────────────────────────────────────────────────────────────
        // Buffer per-instance trampolines.
        // ────────────────────────────────────────────────────────────

        #[doc(hidden)]
        unsafe extern "C" fn __reovim_buffer_buffer_id(
            buffer: *mut ::std::ffi::c_void,
        ) -> u64 {
            ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                if buffer.is_null() {
                    return 0u64;
                }
                // SAFETY: `buffer` is `*mut Arc<dyn Buffer>` from
                // `__reovim_buffer_box_arc_buffer`.
                let arc = unsafe {
                    &*(buffer as *const ::std::sync::Arc<
                        dyn ::reovim_subsys_buffer::Buffer,
                    >)
                };
                let id = arc.id().as_usize() as u64;
                id
            }))
            .unwrap_or(0)
        }

        #[doc(hidden)]
        unsafe extern "C" fn __reovim_buffer_buffer_size(
            buffer: *mut ::std::ffi::c_void,
        ) -> usize {
            ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                if buffer.is_null() {
                    return 0usize;
                }
                // SAFETY: `buffer` is `*mut Arc<dyn Buffer>`.
                let arc = unsafe {
                    &*(buffer as *const ::std::sync::Arc<
                        dyn ::reovim_subsys_buffer::Buffer,
                    >)
                };
                arc.size()
            }))
            .unwrap_or(0)
        }

        #[doc(hidden)]
        unsafe extern "C" fn __reovim_buffer_buffer_is_modified(
            buffer: *mut ::std::ffi::c_void,
        ) -> u8 {
            ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                if buffer.is_null() {
                    return 0u8;
                }
                // SAFETY: `buffer` is `*mut Arc<dyn Buffer>`.
                let arc = unsafe {
                    &*(buffer as *const ::std::sync::Arc<
                        dyn ::reovim_subsys_buffer::Buffer,
                    >)
                };
                u8::from(arc.is_modified())
            }))
            .unwrap_or(0)
        }

        #[doc(hidden)]
        unsafe extern "C" fn __reovim_buffer_buffer_file_path(
            buffer: *mut ::std::ffi::c_void,
            out_ptr: *mut *mut ::std::ffi::c_char,
            out_len: *mut usize,
        ) -> ::std::ffi::c_int {
            let result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                if buffer.is_null() {
                    return ::std::option::Option::<::std::string::String>::None;
                }
                // SAFETY: `buffer` is `*mut Arc<dyn Buffer>`.
                let arc = unsafe {
                    &*(buffer as *const ::std::sync::Arc<
                        dyn ::reovim_subsys_buffer::Buffer,
                    >)
                };
                arc.file_path()
            }));
            match result {
                ::std::result::Result::Ok(::std::option::Option::Some(s)) => {
                    let cs = match ::std::ffi::CString::new(s) {
                        ::std::result::Result::Ok(cs) => cs,
                        ::std::result::Result::Err(_) => {
                            // SAFETY: out_ptr / out_len writable.
                            unsafe {
                                *out_ptr = ::std::ptr::null_mut();
                                *out_len = 0;
                            }
                            return -1;
                        }
                    };
                    let len = cs.as_bytes().len();
                    let raw = cs.into_raw();
                    // SAFETY: out_ptr / out_len writable; host frees
                    // via `destroy_file_path(raw, len)`.
                    unsafe {
                        *out_ptr = raw;
                        *out_len = len;
                    }
                    0
                }
                ::std::result::Result::Ok(::std::option::Option::None) => {
                    // SAFETY: writing host-provided out-params.
                    unsafe {
                        *out_ptr = ::std::ptr::null_mut();
                        *out_len = 0;
                    }
                    -1
                }
                ::std::result::Result::Err(_) => -2,
            }
        }

        #[doc(hidden)]
        unsafe extern "C" fn __reovim_buffer_buffer_set_file_path(
            buffer: *mut ::std::ffi::c_void,
            path_ptr: *const ::std::ffi::c_char,
            path_len: usize,
            out_err: *mut *mut ::std::ffi::c_char,
        ) -> ::std::ffi::c_int {
            let result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                if buffer.is_null() {
                    return ::std::result::Result::Err(
                        ::reovim_subsys_buffer::BufferError::Driver(
                            ::std::string::String::from(
                                "null buffer in set_file_path",
                            ),
                        ),
                    );
                }
                // SAFETY: `buffer` is `*mut Arc<dyn Buffer>`.
                let arc = unsafe {
                    &*(buffer as *const ::std::sync::Arc<
                        dyn ::reovim_subsys_buffer::Buffer,
                    >)
                };
                // SAFETY: path bytes are host-pinned per the FFI contract.
                let path = unsafe {
                    __reovim_buffer_path_from_ffi(path_ptr, path_len)
                };
                arc.set_file_path(path)
            }));
            match result {
                ::std::result::Result::Ok(::std::result::Result::Ok(())) => 0,
                ::std::result::Result::Ok(::std::result::Result::Err(e)) => {
                    __reovim_buffer_write_err(out_err, ::std::format!("{}", e));
                    -1
                }
                ::std::result::Result::Err(_) => -2,
            }
        }

        #[doc(hidden)]
        unsafe extern "C" fn __reovim_buffer_buffer_read_bytes(
            buffer: *mut ::std::ffi::c_void,
            start: usize,
            end: usize,
            out_ptr: *mut *mut u8,
            out_len: *mut usize,
            out_err: *mut *mut ::std::ffi::c_char,
        ) -> ::std::ffi::c_int {
            let result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                if buffer.is_null() {
                    return ::std::result::Result::Err(
                        ::reovim_subsys_buffer::BufferError::Driver(
                            ::std::string::String::from(
                                "null buffer in read_bytes",
                            ),
                        ),
                    );
                }
                // SAFETY: `buffer` is `*mut Arc<dyn Buffer>`.
                let arc = unsafe {
                    &*(buffer as *const ::std::sync::Arc<
                        dyn ::reovim_subsys_buffer::Buffer,
                    >)
                };
                arc.read_bytes(start..end)
            }));
            match result {
                ::std::result::Result::Ok(::std::result::Result::Ok(v)) => {
                    let len = v.len();
                    // Boxed-slice round-trip so the allocator layout
                    // matches `destroy_byte_buffer`'s reconstruction.
                    let boxed: ::std::boxed::Box<[u8]> = v.into_boxed_slice();
                    let ptr = ::std::boxed::Box::into_raw(boxed).cast::<u8>();
                    // SAFETY: out_ptr / out_len writable; host frees
                    // via `destroy_byte_buffer(ptr, len)`.
                    unsafe {
                        *out_ptr = ptr;
                        *out_len = len;
                    }
                    0
                }
                ::std::result::Result::Ok(::std::result::Result::Err(e)) => {
                    __reovim_buffer_write_err(out_err, ::std::format!("{}", e));
                    -1
                }
                ::std::result::Result::Err(_) => -2,
            }
        }

        #[doc(hidden)]
        unsafe extern "C" fn __reovim_buffer_buffer_apply_edit(
            buffer: *mut ::std::ffi::c_void,
            edit: *const ::reovim_subsys_buffer::abi::FfiByteEdit,
            out_err: *mut *mut ::std::ffi::c_char,
        ) -> ::std::ffi::c_int {
            let result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                if buffer.is_null() || edit.is_null() {
                    return ::std::result::Result::Err(
                        ::reovim_subsys_buffer::BufferError::Driver(
                            ::std::string::String::from(
                                "null pointer in apply_edit",
                            ),
                        ),
                    );
                }
                // SAFETY: `buffer` is `*mut Arc<dyn Buffer>`.
                let arc = unsafe {
                    &*(buffer as *const ::std::sync::Arc<
                        dyn ::reovim_subsys_buffer::Buffer,
                    >)
                };
                // SAFETY: `edit` is host-pinned for the call.
                let ffi_ref = unsafe { &*edit };
                // `byte_edit_from_ffi` copies the byte slices into
                // owned `Vec<u8>` so the resulting `ByteEdit` does
                // not alias the FFI pointers after the call returns.
                // SAFETY: caller pins the byte slices for the call.
                let owned = unsafe {
                    ::reovim_subsys_buffer::abi::byte_edit_from_ffi(ffi_ref)
                };
                arc.apply_edit(owned)
            }));
            match result {
                ::std::result::Result::Ok(::std::result::Result::Ok(())) => 0,
                ::std::result::Result::Ok(::std::result::Result::Err(e)) => {
                    __reovim_buffer_write_err(out_err, ::std::format!("{}", e));
                    -1
                }
                ::std::result::Result::Err(_) => -2,
            }
        }

        #[doc(hidden)]
        unsafe extern "C" fn __reovim_buffer_buffer_write_to(
            buffer: *mut ::std::ffi::c_void,
            write_cb: unsafe extern "C" fn(
                ctx: *mut ::std::ffi::c_void,
                bytes: *const u8,
                len: usize,
            ) -> ::std::ffi::c_int,
            ctx: *mut ::std::ffi::c_void,
            out_err: *mut *mut ::std::ffi::c_char,
        ) -> ::std::ffi::c_int {
            let result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                if buffer.is_null() {
                    return ::std::result::Result::Err(::std::io::Error::other(
                        "null buffer in write_to",
                    ));
                }
                // SAFETY: `buffer` is `*mut Arc<dyn Buffer>`.
                let arc = unsafe {
                    &*(buffer as *const ::std::sync::Arc<
                        dyn ::reovim_subsys_buffer::Buffer,
                    >)
                };
                let mut writer = __ReovimBufferCallbackWriter {
                    ctx,
                    cb: write_cb,
                };
                arc.write_to(&mut writer)
            }));
            match result {
                ::std::result::Result::Ok(::std::result::Result::Ok(())) => 0,
                ::std::result::Result::Ok(::std::result::Result::Err(e)) => {
                    __reovim_buffer_write_err(out_err, ::std::format!("{}", e));
                    -1
                }
                ::std::result::Result::Err(_) => -2,
            }
        }

        // ────────────────────────────────────────────────────────────
        // Codec attachment trampolines.
        // ────────────────────────────────────────────────────────────

        #[doc(hidden)]
        unsafe extern "C" fn __reovim_buffer_buffer_attach_codec(
            buffer: *mut ::std::ffi::c_void,
            attachment: *const ::reovim_subsys_buffer::abi::FfiCodecAttachment,
            out_id: *mut u32,
            out_err: *mut *mut ::std::ffi::c_char,
        ) -> ::std::ffi::c_int {
            let result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                if buffer.is_null() || attachment.is_null() {
                    return ::std::result::Result::Err(
                        ::reovim_subsys_buffer::BufferError::Driver(
                            ::std::string::String::from(
                                "null pointer in attach_codec",
                            ),
                        ),
                    );
                }
                // SAFETY: `buffer` is `*mut Arc<dyn Buffer>`.
                let arc = unsafe {
                    &*(buffer as *const ::std::sync::Arc<
                        dyn ::reovim_subsys_buffer::Buffer,
                    >)
                };
                // SAFETY: `attachment` is host-pinned for the call.
                let att = unsafe { &*attachment };
                // Recover the codec name from the host-pinned bytes.
                let name_bytes: &[u8] = if att.name_len == 0 {
                    &[]
                } else {
                    // SAFETY: host pins the name string for the slot
                    // lifetime per the `FfiCodecAttachment` contract.
                    unsafe {
                        ::std::slice::from_raw_parts(
                            att.name_ptr.cast::<u8>(),
                            att.name_len,
                        )
                    }
                };
                let name = ::std::string::String::from_utf8_lossy(name_bytes)
                    .into_owned();
                let adapter = __ReovimBufferFfiCodecAdapter {
                    handle: att.handle,
                    notify: att.notify,
                    destroy_handle: att.destroy_handle,
                };
                let record = __ReovimBufferAttachmentRecord {
                    name_ptr: att.name_ptr,
                    name_len: att.name_len,
                    handle: att.handle,
                    notify: att.notify,
                    destroy_handle: att.destroy_handle,
                };
                let attach_result = arc.attach_codec(&name, ::std::boxed::Box::new(adapter));
                attach_result.map(|id| (id, record))
            }));
            match result {
                ::std::result::Result::Ok(::std::result::Result::Ok((id, record))) => {
                    let slot = id.0.get();
                    let key = (buffer as usize, slot);
                    if let ::std::result::Result::Ok(mut tbl) =
                        __reovim_buffer_attachment_table().write()
                    {
                        tbl.insert(key, record);
                    }
                    // SAFETY: out_id is host-provided writable.
                    unsafe { *out_id = slot; }
                    0
                }
                ::std::result::Result::Ok(::std::result::Result::Err(e)) => {
                    __reovim_buffer_write_err(out_err, ::std::format!("{}", e));
                    -1
                }
                ::std::result::Result::Err(_) => -2,
            }
        }

        #[doc(hidden)]
        unsafe extern "C" fn __reovim_buffer_buffer_detach_codec(
            buffer: *mut ::std::ffi::c_void,
            id: u32,
            out_attachment: *mut ::reovim_subsys_buffer::abi::FfiCodecAttachment,
            out_err: *mut *mut ::std::ffi::c_char,
        ) -> ::std::ffi::c_int {
            let result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                if buffer.is_null() || out_attachment.is_null() {
                    return ::std::result::Result::Err(
                        ::reovim_subsys_buffer::BufferError::Driver(
                            ::std::string::String::from(
                                "null pointer in detach_codec",
                            ),
                        ),
                    );
                }
                let slot_id = match ::std::num::NonZeroU32::new(id) {
                    ::std::option::Option::Some(s) => s,
                    ::std::option::Option::None => {
                        return ::std::result::Result::Err(
                            ::reovim_subsys_buffer::BufferError::Driver(
                                ::std::string::String::from(
                                    "detach_codec: id must be non-zero",
                                ),
                            ),
                        );
                    }
                };
                // SAFETY: `buffer` is `*mut Arc<dyn Buffer>`.
                let arc = unsafe {
                    &*(buffer as *const ::std::sync::Arc<
                        dyn ::reovim_subsys_buffer::Buffer,
                    >)
                };
                let codec_id =
                    ::reovim_subsys_buffer::CodecAttachmentId(slot_id);
                let detach_result = arc.detach_codec(codec_id);
                detach_result.map(|boxed| {
                    // Suppress the cdylib-side `Drop` so the host
                    // owns destruction of `handle` via the returned
                    // `FfiCodecAttachment.destroy_handle` field.
                    ::std::mem::forget(boxed);
                })
            }));
            match result {
                ::std::result::Result::Ok(::std::result::Result::Ok(())) => {
                    let key = (buffer as usize, id);
                    let record = if let ::std::result::Result::Ok(mut tbl) =
                        __reovim_buffer_attachment_table().write()
                    {
                        tbl.remove(&key)
                    } else {
                        ::std::option::Option::None
                    };
                    let record = match record {
                        ::std::option::Option::Some(r) => r,
                        ::std::option::Option::None => {
                            // The buffer impl returned a Box that the
                            // cdylib did not record on attach. This
                            // happens only if a Rust-side caller
                            // attached a non-FFI codec directly to the
                            // buffer, which the cdylib loader path
                            // never does. The cdylib cannot route a
                            // host-foreign adapter back through FFI.
                            __reovim_buffer_write_err(
                                out_err,
                                ::std::string::String::from(
                                    "detach_codec: slot not FFI-attached",
                                ),
                            );
                            return -3;
                        }
                    };
                    let out = ::reovim_subsys_buffer::abi::FfiCodecAttachment {
                        name_ptr: record.name_ptr,
                        name_len: record.name_len,
                        handle: record.handle,
                        notify: record.notify,
                        destroy_handle: record.destroy_handle,
                    };
                    // SAFETY: out_attachment is host-provided writable.
                    unsafe {
                        ::std::ptr::write(out_attachment, out);
                    }
                    0
                }
                ::std::result::Result::Ok(::std::result::Result::Err(e)) => {
                    __reovim_buffer_write_err(out_err, ::std::format!("{}", e));
                    -1
                }
                ::std::result::Result::Err(_) => -2,
            }
        }

        #[doc(hidden)]
        unsafe extern "C" fn __reovim_buffer_buffer_list_codecs(
            buffer: *mut ::std::ffi::c_void,
            out_ids: *mut *mut u32,
            out_names: *mut *mut ::reovim_subsys_buffer::abi::FfiCodecName,
            out_count: *mut usize,
            out_err: *mut *mut ::std::ffi::c_char,
        ) -> ::std::ffi::c_int {
            let result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                if buffer.is_null() {
                    return ::std::result::Result::Err(
                        ::std::string::String::from(
                            "null buffer in list_codecs",
                        ),
                    );
                }
                // SAFETY: `buffer` is `*mut Arc<dyn Buffer>`.
                let arc = unsafe {
                    &*(buffer as *const ::std::sync::Arc<
                        dyn ::reovim_subsys_buffer::Buffer,
                    >)
                };
                ::std::result::Result::Ok(arc.list_codecs())
            }));
            match result {
                ::std::result::Result::Ok(::std::result::Result::Ok(pairs)) => {
                    let count = pairs.len();
                    let mut ids: ::std::vec::Vec<u32> =
                        ::std::vec::Vec::with_capacity(count);
                    let mut names: ::std::vec::Vec<
                        ::reovim_subsys_buffer::abi::FfiCodecName,
                    > = ::std::vec::Vec::with_capacity(count);
                    for (cid, name) in pairs {
                        ids.push(cid.0.get());
                        let cs = match ::std::ffi::CString::new(name) {
                            ::std::result::Result::Ok(cs) => cs,
                            ::std::result::Result::Err(_) => ::std::ffi::CString::new(
                                "codec name (interior nul)",
                            ).unwrap(),
                        };
                        let len = cs.as_bytes().len();
                        names.push(::reovim_subsys_buffer::abi::FfiCodecName {
                            ptr: cs.into_raw(),
                            len,
                        });
                    }
                    // Boxed-slice round-trip so the allocator layout
                    // matches `destroy_codec_name_array`.
                    let ids_boxed: ::std::boxed::Box<[u32]> =
                        ids.into_boxed_slice();
                    let names_boxed: ::std::boxed::Box<
                        [::reovim_subsys_buffer::abi::FfiCodecName],
                    > = names.into_boxed_slice();
                    let ids_ptr =
                        ::std::boxed::Box::into_raw(ids_boxed).cast::<u32>();
                    let names_ptr = ::std::boxed::Box::into_raw(names_boxed)
                        .cast::<::reovim_subsys_buffer::abi::FfiCodecName>();
                    // SAFETY: out_* are host-provided writable; host
                    // frees both arrays via `destroy_codec_name_array`.
                    unsafe {
                        *out_ids = ids_ptr;
                        *out_names = names_ptr;
                        *out_count = count;
                    }
                    0
                }
                ::std::result::Result::Ok(::std::result::Result::Err(e)) => {
                    __reovim_buffer_write_err(out_err, e);
                    -1
                }
                ::std::result::Result::Err(_) => -2,
            }
        }

        // ────────────────────────────────────────────────────────────
        // Subscription trampolines (forward-compat stubs).
        //
        // `BufferSubscribable` is not yet wired through the macro;
        // there are no async consumers in the current architecture.
        // The slot exists in the vtable for forward-compat; the
        // stubs immediately invoke `subscriber_destroy` so the host
        // does not leak the handle.
        // ────────────────────────────────────────────────────────────

        #[doc(hidden)]
        unsafe extern "C" fn __reovim_buffer_buffer_subscribe_edits(
            _buffer: *mut ::std::ffi::c_void,
            subscriber_handle: *mut ::std::ffi::c_void,
            _subscriber_cb: unsafe extern "C" fn(
                handle: *mut ::std::ffi::c_void,
                edit: *const ::reovim_subsys_buffer::abi::FfiByteEdit,
            ) -> ::std::ffi::c_int,
            subscriber_destroy: unsafe extern "C" fn(handle: *mut ::std::ffi::c_void),
            out_subscription_id: *mut u32,
            _out_err: *mut *mut ::std::ffi::c_char,
        ) -> ::std::ffi::c_int {
            let _ = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                if !subscriber_handle.is_null() {
                    // SAFETY: host contract: `subscriber_destroy`
                    // accepts the matching `subscriber_handle`.
                    unsafe {
                        subscriber_destroy(subscriber_handle);
                    }
                }
                if !out_subscription_id.is_null() {
                    // SAFETY: out_subscription_id host-writable.
                    unsafe {
                        *out_subscription_id = 0;
                    }
                }
            }));
            0
        }

        #[doc(hidden)]
        unsafe extern "C" fn __reovim_buffer_buffer_unsubscribe_edits(
            _buffer: *mut ::std::ffi::c_void,
            _subscription_id: u32,
            _out_err: *mut *mut ::std::ffi::c_char,
        ) -> ::std::ffi::c_int {
            0
        }

        // ────────────────────────────────────────────────────────────
        // Buffer destruction.
        // ────────────────────────────────────────────────────────────

        #[doc(hidden)]
        unsafe extern "C" fn __reovim_buffer_buffer_destroy(
            buffer: *mut ::std::ffi::c_void,
        ) {
            if buffer.is_null() {
                return;
            }
            let _ = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                // Sweep the side table for any attachments still
                // recorded against this buffer pointer; the buffer
                // impl's own Drop will fan out destroy_handle via
                // each adapter's `Drop`, so the side-table entries
                // are the only thing we need to release.
                let key_addr = buffer as usize;
                if let ::std::result::Result::Ok(mut tbl) =
                    __reovim_buffer_attachment_table().write()
                {
                    tbl.retain(|&(addr, _slot), _record| addr != key_addr);
                }
                // SAFETY: `buffer` was produced by `Box::into_raw`
                // on `Box<Arc<dyn Buffer>>` in
                // `__reovim_buffer_box_arc_buffer`; reboxing
                // and dropping releases the Arc refcount.
                unsafe {
                    ::std::mem::drop(::std::boxed::Box::from_raw(
                        buffer as *mut ::std::sync::Arc<
                            dyn ::reovim_subsys_buffer::Buffer,
                        >,
                    ));
                }
            }));
        }

        // ────────────────────────────────────────────────────────────
        // Driver destruction + allocator-hygiene slots.
        // ────────────────────────────────────────────────────────────

        #[doc(hidden)]
        unsafe extern "C" fn __reovim_buffer_destroy(
            instance: *mut ::std::ffi::c_void,
        ) {
            if instance.is_null() {
                return;
            }
            let _ = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                // SAFETY: `instance` was produced by `Box::into_raw`
                // in `construct`; reboxing and dropping returns the
                // allocation to the driver's allocator.
                unsafe {
                    ::std::mem::drop(::std::boxed::Box::from_raw(
                        instance as *mut #driver_type,
                    ));
                }
            }));
        }

        #[doc(hidden)]
        unsafe extern "C" fn __reovim_buffer_destroy_error_string(
            ptr: *mut ::std::ffi::c_char,
        ) {
            if ptr.is_null() {
                return;
            }
            // SAFETY: `ptr` was produced by `CString::into_raw` in a
            // sibling trampoline; reconsuming it here frees via the
            // driver's allocator.
            let _ = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                unsafe {
                    ::std::mem::drop(::std::ffi::CString::from_raw(ptr));
                }
            }));
        }

        #[doc(hidden)]
        unsafe extern "C" fn __reovim_buffer_destroy_byte_buffer(
            ptr: *mut u8,
            len: usize,
        ) {
            if ptr.is_null() || len == 0 {
                return;
            }
            // SAFETY: `ptr` was produced by `Box::<[u8]>::into_raw`
            // in `buffer_read_bytes`; reconstructing the boxed slice
            // with the matching length returns the allocation.
            let _ = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                unsafe {
                    let slice_ptr: *mut [u8] =
                        ::std::ptr::slice_from_raw_parts_mut(ptr, len);
                    ::std::mem::drop(::std::boxed::Box::from_raw(slice_ptr));
                }
            }));
        }

        #[doc(hidden)]
        unsafe extern "C" fn __reovim_buffer_destroy_file_path(
            ptr: *mut ::std::ffi::c_char,
            _len: usize,
        ) {
            if ptr.is_null() {
                return;
            }
            // SAFETY: `ptr` was produced by `CString::into_raw` in
            // `buffer_file_path`; reconsuming it returns the
            // allocation. `_len` is provided for symmetry with
            // `destroy_byte_buffer` but `CString` carries its own
            // length internally.
            let _ = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                unsafe {
                    ::std::mem::drop(::std::ffi::CString::from_raw(ptr));
                }
            }));
        }

        #[doc(hidden)]
        unsafe extern "C" fn __reovim_buffer_destroy_codec_name_array(
            ids_ptr: *mut u32,
            names_ptr: *mut ::reovim_subsys_buffer::abi::FfiCodecName,
            count: usize,
        ) {
            if count == 0 {
                return;
            }
            let _ = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                if !ids_ptr.is_null() {
                    // SAFETY: `ids_ptr` was produced by
                    // `Box::<[u32]>::into_raw` in
                    // `buffer_list_codecs`.
                    unsafe {
                        let slice_ptr: *mut [u32] =
                            ::std::ptr::slice_from_raw_parts_mut(ids_ptr, count);
                        ::std::mem::drop(::std::boxed::Box::from_raw(slice_ptr));
                    }
                }
                if !names_ptr.is_null() {
                    // SAFETY: `names_ptr` was produced by
                    // `Box::<[FfiCodecName]>::into_raw` in
                    // `buffer_list_codecs`; each
                    // `FfiCodecName.ptr` is a `CString::into_raw`
                    // pointer that we reclaim per-entry before
                    // dropping the outer boxed slice.
                    unsafe {
                        let slice_ptr:
                            *mut [::reovim_subsys_buffer::abi::FfiCodecName] =
                            ::std::ptr::slice_from_raw_parts_mut(names_ptr, count);
                        let boxed = ::std::boxed::Box::from_raw(slice_ptr);
                        for entry in boxed.iter() {
                            if !entry.ptr.is_null() {
                                ::std::mem::drop(
                                    ::std::ffi::CString::from_raw(entry.ptr),
                                );
                            }
                        }
                        ::std::mem::drop(boxed);
                    }
                }
            }));
        }
    };

    expanded.into()
}
