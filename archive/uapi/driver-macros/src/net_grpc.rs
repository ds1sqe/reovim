//! Codegen for `declare_net_grpc_driver!`.
//!
//! Expands to a `#[repr(C)]` static vtable exported under the symbol
//! `REOVIM_NET_GRPC_DRIVER_VTABLE`, plus the per-slot
//! `unsafe extern "C"` trampoline bodies and the 12 typed
//! `tonic::server::NamedService` proxy wrappers (one per stable
//! `reovim.v3.<X>Service` proto identifier; see SP02 Phase 1 §C).
//!
//! Every trampoline body is wrapped in
//! `catch_unwind(AssertUnwindSafe(|| ...))` so panics never unwind
//! across the C ABI.
//!
//! See `docs/architecture/driver-abi-v1.md` §13.5 for the binary contract.
//!
//! # Required driver type contract
//!
//! The user driver type passed to `declare_net_grpc_driver!(MyDriver)`
//! must:
//!
//! 1. Implement `::reovim_subsys_net::GrpcServerDriver` (the async
//!    serve trait).
//! 2. Provide an inherent `pub fn probe() -> NetGrpcDriverProbe`.
//! 3. Provide an inherent `pub fn construct() -> Result<Self,
//!    NetError>` that initialises any internal tokio runtime.
//! 4. Provide an inherent `pub fn runtime_handle(&self) ->
//!    tokio::runtime::Handle` returning a handle to the driver-owned
//!    runtime (per SP02 Phase 1 §D — driver creates its own runtime
//!    to avoid cross-cdylib TLS coupling).
//!
//! These are inherent methods, not a separate trait, so the driver
//! crate can implement them in idiomatic Rust without dragging
//! a lifecycle trait into subsys-net.

use {
    proc_macro::TokenStream,
    quote::quote,
    syn::{Ident, parse_macro_input},
};

/// The 12 stable proto service identifiers (`reovim.v3.<X>Service`).
///
/// Each entry produces one typed proxy wrapper in the macro output;
/// each wrapper impls `tonic::server::NamedService` with the proto
/// name as `const NAME` (required by `tonic::transport::Server::add_service`
/// per SP02 Phase 1 §C resolution).
///
/// Adding a new gRPC service requires both:
/// 1. Adding the proto to `uapi/protocol/`.
/// 2. Appending the proto identifier to this constant.
const SERVICE_NAMES: &[&str] = &[
    "reovim.v3.BufferService",
    "reovim.v3.EditorService",
    "reovim.v3.InputService",
    "reovim.v3.ModuleService",
    "reovim.v3.StateService",
    "reovim.v3.ServerService",
    "reovim.v3.NotificationService",
    "reovim.v3.PresenceService",
    "reovim.v3.ExtensionService",
    "reovim.v3.CommandService",
    "reovim.v3.DebugService",
    "reovim.v3.ClientDebugService",
];

