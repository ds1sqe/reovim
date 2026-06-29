//! Executable artifact resolver for exec admission.
//!
//! `exec` owns process admission and pending dispatch. This module owns the
//! executable-byte provider policy so later block/fs loaders can extend one
//! resolver without teaching exec about each storage shape.

use {
    crate::{
        block, exec_body, exec_bundle,
        program::{self, LoadedProgram, ProgramDescriptor, ProgramLoadError},
        rootd::{self, LoadedPayloadProgram, PayloadDescriptor, PayloadLoadError},
        source_media,
        source_store::{
            self, ExecutableSourceStore, MAX_SOURCE_MEDIA_ARTIFACT_BYTES,
            MAX_SOURCE_MEDIA_CATALOG_BYTES, SourceArtifactNamespace, SourceArtifactOrigin,
        },
    },
    core::{
        cell::UnsafeCell,
        slice,
        sync::atomic::{AtomicBool, Ordering},
    },
};

const MAX_EXEC_ARTIFACT_BODY_RECORDS: usize = 8;

/// Provenance for executable bytes admitted by exec.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecArtifactOrigin {
    /// No executable bytes were resolved.
    None,
    /// Bytes came from the runtime installed source overlay.
    InstalledOverlay,
    /// Bytes were loaded from a checked source-media catalog entry.
    SourceMediaCatalog,
    /// Bytes were loaded from a single checked source-media artifact.
    SourceMediaSingle,
    /// Bytes were loaded from a checked executable bundle block target.
    BlockBundle,
    /// Bytes came from image-linked source artifacts.
    ImageLinked,
}

impl ExecArtifactOrigin {
    /// Stable diagnostic word.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::InstalledOverlay => "installed-overlay",
            Self::SourceMediaCatalog => "source-media-catalog",
            Self::SourceMediaSingle => "source-media-single",
            Self::BlockBundle => "block-bundle",
            Self::ImageLinked => "image-linked",
        }
    }

    /// Whether this origin loaded bytes directly from source media.
    #[must_use]
    pub const fn is_source_media(self) -> bool {
        matches!(self, Self::SourceMediaCatalog | Self::SourceMediaSingle)
    }
}

/// Executable namespace for one resolved artifact descriptor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecArtifactKind {
    /// `/bin` program artifact.
    Bin,
    /// `/payload` artifact.
    Payload,
}

/// Checked executable artifact envelope format retained at admission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecArtifactFormat {
    /// No executable bytes were admitted.
    None,
    /// Bytes were already resident in the image or runtime overlay.
    Direct,
    /// Offset zero was one checked artifact envelope.
    SingleArtifact,
    /// A checked catalog entry selected the artifact envelope.
    Catalog,
}

impl ExecArtifactFormat {
    /// Stable diagnostic word.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Direct => "direct",
            Self::SingleArtifact => "artifact",
            Self::Catalog => "catalog",
        }
    }
}

/// Executable body format retained at admission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecArtifactBodyFormat {
    /// No executable body was admitted.
    None,
    /// Body is an image-linked no_std program.
    LinkedImage,
    /// Body is a checked Reovim executable wrapper.
    ReovimExecBody,
    /// Body is the transitional source-image interpreter format.
    SourceImage,
}

impl ExecArtifactBodyFormat {
    /// Stable diagnostic word.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::LinkedImage => "linked-image",
            Self::ReovimExecBody => "reovim-exec-body",
            Self::SourceImage => "source-image",
        }
    }
}

/// Inner executable body contract retained at admission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecArtifactBodyInnerFormat {
    /// No checked Reovim executable body wrapper was admitted.
    None,
    /// Checked `/bin` source-image compatibility body.
    BinSourceImage,
    /// Checked `/bin` body executed through Reovim domain uapi over raw syscall.
    BinUapiV1,
    /// Checked `/payload` source-image compatibility body.
    PayloadSourceImage,
}

impl ExecArtifactBodyInnerFormat {
    /// Stable diagnostic word.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::BinSourceImage => "bin-source-image",
            Self::BinUapiV1 => "bin-uapi-v1",
            Self::PayloadSourceImage => "payload-source-image",
        }
    }
}

fn checked_exec_body_inner_format(bytes: &'static [u8]) -> ExecArtifactBodyInnerFormat {
    match exec_body::parse_exec_body(bytes) {
        Ok(body) => exec_body_inner_format(body.inner),
        Err(_) => ExecArtifactBodyInnerFormat::None,
    }
}

/// Returns the executable body contract for a loaded `/bin` program.
#[must_use]
pub fn program_body_formats(
    program: LoadedProgram,
) -> (ExecArtifactBodyFormat, ExecArtifactBodyInnerFormat) {
    match program.image_kind {
        program::ProgramImageKind::LinkedBin => {
            (ExecArtifactBodyFormat::LinkedImage, ExecArtifactBodyInnerFormat::None)
        }
        program::ProgramImageKind::SourceImage => {
            (ExecArtifactBodyFormat::SourceImage, ExecArtifactBodyInnerFormat::None)
        }
        program::ProgramImageKind::ReovimExecBody => (
            ExecArtifactBodyFormat::ReovimExecBody,
            checked_exec_body_inner_format(program.source_bytes()),
        ),
    }
}

/// Returns the executable body contract for a loaded `/payload` program.
#[must_use]
pub fn payload_body_formats(
    payload: LoadedPayloadProgram,
) -> (ExecArtifactBodyFormat, ExecArtifactBodyInnerFormat) {
    match payload.image_kind {
        rootd::PayloadImageKind::SourceImage => {
            (ExecArtifactBodyFormat::SourceImage, ExecArtifactBodyInnerFormat::None)
        }
        rootd::PayloadImageKind::ReovimExecBody => (
            ExecArtifactBodyFormat::ReovimExecBody,
            checked_exec_body_inner_format(payload.source_bytes()),
        ),
    }
}

