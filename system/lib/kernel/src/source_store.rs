//! Executable source-store view for `/bin` and `/payload` admission.
//!
//! The current OS image links source artifacts into static slices. Exec
//! admission should not depend on that representation directly: later block or
//! filesystem-backed executable loading can replace this store boundary while
//! keeping the same `/bin` and `/payload` admission paths.

use {
    crate::{
        program::{ProgramImageKind, ProgramSourceArtifact},
        rootd::{PayloadImageKind, PayloadSourceArtifact},
    },
    core::{
        cell::UnsafeCell,
        slice,
        sync::atomic::{AtomicBool, Ordering},
    },
};

/// Maximum source artifacts rendered in one diagnostic snapshot.
pub const MAX_SOURCE_ARTIFACT_RECORDS: usize = 32;
/// Maximum installed executable source artifacts retained outside image tables.
pub const MAX_INSTALLED_SOURCE_ARTIFACTS: usize = 4;
/// Maximum bytes in one installed executable source artifact.
pub const MAX_INSTALLED_SOURCE_BYTES: usize = 256;
/// Maximum bytes in one block-backed source-media artifact envelope.
pub const MAX_SOURCE_MEDIA_ARTIFACT_BYTES: usize = MAX_INSTALLED_SOURCE_BYTES + 256;
/// Maximum bytes in one block-backed source-media catalog.
pub const MAX_SOURCE_MEDIA_CATALOG_BYTES: usize = 512;
/// Maximum source-media catalog entries rendered in one diagnostic snapshot.
pub const MAX_SOURCE_MEDIA_CATALOG_RECORDS: usize = 8;

const SOURCE_MEDIA_MAGIC: &[u8] = b"reovim-source-media-v1";
const SOURCE_MEDIA_NAMESPACE_PREFIX: &[u8] = b"namespace=";
const SOURCE_MEDIA_PATH_PREFIX: &[u8] = b"path=";
const SOURCE_MEDIA_BYTES_PREFIX: &[u8] = b"bytes=";
const SOURCE_MEDIA_CHECKSUM_PREFIX: &[u8] = b"checksum=";
const SOURCE_MEDIA_CATALOG_MAGIC: &[u8] = b"reovim-source-media-catalog-v1";
const SOURCE_MEDIA_ENTRY_PREFIX: &[u8] = b"entry ";
const SOURCE_MEDIA_ENTRY_NAMESPACE_PREFIX: &[u8] = b"namespace=";
const SOURCE_MEDIA_ENTRY_PATH_PREFIX: &[u8] = b"path=";
const SOURCE_MEDIA_ENTRY_OFFSET_PREFIX: &[u8] = b"offset=";
const SOURCE_MEDIA_ENTRY_BYTES_PREFIX: &[u8] = b"bytes=";
const SOURCE_MEDIA_ENTRY_CHECKSUM_PREFIX: &[u8] = b"checksum=";

/// Executable source artifact namespace.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceArtifactNamespace {
    /// `/bin` program source artifact.
    Bin,
    /// `/payload` source artifact.
    Payload,
}

impl SourceArtifactNamespace {
    /// Stable diagnostic word.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Bin => "bin",
            Self::Payload => "payload",
        }
    }
}

/// Origin of one executable source artifact.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceArtifactOrigin {
    /// Source bytes are linked into the boot image's source table.
    Image,
    /// Source bytes were installed into the kernel source overlay at runtime.
    Installed,
}

impl SourceArtifactOrigin {
    /// Stable diagnostic word.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Installed => "installed",
        }
    }
}

/// One executable source artifact visible to admission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceArtifactRecord {
    /// Artifact namespace.
    pub namespace: SourceArtifactNamespace,
    /// Loader-visible artifact path.
    pub path: &'static str,
    /// Loader/source kind.
    pub loader: &'static str,
    /// Artifact byte length.
    pub bytes_len: usize,
    /// Where this source artifact came from.
    pub origin: SourceArtifactOrigin,
}

impl SourceArtifactRecord {
    /// Empty source artifact record used for bounded snapshots.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            namespace: SourceArtifactNamespace::Bin,
            path: "",
            loader: "",
            bytes_len: 0,
            origin: SourceArtifactOrigin::Image,
        }
    }
}

