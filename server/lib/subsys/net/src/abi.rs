//! C-stable ABI types for the net-grpc server driver.
//!
//! These types are the runtime-loading counterpart to the
//! [`crate::driver::GrpcServerDriver`] trait. A cdylib driver exports
//! a static vtable under the symbol `REOVIM_NET_GRPC_DRIVER_VTABLE`
//! (spelled in the driver's codegen, not re-exported here). The host
//! loader (`server/lib/subsys/driver-loader/`) reads the vtable
//! through this module.
//!
//! Binary contract: `docs/architecture/driver-abi-v1.md` §13.5.

use {
    reovim_kernel::api::v1::Version,
    std::{
        ffi::{c_char, c_int, c_void},
        os::fd::RawFd,
    },
};

/// ABI version epoch for the net-grpc driver vtable layout.
///
/// Hosts reject cdylibs whose vtable reports a different value. Any
/// field reorder, add, or remove bumps this epoch. Orthogonal to
/// [`REOVIM_NET_GRPC_DRIVER_API_VERSION`], which tracks
/// backwards-compatible semantics.
pub const REOVIM_NET_GRPC_DRIVER_ABI_VERSION: u32 = 1;

/// API version of the net-grpc driver contract.
///
/// Semver: host accepts a driver with the same major and `minor >=
/// host.minor`.
pub const REOVIM_NET_GRPC_DRIVER_API_VERSION: Version = Version::new(1, 0, 0);

/// Static probe metadata returned by the driver before construction.
///
/// `kind` and `name` are zero-padded UTF-8 byte arrays. The host
/// treats a zero-byte `kind` as "non-driver, skip".
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NetGrpcDriverProbe {
    /// Driver kind: `b"net_grpc\0...\0"` (must equal `"net_grpc"` for
    /// loader acceptance).
    pub kind: [u8; 64],
    /// Human-readable driver name (e.g. `b"reovim-driver-net-grpc\0...\0"`).
    pub name: [u8; 128],
}

