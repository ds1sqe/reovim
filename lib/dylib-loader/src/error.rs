//! Error types for [`crate::Library`] operations.
//!
//! [`LoaderError`] wraps `libloading::Error` without exposing the
//! upstream type in the public API — callers match on the variant, not
//! on the upstream error. Keeps reovim consumers insulated from
//! `libloading` version bumps.

use std::path::PathBuf;

/// Failure mode for [`crate::Library::open`] and symbol resolution.
///
/// Variants carry the source path or symbol name so the caller can
/// attribute the failure without re-plumbing context through
/// `?`-chains. The inner `String` is `libloading`'s error text; we
/// store the text (not the typed `libloading::Error`) so the public
/// surface does not leak the dependency.
#[derive(Debug, thiserror::Error)]
pub enum LoaderError {
    /// Failed to dlopen a shared object at `path`.
    #[error("failed to open shared object at {path}: {source_text}")]
    LibraryOpen {
        /// The path the caller asked to open.
        path: PathBuf,
        /// Upstream `libloading::Error` rendered as text.
        source_text: String,
    },
    /// Failed to resolve a symbol by name from an open library.
    #[error("symbol {symbol} not found in shared object: {source_text}")]
    SymbolNotFound {
        /// The symbol name requested.
        symbol: String,
        /// Upstream `libloading::Error` rendered as text.
        source_text: String,
    },
}

/// Per-entry failure mode from [`crate::scan_paths`].
///
/// The scan API itself is infallible — a malformed cdylib, an
/// unreadable file, or a missing directory becomes a structured
/// `ScanEntryError` attached to the offending path and the scan
/// continues. Callers map over the per-entry results to surface
/// or ignore failures as they see fit.
#[derive(Debug, thiserror::Error)]
pub enum ScanEntryError {
    /// Failed to stat, read, or access a candidate cdylib.
    #[error("io error on {path}: {source}")]
    Io {
        /// The path that triggered the error.
        path: std::path::PathBuf,
        /// Underlying `io::Error`.
        source: std::io::Error,
    },
    /// The file exists and is readable but is not a valid shared
    /// object (for example: a text file with a `.so` extension, or a
    /// truncated cdylib).
    #[error("{path} is not a shared object: {source_text}")]
    NotALibrary {
        /// The offending path.
        path: std::path::PathBuf,
        /// Upstream `libloading::Error` rendered as text.
        source_text: String,
    },
    /// `dlopen` / `LoadLibrary` rejected the file for a reason other
    /// than format — missing dependency, symbol-resolution failure in
    /// a static initializer, permission error on a followed symlink.
    #[error("dlopen failed for {path}: {source_text}")]
    DlopenFailed {
        /// The offending path.
        path: std::path::PathBuf,
        /// Upstream error text.
        source_text: String,
    },
}

#[cfg(test)]
#[path = "error_tests.rs"]
mod error_tests;
