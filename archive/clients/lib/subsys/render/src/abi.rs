//! C-stable ABI types for the client-render driver.
//!
//! These sibling types are the runtime-loading counterpart to
//! [`crate::target::RenderTarget`] and [`crate::client_render::ClientRender`].
//! A cdylib driver exports a static vtable under the symbol
//! [`REOVIM_CLIENT_RENDER_DRIVER_VTABLE`] (spelled in the driver's
//! codegen, not re-exported here). The host loader reads the vtable
//! through this crate.
//!
//! Binary contract: see `docs/architecture/driver-abi-v1.md`.

use {
    crate::client_render::ClientRenderDriverProbe,
    reovim_kernel::api::v1::Version,
    std::ffi::{c_char, c_int, c_void},
};

/// ABI version epoch for the client-render driver vtable layout.
///
/// Hosts reject cdylibs whose vtable reports a different value. Any
/// field reorder, add, or remove bumps this epoch. Orthogonal to
/// [`REOVIM_CLIENT_RENDER_DRIVER_API_VERSION`], which tracks
/// backwards-compatible semantics.
pub const REOVIM_CLIENT_RENDER_DRIVER_ABI_VERSION: u32 = 1;

/// API version of the client-render driver contract.
///
/// Semver: host accepts a driver with the same major and `minor >=
/// host.minor`.
pub const REOVIM_CLIENT_RENDER_DRIVER_API_VERSION: Version = Version::new(1, 0, 0);

/// Sub-vtable returned from [`ClientRenderVTable::target`].
///
/// Each `RenderTarget` sub-handle is a `(vtable, handle)` pair.
/// The `handle` borrows from the owning driver instance; the host
/// expresses that borrow with a lifetime on the safe wrapper.
#[repr(C)]
#[derive(Debug)]
pub struct RenderTargetVTable {
    /// ABI epoch for this sub-vtable; separate from the driver's.
    pub abi_version: u32,
    /// Forward-compat size guard.
    pub size_of_self: usize,
    /// Submit encoded render commands.
    ///
    /// Returns 0 on success, `-1` on driver error with `*out_err` set,
    /// `-2` if a panic was caught at the boundary.
    pub submit: unsafe extern "C" fn(
        handle: *mut c_void,
        data: *const u8,
        len: usize,
        out_err: *mut *mut c_char,
    ) -> c_int,
}

/// Main vtable exported by a client-render driver cdylib.
#[repr(C)]
#[derive(Debug)]
pub struct ClientRenderVTable {
    /// Layout epoch. Exact-match required; see
    /// [`REOVIM_CLIENT_RENDER_DRIVER_ABI_VERSION`].
    pub abi_version: u32,
    /// API semver. Host accepts same major, minor >= required.
    pub api_version: Version,
    /// Size of this struct as observed by the driver build. Forward-
    /// compat check against `mem::size_of::<ClientRenderVTable>()` on
    /// the host.
    pub size_of_self: usize,
    /// Static probe metadata. Pure data; called without constructing
    /// the driver.
    pub probe: unsafe extern "C" fn() -> ClientRenderDriverProbe,
    /// Construct a driver instance.
    ///
    /// - `platform`: opaque platform handle (null in Phase 0; tightened
    ///   to `FfiPlatformCaps` in Phase 2 per master-plan §O7).
    /// - On success writes the instance pointer into `out_instance`
    ///   and returns 0.
    /// - On error writes a driver-allocated C-string into `out_err`
    ///   (host frees via `destroy_error_string`) and returns `-1`.
    /// - `-2` indicates a panic was caught.
    pub construct: unsafe extern "C" fn(
        platform: *mut c_void,
        out_instance: *mut *mut c_void,
        out_err: *mut *mut c_char,
    ) -> c_int,
    /// Borrow the driver's current render target.
    ///
    /// Returns a `(vtable, handle)` pair via the two out-params. The
    /// handle borrows from the instance; the host wraps it with a
    /// lifetime anchored to the driver wrapper.
    pub target: unsafe extern "C" fn(
        instance: *mut c_void,
        out_vtable: *mut *const RenderTargetVTable,
        out_handle: *mut *mut c_void,
        out_err: *mut *mut c_char,
    ) -> c_int,
    /// Drain outstanding work. Called before `destroy` in the happy
    /// path.
    pub shutdown: unsafe extern "C" fn(instance: *mut c_void, out_err: *mut *mut c_char) -> c_int,
    /// Destroy the instance and reclaim its memory.
    ///
    /// Called after `shutdown` (or immediately in a Drop-on-error
    /// path). Infallible.
    pub destroy: unsafe extern "C" fn(instance: *mut c_void),
    /// Free a C-string previously allocated by a driver trampoline.
    ///
    /// Host never calls `libc::free` directly; ownership round-trips
    /// through this slot exactly once per allocated string.
    pub destroy_error_string: unsafe extern "C" fn(ptr: *mut c_char),
}

#[cfg(test)]
#[path = "abi_tests.rs"]
mod tests;
