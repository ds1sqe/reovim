//! Executable artifact resolver for exec admission.
//!
//! `exec` owns process admission and pending dispatch. This module owns the
//! executable-byte provider policy so later block/fs loaders can extend one
//! resolver without teaching exec about each storage shape.

use crate::{
    block, exec_bundle,
    program::{self, LoadedProgram, ProgramDescriptor, ProgramLoadError},
    rootd::{LoadedPayloadProgram, PayloadDescriptor, PayloadLoadError},
    source_media,
    source_store::{
        self, ExecutableSourceStore, MAX_SOURCE_MEDIA_ARTIFACT_BYTES, SourceArtifactNamespace,
        SourceArtifactOrigin,
    },
};

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
}

/// Loaded payload executable bytes plus provider provenance.
#[derive(Clone, Copy, Debug)]
pub struct LoadedPayloadArtifact {
    /// Loaded payload program.
    pub payload: LoadedPayloadProgram,
    /// Provider that supplied the admitted bytes.
    pub origin: ExecArtifactOrigin,
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
    /// Executable bundle matched but could not be installed into the overlay.
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
    let source_path = descriptor.image.source_path();
    let direct_origin = source_store
        .program_origin(source_path)
        .map(source_origin)
        .unwrap_or(ExecArtifactOrigin::None);

