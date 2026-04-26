//! Safe wrappers over a runtime-loaded client-render driver cdylib.

use {
    crate::{
        error::{LoadError, ScanEntryError, ValidationError},
        rc::{HasErrorStringDestructor, RcOutcome, classify_rc, load_to_scan_error, translate_rc},
        validation::{ClientRenderExpectations, check_client_render},
    },
    reovim_client_subsys_render::{
        abi::{ClientRenderVTable, RenderTargetVTable},
        target::{RenderError, RenderTarget},
    },
    reovim_dylib_loader::{Kind, Library, PathResolverBuilder, Symbol, scan_paths},
    std::{
        ffi::{c_char, c_void},
        marker::PhantomData,
        path::Path,
        ptr,
    },
};

impl HasErrorStringDestructor for ClientRenderVTable {
    fn destroy_error_string_fn(&self) -> unsafe extern "C" fn(*mut c_char) {
        self.destroy_error_string
    }
}

/// Static-symbol name exported by a client-render driver cdylib.
const VTABLE_SYMBOL: &[u8] = b"REOVIM_CLIENT_RENDER_DRIVER_VTABLE";

/// A client-render driver loaded from a cdylib.
///
/// Owns the [`reovim_dylib_loader::Library`] handle, the driver
/// instance pointer, and a `'static` reference to the driver's
/// exported vtable. Field order matters: `Drop` runs `instance`-bound
/// destructors first and drops `_lib` last so dispatching into the
/// cdylib stays sound until the very end.
pub struct LoadedClientRender {
    /// Live instance handle; driver-owned heap pointer.
    instance: *mut c_void,
    /// Borrowed from the cdylib's `.rodata`; valid as long as `_lib`
    /// is not dropped.
    vtable: &'static ClientRenderVTable,
    /// Keeps the cdylib mapped. Dropped last.
    _lib: Library,
}

// SAFETY: The driver contract (see `docs/architecture/driver-abi-v1.md`)
// requires every trampoline to be thread-safe; the macro-generated
// wrappers plus `catch_unwind` preserve `Send + Sync` as long as the
// user's `ClientRender` impl is `Send + Sync`, which is required by
// the trait.
unsafe impl Send for LoadedClientRender {}
unsafe impl Sync for LoadedClientRender {}

// SAFETY: Same contract as `LoadedClientRender`. The sub-handle
// borrows from the parent and carries the same thread-safety guarantee.
unsafe impl Send for LoadedRenderTarget<'_> {}
unsafe impl Sync for LoadedRenderTarget<'_> {}

impl LoadedClientRender {
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

    /// Header-only check: does the cdylib at `path` export a valid
    /// client-render-driver vtable?
    ///
    /// Opens the cdylib, resolves the vtable symbol, and runs the same
    /// header validation `load_from_path` runs. The cdylib is unmapped
    /// before this function returns; no driver instance is constructed
    /// (so the driver's `construct` slot is never called). Used by the
    /// runtime lazy-load path to disambiguate render-vs-debug cdylibs
    /// before committing to the heavier `load_from_path`.
    ///
    /// # Errors
    ///
    /// - `LoadError::LibraryOpen` if `dlopen` or symbol resolution
    ///   fails (typical when the cdylib does not export the render
    ///   vtable, e.g. because it is a debug driver).
    /// - `LoadError::Validation` if the vtable header does not match
    ///   the host's expectations.
    pub fn probe_from_path(path: &Path) -> Result<(), LoadError> {
        let lib = Library::open(path).map_err(|e| LoadError::LibraryOpen(e.to_string()))?;
        // SAFETY: `VTABLE_SYMBOL` is the stable contract exported by
        // every client-render driver cdylib (see
        // `docs/architecture/driver-abi-v1.md`).
        let sym: Symbol<'_, *const ClientRenderVTable> = unsafe { lib.symbol(VTABLE_SYMBOL) }
            .map_err(|e| LoadError::LibraryOpen(e.to_string()))?;
        let vtable_ptr: *const ClientRenderVTable = *sym;
        // SAFETY: header fields are initialized by the cdylib's static
        // initializer; reading them before any other vtable field is
        // sound regardless of validation outcome.
        unsafe { check_client_render(vtable_ptr, ClientRenderExpectations::from_host()) }?;
        drop(lib);
        Ok(())
    }

