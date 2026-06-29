//! Kernel executable admission and pending-dispatch state.
//!
//! The root shell and `/bin` programs ask for exec/spawn through syscall
//! handles, but the pending executable image queues belong to the kernel exec
//! service. This keeps syscall tracing separate from process/scheduler state so
//! later media-backed executable images have one owner to extend.

use {
    crate::{
        exec_artifact::{
            self, ExecArtifactImage, ExecArtifactKind, ExecArtifactLoadError,
            ExecArtifactLoadErrorKind, ExecArtifactMetadata, ExecArtifactOrigin,
        },
        mm,
        proc::{self, ProcessAdoptionSnapshot, ProcessHandle, ProcessState, ROOTD_PID, SHELL_PID},
        program::{
            self, LoadedProgram, ProgramArgvBuffer, ProgramDescriptor, ProgramEnvBuffer,
            ProgramStatus,
        },
        rootd::{self, LoadedPayloadProgram, PayloadDescriptor, PayloadLaunchResult},
        source_store::ExecutableSourceStore,
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
    /// The executable image loaded, but bounded exec admission had no slot.
    ProcessAdmissionFailed,
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
    /// Executable bundle was present but did not contain a valid checked artifact.
    BlockBundleInvalid,
    /// Executable bundle contained a different namespace or path.
    BlockBundleMismatch,
    /// Executable bundle matched but could not be installed into the source overlay.
    BlockBundleInstallFailed,
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
            Self::BlockBundleInvalid => "block-bundle-invalid",
            Self::BlockBundleMismatch => "block-bundle-mismatch",
            Self::BlockBundleInstallFailed => "block-bundle-install-failed",
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
    /// Provider that supplied the executable bytes, when resolved.
    pub origin: ExecArtifactOrigin,
    /// Checked artifact envelope format.
    pub artifact_format: exec_artifact::ExecArtifactFormat,
    /// Executable body format.
    pub artifact_body_format: exec_artifact::ExecArtifactBodyFormat,
    /// Inner checked executable body format.
    pub artifact_body_inner_format: exec_artifact::ExecArtifactBodyInnerFormat,
    /// Checked artifact envelope byte count.
    pub artifact_bytes_len: usize,
    /// Executable body byte count admitted by the loader.
    pub artifact_body_bytes_len: usize,
    /// Checksum of admitted executable body bytes.
    pub artifact_checksum: u32,
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
            origin: ExecArtifactOrigin::None,
            artifact_format: exec_artifact::ExecArtifactFormat::None,
            artifact_body_format: exec_artifact::ExecArtifactBodyFormat::None,
            artifact_body_inner_format: exec_artifact::ExecArtifactBodyInnerFormat::None,
            artifact_bytes_len: 0,
            artifact_body_bytes_len: 0,
            artifact_checksum: 0,
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
    /// Executable body format retained with this pending invocation.
    pub artifact_body_format: exec_artifact::ExecArtifactBodyFormat,
    /// Inner checked executable body format retained with this pending invocation.
    pub artifact_body_inner_format: exec_artifact::ExecArtifactBodyInnerFormat,
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
            artifact_body_format: exec_artifact::ExecArtifactBodyFormat::None,
            artifact_body_inner_format: exec_artifact::ExecArtifactBodyInnerFormat::None,
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
    const fn from_artifact_image(image: ExecArtifactImage) -> Self {
        Self::Descriptor {
            path: image.path,
            loader: image.loader,
            source_path: image.source_path,
            entry_name: image.entry_name,
            kind: match image.kind {
                ExecArtifactKind::Bin => ExecLoadKind::Bin,
                ExecArtifactKind::Payload => ExecLoadKind::Payload,
            },
        }
    }

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

const fn reason_for_origin(origin: ExecArtifactOrigin) -> ExecLoadReason {
    if origin.is_source_media() {
        ExecLoadReason::LoadedFromSourceMedia
    } else {
        ExecLoadReason::Loaded
    }
}

const fn reason_for_artifact_error(error: ExecArtifactLoadErrorKind) -> ExecLoadReason {
    match error {
        ExecArtifactLoadErrorKind::SourceNotFound => ExecLoadReason::SourceNotFound,
        ExecArtifactLoadErrorKind::InvalidImage => ExecLoadReason::InvalidImage,
        ExecArtifactLoadErrorKind::SourceMediaInvalid => ExecLoadReason::SourceMediaInvalid,
        ExecArtifactLoadErrorKind::SourceMediaMismatch => ExecLoadReason::SourceMediaMismatch,
        ExecArtifactLoadErrorKind::SourceMediaInstallFailed => {
            ExecLoadReason::SourceMediaInstallFailed
        }
        ExecArtifactLoadErrorKind::BlockBundleInvalid => ExecLoadReason::BlockBundleInvalid,
        ExecArtifactLoadErrorKind::BlockBundleMismatch => ExecLoadReason::BlockBundleMismatch,
        ExecArtifactLoadErrorKind::BlockBundleInstallFailed => {
            ExecLoadReason::BlockBundleInstallFailed
        }
    }
}

const fn exec_error_for_artifact_error(error: ExecArtifactLoadErrorKind) -> ExecLoadError {
    match error {
        ExecArtifactLoadErrorKind::InvalidImage
        | ExecArtifactLoadErrorKind::SourceMediaInvalid
        | ExecArtifactLoadErrorKind::BlockBundleInvalid => ExecLoadError::InvalidImage,
        ExecArtifactLoadErrorKind::SourceNotFound
        | ExecArtifactLoadErrorKind::SourceMediaMismatch
        | ExecArtifactLoadErrorKind::SourceMediaInstallFailed
        | ExecArtifactLoadErrorKind::BlockBundleMismatch
        | ExecArtifactLoadErrorKind::BlockBundleInstallFailed => ExecLoadError::SourceNotFound,
    }
}

fn address_space_image_for_payload(payload: LoadedPayloadProgram) -> mm::AddressSpaceImage {
    mm::AddressSpaceImage::source(
        payload.source_path,
        payload.source_bytes().len(),
        crate::source_store::source_media_checksum32(payload.source_bytes()),
    )
}

fn image_for_artifact_error(error: ExecArtifactLoadError) -> Option<LoadedExecImage> {
    error.image.map(LoadedExecImage::from_artifact_image)
}

/// Loads argv0 through the kernel `/bin` exec service.
///
/// Today this asks the artifact resolver for source bytes, then records exec
/// admission here with the same owner as pending dispatch.
pub fn load_bin_program(
    programs: &'static [ProgramDescriptor],
    source_store: ExecutableSourceStore,
    argv0: &str,
) -> Result<LoadedProgram, ExecLoadError> {
    if argv0.is_empty() {
        with_exec(|exec| {
            exec.record_load(
                argv0,
                ExecLoadStatus::Error,
                ExecLoadReason::EmptyArgv0,
                None,
                ExecArtifactOrigin::None,
                ExecArtifactMetadata::empty(),
            );
        });
        return Err(ExecLoadError::EmptyArgv0);
    }
    match exec_artifact::load_bin(programs, source_store, argv0) {
        Ok(Some(artifact)) => {
            with_exec(|exec| {
                exec.record_load(
                    argv0,
                    ExecLoadStatus::Ok,
                    reason_for_origin(artifact.origin),
                    Some(LoadedExecImage::Bin(artifact.program)),
                    artifact.origin,
                    artifact.metadata,
                );
            });
            Ok(artifact.program)
        }
        Ok(None) => {
            with_exec(|exec| {
                exec.record_load(
                    argv0,
                    ExecLoadStatus::Error,
                    ExecLoadReason::NotFound,
                    None,
                    ExecArtifactOrigin::None,
                    ExecArtifactMetadata::empty(),
                );
            });
            Err(ExecLoadError::NotFound)
        }
        Err(error) => {
            let reason = reason_for_artifact_error(error.kind);
            let exec_error = exec_error_for_artifact_error(error.kind);
            with_exec(|exec| {
                exec.record_load(
                    argv0,
                    ExecLoadStatus::Error,
                    reason,
                    image_for_artifact_error(error),
                    error.origin,
                    error.metadata,
                );
            });
            Err(exec_error)
        }
    }
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
            exec.record_load(
                name,
                ExecLoadStatus::Error,
                ExecLoadReason::EmptyArgv0,
                None,
                ExecArtifactOrigin::None,
                ExecArtifactMetadata::empty(),
            );
        });
        return Err(ExecLoadError::EmptyArgv0);
    }

    match exec_artifact::load_payload(payloads, source_store, name) {
        Ok(Some(artifact)) => {
            with_exec(|exec| {
                exec.record_load(
                    name,
                    ExecLoadStatus::Ok,
                    reason_for_origin(artifact.origin),
                    Some(LoadedExecImage::Payload(artifact.payload)),
                    artifact.origin,
                    artifact.metadata,
                );
            });
            Ok(artifact.payload)
        }
        Ok(None) => {
            with_exec(|exec| {
                exec.record_load(
                    name,
                    ExecLoadStatus::Error,
                    ExecLoadReason::NotFound,
                    None,
                    ExecArtifactOrigin::None,
                    ExecArtifactMetadata::empty(),
                );
            });
            Err(ExecLoadError::NotFound)
        }
        Err(error) => {
            let reason = reason_for_artifact_error(error.kind);
            let exec_error = exec_error_for_artifact_error(error.kind);
            with_exec(|exec| {
                exec.record_load(
                    name,
                    ExecLoadStatus::Error,
                    reason,
                    image_for_artifact_error(error),
                    error.origin,
                    error.metadata,
                );
            });
            Err(exec_error)
        }
    }
}

