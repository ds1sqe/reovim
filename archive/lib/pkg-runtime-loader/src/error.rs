//! Errors surfaced by [`crate::load_registry`] and
//! [`crate::RuntimeLoaderConfig::load`].

use std::path::PathBuf;

/// Error returned when reading or parsing the runtime lockfile, or
/// when building the [`crate::LazyRegistry`] from it.
///
/// A missing lockfile is NOT modeled here — it's a normal state for
/// a host that hasn't run `pkg install`, and
/// [`crate::load_registry`] returns an empty registry instead.
#[derive(Debug, thiserror::Error)]
pub enum RuntimeLoaderError {
    /// The lockfile exists but could not be read from disk.
    #[error("lockfile not readable at {path}: {reason}")]
    LockfileReadFailed {
        /// Path that was attempted.
        path: PathBuf,
        /// Underlying I/O error stringified for `Display`.
        reason: String,
    },
    /// The lockfile was read but did not parse as a valid `pkg.lock`.
    #[error("lockfile at {path} is malformed: {source}")]
    LockfileMalformed {
        /// Lockfile path that failed to parse.
        path: PathBuf,
        /// Underlying parser error.
        #[source]
        source: reovim_pkg_lockfile::LockfileError,
    },
    /// The lockfile parsed but `LazyRegistry::from_lockfile` rejected
    /// one of its packages (e.g. malformed trigger string).
    #[error("lazy registry build failed for lockfile at {path}: {source}")]
    RegistryBuildFailed {
        /// Lockfile path whose contents failed registry construction.
        path: PathBuf,
        /// Underlying registry-build error.
        #[source]
        source: reovim_pkg_lazyload::LazyError,
    },
}