/// Verified executable artifact metadata retained with one admission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExecArtifactMetadata {
    /// Artifact envelope format.
    pub format: ExecArtifactFormat,
    /// Executable body format.
    pub body_format: ExecArtifactBodyFormat,
    /// Inner checked executable body format, when [`Self::body_format`] is
    /// [`ExecArtifactBodyFormat::ReovimExecBody`].
    pub body_inner_format: ExecArtifactBodyInnerFormat,
    /// Exact checked envelope byte count, or zero for direct image/overlay loads.
    pub artifact_bytes_len: usize,
    /// Executable body byte count carried by the envelope or direct source image.
    pub body_bytes_len: usize,
    /// Checksum of the executable body bytes, or zero when no body bytes exist.
    pub checksum: u32,
}

impl ExecArtifactMetadata {
    /// Empty metadata for failed or missing admissions.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            format: ExecArtifactFormat::None,
            body_format: ExecArtifactBodyFormat::None,
            body_inner_format: ExecArtifactBodyInnerFormat::None,
            artifact_bytes_len: 0,
            body_bytes_len: 0,
            checksum: 0,
        }
    }

    fn direct_source(source_bytes: &[u8]) -> Self {
        Self {
            format: ExecArtifactFormat::Direct,
            body_format: ExecArtifactBodyFormat::SourceImage,
            body_inner_format: ExecArtifactBodyInnerFormat::None,
            artifact_bytes_len: 0,
            body_bytes_len: source_bytes.len(),
            checksum: source_store::source_media_checksum32(source_bytes),
        }
    }

    const fn direct_linked() -> Self {
        Self {
            format: ExecArtifactFormat::Direct,
            body_format: ExecArtifactBodyFormat::LinkedImage,
            body_inner_format: ExecArtifactBodyInnerFormat::None,
            artifact_bytes_len: 0,
            body_bytes_len: 0,
            checksum: 0,
        }
    }

    const fn source_media(read: source_media::SourceMediaArtifactRead<'_>) -> Self {
        Self {
            format: match read.format {
                source_media::SourceMediaArtifactFormat::SingleArtifact => {
                    ExecArtifactFormat::SingleArtifact
                }
                source_media::SourceMediaArtifactFormat::Catalog => ExecArtifactFormat::Catalog,
            },
            body_format: ExecArtifactBodyFormat::SourceImage,
            body_inner_format: ExecArtifactBodyInnerFormat::None,
            artifact_bytes_len: read.artifact_bytes_len,
            body_bytes_len: read.artifact.source_bytes.len(),
            checksum: read.artifact.checksum,
        }
    }

    fn exec_bundle(read: exec_bundle::ExecBundleArtifactRead<'_>) -> Self {
        let (body_format, body_inner_format) =
            match exec_body::parse_exec_body(read.artifact.source_bytes) {
                Ok(body) => {
                    (ExecArtifactBodyFormat::ReovimExecBody, exec_body_inner_format(body.inner))
                }
                Err(_) => (ExecArtifactBodyFormat::SourceImage, ExecArtifactBodyInnerFormat::None),
            };
        Self {
            format: match read.format {
                exec_bundle::ExecBundleArtifactFormat::SingleArtifact => {
                    ExecArtifactFormat::SingleArtifact
                }
                exec_bundle::ExecBundleArtifactFormat::Catalog => ExecArtifactFormat::Catalog,
            },
            body_format,
            body_inner_format,
            artifact_bytes_len: read.artifact_bytes_len,
            body_bytes_len: read.artifact.source_bytes.len(),
            checksum: read.artifact.checksum,
        }
    }
}

const fn exec_body_inner_format(
    inner: exec_body::ExecBodyInnerFormat,
) -> ExecArtifactBodyInnerFormat {
    match inner {
        exec_body::ExecBodyInnerFormat::BinSourceImage => {
            ExecArtifactBodyInnerFormat::BinSourceImage
        }
        exec_body::ExecBodyInnerFormat::BinUapiV1 => ExecArtifactBodyInnerFormat::BinUapiV1,
        exec_body::ExecBodyInnerFormat::PayloadSourceImage => {
            ExecArtifactBodyInnerFormat::PayloadSourceImage
        }
    }
}

/// Descriptor metadata for a resolved or partially resolved executable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExecArtifactImage {
    /// Process-visible executable path.
    pub path: &'static str,
    /// Loader/source kind.
    pub loader: &'static str,
    /// Loader-visible source artifact path.
    pub source_path: &'static str,
    /// Stable executable entry name.
    pub entry_name: &'static str,
    /// Executable namespace.
    pub kind: ExecArtifactKind,
}

/// Loaded `/bin` executable bytes plus provider provenance.
#[derive(Clone, Copy, Debug)]
pub struct LoadedBinArtifact {
    /// Loaded `/bin` program.
    pub program: LoadedProgram,
    /// Provider that supplied the admitted bytes.
    pub origin: ExecArtifactOrigin,
    /// Verified executable artifact metadata.
    pub metadata: ExecArtifactMetadata,
}

/// Loaded payload executable bytes plus provider provenance.
#[derive(Clone, Copy, Debug)]
pub struct LoadedPayloadArtifact {
    /// Loaded payload program.
    pub payload: LoadedPayloadProgram,
    /// Provider that supplied the admitted bytes.
    pub origin: ExecArtifactOrigin,
    /// Verified executable artifact metadata.
    pub metadata: ExecArtifactMetadata,
}

/// Summary of runtime `/bin` descriptors discovered from an executable bundle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExecBundleDescriptorInstall {
    /// Bundle root was present and structurally usable.
    pub available: bool,
    /// Number of `/bin` descriptors installed or already present.
    pub installed: usize,
    /// Whether the catalog had more entries than the bounded snapshot held.
    pub truncated: bool,
}