    match program::load_argv0(programs, source_store, argv0) {
        Ok(Some(program)) => Ok(Some(LoadedBinArtifact {
            program,
            origin: direct_origin,
        })),
        Ok(None) => Ok(None),
        Err(ProgramLoadError::InvalidImage) => Err(ExecArtifactLoadError::new(
            ExecArtifactLoadErrorKind::InvalidImage,
            image,
            direct_origin,
        )),
        Err(ProgramLoadError::SourceNotFound) => {
            let origin = match load_block_bundle(SourceArtifactNamespace::Bin, source_path) {
                Ok(Some(origin)) => origin,
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
            match program::load_argv0(programs, source_store, argv0) {
                Ok(Some(program)) => Ok(Some(LoadedBinArtifact { program, origin })),
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
                    origin,
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
    let mut index = 0usize;
    while index < payloads.len() {
        if payloads[index].name == name {
            let descriptor = &payloads[index];
            let image = Some(payload_image(descriptor));
            let source_path = descriptor.image.source_path();
            let direct_origin = source_store
                .payload_origin(source_path)
                .map(source_origin)
                .unwrap_or(ExecArtifactOrigin::None);

            return match LoadedPayloadProgram::from_descriptor(index, descriptor, source_store) {
                Ok(payload) => Ok(Some(LoadedPayloadArtifact {
                    payload,
                    origin: direct_origin,
                })),
                Err(PayloadLoadError::InvalidImage) => Err(ExecArtifactLoadError::new(
                    ExecArtifactLoadErrorKind::InvalidImage,
                    image,
                    direct_origin,
                )),
                Err(PayloadLoadError::SourceNotFound) => {
                    let origin =
                        match load_block_bundle(SourceArtifactNamespace::Payload, source_path) {
                            Ok(Some(origin)) => origin,
                            Ok(None) => {
                                load_source_media(SourceArtifactNamespace::Payload, source_path)
                                    .map_err(|kind| {
                                        ExecArtifactLoadError::new(
                                            kind,
                                            image,
                                            ExecArtifactOrigin::None,
                                        )
                                    })?
                            }
                            Err(kind) => {
                                return Err(ExecArtifactLoadError::new(
                                    kind,
                                    image,
                                    ExecArtifactOrigin::BlockBundle,
                                ));
                            }
                        };
                    match LoadedPayloadProgram::from_descriptor(index, descriptor, source_store) {
                        Ok(payload) => Ok(Some(LoadedPayloadArtifact { payload, origin })),
                        Err(PayloadLoadError::SourceNotFound) => Err(ExecArtifactLoadError::new(
                            ExecArtifactLoadErrorKind::SourceNotFound,
                            image,
                            ExecArtifactOrigin::None,
                        )),
                        Err(PayloadLoadError::InvalidImage) => Err(ExecArtifactLoadError::new(
                            ExecArtifactLoadErrorKind::InvalidImage,
                            image,
                            origin,
                        )),
                    }
                }
            };
        }
        index += 1;
    }
    Ok(None)
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

    let descriptor = install_discovered_bin_source(
        expected_path,
        read.artifact.source_bytes,
        origin,
        ExecArtifactLoadErrorKind::SourceMediaInstallFailed,
    )?;
    load_installed_discovered_bin(programs, source_store, argv0, descriptor, origin).map(Some)
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

    let descriptor = install_discovered_bin_source(
        expected_path,
        read.artifact.source_bytes,
        ExecArtifactOrigin::BlockBundle,
        ExecArtifactLoadErrorKind::BlockBundleInstallFailed,
    )?;
    load_installed_discovered_bin(
        programs,
        source_store,
        argv0,
        descriptor,
        ExecArtifactOrigin::BlockBundle,
    )
    .map(Some)
}

fn install_discovered_bin_source(
    expected_path: &str,
    source_bytes: &[u8],
    origin: ExecArtifactOrigin,
    install_error: ExecArtifactLoadErrorKind,
) -> Result<&'static ProgramDescriptor, ExecArtifactLoadError> {
    let descriptor = program::install_media_program(expected_path)
        .map_err(|_| ExecArtifactLoadError::new(install_error, None, origin))?;
    let image = Some(bin_image(descriptor));
    source_store::install_source(
        SourceArtifactNamespace::Bin,
        descriptor.image.source_path(),
        source_bytes,
    )
    .map_err(|_| ExecArtifactLoadError::new(install_error, image, origin))?;
    Ok(descriptor)
}

fn load_installed_discovered_bin(
    programs: &'static [ProgramDescriptor],
    source_store: ExecutableSourceStore,
    argv0: &str,
    descriptor: &'static ProgramDescriptor,
    origin: ExecArtifactOrigin,
) -> Result<LoadedBinArtifact, ExecArtifactLoadError> {
    let image = Some(bin_image(descriptor));
    match program::load_argv0(programs, source_store, argv0) {
        Ok(Some(program)) => Ok(LoadedBinArtifact { program, origin }),
        Ok(None) | Err(ProgramLoadError::SourceNotFound) => Err(ExecArtifactLoadError::new(
            match origin {
                ExecArtifactOrigin::BlockBundle => {
                    ExecArtifactLoadErrorKind::BlockBundleInstallFailed
                }
                _ => ExecArtifactLoadErrorKind::SourceMediaInstallFailed,
            },
            image,
            origin,
        )),
        Err(ProgramLoadError::InvalidImage) => Err(ExecArtifactLoadError::new(
            ExecArtifactLoadErrorKind::InvalidImage,
            image,
            origin,
        )),
    }
}

fn load_source_media(
    namespace: SourceArtifactNamespace,
    expected_path: &'static str,
) -> Result<ExecArtifactOrigin, ExecArtifactLoadErrorKind> {
    let mut bytes = [0u8; MAX_SOURCE_MEDIA_ARTIFACT_BYTES];
    let read = source_media::read_checked_artifact(namespace, expected_path, &mut bytes)
        .map_err(source_media_error_kind)?;
    source_store::install_source(namespace, expected_path, read.artifact.source_bytes)
        .map_err(|_| ExecArtifactLoadErrorKind::SourceMediaInstallFailed)?;
    Ok(source_media_origin(read.format))
}

fn load_block_bundle(
    namespace: SourceArtifactNamespace,
    expected_path: &'static str,
) -> Result<Option<ExecArtifactOrigin>, ExecArtifactLoadErrorKind> {
    if block::exec_bundle_status().is_none() {
        return Ok(None);
    }

    let mut bytes = [0u8; MAX_SOURCE_MEDIA_ARTIFACT_BYTES];
    let read = exec_bundle::read_checked_artifact(namespace, expected_path, &mut bytes)
        .map_err(exec_bundle_error_kind)?;
    source_store::install_source(namespace, expected_path, read.artifact.source_bytes)
        .map_err(|_| ExecArtifactLoadErrorKind::BlockBundleInstallFailed)?;
    Ok(Some(ExecArtifactOrigin::BlockBundle))
}

const fn bin_image(descriptor: &ProgramDescriptor) -> ExecArtifactImage {
    ExecArtifactImage {
        path: descriptor.path,
        loader: descriptor.image_kind().as_str(),
        source_path: descriptor.image.source_path(),
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
