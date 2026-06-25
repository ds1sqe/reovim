//! Checked block-backed executable bundle provider.
//!
//! This is the first provider beyond the source-media bridge. It reads a
//! checked executable catalog or artifact envelope from an explicit block target
//! and leaves filesystem enumeration to later cuts.

use crate::{
    block::{self, BlockIoResult},
    source_store::{
        self, MAX_SOURCE_MEDIA_ARTIFACT_BYTES, MAX_SOURCE_MEDIA_CATALOG_BYTES,
        SourceArtifactNamespace,
    },
};

const EXEC_BUNDLE_MAGIC: &[u8] = b"reovim-exec-bundle-v1";
const EXEC_BUNDLE_CATALOG_MAGIC: &[u8] = b"reovim-exec-bundle-catalog-v1";
const EXEC_BUNDLE_NAMESPACE_PREFIX: &[u8] = b"namespace=";
const EXEC_BUNDLE_PATH_PREFIX: &[u8] = b"path=";
const EXEC_BUNDLE_BYTES_PREFIX: &[u8] = b"bytes=";
const EXEC_BUNDLE_CHECKSUM_PREFIX: &[u8] = b"checksum=";
const EXEC_BUNDLE_ENTRY_PREFIX: &[u8] = b"entry ";
const EXEC_BUNDLE_ENTRY_NAMESPACE_PREFIX: &[u8] = b"namespace=";
const EXEC_BUNDLE_ENTRY_PATH_PREFIX: &[u8] = b"path=";
const EXEC_BUNDLE_ENTRY_OFFSET_PREFIX: &[u8] = b"offset=";
const EXEC_BUNDLE_ENTRY_BYTES_PREFIX: &[u8] = b"bytes=";
const EXEC_BUNDLE_ENTRY_CHECKSUM_PREFIX: &[u8] = b"checksum=";

/// Error while parsing a block-backed executable bundle artifact.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecBundleArtifactError {
    /// The artifact did not start with the executable-bundle magic header.
    MissingMagic,
    /// The namespace row was missing or malformed.
    MissingNamespace,
    /// The namespace value is not a supported executable artifact namespace.
    InvalidNamespace,
    /// The path row was missing or malformed.
    MissingPath,
    /// The path row was not UTF-8 text.
    InvalidPath,
    /// The payload byte-count row was missing or malformed.
    MissingBytes,
    /// The checksum row was missing or malformed.
    MissingChecksum,
    /// The declared payload length does not match the artifact body.
    BodyLengthMismatch,
    /// The declared checksum does not match the artifact body.
    ChecksumMismatch,
    /// The source payload is larger than the runtime executable cache admits.
    SourceTooLarge,
}

impl ExecBundleArtifactError {
    /// Stable diagnostic word.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MissingMagic => "missing-magic",
            Self::MissingNamespace => "missing-namespace",
            Self::InvalidNamespace => "invalid-namespace",
            Self::MissingPath => "missing-path",
            Self::InvalidPath => "invalid-path",
            Self::MissingBytes => "missing-bytes",
            Self::MissingChecksum => "missing-checksum",
            Self::BodyLengthMismatch => "body-length-mismatch",
            Self::ChecksumMismatch => "checksum-mismatch",
            Self::SourceTooLarge => "source-too-large",
        }
    }
}

/// Parsed executable bundle artifact.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExecBundleArtifact<'a> {
    /// Artifact namespace declared by the block bundle.
    pub namespace: SourceArtifactNamespace,
    /// Loader-visible artifact path declared by the block bundle.
    pub path: &'a str,
    /// Checked executable source bytes.
    pub source_bytes: &'a [u8],
    /// Verified checksum over source bytes.
    pub checksum: u32,
}

/// Error while parsing an executable bundle catalog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecBundleCatalogError {
    /// The catalog did not start with the executable-bundle catalog magic.
    MissingMagic,
    /// The body byte-count row was missing or malformed.
    MissingBytes,
    /// The checksum row was missing or malformed.
    MissingChecksum,
    /// The declared catalog body length does not fit the bytes read.
    BodyLengthMismatch,
    /// The declared checksum does not match the catalog body.
    ChecksumMismatch,
    /// No matching entry exists for the requested namespace/path.
    NotFound,
    /// A catalog entry row was malformed.
    InvalidEntry,
    /// A catalog entry path was not UTF-8 text.
    InvalidPath,
}