impl ExecBundleDescriptorInstall {
    const fn unavailable() -> Self {
        Self {
            available: false,
            installed: 0,
            truncated: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct LoadedArtifactMetadata {
    origin: ExecArtifactOrigin,
    metadata: ExecArtifactMetadata,
    source_bytes: Option<&'static [u8]>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CachedArtifactBodyRead {
    metadata: ExecArtifactMetadata,
    bytes: &'static [u8],
}

#[derive(Clone, Copy)]
struct CachedArtifactBodySlot {
    namespace: SourceArtifactNamespace,
    path: &'static str,
    metadata: ExecArtifactMetadata,
    bytes: [u8; source_store::MAX_SOURCE_MEDIA_ARTIFACT_BYTES],
    len: usize,
    occupied: bool,
}

impl CachedArtifactBodySlot {
    const fn empty() -> Self {
        Self {
            namespace: SourceArtifactNamespace::Bin,
            path: "",
            metadata: ExecArtifactMetadata::empty(),
            bytes: [0u8; source_store::MAX_SOURCE_MEDIA_ARTIFACT_BYTES],
            len: 0,
            occupied: false,
        }
    }

    fn write(
        &mut self,
        namespace: SourceArtifactNamespace,
        path: &'static str,
        bytes: &[u8],
        metadata: ExecArtifactMetadata,
    ) -> bool {
        if bytes.len() > self.bytes.len() {
            return false;
        }
        self.namespace = namespace;
        self.path = path;
        self.metadata = metadata;
        self.len = bytes.len();
        self.bytes[..self.len].copy_from_slice(bytes);
        self.occupied = true;
        true
    }
}

struct CachedArtifactBodies {
    slots: [CachedArtifactBodySlot; MAX_EXEC_ARTIFACT_BODY_RECORDS],
}

impl CachedArtifactBodies {
    const fn new() -> Self {
        Self {
            slots: [CachedArtifactBodySlot::empty(); MAX_EXEC_ARTIFACT_BODY_RECORDS],
        }
    }

    fn reset(&mut self) {
        self.slots = [CachedArtifactBodySlot::empty(); MAX_EXEC_ARTIFACT_BODY_RECORDS];
    }

    fn install(
        &mut self,
        namespace: SourceArtifactNamespace,
        path: &'static str,
        bytes: &[u8],
        metadata: ExecArtifactMetadata,
    ) -> Option<usize> {
        let mut index = 0usize;
        while index < self.slots.len() {
            if self.slots[index].occupied
                && self.slots[index].namespace == namespace
                && self.slots[index].path == path
            {
                return self.slots[index]
                    .write(namespace, path, bytes, metadata)
                    .then_some(index);
            }
            index += 1;
        }

        index = 0;
        while index < self.slots.len() {
            if !self.slots[index].occupied {
                return self.slots[index]
                    .write(namespace, path, bytes, metadata)
                    .then_some(index);
            }
            index += 1;
        }
        None
    }

    fn find(&self, namespace: SourceArtifactNamespace, path: &'static str) -> Option<usize> {
        let mut index = 0usize;
        while index < self.slots.len() {
            if self.slots[index].occupied
                && self.slots[index].namespace == namespace
                && self.slots[index].path == path
            {
                return Some(index);
            }
            index += 1;
        }
        None
    }
}

struct CachedArtifactBodiesCell(UnsafeCell<CachedArtifactBodies>);

// SAFETY: mutable access is serialized by `CACHED_ARTIFACT_BODIES_LOCK`.
unsafe impl Sync for CachedArtifactBodiesCell {}

static CACHED_ARTIFACT_BODIES: CachedArtifactBodiesCell =
    CachedArtifactBodiesCell(UnsafeCell::new(CachedArtifactBodies::new()));
static CACHED_ARTIFACT_BODIES_LOCK: AtomicBool = AtomicBool::new(false);

struct CachedArtifactBodiesGuard;

impl CachedArtifactBodiesGuard {
    fn acquire() -> Self {
        while CACHED_ARTIFACT_BODIES_LOCK
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }
        Self
    }
}

impl Drop for CachedArtifactBodiesGuard {
    fn drop(&mut self) {
        CACHED_ARTIFACT_BODIES_LOCK.store(false, Ordering::Release);
    }
}

fn with_cached_artifact_bodies<R>(f: impl FnOnce(&mut CachedArtifactBodies) -> R) -> R {
    let _guard = CachedArtifactBodiesGuard::acquire();
    // SAFETY: `CACHED_ARTIFACT_BODIES_LOCK` serializes access to the cache.
    let cache = unsafe { &mut *CACHED_ARTIFACT_BODIES.0.get() };
    f(cache)
}

fn cached_artifact_body_read(index: usize) -> CachedArtifactBodyRead {
    // SAFETY: cached artifact body slots are static for the process lifetime.
    // Replacing the same namespace/path has the same lifetime caveat as the
    // existing runtime source overlay: do not replace bytes for a live image.
    unsafe {
        let cache = &*CACHED_ARTIFACT_BODIES.0.get();
        let slot = &cache.slots[index];
        CachedArtifactBodyRead {
            metadata: slot.metadata,
            bytes: slice::from_raw_parts(slot.bytes.as_ptr(), slot.len),
        }
    }
}

fn cache_artifact_body(
    namespace: SourceArtifactNamespace,
    path: &'static str,
    bytes: &[u8],
    metadata: ExecArtifactMetadata,
) -> Option<CachedArtifactBodyRead> {
    let index =
        with_cached_artifact_bodies(|cache| cache.install(namespace, path, bytes, metadata))?;
    Some(cached_artifact_body_read(index))
}

fn find_cached_artifact_body(
    namespace: SourceArtifactNamespace,
    path: &'static str,
) -> Option<CachedArtifactBodyRead> {
    let index = with_cached_artifact_bodies(|cache| cache.find(namespace, path))?;
    Some(cached_artifact_body_read(index))
}

/// Clears exec-owned provider artifact body cache.
pub(crate) fn reset_cached_artifact_bodies() {
    with_cached_artifact_bodies(CachedArtifactBodies::reset);
}

/// Provider-level failure while resolving executable bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecArtifactLoadErrorKind {
    /// The resolved source artifact was unavailable.
    SourceNotFound,
    /// Source bytes failed loader validation.
    InvalidImage,
    /// Source media was present but not structurally/checksum valid.
    SourceMediaInvalid,
    /// Source media declared a different namespace or path.
    SourceMediaMismatch,
    /// Matching source-media bytes could not be installed into the overlay.
    SourceMediaInstallFailed,
    /// Executable bundle was present but failed validation.
    BlockBundleInvalid,
    /// Executable bundle contained a different namespace or path.
    BlockBundleMismatch,
    /// Executable bundle matched but could not be retained for loading.
    BlockBundleInstallFailed,
}

/// Artifact resolver failure with any metadata available before failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExecArtifactLoadError {
    /// Failure category.
    pub kind: ExecArtifactLoadErrorKind,
    /// Descriptor metadata, when descriptor resolution succeeded.
    pub image: Option<ExecArtifactImage>,
    /// Provider that supplied invalid bytes, when known.
    pub origin: ExecArtifactOrigin,
    /// Artifact metadata that was validated before the failure, when any.
    pub metadata: ExecArtifactMetadata,
}

