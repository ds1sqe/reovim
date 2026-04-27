//! Safe wrappers over a runtime-loaded client-debug driver cdylib.

use {
    crate::{
        error::{LoadError, ScanEntryError, ValidationError},
        rc::{HasErrorStringDestructor, RcOutcome, classify_rc, load_to_scan_error, translate_rc},
        validation::{ClientDebugExpectations, check_client_debug},
    },
    reovim_client_subsys_debug::{
        abi::{ClientDebugVTable, DebugObserverVTable},
        client_debug::DebugProbe,
    },
    reovim_dylib_loader::{Kind, Library, PathResolverBuilder, Symbol, scan_paths},
    std::{
        ffi::{c_char, c_void},
        marker::PhantomData,
        path::Path,
        ptr,
    },
};

/// Static-symbol name exported by a client-debug driver cdylib.
const VTABLE_SYMBOL: &[u8] = b"REOVIM_CLIENT_DEBUG_DRIVER_VTABLE";

impl HasErrorStringDestructor for ClientDebugVTable {
    fn destroy_error_string_fn(&self) -> unsafe extern "C" fn(*mut c_char) {
        self.destroy_error_string
    }
}

/// A client-debug driver loaded from a cdylib.
///
/// Owns the [`reovim_dylib_loader::Library`] handle, the driver
/// instance pointer, and a `'static` reference to the driver's
/// exported vtable. Field order matters: `Drop` runs `instance`-bound
/// destructors first and drops `_lib` last so dispatching into the
/// cdylib stays sound until the very end.
pub struct LoadedClientDebug {
    /// Live instance handle; driver-owned heap pointer.
    instance: *mut c_void,
    /// Borrowed from the cdylib's `.rodata`; valid as long as `_lib`
    /// is not dropped.
    vtable: &'static ClientDebugVTable,
    /// Keeps the cdylib mapped. Dropped last.
    _lib: Library,
}

// SAFETY: The driver contract (see `docs/architecture/driver-abi-v1.md`)
// requires every trampoline to be thread-safe; the macro-generated
// wrappers plus `catch_unwind` preserve `Send + Sync` as long as the
// user's `ClientDebugSurface` impl is `Send + Sync`, which is required
// by the trait.
unsafe impl Send for LoadedClientDebug {}
unsafe impl Sync for LoadedClientDebug {}

// SAFETY: Same contract as `LoadedClientDebug`. The sub-handle borrows
// from the parent and carries the same thread-safety guarantee.
unsafe impl Send for LoadedDebugObserver<'_> {}
unsafe impl Sync for LoadedDebugObserver<'_> {}

