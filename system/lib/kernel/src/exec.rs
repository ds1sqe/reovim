//! Kernel executable admission and pending-dispatch state.
//!
//! The root shell and `/bin` programs ask for exec/spawn through syscall
//! handles, but the pending executable image queues belong to the kernel exec
//! service. This keeps syscall tracing separate from process/scheduler state so
//! later media-backed executable images have one owner to extend.

use {
    crate::{
        block,
        proc::{self, ProcessHandle, ProcessState, SHELL_PID},
        program::{
            self, LoadedProgram, ProgramArgvBuffer, ProgramDescriptor, ProgramLoadError,
            ProgramStatus,
        },
        rootd::{LoadedPayloadProgram, PayloadDescriptor, PayloadLaunchResult, PayloadLoadError},
        source_store::{
            self, ExecutableSourceStore, MAX_SOURCE_MEDIA_ARTIFACT_BYTES,
            MAX_SOURCE_MEDIA_CATALOG_BYTES, SourceArtifactNamespace,
        },
    },
    core::{
        cell::UnsafeCell,
        sync::atomic::{AtomicBool, Ordering},
    },
};

/// Maximum pending executable invocations.
pub const MAX_PENDING_EXEC_INVOCATIONS: usize = proc::MAX_PROCESSES;
/// Maximum pending executable records across `/bin` and payload queues.
pub const MAX_PENDING_EXEC_RECORDS: usize = MAX_PENDING_EXEC_INVOCATIONS * 2;
/// Maximum retained executable load/admission records.
pub const MAX_EXEC_LOAD_RECORDS: usize = 32;
/// Maximum argv0 bytes retained in one executable load/admission record.
pub const MAX_EXEC_LOAD_ARGV0_BYTES: usize = program::MAX_PROGRAM_ARG_BYTES;
const MAX_PENDING_EXEC_STDIN_BYTES: usize = program::MAX_PROGRAM_STDIN_BYTES;

/// Reason an executable image could not be loaded for exec.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecLoadError {
    /// The shell did not supply an argv0.
    EmptyArgv0,
    /// No executable image is registered for the requested argv0.
    NotFound,
    /// The resolved executable source path was not present in the source store.
    SourceNotFound,
    /// The resolved executable image failed loader validation.
    InvalidImage,
}

/// Result status retained for one executable load/admission attempt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecLoadStatus {
    /// Empty slot.
    Empty,
    /// Executable image metadata was resolved.
    Ok,
    /// Executable image metadata could not be resolved.
    Error,
}

impl ExecLoadStatus {
    /// Stable diagnostic word.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::Ok => "ok",
            Self::Error => "error",
        }
    }
}

/// Executable namespace for one retained load/admission record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecLoadKind {
    /// Empty slot.
    None,
    /// Image `/bin` program.
    Bin,
    /// Launch-profile `/payload` program.
    Payload,
}

impl ExecLoadKind {
    /// Stable diagnostic word.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Bin => "bin",
            Self::Payload => "payload",
        }
    }
}

/// Reason retained for one executable load/admission attempt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecLoadReason {
    /// Empty slot.
    None,
    /// The executable was loaded from the active image `/bin` catalog.
    Loaded,
    /// The executable source artifact was loaded from checked source media.
    LoadedFromSourceMedia,
    /// The request supplied no argv0.
    EmptyArgv0,
    /// No executable image was found for argv0.
    NotFound,
    /// The resolved executable source path was not present in the source store.
    SourceNotFound,
    /// The resolved executable image failed loader validation.
    InvalidImage,
    /// Source media was present but did not contain a valid checked artifact.
    SourceMediaInvalid,
    /// Source media contained a different namespace or path.
    SourceMediaMismatch,
    /// Source media matched but could not be installed into the source overlay.
    SourceMediaInstallFailed,
}

impl ExecLoadReason {
    /// Stable diagnostic word.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Loaded => "loaded",
            Self::LoadedFromSourceMedia => "loaded-source-media",
            Self::EmptyArgv0 => "empty-argv0",
            Self::NotFound => "not-found",
            Self::SourceNotFound => "source-not-found",
            Self::InvalidImage => "invalid-image",
            Self::SourceMediaInvalid => "source-media-invalid",
            Self::SourceMediaMismatch => "source-media-mismatch",
            Self::SourceMediaInstallFailed => "source-media-install-failed",
        }
    }
}