#[allow(clippy::too_many_lines)] // FFI codegen emits many trampolines + 12 wrappers
pub fn expand(input: TokenStream) -> TokenStream {
    let driver_type: Ident = parse_macro_input!(input as Ident);

    let proxy_wrappers: ::std::vec::Vec<proc_macro2::TokenStream> = SERVICE_NAMES
        .iter()
        .map(|name| {
            let wrapper_ident = wrapper_ident_for(name);
            quote! {
                #[doc(hidden)]
                #[derive(Clone)]
                pub struct #wrapper_ident {
                    pub handle: *mut ::std::ffi::c_void,
                    pub dispatch: unsafe extern "C" fn(
                        handle: *mut ::std::ffi::c_void,
                        req_bytes: *const u8,
                        req_len: usize,
                        ctx: *mut ::std::ffi::c_void,
                        complete_cb: unsafe extern "C" fn(
                            ctx: *mut ::std::ffi::c_void,
                            status: ::std::ffi::c_int,
                            resp_bytes: *const u8,
                            resp_len: usize,
                        ),
                    ) -> ::std::ffi::c_int,
                }

                // SAFETY: the inner handle is a host-allocated
                // BoxCloneService protected by the host's Send + Sync
                // bounds; dispatch is a static function pointer.
                #[allow(unsafe_code)]
                unsafe impl ::std::marker::Send for #wrapper_ident {}
                #[allow(unsafe_code)]
                unsafe impl ::std::marker::Sync for #wrapper_ident {}

                impl ::tonic::server::NamedService for #wrapper_ident {
                    const NAME: &'static str = #name;
                }
            }
        })
        .collect();

    let expanded = quote! {
        // Main driver vtable exported under the canonical symbol name.
        #[unsafe(no_mangle)]
        pub static REOVIM_NET_GRPC_DRIVER_VTABLE:
            ::reovim_subsys_net::abi::NetGrpcVTable =
            ::reovim_subsys_net::abi::NetGrpcVTable {
                abi_version:
                    ::reovim_subsys_net::abi::REOVIM_NET_GRPC_DRIVER_ABI_VERSION,
                api_version:
                    ::reovim_subsys_net::abi::REOVIM_NET_GRPC_DRIVER_API_VERSION,
                size_of_self: ::std::mem::size_of::<
                    ::reovim_subsys_net::abi::NetGrpcVTable,
                >(),
                probe: __reovim_net_grpc_probe,
                construct: __reovim_net_grpc_construct,
                serve: __reovim_net_grpc_serve,
                shutdown: __reovim_net_grpc_shutdown,
                destroy: __reovim_net_grpc_destroy,
                destroy_error_string: __reovim_net_grpc_destroy_error_string,
            };

        #[doc(hidden)]
        unsafe extern "C" fn __reovim_net_grpc_probe()
            -> ::reovim_subsys_net::abi::NetGrpcDriverProbe
        {
            ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                <#driver_type>::probe()
            }))
            .unwrap_or_else(|_|
                ::reovim_subsys_net::abi::NetGrpcDriverProbe::new("", "")
            )
        }

        #[doc(hidden)]
        unsafe extern "C" fn __reovim_net_grpc_construct(
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
                    let msg = ::std::format!("{}", e);
                    let cs = match ::std::ffi::CString::new(msg) {
                        ::std::result::Result::Ok(cs) => cs,
                        ::std::result::Result::Err(_) =>
                            ::std::ffi::CString::new("driver error (interior nul)").unwrap(),
                    };
                    // SAFETY: writing driver-allocated pointer.
                    unsafe { *out_err = cs.into_raw(); }
                    -1
                }
                ::std::result::Result::Err(_) => -2,
            }
        }

        #[doc(hidden)]
        #[allow(clippy::too_many_lines)] // serve trampoline marshalling
        unsafe extern "C" fn __reovim_net_grpc_serve(
            instance: *mut ::std::ffi::c_void,
            config: *const ::reovim_subsys_net::abi::FfiTransportConfig,
            descriptors_ptr: *const ::reovim_subsys_net::abi::FfiServiceDescriptor,
            descriptors_len: usize,
            shutdown_fd: ::reovim_subsys_net::abi::ShutdownFd,
            bind_ready_fd: ::reovim_subsys_net::abi::ShutdownFd,
            port_writeback: *const ::std::sync::atomic::AtomicU16,
            out_err: *mut *mut ::std::ffi::c_char,
        ) -> ::std::ffi::c_int {
            let result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                if instance.is_null() || config.is_null() || port_writeback.is_null() {
                    return ::std::result::Result::Err(
                        ::reovim_subsys_net::NetError::Io(
                            "null pointer in serve trampoline".into(),
                        ),
                    );
                }
                // SAFETY: instance came from Box::into_raw in construct;
                // we borrow rather than consume so destroy can free it.
                let driver = unsafe { &*(instance as *const #driver_type) };
                // SAFETY: config is a borrowed view from the host stack
                // valid for the duration of this call.
                let cfg_ffi = unsafe { &*config };
                let cfg = ffi_to_transport_config(cfg_ffi);
                let descs: &[::reovim_subsys_net::abi::FfiServiceDescriptor] =
                    if descriptors_len == 0 {
                        &[]
                    } else {
                        // SAFETY: host-supplied slice; len is authoritative.
                        unsafe {
                            ::std::slice::from_raw_parts(descriptors_ptr, descriptors_len)
                        }
                    };
                let typed = build_typed_descriptors(descs);
                // SAFETY: port_writeback is host-allocated via Box::leak
                // (per SP02 Phase 1 fd-arch round-1 #2); pinned for
                // process lifetime.
                let writeback: &'static ::std::sync::atomic::AtomicU16 =
                    unsafe { &*(port_writeback as *const _) };

                // Driver creates and owns its own tokio runtime (per
                // SP02 Phase 1 §D); block_on its serve future from a
                // host-owned worker thread (the host invokes this
                // trampoline via spawn_blocking).
                use ::reovim_subsys_net::GrpcServerDriver as _;
                let handle = driver.runtime_handle();
                handle.block_on(driver.serve(
                    cfg, typed, shutdown_fd, bind_ready_fd, writeback,
                ))
            }));
            match result {
                ::std::result::Result::Ok(::std::result::Result::Ok(())) => 0,
                ::std::result::Result::Ok(::std::result::Result::Err(e)) => {
                    let msg = ::std::format!("{}", e);
                    let cs = match ::std::ffi::CString::new(msg) {
                        ::std::result::Result::Ok(cs) => cs,
                        ::std::result::Result::Err(_) =>
                            ::std::ffi::CString::new("serve error").unwrap(),
                    };
                    // SAFETY: writing driver-allocated pointer.
                    unsafe { *out_err = cs.into_raw(); }
                    -1
                }
                ::std::result::Result::Err(_) => -2,
            }
        }

        #[doc(hidden)]
        unsafe extern "C" fn __reovim_net_grpc_shutdown(
            instance: *mut ::std::ffi::c_void,
            _out_err: *mut *mut ::std::ffi::c_char,
        ) -> ::std::ffi::c_int {
            // The production shutdown path is shutdown_fd readable.
            // This slot is symmetry-preserving: if the host wants a
            // synchronous shutdown poke, it can call this. Default
            // behaviour: no-op (the serve loop owns its own shutdown
            // future driven by the fd).
            let _ = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                let _ = instance; // unused; reserved for future use
            }));
            0
        }

        #[doc(hidden)]
        unsafe extern "C" fn __reovim_net_grpc_destroy(
            instance: *mut ::std::ffi::c_void,
        ) {
            if instance.is_null() {
                return;
            }
            let _ = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                // SAFETY: instance was produced by Box::into_raw in
                // construct; re-boxing and dropping returns the
                // allocation to the driver's allocator.
                unsafe {
                    ::std::mem::drop(
                        ::std::boxed::Box::from_raw(instance as *mut #driver_type),
                    );
                }
            }));
        }

        #[doc(hidden)]
        unsafe extern "C" fn __reovim_net_grpc_destroy_error_string(
            ptr: *mut ::std::ffi::c_char,
        ) {
            if ptr.is_null() {
                return;
            }
            // SAFETY: ptr was produced by CString::into_raw in a sibling
            // trampoline; re-consuming it here frees via driver allocator.
            let _ = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| unsafe {
                ::std::mem::drop(::std::ffi::CString::from_raw(ptr));
            }));
        }

        // ────────────────────────────────────────────────────────────
        // Helpers + 12 typed proxy wrappers (Phase 1 §C).
        // ────────────────────────────────────────────────────────────

        #[doc(hidden)]
        fn ffi_to_transport_config(
            ffi: &::reovim_subsys_net::abi::FfiTransportConfig,
        ) -> ::reovim_subsys_net::TransportConfig {
            let enable_grpc_web = ffi.enable_grpc_web != 0;
            match ffi.kind {
                0 => {
                    // Tcp
                    let host_bytes: &[u8] = if ffi.host_len == 0 {
                        &[]
                    } else {
                        // SAFETY: host_ptr is host-pinned for the call.
                        unsafe {
                            ::std::slice::from_raw_parts(
                                ffi.host_ptr.cast::<u8>(),
                                ffi.host_len,
                            )
                        }
                    };
                    let host = ::std::string::String::from_utf8_lossy(host_bytes).into_owned();
                    ::reovim_subsys_net::TransportConfig::tcp(host, ffi.port)
                        .with_grpc_web(enable_grpc_web)
                }
                1 => {
                    // UnixSocket
                    let path_bytes: &[u8] = if ffi.path_len == 0 {
                        &[]
                    } else {
                        // SAFETY: path_ptr is host-pinned for the call.
                        unsafe {
                            ::std::slice::from_raw_parts(
                                ffi.path_ptr.cast::<u8>(),
                                ffi.path_len,
                            )
                        }
                    };
                    let path = ::std::path::PathBuf::from(
                        ::std::string::String::from_utf8_lossy(path_bytes).into_owned(),
                    );
                    ::reovim_subsys_net::TransportConfig::unix_socket(path)
                        .with_grpc_web(enable_grpc_web)
                }
                _ => ::reovim_subsys_net::TransportConfig::Stdio,
            }
        }

        #[doc(hidden)]
        fn build_typed_descriptors(
            descs: &[::reovim_subsys_net::abi::FfiServiceDescriptor],
        ) -> ::std::vec::Vec<::reovim_subsys_net::ServiceDescriptor> {
            // The host-side build_service_descriptors emits in canonical
            // order (matches SERVICE_NAMES below). The driver-side
            // typed wrappers carry NamedService::NAME compiled in.
            //
            // For the Phase 4 driver implementation, this helper is a
            // placeholder: the actual descriptor → typed-wrapper
            // translation happens in the driver's serve body, which
            // builds the tonic Router from the FFI descriptors using
            // the typed wrappers above. This empty Vec is here to
            // satisfy the trampoline signature; the driver never
            // calls GrpcServerDriver::serve through this path.
            //
            // TODO(SP02 Phase 4): the production driver overrides this
            // by NOT going through GrpcServerDriver::serve from the
            // trampoline — the trampoline calls a driver-internal
            // method that takes &[FfiServiceDescriptor] directly. The
            // GrpcServerDriver trait's typed-Vec interface is for
            // in-process callers (server_tests, integration tests).
            let _ = descs;
            ::std::vec::Vec::new()
        }

        // 12 typed proxy wrappers, one per stable proto service name.
        // Each impls tonic::server::NamedService with NAME = the proto
        // identifier. tonic::transport::Server::add_service requires
        // const NAME at the type level; per-instance runtime names
        // cannot satisfy the bound (Phase 1 §C resolution).
        //
        // The tower::Service impl is hand-written in the driver
        // crate's driver.rs because it depends on tonic body types
        // not stably exposed from subsys-net.
        #(#proxy_wrappers)*
    };

    expanded.into()
}

/// Build a wrapper struct identifier from a proto service name.
///
/// `"reovim.v3.BufferService"` → `__ReovimBufferServiceProxy`.
fn wrapper_ident_for(name: &str) -> Ident {
    let last = name.rsplit('.').next().unwrap_or(name);
    let camel = format!("__Reovim{last}Proxy");
    Ident::new(&camel, proc_macro2::Span::call_site())
}