impl ExecArtifactLoadError {
    const fn new(
        kind: ExecArtifactLoadErrorKind,
        image: Option<ExecArtifactImage>,
        origin: ExecArtifactOrigin,
    ) -> Self {
        Self {
            kind,
            image,
            origin,
            metadata: ExecArtifactMetadata::empty(),
        }
    }

    const fn with_metadata(
        kind: ExecArtifactLoadErrorKind,
        image: Option<ExecArtifactImage>,
        origin: ExecArtifactOrigin,
        metadata: ExecArtifactMetadata,
    ) -> Self {
        Self {
            kind,
            image,
            origin,
            metadata,
        }
    }
}

/// Loads argv0 as a `/bin` executable artifact.
pub fn load_bin(
    programs: &'static [ProgramDescriptor],
    source_store: ExecutableSourceStore,
    argv0: &str,
) -> Result<Option<LoadedBinArtifact>, ExecArtifactLoadError> {
    let Some((_, descriptor)) = program::resolve_argv0(programs, argv0) else {
        return load_provider_discovered_bin(programs, source_store, argv0);
    };

    let image = Some(bin_image(descriptor));
    let source_path = descriptor.source_path();
    let direct_origin = if descriptor.image_kind() == program::ProgramImageKind::LinkedBin {
        ExecArtifactOrigin::ImageLinked
    } else {
        source_store
            .program_origin(source_path)
            .map(source_origin)
            .unwrap_or(ExecArtifactOrigin::None)
    };

    match program::load_argv0(programs, source_store, argv0) {
        Ok(Some(program)) => Ok(Some(LoadedBinArtifact {
            program,
            origin: direct_origin,
            metadata: bin_metadata(program),
        })),
        Ok(None) => Ok(None),
        Err(ProgramLoadError::InvalidImage) => Err(ExecArtifactLoadError::new(
            ExecArtifactLoadErrorKind::InvalidImage,
            image,
            direct_origin,
        )),
        Err(ProgramLoadError::SourceNotFound) => {
            let artifact = match load_block_bundle(SourceArtifactNamespace::Bin, source_path) {
                Ok(Some(artifact)) => artifact,
                Ok(None) => load_source_media(SourceArtifactNamespace::Bin, source_path).map_err(
                    |kind| ExecArtifactLoadError::new(kind, image, ExecArtifactOrigin::None),
                )?,
                Err(kind) => {
                    return Err(ExecArtifactLoadError::new(
                        kind,
                        image,
                        ExecArtifactOrigin::BlockBundle,
                    ));
                }
            };
            let loaded = if let Some(source_bytes) = artifact.source_bytes {
                load_bin_provider_body(programs, argv0, source_bytes, artifact.metadata)
            } else {
                program::load_argv0(programs, source_store, argv0)
            };
            match loaded {
                Ok(Some(program)) => Ok(Some(LoadedBinArtifact {
                    program,
                    origin: artifact.origin,
                    metadata: artifact.metadata,
                })),
                Ok(None) | Err(ProgramLoadError::SourceNotFound) => {
                    Err(ExecArtifactLoadError::new(
                        ExecArtifactLoadErrorKind::SourceNotFound,
                        image,
                        ExecArtifactOrigin::None,
                    ))
                }
                Err(ProgramLoadError::InvalidImage) => Err(ExecArtifactLoadError::new(
                    ExecArtifactLoadErrorKind::InvalidImage,
                    image,
                    artifact.origin,
                )),
            }
        }
    }
}

/// Loads a payload executable artifact by launch-profile name.
pub fn load_payload(
    payloads: &[PayloadDescriptor],
    source_store: ExecutableSourceStore,
    name: &str,
) -> Result<Option<LoadedPayloadArtifact>, ExecArtifactLoadError> {
    let Some((index, descriptor)) = rootd::find_payload_by_name(payloads, name) else {
        return load_provider_discovered_payload(payloads, source_store, name);
    };

    load_resolved_payload(index, descriptor, source_store)
}

fn load_resolved_payload(
    index: usize,
    descriptor: &PayloadDescriptor,
    source_store: ExecutableSourceStore,
) -> Result<Option<LoadedPayloadArtifact>, ExecArtifactLoadError> {
    let image = Some(payload_image(descriptor));
    let source_path = descriptor.image.source_path();
    let direct_origin = source_store
        .payload_origin(source_path)
        .map(source_origin)
        .unwrap_or(ExecArtifactOrigin::None);

    match LoadedPayloadProgram::from_descriptor(index, descriptor, source_store) {
        Ok(payload) => Ok(Some(LoadedPayloadArtifact {
            payload,
            origin: direct_origin,
            metadata: payload_metadata(payload),
        })),
        Err(PayloadLoadError::InvalidImage) => Err(ExecArtifactLoadError::new(
            ExecArtifactLoadErrorKind::InvalidImage,
            image,
            direct_origin,
        )),
        Err(PayloadLoadError::SourceNotFound) => {
            let artifact = match load_block_bundle(SourceArtifactNamespace::Payload, source_path) {
                Ok(Some(artifact)) => artifact,
                Ok(None) => load_source_media(SourceArtifactNamespace::Payload, source_path)
                    .map_err(|kind| {
                        ExecArtifactLoadError::new(kind, image, ExecArtifactOrigin::None)
                    })?,
                Err(kind) => {
                    return Err(ExecArtifactLoadError::new(
                        kind,
                        image,
                        ExecArtifactOrigin::BlockBundle,
                    ));
                }
            };
            let loaded = if let Some(source_bytes) = artifact.source_bytes {
                load_payload_provider_body(index, descriptor, source_bytes, artifact.metadata)
            } else {
                LoadedPayloadProgram::from_descriptor(index, descriptor, source_store)
            };
            match loaded {
                Ok(payload) => Ok(Some(LoadedPayloadArtifact {
                    payload,
                    origin: artifact.origin,
                    metadata: artifact.metadata,
                })),
                Err(PayloadLoadError::SourceNotFound) => Err(ExecArtifactLoadError::new(
                    ExecArtifactLoadErrorKind::SourceNotFound,
                    image,
                    ExecArtifactOrigin::None,
                )),
                Err(PayloadLoadError::InvalidImage) => Err(ExecArtifactLoadError::new(
                    ExecArtifactLoadErrorKind::InvalidImage,
                    image,
                    artifact.origin,
                )),
            }
        }
    }
}