/// Empty source artifact record used for bounded snapshots.
pub const EMPTY_SOURCE_ARTIFACT_RECORD: SourceArtifactRecord = SourceArtifactRecord::empty();

/// Error while installing a runtime executable source artifact.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceInstallError {
    /// The source path was empty.
    EmptyPath,
    /// The source artifact exceeded [`MAX_INSTALLED_SOURCE_BYTES`].
    TooLarge,
    /// No installed-source slot was available.
    NoSlot,
}

/// Error while parsing a block-backed source-media artifact.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceMediaArtifactError {
    /// The artifact did not start with the source-media magic header.
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
    /// The source payload is larger than the runtime overlay admits.
    SourceTooLarge,
}

impl SourceMediaArtifactError {
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

/// Error while parsing a block-backed source-media catalog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceMediaCatalogError {
    /// The catalog did not start with the catalog magic header.
    MissingMagic,
    /// The catalog body byte-count row was missing or malformed.
    MissingBytes,
    /// The catalog checksum row was missing or malformed.
    MissingChecksum,
    /// The declared body length does not match the available catalog bytes.
    BodyLengthMismatch,
    /// The declared checksum does not match the catalog body.
    ChecksumMismatch,
    /// No entry matched the requested namespace and path.
    NotFound,
    /// A catalog entry was malformed.
    InvalidEntry,
    /// A matching entry path was not UTF-8 text.
    InvalidPath,
}

impl SourceMediaCatalogError {
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

/// One checked source-media catalog entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceMediaCatalogEntry<'a> {
    /// Source artifact namespace declared by the catalog entry.
    pub namespace: SourceArtifactNamespace,
    /// Loader-visible artifact path declared by the catalog entry.
    pub path: &'a str,
    /// Byte offset where the checked source-media artifact envelope begins.
    pub offset: usize,
    /// Exact checked source-media artifact envelope byte length.
    pub artifact_bytes_len: usize,
    /// Verified checksum over the checked source-media artifact envelope bytes.
    pub checksum: u32,
}

/// Empty source-media catalog entry used for bounded snapshots.
pub const EMPTY_SOURCE_MEDIA_CATALOG_ENTRY: SourceMediaCatalogEntry<'static> =
    SourceMediaCatalogEntry {
        namespace: SourceArtifactNamespace::Bin,
        path: "",
        offset: 0,
        artifact_bytes_len: 0,
        checksum: 0,
    };

/// Parsed block-backed source-media artifact.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceMediaArtifact<'a> {
    /// Source artifact namespace declared by the media bytes.
    pub namespace: SourceArtifactNamespace,
    /// Loader-visible artifact path declared by the media bytes.
    pub path: &'a str,
    /// Executable source payload bytes.
    pub source_bytes: &'a [u8],
    /// Verified checksum over [`Self::source_bytes`].
    pub checksum: u32,
}

/// Computes the source-media artifact checksum.
///
/// This is FNV-1a over the executable source payload. The checksum is an
/// admission guard for bounded source-media fixtures, not a cryptographic
/// authenticity primitive.
#[must_use]
pub fn source_media_checksum32(bytes: &[u8]) -> u32 {
    let mut hash = 0x811c9dc5u32;
    let mut index = 0usize;
    while index < bytes.len() {
        hash ^= bytes[index] as u32;
        hash = hash.wrapping_mul(16_777_619);
        index += 1;
    }
    hash
}

