//! Safe wrapper over a runtime-loaded buffer-driver cdylib.
//!
//! Mirrors [`crate::net_grpc::LoadedNetGrpc`] but targets the
//! buffer-driver family ([`reovim_subsys_buffer::abi::BufferVTable`]).
//! Lifecycle is the same shape: open → validate → construct → serve →
//! drop (`buffer_destroy`/`destroy`).
//!
//! # Driver vs buffer lifetime
//!
//! Per-buffer instances ([`crate::buffer_instance::LoadedBufferInstance`])
//! must outlive the driver wrapper that produced them: they hold raw
//! pointers into the cdylib's `.rodata` (the vtable) and call into the
//! cdylib whenever a `Buffer` method is invoked. To keep the cdylib
//! mapped for that whole window we share a single
//! [`BufferLibraryGuard`] between the [`LoadedBuffer`] and every
//! [`LoadedBufferInstance`] derived from it via an `Arc`.
//!
//! `BufferLibraryGuard::Drop` is the single owner of `vtable.destroy`
//! and of unmapping the cdylib. It only runs after the last `Arc`
//! clone — driver wrapper plus every live buffer instance — is gone.
//!
//! # Field order (load-bearing for `Drop`)
//!
//! Inside [`BufferLibraryGuard`]:
//!
//! 1. `instance` — driver-allocated heap pointer; freed by
//!    `vtable.destroy` in [`Drop::drop`].
//! 2. `vtable` — borrowed from the cdylib's `.rodata`; valid as long
//!    as `lib` stays mapped.
//! 3. `lib` — keeps the cdylib mapped. Dropped LAST so dispatching
//!    `vtable.destroy` from `Drop::drop` is sound.

use {
    crate::{
        buffer_instance::LoadedBufferInstance,
        error::{LoadError, ScanEntryError},
        rc::{HasErrorStringDestructor, RcOutcome, classify_rc, load_to_scan_error, translate_rc},
        validation::{BufferExpectations, check_buffer},
    },
    reovim_dylib_loader::{Kind, Library, PathResolverBuilder, scan_paths},
    reovim_kernel::api::v1::BufferId,
    reovim_subsys_buffer::{Buffer, BufferDriver, BufferError, abi::BufferVTable},
    std::{
        ffi::{c_char, c_void},
        path::Path,
        ptr,
        sync::Arc,
    },
};

/// Static-symbol name exported by a buffer-driver cdylib.
pub(crate) const VTABLE_SYMBOL: &[u8] = b"REOVIM_BUFFER_DRIVER_VTABLE";

/// Shared mapping + driver-instance ownership.
///
/// Held inside an `Arc` shared between [`LoadedBuffer`] and every
/// [`LoadedBufferInstance`] it produces. The single `Drop` runs once
/// after the last clone is released.
pub(crate) struct BufferLibraryGuard {
    pub(crate) instance: *mut c_void,
    pub(crate) vtable: &'static BufferVTable,
    _lib: Library,
}

// SAFETY: The driver contract (see `docs/architecture/driver-abi-v1.md`
// §13.5) requires every trampoline to be thread-safe; the macro-
// generated wrappers plus `catch_unwind` preserve `Send + Sync` as
// long as the user's `BufferDriver` impl is `Send + Sync`, which is
// required by the trait. The vtable static lives in the cdylib's
// `.rodata` and is read-only, so the `&'static BufferVTable` is sound
// to share across threads.
unsafe impl Send for BufferLibraryGuard {}
unsafe impl Sync for BufferLibraryGuard {}

impl Drop for BufferLibraryGuard {
    fn drop(&mut self) {
        // SAFETY: `self.vtable.destroy` was validated at load time and
        // the library is still mapped (the `_lib` field, dropped last
        // by struct field order, keeps the cdylib alive through this
        // destructor). `instance` was returned by `vtable.construct`.
        unsafe { (self.vtable.destroy)(self.instance) };
        // `_lib` drops last by field order, unmapping the cdylib after
        // the driver's destructor has returned.
    }
}

