//! Procedural macros for reovim dynamic driver development.
//!
//! Sibling crate to `uapi/module-macros/`. Generates FFI-safe entry
//! points for runtime-loaded driver cdylibs. Each driver trait has its
//! own `declare_*_driver!` macro whose output is a `#[repr(C)]` vtable
//! exported under the convention
//! `REOVIM_<KIND>_DRIVER_VTABLE: <Kind>DriverVTable`.
//!
//! Macro codegen emits fully-qualified paths into the consumer's crate
//! (e.g. `::reovim_client_subsys_render::abi::ClientRenderVTable`); the
//! macro crate carries no compile-time dependency on any subsys crate.

mod buffer;
mod client_debug;
mod client_render;
mod net_grpc;

use proc_macro::TokenStream;

/// Generates FFI entry points for a client render driver cdylib.
///
/// The user type must implement
/// `::reovim_client_subsys_render::client_render::ClientRender`. The
/// macro emits a static `#[repr(C)]` vtable under the symbol
/// `REOVIM_CLIENT_RENDER_DRIVER_VTABLE`, plus auto-generated
/// `unsafe extern "C"` trampolines with `catch_unwind` panic isolation.
///
/// See `docs/architecture/driver-abi-v1.md` for the binary contract.
#[proc_macro]
pub fn declare_client_render_driver(input: TokenStream) -> TokenStream {
    client_render::expand(input)
}

/// Generates FFI entry points for a client debug-surface driver cdylib.
///
/// The user type must implement
/// `::reovim_client_subsys_debug::client_debug::ClientDebugSurface`. The
/// macro emits a static `#[repr(C)]` vtable under the symbol
/// `REOVIM_CLIENT_DEBUG_DRIVER_VTABLE`, plus auto-generated
/// `unsafe extern "C"` trampolines (including a `DebugObserver`
/// sub-vtable whose `next_frame` slot is a panic-isolated iterator
/// pump) with `catch_unwind` panic isolation.
///
/// See `docs/architecture/driver-abi-v1.md` for the binary contract.
#[proc_macro]
pub fn declare_client_debug_driver(input: TokenStream) -> TokenStream {
    client_debug::expand(input)
}

/// Generates FFI entry points for a net-grpc server driver cdylib.
///
/// The user driver type must implement
/// `::reovim_subsys_net::GrpcServerDriver` (the async serve trait)
/// plus three inherent methods documented in
/// `uapi/driver-macros/src/net_grpc.rs`: `probe()`, `construct()`,
/// and `runtime_handle()`. The macro emits the canonical vtable
/// under `REOVIM_NET_GRPC_DRIVER_VTABLE` plus 12 typed
/// `tonic::server::NamedService` proxy wrappers (one per stable
/// `reovim.v3.<X>Service`) with `catch_unwind` panic isolation on
/// every trampoline.
///
/// See `docs/architecture/driver-abi-v1.md` §13.5 for the binary
/// contract.
#[proc_macro]
pub fn declare_net_grpc_driver(input: TokenStream) -> TokenStream {
    net_grpc::expand(input)
}

/// Generates FFI entry points for a buffer server driver cdylib.
///
/// The user driver type must implement
/// `::reovim_subsys_buffer::BufferDriver` (the factory trait) plus
/// two inherent methods documented in
/// `uapi/driver-macros/src/buffer.rs`: `probe()` and `construct()`.
/// The macro emits the canonical vtable under
/// `REOVIM_BUFFER_DRIVER_VTABLE` (27 fn-pointer slots covering
/// driver lifecycle, the `BufferDriver` factory surface, the
/// `Buffer` per-instance surface, the multi-attachment codec slot
/// table, the async subscription bridge, and allocator-hygiene
/// destructors) with `catch_unwind` panic isolation on every
/// trampoline.
///
/// See `docs/architecture/driver-abi-v1.md` for the binary contract.
#[proc_macro]
pub fn declare_buffer_driver(input: TokenStream) -> TokenStream {
    buffer::expand(input)
}
