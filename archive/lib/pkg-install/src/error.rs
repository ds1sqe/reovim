//! Installer error type.

use std::path::PathBuf;

use reovim_pkg_manifest::ManifestError;

/// Every failure mode the installer exposes.
///
/// Each variant carries enough context for a plain `eprintln!("{err}")`
/// to locate the faulty package or filesystem path.
#[derive(Debug, thiserror::Error)]
pub enum InstallError {
    /// The resolver refused the dep graph.
    #[error(transparent)]
    Resolve(#[from] reovim_pkg_resolver::ResolveError),
    /// A `pkg.toml` reached by the installer failed to parse.
    #[error("failed to parse manifest at `{at}`: {source}", at = .at.display())]
    Manifest {
        /// Path to the offending manifest file.
        at: PathBuf,
        /// Underlying manifest parse error.
        #[source]
        source: ManifestError,
    },
    /// The lockfile at the configured path failed to parse or write.
    #[error(transparent)]
    Lockfile(#[from] reovim_pkg_lockfile::LockfileError),
    /// A path-dep manifest is missing the `[package].kind` field.
    #[error("path-dep manifest `{pkg}` is missing `[package].kind` (driver or module)")]
    MissingPackageKind {
        /// Package name declared by the offending manifest.
        pkg: String,
    },
    /// The package directory has no `dist/` subdirectory.
    #[error("package `{pkg}` has no `dist/` directory at `{at}`", at = .at.display())]
    MissingDistDir {
        /// Package name.
        pkg: String,
        /// Probed dist path.
        at: PathBuf,
    },
    /// The expected cdylib file is missing from `dist/`.
    #[error("cdylib for `{pkg}` not found at `{probed}`", probed = .probed.display())]
    MissingArtifact {
        /// Package name.
        pkg: String,
        /// Path the installer probed.
        probed: PathBuf,
    },
    /// The cdylib exists but `dlopen` rejected it.
    #[error("cdylib at `{at}` is not loadable: {reason}", at = .at.display())]
    NotLoadable {
        /// Path to the rejected cdylib.
        at: PathBuf,
        /// Message from the underlying loader.
        reason: String,
    },
    /// An I/O error reading a package file (manifest or cdylib).
    #[error("failed to read `{at}`: {reason}", at = .at.display())]
    ArtifactReadFailed {
        /// Path that failed to read.
        at: PathBuf,
        /// OS error message.
        reason: String,
    },
    /// An I/O error writing to the library root.
    #[error("failed to write `{at}`: {reason}", at = .at.display())]
    WriteFailed {
        /// Path that failed to write.
        at: PathBuf,
        /// OS error message.
        reason: String,
    },
    /// `uninstall` target is not in the inventory.
    #[error("package `{name}` is not installed")]
    NotInstalled {
        /// Package name.
        name: String,
    },
    /// Existing lockfile entry's sha256 disagrees with the bytes now
    /// on disk. Either the cdylib changed between runs or the
    /// lockfile was tampered with.
    #[error(
        "cdylib for `{pkg}` has changed since the last install: expected sha256 `{expected}`, got `{actual}`"
    )]
    TamperedArtifact {
        /// Package name.
        pkg: String,
        /// Digest recorded in the existing lockfile.
        expected: String,
        /// Digest just computed from the cdylib on disk.
        actual: String,
    },
    /// A resolved package came from a non-local source. Phase 2
    /// supports local paths only.
    #[error("package `{pkg}` has a non-local source; Phase 2 supports only local-path deps")]
    UnsupportedSource {
        /// Package name.
        pkg: String,
    },
}
