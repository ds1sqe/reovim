//! Shared FFI return-code and error-string helpers.
//!
//! Every server driver family exports a vtable whose slots return a
//! `c_int` return code plus an optional driver-allocated error C-string
//! out-param. This module centralizes the "decode the rc, read the
//! string, round-trip it back through the driver's
//! `destroy_error_string` slot" logic so each family's loader wrapper
//! carries no duplicated per-slot plumbing.
//!
//! Parameterization is by trait: every vtable that carries a
//! `destroy_error_string: unsafe extern "C" fn(*mut c_char)` slot
//! implements [`HasErrorStringDestructor`] and plugs into the helpers.

use {
    crate::error::{LoadError, ScanEntryError, ValidationError},
    std::ffi::{CStr, c_char, c_int},
};

/// Vtables that carry a `destroy_error_string` slot.
///
/// Every server driver vtable that follows the standard `c_int + out_err`
/// trampoline calling convention implements this with a one-line
/// accessor. The trait keeps the helpers below generic without
/// introducing a new public API surface: the trait is `pub`, and the
/// vtable types themselves stay untouched.
pub trait HasErrorStringDestructor {
    /// The driver-allocated-C-string destructor slot.
    fn destroy_error_string_fn(&self) -> unsafe extern "C" fn(*mut c_char);
}

/// Copy a driver-allocated error string into an owned `String`, then
/// round-trip the buffer back through the driver's
/// `destroy_error_string` slot. Returns `"<no error message>"` if the
/// pointer was null.
pub fn read_and_free_error<V: HasErrorStringDestructor>(vtable: &V, ptr: *mut c_char) -> String {
    if ptr.is_null() {
        return "<no error message>".to_owned();
    }
    // SAFETY: `ptr` is a driver-allocated null-terminated C-string.
    let message = unsafe { CStr::from_ptr(ptr) }
        .to_string_lossy()
        .into_owned();
    // SAFETY: ownership round-trips to the driver's allocator.
    unsafe { (vtable.destroy_error_string_fn())(ptr) };
    message
}

/// Outcome of a trampoline `c_int` return + error-string out-param,
/// decoupled from the specific `Error` type that a given call site
/// wants to surface. [`translate_rc`] maps this to [`LoadError`]; slot-
/// specific call sites map it themselves (e.g. net-grpc's `serve` maps
/// to `NetError`).
pub enum RcOutcome {
    Ok,
    Panicked,
    Error(String),
}

/// Classify an FFI return code + error-string out-param into an
/// [`RcOutcome`]. Always consumes `err_ptr` (frees spurious-on-success
/// or panic-path strings, reads + frees the error-path string).
pub fn classify_rc<V: HasErrorStringDestructor>(
    rc: c_int,
    err_ptr: *mut c_char,
    vtable: &V,
) -> RcOutcome {
    match rc {
        0 => {
            if !err_ptr.is_null() {
                // Defensive: a driver that writes an error string on
                // success is misbehaving but we still free the buffer.
                // SAFETY: driver-allocated CString round-trip.
                unsafe { (vtable.destroy_error_string_fn())(err_ptr) };
            }
            RcOutcome::Ok
        }
        -2 => {
            if !err_ptr.is_null() {
                // SAFETY: driver-allocated CString round-trip.
                unsafe { (vtable.destroy_error_string_fn())(err_ptr) };
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
pub fn translate_rc<V: HasErrorStringDestructor>(
    rc: c_int,
    err_ptr: *mut c_char,
    vtable: &V,
) -> Result<(), LoadError> {
    match classify_rc(rc, err_ptr, vtable) {
        RcOutcome::Ok => Ok(()),
        RcOutcome::Panicked => Err(LoadError::DriverPanicked),
        RcOutcome::Error(msg) => Err(LoadError::DriverError(msg)),
    }
}

/// Convert a single-driver [`LoadError`] into the per-scan-entry
/// [`ScanEntryError`] returned by `Loaded<Kind>::from_path_scan`
/// helpers.
///
/// `LibraryOpen` at this point — i.e. AFTER the scan layer accepted
/// the cdylib via `entry.outcome == Ok(lib)` — is structurally only
/// reachable when `validate_and_construct`'s symbol lookup failed
/// because the file is not a reovim driver (no `REOVIM_*_VTABLE`
/// export). Filesystem-level open failures are reported by the scan
/// layer as `DylibScanError` and route through [`ScanEntryError::Loader`]
/// instead. The "missing vtable symbol" outcome is therefore an ABI
/// mismatch in the cdylib's perspective: there is no real vtable to
/// read, so we model it as `AbiMismatch(VtablePointerNull)` (a future
/// refinement could add a dedicated `SymbolNotFound` variant; matched
/// to client-side `clients/lib/subsys/driver-loader/src/rc.rs` for
/// parity).
pub fn load_to_scan_error(err: LoadError) -> ScanEntryError {
    match err {
        LoadError::LibraryOpen(_) => {
            ScanEntryError::AbiMismatch(ValidationError::VtablePointerNull)
        }
        LoadError::Validation(v) => ScanEntryError::AbiMismatch(v),
        LoadError::DriverError(msg) => ScanEntryError::DriverError(msg),
        LoadError::DriverPanicked => ScanEntryError::DriverPanicked,
    }
}

#[cfg(test)]
#[path = "rc_tests.rs"]
mod tests;
