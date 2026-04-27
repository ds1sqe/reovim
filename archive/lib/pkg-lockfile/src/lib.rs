//! `pkg.lock` parser and writer for the reovim package manager.
//!
//! Cargo-style lockfile: a top-level `version` integer plus a list
//! of `[[package]]` tables. Each entry records the resolved
//! version, source, rustc target triple the cdylib was built for,
//! and optional SHA-256 digest for tamper detection. Serialization
//! is deterministic: packages are sorted by `(name, version)` before
//! emission.

#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![forbid(missing_docs)]

mod package_lock;
mod parser;
mod writer;

pub use crate::package_lock::{PackageLock, Source};

/// A parsed `pkg.lock` lockfile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lockfile {
    /// Lockfile format version. Starts at `1`.
    pub version: u32,
    /// Resolved packages. The writer sorts by `(name, version)`; the
    /// parser preserves input order but [`Lockfile::to_toml_string`]
    /// re-sorts before emission so serialized output is deterministic
    /// regardless of input order.
    pub packages: Vec<PackageLock>,
}

/// Error returned by lockfile parse and serialize operations.
#[derive(Debug, thiserror::Error)]
pub enum LockfileError {
    /// The TOML source could not be deserialized.
    #[error("lockfile parse error: {0}")]
    TomlDe(#[from] toml::de::Error),
    /// The lockfile value could not be serialized.
    #[error("lockfile serialize error: {0}")]
    TomlSer(#[from] toml::ser::Error),
    /// A `sha256` field was not a 64-character lowercase hex string.
    #[error("invalid sha256 for `{name}`: must be 64 lowercase hex characters, got `{value}`")]
    InvalidSha256 {
        /// Package name whose sha256 field was rejected.
        name: String,
        /// The offending hex string.
        value: String,
    },
}