impl LoadedClientDebug {
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
        Self::validate_and_construct(lib).map_err(|err| match err {
            LoadError::Validation(v) => {
                reovim_pkg_runtime_loader::enrich_validation_error(path, v, |package, source| {
                    LoadError::AbiMismatchAtPackage { package, source }
                })
            }
            other => other,
        })
    }

    /// Read the driver's static probe WITHOUT constructing an instance.
    ///
    /// Opens the cdylib, resolves and validates the vtable header,
    /// then calls the `probe` slot (a pure `const` data query per the
    /// ABI). The cdylib is unmapped when this function returns. Used
    /// by `reovim cli debug probe` to enumerate debug-capable drivers.
    ///
    /// # Errors
    ///
    /// Same as [`Self::load_from_path`], but `DriverPanicked` becomes
    /// surfaceable here because the probe trampoline itself is
    /// `catch_unwind`-wrapped.
    pub fn probe_from_path(path: &Path) -> Result<DebugProbe, LoadError> {
        let lib = Library::open(path).map_err(|e| LoadError::LibraryOpen(e.to_string()))?;
        // SAFETY: the vtable symbol is the stable contract exported by
        // every debug-family driver cdylib.
        let sym: Symbol<'_, *const ClientDebugVTable> = unsafe { lib.symbol(VTABLE_SYMBOL) }
            .map_err(|e| LoadError::LibraryOpen(e.to_string()))?;
        let vtable_ptr: *const ClientDebugVTable = *sym;
        // SAFETY: header fields are initialized by the static
        // initializer in the cdylib.
        unsafe { check_client_debug(vtable_ptr, ClientDebugExpectations::from_host()) }?;
        // SAFETY: validation passed; pointer is non-null and the
        // library is still mapped for the duration of this call.
        let vtable: &ClientDebugVTable = unsafe { &*vtable_ptr };
        // SAFETY: probe is wrapped by the driver macro's
        // `catch_unwind`; returned value is plain `#[repr(C)]` data.
        let probe = unsafe { (vtable.probe)() };
        drop(lib);
        Ok(probe)
    }

    /// Load every `driver/`-kind cdylib under `root`, returning one
    /// per-entry result per candidate.
    ///
    /// Same scan-path policy as `LoadedClientRender::from_path_scan`.
    #[must_use]
    pub fn from_path_scan(root: &Path) -> Vec<Result<Self, ScanEntryError>> {
        Self::scan_and_construct(root, |_| true)
    }

    /// Eager-filtered variant of [`Self::from_path_scan`].
    ///
    /// Same `keep` semantics as
    /// [`LoadedClientRender::from_path_scan_filtered`] — the filter
    /// drops registry-lazy entries before construction; non-convention
    /// filenames fall through to the eager path.
    ///
    /// [`LoadedClientRender::from_path_scan_filtered`]: crate::LoadedClientRender::from_path_scan_filtered
    #[must_use]
    pub fn from_path_scan_filtered(
        root: &Path,
        registry: &reovim_pkg_lazyload::LazyRegistry,
    ) -> Vec<Result<Self, ScanEntryError>> {
        Self::scan_and_construct(root, |path| {
            reovim_pkg_runtime_loader::package_name_for_path(path)
                .is_none_or(|name| !registry.is_lazy(&name))
        })
    }

    /// Single-pass scan + construct helper shared by `from_path_scan`
    /// and `from_path_scan_filtered`.
    fn scan_and_construct(
        root: &Path,
        keep: impl Fn(&Path) -> bool,
    ) -> Vec<Result<Self, ScanEntryError>> {
        let resolver = PathResolverBuilder::for_kind(Kind::Driver)
            .without_system_fallback()
            .push_cli_path(root.join(Kind::Driver.subdir()))
            .build();
        scan_paths(resolver.paths())
            .into_entries()
            .into_iter()
            .filter(|entry| keep(&entry.path))
            .map(|entry| match entry.outcome {
                Ok(lib) => Self::validate_and_construct(lib).map_err(|err| match err {
                    LoadError::Validation(v) => reovim_pkg_runtime_loader::enrich_validation_error(
                        &entry.path,
                        v,
                        |package, source| ScanEntryError::AbiMismatchAtPackage { package, source },
                    ),
                    other => load_to_scan_error(other),
                }),
                Err(e) => Err(ScanEntryError::Loader(e)),
            })
            .collect()
    }

    /// Run ABI validation and `construct` against an already-opened
    /// library. Shared between [`Self::load_from_path`] and
    /// [`Self::from_path_scan`].
    fn validate_and_construct(lib: Library) -> Result<Self, LoadError> {
        // SAFETY: the contract in `docs/architecture/driver-abi-v1.md`
        // fixes this symbol to a `ClientDebugVTable` static.
        let sym: Symbol<'_, *const ClientDebugVTable> = unsafe { lib.symbol(VTABLE_SYMBOL) }
            .map_err(|e| LoadError::LibraryOpen(e.to_string()))?;
        let vtable_ptr: *const ClientDebugVTable = *sym;

        // SAFETY: header fields are initialized by the cdylib's static
        // initializer.
        unsafe { check_client_debug(vtable_ptr, ClientDebugExpectations::from_host()) }?;

        // SAFETY: validation passed; the vtable lives in the cdylib's
        // `.rodata` and stays valid while `self._lib` is mapped.
        let vtable: &'static ClientDebugVTable = unsafe { &*vtable_ptr };

        let mut instance: *mut c_void = ptr::null_mut();
        let mut err_ptr: *mut c_char = ptr::null_mut();
        // TODO(O7): FfiPlatformCaps repr(C) stability — Phase 2 tightens this.
        let platform: *mut c_void = ptr::null_mut();

        // SAFETY: `construct` is a valid function pointer in a vtable
        // that just passed header validation. The out-param pointers
        // are writable locals. The trampoline body is wrapped in
        // `catch_unwind` by the driver macro.
        let rc = unsafe { (vtable.construct)(platform, &raw mut instance, &raw mut err_ptr) };
        translate_rc(rc, err_ptr, vtable)?;
        Ok(Self {
            instance,
            vtable,
            _lib: lib,
        })
    }

    /// Read the driver's static probe (after `construct`). For the
    /// pre-construct query use [`Self::probe_from_path`].
    #[must_use]
    pub fn probe(&self) -> DebugProbe {
        // SAFETY: vtable passed header validation; probe is pure data.
        unsafe { (self.vtable.probe)() }
    }

    /// Open an observer for the given selector.
    ///
    /// The returned handle holds an exclusive borrow on the driver so
    /// no two observers coexist.
    ///
    /// # Errors
    ///
    /// - `LoadError::DriverError` if the driver's `observe_start` slot
    ///   fails (e.g. unknown selector).
    /// - `LoadError::DriverPanicked` if the slot caught a panic.
    /// - `LoadError::Validation(VtablePointerNull)` if the driver
    ///   returned success with a null observer vtable.
    pub fn observe(&mut self, selector: &[u8]) -> Result<LoadedDebugObserver<'_>, LoadError> {
        let (sub_vtable, sub_handle) = call_observe_start(self.vtable, self.instance, selector)?;
        Ok(LoadedDebugObserver {
            vtable: sub_vtable,
            handle: sub_handle,
            driver_vtable: self.vtable,
            _borrow: PhantomData,
        })
    }

    /// Execute a one-shot drive command. Opaque bytes in, opaque bytes
    /// out. The response `Vec` is driver-allocated; the host's
    /// `destroy_bytes` round-trip happens here before returning.
    ///
    /// # Errors
    ///
    /// - `LoadError::DriverError` if the driver's `drive` slot fails.
    /// - `LoadError::DriverPanicked` if the slot caught a panic.
    pub fn drive(&mut self, command: &[u8]) -> Result<Vec<u8>, LoadError> {
        let mut out_ptr: *mut u8 = ptr::null_mut();
        let mut out_len: usize = 0;
        let mut err_ptr: *mut c_char = ptr::null_mut();
        // SAFETY: `drive` is a valid function pointer in the validated
        // vtable; `command.as_ptr()/len` describe a valid slice for the
        // call's duration; out-params are writable locals.
        let rc = unsafe {
            (self.vtable.drive)(
                self.instance,
                command.as_ptr(),
                command.len(),
                &raw mut out_ptr,
                &raw mut out_len,
                &raw mut err_ptr,
            )
        };
        translate_rc(rc, err_ptr, self.vtable)?;
        Ok(copy_and_free_bytes(self.vtable, out_ptr, out_len))
    }
}

