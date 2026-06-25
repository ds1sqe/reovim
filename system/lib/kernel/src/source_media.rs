//! Checked executable source-media service.
//!
//! This module is the current block-backed artifact lookup boundary for
//! executable admission. The block service owns target IO, `source_store` owns
//! executable source artifact syntax, and this service owns catalog/single-
//! artifact selection plus checksum/path validation.

use crate::{
    block::{self, BlockIoResult},
    source_store::{
        self, MAX_SOURCE_MEDIA_ARTIFACT_BYTES, MAX_SOURCE_MEDIA_CATALOG_BYTES,
        SourceArtifactNamespace, SourceMediaArtifact, SourceMediaArtifactError,
        SourceMediaCatalogEntry, SourceMediaCatalogError,
    },
};

/// Root source-media snapshot result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceMediaSnapshot<'a> {
    /// No source-media target is installed.
    Unavailable { read: BlockIoResult },
    /// The source-media target returned a failed or empty root read.
    ReadError { read: BlockIoResult },
    /// Offset zero contains a checked catalog.
    Catalog {
        read: BlockIoResult,
        count: usize,
        truncated: bool,
    },
    /// Offset zero contains one checked source artifact.
    Artifact {
        read: BlockIoResult,
        artifact: SourceMediaArtifact<'a>,
    },
    /// Offset zero looked like a catalog but failed validation.
    InvalidCatalog {
        read: BlockIoResult,
        error: SourceMediaCatalogError,
    },
    /// Offset zero was not a catalog and failed single-artifact validation.
    InvalidArtifact {
        read: BlockIoResult,
        error: SourceMediaArtifactError,
    },
}

/// How a checked source-media artifact was selected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceMediaArtifactFormat {
    /// Offset zero was the artifact envelope.
    SingleArtifact,
    /// Offset zero was a catalog entry pointing at the artifact envelope.
    Catalog,
}

impl SourceMediaArtifactFormat {
    /// Stable diagnostic word.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SingleArtifact => "artifact",
            Self::Catalog => "catalog",
        }
    }
}

/// Error while reading a checked source-media artifact.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceMediaReadError {
    /// No source-media target is installed.
    Unavailable,
    /// A block read failed or returned too few bytes.
    ReadFailed,
    /// The catalog or artifact failed structural/checksum validation.
    Invalid,
    /// The artifact namespace did not match the requested namespace.
    NamespaceMismatch,
    /// No catalog entry or single artifact matched the requested path.
    PathMismatch,
}

impl SourceMediaReadError {
    /// Stable diagnostic word.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unavailable => "source-media-unavailable",
            Self::ReadFailed => "source-media-read-failed",
            Self::Invalid => "source-media-invalid",
            Self::NamespaceMismatch => "source-media-namespace-mismatch",
            Self::PathMismatch => "source-media-path-mismatch",
        }
    }
}

/// Checked source-media artifact read from the block target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceMediaArtifactRead<'a> {
    /// Block IO result for the artifact bytes.
    pub read: BlockIoResult,
    /// Whether the artifact came directly from offset zero or through a catalog.
    pub format: SourceMediaArtifactFormat,
    /// Exact artifact envelope length validated by the lookup.
    pub artifact_bytes_len: usize,
    /// Parsed checked artifact.
    pub artifact: SourceMediaArtifact<'a>,
}

/// Snapshots the source-media root object at offset zero.
///
/// Catalog entry paths stored in `entries` borrow from `root`, so callers must
/// keep both buffers live while rendering the snapshot.
pub fn snapshot_root<'a>(
    root: &'a mut [u8; MAX_SOURCE_MEDIA_CATALOG_BYTES],
    entries: &mut [SourceMediaCatalogEntry<'a>],
) -> SourceMediaSnapshot<'a> {
    let read = block::read_source_media_artifact(root);
    if !read.available {
        return SourceMediaSnapshot::Unavailable { read };
    }
    if !read.ok || read.bytes == 0 {
        return SourceMediaSnapshot::ReadError { read };
    }

    let root_bytes = &root[..read.bytes];
    match source_store::snapshot_source_media_catalog_entries(root_bytes, entries) {
        Ok((count, truncated)) => SourceMediaSnapshot::Catalog {
            read,
            count,
            truncated,
        },
        Err(SourceMediaCatalogError::MissingMagic) => {
            match source_store::parse_source_media_artifact(root_bytes) {
                Ok(artifact) => SourceMediaSnapshot::Artifact { read, artifact },
                Err(error) => SourceMediaSnapshot::InvalidArtifact { read, error },
            }
        }
        Err(error) => SourceMediaSnapshot::InvalidCatalog { read, error },
    }
}

