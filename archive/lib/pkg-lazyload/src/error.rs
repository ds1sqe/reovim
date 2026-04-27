//! Error types surfaced by this crate.

use reovim_pkg_manifest::ManifestError;

/// Error returned when building or querying a [`crate::LazyRegistry`].
#[derive(Debug, thiserror::Error)]
pub enum LazyError {
    /// A lockfile package's `trigger` field did not parse as a valid
    /// [`reovim_pkg_manifest::LazyTrigger`] string.
    #[error("package `{pkg}`: malformed trigger `{value}`")]
    MalformedTrigger {
        /// Package name whose `trigger` field was malformed.
        pkg: String,
        /// Offending string as read from the lockfile.
        value: String,
        /// Underlying manifest parse error. Boxed to keep the
        /// `Result` returned by [`crate::LazyRegistry::from_lockfile`]
        /// inside the `clippy::result_large_err` 128-byte budget.
        #[source]
        source: Box<ManifestError>,
    },
}
