//! Safe wrappers over a runtime-loaded net-grpc driver cdylib.
//!
//! Mirrors `clients/lib/subsys/driver-loader/src/client_render.rs` and
//! `client_debug.rs` but targets the server-side gRPC driver family
//! ([`reovim_subsys_net::abi::NetGrpcVTable`]). Lifecycle is the same
//! shape: open → validate → construct → serve → drop (shutdown +
//! destroy).
//!
//! # Phase 5 scope (lifecycle MVP)
//!
//! This file implements the lifecycle FFI plumbing only. The
//! [`LoadedNetGrpc::serve`] entry point accepts an EMPTY descriptor
//! list and rejects non-empty lists with [`reovim_subsys_net::NetError::Io`]
//! pointing at the `SP02b` deferral; the host-allocated
//! `BoxCloneService` → [`reovim_subsys_net::abi::FfiServiceDescriptor`]
//! marshalling (HTTP body framing, gRPC envelope, dispatch trampoline,
//! handle-destructor wiring) is substantial engineering deferred to a
//! follow-on (see `tmp/deferral-draft-sp02b-dispatch-call.md`).
//!
//! Once `SP02b` lands, the empty-list guard in [`LoadedNetGrpc::serve`]
//! is replaced by a translation step that builds one
//! [`reovim_subsys_net::abi::FfiServiceDescriptor`] per host descriptor,
//! pinning the `CString` name buffers and the dispatch trampoline for
//! the duration of the call.

use {
    crate::{
        error::{LoadError, ScanEntryError},
        rc::{HasErrorStringDestructor, RcOutcome, classify_rc, load_to_scan_error, translate_rc},
        validation::{NetGrpcExpectations, check_net_grpc},
    },
    reovim_dylib_loader::{Kind, Library, PathResolverBuilder, Symbol, scan_paths},
    reovim_subsys_net::{
        NetError, ServiceDescriptor, TransportConfig,
        abi::{NetGrpcVTable, ShutdownFd},
    },
    std::{
        ffi::{c_char, c_void},
        path::Path,
        ptr,
        sync::atomic::AtomicU16,
    },
};

impl HasErrorStringDestructor for NetGrpcVTable {
    fn destroy_error_string_fn(&self) -> unsafe extern "C" fn(*mut c_char) {
        self.destroy_error_string
    }
}

/// Static-symbol name exported by a net-grpc driver cdylib.
const VTABLE_SYMBOL: &[u8] = b"REOVIM_NET_GRPC_DRIVER_VTABLE";

/// A net-grpc driver loaded from a cdylib.
///
/// Owns the [`reovim_dylib_loader::Library`] handle, the driver
/// instance pointer, and a `'static` reference to the driver's
/// exported vtable.
///
/// # Field order (load-bearing for `Drop`)
///
/// 1. `instance` — driver-allocated heap pointer; freed by
///    `vtable.destroy` in [`Drop::drop`].
/// 2. `vtable` — borrowed from the cdylib's `.rodata`; valid as long
///    as `_lib` stays mapped.
/// 3. `_lib` — keeps the cdylib mapped. Dropped LAST so dispatching
///    `vtable.shutdown` and `vtable.destroy` from `Drop::drop` is
///    sound (the function pointers still resolve into a live mapping).
pub struct LoadedNetGrpc {
    instance: *mut c_void,
    vtable: &'static NetGrpcVTable,
    _lib: Library,
}

// SAFETY: The driver contract (see `docs/architecture/driver-abi-v1.md`
// §13.5) requires every trampoline to be thread-safe; the macro-
// generated wrappers plus `catch_unwind` preserve `Send + Sync` as
// long as the user's `GrpcServerDriver` impl is `Send + Sync`, which
// is required by the trait.
unsafe impl Send for LoadedNetGrpc {}
unsafe impl Sync for LoadedNetGrpc {}

impl LoadedNetGrpc {
    /// Open a cdylib, validate its vtable header, and construct a
    /// driver instance.
    ///
    /// # Errors
    ///
    /// - `LoadError::LibraryOpen` if `dlopen` or symbol resolution fails.
    /// - `LoadError::Validation` if the vtable header does not match
    ///   the host's expectations.
    /// - `LoadError::DriverError` if the driver's `construct` returns a
    ///   negative error code.
    /// - `LoadError::DriverPanicked` if `construct` caught a panic.
    pub fn load_from_path(path: &Path) -> Result<Self, LoadError> {
        let lib = Library::open(path).map_err(|e| LoadError::LibraryOpen(e.to_string()))?;
        Self::validate_and_construct(lib)
    }