impl ExecBundleCatalogError {
    /// Stable diagnostic word.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MissingMagic => "missing-magic",
            Self::MissingBytes => "missing-bytes",
            Self::MissingChecksum => "missing-checksum",
            Self::BodyLengthMismatch => "body-length-mismatch",
            Self::ChecksumMismatch => "checksum-mismatch",
            Self::NotFound => "not-found",
            Self::InvalidEntry => "invalid-entry",
            Self::InvalidPath => "invalid-path",
        }
    }
}

/// One checked executable bundle catalog entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExecBundleCatalogEntry<'a> {
    /// Executable artifact namespace declared by the catalog entry.
    pub namespace: SourceArtifactNamespace,
    /// Loader-visible artifact path declared by the catalog entry.
    pub path: &'a str,
    /// Byte offset where the checked executable artifact envelope begins.
    pub offset: usize,
    /// Exact checked executable artifact envelope byte length.
    pub artifact_bytes_len: usize,
    /// Verified checksum over the checked executable artifact envelope bytes.
    pub checksum: u32,
}

/// How a checked executable bundle artifact was selected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecBundleArtifactFormat {
    /// Offset zero was the artifact envelope.
    SingleArtifact,
    /// Offset zero was a catalog entry pointing at the artifact envelope.
    Catalog,
}

impl ExecBundleArtifactFormat {
    /// Stable diagnostic word.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SingleArtifact => "artifact",
            Self::Catalog => "catalog",
        }
    }
}

/// Error while reading a checked executable bundle artifact.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecBundleReadError {
    /// No executable-bundle target is installed.
    Unavailable,
    /// A block read failed or returned no bytes.
    ReadFailed,
    /// The artifact failed structural/checksum validation.
    Invalid,
    /// The artifact namespace did not match the requested namespace.
    NamespaceMismatch,
    /// The artifact path did not match the requested path.
    PathMismatch,
}

/// Checked executable bundle artifact read from the block target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExecBundleArtifactRead<'a> {
    /// Block IO result for the artifact bytes.
    pub read: BlockIoResult,
    /// Whether the artifact came directly from offset zero or through a catalog.
    pub format: ExecBundleArtifactFormat,
    /// Exact artifact envelope length validated by the lookup.
    pub artifact_bytes_len: usize,
    /// Parsed checked artifact.
    pub artifact: ExecBundleArtifact<'a>,
}

/// Reads and validates one executable bundle artifact by namespace and path.
pub fn read_checked_artifact<'a>(
    namespace: SourceArtifactNamespace,
    expected_path: &str,
    out: &'a mut [u8; MAX_SOURCE_MEDIA_ARTIFACT_BYTES],
) -> Result<ExecBundleArtifactRead<'a>, ExecBundleReadError> {
    let mut catalog = [0u8; MAX_SOURCE_MEDIA_CATALOG_BYTES];
    let read = block::read_exec_bundle_artifact(&mut catalog);
    if !read.available {
        return Err(ExecBundleReadError::Unavailable);
    }
    if !read.ok || read.bytes == 0 {
        return Err(ExecBundleReadError::ReadFailed);
    }
    match find_exec_bundle_catalog_entry(&catalog[..read.bytes], namespace, expected_path) {
        Ok(entry) => read_catalog_artifact(namespace, expected_path, entry, out),
        Err(ExecBundleCatalogError::MissingMagic) => {
            let read = block::read_exec_bundle_artifact(out);
            if !read.available {
                return Err(ExecBundleReadError::Unavailable);
            }
            if !read.ok || read.bytes == 0 {
                return Err(ExecBundleReadError::ReadFailed);
            }
            parse_expected_artifact(
                namespace,
                expected_path,
                &out[..read.bytes],
                read,
                ExecBundleArtifactFormat::SingleArtifact,
                read.bytes,
            )
        }
        Err(ExecBundleCatalogError::NotFound) => Err(ExecBundleReadError::PathMismatch),
        Err(_) => Err(ExecBundleReadError::Invalid),
    }
}