/// Installs bounded runtime `/bin` descriptors named by the executable-bundle root.
///
/// This exposes provider-backed `/bin` names before first execution. It does
/// not install source bytes; exec admission still re-reads and validates the
/// selected artifact envelope before a process can run.
pub fn install_exec_bundle_bin_descriptors() -> ExecBundleDescriptorInstall {
    let mut root = [0u8; MAX_SOURCE_MEDIA_CATALOG_BYTES];
    let mut entries = [exec_bundle::EMPTY_EXEC_BUNDLE_CATALOG_ENTRY;
        exec_bundle::MAX_EXEC_BUNDLE_CATALOG_RECORDS];
    match exec_bundle::snapshot_root(&mut root, &mut entries) {
        exec_bundle::ExecBundleSnapshot::Catalog {
            count, truncated, ..
        } => {
            let mut installed = 0usize;
            let mut index = 0usize;
            while index < count {
                if entries[index].namespace == SourceArtifactNamespace::Bin
                    && program::install_media_program(entries[index].path).is_ok()
                {
                    installed += 1;
                }
                index += 1;
            }
            ExecBundleDescriptorInstall {
                available: true,
                installed,
                truncated,
            }
        }
        exec_bundle::ExecBundleSnapshot::Artifact { artifact, .. } => {
            let installed = if artifact.namespace == SourceArtifactNamespace::Bin
                && program::install_media_program(artifact.path).is_ok()
            {
                1
            } else {
                0
            };
            ExecBundleDescriptorInstall {
                available: true,
                installed,
                truncated: false,
            }
        }
        exec_bundle::ExecBundleSnapshot::Unavailable { .. }
        | exec_bundle::ExecBundleSnapshot::ReadError { .. }
        | exec_bundle::ExecBundleSnapshot::InvalidCatalog { .. }
        | exec_bundle::ExecBundleSnapshot::InvalidArtifact { .. } => {
            ExecBundleDescriptorInstall::unavailable()
        }
    }
}

/// Installs bounded runtime `/payload` descriptors named by the executable-bundle root.
///
/// This exposes provider-backed payload names before first launch. It does not
/// install source bytes; payload admission still re-reads and validates the
/// selected artifact envelope before a process can run.
pub fn install_exec_bundle_payload_descriptors() -> ExecBundleDescriptorInstall {
    let mut root = [0u8; MAX_SOURCE_MEDIA_CATALOG_BYTES];
    let mut entries = [exec_bundle::EMPTY_EXEC_BUNDLE_CATALOG_ENTRY;
        exec_bundle::MAX_EXEC_BUNDLE_CATALOG_RECORDS];
    match exec_bundle::snapshot_root(&mut root, &mut entries) {
        exec_bundle::ExecBundleSnapshot::Catalog {
            count, truncated, ..
        } => {
            let mut installed = 0usize;
            let mut index = 0usize;
            while index < count {
                if entries[index].namespace == SourceArtifactNamespace::Payload
                    && rootd::install_media_payload(entries[index].path).is_ok()
                {
                    installed += 1;
                }
                index += 1;
            }
            ExecBundleDescriptorInstall {
                available: true,
                installed,
                truncated,
            }
        }
        exec_bundle::ExecBundleSnapshot::Artifact { artifact, .. } => {
            let installed = if artifact.namespace == SourceArtifactNamespace::Payload
                && rootd::install_media_payload(artifact.path).is_ok()
            {
                1
            } else {
                0
            };
            ExecBundleDescriptorInstall {
                available: true,
                installed,
                truncated: false,
            }
        }
        exec_bundle::ExecBundleSnapshot::Unavailable { .. }
        | exec_bundle::ExecBundleSnapshot::ReadError { .. }
        | exec_bundle::ExecBundleSnapshot::InvalidCatalog { .. }
        | exec_bundle::ExecBundleSnapshot::InvalidArtifact { .. } => {
            ExecBundleDescriptorInstall::unavailable()
        }
    }
}

fn load_provider_discovered_bin(
    programs: &'static [ProgramDescriptor],
    source_store: ExecutableSourceStore,
    argv0: &str,
) -> Result<Option<LoadedBinArtifact>, ExecArtifactLoadError> {
    let mut path = [0u8; program::MAX_MEDIA_PROGRAM_PATH_BYTES];
    let Some(expected_path) = program::media_program_path_from_argv0(argv0, &mut path) else {
        return Ok(None);
    };

    if block::exec_bundle_status().is_some() {
        match load_discovered_block_bundle(programs, source_store, argv0, expected_path) {
            Ok(Some(artifact)) => return Ok(Some(artifact)),
            Ok(None) => {}
            Err(error) => return Err(error),
        }
    }

    if block::source_media_status().is_none() {
        return Ok(None);
    }

    let mut artifact_bytes = [0u8; MAX_SOURCE_MEDIA_ARTIFACT_BYTES];
    let read = source_media::read_checked_artifact(
        SourceArtifactNamespace::Bin,
        expected_path,
        &mut artifact_bytes,
    )
    .map_err(source_media_error_kind)
    .map_err(|kind| ExecArtifactLoadError::new(kind, None, ExecArtifactOrigin::None))?;
    let origin = source_media_origin(read.format);
    let metadata = ExecArtifactMetadata::source_media(read);

    let descriptor = install_discovered_bin_source(
        expected_path,
        read.artifact.source_bytes,
        origin,
        metadata,
        ExecArtifactLoadErrorKind::SourceMediaInstallFailed,
    )?;
    load_installed_discovered_bin(programs, source_store, argv0, descriptor, origin, metadata, None)
        .map(Some)
}