    /// Load every `driver/`-kind cdylib under `root`, returning one
    /// per-entry result per candidate.
    ///
    /// Same scan-path policy as the client-side
    /// `LoadedClientRender::from_path_scan`: system fallback paths are
    /// skipped, and per-entry errors are returned in their own `Err`
    /// arm rather than aborting the whole scan.
    ///
    /// The returned vec MAY be entirely populated with `Err` entries
    /// (every staged candidate failed validation/construct) or empty
    /// (`<root>/driver/` does not exist). Callers that require at least
    /// one usable driver must check both conditions; this helper makes
    /// no guarantee about the ratio of `Ok` to `Err` entries.
    #[must_use]
    pub fn from_path_scan(root: &Path) -> Vec<Result<Self, ScanEntryError>> {
        let resolver = PathResolverBuilder::for_kind(Kind::Driver)
            .without_system_fallback()
            .push_cli_path(root.join(Kind::Driver.subdir()))
            .build();
        scan_paths(resolver.paths())
            .into_entries()
            .into_iter()
            .map(|entry| match entry.outcome {
                Ok(lib) => Self::validate_and_construct(lib).map_err(load_to_scan_error),
                Err(e) => Err(ScanEntryError::Loader(e)),
            })
            .collect()
    }

    /// Run ABI validation and `construct` against an already-opened
    /// library. Shared between [`Self::load_from_path`] and
    /// [`Self::from_path_scan`].
    fn validate_and_construct(lib: Library) -> Result<Self, LoadError> {
        // SAFETY: resolving a `*const T` symbol is unsafe because
        // `libloading` cannot verify the exported type matches. The
        // contract in `docs/architecture/driver-abi-v1.md` §13.5 fixes
        // this symbol to a `NetGrpcVTable` static.
        let sym: Symbol<'_, *const NetGrpcVTable> = unsafe { lib.symbol(VTABLE_SYMBOL) }
            .map_err(|e| LoadError::LibraryOpen(e.to_string()))?;
        let vtable_ptr: *const NetGrpcVTable = *sym;

        // SAFETY: the vtable's first three header fields are
        // initialized by the driver's static initializer; reading them
        // before any other field is sound.
        unsafe { check_net_grpc(vtable_ptr, NetGrpcExpectations::from_host()) }?;

        // SAFETY: validation passed, so the pointer is non-null. The
        // vtable lives in the cdylib's `.rodata`; it is valid for as
        // long as the `Library` is mapped, which we keep alive in
        // `self._lib`.
        let vtable: &'static NetGrpcVTable = unsafe { &*vtable_ptr };

        let mut instance: *mut c_void = ptr::null_mut();
        let mut err_ptr: *mut c_char = ptr::null_mut();

        // SAFETY: `construct` is a valid function pointer in a vtable
        // that just passed header validation. The out-param pointers
        // are writable locals. The trampoline body is wrapped in
        // `catch_unwind` by the driver macro, so panics do not unwind
        // across the FFI boundary.
        let rc = unsafe { (vtable.construct)(&raw mut instance, &raw mut err_ptr) };
        translate_rc(rc, err_ptr, vtable)?;
        Ok(Self {
            instance,
            vtable,
            _lib: lib,
        })
    }

    /// Serve the configured transport with the supplied descriptors
    /// until `shutdown_fd` becomes readable.
    ///
    /// # Phase 5 scope
    ///
    /// `descriptors` MUST be empty. Non-empty descriptor lists return
    /// `Err(NetError::Io)` pointing at the `SP02b` deferral
    /// (`tmp/deferral-draft-sp02b-dispatch-call.md`). The host-side
    /// `BoxCloneService` → `FfiServiceDescriptor` marshalling is its
    /// own substantial engineering effort and is intentionally out of
    /// scope for the lifecycle MVP.
    ///
    /// # Concurrency
    ///
    /// The vtable's `serve` slot is synchronous (it `block_on`s the
    /// driver's owned tokio runtime per SP02 Phase 1 §D). Host async
    /// callers MUST drive this method on a `tokio::task::spawn_blocking`
    /// worker so the host runtime is not blocked.
    ///
    /// # Errors
    ///
    /// - [`NetError::Io`] if `descriptors` is non-empty (`SP02b` deferral).
    /// - [`NetError::ServeFailed`] / [`NetError::Io`] if the driver's
    ///   `serve` slot reports a failure.
    /// - [`NetError::Io("driver panicked at FFI boundary")`] if `serve`
    ///   caught a panic (-2).
    ///
    /// # Safety
    ///
    /// `port_writeback` MUST be a `'static` reference (allocated via
    /// `Box::leak`) so its address remains valid for the entire
    /// duration of the FFI call. The driver vtable's `serve` slot
    /// accepts a raw pointer to the atomic, but on the Rust side the
    /// `'static` bound enforces the host invariant.
    pub fn serve(
        &self,
        config: &TransportConfig,
        descriptors: Vec<ServiceDescriptor>,
        shutdown_fd: ShutdownFd,
        bind_ready_fd: ShutdownFd,
        port_writeback: &'static AtomicU16,
    ) -> Result<(), NetError> {
        invoke_serve(
            self.vtable,
            self.instance,
            config,
            descriptors,
            shutdown_fd,
            bind_ready_fd,
            port_writeback,
        )
    }
}