    /// Load every `driver/`-kind cdylib under `root`, returning one
    /// per-entry result per candidate.
    ///
    /// The scan is infallible at the API level: a malformed cdylib,
    /// an unreadable file, or a missing `<root>/driver/` directory
    /// never panics — it surfaces as a [`ScanEntryError`] on the
    /// corresponding entry's `Err` arm. System fallback paths (XDG,
    /// `/usr/lib/reovim/`) are not consulted; `root` is the single
    /// search root so `reovim-dev` can point at staging directories
    /// without shadowing.
    ///
    // TODO(#769-phase-1.5): hoist a shared `PathScannable` trait so
    // this impl and the server-side `ModuleLoader::from_path_scan`
    // share one signature instead of two parallel ones.
    #[must_use]
    pub fn from_path_scan(root: &Path) -> Vec<Result<Self, ScanEntryError>> {
        Self::scan_and_construct(root, |_| true)
    }

    /// Eager-filtered variant of [`Self::from_path_scan`].
    ///
    /// Walks the same single-pass scan and skips entries whose package
    /// name (recovered via [`pkg_name_from_cdylib_filename`]) is
    /// registry-classified lazy. Entries whose filename does not match
    /// the convention fall through to the eager path — the filter is
    /// conservative.
    ///
    /// [`pkg_name_from_cdylib_filename`]: reovim_dylib_loader::pkg_name_from_cdylib_filename
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
    /// and `from_path_scan_filtered`. The caller's `keep` predicate
    /// is applied before `validate_and_construct` so a filtered-out
    /// entry is never opened beyond the initial scan.
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
        // SAFETY: resolving a `*const T` symbol is unsafe because
        // `libloading` cannot verify the exported type matches. The
        // contract in `docs/architecture/driver-abi-v1.md` fixes this
        // symbol to a `ClientRenderVTable` static.
        let sym: Symbol<'_, *const ClientRenderVTable> = unsafe { lib.symbol(VTABLE_SYMBOL) }
            .map_err(|e| LoadError::LibraryOpen(e.to_string()))?;
        let vtable_ptr: *const ClientRenderVTable = *sym;

        // SAFETY: The vtable's first three header fields are
        // initialized by the driver's static initializer; reading them
        // before any other field is sound.
        unsafe { check_client_render(vtable_ptr, ClientRenderExpectations::from_host()) }?;

        // SAFETY: validation passed, so the pointer is non-null. The
        // vtable lives in the cdylib's `.rodata`; it is valid for as
        // long as the `Library` is mapped, which we keep alive in
        // `self._lib`.
        let vtable: &'static ClientRenderVTable = unsafe { &*vtable_ptr };

        let mut instance: *mut c_void = ptr::null_mut();
        let mut err_ptr: *mut c_char = ptr::null_mut();
        // TODO(O7): FfiPlatformCaps repr(C) stability — Phase 2 tightens this.
        let platform: *mut c_void = ptr::null_mut();

        // SAFETY: `construct` is a valid function pointer in a vtable
        // that just passed header validation. The out-param pointers
        // are writable locals. The trampoline body is wrapped in
        // `catch_unwind` by the driver macro, so panics do not unwind
        // across the FFI boundary.
        let rc = unsafe { (vtable.construct)(platform, &raw mut instance, &raw mut err_ptr) };
        translate_rc(rc, err_ptr, vtable)?;
        Ok(Self {
            instance,
            vtable,
            _lib: lib,
        })
    }

    /// Borrow the driver's render target for frame submission.
    ///
    /// The returned handle holds an exclusive borrow on the driver so
    /// no two targets coexist.
    ///
    /// # Errors
    ///
    /// - `LoadError::DriverError` if the driver's `target` slot fails.
    /// - `LoadError::DriverPanicked` if `target` caught a panic.
    pub fn target(&mut self) -> Result<LoadedRenderTarget<'_>, LoadError> {
        let (sub_vtable, sub_handle) = call_target(self.vtable, self.instance)?;
        Ok(LoadedRenderTarget {
            vtable: sub_vtable,
            handle: sub_handle,
            driver_vtable: self.vtable,
            _borrow: PhantomData,
        })
    }
}