fn load_discovered_block_bundle(
    programs: &'static [ProgramDescriptor],
    source_store: ExecutableSourceStore,
    argv0: &str,
    expected_path: &str,
) -> Result<Option<LoadedBinArtifact>, ExecArtifactLoadError> {
    let mut artifact_bytes = [0u8; MAX_SOURCE_MEDIA_ARTIFACT_BYTES];
    let read = match exec_bundle::read_checked_artifact(
        SourceArtifactNamespace::Bin,
        expected_path,
        &mut artifact_bytes,
    ) {
        Ok(read) => read,
        Err(exec_bundle::ExecBundleReadError::Unavailable) => return Ok(None),
        Err(error) => {
            return Err(ExecArtifactLoadError::new(
                exec_bundle_error_kind(error),
                None,
                ExecArtifactOrigin::BlockBundle,
            ));
        }
    };
    let metadata = ExecArtifactMetadata::exec_bundle(read);

    let descriptor = install_discovered_bin_descriptor(
        expected_path,
        ExecArtifactOrigin::BlockBundle,
        metadata,
        ExecArtifactLoadErrorKind::BlockBundleInstallFailed,
    )?;
    let source_bytes = cache_artifact_body(
        SourceArtifactNamespace::Bin,
        descriptor.image.source_path(),
        read.artifact.source_bytes,
        metadata,
    )
    .ok_or_else(|| {
        ExecArtifactLoadError::with_metadata(
            ExecArtifactLoadErrorKind::BlockBundleInstallFailed,
            Some(bin_image(descriptor)),
            ExecArtifactOrigin::BlockBundle,
            metadata,
        )
    })?;
    load_installed_discovered_bin(
        programs,
        source_store,
        argv0,
        descriptor,
        ExecArtifactOrigin::BlockBundle,
        metadata,
        Some(source_bytes.bytes),
    )
    .map(Some)
}

fn install_discovered_bin_descriptor(
    expected_path: &str,
    origin: ExecArtifactOrigin,
    metadata: ExecArtifactMetadata,
    install_error: ExecArtifactLoadErrorKind,
) -> Result<&'static ProgramDescriptor, ExecArtifactLoadError> {
    program::install_media_program(expected_path)
        .map_err(|_| ExecArtifactLoadError::with_metadata(install_error, None, origin, metadata))
}

fn install_discovered_bin_source(
    expected_path: &str,
    source_bytes: &[u8],
    origin: ExecArtifactOrigin,
    metadata: ExecArtifactMetadata,
    install_error: ExecArtifactLoadErrorKind,
) -> Result<&'static ProgramDescriptor, ExecArtifactLoadError> {
    let descriptor = program::install_media_program(expected_path)
        .map_err(|_| ExecArtifactLoadError::with_metadata(install_error, None, origin, metadata))?;
    let image = Some(bin_image(descriptor));
    source_store::install_source(
        SourceArtifactNamespace::Bin,
        descriptor.image.source_path(),
        source_bytes,
    )
    .map_err(|_| ExecArtifactLoadError::with_metadata(install_error, image, origin, metadata))?;
    Ok(descriptor)
}

fn load_installed_discovered_bin(
    programs: &'static [ProgramDescriptor],
    source_store: ExecutableSourceStore,
    argv0: &str,
    descriptor: &'static ProgramDescriptor,
    origin: ExecArtifactOrigin,
    metadata: ExecArtifactMetadata,
    source_bytes: Option<&'static [u8]>,
) -> Result<LoadedBinArtifact, ExecArtifactLoadError> {
    let image = Some(bin_image(descriptor));
    let loaded = if let Some(source_bytes) = source_bytes {
        load_bin_provider_body(programs, argv0, source_bytes, metadata)
    } else {
        program::load_argv0(programs, source_store, argv0)
    };
    match loaded {
        Ok(Some(program)) => Ok(LoadedBinArtifact {
            program,
            origin,
            metadata,
        }),
        Ok(None) | Err(ProgramLoadError::SourceNotFound) => {
            Err(ExecArtifactLoadError::with_metadata(
                match origin {
                    ExecArtifactOrigin::BlockBundle => {
                        ExecArtifactLoadErrorKind::BlockBundleInstallFailed
                    }
                    _ => ExecArtifactLoadErrorKind::SourceMediaInstallFailed,
                },
                image,
                origin,
                metadata,
            ))
        }
        Err(ProgramLoadError::InvalidImage) => Err(ExecArtifactLoadError::with_metadata(
            ExecArtifactLoadErrorKind::InvalidImage,
            image,
            origin,
            metadata,
        )),
    }
}

fn load_bin_provider_body(
    programs: &'static [ProgramDescriptor],
    argv0: &str,
    bytes: &'static [u8],
    metadata: ExecArtifactMetadata,
) -> Result<Option<LoadedProgram>, ProgramLoadError> {
    match metadata.body_format {
        ExecArtifactBodyFormat::ReovimExecBody => {
            program::load_argv0_exec_body_bytes(programs, argv0, bytes)
        }
        ExecArtifactBodyFormat::SourceImage => {
            program::load_argv0_source_bytes(programs, argv0, bytes)
        }
        ExecArtifactBodyFormat::None | ExecArtifactBodyFormat::LinkedImage => {
            Err(ProgramLoadError::InvalidImage)
        }
    }
}