/// Parses a checked block-backed source-media artifact envelope.
pub fn parse_source_media_artifact(
    bytes: &[u8],
) -> Result<SourceMediaArtifact<'_>, SourceMediaArtifactError> {
    let (magic, offset) =
        next_source_media_line(bytes, 0).ok_or(SourceMediaArtifactError::MissingMagic)?;
    if magic != SOURCE_MEDIA_MAGIC {
        return Err(SourceMediaArtifactError::MissingMagic);
    }

    let (namespace_bytes, offset) = parse_source_media_row(
        bytes,
        offset,
        SOURCE_MEDIA_NAMESPACE_PREFIX,
        SourceMediaArtifactError::MissingNamespace,
    )?;
    let namespace = match namespace_bytes {
        b"bin" => SourceArtifactNamespace::Bin,
        b"payload" => SourceArtifactNamespace::Payload,
        _ => return Err(SourceMediaArtifactError::InvalidNamespace),
    };

    let (path_bytes, offset) = parse_source_media_row(
        bytes,
        offset,
        SOURCE_MEDIA_PATH_PREFIX,
        SourceMediaArtifactError::MissingPath,
    )?;
    let path = match core::str::from_utf8(path_bytes) {
        Ok(path) if !path.is_empty() => path,
        Ok(_) => return Err(SourceMediaArtifactError::MissingPath),
        Err(_) => return Err(SourceMediaArtifactError::InvalidPath),
    };

    let (declared_len_bytes, offset) = parse_source_media_row(
        bytes,
        offset,
        SOURCE_MEDIA_BYTES_PREFIX,
        SourceMediaArtifactError::MissingBytes,
    )?;
    let declared_len = parse_source_media_usize(declared_len_bytes)
        .ok_or(SourceMediaArtifactError::MissingBytes)?;

    let (checksum_bytes, offset) = parse_source_media_row(
        bytes,
        offset,
        SOURCE_MEDIA_CHECKSUM_PREFIX,
        SourceMediaArtifactError::MissingChecksum,
    )?;
    let checksum =
        parse_source_media_u32(checksum_bytes).ok_or(SourceMediaArtifactError::MissingChecksum)?;

    let body = &bytes[offset..];
    if body.len() != declared_len {
        return Err(SourceMediaArtifactError::BodyLengthMismatch);
    }
    if body.len() > MAX_INSTALLED_SOURCE_BYTES {
        return Err(SourceMediaArtifactError::SourceTooLarge);
    }
    if source_media_checksum32(body) != checksum {
        return Err(SourceMediaArtifactError::ChecksumMismatch);
    }

    Ok(SourceMediaArtifact {
        namespace,
        path,
        source_bytes: body,
        checksum,
    })
}

/// Finds a checked source-media catalog entry for `namespace` and `path`.
pub fn find_source_media_catalog_entry<'a>(
    bytes: &'a [u8],
    namespace: SourceArtifactNamespace,
    path: &str,
) -> Result<SourceMediaCatalogEntry<'a>, SourceMediaCatalogError> {
    let body = parse_source_media_catalog_body(bytes)?;
    let mut offset = 0usize;
    while offset < body.len() {
        let Some((line, next)) = next_source_media_line(body, offset) else {
            break;
        };
        offset = next;
        if line.is_empty() {
            continue;
        }
        if line.len() < SOURCE_MEDIA_ENTRY_PREFIX.len()
            || &line[..SOURCE_MEDIA_ENTRY_PREFIX.len()] != SOURCE_MEDIA_ENTRY_PREFIX
        {
            return Err(SourceMediaCatalogError::InvalidEntry);
        }
        let entry = parse_source_media_catalog_entry(&line[SOURCE_MEDIA_ENTRY_PREFIX.len()..])?;
        if entry.namespace == namespace && entry.path == path {
            return Ok(entry);
        }
    }
    Err(SourceMediaCatalogError::NotFound)
}

/// Snapshots checked source-media catalog entries into `out`.
///
/// The returned boolean reports whether additional valid entries existed after
/// the bounded output buffer filled. Malformed entries still fail the snapshot
/// even after truncation so diagnostics do not hide a corrupt manifest tail.
pub fn snapshot_source_media_catalog_entries<'a>(
    bytes: &'a [u8],
    out: &mut [SourceMediaCatalogEntry<'a>],
) -> Result<(usize, bool), SourceMediaCatalogError> {
    let body = parse_source_media_catalog_body(bytes)?;
    let mut offset = 0usize;
    let mut written = 0usize;
    let mut truncated = false;
    while offset < body.len() {
        let Some((line, next)) = next_source_media_line(body, offset) else {
            break;
        };
        offset = next;
        if line.is_empty() {
            continue;
        }
        if line.len() < SOURCE_MEDIA_ENTRY_PREFIX.len()
            || &line[..SOURCE_MEDIA_ENTRY_PREFIX.len()] != SOURCE_MEDIA_ENTRY_PREFIX
        {
            return Err(SourceMediaCatalogError::InvalidEntry);
        }
        let entry = parse_source_media_catalog_entry(&line[SOURCE_MEDIA_ENTRY_PREFIX.len()..])?;
        if written < out.len() {
            out[written] = entry;
            written += 1;
        } else {
            truncated = true;
        }
    }
    Ok((written, truncated))
}

