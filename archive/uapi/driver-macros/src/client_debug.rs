//! Codegen for `declare_client_debug_driver!`.
//!
//! Expands to a `#[repr(C)]` static vtable exported under the symbol
//! `REOVIM_CLIENT_DEBUG_DRIVER_VTABLE` plus the per-slot `unsafe extern
//! "C"` trampoline bodies. Every trampoline body is wrapped in
//! `catch_unwind(AssertUnwindSafe(|| ...))` so panics never unwind
//! across the C ABI. The observer sub-vtable's `next_frame` slot is the
//! genuinely novel surface vs. the render family: a sub-handle iterator
//! pump that can produce, end, or error on every call.
//!
//! See `docs/architecture/driver-abi-v1.md` for the binary contract.

use {
    proc_macro::TokenStream,
    quote::quote,
    syn::{Ident, parse_macro_input},
};

#[allow(clippy::too_many_lines)] // FFI codegen emits many trampolines
pub fn expand(input: TokenStream) -> TokenStream {
    let driver_type: Ident = parse_macro_input!(input as Ident);

    let expanded = quote! {
        // Sub-vtable for DebugObserver sub-handles. The host reads this
        // pointer through the driver vtable's `observe_start` slot.
        // Shared across all observers of this driver.
        #[doc(hidden)]
        static __REOVIM_DEBUG_OBSERVER_VTABLE:
            ::reovim_client_subsys_debug::abi::DebugObserverVTable =
            ::reovim_client_subsys_debug::abi::DebugObserverVTable {
                abi_version:
                    ::reovim_client_subsys_debug::abi::REOVIM_CLIENT_DEBUG_DRIVER_ABI_VERSION,
                size_of_self: ::std::mem::size_of::<
                    ::reovim_client_subsys_debug::abi::DebugObserverVTable,
                >(),
                next_frame: __reovim_client_debug_observer_next_frame,
                close: __reovim_client_debug_observer_close,
            };

        // Main driver vtable exported under the canonical symbol name.
        #[unsafe(no_mangle)]
        pub static REOVIM_CLIENT_DEBUG_DRIVER_VTABLE:
            ::reovim_client_subsys_debug::abi::ClientDebugVTable =
            ::reovim_client_subsys_debug::abi::ClientDebugVTable {
                abi_version:
                    ::reovim_client_subsys_debug::abi::REOVIM_CLIENT_DEBUG_DRIVER_ABI_VERSION,
                api_version:
                    ::reovim_client_subsys_debug::abi::REOVIM_CLIENT_DEBUG_DRIVER_API_VERSION,
                size_of_self: ::std::mem::size_of::<
                    ::reovim_client_subsys_debug::abi::ClientDebugVTable,
                >(),
                probe: __reovim_client_debug_probe,
                construct: __reovim_client_debug_construct,
                observe_start: __reovim_client_debug_observe_start,
                drive: __reovim_client_debug_drive,
                shutdown: __reovim_client_debug_shutdown,
                destroy: __reovim_client_debug_destroy,
                destroy_error_string: __reovim_client_debug_destroy_error_string,
                destroy_bytes: __reovim_client_debug_destroy_bytes,
            };

        #[doc(hidden)]
        unsafe extern "C" fn __reovim_client_debug_probe()
            -> ::reovim_client_subsys_debug::client_debug::DebugProbe
        {
            ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                <#driver_type as ::reovim_client_subsys_debug::client_debug::ClientDebugSurface>
                    ::probe()
            }))
            .unwrap_or_else(|_|
                ::reovim_client_subsys_debug::client_debug::DebugProbe::new("", "", &[], &[])
            )
        }

        #[doc(hidden)]
        unsafe extern "C" fn __reovim_client_debug_construct(
            _platform: *mut ::std::ffi::c_void,
            out_instance: *mut *mut ::std::ffi::c_void,
            out_err: *mut *mut ::std::ffi::c_char,
        ) -> ::std::ffi::c_int {
            let result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                <#driver_type as ::reovim_client_subsys_debug::client_debug::ClientDebugSurface>
                    ::construct(_platform)
            }));
            match result {
                ::std::result::Result::Ok(::std::result::Result::Ok(driver)) => {
                    let boxed = ::std::boxed::Box::new(driver);
                    // SAFETY: `out_instance` is a writable pointer
                    // provided by the host loader.
                    unsafe { *out_instance = ::std::boxed::Box::into_raw(boxed).cast(); }
                    0
                }
                ::std::result::Result::Ok(::std::result::Result::Err(e)) => {
                    let msg = ::std::format!("{}", e);
                    let cs = match ::std::ffi::CString::new(msg) {
                        ::std::result::Result::Ok(cs) => cs,
                        ::std::result::Result::Err(_) =>
                            ::std::ffi::CString::new("driver error (interior nul)").unwrap(),
                    };
                    // SAFETY: writing driver-allocated pointer to host out-param.
                    unsafe { *out_err = cs.into_raw(); }
                    -1
                }
                ::std::result::Result::Err(_) => -2,
            }
        }

        #[doc(hidden)]
        unsafe extern "C" fn __reovim_client_debug_observe_start(
            instance: *mut ::std::ffi::c_void,
            selector: *const u8,
            selector_len: usize,
            out_vtable: *mut *const ::reovim_client_subsys_debug::abi::DebugObserverVTable,
            out_handle: *mut *mut ::std::ffi::c_void,
            out_err: *mut *mut ::std::ffi::c_char,
        ) -> ::std::ffi::c_int {
            let result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                // SAFETY: `instance` is the Box<_> pointer from `construct`.
                let drv = unsafe { &mut *(instance as *mut #driver_type) };
                let slice: &[u8] = if selector_len == 0 {
                    &[]
                } else {
                    // SAFETY: host provides a slice valid for this call.
                    unsafe { ::std::slice::from_raw_parts(selector, selector_len) }
                };
                <#driver_type as ::reovim_client_subsys_debug::client_debug::ClientDebugSurface>
                    ::observe(drv, slice)
            }));
            match result {
                ::std::result::Result::Ok(::std::result::Result::Ok(observer)) => {
                    // Observer is `Box<dyn DebugObserver + Send + '_>` — a
                    // fat pointer. Box-in-box gives a thin raw pointer
                    // suitable for *mut c_void. The lifetime is erased
                    // at the cast; the host's LoadedDebugObserver<'a>
                    // wrapper carries the borrow discipline.
                    let outer: ::std::boxed::Box<
                        ::std::boxed::Box<
                            dyn ::reovim_client_subsys_debug::observer::DebugObserver + Send,
                        >,
                    > = ::std::boxed::Box::new(unsafe {
                        // SAFETY: lifetime erasure from '_ (&mut driver
                        // borrow) to 'static. Soundness: the host's safe
                        // wrapper `LoadedDebugObserver<'a>` holds
                        // PhantomData<&'a mut LoadedClientDebug>, so the
                        // driver instance cannot be dropped while the
                        // observer exists. The observer is freed by the
                        // host BEFORE the instance through the
                        // observer-vtable's `close` slot.
                        ::std::mem::transmute::<
                            ::std::boxed::Box<
                                dyn ::reovim_client_subsys_debug::observer::DebugObserver + Send + '_,
                            >,
                            ::std::boxed::Box<
                                dyn ::reovim_client_subsys_debug::observer::DebugObserver + Send,
                            >,
                        >(observer)
                    });
                    let ptr = ::std::boxed::Box::into_raw(outer);
                    // SAFETY: writing driver-allocated pointers to host out-params.
                    unsafe {
                        *out_vtable = &__REOVIM_DEBUG_OBSERVER_VTABLE;
                        *out_handle = ptr.cast();
                    }
                    0
                }
                ::std::result::Result::Ok(::std::result::Result::Err(e)) => {
                    let msg = ::std::format!("{}", e);
                    let cs = match ::std::ffi::CString::new(msg) {
                        ::std::result::Result::Ok(cs) => cs,
                        ::std::result::Result::Err(_) =>
                            ::std::ffi::CString::new("observe error").unwrap(),
                    };
                    // SAFETY: writing driver-allocated CString pointer.
                    unsafe { *out_err = cs.into_raw(); }
                    -1
                }
                ::std::result::Result::Err(_) => -2,
            }
        }

        #[doc(hidden)]
        unsafe extern "C" fn __reovim_client_debug_observer_next_frame(
            handle: *mut ::std::ffi::c_void,
            out_ptr: *mut *mut u8,
            out_len: *mut usize,
            out_err: *mut *mut ::std::ffi::c_char,
        ) -> ::std::ffi::c_int {
            let result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                // SAFETY: `handle` was produced by
                // `__reovim_client_debug_observe_start` as
                // `Box<Box<dyn DebugObserver + Send>>::into_raw`.
                let obs = unsafe {
                    &mut *(handle
                        as *mut ::std::boxed::Box<
                            dyn ::reovim_client_subsys_debug::observer::DebugObserver + Send,
                        >)
                };
                <dyn ::reovim_client_subsys_debug::observer::DebugObserver + Send
                    as ::reovim_client_subsys_debug::observer::DebugObserver>
                    ::next_frame(&mut **obs)
            }));
            match result {
                ::std::result::Result::Ok(::std::result::Result::Ok(
                    ::std::option::Option::Some(mut bytes),
                )) => {
                    // Shrink to fit so the two-arg `destroy_bytes(ptr,
                    // len)` ABI can reconstruct the Vec with
                    // `capacity == len` without leaking slack.
                    bytes.shrink_to_fit();
                    let (ptr, len, _cap) = {
                        // Vec::into_raw_parts is still unstable; replicate
                        // its effect with ManuallyDrop + pointer reads.
                        let mut md = ::std::mem::ManuallyDrop::new(bytes);
                        (md.as_mut_ptr(), md.len(), md.capacity())
                    };
                    // SAFETY: writing driver-owned pointers to host out-params.
                    unsafe {
                        *out_ptr = ptr;
                        *out_len = len;
                    }
                    0
                }
                ::std::result::Result::Ok(::std::result::Result::Ok(
                    ::std::option::Option::None,
                )) => 1,
                ::std::result::Result::Ok(::std::result::Result::Err(e)) => {
                    let msg = ::std::format!("{}", e);
                    if let ::std::result::Result::Ok(cs) = ::std::ffi::CString::new(msg) {
                        // SAFETY: writing driver-allocated CString pointer.
                        unsafe { *out_err = cs.into_raw(); }
                    }
                    -1
                }
                ::std::result::Result::Err(_) => -2,
            }
        }

        #[doc(hidden)]
        unsafe extern "C" fn __reovim_client_debug_observer_close(
            handle: *mut ::std::ffi::c_void,
        ) {
            if handle.is_null() {
                return;
            }
            let _ = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                // SAFETY: `handle` was produced by `observe_start` as
                // `Box::into_raw(Box<Box<dyn DebugObserver + Send>>)`;
                // re-boxing and dropping drops the inner trait object
                // and then the outer box.
                unsafe {
                    ::std::mem::drop(::std::boxed::Box::from_raw(
                        handle
                            as *mut ::std::boxed::Box<
                                dyn ::reovim_client_subsys_debug::observer::DebugObserver + Send,
                            >,
                    ));
                }
            }));
        }

        #[doc(hidden)]
        unsafe extern "C" fn __reovim_client_debug_drive(
            instance: *mut ::std::ffi::c_void,
            command: *const u8,
            command_len: usize,
            out_ptr: *mut *mut u8,
            out_len: *mut usize,
            out_err: *mut *mut ::std::ffi::c_char,
        ) -> ::std::ffi::c_int {
            let result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                // SAFETY: `instance` is the Box<_> pointer from `construct`.
                let drv = unsafe { &mut *(instance as *mut #driver_type) };
                let slice: &[u8] = if command_len == 0 {
                    &[]
                } else {
                    // SAFETY: host provides a slice valid for this call.
                    unsafe { ::std::slice::from_raw_parts(command, command_len) }
                };
                <#driver_type as ::reovim_client_subsys_debug::client_debug::ClientDebugSurface>
                    ::drive(drv, slice)
            }));
            match result {
                ::std::result::Result::Ok(::std::result::Result::Ok(mut bytes)) => {
                    // Shrink to fit: the two-arg `destroy_bytes(ptr,
                    // len)` ABI requires `capacity == len`.
                    bytes.shrink_to_fit();
                    let (ptr, len, _cap) = {
                        let mut md = ::std::mem::ManuallyDrop::new(bytes);
                        (md.as_mut_ptr(), md.len(), md.capacity())
                    };
                    // SAFETY: writing driver-owned pointers to host out-params.
                    unsafe {
                        *out_ptr = ptr;
                        *out_len = len;
                    }
                    0
                }
                ::std::result::Result::Ok(::std::result::Result::Err(e)) => {
                    let msg = ::std::format!("{}", e);
                    let cs = match ::std::ffi::CString::new(msg) {
                        ::std::result::Result::Ok(cs) => cs,
                        ::std::result::Result::Err(_) =>
                            ::std::ffi::CString::new("drive error").unwrap(),
                    };
                    // SAFETY: writing driver-allocated CString pointer.
                    unsafe { *out_err = cs.into_raw(); }
                    -1
                }
                ::std::result::Result::Err(_) => -2,
            }
        }

        #[doc(hidden)]
        unsafe extern "C" fn __reovim_client_debug_shutdown(
            instance: *mut ::std::ffi::c_void,
            out_err: *mut *mut ::std::ffi::c_char,
        ) -> ::std::ffi::c_int {
            let result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                // SAFETY: `instance` is the Box<_> pointer from `construct`.
                let drv = unsafe { &mut *(instance as *mut #driver_type) };
                <#driver_type as ::reovim_client_subsys_debug::client_debug::ClientDebugSurface>
                    ::shutdown(drv)
            }));
            match result {
                ::std::result::Result::Ok(::std::result::Result::Ok(())) => 0,
                ::std::result::Result::Ok(::std::result::Result::Err(e)) => {
                    let msg = ::std::format!("{}", e);
                    if let ::std::result::Result::Ok(cs) = ::std::ffi::CString::new(msg) {
                        // SAFETY: writing driver-allocated CString pointer.
                        unsafe { *out_err = cs.into_raw(); }
                    }
                    -1
                }
                ::std::result::Result::Err(_) => -2,
            }
        }

        #[doc(hidden)]
        unsafe extern "C" fn __reovim_client_debug_destroy(
            instance: *mut ::std::ffi::c_void,
        ) {
            if instance.is_null() {
                return;
            }
            let _ = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                // SAFETY: `instance` was produced by `Box::into_raw` in
                // `construct`; re-boxing and dropping returns the
                // allocation to the driver's allocator.
                unsafe {
                    ::std::mem::drop(::std::boxed::Box::from_raw(instance as *mut #driver_type));
                }
            }));
        }

        #[doc(hidden)]
        unsafe extern "C" fn __reovim_client_debug_destroy_error_string(
            ptr: *mut ::std::ffi::c_char,
        ) {
            if ptr.is_null() {
                return;
            }
            // SAFETY: `ptr` was produced by `CString::into_raw` in a
            // sibling trampoline; re-consuming it here frees it via the
            // driver's allocator.
            let _ = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| unsafe {
                ::std::mem::drop(::std::ffi::CString::from_raw(ptr));
            }));
        }

        #[doc(hidden)]
        unsafe extern "C" fn __reovim_client_debug_destroy_bytes(
            ptr: *mut u8,
            len: usize,
        ) {
            if ptr.is_null() {
                return;
            }
            // SAFETY: `ptr`/`len` were produced by a sibling trampoline
            // from a `Vec<u8>` AFTER `shrink_to_fit()`, so
            // `capacity == len` at the boundary. Reconstructing with
            // `from_raw_parts(ptr, len, len)` frees the exact
            // allocation the driver handed out — this is the ABI-
            // required idiom under the two-arg `destroy_bytes(ptr, len)`
            // slot.
            #[allow(clippy::same_length_and_capacity)]
            let _ = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| unsafe {
                ::std::mem::drop(::std::vec::Vec::from_raw_parts(ptr, len, len));
            }));
        }
    };

    expanded.into()
}