fn load_provider_discovered_payload(
    payloads: &[PayloadDescriptor],
    source_store: ExecutableSourceStore,
    name: &str,
) -> Result<Option<LoadedPayloadArtifact>, ExecArtifactLoadError> {
    let mut path = [0u8; rootd::MAX_MEDIA_PAYLOAD_PATH_BYTES];
    let Some(expected_path) = rootd::media_payload_path_from_name(name, &mut path) else {
        return Ok(None);
    };

    if block::exec_bundle_status().is_some() {
        match load_discovered_payload_block_bundle(payloads, source_store, name, expected_path) {
            Ok(Some(artifact)) => return Ok(Some(artifact)),
            Ok(None) => {}
            Err(error) => return Err(error),
        }
    }

    if block::source_media_status().is_none() {
        return Ok(None);
    }

    let mut artifact_bytes = [0u8; MAX_SOURCE_MEDIA_ARTIFACT_BYTES];
    let read = source_media::read_checked_artifact(
        SourceArtifactNamespace::Payload,
        expected_path,
        &mut artifact_bytes,
    )
    .map_err(source_media_error_kind)
    .map_err(|kind| ExecArtifactLoadError::new(kind, None, ExecArtifactOrigin::None))?;
    let origin = source_media_origin(read.format);
    let metadata = ExecArtifactMetadata::source_media(read);

    let (index, descriptor) = install_discovered_payload_source(
        expected_path,
        read.artifact.source_bytes,
        origin,
        metadata,
        ExecArtifactLoadErrorKind::SourceMediaInstallFailed,
    )?;
    load_installed_discovered_payload(
        payloads,
        source_store,
        name,
        index,
        descriptor,
        origin,
        metadata,
        None,
    )
    .map(Some)
}

fn load_discovered_payload_block_bundle(
    payloads: &[PayloadDescriptor],
    source_store: ExecutableSourceStore,
    name: &str,
    expected_path: &str,
) -> Result<Option<LoadedPayloadArtifact>, ExecArtifactLoadError> {
    let mut artifact_bytes = [0u8; MAX_SOURCE_MEDIA_ARTIFACT_BYTES];
    let read = match exec_bundle::read_checked_artifact(
        SourceArtifactNamespace::Payload,
        expected_path,
        &mut artifact_bytes,
    ) {
        Ok(read) => read,
        Err(exec_bundle::ExecBundleReadError::Unavailable) => return Ok(None),
        Err(error) => {
            return Err(ExecArtifactLoadError::new(
                exec_bundle_error_kind(error),
                None,
                ExecArtifactOrigin::BlockBundle,
            ));
        }
    };
    let metadata = ExecArtifactMetadata::exec_bundle(read);

    let (index, descriptor) = install_discovered_payload_descriptor(
        expected_path,
        ExecArtifactOrigin::BlockBundle,
        metadata,
        ExecArtifactLoadErrorKind::BlockBundleInstallFailed,
    )?;
    let source_bytes = cache_artifact_body(
        SourceArtifactNamespace::Payload,
        descriptor.image.source_path(),
        read.artifact.source_bytes,
        metadata,
    )
    .ok_or_else(|| {
        ExecArtifactLoadError::with_metadata(
            ExecArtifactLoadErrorKind::BlockBundleInstallFailed,
            Some(payload_image(descriptor)),
            ExecArtifactOrigin::BlockBundle,
            metadata,
        )
    })?;
    load_installed_discovered_payload(
        payloads,
        source_store,
        name,
        index,
        descriptor,
        ExecArtifactOrigin::BlockBundle,
        metadata,
        Some(source_bytes.bytes),
    )
    .map(Some)
}

fn install_discovered_payload_descriptor(
    expected_path: &str,
    origin: ExecArtifactOrigin,
    metadata: ExecArtifactMetadata,
    install_error: ExecArtifactLoadErrorKind,
) -> Result<(usize, &'static PayloadDescriptor), ExecArtifactLoadError> {
    rootd::install_media_payload(expected_path)
        .map_err(|_| ExecArtifactLoadError::with_metadata(install_error, None, origin, metadata))
}

fn install_discovered_payload_source(
    expected_path: &str,
    source_bytes: &[u8],
    origin: ExecArtifactOrigin,
    metadata: ExecArtifactMetadata,
    install_error: ExecArtifactLoadErrorKind,
) -> Result<(usize, &'static PayloadDescriptor), ExecArtifactLoadError> {
    let (index, descriptor) = rootd::install_media_payload(expected_path)
        .map_err(|_| ExecArtifactLoadError::with_metadata(install_error, None, origin, metadata))?;
    let image = Some(payload_image(descriptor));
    source_store::install_source(
        SourceArtifactNamespace::Payload,
        descriptor.image.source_path(),
        source_bytes,
    )
    .map_err(|_| ExecArtifactLoadError::with_metadata(install_error, image, origin, metadata))?;
    Ok((index, descriptor))
}

fn load_installed_discovered_payload(
    _payloads: &[PayloadDescriptor],
    source_store: ExecutableSourceStore,
    _name: &str,
    index: usize,
    descriptor: &'static PayloadDescriptor,
    origin: ExecArtifactOrigin,
    metadata: ExecArtifactMetadata,
    source_bytes: Option<&'static [u8]>,
) -> Result<LoadedPayloadArtifact, ExecArtifactLoadError> {
    let image = Some(payload_image(descriptor));
    let loaded = if let Some(source_bytes) = source_bytes {
        load_payload_provider_body(index, descriptor, source_bytes, metadata)
    } else {
        LoadedPayloadProgram::from_descriptor(index, descriptor, source_store)
    };
    match loaded {
        Ok(payload) => Ok(LoadedPayloadArtifact {
            payload,
            origin,
            metadata,
        }),
        Err(PayloadLoadError::SourceNotFound) => Err(ExecArtifactLoadError::with_metadata(
            match origin {
                ExecArtifactOrigin::BlockBundle => {
                    ExecArtifactLoadErrorKind::BlockBundleInstallFailed
                }
                _ => ExecArtifactLoadErrorKind::SourceMediaInstallFailed,
            },
            image,
            origin,
            metadata,
        )),
        Err(PayloadLoadError::InvalidImage) => Err(ExecArtifactLoadError::with_metadata(
            ExecArtifactLoadErrorKind::InvalidImage,
            image,
            origin,
            metadata,
        )),
    }
}