/// Shared serve trampoline body, factored out so unit tests can drive
/// it with a hand-built stub vtable + sentinel instance without going
/// through `dlopen`.
///
/// Matches the client-side pattern (`call_target`, `call_observe_start`)
/// where the lifecycle method's body lives in a free function so the
/// unit test surface does not need to fabricate a `Library`.
//
// `descriptors` is taken by value because the post-SP02b expansion
// consumes the Vec to produce one FFI descriptor per host service
// (handle alloc, dispatch trampoline pinning, name CString
// ownership). Phase 5 only branches on emptiness, but the by-value
// shape is the long-term contract.
#[allow(clippy::needless_pass_by_value)]
fn invoke_serve(
    vtable: &NetGrpcVTable,
    instance: *mut c_void,
    config: &TransportConfig,
    descriptors: Vec<ServiceDescriptor>,
    shutdown_fd: ShutdownFd,
    bind_ready_fd: ShutdownFd,
    port_writeback: &'static AtomicU16,
) -> Result<(), NetError> {
    if !descriptors.is_empty() {
        return Err(NetError::Io(
            "dispatch_call marshalling not yet implemented; tracked at SP02b".into(),
        ));
    }

    // `cfg_ffi` borrows from `config`; we keep `config` alive on this
    // stack frame through the entire FFI call so the host / path
    // string pointers inside `cfg_ffi` stay valid.
    let cfg_ffi = config.into_ffi();
    let mut err_ptr: *mut c_char = ptr::null_mut();
    let writeback_ptr: *const AtomicU16 = port_writeback;

    // SAFETY: `vtable.serve` was validated at load time; `instance` is
    // the pointer the driver returned from `construct` (or a test
    // sentinel for unit tests). The descriptor pointer + length is
    // `(null, 0)` because we rejected non-empty descriptors above.
    // `writeback_ptr` points at a `'static` atomic owned by the caller,
    // valid for the whole call. The trampoline body is
    // `catch_unwind`-wrapped by the driver macro.
    let rc = unsafe {
        (vtable.serve)(
            instance,
            &raw const cfg_ffi,
            ptr::null(),
            0,
            shutdown_fd,
            bind_ready_fd,
            writeback_ptr,
            &raw mut err_ptr,
        )
    };
    match classify_rc(rc, err_ptr, vtable) {
        RcOutcome::Ok => Ok(()),
        RcOutcome::Panicked => Err(NetError::Io("driver panicked at FFI boundary".into())),
        RcOutcome::Error(msg) => Err(NetError::ServeFailed(msg)),
    }
}

impl Drop for LoadedNetGrpc {
    fn drop(&mut self) {
        // Best-effort shutdown; ignore errors because Drop cannot
        // propagate them. `translate_rc` still frees any driver-
        // allocated error string to avoid leaks.
        let mut err_ptr: *mut c_char = ptr::null_mut();
        // SAFETY: `self.vtable.shutdown` and `self.vtable.destroy` were
        // validated at load time and the library is still mapped (the
        // `_lib` field, dropped last by struct field order, keeps the
        // cdylib alive through this destructor).
        let rc = unsafe { (self.vtable.shutdown)(self.instance, &raw mut err_ptr) };
        let _ = translate_rc(rc, err_ptr, self.vtable);
        // SAFETY: same contract; the driver re-boxes `instance` and
        // drops it.
        unsafe { (self.vtable.destroy)(self.instance) };
        // `_lib` drops last by field order, unmapping the cdylib after
        // the driver's destructor has returned.
    }
}

#[cfg(test)]
#[path = "net_grpc_tests.rs"]
mod tests;