fn parse_source_media_catalog_body(bytes: &[u8]) -> Result<&[u8], SourceMediaCatalogError> {
    let (magic, offset) =
        next_source_media_line(bytes, 0).ok_or(SourceMediaCatalogError::MissingMagic)?;
    if magic != SOURCE_MEDIA_CATALOG_MAGIC {
        return Err(SourceMediaCatalogError::MissingMagic);
    }

    let (declared_len_bytes, offset) = parse_source_media_catalog_row(
        bytes,
        offset,
        SOURCE_MEDIA_BYTES_PREFIX,
        SourceMediaCatalogError::MissingBytes,
    )?;
    let declared_len = parse_source_media_usize(declared_len_bytes)
        .ok_or(SourceMediaCatalogError::MissingBytes)?;

    let (checksum_bytes, offset) = parse_source_media_catalog_row(
        bytes,
        offset,
        SOURCE_MEDIA_CHECKSUM_PREFIX,
        SourceMediaCatalogError::MissingChecksum,
    )?;
    let checksum =
        parse_source_media_u32(checksum_bytes).ok_or(SourceMediaCatalogError::MissingChecksum)?;
    if bytes.len() < offset + declared_len {
        return Err(SourceMediaCatalogError::BodyLengthMismatch);
    }
    let body = &bytes[offset..offset + declared_len];
    if source_media_checksum32(body) != checksum {
        return Err(SourceMediaCatalogError::ChecksumMismatch);
    }
    Ok(body)
}

fn parse_source_media_catalog_row<'a>(
    bytes: &'a [u8],
    offset: usize,
    prefix: &[u8],
    error: SourceMediaCatalogError,
) -> Result<(&'a [u8], usize), SourceMediaCatalogError> {
    let (line, next) = next_source_media_line(bytes, offset).ok_or(error)?;
    if line.len() < prefix.len() || &line[..prefix.len()] != prefix {
        return Err(error);
    }
    Ok((&line[prefix.len()..], next))
}

fn parse_source_media_catalog_entry(
    line: &[u8],
) -> Result<SourceMediaCatalogEntry<'_>, SourceMediaCatalogError> {
    let (namespace_bytes, next) = parse_entry_field(
        line,
        0,
        SOURCE_MEDIA_ENTRY_NAMESPACE_PREFIX,
        SourceMediaCatalogError::InvalidEntry,
    )?;
    let namespace = match namespace_bytes {
        b"bin" => SourceArtifactNamespace::Bin,
        b"payload" => SourceArtifactNamespace::Payload,
        _ => return Err(SourceMediaCatalogError::InvalidEntry),
    };

    let (path_bytes, next) = parse_entry_field(
        line,
        next,
        SOURCE_MEDIA_ENTRY_PATH_PREFIX,
        SourceMediaCatalogError::InvalidEntry,
    )?;
    let path =
        core::str::from_utf8(path_bytes).map_err(|_| SourceMediaCatalogError::InvalidPath)?;
    if path.is_empty() {
        return Err(SourceMediaCatalogError::InvalidEntry);
    }

    let (offset_bytes, next) = parse_entry_field(
        line,
        next,
        SOURCE_MEDIA_ENTRY_OFFSET_PREFIX,
        SourceMediaCatalogError::InvalidEntry,
    )?;
    let offset =
        parse_source_media_usize(offset_bytes).ok_or(SourceMediaCatalogError::InvalidEntry)?;

    let (artifact_bytes, next) = parse_entry_field(
        line,
        next,
        SOURCE_MEDIA_ENTRY_BYTES_PREFIX,
        SourceMediaCatalogError::InvalidEntry,
    )?;
    let artifact_bytes_len =
        parse_source_media_usize(artifact_bytes).ok_or(SourceMediaCatalogError::InvalidEntry)?;

    let (checksum_bytes, next) = parse_entry_field(
        line,
        next,
        SOURCE_MEDIA_ENTRY_CHECKSUM_PREFIX,
        SourceMediaCatalogError::InvalidEntry,
    )?;
    if next != line.len() {
        return Err(SourceMediaCatalogError::InvalidEntry);
    }
    let checksum =
        parse_source_media_u32(checksum_bytes).ok_or(SourceMediaCatalogError::InvalidEntry)?;

    Ok(SourceMediaCatalogEntry {
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
    error: SourceMediaCatalogError,
) -> Result<(&'a [u8], usize), SourceMediaCatalogError> {
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

fn parse_source_media_row<'a>(
    bytes: &'a [u8],
    offset: usize,
    prefix: &[u8],
    error: SourceMediaArtifactError,
) -> Result<(&'a [u8], usize), SourceMediaArtifactError> {
    let (line, next) = next_source_media_line(bytes, offset).ok_or(error)?;
    if line.len() < prefix.len() || &line[..prefix.len()] != prefix {
        return Err(error);
    }
    Ok((&line[prefix.len()..], next))
}

fn next_source_media_line(bytes: &[u8], offset: usize) -> Option<(&[u8], usize)> {
    if offset >= bytes.len() {
        return None;
    }
    let mut index = offset;
    while index < bytes.len() {
        if bytes[index] == b'\n' {
            return Some((&bytes[offset..index], index + 1));
        }
        index += 1;
    }
    None
}

fn parse_source_media_usize(bytes: &[u8]) -> Option<usize> {
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
        value = value.checked_mul(10)?.checked_add((byte - b'0') as usize)?;
        index += 1;
    }
    Some(value)
}