fn load_payload_provider_body(
    index: usize,
    descriptor: &PayloadDescriptor,
    bytes: &'static [u8],
    metadata: ExecArtifactMetadata,
) -> Result<LoadedPayloadProgram, PayloadLoadError> {
    match metadata.body_format {
        ExecArtifactBodyFormat::ReovimExecBody => {
            LoadedPayloadProgram::from_descriptor_exec_body_bytes(index, descriptor, bytes)
        }
        ExecArtifactBodyFormat::SourceImage => {
            LoadedPayloadProgram::from_descriptor_source_bytes(index, descriptor, bytes)
        }
        ExecArtifactBodyFormat::None | ExecArtifactBodyFormat::LinkedImage => {
            Err(PayloadLoadError::InvalidImage)
        }
    }
}

fn load_source_media(
    namespace: SourceArtifactNamespace,
    expected_path: &'static str,
) -> Result<LoadedArtifactMetadata, ExecArtifactLoadErrorKind> {
    let mut bytes = [0u8; MAX_SOURCE_MEDIA_ARTIFACT_BYTES];
    let read = source_media::read_checked_artifact(namespace, expected_path, &mut bytes)
        .map_err(source_media_error_kind)?;
    let origin = source_media_origin(read.format);
    let metadata = ExecArtifactMetadata::source_media(read);
    source_store::install_source(namespace, expected_path, read.artifact.source_bytes)
        .map_err(|_| ExecArtifactLoadErrorKind::SourceMediaInstallFailed)?;
    Ok(LoadedArtifactMetadata {
        origin,
        metadata,
        source_bytes: None,
    })
}

fn load_block_bundle(
    namespace: SourceArtifactNamespace,
    expected_path: &'static str,
) -> Result<Option<LoadedArtifactMetadata>, ExecArtifactLoadErrorKind> {
    if block::exec_bundle_status().is_none() {
        return Ok(find_cached_artifact_body(namespace, expected_path).map(|cached| {
            LoadedArtifactMetadata {
                origin: ExecArtifactOrigin::BlockBundle,
                metadata: cached.metadata,
                source_bytes: Some(cached.bytes),
            }
        }));
    }

    let mut bytes = [0u8; MAX_SOURCE_MEDIA_ARTIFACT_BYTES];
    let read = exec_bundle::read_checked_artifact(namespace, expected_path, &mut bytes)
        .map_err(exec_bundle_error_kind)?;
    let metadata = ExecArtifactMetadata::exec_bundle(read);
    let source_bytes =
        cache_artifact_body(namespace, expected_path, read.artifact.source_bytes, metadata)
            .ok_or(ExecArtifactLoadErrorKind::BlockBundleInstallFailed)?;
    Ok(Some(LoadedArtifactMetadata {
        origin: ExecArtifactOrigin::BlockBundle,
        metadata,
        source_bytes: Some(source_bytes.bytes),
    }))
}

const fn bin_image(descriptor: &ProgramDescriptor) -> ExecArtifactImage {
    ExecArtifactImage {
        path: descriptor.path,
        loader: descriptor.image_kind().as_str(),
        source_path: descriptor.source_path(),
        entry_name: descriptor.entry_name,
        kind: ExecArtifactKind::Bin,
    }
}

fn payload_image(descriptor: &PayloadDescriptor) -> ExecArtifactImage {
    ExecArtifactImage {
        path: descriptor.path,
        loader: descriptor.image_kind().as_str(),
        source_path: descriptor.image.source_path(),
        entry_name: descriptor.entry_name,
        kind: ExecArtifactKind::Payload,
    }
}

fn bin_metadata(program: LoadedProgram) -> ExecArtifactMetadata {
    match program.image_kind {
        program::ProgramImageKind::LinkedBin => ExecArtifactMetadata::direct_linked(),
        program::ProgramImageKind::SourceImage | program::ProgramImageKind::ReovimExecBody => {
            ExecArtifactMetadata::direct_source(program.source_bytes())
        }
    }
}

fn payload_metadata(payload: LoadedPayloadProgram) -> ExecArtifactMetadata {
    ExecArtifactMetadata::direct_source(payload.source_bytes())
}

const fn source_origin(origin: SourceArtifactOrigin) -> ExecArtifactOrigin {
    match origin {
        SourceArtifactOrigin::Image => ExecArtifactOrigin::ImageLinked,
        SourceArtifactOrigin::Installed => ExecArtifactOrigin::InstalledOverlay,
    }
}

const fn source_media_origin(
    format: source_media::SourceMediaArtifactFormat,
) -> ExecArtifactOrigin {
    match format {
        source_media::SourceMediaArtifactFormat::Catalog => ExecArtifactOrigin::SourceMediaCatalog,
        source_media::SourceMediaArtifactFormat::SingleArtifact => {
            ExecArtifactOrigin::SourceMediaSingle
        }
    }
}

const fn source_media_error_kind(
    error: source_media::SourceMediaReadError,
) -> ExecArtifactLoadErrorKind {
    match error {
        source_media::SourceMediaReadError::Unavailable
        | source_media::SourceMediaReadError::ReadFailed => {
            ExecArtifactLoadErrorKind::SourceNotFound
        }
        source_media::SourceMediaReadError::Invalid => {
            ExecArtifactLoadErrorKind::SourceMediaInvalid
        }
        source_media::SourceMediaReadError::NamespaceMismatch
        | source_media::SourceMediaReadError::PathMismatch => {
            ExecArtifactLoadErrorKind::SourceMediaMismatch
        }
    }
}

const fn exec_bundle_error_kind(
    error: exec_bundle::ExecBundleReadError,
) -> ExecArtifactLoadErrorKind {
    match error {
        exec_bundle::ExecBundleReadError::Unavailable
        | exec_bundle::ExecBundleReadError::ReadFailed
        | exec_bundle::ExecBundleReadError::Invalid => {
            ExecArtifactLoadErrorKind::BlockBundleInvalid
        }
        exec_bundle::ExecBundleReadError::NamespaceMismatch
        | exec_bundle::ExecBundleReadError::PathMismatch => {
            ExecArtifactLoadErrorKind::BlockBundleMismatch
        }
    }
}