fn read_catalog_artifact<'a>(
    namespace: SourceArtifactNamespace,
    expected_path: &str,
    entry: ExecBundleCatalogEntry<'_>,
    out: &'a mut [u8; MAX_SOURCE_MEDIA_ARTIFACT_BYTES],
) -> Result<ExecBundleArtifactRead<'a>, ExecBundleReadError> {
    if entry.artifact_bytes_len > MAX_SOURCE_MEDIA_ARTIFACT_BYTES {
        return Err(ExecBundleReadError::Invalid);
    }
    let read = block::read_exec_bundle_artifact_at(entry.offset, out);
    if !read.available {
        return Err(ExecBundleReadError::Unavailable);
    }
    if !read.ok || read.bytes == 0 {
        return Err(ExecBundleReadError::ReadFailed);
    }
    if read.bytes < entry.artifact_bytes_len {
        return Err(ExecBundleReadError::ReadFailed);
    }
    let artifact_bytes = &out[..entry.artifact_bytes_len];
    if source_store::source_media_checksum32(artifact_bytes) != entry.checksum {
        return Err(ExecBundleReadError::Invalid);
    }
    parse_expected_artifact(
        namespace,
        expected_path,
        artifact_bytes,
        read,
        ExecBundleArtifactFormat::Catalog,
        entry.artifact_bytes_len,
    )
}

fn parse_expected_artifact<'a>(
    namespace: SourceArtifactNamespace,
    expected_path: &str,
    artifact_bytes: &'a [u8],
    read: BlockIoResult,
    format: ExecBundleArtifactFormat,
    artifact_bytes_len: usize,
) -> Result<ExecBundleArtifactRead<'a>, ExecBundleReadError> {
    let artifact =
        parse_exec_bundle_artifact(artifact_bytes).map_err(|_| ExecBundleReadError::Invalid)?;
    if artifact.namespace != namespace {
        return Err(ExecBundleReadError::NamespaceMismatch);
    }
    if artifact.path != expected_path {
        return Err(ExecBundleReadError::PathMismatch);
    }
    Ok(ExecBundleArtifactRead {
        read,
        format,
        artifact_bytes_len,
        artifact,
    })
}

/// Parses a checked block-backed executable bundle artifact envelope.
pub fn parse_exec_bundle_artifact(
    bytes: &[u8],
) -> Result<ExecBundleArtifact<'_>, ExecBundleArtifactError> {
    let (magic, offset) = next_line(bytes, 0).ok_or(ExecBundleArtifactError::MissingMagic)?;
    if magic != EXEC_BUNDLE_MAGIC {
        return Err(ExecBundleArtifactError::MissingMagic);
    }

    let (namespace_bytes, offset) = parse_row(
        bytes,
        offset,
        EXEC_BUNDLE_NAMESPACE_PREFIX,
        ExecBundleArtifactError::MissingNamespace,
    )?;
    let namespace = match namespace_bytes {
        b"bin" => SourceArtifactNamespace::Bin,
        b"payload" => SourceArtifactNamespace::Payload,
        _ => return Err(ExecBundleArtifactError::InvalidNamespace),
    };

    let (path_bytes, offset) =
        parse_row(bytes, offset, EXEC_BUNDLE_PATH_PREFIX, ExecBundleArtifactError::MissingPath)?;
    let path = match core::str::from_utf8(path_bytes) {
        Ok(path) if !path.is_empty() => path,
        Ok(_) => return Err(ExecBundleArtifactError::MissingPath),
        Err(_) => return Err(ExecBundleArtifactError::InvalidPath),
    };

    let (declared_len_bytes, offset) =
        parse_row(bytes, offset, EXEC_BUNDLE_BYTES_PREFIX, ExecBundleArtifactError::MissingBytes)?;
    let declared_len =
        parse_usize(declared_len_bytes).ok_or(ExecBundleArtifactError::MissingBytes)?;

    let (checksum_bytes, offset) = parse_row(
        bytes,
        offset,
        EXEC_BUNDLE_CHECKSUM_PREFIX,
        ExecBundleArtifactError::MissingChecksum,
    )?;
    let checksum = parse_u32(checksum_bytes).ok_or(ExecBundleArtifactError::MissingChecksum)?;

    let body = &bytes[offset..];
    if body.len() != declared_len {
        return Err(ExecBundleArtifactError::BodyLengthMismatch);
    }
    if body.len() > source_store::MAX_INSTALLED_SOURCE_BYTES {
        return Err(ExecBundleArtifactError::SourceTooLarge);
    }
    if source_store::source_media_checksum32(body) != checksum {
        return Err(ExecBundleArtifactError::ChecksumMismatch);
    }

    Ok(ExecBundleArtifact {
        namespace,
        path,
        source_bytes: body,
        checksum,
    })
}