/// Reads and validates one source-media artifact by namespace and path.
pub fn read_checked_artifact<'a>(
    namespace: SourceArtifactNamespace,
    expected_path: &str,
    out: &'a mut [u8; MAX_SOURCE_MEDIA_ARTIFACT_BYTES],
) -> Result<SourceMediaArtifactRead<'a>, SourceMediaReadError> {
    let mut catalog = [0u8; MAX_SOURCE_MEDIA_CATALOG_BYTES];
    let read = block::read_source_media_artifact(&mut catalog);
    if !read.available {
        return Err(SourceMediaReadError::Unavailable);
    }
    if !read.ok || read.bytes == 0 {
        return Err(SourceMediaReadError::ReadFailed);
    }

    match source_store::find_source_media_catalog_entry(
        &catalog[..read.bytes],
        namespace,
        expected_path,
    ) {
        Ok(entry) => read_catalog_artifact(namespace, expected_path, entry, out),
        Err(SourceMediaCatalogError::MissingMagic) => {
            let read = block::read_source_media_artifact(out);
            if !read.available {
                return Err(SourceMediaReadError::Unavailable);
            }
            if !read.ok || read.bytes == 0 {
                return Err(SourceMediaReadError::ReadFailed);
            }
            parse_expected_artifact(
                namespace,
                expected_path,
                &out[..read.bytes],
                read,
                SourceMediaArtifactFormat::SingleArtifact,
                read.bytes,
            )
        }
        Err(SourceMediaCatalogError::NotFound) => Err(SourceMediaReadError::PathMismatch),
        Err(_) => Err(SourceMediaReadError::Invalid),
    }
}

fn read_catalog_artifact<'a>(
    namespace: SourceArtifactNamespace,
    expected_path: &str,
    entry: SourceMediaCatalogEntry<'_>,
    out: &'a mut [u8; MAX_SOURCE_MEDIA_ARTIFACT_BYTES],
) -> Result<SourceMediaArtifactRead<'a>, SourceMediaReadError> {
    if entry.artifact_bytes_len > MAX_SOURCE_MEDIA_ARTIFACT_BYTES {
        return Err(SourceMediaReadError::Invalid);
    }
    let read = block::read_source_media_artifact_at(entry.offset, out);
    if !read.available {
        return Err(SourceMediaReadError::Unavailable);
    }
    if !read.ok || read.bytes == 0 {
        return Err(SourceMediaReadError::ReadFailed);
    }
    if read.bytes < entry.artifact_bytes_len {
        return Err(SourceMediaReadError::ReadFailed);
    }
    let artifact_bytes = &out[..entry.artifact_bytes_len];
    if source_store::source_media_checksum32(artifact_bytes) != entry.checksum {
        return Err(SourceMediaReadError::Invalid);
    }
    parse_expected_artifact(
        namespace,
        expected_path,
        artifact_bytes,
        read,
        SourceMediaArtifactFormat::Catalog,
        entry.artifact_bytes_len,
    )
}

fn parse_expected_artifact<'a>(
    namespace: SourceArtifactNamespace,
    expected_path: &str,
    artifact_bytes: &'a [u8],
    read: BlockIoResult,
    format: SourceMediaArtifactFormat,
    artifact_bytes_len: usize,
) -> Result<SourceMediaArtifactRead<'a>, SourceMediaReadError> {
    let artifact = source_store::parse_source_media_artifact(artifact_bytes)
        .map_err(|_| SourceMediaReadError::Invalid)?;
    if artifact.namespace != namespace {
        return Err(SourceMediaReadError::NamespaceMismatch);
    }
    if artifact.path != expected_path {
        return Err(SourceMediaReadError::PathMismatch);
    }
    Ok(SourceMediaArtifactRead {
        read,
        format,
        artifact_bytes_len,
        artifact,
    })
}

#[cfg(feature = "selftest")]
#[path = "source_media_tests.rs"]
mod tests;