impl HasErrorStringDestructor for BufferVTable {
    fn destroy_error_string_fn(&self) -> unsafe extern "C" fn(*mut c_char) {
        self.destroy_error_string
    }
}

/// A buffer driver loaded from a cdylib.
///
/// Wraps a shared [`BufferLibraryGuard`] so that buffer instances
/// produced via `BufferDriver::create_buffer`/`open_buffer`/`get_buffer`
/// can outlive the driver wrapper without unmapping the cdylib while
/// they still hold function-pointer references into it.
pub struct LoadedBuffer {
    pub(crate) guard: Arc<BufferLibraryGuard>,
}

impl LoadedBuffer {
    /// Open a cdylib, validate its vtable header, and construct a
    /// driver instance.
    ///
    /// # Errors
    ///
    /// - [`LoadError::LibraryOpen`] if `dlopen` or symbol resolution fails.
    /// - [`LoadError::Validation`] if the vtable header does not match
    ///   the host's expectations.
    /// - [`LoadError::DriverError`] if the driver's `construct` returns
    ///   a negative error code.
    /// - [`LoadError::DriverPanicked`] if `construct` caught a panic.
    pub fn load_from_path(path: &Path) -> Result<Self, LoadError> {
        let lib = Library::open(path).map_err(|e| LoadError::LibraryOpen(e.to_string()))?;
        Self::validate_and_construct(lib)
    }

    /// Load every `driver/`-kind cdylib under `root`, returning one
    /// per-entry result per candidate.
    ///
    /// Same scan-path policy as [`crate::net_grpc::LoadedNetGrpc::from_path_scan`]:
    /// system fallback paths are skipped and per-entry errors are
    /// returned in their own `Err` arm rather than aborting the whole
    /// scan.
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
        // contract in `docs/architecture/driver-abi-v1.md` fixes this
        // symbol to a `BufferVTable` static.
        let sym: reovim_dylib_loader::Symbol<'_, *const BufferVTable> =
            unsafe { lib.symbol(VTABLE_SYMBOL) }
                .map_err(|e| LoadError::LibraryOpen(e.to_string()))?;
        let vtable_ptr: *const BufferVTable = *sym;

        // SAFETY: the vtable's first three header fields are
        // initialized by the driver's static initializer; reading them
        // before any other field is sound.
        unsafe { check_buffer(vtable_ptr, BufferExpectations::from_host()) }?;

        // SAFETY: validation passed, so the pointer is non-null. The
        // vtable lives in the cdylib's `.rodata`; it is valid for as
        // long as the `Library` is mapped, which the guard keeps alive.
        let vtable: &'static BufferVTable = unsafe { &*vtable_ptr };

        let mut instance: *mut c_void = ptr::null_mut();
        let mut err_ptr: *mut c_char = ptr::null_mut();

        // SAFETY: `construct` is a valid function pointer in a vtable
        // that just passed header validation. The out-param pointers
        // are writable locals. The trampoline body is wrapped in
        // `catch_unwind` by the driver macro.
        let rc = unsafe { (vtable.construct)(&raw mut instance, &raw mut err_ptr) };
        translate_rc(rc, err_ptr, vtable)?;
        Ok(Self {
            guard: Arc::new(BufferLibraryGuard {
                instance,
                vtable,
                _lib: lib,
            }),
        })
    }

    /// Wrap a raw FFI buffer handle in a [`LoadedBufferInstance`] that
    /// shares this driver's library guard.
    fn wrap_buffer(&self, handle: *mut c_void) -> Arc<dyn Buffer> {
        Arc::new(LoadedBufferInstance::from_raw(handle, Arc::clone(&self.guard)))
    }
}