/// Invoke the driver's `observe_start` trampoline and return the
/// sub-vtable / sub-handle pair.
fn call_observe_start(
    vtable: &ClientDebugVTable,
    instance: *mut c_void,
    selector: &[u8],
) -> Result<(*const DebugObserverVTable, *mut c_void), LoadError> {
    let mut sub_vtable: *const DebugObserverVTable = ptr::null();
    let mut sub_handle: *mut c_void = ptr::null_mut();
    let mut err_ptr: *mut c_char = ptr::null_mut();

    // SAFETY: `observe_start` was validated at load time; `instance` is
    // the `construct` pointer; `selector` describes a valid byte slice
    // for the call's duration; out-params are local.
    let rc = unsafe {
        (vtable.observe_start)(
            instance,
            selector.as_ptr(),
            selector.len(),
            &raw mut sub_vtable,
            &raw mut sub_handle,
            &raw mut err_ptr,
        )
    };
    translate_rc(rc, err_ptr, vtable)?;
    if sub_vtable.is_null() {
        return Err(LoadError::Validation(ValidationError::VtablePointerNull));
    }
    Ok((sub_vtable, sub_handle))
}

/// Copy `len` bytes from a driver-allocated buffer into an owned
/// `Vec<u8>`, then free the source through the driver's
/// `destroy_bytes` slot. A null pointer produces an empty vec.
fn copy_and_free_bytes(vtable: &ClientDebugVTable, ptr: *mut u8, len: usize) -> Vec<u8> {
    if ptr.is_null() || len == 0 {
        if !ptr.is_null() {
            // SAFETY: ABI contract — even a zero-length allocation
            // round-trips through `destroy_bytes` exactly once.
            unsafe { (vtable.destroy_bytes)(ptr, len) };
        }
        return Vec::new();
    }
    // SAFETY: `ptr`/`len` describe a driver-allocated byte buffer
    // valid for reads. We COPY out, then hand the original back to the
    // driver for free. The copy avoids leaking driver allocator state
    // into host code that expects `Vec` owns its allocation.
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len) }.to_vec();
    // SAFETY: round-tripping ownership to the driver's allocator via
    // the ABI-specified `destroy_bytes(ptr, len)` slot. The driver's
    // trampoline reconstructs with `Vec::from_raw_parts(ptr, len, len)`
    // which is sound because the sibling trampoline that wrote
    // `(ptr, len)` called `shrink_to_fit()` first.
    unsafe { (vtable.destroy_bytes)(ptr, len) };
    bytes
}