/// Finds a checked executable bundle catalog entry for `namespace` and `path`.
pub fn find_exec_bundle_catalog_entry<'a>(
    bytes: &'a [u8],
    namespace: SourceArtifactNamespace,
    path: &str,
) -> Result<ExecBundleCatalogEntry<'a>, ExecBundleCatalogError> {
    let body = parse_exec_bundle_catalog_body(bytes)?;
    let mut offset = 0usize;
    while offset < body.len() {
        let Some((line, next)) = next_line(body, offset) else {
            break;
        };
        offset = next;
        if line.is_empty() {
            continue;
        }
        if line.len() < EXEC_BUNDLE_ENTRY_PREFIX.len()
            || &line[..EXEC_BUNDLE_ENTRY_PREFIX.len()] != EXEC_BUNDLE_ENTRY_PREFIX
        {
            return Err(ExecBundleCatalogError::InvalidEntry);
        }
        let entry = parse_exec_bundle_catalog_entry(&line[EXEC_BUNDLE_ENTRY_PREFIX.len()..])?;
        if entry.namespace == namespace && entry.path == path {
            return Ok(entry);
        }
    }
    Err(ExecBundleCatalogError::NotFound)
}

fn parse_exec_bundle_catalog_body(bytes: &[u8]) -> Result<&[u8], ExecBundleCatalogError> {
    let (magic, offset) = next_line(bytes, 0).ok_or(ExecBundleCatalogError::MissingMagic)?;
    if magic != EXEC_BUNDLE_CATALOG_MAGIC {
        return Err(ExecBundleCatalogError::MissingMagic);
    }

    let (declared_len_bytes, offset) = parse_catalog_row(
        bytes,
        offset,
        EXEC_BUNDLE_BYTES_PREFIX,
        ExecBundleCatalogError::MissingBytes,
    )?;
    let declared_len =
        parse_usize(declared_len_bytes).ok_or(ExecBundleCatalogError::MissingBytes)?;

    let (checksum_bytes, offset) = parse_catalog_row(
        bytes,
        offset,
        EXEC_BUNDLE_CHECKSUM_PREFIX,
        ExecBundleCatalogError::MissingChecksum,
    )?;
    let checksum = parse_u32(checksum_bytes).ok_or(ExecBundleCatalogError::MissingChecksum)?;
    if bytes.len() < offset + declared_len {
        return Err(ExecBundleCatalogError::BodyLengthMismatch);
    }
    let body = &bytes[offset..offset + declared_len];
    if source_store::source_media_checksum32(body) != checksum {
        return Err(ExecBundleCatalogError::ChecksumMismatch);
    }
    Ok(body)
}

fn parse_catalog_row<'a>(
    bytes: &'a [u8],
    offset: usize,
    prefix: &[u8],
    error: ExecBundleCatalogError,
) -> Result<(&'a [u8], usize), ExecBundleCatalogError> {
    let (line, next) = next_line(bytes, offset).ok_or(error)?;
    if line.len() < prefix.len() || &line[..prefix.len()] != prefix {
        return Err(error);
    }
    Ok((&line[prefix.len()..], next))
}

