//! Codegen for `declare_client_render_driver!`.
//!
//! Expands to a `#[repr(C)]` static vtable exported under the symbol
//! `REOVIM_CLIENT_RENDER_DRIVER_VTABLE` plus the per-slot `unsafe
//! extern "C"` trampoline bodies. Every trampoline body is wrapped in
//! `catch_unwind(AssertUnwindSafe(|| ...))` so panics never unwind
//! across the C ABI.
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
        // Sub-vtable for RenderTarget handles. The host reads this
        // pointer through the driver vtable's `target` slot. Shared
        // across all instances of this driver.
        #[doc(hidden)]
        static __REOVIM_RENDER_TARGET_VTABLE:
            ::reovim_client_subsys_render::abi::RenderTargetVTable =
            ::reovim_client_subsys_render::abi::RenderTargetVTable {
                abi_version:
                    ::reovim_client_subsys_render::abi::REOVIM_CLIENT_RENDER_DRIVER_ABI_VERSION,
                size_of_self: ::std::mem::size_of::<
                    ::reovim_client_subsys_render::abi::RenderTargetVTable,
                >(),
                submit: __reovim_client_render_submit,
            };

        // Main driver vtable exported under the canonical symbol name.
        #[unsafe(no_mangle)]
        pub static REOVIM_CLIENT_RENDER_DRIVER_VTABLE:
            ::reovim_client_subsys_render::abi::ClientRenderVTable =
            ::reovim_client_subsys_render::abi::ClientRenderVTable {
                abi_version:
                    ::reovim_client_subsys_render::abi::REOVIM_CLIENT_RENDER_DRIVER_ABI_VERSION,
                api_version:
                    ::reovim_client_subsys_render::abi::REOVIM_CLIENT_RENDER_DRIVER_API_VERSION,
                size_of_self: ::std::mem::size_of::<
                    ::reovim_client_subsys_render::abi::ClientRenderVTable,
                >(),
                probe: __reovim_client_render_probe,
                construct: __reovim_client_render_construct,
                target: __reovim_client_render_target,
                shutdown: __reovim_client_render_shutdown,
                destroy: __reovim_client_render_destroy,
                destroy_error_string: __reovim_client_render_destroy_error_string,
            };

        #[doc(hidden)]
        unsafe extern "C" fn __reovim_client_render_probe()
            -> ::reovim_client_subsys_render::client_render::ClientRenderDriverProbe
        {
            // SAFETY: no unsafe operations; `probe` is a pure `const`
            // query invoked before `construct`. On panic we fall back
            // to a blank probe so the ABI signature is preserved; the
            // host treats a blank `kind` as a non-driver and skips it.
            ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                <#driver_type as ::reovim_client_subsys_render::client_render::ClientRender>
                    ::probe()
            }))
            .unwrap_or_else(|_|
                ::reovim_client_subsys_render::client_render::ClientRenderDriverProbe::new("", "")
            )
        }

        #[doc(hidden)]
        unsafe extern "C" fn __reovim_client_render_construct(
            _platform: *mut ::std::ffi::c_void,
            out_instance: *mut *mut ::std::ffi::c_void,
            out_err: *mut *mut ::std::ffi::c_char,
        ) -> ::std::ffi::c_int {
            // TODO(O7): FfiPlatformCaps repr(C) stability — Phase 2 tightens this.
            let result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                <#driver_type as ::reovim_client_subsys_render::client_render::ClientRender>
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
                    // SAFETY: writing a driver-allocated pointer into
                    // the host-provided out-param slot.
                    unsafe { *out_err = cs.into_raw(); }
                    -1
                }
                ::std::result::Result::Err(_) => -2,
            }
        }

        #[doc(hidden)]
        unsafe extern "C" fn __reovim_client_render_target(
            instance: *mut ::std::ffi::c_void,
            out_vtable: *mut *const ::reovim_client_subsys_render::abi::RenderTargetVTable,
            out_handle: *mut *mut ::std::ffi::c_void,
            _out_err: *mut *mut ::std::ffi::c_char,
        ) -> ::std::ffi::c_int {
            // The sub-handle is the driver instance itself: the
            // submit trampoline casts it back to `*mut #driver_type`
            // and dispatches `ClientRender::target(&mut *).submit(...)`.
            // We bounce through `catch_unwind` to keep the target slot
            // consistent with the other trampolines (null-instance
            // callers cannot unwind), but no user code runs here.
            let result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                if instance.is_null() {
                    ::std::result::Result::Err("null instance")
                } else {
                    ::std::result::Result::Ok(())
                }
            }));
            match result {
                ::std::result::Result::Ok(::std::result::Result::Ok(())) => {
                    // SAFETY: writing driver-owned pointers to host
                    // out-params. The sub-handle aliases the driver
                    // instance; the host's `LoadedRenderTarget<'a>`
                    // lifetime captures this borrow.
                    unsafe {
                        *out_vtable = &__REOVIM_RENDER_TARGET_VTABLE;
                        *out_handle = instance;
                    }
                    0
                }
                ::std::result::Result::Ok(::std::result::Result::Err(_)) => -1,
                ::std::result::Result::Err(_) => -2,
            }
        }

        #[doc(hidden)]
        unsafe extern "C" fn __reovim_client_render_submit(
            handle: *mut ::std::ffi::c_void,
            data: *const u8,
            len: usize,
            out_err: *mut *mut ::std::ffi::c_char,
        ) -> ::std::ffi::c_int {
            let result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                // SAFETY: `handle` is the driver-instance pointer
                // delivered by `__reovim_client_render_target`; it
                // aliases the Box<DriverType> produced by `construct`.
                // `data`/`len` describe a host-side byte slice valid
                // for the duration of this call.
                let drv = unsafe { &mut *(handle as *mut #driver_type) };
                let slice: &[u8] = if len == 0 {
                    &[]
                } else {
                    // SAFETY: caller contract; see above.
                    unsafe { ::std::slice::from_raw_parts(data, len) }
                };
                use ::reovim_client_subsys_render::target::RenderTarget;
                <#driver_type as ::reovim_client_subsys_render::client_render::ClientRender>
                    ::target(drv)
                    .submit(slice)
            }));
            match result {
                ::std::result::Result::Ok(::std::result::Result::Ok(())) => 0,
                ::std::result::Result::Ok(::std::result::Result::Err(e)) => {
                    let msg = ::std::format!("{}", e);
                    let cs = match ::std::ffi::CString::new(msg) {
                        ::std::result::Result::Ok(cs) => cs,
                        ::std::result::Result::Err(_) =>
                            ::std::ffi::CString::new("submit error").unwrap(),
                    };
                    // SAFETY: writing driver-allocated pointer to
                    // host-provided out-param.
                    unsafe { *out_err = cs.into_raw(); }
                    -1
                }
                ::std::result::Result::Err(_) => -2,
            }
        }

        #[doc(hidden)]
        unsafe extern "C" fn __reovim_client_render_shutdown(
            instance: *mut ::std::ffi::c_void,
            out_err: *mut *mut ::std::ffi::c_char,
        ) -> ::std::ffi::c_int {
            let result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                // SAFETY: `instance` is the Box<_> pointer from `construct`.
                let drv = unsafe { &mut *(instance as *mut #driver_type) };
                <#driver_type as ::reovim_client_subsys_render::client_render::ClientRender>
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
        unsafe extern "C" fn __reovim_client_render_destroy(
            instance: *mut ::std::ffi::c_void,
        ) {
            if instance.is_null() {
                return;
            }
            let _ = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                // SAFETY: `instance` was produced by `Box::into_raw`
                // in `construct`; re-boxing and dropping returns the
                // allocation to the driver's allocator.
                unsafe {
                    ::std::mem::drop(::std::boxed::Box::from_raw(instance as *mut #driver_type));
                }
            }));
        }

        #[doc(hidden)]
        unsafe extern "C" fn __reovim_client_render_destroy_error_string(
            ptr: *mut ::std::ffi::c_char,
        ) {
            if ptr.is_null() {
                return;
            }
            // SAFETY: `ptr` was produced by `CString::into_raw` in a
            // sibling trampoline above; re-consuming it here frees it
            // via the driver's allocator.
            let _ = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| unsafe {
                ::std::mem::drop(::std::ffi::CString::from_raw(ptr));
            }));
        }
    };

    expanded.into()
}