/// Invoke the driver's `target` trampoline and return the sub-vtable /
/// sub-handle pair, or a `LoadError` if the trampoline reported failure
/// or returned a null vtable pointer on success.
fn call_target(
    vtable: &ClientRenderVTable,
    instance: *mut c_void,
) -> Result<(*const RenderTargetVTable, *mut c_void), LoadError> {
    let mut sub_vtable: *const RenderTargetVTable = ptr::null();
    let mut sub_handle: *mut c_void = ptr::null_mut();
    let mut err_ptr: *mut c_char = ptr::null_mut();

    // SAFETY: `vtable.target` is valid per header validation performed
    // at load time; `instance` is the pointer the driver returned from
    // `construct`. Out-params are local.
    let rc = unsafe {
        (vtable.target)(instance, &raw mut sub_vtable, &raw mut sub_handle, &raw mut err_ptr)
    };
    translate_rc(rc, err_ptr, vtable)?;
    if sub_vtable.is_null() {
        return Err(LoadError::Validation(ValidationError::VtablePointerNull));
    }
    Ok((sub_vtable, sub_handle))
}

impl Drop for LoadedClientRender {
    fn drop(&mut self) {
        // Best-effort shutdown; ignore errors because Drop cannot
        // propagate them. `translate_rc` still frees any driver-
        // allocated error string to avoid leaks.
        let mut err_ptr: *mut c_char = ptr::null_mut();
        // SAFETY: `self.vtable.shutdown` and `self.vtable.destroy` were
        // validated at load time and the library is still mapped.
        let rc = unsafe { (self.vtable.shutdown)(self.instance, &raw mut err_ptr) };
        let _ = translate_rc(rc, err_ptr, self.vtable);
        // SAFETY: same contract.
        unsafe { (self.vtable.destroy)(self.instance) };
        // `_lib` drops last by field order, unmapping the cdylib after
        // the driver's destructor has returned.
    }
}

/// Safe handle to a `RenderTarget` borrowed from a [`LoadedClientRender`].
pub struct LoadedRenderTarget<'a> {
    pub(crate) vtable: *const RenderTargetVTable,
    pub(crate) handle: *mut c_void,
    /// Driver vtable used to free error strings produced by `submit`.
    pub(crate) driver_vtable: &'static ClientRenderVTable,
    pub(crate) _borrow: PhantomData<&'a mut LoadedClientRender>,
}

impl RenderTarget for LoadedRenderTarget<'_> {
    fn submit(&mut self, data: &[u8]) -> Result<(), RenderError> {
        let mut err_ptr: *mut c_char = ptr::null_mut();
        // SAFETY: `self.vtable` was validated non-null when the target
        // was obtained; it points into the driver's `.rodata`.
        let submit = unsafe { (*self.vtable).submit };
        // SAFETY: submit trampoline is `catch_unwind`-wrapped by the
        // driver macro. `data.as_ptr()` is valid for `data.len()` bytes
        // of reads for the duration of the call.
        let rc = unsafe { submit(self.handle, data.as_ptr(), data.len(), &raw mut err_ptr) };
        match classify_rc(rc, err_ptr, self.driver_vtable) {
            RcOutcome::Ok => Ok(()),
            RcOutcome::Panicked => {
                Err(RenderError::InvalidData("driver panicked at FFI boundary".to_owned()))
            }
            RcOutcome::Error(msg) => Err(RenderError::InvalidData(msg)),
        }
    }
}

#[cfg(test)]
#[path = "client_render_tests.rs"]
mod tests;