impl BufferDriver for LoadedBuffer {
    fn kind(&self) -> &'static str {
        "buffer"
    }

    fn create_buffer(
        &self,
        initial_bytes: &[u8],
        file_path: Option<String>,
    ) -> Result<Arc<dyn Buffer>, BufferError> {
        invoke_create_buffer(
            self.guard.vtable,
            self.guard.instance,
            initial_bytes,
            file_path.as_deref(),
        )
        .map(|handle| self.wrap_buffer(handle))
    }

    fn open_buffer(&self, file_path: &str) -> Result<Arc<dyn Buffer>, BufferError> {
        invoke_open_buffer(self.guard.vtable, self.guard.instance, file_path)
            .map(|handle| self.wrap_buffer(handle))
    }

    fn list_buffers(&self) -> Vec<BufferId> {
        invoke_list_buffers(self.guard.vtable, self.guard.instance).unwrap_or_default()
    }

    fn get_buffer(&self, id: BufferId) -> Option<Arc<dyn Buffer>> {
        invoke_get_buffer(self.guard.vtable, self.guard.instance, id)
            .map(|handle| self.wrap_buffer(handle))
    }

    fn close_buffer(&self, id: BufferId) -> Result<(), BufferError> {
        invoke_close_buffer(self.guard.vtable, self.guard.instance, id)
    }
}

// ────────────────────────────────────────────────────────────────────
// Free-function trampoline bodies. Factored out so unit tests can
// drive each path with a hand-built stub vtable + sentinel instance
// without going through `dlopen`. Mirrors the `invoke_serve` pattern
// in `net_grpc.rs`.
// ────────────────────────────────────────────────────────────────────

/// Map a non-`Ok` [`RcOutcome`] to a [`BufferError`].
///
/// Shared by the driver wrapper and the per-buffer wrapper
/// ([`crate::buffer_instance`]); both need the same translation.
pub(crate) fn rc_to_buffer_error(outcome: RcOutcome) -> BufferError {
    match outcome {
        RcOutcome::Ok => unreachable!("rc_to_buffer_error called on Ok"),
        RcOutcome::Panicked => BufferError::Driver("driver panicked at FFI boundary".to_owned()),
        RcOutcome::Error(msg) => BufferError::Driver(msg),
    }
}

#[allow(clippy::cast_possible_truncation)] // usize ↔ u64 conversions are sound on 64-bit
pub(crate) fn invoke_create_buffer(
    vtable: &BufferVTable,
    instance: *mut c_void,
    initial_bytes: &[u8],
    file_path: Option<&str>,
) -> Result<*mut c_void, BufferError> {
    let (path_ptr, path_len) = file_path
        .map_or((ptr::null::<c_char>(), 0usize), |p| (p.as_ptr().cast::<c_char>(), p.len()));
    let mut out_buffer: *mut c_void = ptr::null_mut();
    let mut out_id: u64 = 0;
    let mut err_ptr: *mut c_char = ptr::null_mut();

    // SAFETY: `vtable.create_buffer` was validated at load time. The
    // byte and path pointers are pinned for the duration of this call
    // (the slice + Option<&str> outlive the FFI dispatch). Out-params
    // are writable locals.
    let rc = unsafe {
        (vtable.create_buffer)(
            instance,
            initial_bytes.as_ptr(),
            initial_bytes.len(),
            path_ptr,
            path_len,
            &raw mut out_buffer,
            &raw mut out_id,
            &raw mut err_ptr,
        )
    };
    match classify_rc(rc, err_ptr, vtable) {
        RcOutcome::Ok => Ok(out_buffer),
        other => Err(rc_to_buffer_error(other)),
    }
}

pub(crate) fn invoke_open_buffer(
    vtable: &BufferVTable,
    instance: *mut c_void,
    file_path: &str,
) -> Result<*mut c_void, BufferError> {
    let mut out_buffer: *mut c_void = ptr::null_mut();
    let mut out_id: u64 = 0;
    let mut err_ptr: *mut c_char = ptr::null_mut();

    // SAFETY: `vtable.open_buffer` was validated at load time. Path
    // pointer is pinned for the duration of this call.
    let rc = unsafe {
        (vtable.open_buffer)(
            instance,
            file_path.as_ptr().cast::<c_char>(),
            file_path.len(),
            &raw mut out_buffer,
            &raw mut out_id,
            &raw mut err_ptr,
        )
    };
    match classify_rc(rc, err_ptr, vtable) {
        RcOutcome::Ok => Ok(out_buffer),
        other => Err(rc_to_buffer_error(other)),
    }
}

