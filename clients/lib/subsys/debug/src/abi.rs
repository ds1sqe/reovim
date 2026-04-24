//! C-stable ABI types for the client-debug driver.
//!
//! These sibling types are the runtime-loading counterpart to
//! [`crate::client_debug::ClientDebugSurface`] and
//! [`crate::observer::DebugObserver`]. A cdylib driver exports a static
//! vtable under the symbol `REOVIM_CLIENT_DEBUG_DRIVER_VTABLE` (spelled
//! in the driver's codegen, not re-exported here). The host loader reads
//! the vtable through this crate.
//!
//! Binary contract: see `docs/architecture/driver-abi-v1.md`.

use {
    crate::client_debug::DebugProbe,
    reovim_kernel::api::v1::Version,
    std::ffi::{c_char, c_int, c_void},
};

/// ABI version epoch for the client-debug driver vtable layout.
///
/// Hosts reject cdylibs whose vtable reports a different value. Any
/// field reorder, add, or remove bumps this epoch. Orthogonal to
/// [`REOVIM_CLIENT_DEBUG_DRIVER_API_VERSION`], which tracks
/// backwards-compatible semantics.
pub const REOVIM_CLIENT_DEBUG_DRIVER_ABI_VERSION: u32 = 1;

/// API version of the client-debug driver contract.
///
/// Semver: host accepts a driver with the same major and `minor >=
/// host.minor`.
pub const REOVIM_CLIENT_DEBUG_DRIVER_API_VERSION: Version = Version::new(1, 0, 0);

/// Sub-vtable returned from [`ClientDebugVTable::observe_start`].
///
/// Each `DebugObserver` sub-handle is a `(vtable, handle)` pair. The
/// `handle` borrows from the owning driver instance; the host expresses
/// that borrow with a lifetime on the safe wrapper.
#[repr(C)]
#[derive(Debug)]
pub struct DebugObserverVTable {
    /// ABI epoch for this sub-vtable; separate from the driver's.
    pub abi_version: u32,
    /// Forward-compat size guard.
    pub size_of_self: usize,
    /// Pump the next frame.
    ///
    /// - `0` with `*out_ptr`/`*out_len` populated: frame produced; host
    ///   owns the bytes until it frees them through the driver's
    ///   `destroy_bytes` slot.
    /// - `1`: end of stream. `*out_ptr`/`*out_len` untouched.
    /// - `-1`: driver error; `*out_err` populated (host frees via
    ///   `destroy_error_string`).
    /// - `-2`: a panic was caught at the trampoline boundary.
    pub next_frame: unsafe extern "C" fn(
        handle: *mut c_void,
        out_ptr: *mut *mut u8,
        out_len: *mut usize,
        out_err: *mut *mut c_char,
    ) -> c_int,
    /// Close the observer and reclaim its handle. Infallible.
    pub close: unsafe extern "C" fn(handle: *mut c_void),
}

/// Main vtable exported by a client-debug driver cdylib.
#[repr(C)]
#[derive(Debug)]
pub struct ClientDebugVTable {
    /// Layout epoch. Exact-match required; see
    /// [`REOVIM_CLIENT_DEBUG_DRIVER_ABI_VERSION`].
    pub abi_version: u32,
    /// API semver. Host accepts same major, minor >= required.
    pub api_version: Version,
    /// Size of this struct as observed by the driver build. Forward-
    /// compat check against `mem::size_of::<ClientDebugVTable>()` on the
    /// host.
    pub size_of_self: usize,
    /// Static probe metadata. Pure data; called without constructing the
    /// driver. Used by `reovim cli debug probe` to enumerate schemas.
    pub probe: unsafe extern "C" fn() -> DebugProbe,
    /// Construct a driver instance.
    ///
    /// - `platform`: opaque platform handle (null in Phase 0; tightened
    ///   to `FfiPlatformCaps` in Phase 2 per master-plan §O7).
    /// - On success writes the instance pointer into `out_instance` and
    ///   returns 0.
    /// - On error writes a driver-allocated C-string into `out_err`
    ///   (host frees via `destroy_error_string`) and returns `-1`.
    /// - `-2` indicates a panic was caught.
    pub construct: unsafe extern "C" fn(
        platform: *mut c_void,
        out_instance: *mut *mut c_void,
        out_err: *mut *mut c_char,
    ) -> c_int,
    /// Open an observer for the given selector.
    ///
    /// On success writes a `(*const DebugObserverVTable, *mut c_void)`
    /// pair through the two out-params. The handle borrows from the
    /// instance; the host wraps it with a lifetime anchored to the
    /// driver wrapper.
    pub observe_start: unsafe extern "C" fn(
        instance: *mut c_void,
        selector: *const u8,
        selector_len: usize,
        out_vtable: *mut *const DebugObserverVTable,
        out_handle: *mut *mut c_void,
        out_err: *mut *mut c_char,
    ) -> c_int,
    /// One-shot drive command.
    ///
    /// Opaque bytes in, opaque bytes out. On success writes
    /// driver-allocated response bytes to `*out_ptr`/`*out_len`; host
    /// frees via `destroy_bytes`.
    pub drive: unsafe extern "C" fn(
        instance: *mut c_void,
        command: *const u8,
        command_len: usize,
        out_ptr: *mut *mut u8,
        out_len: *mut usize,
        out_err: *mut *mut c_char,
    ) -> c_int,
    /// Drain outstanding work. Called before `destroy` in the happy
    /// path.
    pub shutdown: unsafe extern "C" fn(instance: *mut c_void, out_err: *mut *mut c_char) -> c_int,
    /// Destroy the instance and reclaim its memory.
    ///
    /// Called after `shutdown` (or immediately in a Drop-on-error path).
    /// Infallible.
    pub destroy: unsafe extern "C" fn(instance: *mut c_void),
    /// Free a C-string previously allocated by a driver trampoline.
    ///
    /// Host never calls `libc::free` directly; ownership round-trips
    /// through this slot exactly once per allocated string.
    pub destroy_error_string: unsafe extern "C" fn(ptr: *mut c_char),
    /// Free a byte buffer previously allocated by a driver trampoline
    /// (observer frame body or drive response).
    ///
    /// The ABI carries `(ptr, len)` only: driver trampolines MUST call
    /// `vec.shrink_to_fit()` before `Vec::into_raw_parts()` so the
    /// host's reconstruction with `from_raw_parts(ptr, len, len)` frees
    /// the full allocation without leaking slack capacity.
    pub destroy_bytes: unsafe extern "C" fn(ptr: *mut u8, len: usize),
}

#[cfg(test)]
#[path = "abi_tests.rs"]
mod tests;