fn parse_source_media_u32(bytes: &[u8]) -> Option<u32> {
    let value = parse_source_media_usize(bytes)?;
    if value > u32::MAX as usize {
        return None;
    }
    Some(value as u32)
}

impl SourceInstallError {
    /// Stable diagnostic word.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::EmptyPath => "empty-path",
            Self::TooLarge => "too-large",
            Self::NoSlot => "no-slot",
        }
    }
}

#[derive(Clone, Copy)]
struct InstalledSourceSlot {
    namespace: SourceArtifactNamespace,
    path: &'static str,
    bytes: [u8; MAX_INSTALLED_SOURCE_BYTES],
    len: usize,
    occupied: bool,
}

impl InstalledSourceSlot {
    const fn empty() -> Self {
        Self {
            namespace: SourceArtifactNamespace::Bin,
            path: "",
            bytes: [0u8; MAX_INSTALLED_SOURCE_BYTES],
            len: 0,
            occupied: false,
        }
    }

    fn write(&mut self, namespace: SourceArtifactNamespace, path: &'static str, bytes: &[u8]) {
        self.namespace = namespace;
        self.path = path;
        self.len = 0;
        while self.len < bytes.len() {
            self.bytes[self.len] = bytes[self.len];
            self.len += 1;
        }
        while self.len < self.bytes.len() {
            self.bytes[self.len] = 0;
            self.len += 1;
        }
        self.len = bytes.len();
        self.occupied = true;
    }

    fn clear(&mut self) {
        *self = Self::empty();
    }
}

struct InstalledSourceTable {
    slots: [InstalledSourceSlot; MAX_INSTALLED_SOURCE_ARTIFACTS],
}

impl InstalledSourceTable {
    const fn new() -> Self {
        Self {
            slots: [InstalledSourceSlot::empty(); MAX_INSTALLED_SOURCE_ARTIFACTS],
        }
    }

    fn reset(&mut self) {
        let mut index = 0usize;
        while index < self.slots.len() {
            self.slots[index].clear();
            index += 1;
        }
    }

    fn install(
        &mut self,
        namespace: SourceArtifactNamespace,
        path: &'static str,
        bytes: &[u8],
    ) -> Result<(), SourceInstallError> {
        if path.is_empty() {
            return Err(SourceInstallError::EmptyPath);
        }
        if bytes.len() > MAX_INSTALLED_SOURCE_BYTES {
            return Err(SourceInstallError::TooLarge);
        }

        let mut first_empty = None;
        let mut index = 0usize;
        while index < self.slots.len() {
            let slot = self.slots[index];
            if slot.occupied && slot.namespace == namespace && slot.path == path {
                self.slots[index].write(namespace, path, bytes);
                return Ok(());
            }
            if first_empty.is_none() && !slot.occupied {
                first_empty = Some(index);
            }
            index += 1;
        }

        let Some(slot) = first_empty else {
            return Err(SourceInstallError::NoSlot);
        };
        self.slots[slot].write(namespace, path, bytes);
        Ok(())
    }