fn parse_exec_bundle_catalog_entry(
    line: &[u8],
) -> Result<ExecBundleCatalogEntry<'_>, ExecBundleCatalogError> {
    let (namespace_bytes, next) = parse_entry_field(
        line,
        0,
        EXEC_BUNDLE_ENTRY_NAMESPACE_PREFIX,
        ExecBundleCatalogError::InvalidEntry,
    )?;
    let namespace = match namespace_bytes {
        b"bin" => SourceArtifactNamespace::Bin,
        b"payload" => SourceArtifactNamespace::Payload,
        _ => return Err(ExecBundleCatalogError::InvalidEntry),
    };

    let (path_bytes, next) = parse_entry_field(
        line,
        next,
        EXEC_BUNDLE_ENTRY_PATH_PREFIX,
        ExecBundleCatalogError::InvalidEntry,
    )?;
    let path = core::str::from_utf8(path_bytes).map_err(|_| ExecBundleCatalogError::InvalidPath)?;
    if path.is_empty() {
        return Err(ExecBundleCatalogError::InvalidEntry);
    }

    let (offset_bytes, next) = parse_entry_field(
        line,
        next,
        EXEC_BUNDLE_ENTRY_OFFSET_PREFIX,
        ExecBundleCatalogError::InvalidEntry,
    )?;
    let offset = parse_usize(offset_bytes).ok_or(ExecBundleCatalogError::InvalidEntry)?;

    let (artifact_bytes, next) = parse_entry_field(
        line,
        next,
        EXEC_BUNDLE_ENTRY_BYTES_PREFIX,
        ExecBundleCatalogError::InvalidEntry,
    )?;
    let artifact_bytes_len =
        parse_usize(artifact_bytes).ok_or(ExecBundleCatalogError::InvalidEntry)?;

    let (checksum_bytes, next) = parse_entry_field(
        line,
        next,
        EXEC_BUNDLE_ENTRY_CHECKSUM_PREFIX,
        ExecBundleCatalogError::InvalidEntry,
    )?;
    if next != line.len() {
        return Err(ExecBundleCatalogError::InvalidEntry);
    }
    let checksum = parse_u32(checksum_bytes).ok_or(ExecBundleCatalogError::InvalidEntry)?;

    Ok(ExecBundleCatalogEntry {
        namespace,
        path,
        offset,
        artifact_bytes_len,
        checksum,
    })
}

fn parse_entry_field<'a>(
    line: &'a [u8],
    mut offset: usize,
    prefix: &[u8],
    error: ExecBundleCatalogError,
) -> Result<(&'a [u8], usize), ExecBundleCatalogError> {
    if offset > line.len() {
        return Err(error);
    }
    if offset > 0 {
        if offset >= line.len() || line[offset] != b' ' {
            return Err(error);
        }
        offset += 1;
    }
    if line.len() < offset + prefix.len() || &line[offset..offset + prefix.len()] != prefix {
        return Err(error);
    }
    let start = offset + prefix.len();
    let mut end = start;
    while end < line.len() && line[end] != b' ' {
        end += 1;
    }
    if start == end {
        return Err(error);
    }
    Ok((&line[start..end], end))
}

fn parse_row<'a>(
    bytes: &'a [u8],
    offset: usize,
    prefix: &[u8],
    error: ExecBundleArtifactError,
) -> Result<(&'a [u8], usize), ExecBundleArtifactError> {
    let (line, next) = next_line(bytes, offset).ok_or(error)?;
    if !line.starts_with(prefix) {
        return Err(error);
    }
    Ok((&line[prefix.len()..], next))
}

fn next_line(bytes: &[u8], offset: usize) -> Option<(&[u8], usize)> {
    if offset >= bytes.len() {
        return None;
    }

    let mut end = offset;
    while end < bytes.len() && bytes[end] != b'\n' {
        end += 1;
    }

    let mut next = end;
    if next < bytes.len() && bytes[next] == b'\n' {
        next += 1;
    }

    let mut line = &bytes[offset..end];
    if line.last() == Some(&b'\r') {
        line = &line[..line.len() - 1];
    }

    Some((line, next))
}

fn parse_usize(bytes: &[u8]) -> Option<usize> {
    if bytes.is_empty() {
        return None;
    }
    let mut value = 0usize;
    let mut index = 0usize;
    while index < bytes.len() {
        let byte = bytes[index];
        if !byte.is_ascii_digit() {
            return None;
        }
        value = value.checked_mul(10)?;
        value = value.checked_add((byte - b'0') as usize)?;
        index += 1;
    }
    Some(value)
}

fn parse_u32(bytes: &[u8]) -> Option<u32> {
    let value = parse_usize(bytes)?;
    if value > u32::MAX as usize {
        return None;
    }
    Some(value as u32)
}

#[cfg(feature = "selftest")]
#[path = "exec_bundle_tests.rs"]
mod tests;