impl Drop for LoadedClientDebug {
    fn drop(&mut self) {
        // Best-effort shutdown; ignore errors because Drop cannot
        // propagate them. `translate_rc` still frees any driver-
        // allocated error string to avoid leaks.
        let mut err_ptr: *mut c_char = ptr::null_mut();
        // SAFETY: the library is still mapped (Drop runs before `_lib`
        // is dropped by field order); the vtable slots passed
        // validation at load time.
        let rc = unsafe { (self.vtable.shutdown)(self.instance, &raw mut err_ptr) };
        let _ = translate_rc(rc, err_ptr, self.vtable);
        // SAFETY: same contract.
        unsafe { (self.vtable.destroy)(self.instance) };
        // `_lib` drops last by field order, unmapping the cdylib.
    }
}

/// Safe handle to a [`DebugObserver`] borrowed from a
/// [`LoadedClientDebug`].
pub struct LoadedDebugObserver<'a> {
    pub(crate) vtable: *const DebugObserverVTable,
    pub(crate) handle: *mut c_void,
    /// Driver vtable used to free error strings and frame buffers.
    pub(crate) driver_vtable: &'static ClientDebugVTable,
    pub(crate) _borrow: PhantomData<&'a mut LoadedClientDebug>,
}

impl LoadedDebugObserver<'_> {
    /// Pump the next frame.
    ///
    /// Returns `Ok(Some(bytes))` for a frame, `Ok(None)` at end of
    /// stream, `Err` on driver error or caught panic.
    ///
    /// # Errors
    ///
    /// - `LoadError::DriverError(msg)` if the driver reported a
    ///   stream-level error.
    /// - `LoadError::DriverPanicked` if the trampoline caught a panic.
    pub fn next_frame(&mut self) -> Result<Option<Vec<u8>>, LoadError> {
        let mut out_ptr: *mut u8 = ptr::null_mut();
        let mut out_len: usize = 0;
        let mut err_ptr: *mut c_char = ptr::null_mut();
        // SAFETY: `self.vtable` was validated non-null when the
        // observer was opened; the driver instance outlives this
        // observer per the borrow discipline.
        let next_frame = unsafe { (*self.vtable).next_frame };
        // SAFETY: the trampoline is `catch_unwind`-wrapped by the
        // driver macro. Out-params are writable locals.
        let rc = unsafe {
            next_frame(self.handle, &raw mut out_ptr, &raw mut out_len, &raw mut err_ptr)
        };
        // rc == 1 means end-of-stream — the observer vtable's custom
        // value outside the generic {0, -1, -2} set classify_rc
        // handles. Intercept before delegating.
        if rc == 1 {
            if !err_ptr.is_null() {
                // Defensive: a driver that writes an error on EOS is
                // misbehaving; free the buffer anyway.
                // SAFETY: driver-allocated CString round-trip.
                unsafe { (self.driver_vtable.destroy_error_string)(err_ptr) };
            }
            return Ok(None);
        }
        match classify_rc(rc, err_ptr, self.driver_vtable) {
            RcOutcome::Ok => Ok(Some(copy_and_free_bytes(self.driver_vtable, out_ptr, out_len))),
            RcOutcome::Panicked => Err(LoadError::DriverPanicked),
            RcOutcome::Error(msg) => Err(LoadError::DriverError(msg)),
        }
    }
}

impl Drop for LoadedDebugObserver<'_> {
    fn drop(&mut self) {
        // SAFETY: `self.vtable` is the sub-vtable returned by
        // `observe_start`; `call_observe_start` rejects null sub-
        // vtables before constructing `LoadedDebugObserver`, so both
        // pointers are non-null at drop time. `close` is infallible
        // per the ABI and its trampoline is `catch_unwind`-wrapped.
        let close = unsafe { (*self.vtable).close };
        // SAFETY: same contract.
        unsafe { close(self.handle) };
    }
}

#[cfg(test)]
#[path = "client_debug_tests.rs"]
mod tests;