    fn find(&self, namespace: SourceArtifactNamespace, path: &'static str) -> Option<usize> {
        let mut index = 0usize;
        while index < self.slots.len() {
            let slot = self.slots[index];
            if slot.occupied && slot.namespace == namespace && slot.path == path {
                return Some(index);
            }
            index += 1;
        }
        None
    }

    fn count_namespace(&self, namespace: SourceArtifactNamespace) -> usize {
        let mut count = 0usize;
        let mut index = 0usize;
        while index < self.slots.len() {
            let slot = self.slots[index];
            if slot.occupied && slot.namespace == namespace {
                count += 1;
            }
            index += 1;
        }
        count
    }

    fn snapshot(&self, out: &mut [SourceArtifactRecord]) -> usize {
        let mut written = 0usize;
        let mut index = 0usize;
        while index < self.slots.len() && written < out.len() {
            let slot = self.slots[index];
            if slot.occupied {
                out[written] = SourceArtifactRecord {
                    namespace: slot.namespace,
                    path: slot.path,
                    loader: "source-image",
                    bytes_len: slot.len,
                    origin: SourceArtifactOrigin::Installed,
                };
                written += 1;
            }
            index += 1;
        }
        written
    }
}

struct InstalledSourceCell(UnsafeCell<InstalledSourceTable>);

// SAFETY: mutable access is serialized by `INSTALLED_SOURCE_LOCK`.
unsafe impl Sync for InstalledSourceCell {}

static INSTALLED_SOURCES: InstalledSourceCell =
    InstalledSourceCell(UnsafeCell::new(InstalledSourceTable::new()));
static INSTALLED_SOURCE_LOCK: AtomicBool = AtomicBool::new(false);

struct InstalledSourceGuard;

impl InstalledSourceGuard {
    fn acquire() -> Self {
        while INSTALLED_SOURCE_LOCK
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }
        Self
    }
}

impl Drop for InstalledSourceGuard {
    fn drop(&mut self) {
        INSTALLED_SOURCE_LOCK.store(false, Ordering::Release);
    }
}

fn with_installed_sources<R>(f: impl FnOnce(&mut InstalledSourceTable) -> R) -> R {
    let _guard = InstalledSourceGuard::acquire();
    // SAFETY: `INSTALLED_SOURCE_LOCK` serializes access to the installed-source table.
    let sources = unsafe { &mut *INSTALLED_SOURCES.0.get() };
    f(sources)
}

fn installed_source_bytes(index: usize) -> &'static [u8] {
    // SAFETY: installed source storage is static for the process lifetime. The
    // returned slice remains valid after the lock is released; callers must not
    // reinstall the same path while a loaded executable from the old bytes is
    // still running.
    unsafe {
        let table = &*INSTALLED_SOURCES.0.get();
        let slot = &table.slots[index];
        slice::from_raw_parts(slot.bytes.as_ptr(), slot.len)
    }
}

/// Clears all runtime-installed executable source artifacts.
pub fn reset_installed_sources() {
    with_installed_sources(InstalledSourceTable::reset);
}

/// Installs or replaces one runtime executable source artifact.
///
/// The installed source overlay is checked before image-linked source tables.
/// This gives future block/fs loaders a bounded admission path without changing
/// `/bin` or `/payload` descriptors.
pub fn install_source(
    namespace: SourceArtifactNamespace,
    path: &'static str,
    bytes: &[u8],
) -> Result<(), SourceInstallError> {
    with_installed_sources(|sources| sources.install(namespace, path, bytes))
}

fn installed_source_index(namespace: SourceArtifactNamespace, path: &'static str) -> Option<usize> {
    with_installed_sources(|sources| sources.find(namespace, path))
}

fn installed_source_count(namespace: SourceArtifactNamespace) -> usize {
    with_installed_sources(|sources| sources.count_namespace(namespace))
}

fn installed_source_snapshot(out: &mut [SourceArtifactRecord]) -> usize {
    with_installed_sources(|sources| sources.snapshot(out))
}

/// Source artifacts visible to executable admission.
#[derive(Clone, Copy, Debug)]
pub struct ExecutableSourceStore {
    program_sources: &'static [ProgramSourceArtifact],
    payload_sources: &'static [PayloadSourceArtifact],
}