/// Retained executable load/admission record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExecLoadRecord {
    /// Monotonic executable load sequence.
    pub seq: usize,
    argv0: [u8; MAX_EXEC_LOAD_ARGV0_BYTES],
    argv0_len: usize,
    /// Whether argv0 was truncated in the retained diagnostic record.
    pub argv0_truncated: bool,
    /// Resolved executable path, when load succeeded.
    pub path: &'static str,
    /// Loader/source kind, when load succeeded.
    pub loader: &'static str,
    /// Loader-visible source artifact path, when a descriptor resolved.
    pub source_path: &'static str,
    /// Stable executable entry name, when load succeeded.
    pub entry_name: &'static str,
    /// Executable namespace for this admission attempt.
    pub kind: ExecLoadKind,
    /// Load/admission status.
    pub status: ExecLoadStatus,
    /// Load/admission reason.
    pub reason: ExecLoadReason,
}

impl ExecLoadRecord {
    /// Empty load/admission record used for bounded snapshots.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            seq: 0,
            argv0: [0u8; MAX_EXEC_LOAD_ARGV0_BYTES],
            argv0_len: 0,
            argv0_truncated: false,
            path: "",
            loader: "",
            source_path: "",
            entry_name: "",
            kind: ExecLoadKind::None,
            status: ExecLoadStatus::Empty,
            reason: ExecLoadReason::None,
        }
    }

    fn write_argv0(&mut self, argv0: &str) {
        self.argv0_len = 0;
        self.argv0_truncated = false;
        let bytes = argv0.as_bytes();
        while self.argv0_len < bytes.len() && self.argv0_len < self.argv0.len() {
            self.argv0[self.argv0_len] = bytes[self.argv0_len];
            self.argv0_len += 1;
        }
        self.argv0_truncated = self.argv0_len < bytes.len();
    }

    /// Returns retained argv0 text.
    #[must_use]
    pub fn argv0(&self) -> &str {
        // argv0 originates from a Rust `str`; retained bytes are a prefix of it.
        unsafe { core::str::from_utf8_unchecked(&self.argv0[..self.argv0_len]) }
    }
}

/// Empty exec-load record used for bounded snapshots.
pub const EMPTY_EXEC_LOAD_RECORD: ExecLoadRecord = ExecLoadRecord::empty();

/// Retained pending executable invocation record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PendingExecRecord {
    /// Pending process identifier.
    pub pid: usize,
    /// Parent process identifier.
    pub parent_pid: usize,
    /// Pending task identifier.
    pub task_id: usize,
    /// Executable path.
    pub path: &'static str,
    /// Loader/source kind.
    pub loader: &'static str,
    /// Stable executable entry name.
    pub entry_name: &'static str,
    /// Executable namespace.
    pub kind: ExecLoadKind,
    /// Initial stdin bytes retained for pending `/bin` invocations.
    pub stdin_len: usize,
}

impl PendingExecRecord {
    /// Empty pending executable record used for bounded snapshots.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            pid: 0,
            parent_pid: 0,
            task_id: 0,
            path: "",
            loader: "",
            entry_name: "",
            kind: ExecLoadKind::None,
            stdin_len: 0,
        }
    }
}

/// Empty pending executable record used for bounded snapshots.
pub const EMPTY_PENDING_EXEC_RECORD: PendingExecRecord = PendingExecRecord::empty();

#[derive(Clone, Copy)]
enum LoadedExecImage {
    Bin(LoadedProgram),
    Payload(LoadedPayloadProgram),
    Descriptor {
        path: &'static str,
        loader: &'static str,
        source_path: &'static str,
        entry_name: &'static str,
        kind: ExecLoadKind,
    },
}

impl LoadedExecImage {
    const fn path(self) -> &'static str {
        match self {
            Self::Bin(program) => program.descriptor.path,
            Self::Payload(payload) => payload.path,
            Self::Descriptor { path, .. } => path,
        }
    }

    const fn loader(self) -> &'static str {
        match self {
            Self::Bin(program) => program.image_kind.as_str(),
            Self::Payload(payload) => payload.image_kind.as_str(),
            Self::Descriptor { loader, .. } => loader,
        }
    }

    const fn entry_name(self) -> &'static str {
        match self {
            Self::Bin(program) => program.descriptor.entry_name,
            Self::Payload(payload) => payload.entry_name,
            Self::Descriptor { entry_name, .. } => entry_name,
        }
    }

    const fn source_path(self) -> &'static str {
        match self {
            Self::Bin(program) => program.source_path,
            Self::Payload(payload) => payload.source_path,
            Self::Descriptor { source_path, .. } => source_path,
        }
    }

    const fn kind(self) -> ExecLoadKind {
        match self {
            Self::Bin(_) => ExecLoadKind::Bin,
            Self::Payload(_) => ExecLoadKind::Payload,
            Self::Descriptor { kind, .. } => kind,
        }
    }
}