#[allow(clippy::cast_possible_truncation)] // u64 → usize is sound on 64-bit
pub(crate) fn invoke_list_buffers(
    vtable: &BufferVTable,
    instance: *mut c_void,
) -> Result<Vec<BufferId>, BufferError> {
    let mut ids_ptr: *mut u64 = ptr::null_mut();
    let mut count: usize = 0;
    let mut err_ptr: *mut c_char = ptr::null_mut();

    // SAFETY: `vtable.list_buffers` was validated at load time. The
    // out-params are writable locals.
    let rc = unsafe {
        (vtable.list_buffers)(instance, &raw mut ids_ptr, &raw mut count, &raw mut err_ptr)
    };
    match classify_rc(rc, err_ptr, vtable) {
        RcOutcome::Ok => {
            let ids = if ids_ptr.is_null() || count == 0 {
                Vec::new()
            } else {
                // SAFETY: the driver allocated `count` `u64` ids at
                // `ids_ptr`; we copy them out before handing the
                // allocation back via `destroy_id_list`.
                let slice = unsafe { std::slice::from_raw_parts(ids_ptr, count) };
                slice
                    .iter()
                    .map(|raw| BufferId::from_raw(*raw as usize))
                    .collect()
            };
            if !ids_ptr.is_null() {
                // SAFETY: `ids_ptr` was driver-allocated by the same
                // call we're now freeing; the destructor matches the
                // allocator.
                unsafe { (vtable.destroy_id_list)(ids_ptr, count) };
            }
            Ok(ids)
        }
        other => Err(rc_to_buffer_error(other)),
    }
}

#[allow(clippy::cast_possible_truncation)] // usize → u64 is sound on 64-bit
pub(crate) fn invoke_get_buffer(
    vtable: &BufferVTable,
    instance: *mut c_void,
    id: BufferId,
) -> Option<*mut c_void> {
    let mut out_buffer: *mut c_void = ptr::null_mut();
    let mut err_ptr: *mut c_char = ptr::null_mut();

    // SAFETY: `vtable.get_buffer` was validated at load time. Out-
    // params are writable locals.
    let rc = unsafe {
        (vtable.get_buffer)(instance, id.as_usize() as u64, &raw mut out_buffer, &raw mut err_ptr)
    };
    // The macro contract returns 0 + non-null on hit, -1 + null on
    // miss, -1 + err_ptr on error. We treat both miss and error as
    // None for the trait surface (matching the trait's `Option<...>`
    // return). Free any spurious error string to avoid leaks.
    match classify_rc(rc, err_ptr, vtable) {
        RcOutcome::Ok if !out_buffer.is_null() => Some(out_buffer),
        _ => None,
    }
}

#[allow(clippy::cast_possible_truncation)] // usize → u64 is sound on 64-bit
pub(crate) fn invoke_close_buffer(
    vtable: &BufferVTable,
    instance: *mut c_void,
    id: BufferId,
) -> Result<(), BufferError> {
    let mut err_ptr: *mut c_char = ptr::null_mut();

    // SAFETY: `vtable.close_buffer` was validated at load time. Out-
    // param is a writable local.
    let rc = unsafe { (vtable.close_buffer)(instance, id.as_usize() as u64, &raw mut err_ptr) };
    match classify_rc(rc, err_ptr, vtable) {
        RcOutcome::Ok => Ok(()),
        other => Err(rc_to_buffer_error(other)),
    }
}

#[cfg(test)]
#[path = "buffer_tests.rs"]
mod tests;
