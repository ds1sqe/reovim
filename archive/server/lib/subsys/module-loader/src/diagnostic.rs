//! Subsys-tier load diagnostic.
//!
//! Wraps the kernel-pure [`ModuleError`] so the eager-filtered scan
//! path can re-emit an `IncompatibleVersion` failure with the package
//! name recovered from the cdylib filename. The kernel `ModuleError`
//! surface is unchanged: only
//! [`ModuleLoader::from_path_scan_filtered_diag`](crate::loader::ModuleLoader::from_path_scan_filtered_diag)
//! returns this type. Other entry points that historically returned
//! `ModuleError` continue to do so.
//!
//! Lives at the subsys tier rather than as a kernel-error variant
//! because package-manager terminology (the recovered package name)
//! must not leak into the kernel public API; the kernel stays
//! policy-agnostic.

use {reovim_kernel::api::v1::ModuleError, thiserror::Error};

/// Subsys-tier load diagnostic.
#[derive(Debug, Error)]
pub enum LoadDiagnostic {
    /// Underlying error was not enrichable: either the cdylib filename
    /// did not match the reovim package-naming convention, or the error
    /// is not [`ModuleError::IncompatibleVersion`].
    #[error(transparent)]
    Bare(#[from] ModuleError),

    /// `IncompatibleVersion` produced by a cdylib whose filename
    /// matched the reovim package-naming convention. The recovered
    /// package name is surfaced in `Display` so user-facing diagnostics
    /// name the offending package; the original [`ModuleError`] is
    /// retained in `#[source]` for callers that want to match on the
    /// specific mismatch kind.
    #[error("ABI mismatch in package `{package}`: {source}")]
    AbiMismatchAtPackage {
        package: String,
        #[source]
        source: ModuleError,
    },
}

#[cfg(test)]
#[path = "diagnostic_tests.rs"]
mod diagnostic_tests;