impl NetGrpcDriverProbe {
    /// Build a probe value from `kind` and `name` strings.
    ///
    /// Both strings are truncated if they exceed the array capacity.
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

/// FFI image of [`crate::transport::TransportConfig`].
///
/// `kind` discriminates the transport variant; the per-variant fields
/// are populated only for the matching variant. String fields are
/// host-pinned for the duration of the `serve` call.
#[repr(C)]
#[derive(Debug)]
pub struct FfiTransportConfig {
    /// Transport discriminant: 0=Tcp, 1=UnixSocket, 2=Stdio (Stdio is
    /// rejected by net-grpc).
    pub kind: u8,
    /// TCP host (e.g. `b"127.0.0.1"`); null-terminated NOT required —
    /// `host_len` is authoritative. Populated only when `kind == 0`.
    pub host_ptr: *const c_char,
    /// Strict byte length of the host string, no trailing nul.
    pub host_len: usize,
    /// TCP port. Populated only when `kind == 0`.
    pub port: u16,
    /// `UnixSocket` path bytes. Populated only when `kind == 1`.
    pub path_ptr: *const c_char,
    /// Strict byte length of the path string, no trailing nul.
    pub path_len: usize,
    /// gRPC-Web layering: 0 = false, 1 = true. (Per fd-arch round-1
    /// #1: u8 chosen over `bool` for cross-language ABI portability.)
    pub enable_grpc_web: u8,
}

/// FFI image of an entry in [`crate::driver::ServiceDescriptor`]'s
/// host-supplied list.
///
/// Each descriptor identifies one gRPC service the driver should
/// route. `handle` is a host-allocated pointer to the
/// `tower::util::BoxCloneService` instance; `dispatch_call` is the
/// host-side function pointer that drives the boxed service when the
/// driver receives a request for the matching service path.
#[repr(C)]
pub struct FfiServiceDescriptor {
    /// Proto service name (e.g. `b"reovim.v3.BufferService"`); used
    /// only for diagnostics. The driver-side typed wrappers carry the
    /// canonical names compiled in (see SP02 Phase 1 §C).
    pub service_name_ptr: *const c_char,
    /// Strict byte length of the service-name string, no trailing nul.
    pub service_name_len: usize,
    /// Opaque host-owned handle to a `BoxCloneService<Request<BoxBody>,
    /// Response<BoxBody>, Infallible>`. The driver passes this back
    /// verbatim into `dispatch_call`.
    pub handle: *mut c_void,
    /// Dispatch a request through the host-owned service. Driver-side
    /// proxy services call this when they receive a request.
    ///
    /// The `complete_cb` is invoked once per call with the encoded
    /// response bytes (or an error status). The driver's proxy
    /// future wraps `complete_cb` into a `oneshot::Sender`.
    pub dispatch_call: unsafe extern "C" fn(
        handle: *mut c_void,
        req_bytes: *const u8,
        req_len: usize,
        ctx: *mut c_void,
        complete_cb: unsafe extern "C" fn(
            ctx: *mut c_void,
            status: c_int,
            resp_bytes: *const u8,
            resp_len: usize,
        ),
    ) -> c_int,
    /// Free the host-owned handle. Called by the driver when the
    /// proxy service is dropped (after `serve` returns).
    pub destroy_handle: unsafe extern "C" fn(handle: *mut c_void),
}

/// File-descriptor-shaped shutdown signal.
///
/// The host writes a single byte to the writable end of an
/// `eventfd(2)` (Linux) or `pipe2(2)` (macOS, Windows uses an
/// equivalent). The driver wraps the readable end in
/// `tokio::io::unix::AsyncFd` and treats readability as the shutdown
/// trigger.
///
/// Ownership: the host owns the fd. The driver does NOT close it on
/// drop — `mem::forget` semantics, or `BorrowedFd` initially.
#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct ShutdownFd(pub RawFd);

/// Vtable exported by a net-grpc cdylib driver under the symbol
/// `REOVIM_NET_GRPC_DRIVER_VTABLE`.
///
/// Field order is load-bearing for the binary contract; reordering
/// requires bumping [`REOVIM_NET_GRPC_DRIVER_ABI_VERSION`]. The host
/// validates `abi_version`, `api_version`, and `size_of_self` before
/// invoking any other slot.
#[repr(C)]
pub struct NetGrpcVTable {
    /// ABI epoch — must equal [`REOVIM_NET_GRPC_DRIVER_ABI_VERSION`]
    /// on the host.
    pub abi_version: u32,
    /// Semver API version — host accepts same major + `minor >=
    /// host.minor`.
    pub api_version: Version,
    /// Size of this struct as observed by the driver build. Host
    /// rejects mismatches (forward-compat guard).
    pub size_of_self: usize,
    /// Static probe metadata. Pure data; called without constructing
    /// the driver.
    pub probe: unsafe extern "C" fn() -> NetGrpcDriverProbe,
    /// Construct a driver instance.
    ///
    /// - On success writes the instance pointer into `out_instance`
    ///   and returns 0.
    /// - On error writes a driver-allocated C-string into `out_err`
    ///   (host frees via `destroy_error_string`) and returns `-1`.
    /// - `-2` indicates a panic was caught.
    pub construct:
        unsafe extern "C" fn(out_instance: *mut *mut c_void, out_err: *mut *mut c_char) -> c_int,
    /// Serve the configured transport with the supplied descriptors
    /// until `shutdown_fd` becomes readable.
    ///
    /// Blocks the calling thread (the host invokes via
    /// `tokio::task::spawn_blocking`; the driver internally drives an
    /// owned `tokio::runtime::Runtime` per SP02 Phase 1 §D).
    ///
    /// Returns 0 on clean shutdown, `-1` on driver error with `*out_err`
    /// set, `-2` if a panic was caught.
    pub serve: unsafe extern "C" fn(
        instance: *mut c_void,
        config: *const FfiTransportConfig,
        descriptors_ptr: *const FfiServiceDescriptor,
        descriptors_len: usize,
        shutdown_fd: ShutdownFd,
        bind_ready_fd: ShutdownFd,
        port_writeback: *const std::sync::atomic::AtomicU16,
        out_err: *mut *mut c_char,
    ) -> c_int,
    /// Request graceful shutdown without consuming the instance.
    ///
    /// Provided for symmetry with other driver families. The
    /// production shutdown path is `shutdown_fd` becoming readable;
    /// this slot exists for tests that hold the instance and want a
    /// non-fd shutdown.
    ///
    /// **Must be idempotent.** The host's safe wrapper calls this slot
    /// from its `Drop` impl regardless of whether `serve` already
    /// observed `shutdown_fd` and returned cleanly. A driver whose
    /// `serve` already shut down internally MUST treat a subsequent
    /// `shutdown` call as a no-op success (return 0) rather than
    /// returning an error or panicking.
    pub shutdown: unsafe extern "C" fn(instance: *mut c_void, out_err: *mut *mut c_char) -> c_int,
    /// Drop the driver instance. Host calls this once after `serve`
    /// returns and before `Library::drop` unloads the cdylib.
    pub destroy: unsafe extern "C" fn(instance: *mut c_void),
    /// Free a `CString` allocated by a driver trampoline.
    pub destroy_error_string: unsafe extern "C" fn(ptr: *mut c_char),
}

#[cfg(test)]
#[path = "abi_tests.rs"]
mod tests;