const fn descriptor_image(
    descriptor: &ProgramDescriptor,
    source_path: &'static str,
) -> LoadedExecImage {
    LoadedExecImage::Descriptor {
        path: descriptor.path,
        loader: descriptor.image_kind().as_str(),
        source_path,
        entry_name: descriptor.entry_name,
        kind: ExecLoadKind::Bin,
    }
}

const fn payload_descriptor_image(
    descriptor: &PayloadDescriptor,
    source_path: &'static str,
) -> LoadedExecImage {
    LoadedExecImage::Descriptor {
        path: descriptor.path,
        loader: descriptor.image_kind().as_str(),
        source_path,
        entry_name: descriptor.entry_name,
        kind: ExecLoadKind::Payload,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SourceMediaAdmissionError {
    NoCatalog,
    Unavailable,
    ReadFailed,
    Invalid,
    Mismatch,
    InstallFailed,
}

impl SourceMediaAdmissionError {
    const fn reason(self) -> ExecLoadReason {
        match self {
            Self::NoCatalog => ExecLoadReason::SourceNotFound,
            Self::Unavailable | Self::ReadFailed => ExecLoadReason::SourceNotFound,
            Self::Invalid => ExecLoadReason::SourceMediaInvalid,
            Self::Mismatch => ExecLoadReason::SourceMediaMismatch,
            Self::InstallFailed => ExecLoadReason::SourceMediaInstallFailed,
        }
    }

    const fn error(self) -> ExecLoadError {
        match self {
            Self::Invalid => ExecLoadError::InvalidImage,
            Self::NoCatalog
            | Self::Unavailable
            | Self::ReadFailed
            | Self::Mismatch
            | Self::InstallFailed => ExecLoadError::SourceNotFound,
        }
    }
}

fn install_checked_source_media_artifact(
    namespace: SourceArtifactNamespace,
    expected_path: &'static str,
    artifact_bytes: &[u8],
) -> Result<(), SourceMediaAdmissionError> {
    let artifact = source_store::parse_source_media_artifact(artifact_bytes)
        .map_err(|_| SourceMediaAdmissionError::Invalid)?;
    if artifact.namespace != namespace || artifact.path != expected_path {
        return Err(SourceMediaAdmissionError::Mismatch);
    }
    source_store::install_source(namespace, expected_path, artifact.source_bytes)
        .map_err(|_| SourceMediaAdmissionError::InstallFailed)
}

fn install_matching_source_media_from_catalog(
    namespace: SourceArtifactNamespace,
    expected_path: &'static str,
) -> Result<(), SourceMediaAdmissionError> {
    let mut catalog = [0u8; MAX_SOURCE_MEDIA_CATALOG_BYTES];
    let read = block::read_source_media_artifact(&mut catalog);
    if !read.available {
        return Err(SourceMediaAdmissionError::Unavailable);
    }
    if !read.ok || read.bytes == 0 {
        return Err(SourceMediaAdmissionError::ReadFailed);
    }
    let entry = match source_store::find_source_media_catalog_entry(
        &catalog[..read.bytes],
        namespace,
        expected_path,
    ) {
        Ok(entry) => entry,
        Err(source_store::SourceMediaCatalogError::MissingMagic) => {
            return Err(SourceMediaAdmissionError::NoCatalog);
        }
        Err(source_store::SourceMediaCatalogError::NotFound) => {
            return Err(SourceMediaAdmissionError::Mismatch);
        }
        Err(_) => return Err(SourceMediaAdmissionError::Invalid),
    };
    if entry.artifact_bytes_len > MAX_SOURCE_MEDIA_ARTIFACT_BYTES {
        return Err(SourceMediaAdmissionError::Invalid);
    }

    let mut bytes = [0u8; MAX_SOURCE_MEDIA_ARTIFACT_BYTES];
    let read = block::read_source_media_artifact_at(entry.offset, &mut bytes);
    if !read.available {
        return Err(SourceMediaAdmissionError::Unavailable);
    }
    if !read.ok || read.bytes == 0 {
        return Err(SourceMediaAdmissionError::ReadFailed);
    }
    if read.bytes < entry.artifact_bytes_len {
        return Err(SourceMediaAdmissionError::ReadFailed);
    }
    let artifact_bytes = &bytes[..entry.artifact_bytes_len];
    if source_store::source_media_checksum32(artifact_bytes) != entry.checksum {
        return Err(SourceMediaAdmissionError::Invalid);
    }
    install_checked_source_media_artifact(namespace, expected_path, artifact_bytes)
}

fn install_matching_source_media_single(
    namespace: SourceArtifactNamespace,
    expected_path: &'static str,
) -> Result<(), SourceMediaAdmissionError> {
    let mut bytes = [0u8; MAX_SOURCE_MEDIA_ARTIFACT_BYTES];
    let read = block::read_source_media_artifact(&mut bytes);
    if !read.available {
        return Err(SourceMediaAdmissionError::Unavailable);
    }
    if !read.ok || read.bytes == 0 {
        return Err(SourceMediaAdmissionError::ReadFailed);
    }
    install_checked_source_media_artifact(namespace, expected_path, &bytes[..read.bytes])
}

fn install_matching_source_media(
    namespace: SourceArtifactNamespace,
    expected_path: &'static str,
) -> Result<(), SourceMediaAdmissionError> {
    match install_matching_source_media_from_catalog(namespace, expected_path) {
        Ok(()) => Ok(()),
        Err(SourceMediaAdmissionError::NoCatalog) => {
            install_matching_source_media_single(namespace, expected_path)
        }
        Err(error) => Err(error),
    }
}

/// Loads argv0 through the kernel `/bin` exec service.
///
/// Today this resolves the image-supplied `/bin` catalog. Keeping the load
/// operation here gives later media-backed images the same owner as pending
/// executable admission and scheduler dispatch.
pub fn load_bin_program(
    programs: &'static [ProgramDescriptor],
    source_store: ExecutableSourceStore,
    argv0: &str,
) -> Result<LoadedProgram, ExecLoadError> {
    if argv0.is_empty() {
        with_exec(|exec| {
            exec.record_load(argv0, ExecLoadStatus::Error, ExecLoadReason::EmptyArgv0, None);
        });
        return Err(ExecLoadError::EmptyArgv0);
    }
    let program = match program::load_argv0(programs, source_store, argv0) {
        Ok(Some(program)) => program,
        Ok(None) => {
            with_exec(|exec| {
                exec.record_load(argv0, ExecLoadStatus::Error, ExecLoadReason::NotFound, None);
            });
            return Err(ExecLoadError::NotFound);
        }
        Err(ProgramLoadError::SourceNotFound) => {
            let resolved = program::resolve_argv0(programs, argv0);
            let image = resolved.map(|(_, descriptor)| {
                descriptor_image(descriptor, descriptor.image.source_path())
            });
            if let Some((_, descriptor)) = resolved {
                match install_matching_source_media(
                    SourceArtifactNamespace::Bin,
                    descriptor.image.source_path(),
                ) {
                    Ok(()) => match program::load_argv0(programs, source_store, argv0) {
                        Ok(Some(program)) => {
                            with_exec(|exec| {
                                exec.record_load(
                                    argv0,
                                    ExecLoadStatus::Ok,
                                    ExecLoadReason::LoadedFromSourceMedia,
                                    Some(LoadedExecImage::Bin(program)),
                                );
                            });
                            return Ok(program);
                        }
                        Ok(None) | Err(ProgramLoadError::SourceNotFound) => {}
                        Err(ProgramLoadError::InvalidImage) => {
                            with_exec(|exec| {
                                exec.record_load(
                                    argv0,
                                    ExecLoadStatus::Error,
                                    ExecLoadReason::InvalidImage,
                                    image,
                                );
                            });
                            return Err(ExecLoadError::InvalidImage);
                        }
                    },
                    Err(error) => {
                        with_exec(|exec| {
                            exec.record_load(argv0, ExecLoadStatus::Error, error.reason(), image);
                        });
                        return Err(error.error());
                    }
                }
            }
            with_exec(|exec| {
                exec.record_load(
                    argv0,
                    ExecLoadStatus::Error,
                    ExecLoadReason::SourceNotFound,
                    image,
                );
            });
            return Err(ExecLoadError::SourceNotFound);
        }
        Err(ProgramLoadError::InvalidImage) => {
            let image = program::resolve_argv0(programs, argv0).map(|(_, descriptor)| {
                descriptor_image(descriptor, descriptor.image.source_path())
            });
            with_exec(|exec| {
                exec.record_load(argv0, ExecLoadStatus::Error, ExecLoadReason::InvalidImage, image);
            });
            return Err(ExecLoadError::InvalidImage);
        }
    };
    with_exec(|exec| {
        exec.record_load(
            argv0,
            ExecLoadStatus::Ok,
            ExecLoadReason::Loaded,
            Some(LoadedExecImage::Bin(program)),
        );
    });
    Ok(program)
}

/// Loads a payload through the kernel exec service.
///
/// Payload admission is retained in the same bounded exec-load table as `/bin`
/// admission so later media-backed payload loaders have one diagnostic path.
pub fn load_payload_by_name(
    payloads: &[PayloadDescriptor],
    source_store: ExecutableSourceStore,
    name: &str,
) -> Result<LoadedPayloadProgram, ExecLoadError> {
    if name.is_empty() {
        with_exec(|exec| {
            exec.record_load(name, ExecLoadStatus::Error, ExecLoadReason::EmptyArgv0, None);
        });
        return Err(ExecLoadError::EmptyArgv0);
    }

    let mut index = 0usize;
    while index < payloads.len() {
        if payloads[index].name == name {
            let payload = match LoadedPayloadProgram::from_descriptor(
                index,
                &payloads[index],
                source_store,
            ) {
                Ok(payload) => payload,
                Err(PayloadLoadError::SourceNotFound) => {
                    let image = Some(payload_descriptor_image(
                        &payloads[index],
                        payloads[index].image.source_path(),
                    ));
                    match install_matching_source_media(
                        SourceArtifactNamespace::Payload,
                        payloads[index].image.source_path(),
                    ) {
                        Ok(()) => match LoadedPayloadProgram::from_descriptor(
                            index,
                            &payloads[index],
                            source_store,
                        ) {
                            Ok(payload) => {
                                with_exec(|exec| {
                                    exec.record_load(
                                        name,
                                        ExecLoadStatus::Ok,
                                        ExecLoadReason::LoadedFromSourceMedia,
                                        Some(LoadedExecImage::Payload(payload)),
                                    );
                                });
                                return Ok(payload);
                            }
                            Err(PayloadLoadError::SourceNotFound) => {}
                            Err(PayloadLoadError::InvalidImage) => {
                                with_exec(|exec| {
                                    exec.record_load(
                                        name,
                                        ExecLoadStatus::Error,
                                        ExecLoadReason::InvalidImage,
                                        image,
                                    );
                                });
                                return Err(ExecLoadError::InvalidImage);
                            }
                        },
                        Err(error) => {
                            with_exec(|exec| {
                                exec.record_load(
                                    name,
                                    ExecLoadStatus::Error,
                                    error.reason(),
                                    image,
                                );
                            });
                            return Err(error.error());
                        }
                    }
                    with_exec(|exec| {
                        exec.record_load(
                            name,
                            ExecLoadStatus::Error,
                            ExecLoadReason::SourceNotFound,
                            Some(payload_descriptor_image(
                                &payloads[index],
                                payloads[index].image.source_path(),
                            )),
                        );
                    });
                    return Err(ExecLoadError::SourceNotFound);
                }
                Err(PayloadLoadError::InvalidImage) => {
                    with_exec(|exec| {
                        exec.record_load(
                            name,
                            ExecLoadStatus::Error,
                            ExecLoadReason::InvalidImage,
                            Some(payload_descriptor_image(
                                &payloads[index],
                                payloads[index].image.source_path(),
                            )),
                        );
                    });
                    return Err(ExecLoadError::InvalidImage);
                }
            };
            with_exec(|exec| {
                exec.record_load(
                    name,
                    ExecLoadStatus::Ok,
                    ExecLoadReason::Loaded,
                    Some(LoadedExecImage::Payload(payload)),
                );
            });
            return Ok(payload);
        }
        index += 1;
    }

    with_exec(|exec| {
        exec.record_load(name, ExecLoadStatus::Error, ExecLoadReason::NotFound, None);
    });
    Err(ExecLoadError::NotFound)
}

/// Pending `/bin` program invocation selected later by the scheduler.
#[derive(Clone, Copy, Debug)]
pub struct PendingProgramInvocation {
    handle: ProcessHandle,
    program: LoadedProgram,
    argv: ProgramArgvBuffer,
    stdin: [u8; MAX_PENDING_EXEC_STDIN_BYTES],
    stdin_len: usize,
}

impl PendingProgramInvocation {
    fn new(
        handle: ProcessHandle,
        program: LoadedProgram,
        argv: ProgramArgvBuffer,
        stdin: &[u8],
    ) -> Self {
        let mut pending = Self {
            handle,
            program,
            argv,
            stdin: [0u8; MAX_PENDING_EXEC_STDIN_BYTES],
            stdin_len: 0,
        };
        while pending.stdin_len < stdin.len() && pending.stdin_len < pending.stdin.len() {
            pending.stdin[pending.stdin_len] = stdin[pending.stdin_len];
            pending.stdin_len += 1;
        }
        pending
    }

    /// Process/task handle assigned to the invocation.
    #[must_use]
    pub const fn handle(self) -> ProcessHandle {
        self.handle
    }

    /// Loaded program selected at exec time.
    #[must_use]
    pub const fn program(self) -> LoadedProgram {
        self.program
    }

    /// Owned argv selected at exec admission.
    #[must_use]
    pub const fn argv(&self) -> &ProgramArgvBuffer {
        &self.argv
    }

    /// Initial stdin payload attached to this invocation.
    #[must_use]
    pub fn stdin(&self) -> &[u8] {
        &self.stdin[..self.stdin_len]
    }
}

#[derive(Clone, Copy, Debug)]
struct PendingProgramSlot {
    pending: Option<PendingProgramInvocation>,
}

impl PendingProgramSlot {
    const fn empty() -> Self {
        Self { pending: None }
    }

    fn write(&mut self, pending: PendingProgramInvocation) {
        self.pending = Some(pending);
    }
}

/// Pending payload image selected later by the scheduler.
#[derive(Clone, Copy, Debug)]
pub struct PendingPayloadInvocation {
    parent: ProcessHandle,
    child: ProcessHandle,
    payload: LoadedPayloadProgram,
}

impl PendingPayloadInvocation {
    const fn new(
        parent: ProcessHandle,
        child: ProcessHandle,
        payload: LoadedPayloadProgram,
    ) -> Self {
        Self {
            parent,
            child,
            payload,
        }
    }

    /// Parent process waiting for this payload child.
    #[must_use]
    pub const fn parent(self) -> ProcessHandle {
        self.parent
    }

    /// Child process selected by payload spawn.
    #[must_use]
    pub const fn child(self) -> ProcessHandle {
        self.child
    }

    /// Loaded payload image selected before scheduler dispatch.
    #[must_use]
    pub const fn payload(self) -> LoadedPayloadProgram {
        self.payload
    }
}

#[derive(Clone, Copy, Debug)]
struct PendingPayloadSlot {
    pending: Option<PendingPayloadInvocation>,
}

impl PendingPayloadSlot {
    const fn empty() -> Self {
        Self { pending: None }
    }

    fn write(&mut self, pending: PendingPayloadInvocation) {
        self.pending = Some(pending);
    }
}

struct ExecState {
    pending_programs: [PendingProgramSlot; MAX_PENDING_EXEC_INVOCATIONS],
    pending_payloads: [PendingPayloadSlot; MAX_PENDING_EXEC_INVOCATIONS],
    load_records: [ExecLoadRecord; MAX_EXEC_LOAD_RECORDS],
    next_load_seq: usize,
}

impl ExecState {
    const fn new() -> Self {
        Self {
            pending_programs: [PendingProgramSlot::empty(); MAX_PENDING_EXEC_INVOCATIONS],
            pending_payloads: [PendingPayloadSlot::empty(); MAX_PENDING_EXEC_INVOCATIONS],
            load_records: [ExecLoadRecord::empty(); MAX_EXEC_LOAD_RECORDS],
            next_load_seq: 1,
        }
    }

    fn reset(&mut self) {
        self.pending_programs = [PendingProgramSlot::empty(); MAX_PENDING_EXEC_INVOCATIONS];
        self.pending_payloads = [PendingPayloadSlot::empty(); MAX_PENDING_EXEC_INVOCATIONS];
        self.load_records = [ExecLoadRecord::empty(); MAX_EXEC_LOAD_RECORDS];
        self.next_load_seq = 1;
    }

    fn record_load(
        &mut self,
        argv0: &str,
        status: ExecLoadStatus,
        reason: ExecLoadReason,
        image: Option<LoadedExecImage>,
    ) {
        let seq = self.next_load_seq;
        self.next_load_seq = self.next_load_seq.saturating_add(1);
        let slot = (seq.saturating_sub(1)) % self.load_records.len();
        let mut record = ExecLoadRecord::empty();
        record.seq = seq;
        record.write_argv0(argv0);
        record.status = status;
        record.reason = reason;
        if let Some(image) = image {
            record.path = image.path();
            record.loader = image.loader();
            record.source_path = image.source_path();
            record.entry_name = image.entry_name();
            record.kind = image.kind();
        }
        self.load_records[slot] = record;
    }

    fn snapshot_loads(&self, out: &mut [ExecLoadRecord]) -> usize {
        let total = self.next_load_seq.saturating_sub(1);
        let retained = core::cmp::min(total, self.load_records.len());
        let first_seq = total.saturating_sub(retained).saturating_add(1);
        let mut written = 0usize;
        let mut seq = first_seq;
        while seq <= total && written < out.len() {
            let slot = (seq.saturating_sub(1)) % self.load_records.len();
            let record = self.load_records[slot];
            if record.seq == seq {
                out[written] = record;
                written += 1;
            }
            seq += 1;
        }
        written
    }

    fn snapshot_pending(&self, out: &mut [PendingExecRecord]) -> usize {
        let mut written = 0usize;
        let mut index = 0usize;
        while index < self.pending_programs.len() && written < out.len() {
            if let Some(pending) = self.pending_programs[index].pending {
                let handle = pending.handle;
                let parent_pid = proc::process(handle.pid).map_or(0, |record| record.parent_pid);
                out[written] = PendingExecRecord {
                    pid: handle.pid,
                    parent_pid,
                    task_id: handle.task_id,
                    path: handle.program_path,
                    loader: handle.loader,
                    entry_name: handle.entry_name,
                    kind: ExecLoadKind::Bin,
                    stdin_len: pending.stdin_len,
                };
                written += 1;
            }
            index += 1;
        }

        index = 0;
        while index < self.pending_payloads.len() && written < out.len() {
            if let Some(pending) = self.pending_payloads[index].pending {
                let child = pending.child;
                out[written] = PendingExecRecord {
                    pid: child.pid,
                    parent_pid: pending.parent.pid,
                    task_id: child.task_id,
                    path: child.program_path,
                    loader: child.loader,
                    entry_name: child.entry_name,
                    kind: ExecLoadKind::Payload,
                    stdin_len: 0,
                };
                written += 1;
            }
            index += 1;
        }
        written
    }

    fn store_pending_program(&mut self, pending: PendingProgramInvocation) {
        let mut index = 0usize;
        while index < self.pending_programs.len() {
            if self.pending_programs[index].pending.is_none() {
                self.pending_programs[index].write(pending);
                return;
            }
            index += 1;
        }

        let slot = pending.handle.pid % self.pending_programs.len();
        self.pending_programs[slot].write(pending);
    }

    fn take_pending_program(&mut self, pid: usize) -> Option<PendingProgramInvocation> {
        let mut index = 0usize;
        while index < self.pending_programs.len() {
            if matches!(self.pending_programs[index].pending, Some(pending) if pending.handle.pid == pid)
            {
                let pending = self.pending_programs[index].pending?;
                self.pending_programs[index] = PendingProgramSlot::empty();
                return Some(pending);
            }
            index += 1;
        }
        None
    }

    fn store_pending_payload(&mut self, pending: PendingPayloadInvocation) {
        let mut index = 0usize;
        while index < self.pending_payloads.len() {
            if self.pending_payloads[index].pending.is_none() {
                self.pending_payloads[index].write(pending);
                return;
            }
            index += 1;
        }

        let slot = pending.child.pid % self.pending_payloads.len();
        self.pending_payloads[slot].write(pending);
    }

    fn take_pending_payload(&mut self, pid: usize) -> Option<PendingPayloadInvocation> {
        let mut index = 0usize;
        while index < self.pending_payloads.len() {
            if matches!(self.pending_payloads[index].pending, Some(pending) if pending.child.pid == pid)
            {
                let pending = self.pending_payloads[index].pending?;
                self.pending_payloads[index] = PendingPayloadSlot::empty();
                return Some(pending);
            }
            index += 1;
        }
        None
    }
}

struct ExecCell(UnsafeCell<ExecState>);

// SAFETY: access is serialized by `EXEC_LOCK`.
unsafe impl Sync for ExecCell {}

static EXEC: ExecCell = ExecCell(UnsafeCell::new(ExecState::new()));
static EXEC_LOCK: AtomicBool = AtomicBool::new(false);

struct ExecGuard;

impl ExecGuard {
    fn acquire() -> Self {
        while EXEC_LOCK
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }
        Self
    }
}

impl Drop for ExecGuard {
    fn drop(&mut self) {
        EXEC_LOCK.store(false, Ordering::Release);
    }
}

fn with_exec<R>(f: impl FnOnce(&mut ExecState) -> R) -> R {
    let _guard = ExecGuard::acquire();
    // SAFETY: `EXEC_LOCK` serializes all access to the exec state.
    let exec = unsafe { &mut *EXEC.0.get() };
    f(exec)
}

/// Clears pending executable image queues.
pub fn reset() {
    with_exec(ExecState::reset);
}

/// Copies retained executable load/admission records into `out`.
pub fn snapshot_loads(out: &mut [ExecLoadRecord]) -> usize {
    with_exec(|exec| exec.snapshot_loads(out))
}

/// Copies pending executable invocation records into `out`.
pub fn snapshot_pending(out: &mut [PendingExecRecord]) -> usize {
    with_exec(|exec| exec.snapshot_pending(out))
}

/// Starts a root-shell `/bin` exec request and leaves it pending for scheduler dispatch.
#[must_use]
pub fn spawn_bin_program(program: LoadedProgram, argv: ProgramArgvBuffer) -> ProcessHandle {
    spawn_bin_program_with_stdin(program, argv, &[])
}

/// Starts a root-shell `/bin` exec request with a bounded initial stdin payload.
#[must_use]
pub fn spawn_bin_program_with_stdin(
    program: LoadedProgram,
    argv: ProgramArgvBuffer,
    stdin: &[u8],
) -> ProcessHandle {
    let handle = proc::spawn_program(SHELL_PID, program);
    with_exec(|exec| {
        exec.store_pending_program(PendingProgramInvocation::new(handle, program, argv, stdin));
    });
    handle
}

/// Starts a child `/bin` exec request and leaves it pending for scheduler dispatch.
#[must_use]
pub fn spawn_program_child_with_stdin(
    parent: ProcessHandle,
    program: LoadedProgram,
    argv: ProgramArgvBuffer,
    stdin: &[u8],
) -> ProcessHandle {
    let child = proc::spawn_child(
        parent.pid,
        parent.task_id,
        program.descriptor.path,
        program.image_kind.as_str(),
        program.descriptor.entry_name,
    );
    with_exec(|exec| {
        exec.store_pending_program(PendingProgramInvocation::new(child, program, argv, stdin));
    });
    child
}

/// Spawns a payload child and leaves its loaded image pending for dispatch.
#[must_use]
pub fn spawn_payload_child(parent: ProcessHandle, payload: LoadedPayloadProgram) -> ProcessHandle {
    let child = proc::spawn_child(
        parent.pid,
        parent.task_id,
        payload.path,
        payload.image_kind.as_str(),
        payload.entry_name,
    );
    with_exec(|exec| {
        exec.store_pending_payload(PendingPayloadInvocation::new(parent, child, payload));
    });
    child
}

/// Dispatches the next scheduler-ready process.
#[must_use]
pub fn dispatch_next_ready_process() -> Option<ProcessHandle> {
    proc::dispatch_next_ready_process()
}

/// Takes the pending `/bin` program invocation for a scheduler-selected process.
pub fn take_pending_program(pid: usize) -> Option<PendingProgramInvocation> {
    with_exec(|exec| exec.take_pending_program(pid))
}

/// Drops a pending program invocation without running it.
pub fn discard_pending_program(pid: usize) {
    let _ = take_pending_program(pid);
}

/// Takes the pending payload invocation for a scheduler-selected process.
pub fn take_pending_payload(pid: usize) -> Option<PendingPayloadInvocation> {
    with_exec(|exec| exec.take_pending_payload(pid))
}

/// Drops a pending payload invocation without running it.
pub fn discard_pending_payload(pid: usize) {
    let _ = take_pending_payload(pid);
}

/// Completes a program process.
pub(crate) fn complete_program(pid: usize, status: ProgramStatus) {
    let (state, exit_code) = match status {
        ProgramStatus::Empty => (ProcessState::Exited, 0),
        ProgramStatus::Ok => (ProcessState::Exited, 0),
        ProgramStatus::Error => (ProcessState::Failed, 1),
        ProgramStatus::Halt => (ProcessState::Halted, 0),
    };
    proc::exit_process(pid, state, exit_code);
}

/// Completes a payload child process.
pub fn complete_payload(pid: usize, result: PayloadLaunchResult) {
    let (state, exit_code) = match result {
        PayloadLaunchResult::Ready => (ProcessState::Exited, 0),
        PayloadLaunchResult::NotConfigured | PayloadLaunchResult::Failed => {
            (ProcessState::Failed, 1)
        }
    };
    proc::exit_process(pid, state, exit_code);
}

#[cfg(feature = "selftest")]
#[path = "exec_tests.rs"]
mod tests;