impl ExecutableSourceStore {
    /// Builds a source-store view over current image-linked source artifacts.
    #[must_use]
    pub const fn new(
        program_sources: &'static [ProgramSourceArtifact],
        payload_sources: &'static [PayloadSourceArtifact],
    ) -> Self {
        Self {
            program_sources,
            payload_sources,
        }
    }

    /// Builds a source-store view containing only `/bin` program artifacts.
    #[must_use]
    pub const fn program_only(program_sources: &'static [ProgramSourceArtifact]) -> Self {
        Self::new(program_sources, &[])
    }

    /// Builds a source-store view containing only payload artifacts.
    #[must_use]
    pub const fn payload_only(payload_sources: &'static [PayloadSourceArtifact]) -> Self {
        Self::new(&[], payload_sources)
    }

    /// Returns the `/bin` artifact count.
    #[must_use]
    pub fn program_count(self) -> usize {
        let mut count = installed_source_count(SourceArtifactNamespace::Bin);
        let mut index = 0usize;
        while index < self.program_sources.len() {
            if installed_source_index(
                SourceArtifactNamespace::Bin,
                self.program_sources[index].path,
            )
            .is_none()
            {
                count += 1;
            }
            index += 1;
        }
        count
    }

    /// Returns the `/payload` artifact count.
    #[must_use]
    pub fn payload_count(self) -> usize {
        let mut count = installed_source_count(SourceArtifactNamespace::Payload);
        let mut index = 0usize;
        while index < self.payload_sources.len() {
            if installed_source_index(
                SourceArtifactNamespace::Payload,
                self.payload_sources[index].path,
            )
            .is_none()
            {
                count += 1;
            }
            index += 1;
        }
        count
    }

    /// Finds a `/bin` source artifact by loader-visible path.
    #[must_use]
    pub fn find_program(self, path: &'static str) -> Option<ProgramSourceArtifact> {
        if let Some(index) = installed_source_index(SourceArtifactNamespace::Bin, path) {
            return Some(ProgramSourceArtifact {
                path,
                kind: ProgramImageKind::SourceImage,
                bytes: installed_source_bytes(index),
            });
        }
        let mut index = 0usize;
        while index < self.program_sources.len() {
            if self.program_sources[index].path == path {
                return Some(self.program_sources[index]);
            }
            index += 1;
        }
        None
    }

    /// Finds a payload source artifact by loader-visible path.
    #[must_use]
    pub fn find_payload(self, path: &'static str) -> Option<PayloadSourceArtifact> {
        if let Some(index) = installed_source_index(SourceArtifactNamespace::Payload, path) {
            return Some(PayloadSourceArtifact {
                path,
                kind: PayloadImageKind::SourceImage,
                bytes: installed_source_bytes(index),
            });
        }
        let mut index = 0usize;
        while index < self.payload_sources.len() {
            if self.payload_sources[index].path == path {
                return Some(self.payload_sources[index]);
            }
            index += 1;
        }
        None
    }

    /// Copies source artifact records into `out`, returning the count copied.
    pub fn snapshot(self, out: &mut [SourceArtifactRecord]) -> usize {
        let mut written = installed_source_snapshot(out);
        let mut index = 0usize;
        while index < self.program_sources.len() && written < out.len() {
            let source = self.program_sources[index];
            if installed_source_index(SourceArtifactNamespace::Bin, source.path).is_some() {
                index += 1;
                continue;
            }
            out[written] = SourceArtifactRecord {
                namespace: SourceArtifactNamespace::Bin,
                path: source.path,
                loader: source.kind.as_str(),
                bytes_len: source.bytes.len(),
                origin: SourceArtifactOrigin::Image,
            };
            written += 1;
            index += 1;
        }

        index = 0;
        while index < self.payload_sources.len() && written < out.len() {
            let source = self.payload_sources[index];
            if installed_source_index(SourceArtifactNamespace::Payload, source.path).is_some() {
                index += 1;
                continue;
            }
            out[written] = SourceArtifactRecord {
                namespace: SourceArtifactNamespace::Payload,
                path: source.path,
                loader: source.kind.as_str(),
                bytes_len: source.bytes.len(),
                origin: SourceArtifactOrigin::Image,
            };
            written += 1;
            index += 1;
        }
        written
    }
}

#[cfg(feature = "selftest")]
#[path = "source_store_tests.rs"]
mod tests;
