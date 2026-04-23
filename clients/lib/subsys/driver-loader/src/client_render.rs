//! Safe wrappers over a runtime-loaded client-render driver cdylib.

use {
    crate::{
        error::{LoadError, ValidationError},
        validation::{ClientRenderExpectations, check_client_render},
    },
    libloading::{Library, Symbol},
    reovim_client_subsys_render::{
        abi::{ClientRenderVTable, RenderTargetVTable},
        target::{RenderError, RenderTarget},
    },
    std::{
        ffi::{CStr, c_char, c_int, c_void},
        marker::PhantomData,
        path::Path,
        ptr,
    },
};

/// Static-symbol name exported by a client-render driver cdylib.
const VTABLE_SYMBOL: &[u8] = b"REOVIM_CLIENT_RENDER_DRIVER_VTABLE";

/// A client-render driver loaded from a cdylib.
///
/// Owns the `libloading::Library` handle, the driver instance pointer,
/// and a `'static` reference to the driver's exported vtable. Field
/// order matters: `Drop` runs `instance`-bound destructors first and
/// drops `_lib` last so dispatching into the cdylib stays sound until
/// the very end.
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
        // SAFETY: `libloading::Library::new` is unsafe because loading
        // a cdylib runs its static initializers, which can execute
        // arbitrary code. Callers must only point this at trusted
        // driver cdylibs resolved from the documented library root.
        let lib = unsafe { Library::new(path) }.map_err(|e| LoadError::LibraryOpen(e.to_string()))?;

        // SAFETY: resolving a `*const T` symbol is unsafe because
        // `libloading` cannot verify the exported type matches. The
        // contract in `docs/architecture/driver-abi-v1.md` fixes this
        // symbol to a `ClientRenderVTable` static.
        let sym: Symbol<'_, *const ClientRenderVTable> = unsafe { lib.get(VTABLE_SYMBOL) }
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
        let rc =
            unsafe { (vtable.construct)(platform, &raw mut instance, &raw mut err_ptr) };
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
        (vtable.target)(
            instance,
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

/// Copy a driver-allocated error string into an owned `String`, then
/// round-trip the buffer back through the driver's `destroy_error_string`
/// slot. Returns `"<no error message>"` if the pointer was null.
fn read_and_free_error(vtable: &ClientRenderVTable, ptr: *mut c_char) -> String {
    if ptr.is_null() {
        return "<no error message>".to_owned();
    }
    // SAFETY: `ptr` is a driver-allocated null-terminated C-string.
    let message = unsafe { CStr::from_ptr(ptr) }
        .to_string_lossy()
        .into_owned();
    // SAFETY: ownership round-trips to the driver's allocator.
    unsafe { (vtable.destroy_error_string)(ptr) };
    message
}

/// Outcome of a trampoline `c_int` return + error-string out-param,
/// decoupled from the specific `Error` type that a given call site
/// wants to surface. `translate_rc` maps this to `LoadError`; `submit`
/// maps it to `RenderError` directly.
enum RcOutcome {
    Ok,
    Panicked,
    Error(String),
}

/// Classify an FFI return code + error-string out-param into an
/// `RcOutcome`. Always consumes `err_ptr` (frees spurious-on-success or
/// panic-path strings, reads + frees the error-path string).
fn classify_rc(
    rc: c_int,
    err_ptr: *mut c_char,
    vtable: &ClientRenderVTable,
) -> RcOutcome {
    match rc {
        0 => {
            if !err_ptr.is_null() {
                // Defensive: a driver that writes an error string on
                // success is misbehaving but we still free the buffer.
                // SAFETY: driver-allocated CString round-trip.
                unsafe { (vtable.destroy_error_string)(err_ptr) };
            }
            RcOutcome::Ok
        }
        -2 => {
            if !err_ptr.is_null() {
                // SAFETY: driver-allocated CString round-trip.
                unsafe { (vtable.destroy_error_string)(err_ptr) };
            }
            RcOutcome::Panicked
        }
        _ => RcOutcome::Error(read_and_free_error(vtable, err_ptr)),
    }
}

/// Translate an FFI return code + error-string out-param into a
/// `Result<(), LoadError>`. Drop-time shutdown call sites that must
/// discard errors can call this and ignore the result; the error
/// string is still round-tripped through the driver's destroy slot.
fn translate_rc(
    rc: c_int,
    err_ptr: *mut c_char,
    vtable: &ClientRenderVTable,
) -> Result<(), LoadError> {
    match classify_rc(rc, err_ptr, vtable) {
        RcOutcome::Ok => Ok(()),
        RcOutcome::Panicked => Err(LoadError::DriverPanicked),
        RcOutcome::Error(msg) => Err(LoadError::DriverError(msg)),
    }
}

#[cfg(test)]
#[path = "client_render_tests.rs"]
mod tests;