/// Pending `/bin` program invocation selected later by the scheduler.
#[derive(Clone, Copy, Debug)]
pub struct PendingProgramInvocation {
    handle: ProcessHandle,
    program: LoadedProgram,
    argv: ProgramArgvBuffer,
    env: ProgramEnvBuffer,
    stdin: [u8; MAX_PENDING_EXEC_STDIN_BYTES],
    stdin_len: usize,
}

impl PendingProgramInvocation {
    fn new(
        handle: ProcessHandle,
        program: LoadedProgram,
        argv: ProgramArgvBuffer,
        env: ProgramEnvBuffer,
        stdin: &[u8],
    ) -> Self {
        let mut pending = Self {
            handle,
            program,
            argv,
            env,
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

    /// Owned env selected at exec admission.
    #[must_use]
    pub const fn env(&self) -> &ProgramEnvBuffer {
        &self.env
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

const fn process_state_allows_pending_reclaim(state: ProcessState) -> bool {
    match state {
        ProcessState::Empty
        | ProcessState::Exited
        | ProcessState::Failed
        | ProcessState::Halted
        | ProcessState::Reaped => true,
        ProcessState::New | ProcessState::Ready | ProcessState::Running | ProcessState::Blocked => {
            false
        }
    }
}

fn pending_pid_reclaimable(pid: usize) -> bool {
    if pid == 0 {
        return true;
    }
    match proc::process(pid) {
        Some(record) => process_state_allows_pending_reclaim(record.state),
        None => true,
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
        origin: ExecArtifactOrigin,
        metadata: ExecArtifactMetadata,
    ) {
        let seq = self.next_load_seq;
        self.next_load_seq = self.next_load_seq.saturating_add(1);
        let slot = (seq.saturating_sub(1)) % self.load_records.len();
        let mut record = ExecLoadRecord::empty();
        record.seq = seq;
        record.write_argv0(argv0);
        record.status = status;
        record.reason = reason;
        record.origin = origin;
        record.artifact_format = metadata.format;
        record.artifact_body_format = metadata.body_format;
        record.artifact_body_inner_format = metadata.body_inner_format;
        record.artifact_bytes_len = metadata.artifact_bytes_len;
        record.artifact_body_bytes_len = metadata.body_bytes_len;
        record.artifact_checksum = metadata.checksum;
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
                let (body_format, body_inner_format) =
                    exec_artifact::program_body_formats(pending.program);
                out[written] = PendingExecRecord {
                    pid: handle.pid,
                    parent_pid,
                    task_id: handle.task_id,
                    path: handle.program_path,
                    loader: handle.loader,
                    entry_name: handle.entry_name,
                    kind: ExecLoadKind::Bin,
                    artifact_body_format: body_format,
                    artifact_body_inner_format: body_inner_format,
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
                let parent_pid =
                    proc::process(child.pid).map_or(pending.parent.pid, |record| record.parent_pid);
                let (body_format, body_inner_format) =
                    exec_artifact::payload_body_formats(pending.payload);
                out[written] = PendingExecRecord {
                    pid: child.pid,
                    parent_pid,
                    task_id: child.task_id,
                    path: child.program_path,
                    loader: child.loader,
                    entry_name: child.entry_name,
                    kind: ExecLoadKind::Payload,
                    artifact_body_format: body_format,
                    artifact_body_inner_format: body_inner_format,
                    stdin_len: 0,
                };
                written += 1;
            }
            index += 1;
        }
        written
    }

    fn reclaim_reclaimable_pending_programs(&mut self) {
        let mut index = 0usize;
        while index < self.pending_programs.len() {
            if let Some(pending) = self.pending_programs[index].pending {
                if pending_pid_reclaimable(pending.handle.pid) {
                    self.pending_programs[index] = PendingProgramSlot::empty();
                }
            }
            index += 1;
        }
    }

    fn reclaim_reclaimable_pending_payloads(&mut self) {
        let mut index = 0usize;
        while index < self.pending_payloads.len() {
            if let Some(pending) = self.pending_payloads[index].pending {
                if pending_pid_reclaimable(pending.child.pid) {
                    self.pending_payloads[index] = PendingPayloadSlot::empty();
                }
            }
            index += 1;
        }
    }

    fn has_pending_program_capacity(&mut self) -> bool {
        self.reclaim_reclaimable_pending_programs();
        let mut index = 0usize;
        while index < self.pending_programs.len() {
            if self.pending_programs[index].pending.is_none() {
                return true;
            }
            index += 1;
        }
        false
    }

    fn has_pending_program_capacity_for_pid(&mut self, pid: usize) -> bool {
        self.reclaim_reclaimable_pending_programs();
        let mut index = 0usize;
        while index < self.pending_programs.len() {
            match self.pending_programs[index].pending {
                Some(pending) if pending.handle.pid == pid => return true,
                None => return true,
                Some(_) => {}
            }
            index += 1;
        }
        false
    }

    fn has_pending_payload_capacity(&mut self) -> bool {
        self.reclaim_reclaimable_pending_payloads();
        let mut index = 0usize;
        while index < self.pending_payloads.len() {
            if self.pending_payloads[index].pending.is_none() {
                return true;
            }
            index += 1;
        }
        false
    }

    fn store_pending_program(&mut self, pending: PendingProgramInvocation) -> bool {
        let _ = self.take_pending_program(pending.handle.pid);
        self.reclaim_reclaimable_pending_programs();
        let mut index = 0usize;
        while index < self.pending_programs.len() {
            if self.pending_programs[index].pending.is_none() {
                self.pending_programs[index].write(pending);
                return true;
            }
            index += 1;
        }
        false
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

    fn store_pending_payload(&mut self, pending: PendingPayloadInvocation) -> bool {
        let _ = self.take_pending_payload(pending.child.pid);
        self.reclaim_reclaimable_pending_payloads();
        let mut index = 0usize;
        while index < self.pending_payloads.len() {
            if self.pending_payloads[index].pending.is_none() {
                self.pending_payloads[index].write(pending);
                return true;
            }
            index += 1;
        }
        false
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
    exec_artifact::reset_cached_artifact_bodies();
    program::reset_media_programs();
    rootd::reset_media_payloads();
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
    try_spawn_bin_program_with_stdin(program, argv, stdin)
        .expect("root-shell program admission has a free exec slot")
}

/// Tries to start a root-shell `/bin` exec request with bounded stdin.
#[must_use]
pub fn try_spawn_bin_program_with_stdin(
    program: LoadedProgram,
    argv: ProgramArgvBuffer,
    stdin: &[u8],
) -> Option<ProcessHandle> {
    spawn_bin_program_for_parent(
        SHELL_PID,
        SHELL_PID,
        program,
        argv,
        ProgramEnvBuffer::empty(),
        stdin,
    )
}

/// Tries to start a root-shell `/bin` exec request with bounded env and stdin.
#[must_use]
pub fn try_spawn_bin_program_with_env_and_stdin(
    program: LoadedProgram,
    argv: ProgramArgvBuffer,
    env: ProgramEnvBuffer,
    stdin: &[u8],
) -> Option<ProcessHandle> {
    spawn_bin_program_for_parent(SHELL_PID, SHELL_PID, program, argv, env, stdin)
}

/// Starts a rootd-owned `/bin` exec request and leaves it pending for scheduler dispatch.
#[must_use]
pub fn spawn_rootd_bin_program_with_stdin(
    program: LoadedProgram,
    argv: ProgramArgvBuffer,
    stdin: &[u8],
) -> ProcessHandle {
    try_spawn_rootd_bin_program_with_stdin(program, argv, stdin)
        .expect("rootd program admission has a free exec slot")
}

/// Tries to start a rootd-owned `/bin` exec request.
#[must_use]
pub fn try_spawn_rootd_bin_program_with_stdin(
    program: LoadedProgram,
    argv: ProgramArgvBuffer,
    stdin: &[u8],
) -> Option<ProcessHandle> {
    spawn_bin_program_for_parent(
        ROOTD_PID,
        ROOTD_PID,
        program,
        argv,
        ProgramEnvBuffer::empty(),
        stdin,
    )
}

fn spawn_bin_program_for_parent(
    parent_pid: usize,
    parent_task_id: usize,
    program: LoadedProgram,
    argv: ProgramArgvBuffer,
    env: ProgramEnvBuffer,
    stdin: &[u8],
) -> Option<ProcessHandle> {
    if !with_exec(|exec| exec.has_pending_program_capacity()) {
        return None;
    }
    let (artifact_body_format, artifact_body_inner_format) =
        exec_artifact::program_body_formats(program);
    let handle = proc::spawn_child_with_program_argv_env_mapped(
        parent_pid,
        parent_task_id,
        program.descriptor.path,
        program.image_kind.as_str(),
        program.descriptor.entry_name,
        proc::address_space_image_for_program(program),
        &argv,
        &env,
        program.descriptor.name,
        artifact_body_format,
        artifact_body_inner_format,
    )?;
    if !with_exec(|exec| {
        exec.store_pending_program(PendingProgramInvocation::new(handle, program, argv, env, stdin))
    }) {
        let _ = proc::exit_process(handle.pid, ProcessState::Failed, 1);
        return None;
    }
    Some(handle)
}

/// Starts a child `/bin` exec request and leaves it pending for scheduler dispatch.
#[must_use]
pub fn spawn_program_child_with_stdin(
    parent: ProcessHandle,
    program: LoadedProgram,
    argv: ProgramArgvBuffer,
    stdin: &[u8],
) -> ProcessHandle {
    try_spawn_program_child_with_stdin(parent, program, argv, stdin)
        .expect("child program admission has a free exec slot")
}

/// Tries to start a child `/bin` exec request.
#[must_use]
pub fn try_spawn_program_child_with_stdin(
    parent: ProcessHandle,
    program: LoadedProgram,
    argv: ProgramArgvBuffer,
    stdin: &[u8],
) -> Option<ProcessHandle> {
    try_spawn_program_child_with_env_and_stdin(
        parent,
        program,
        argv,
        ProgramEnvBuffer::empty(),
        stdin,
    )
}

/// Tries to start a child `/bin` exec request with bounded env.
#[must_use]
pub fn try_spawn_program_child_with_env_and_stdin(
    parent: ProcessHandle,
    program: LoadedProgram,
    argv: ProgramArgvBuffer,
    env: ProgramEnvBuffer,
    stdin: &[u8],
) -> Option<ProcessHandle> {
    if !with_exec(|exec| exec.has_pending_program_capacity()) {
        return None;
    }
    let (artifact_body_format, artifact_body_inner_format) =
        exec_artifact::program_body_formats(program);
    let child = proc::spawn_child_with_program_argv_env_mapped(
        parent.pid,
        parent.task_id,
        program.descriptor.path,
        program.image_kind.as_str(),
        program.descriptor.entry_name,
        proc::address_space_image_for_program(program),
        &argv,
        &env,
        program.descriptor.name,
        artifact_body_format,
        artifact_body_inner_format,
    )?;
    if !with_exec(|exec| {
        exec.store_pending_program(PendingProgramInvocation::new(child, program, argv, env, stdin))
    }) {
        let _ = proc::exit_process(child.pid, ProcessState::Failed, 1);
        return None;
    }
    Some(child)
}

/// Replaces an existing process with a loaded `/bin` image and leaves it pending.
#[must_use]
pub fn replace_bin_program(
    current: ProcessHandle,
    program: LoadedProgram,
    argv: ProgramArgvBuffer,
    stdin: &[u8],
) -> Option<ProcessHandle> {
    replace_bin_program_with_env(current, program, argv, ProgramEnvBuffer::empty(), stdin)
}

/// Replaces an existing process with a loaded `/bin` image and retained env.
#[must_use]
pub fn replace_bin_program_with_env(
    current: ProcessHandle,
    program: LoadedProgram,
    argv: ProgramArgvBuffer,
    env: ProgramEnvBuffer,
    stdin: &[u8],
) -> Option<ProcessHandle> {
    if !with_exec(|exec| exec.has_pending_program_capacity_for_pid(current.pid)) {
        return None;
    }
    let (artifact_body_format, artifact_body_inner_format) =
        exec_artifact::program_body_formats(program);
    let handle = proc::replace_program_image_env_mapped(
        current.pid,
        program.descriptor.path,
        program.image_kind.as_str(),
        program.descriptor.entry_name,
        proc::address_space_image_for_program(program),
        &argv,
        &env,
        program.descriptor.name,
        artifact_body_format,
        artifact_body_inner_format,
    )?;
    if !with_exec(|exec| {
        exec.store_pending_program(PendingProgramInvocation::new(handle, program, argv, env, stdin))
    }) {
        let _ = proc::exit_process(handle.pid, ProcessState::Failed, 1);
        return None;
    }
    Some(handle)
}

/// Spawns a payload child and leaves its loaded image pending for dispatch.
#[must_use]
pub fn spawn_payload_child(parent: ProcessHandle, payload: LoadedPayloadProgram) -> ProcessHandle {
    let argv = ProgramArgvBuffer::from_argv0(payload.name).unwrap_or_else(|_| {
        let mut fallback = ProgramArgvBuffer::empty();
        let _ = fallback.push(payload.path);
        fallback
    });
    spawn_payload_child_with_argv(parent, payload, argv)
}

/// Spawns a payload child with admitted argv metadata.
#[must_use]
pub fn spawn_payload_child_with_argv(
    parent: ProcessHandle,
    payload: LoadedPayloadProgram,
    argv: ProgramArgvBuffer,
) -> ProcessHandle {
    try_spawn_payload_child_with_argv(parent, payload, argv)
        .expect("payload admission has a free exec slot")
}

/// Tries to spawn a payload child with admitted argv metadata.
#[must_use]
pub fn try_spawn_payload_child_with_argv(
    parent: ProcessHandle,
    payload: LoadedPayloadProgram,
    argv: ProgramArgvBuffer,
) -> Option<ProcessHandle> {
    try_spawn_payload_child_with_argv_env(parent, payload, argv, ProgramEnvBuffer::empty())
}

/// Tries to spawn a payload child with admitted argv/env metadata.
#[must_use]
pub fn try_spawn_payload_child_with_argv_env(
    parent: ProcessHandle,
    payload: LoadedPayloadProgram,
    argv: ProgramArgvBuffer,
    env: ProgramEnvBuffer,
) -> Option<ProcessHandle> {
    if !with_exec(|exec| exec.has_pending_payload_capacity()) {
        return None;
    }
    let (artifact_body_format, artifact_body_inner_format) =
        exec_artifact::payload_body_formats(payload);
    let child = proc::spawn_child_with_program_argv_env_mapped(
        parent.pid,
        parent.task_id,
        payload.path,
        payload.image_kind.as_str(),
        payload.entry_name,
        address_space_image_for_payload(payload),
        &argv,
        &env,
        payload.name,
        artifact_body_format,
        artifact_body_inner_format,
    )?;
    if !with_exec(|exec| {
        exec.store_pending_payload(PendingPayloadInvocation::new(parent, child, payload))
    }) {
        let _ = proc::exit_process(child.pid, ProcessState::Failed, 1);
        return None;
    }
    Some(child)
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
pub(crate) fn complete_program(pid: usize, status: ProgramStatus) -> ProcessAdoptionSnapshot {
    let (state, exit_code) = match status {
        ProgramStatus::Blocked => return ProcessAdoptionSnapshot::empty(),
        ProgramStatus::Empty => (ProcessState::Exited, 0),
        ProgramStatus::Ok => (ProcessState::Exited, 0),
        ProgramStatus::Error => (ProcessState::Failed, 1),
        ProgramStatus::ExitCode(0) => (ProcessState::Exited, 0),
        ProgramStatus::ExitCode(code) => (ProcessState::Failed, code),
        ProgramStatus::Replaced => (ProcessState::Exited, 0),
        ProgramStatus::Halt => (ProcessState::Halted, 0),
    };
    proc::exit_process(pid, state, exit_code)
}

/// Completes a payload child process.
pub fn complete_payload(pid: usize, result: PayloadLaunchResult) -> ProcessAdoptionSnapshot {
    let (state, exit_code) = match result {
        PayloadLaunchResult::Ready => (ProcessState::Exited, 0),
        PayloadLaunchResult::Resident => return ProcessAdoptionSnapshot::empty(),
        PayloadLaunchResult::NotConfigured | PayloadLaunchResult::Failed => {
            (ProcessState::Failed, 1)
        }
        PayloadLaunchResult::ExitCode(0) => (ProcessState::Exited, 0),
        PayloadLaunchResult::ExitCode(code) => (ProcessState::Failed, code as i32),
    };
    proc::exit_process(pid, state, exit_code)
}

#[cfg(feature = "selftest")]
#[path = "exec_tests.rs"]
mod tests;
