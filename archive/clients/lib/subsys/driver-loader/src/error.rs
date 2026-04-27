//! Driver-loader error types.

pub use reovim_dylib_loader::ScanEntryError as DylibScanError;
use thiserror::Error;

/// Failure to load or dispatch a driver cdylib.
#[derive(Debug, Error)]
pub enum LoadError {
    /// `dlopen` or symbol resolution failed at the OS level.
    #[error("failed to open driver cdylib: {0}")]
    LibraryOpen(String),

    /// The cdylib is present but its vtable does not meet ABI requirements.
    #[error(transparent)]
    Validation(#[from] ValidationError),

    /// ABI validation failed for a cdylib whose filename matched the
    /// reovim package-naming convention. The recovered package name is
    /// surfaced in `Display` so user-facing diagnostics name the
    /// offending package; the original [`ValidationError`] is retained
    /// in `#[source]` for callers that want to match on the specific
    /// mismatch kind.
    #[error("ABI mismatch in package `{package}`: {source}")]
    AbiMismatchAtPackage {
        package: String,
        #[source]
        source: ValidationError,
    },

    /// The driver's `construct` slot returned an error; payload is the
    /// driver-allocated message copied into an owned `String` before the
    /// driver's `destroy_error_string` slot freed the source buffer.
    #[error("driver returned error: {0}")]
    DriverError(String),

    /// The driver trampoline caught a panic and returned `-2`.
    #[error("driver panicked at FFI boundary")]
    DriverPanicked,
}

/// Binary-ABI mismatch detected before any driver code executes.
#[derive(Debug, Error)]
pub enum ValidationError {
    /// The vtable pointer resolved to null.
    #[error("vtable pointer is null")]
    VtablePointerNull,

    /// `abi_version` does not match the host's expected epoch.
    #[error("ABI version mismatch: driver={found}, host={expected}")]
    AbiVersionMismatch { found: u32, expected: u32 },

    /// `api_version` major differs or driver's minor is below required.
    #[error(
        "API version incompatible: driver={found_major}.{found_minor}.{found_patch}, host requires {expected_major}.{expected_minor}+"
    )]
    ApiVersionIncompatible {
        found_major: u32,
        found_minor: u32,
        found_patch: u32,
        expected_major: u32,
        expected_minor: u32,
    },

    /// Driver was built against a different vtable size; fields may
    /// have been added or reordered.
    #[error("size_of_self mismatch: driver={found}, host={expected}")]
    SizeOfSelfMismatch { found: usize, expected: usize },
}

/// Per-entry failure returned by
/// [`crate::LoadedClientRender::from_path_scan`].
///
/// Bridges `reovim_dylib_loader`'s scan-level failures (filesystem
/// access, malformed cdylib, dlopen refusal) with driver-loader's
/// ABI-level failures (vtable validation, construct rejection). Every
/// per-entry `Result` returned by `from_path_scan` carries one of
/// these on its error arm.
#[derive(Debug, Error)]
pub enum ScanEntryError {
    /// The scan layer rejected the candidate cdylib (missing,
    /// malformed, or dlopen-refused).
    #[error(transparent)]
    Loader(#[from] DylibScanError),

    /// The cdylib opened but its vtable header failed ABI validation.
    #[error(transparent)]
    AbiMismatch(#[from] ValidationError),

    /// ABI validation failed for a scanned cdylib whose filename
    /// matched the reovim package-naming convention. The recovered
    /// package name is surfaced in `Display` so the per-entry
    /// scan result names the offending package.
    #[error("ABI mismatch in package `{package}`: {source}")]
    AbiMismatchAtPackage {
        package: String,
        #[source]
        source: ValidationError,
    },

    /// The driver's `construct` slot returned a non-zero error code.
    #[error("driver returned error: {0}")]
    DriverError(String),

    /// The driver trampoline caught a panic (`construct` returned -2).
    #[error("driver panicked at FFI boundary")]
    DriverPanicked,
}
