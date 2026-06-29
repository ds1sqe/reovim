//! Transitional Reovim syscall proof surface used by image-packaged programs.
//!
//! This is not the final trap entry and not a Linux syscall-number table. It
//! is the kernel-local raw backend plus diagnostics used by linked image `/bin`
//! programs and transitional source payload fixtures to reach process, VFS,
//! log, device, scheduler, and dump services. Domain uapi wrappers lower
//! through the raw `reovim-uapi-syscall` transport spine instead of growing this
//! module into a parallel command API.

use {
    crate::{
        dump, exec, exec_bundle, klog, mm,
        proc::{self, ProcessAdoptionSnapshot, ProcessHandle, ProcessRecord, WaitRecord},
        program::{
            self, BinSourceInstallStatus, LoadedProgram, MAX_PROGRAM_PIPE_BYTES,
            MAX_PROGRAM_STDIN_BYTES, ProgramArgvBuffer, ProgramArgvBuildError, ProgramDescriptor,
            ProgramEnvBuffer, ProgramEnvBuildError, ProgramStatus,
        },
        root_shell::{RootShellSession, execute_loaded_program_argv},
        rootd::{
            BootCheckState, BootImageSummary, ConsoleInputSummary, HardwareProbeResult,
            LoadedPayloadProgram, PayloadDescriptor, PayloadLaunchResult,
            PayloadSourceInstallStatus, PayloadSourceOp, PayloadSourceStep, RootDaemon,
        },
        sched::{self, KernelTaskRecord, SchedulerSnapshot},
        service::{self, EMPTY_SERVICE_RECORD, ServiceRecord},
        source_media,
        source_store::{
            self, EMPTY_SOURCE_ARTIFACT_RECORD, EMPTY_SOURCE_MEDIA_CATALOG_ENTRY,
            ExecutableSourceStore, MAX_SOURCE_ARTIFACT_RECORDS, MAX_SOURCE_MEDIA_ARTIFACT_BYTES,
            MAX_SOURCE_MEDIA_CATALOG_BYTES, MAX_SOURCE_MEDIA_CATALOG_RECORDS,
            SourceArtifactNamespace, SourceArtifactRecord, SourceInstallError,
        },
        vfs::{self, Directory, Node, PathBuf, VfsError},
    },
    core::{
        cell::UnsafeCell,
        mem::{align_of, size_of},
        ptr::NonNull,
        sync::atomic::{AtomicBool, Ordering},
    },
    reovim_uapi_dump::DumpSyncReport,
    reovim_uapi_fs::{DescriptorFlags, FileStatusFlags, OpenAtDir, OpenFlags, SeekWhence},
    reovim_uapi_process::{
        ProcessArg, ProcessControlReport, ProcessEnv, ProcessId, ProcessSleepReport,
        ProcessSpawnMode, ProcessSpawnRequest, ProcessSpawnTarget, ProcessStateCode,
        ProcessTimedWaitReport, ProcessWaitReport,
    },
    reovim_uapi_service::{
        ServiceControlOp, ServiceControlReport, ServiceControlResultCode, ServiceReasonCode,
        ServiceStateCode,
    },
    reovim_uapi_session::SessionControlOp,
    reovim_uapi_source::{
        SourceControlOp, SourceInstallOriginCode, SourceInstallReport, SourceInstallStatusCode,
        SourceNamespaceCode,
    },
    reovim_uapi_syscall::{SyscallArgs, SyscallError, SyscallNr, SyscallRet},
    reovim_uapi_system::{BootInfo, DeviceClass, DeviceEntry},
    reovim_uapi_terminal::{
        PRIMARY_OUTPUT, RawModeRequest, RawModeToken, TerminalError, TerminalId,
    },
};

pub use crate::program::ProgramArgv;

/// First non-stdio descriptor exposed by the current program ABI.
pub const PROGRAM_VFS_FILE_FD: usize = 3;
/// Bounded VFS pseudo-file descriptors available to one program syscall frame.
pub const MAX_PROGRAM_VFS_OPEN_FILES: usize = 4;
/// Bounded descriptors in the transitional program fd table.
pub const MAX_PROGRAM_FD_DESCRIPTORS: usize = PROGRAM_VFS_FILE_FD + MAX_PROGRAM_VFS_OPEN_FILES;
/// Maximum bytes readable from one opened VFS pseudo-file.
pub const MAX_PROGRAM_VFS_FILE_BYTES: usize = dump::MAX_DUMP_ARTIFACT_BYTES + 4096;
/// Maximum process waiters retained for one bounded pipe.
const MAX_PROGRAM_PIPE_WAITERS: usize = proc::MAX_PROCESSES;
const EMPTY_PROGRAM_DESCRIPTORS: [ProgramDescriptor; 0] = [];
const RAW_PROCESS_SPAWN_TARGET_BIN: usize = ProcessSpawnTarget::BIN.raw();
const RAW_PROCESS_SPAWN_TARGET_BIN_WITH_ENV: usize = ProcessSpawnTarget::BIN.with_env().raw();
const RAW_PROCESS_SPAWN_TARGET_BIN_WITH_REQUEST: usize =
    ProcessSpawnTarget::BIN.with_request().raw();
const RAW_PROCESS_SPAWN_TARGET_PAYLOAD: usize = ProcessSpawnTarget::PAYLOAD.raw();
const RAW_PROCESS_SPAWN_TARGET_PAYLOAD_WITH_REQUEST: usize =
    ProcessSpawnTarget::PAYLOAD.with_request().raw();
const RAW_PROCESS_SPAWN_MODE_READY: usize = ProcessSpawnMode::READY.raw();
const RAW_PROCESS_SPAWN_MODE_BLOCKED: usize = ProcessSpawnMode::BLOCKED.raw();
const RAW_PROCESS_SPAWN_MODE_SLEEPING: usize = ProcessSpawnMode::SLEEPING.raw();
const RAW_SERVICE_CONTROL_OP_STOP: usize = ServiceControlOp::STOP.raw();
const RAW_SERVICE_CONTROL_OP_START: usize = ServiceControlOp::START.raw();
const RAW_SERVICE_CONTROL_OP_RESTART: usize = ServiceControlOp::RESTART.raw();
const RAW_SERVICE_CONTROL_OP_REQUEST: usize = ServiceControlOp::REQUEST.raw();
const RAW_SESSION_CONTROL_OP_SHELL_START: usize = SessionControlOp::SHELL_START.raw();
const RAW_SESSION_CONTROL_OP_LINE_DISCIPLINE: usize = SessionControlOp::LINE_DISCIPLINE.raw();
const RAW_SOURCE_CONTROL_OP_INSTALL_BIN_STATUS: usize = SourceControlOp::INSTALL_BIN_STATUS.raw();
const RAW_SOURCE_CONTROL_OP_INSTALL_PAYLOAD_STATUS: usize =
    SourceControlOp::INSTALL_PAYLOAD_STATUS.raw();
const RAW_SOURCE_CONTROL_OP_INSTALL_BIN_MEDIA: usize = SourceControlOp::INSTALL_BIN_MEDIA.raw();
const RAW_SOURCE_CONTROL_OP_INSTALL_PAYLOAD_MEDIA: usize =
    SourceControlOp::INSTALL_PAYLOAD_MEDIA.raw();
const RAW_SOURCE_INSTALL_STATUS_NONE: usize = SourceInstallStatusCode::NONE.raw();
const RAW_SOURCE_INSTALL_STATUS_OK: usize = SourceInstallStatusCode::OK.raw();
const RAW_SOURCE_INSTALL_STATUS_ERROR: usize = SourceInstallStatusCode::ERROR.raw();
const RAW_SOURCE_INSTALL_STATUS_READY: usize = SourceInstallStatusCode::READY.raw();
const RAW_SOURCE_INSTALL_STATUS_FAILED: usize = SourceInstallStatusCode::FAILED.raw();

/// Bounded stdout capture used to feed one scheduled program's output into a
/// later program's stdin.
pub struct ProgramStdoutCapture {
    cell: UnsafeCell<ProgramStdoutCaptureState>,
}

struct ProgramVfsFileBuffer {
    cell: UnsafeCell<ProgramVfsFileBufferState>,
}

// SAFETY: mutable access is serialized by the matching
// `PROGRAM_VFS_FILE_BUFFER_LOCKS` slot.
unsafe impl Sync for ProgramVfsFileBuffer {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ProgramVfsFileBufferState {
    bytes: [u8; MAX_PROGRAM_VFS_FILE_BYTES],
    len: usize,
    truncated: bool,
}

impl ProgramVfsFileBufferState {
    const fn empty() -> Self {
        Self {
            bytes: [0u8; MAX_PROGRAM_VFS_FILE_BYTES],
            len: 0,
            truncated: false,
        }
    }
}

impl ProgramVfsFileBuffer {
    const fn new() -> Self {
        Self {
            cell: UnsafeCell::new(ProgramVfsFileBufferState::empty()),
        }
    }

    fn reset(&self) {
        // SAFETY: the global file-buffer lock is held while this buffer is in use.
        unsafe {
            *self.cell.get() = ProgramVfsFileBufferState::empty();
        }
    }

    fn write(&self, bytes: &[u8]) -> usize {
        // SAFETY: the global file-buffer lock is held while this buffer is in use.
        let state = unsafe { &mut *self.cell.get() };
        let mut written = 0usize;
        while written < bytes.len() && state.len < state.bytes.len() {
            state.bytes[state.len] = bytes[written];
            state.len += 1;
            written += 1;
        }
        if written < bytes.len() {
            state.truncated = true;
        }
        written
    }

    fn len(&self) -> usize {
        // SAFETY: immutable read while the owning program handle holds the lock.
        unsafe { (*self.cell.get()).len }
    }

    fn truncated(&self) -> bool {
        // SAFETY: immutable read while the owning program handle holds the lock.
        unsafe { (*self.cell.get()).truncated }
    }

    fn read_at(&self, cursor: usize, out: &mut [u8]) -> usize {
        // SAFETY: immutable read while the owning program handle holds the lock.
        let state = unsafe { &*self.cell.get() };
        let mut written = 0usize;
        while written < out.len() && cursor + written < state.len {
            out[written] = state.bytes[cursor + written];
            written += 1;
        }
        written
    }
}

static PROGRAM_VFS_FILE_BUFFERS: [ProgramVfsFileBuffer; MAX_PROGRAM_VFS_OPEN_FILES] =
    [const { ProgramVfsFileBuffer::new() }; MAX_PROGRAM_VFS_OPEN_FILES];
static PROGRAM_VFS_FILE_BUFFER_LOCKS: [AtomicBool; MAX_PROGRAM_VFS_OPEN_FILES] =
    [const { AtomicBool::new(false) }; MAX_PROGRAM_VFS_OPEN_FILES];

fn acquire_program_vfs_file_buffer() -> Option<(usize, NonNull<ProgramVfsFileBuffer>)> {
    let mut slot = 0usize;
    while slot < PROGRAM_VFS_FILE_BUFFER_LOCKS.len() {
        if PROGRAM_VFS_FILE_BUFFER_LOCKS[slot]
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            return Some((slot, NonNull::from(&PROGRAM_VFS_FILE_BUFFERS[slot])));
        }
        slot += 1;
    }
    None
}

fn release_program_vfs_file_buffer(slot: usize) {
    if let Some(lock) = PROGRAM_VFS_FILE_BUFFER_LOCKS.get(slot) {
        lock.store(false, Ordering::Release);
    }
}

const fn program_fd_for_descriptor_slot(slot: usize) -> usize {
    slot
}

fn program_descriptor_slot_for_fd(fd: usize) -> Option<usize> {
    if fd < MAX_PROGRAM_FD_DESCRIPTORS {
        Some(fd)
    } else {
        None
    }
}

// SAFETY: capture state is mutated only by the synchronous program dispatch
// frame that owns the matching syscall handle.
unsafe impl Sync for ProgramStdoutCapture {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ProgramStdoutCaptureState {
    bytes: [u8; MAX_PROGRAM_PIPE_BYTES],
    len: usize,
    truncated: bool,
}

impl ProgramStdoutCaptureState {
    const fn empty() -> Self {
        Self {
            bytes: [0u8; MAX_PROGRAM_PIPE_BYTES],
            len: 0,
            truncated: false,
        }
    }
}

impl ProgramStdoutCapture {
    /// Creates an empty stdout capture buffer.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            cell: UnsafeCell::new(ProgramStdoutCaptureState::empty()),
        }
    }

    /// Clears captured bytes.
    pub fn reset(&self) {
        // SAFETY: see the type-level synchronization note above.
        unsafe {
            *self.cell.get() = ProgramStdoutCaptureState::empty();
        }
    }

    fn write(&self, bytes: &[u8]) -> usize {
        // SAFETY: see the type-level synchronization note above.
        let state = unsafe { &mut *self.cell.get() };
        let mut written = 0usize;
        while written < bytes.len() && state.len < state.bytes.len() {
            state.bytes[state.len] = bytes[written];
            state.len += 1;
            written += 1;
        }
        if written < bytes.len() {
            state.truncated = true;
        }
        written
    }

    /// Copies captured bytes into `out` and returns the copied byte count.
    pub fn copy_into(&self, out: &mut [u8]) -> usize {
        // SAFETY: immutable snapshot after synchronous producer dispatch.
        let state = unsafe { &*self.cell.get() };
        let mut copied = 0usize;
        while copied < state.len && copied < out.len() {
            out[copied] = state.bytes[copied];
            copied += 1;
        }
        copied
    }

    /// Returns whether output was truncated by the bounded pipe buffer.
    #[must_use]
    pub fn truncated(&self) -> bool {
        // SAFETY: immutable snapshot after synchronous producer dispatch.
        unsafe { (*self.cell.get()).truncated }
    }
}

/// Maximum retained typed syscall dispatch records.
///
/// Operator transcripts can walk many pseudo-files through descriptor-shaped
/// `open`/`read`/`close`; keep enough history for lifecycle and domain rows to
/// survive alongside those fd records.
pub const MAX_SYSCALL_RECORDS: usize = 256;
/// Maximum active retained raw syscall continuations exposed to diagnostics.
pub const MAX_SYSCALL_CONTINUATION_RECORDS: usize = proc::MAX_PROCESSES;

/// System-kernel syscall operation recorded for `/bin` programs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyscallOp {
    /// Executable image metadata was loaded before spawn.
    ExecLoad,
    /// Root-shell exec spawned a `/bin` program process.
    ExecSpawn,
    /// Current process image was replaced by another `/bin` program.
    ExecReplace,
    /// Standard stream handles were attached to a program.
    StdioAttach,
    /// Program wrote to a file descriptor.
    FdWrite,
    /// Program read from a file descriptor.
    FdRead,
    /// A blocked raw syscall was retained or resumed.
    SyscallContinue,
    /// Program changed a file descriptor offset.
    FdSeek,
    /// Program duplicated a file descriptor.
    FdDuplicate,
    /// Program read descriptor-local flags.
    FdFlagsGet,
    /// Program replaced descriptor-local flags.
    FdFlagsSet,
    /// Program read opened-object status flags.
    FdStatusGet,
    /// Program replaced opened-object status flags.
    FdStatusSet,
    /// Program created a pipe descriptor pair.
    FdPipe,
    /// Program closed a file descriptor.
    FdClose,
    /// Child payload process spawn.
    SpawnChild,
    /// Process transitioned to running.
    ProcessRun,
    /// Current process record query.
    ProcessSelf,
    /// Process transitioned to blocked.
    ProcessBlock,
    /// Process transitioned to scheduler-timed sleep.
    ProcessSleep,
    /// Blocked process transitioned back to ready.
    ProcessWake,
    /// Retained process was explicitly terminated.
    ProcessKill,
    /// Scheduler selected a ready task for execution.
    SchedulerDispatch,
    /// Process exited.
    ProcessExit,
    /// Process requested exit through the raw syscall transport.
    ProcessExitRequest,
    /// Live child process was adopted by rootd after parent exit.
    ProcessAdopt,
    /// Parent process began waiting for a child.
    WaitBegin,
    /// Parent process completed waiting for a child.
    WaitEnd,
    /// Parent process stopped waiting because a child service reached readiness.
    WaitReady,
    /// Parent process stopped waiting because its tick deadline expired.
    WaitTimeout,
    /// Current working directory query.
    SessionCwdGet,
    /// Current working directory update.
    SessionCwdSet,
    /// Shell/session target request from init.
    SessionShellTarget,
    /// Interactive shell/session startup request from the shell target.
    SessionShellStart,
    /// Shell command-line discipline request from the shell target.
    SessionLineDiscipline,
    /// Init service target request.
    InitServiceStart,
    /// Current process published service readiness.
    ServiceReady,
    /// Current process became a resident service.
    ServiceHold,
    /// Retained service instance exited.
    ServiceExited,
    /// Retained service instance failed.
    ServiceFailed,
    /// Retained service instance was explicitly started.
    ServiceStart,
    /// Retained service instance was explicitly stopped.
    ServiceStop,
    /// Retained service instance was explicitly restarted.
    ServiceRestart,
    /// VFS path normalization.
    VfsNormalize,
    /// VFS path lookup.
    VfsLookup,
    /// VFS file open request.
    VfsOpen,
    /// VFS directory listing or file-name query.
    VfsList,
    /// VFS file/pseudo-file read.
    VfsRead,
    /// VFS mount-table snapshot.
    VfsMounts,
    /// Boot facts query.
    BootInfo,
    /// Boot image identity query.
    BootImage,
    /// Device catalog query.
    DeviceCatalog,
    /// Root profile query.
    BootProfile,
    /// Console input status query.
    ConsoleInput,
    /// Payload catalog query.
    PayloadCatalog,
    /// Payload image load request.
    PayloadLoad,
    /// Scheduled payload image execution.
    PayloadRun,
    /// Payload launch request.
    PayloadLaunch,
    /// Lower-provider probe request.
    ProviderProbe,
    /// Retained kernel-log byte read.
    KernelLogRead,
    /// Retained kernel-log stats query.
    KernelLogStats,
    /// Structured kernel event snapshot query.
    SnapshotKernelEvents,
    /// Live dump status query.
    DumpStatus,
    /// Persistent dump flush request.
    DumpSync,
    /// Process table snapshot query.
    SnapshotProcesses,
    /// Interactive shell session snapshot query.
    SnapshotSession,
    /// Service table snapshot query.
    SnapshotServices,
    /// Executable load/admission snapshot query.
    SnapshotExecLoads,
    /// Address-space lifecycle snapshot query.
    SnapshotAddressSpaces,
    /// Address-space page-table snapshot query.
    SnapshotAddressSpacePageTables,
    /// Address-space page-entry snapshot query.
    SnapshotAddressSpacePages,
    /// Address-space memory-object snapshot query.
    SnapshotAddressSpaceObjects,
    /// Pending executable invocation snapshot query.
    SnapshotPendingExecs,
    /// Executable source artifact snapshot query.
    SnapshotSourceStore,
    /// Runtime executable source artifact install request.
    SourceInstall,
    /// Executable source bytes read from source media.
    SourceMediaRead,
    /// Executable source-media manifest snapshot query.
    SourceMediaSnapshot,
    /// Executable bundle bytes read from the provider bundle.
    ExecBundleRead,
    /// Executable bundle manifest snapshot query.
    ExecBundleSnapshot,
    /// Process wait-table snapshot query.
    SnapshotWaits,
    /// Scheduler task table snapshot query.
    SnapshotTasks,
    /// Scheduler run-queue snapshot query.
    SchedulerSnapshot,
    /// Explicit scheduler tick.
    SchedulerTick,
    /// Cooperative scheduler yield.
    YieldNow,
    /// Syscall trace snapshot query.
    SnapshotSyscalls,
    /// Active raw syscall continuation snapshot query.
    SnapshotContinuations,
    /// TTY/console line read request.
    TtyReadLine,
    /// TTY/console clear request.
    TtyClear,
    /// TTY/terminal raw-mode entry request.
    TtyRawEnter,
    /// TTY/terminal raw-mode restore request.
    TtyRawRestore,
    /// TTY/terminal primary-input raw-mode restore request.
    TtyRawRestorePrimary,
    /// Current process requested system halt.
    SystemHalt,
    /// Image-owned `/bin/help` output request.
    ProgramHelp,
}

impl SyscallOp {
    /// Stable diagnostic word.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ExecLoad => "exec-load",
            Self::ExecSpawn => "exec-spawn",
            Self::ExecReplace => "exec-replace",
            Self::StdioAttach => "stdio-attach",
            Self::FdWrite => "fd-write",
            Self::FdRead => "fd-read",
            Self::SyscallContinue => "syscall-continue",
            Self::FdSeek => "fd-seek",
            Self::FdDuplicate => "fd-duplicate",
            Self::FdFlagsGet => "fd-flags-get",
            Self::FdFlagsSet => "fd-flags-set",
            Self::FdStatusGet => "fd-status-get",
            Self::FdStatusSet => "fd-status-set",
            Self::FdPipe => "fd-pipe",
            Self::FdClose => "fd-close",
            Self::SpawnChild => "spawn-child",
            Self::ProcessRun => "process-run",
            Self::ProcessSelf => "process-self",
            Self::ProcessBlock => "process-block",
            Self::ProcessSleep => "process-sleep",
            Self::ProcessWake => "process-wake",
            Self::ProcessKill => "process-kill",
            Self::SchedulerDispatch => "scheduler-dispatch",
            Self::ProcessExit => "process-exit",
            Self::ProcessExitRequest => "process-exit-request",
            Self::ProcessAdopt => "process-adopt",
            Self::WaitBegin => "wait-begin",
            Self::WaitEnd => "wait-end",
            Self::WaitReady => "wait-ready",
            Self::WaitTimeout => "wait-timeout",
            Self::SessionCwdGet => "session-cwd-get",
            Self::SessionCwdSet => "session-cwd-set",
            Self::SessionShellTarget => "session-shell-target",
            Self::SessionShellStart => "session-shell-start",
            Self::SessionLineDiscipline => "session-line-discipline",
            Self::InitServiceStart => "init-service-start",
            Self::ServiceReady => "service-ready",
            Self::ServiceHold => "service-hold",
            Self::ServiceExited => "service-exited",
            Self::ServiceFailed => "service-failed",
            Self::ServiceStart => "service-start",
            Self::ServiceStop => "service-stop",
            Self::ServiceRestart => "service-restart",
            Self::VfsNormalize => "vfs-normalize",
            Self::VfsLookup => "vfs-lookup",
            Self::VfsOpen => "vfs-open",
            Self::VfsList => "vfs-list",
            Self::VfsRead => "vfs-read",
            Self::VfsMounts => "vfs-mounts",
            Self::BootInfo => "boot-info",
            Self::BootImage => "boot-image",
            Self::DeviceCatalog => "device-catalog",
            Self::BootProfile => "boot-profile",
            Self::ConsoleInput => "console-input",
            Self::PayloadCatalog => "payload-catalog",
            Self::PayloadLoad => "payload-load",
            Self::PayloadRun => "payload-run",
            Self::PayloadLaunch => "payload-launch",
            Self::ProviderProbe => "provider-probe",
            Self::KernelLogRead => "kernel-log-read",
            Self::KernelLogStats => "kernel-log-stats",
            Self::SnapshotKernelEvents => "snapshot-kernel-events",
            Self::DumpStatus => "dump-status",
            Self::DumpSync => "dump-sync",
            Self::SnapshotProcesses => "snapshot-processes",
            Self::SnapshotSession => "snapshot-session",
            Self::SnapshotServices => "snapshot-services",
            Self::SnapshotExecLoads => "snapshot-exec-loads",
            Self::SnapshotAddressSpaces => "snapshot-address-spaces",
            Self::SnapshotAddressSpacePageTables => "snapshot-address-space-page-tables",
            Self::SnapshotAddressSpacePages => "snapshot-address-space-pages",
            Self::SnapshotAddressSpaceObjects => "snapshot-address-space-objects",
            Self::SnapshotPendingExecs => "snapshot-pending-execs",
            Self::SnapshotSourceStore => "snapshot-source-store",
            Self::SourceInstall => "source-install",
            Self::SourceMediaRead => "source-media-read",
            Self::SourceMediaSnapshot => "source-media-snapshot",
            Self::ExecBundleRead => "exec-bundle-read",
            Self::ExecBundleSnapshot => "exec-bundle-snapshot",
            Self::SnapshotWaits => "snapshot-waits",
            Self::SnapshotTasks => "snapshot-tasks",
            Self::SchedulerSnapshot => "scheduler-snapshot",
            Self::SchedulerTick => "scheduler-tick",
            Self::YieldNow => "yield-now",
            Self::SnapshotSyscalls => "snapshot-syscalls",
            Self::SnapshotContinuations => "snapshot-continuations",
            Self::TtyReadLine => "tty-read-line",
            Self::TtyClear => "tty-clear",
            Self::TtyRawEnter => "tty-raw-enter",
            Self::TtyRawRestore => "tty-raw-restore",
            Self::TtyRawRestorePrimary => "tty-raw-restore-primary",
            Self::SystemHalt => "system-halt",
            Self::ProgramHelp => "program-help",
        }
    }
}

/// Result status for one typed syscall dispatch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyscallStatus {
    /// Dispatch completed normally.
    Ok,
    /// Dispatch retained blocked work for later scheduler replay.
    Blocked,
    /// Dispatch reached the handler but failed.
    Error,
    /// Dispatch is not available for the current process context.
    Unavailable,
}

impl SyscallStatus {
    /// Stable diagnostic word.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Blocked => "blocked",
            Self::Error => "error",
            Self::Unavailable => "unavailable",
        }
    }
}

/// One retained typed syscall dispatch record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SyscallRecord {
    /// Monotonic syscall dispatch sequence.
    pub seq: usize,
    /// Calling process identifier, or `0` when not attached.
    pub process_id: usize,
    /// Calling task identifier, or `0` when not attached.
    pub task_id: usize,
    /// Calling executable path.
    pub program_path: &'static str,
    /// Loader/source kind for the executable image.
    pub loader: &'static str,
    /// Stable executable entry name.
    pub entry_name: &'static str,
    /// Dispatched operation.
    pub op: SyscallOp,
    /// Dispatch result status.
    pub status: SyscallStatus,
}

impl SyscallRecord {
    /// Empty syscall-table slot.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            seq: 0,
            process_id: 0,
            task_id: 0,
            program_path: "",
            loader: "",
            entry_name: "",
            op: SyscallOp::KernelLogStats,
            status: SyscallStatus::Unavailable,
        }
    }
}

/// Empty syscall record used for bounded snapshots.
pub const EMPTY_SYSCALL_RECORD: SyscallRecord = SyscallRecord::empty();

/// One active retained raw syscall continuation frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SyscallContinuationRecord {
    /// Calling process identifier.
    pub process_id: usize,
    /// Calling task identifier.
    pub task_id: usize,
    /// Calling executable path.
    pub program_path: &'static str,
    /// Loader/source kind for the executable image.
    pub loader: &'static str,
    /// Stable executable entry name.
    pub entry_name: &'static str,
    /// Retained raw syscall number.
    pub nr: SyscallNr,
    /// Semantic operation associated with `nr`.
    pub op: SyscallOp,
    /// Caller-memory assumption carried by the retained frame.
    pub memory: SyscallContinuationMemory,
    /// Retained raw syscall arguments.
    pub args: SyscallArgs,
}

/// Caller-memory policy represented by one retained raw syscall continuation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyscallContinuationMemory {
    /// The retained frame does not carry a caller-memory pointer.
    None,
    /// The direct backend retained a raw caller read buffer pointer.
    RawReadBuffer,
    /// The direct backend retained a raw caller write buffer pointer.
    RawWriteBuffer,
    /// The direct backend retained a raw caller wait-report pointer.
    RawWaitReport,
}

impl SyscallContinuationMemory {
    /// Stable diagnostic word.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::RawReadBuffer => "raw-read-buffer",
            Self::RawWriteBuffer => "raw-write-buffer",
            Self::RawWaitReport => "raw-wait-report",
        }
    }
}

impl SyscallContinuationRecord {
    /// Empty continuation-table slot.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            process_id: 0,
            task_id: 0,
            program_path: "",
            loader: "",
            entry_name: "",
            nr: SyscallNr::INVALID,
            op: SyscallOp::SyscallContinue,
            memory: SyscallContinuationMemory::None,
            args: SyscallArgs::EMPTY,
        }
    }
}

/// Empty syscall continuation record used for bounded snapshots.
pub const EMPTY_SYSCALL_CONTINUATION_RECORD: SyscallContinuationRecord =
    SyscallContinuationRecord::empty();

struct SyscallTrace {
    records: [SyscallRecord; MAX_SYSCALL_RECORDS],
    next_seq: usize,
}

impl SyscallTrace {
    const fn new() -> Self {
        Self {
            records: [SyscallRecord::empty(); MAX_SYSCALL_RECORDS],
            next_seq: 1,
        }
    }

    fn reset(&mut self) {
        self.records = [SyscallRecord::empty(); MAX_SYSCALL_RECORDS];
        self.next_seq = 1;
    }

    fn record(&mut self, current: Option<SyscallContext>, op: SyscallOp, status: SyscallStatus) {
        let seq = self.next_seq;
        self.next_seq = self.next_seq.saturating_add(1);
        let (process_id, task_id, program_path) = match current {
            Some(ctx) => (ctx.pid, ctx.task_id, ctx.program_path),
            None => (0, 0, ""),
        };
        let (loader, entry_name) = match current {
            Some(ctx) => (ctx.loader, ctx.entry_name),
            None => ("", ""),
        };
        let slot = (seq.saturating_sub(1)) % self.records.len();
        self.records[slot] = SyscallRecord {
            seq,
            process_id,
            task_id,
            program_path,
            loader,
            entry_name,
            op,
            status,
        };
    }

    fn snapshot(&self, out: &mut [SyscallRecord]) -> usize {
        let total = self.next_seq.saturating_sub(1);
        let retained = core::cmp::min(total, self.records.len());
        let first_seq = total.saturating_sub(retained).saturating_add(1);
        let mut written = 0usize;
        let mut seq = first_seq;
        while seq <= total && written < out.len() {
            let slot = (seq.saturating_sub(1)) % self.records.len();
            let record = self.records[slot];
            if record.seq == seq {
                out[written] = record;
                written += 1;
            }
            seq += 1;
        }
        written
    }
}

struct SyscallCell(UnsafeCell<SyscallTrace>);

// SAFETY: access is serialized by `SYSCALL_LOCK`.
unsafe impl Sync for SyscallCell {}

static SYSCALLS: SyscallCell = SyscallCell(UnsafeCell::new(SyscallTrace::new()));
static SYSCALL_LOCK: AtomicBool = AtomicBool::new(false);

struct SyscallGuard;

impl SyscallGuard {
    fn acquire() -> Self {
        while SYSCALL_LOCK
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }
        Self
    }
}

impl Drop for SyscallGuard {
    fn drop(&mut self) {
        SYSCALL_LOCK.store(false, Ordering::Release);
    }
}

fn with_syscalls<R>(f: impl FnOnce(&mut SyscallTrace) -> R) -> R {
    let _guard = SyscallGuard::acquire();
    // SAFETY: `SYSCALL_LOCK` serializes access to the syscall trace.
    let trace = unsafe { &mut *SYSCALLS.0.get() };
    f(trace)
}

/// Clears the retained typed syscall dispatch records.
pub fn reset() {
    exec::reset();
    program::reset_bin_uapi_resume_frames();
    crate::rootd::reset_shell_pipeline_continuations();
    reset_payload_source_continuations();
    reset_process_fd_tables();
    reset_program_syscall_continuations();
    reset_program_open_file_table();
    reset_program_pipe_table();
    with_syscalls(SyscallTrace::reset);
}

/// Copies retained syscall dispatch records into `out`, returning the count.
pub fn snapshot_syscalls(out: &mut [SyscallRecord]) -> usize {
    with_syscalls(|trace| trace.snapshot(out))
}

fn record_syscall(current: Option<SyscallContext>, op: SyscallOp, status: SyscallStatus) {
    with_syscalls(|trace| trace.record(current, op, status));
    if op == SyscallOp::SyscallContinue {
        append_syscall_continue_event(current, status);
    }
}

fn record_context(ctx: SyscallContext, op: SyscallOp, status: SyscallStatus) {
    record_syscall(Some(ctx), op, status);
}

fn append_syscall_continue_event(current: Option<SyscallContext>, status: SyscallStatus) {
    let Some(ctx) = current else {
        return;
    };
    klog::append_event_with_source_context(
        "process",
        "syscall",
        syscall_continue_event_severity(status),
        syscall_continue_event_kind(status),
        ctx.pid,
        ctx.task_id,
    );
}

const fn syscall_continue_event_kind(status: SyscallStatus) -> &'static str {
    match status {
        SyscallStatus::Ok => "syscall-continue-ok",
        SyscallStatus::Error => "syscall-continue-error",
        SyscallStatus::Unavailable => "syscall-continue-unavailable",
        SyscallStatus::Blocked => "syscall-continue-blocked",
    }
}

const fn syscall_continue_event_severity(status: SyscallStatus) -> &'static str {
    match status {
        SyscallStatus::Ok | SyscallStatus::Blocked => "info",
        SyscallStatus::Error | SyscallStatus::Unavailable => "warn",
    }
}

const fn rootd_syscall_context() -> SyscallContext {
    SyscallContext {
        pid: proc::ROOTD_PID,
        task_id: 1,
        program_path: "rootd",
        loader: "kernel",
        entry_name: "rootd_main",
    }
}

const fn dump_sync_syscall_status(status: dump::DumpSyncStatus) -> SyscallStatus {
    if status.written {
        SyscallStatus::Ok
    } else if status.persistent_available {
        SyscallStatus::Error
    } else {
        SyscallStatus::Unavailable
    }
}

/// Attempts a root-daemon dump flush and records it as a typed Reovim syscall.
pub fn sync_dump_from_rootd(
    image: dump::DumpImageIdentity,
    boot_info: reovim_uapi_system::BootInfo,
    devices: &[reovim_uapi_system::DeviceEntry],
) -> dump::DumpSyncStatus {
    let status = dump::sync_with_context(image, boot_info, devices);
    record_context(rootd_syscall_context(), SyscallOp::DumpSync, dump_sync_syscall_status(status));
    status
}

const fn process_exit_syscall_status(status: ProgramStatus) -> SyscallStatus {
    match status {
        ProgramStatus::Error => SyscallStatus::Error,
        ProgramStatus::Blocked => SyscallStatus::Blocked,
        ProgramStatus::ExitCode(code) => {
            if code == 0 {
                SyscallStatus::Ok
            } else {
                SyscallStatus::Error
            }
        }
        ProgramStatus::Empty
        | ProgramStatus::Ok
        | ProgramStatus::Replaced
        | ProgramStatus::Halt => SyscallStatus::Ok,
    }
}

const fn process_run_status(handle: Option<ProcessHandle>, expected_pid: usize) -> SyscallStatus {
    match handle {
        Some(handle) if handle.pid == expected_pid => SyscallStatus::Ok,
        Some(_) | None => SyscallStatus::Error,
    }
}

const fn payload_syscall_status(result: PayloadLaunchResult) -> SyscallStatus {
    match result {
        PayloadLaunchResult::Ready => SyscallStatus::Ok,
        PayloadLaunchResult::Resident => SyscallStatus::Ok,
        PayloadLaunchResult::NotConfigured => SyscallStatus::Unavailable,
        PayloadLaunchResult::Failed => SyscallStatus::Error,
        PayloadLaunchResult::ExitCode(code) => {
            if code == 0 {
                SyscallStatus::Ok
            } else {
                SyscallStatus::Error
            }
        }
    }
}

const fn terminal_syscall_status(error: TerminalError) -> SyscallStatus {
    match error {
        TerminalError::NotTerminal | TerminalError::Unsupported => SyscallStatus::Unavailable,
        TerminalError::InvalidTarget | TerminalError::Busy | TerminalError::PermissionDenied => {
            SyscallStatus::Error
        }
    }
}

const fn syscall_error_from_terminal(error: TerminalError) -> SyscallError {
    match error {
        TerminalError::NotTerminal => SyscallError::BAD_DESCRIPTOR,
        TerminalError::Unsupported => SyscallError::UNSUPPORTED,
        TerminalError::InvalidTarget => SyscallError::INVALID_ARGUMENT,
        TerminalError::Busy => SyscallError::BUSY,
        TerminalError::PermissionDenied => SyscallError::PROTECTED_PROCESS,
    }
}

const fn payload_result_from_child_status(status: ProgramStatus) -> PayloadLaunchResult {
    match status {
        ProgramStatus::Ok | ProgramStatus::Replaced | ProgramStatus::ExitCode(0) => {
            PayloadLaunchResult::Ready
        }
        ProgramStatus::ExitCode(code) if code > 0 && code <= u8::MAX as i32 => {
            PayloadLaunchResult::ExitCode(code as u8)
        }
        ProgramStatus::Empty
        | ProgramStatus::Error
        | ProgramStatus::ExitCode(_)
        | ProgramStatus::Blocked
        | ProgramStatus::Halt => PayloadLaunchResult::Failed,
    }
}

const fn payload_result_from_wait_exit(wait: WaitRecord) -> PayloadLaunchResult {
    if !wait.completed {
        return PayloadLaunchResult::Failed;
    }
    match wait.exit_code {
        0 => PayloadLaunchResult::Ready,
        code if code > 0 && code <= u8::MAX as i32 => PayloadLaunchResult::ExitCode(code as u8),
        _ => PayloadLaunchResult::Failed,
    }
}

const fn payload_result_from_timed_wait_exit(wait: ProcessTimedWaitResult) -> PayloadLaunchResult {
    if wait.timed_out || !wait.completed {
        return PayloadLaunchResult::Failed;
    }
    match wait.exit_code {
        0 => PayloadLaunchResult::Ready,
        code if code > 0 && code <= u8::MAX as i32 => PayloadLaunchResult::ExitCode(code as u8),
        _ => PayloadLaunchResult::Failed,
    }
}

const fn input_state_word(state: BootCheckState) -> &'static [u8] {
    match state {
        BootCheckState::Ok => b"ready",
        BootCheckState::Warn => b"unavailable",
    }
}

fn manual_next_step(input: ConsoleInputSummary) -> &'static [u8] {
    if input.usb_keyboard == BootCheckState::Ok {
        b"type-shell-command"
    } else if input.usb_keyboard_probe_enabled {
        match input.usb_keyboard_last_poll {
            "device-tree-disabled-or-missing"
            | "xhci-controller-not-ready"
            | "xhci-reset-in-progress"
            | "xhci-host-system-error"
            | "no-pcie-xhci-controller"
            | "xhci-bar-unconfigured"
            | "xhci-capabilities-invalid"
            | "no-connected-root-port" => b"probe-pcie",
            "needs-controller-init" => b"probe-xhci-start",
            "needs-enumeration" | "hid-enumeration-failed" => b"probe-xhci-read-keyboard-report",
            "keyboard-report-event-mismatch" | "keyboard-report-transfer-failed" => {
                b"probe-xhci-read-keyboard-report"
            }
            "decoded-pending" | "report-ready" => b"type-shell-command",
            "report-pending" => b"press-usb-key",
            "not-polled" | "unknown" => b"probe-usb-keyboard",
            _ => b"probe-usb-keyboard",
        }
    } else {
        b"probe-help"
    }
}

/// Standard program stream kind exposed to image-packaged `/bin` programs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProgramStream {
    /// Program stdin.
    Stdin,
    /// Program stdout.
    Stdout,
    /// Program stderr.
    Stderr,
}

impl ProgramStream {
    /// Stable stream name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Stdin => "stdin",
            Self::Stdout => "stdout",
            Self::Stderr => "stderr",
        }
    }

    /// Reovim standard stream descriptor number.
    #[must_use]
    pub const fn fd(self) -> usize {
        match self {
            Self::Stdin => 0,
            Self::Stdout => 1,
            Self::Stderr => 2,
        }
    }

    /// Whether this stream currently accepts reads.
    #[must_use]
    pub const fn readable(self) -> bool {
        matches!(self, Self::Stdin)
    }

    /// Whether this stream currently accepts writes.
    #[must_use]
    pub const fn writable(self) -> bool {
        matches!(self, Self::Stdout | Self::Stderr)
    }
}

/// One standard program stream handle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProgramStreamHandle {
    /// Descriptor number in the Reovim program ABI.
    pub fd: usize,
    /// Stream kind attached to the descriptor.
    pub stream: ProgramStream,
}

impl ProgramStreamHandle {
    /// Creates a standard stream handle for `stream`.
    #[must_use]
    pub const fn new(stream: ProgramStream) -> Self {
        Self {
            fd: stream.fd(),
            stream,
        }
    }

    /// Stable stream name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        self.stream.name()
    }

    /// Whether this handle currently accepts reads.
    #[must_use]
    pub const fn readable(self) -> bool {
        self.stream.readable()
    }

    /// Whether this handle currently accepts writes.
    #[must_use]
    pub const fn writable(self) -> bool {
        self.stream.writable()
    }
}

/// Standard stream table attached to one image-program invocation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProgramStdio {
    /// Standard input descriptor.
    pub stdin: ProgramStreamHandle,
    /// Standard output descriptor.
    pub stdout: ProgramStreamHandle,
    /// Standard error descriptor.
    pub stderr: ProgramStreamHandle,
}

impl ProgramStdio {
    /// Creates the standard Reovim program stream table.
    #[must_use]
    pub const fn standard() -> Self {
        Self {
            stdin: ProgramStreamHandle::new(ProgramStream::Stdin),
            stdout: ProgramStreamHandle::new(ProgramStream::Stdout),
            stderr: ProgramStreamHandle::new(ProgramStream::Stderr),
        }
    }
}

/// Program fd write failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProgramIoError {
    /// The fd is not attached to this process.
    BadFd,
    /// The fd operation arguments are invalid.
    InvalidArgument,
    /// No descriptor or open-file-description slot is available.
    Busy,
    /// Retained blocking state could not be admitted.
    NoSpace,
    /// The operation would block and the opened object is nonblocking.
    WouldBlock,
    /// The fd exists but is not readable.
    NotReadable,
    /// The fd exists but is not writable.
    NotWritable,
    /// The fd exists but does not support seek.
    NotSeekable,
    /// The fd exists but is not a directory stream.
    NotDirectory,
}

impl ProgramIoError {
    /// Stable diagnostic word.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BadFd => "bad-fd",
            Self::InvalidArgument => "invalid-argument",
            Self::Busy => "busy",
            Self::NoSpace => "no-space",
            Self::WouldBlock => "would-block",
            Self::NotReadable => "not-readable",
            Self::NotWritable => "not-writable",
            Self::NotSeekable => "not-seekable",
            Self::NotDirectory => "not-directory",
        }
    }
}

const fn syscall_error_from_program_io(error: ProgramIoError) -> SyscallError {
    match error {
        ProgramIoError::BadFd => SyscallError::BAD_DESCRIPTOR,
        ProgramIoError::InvalidArgument => SyscallError::INVALID_ARGUMENT,
        ProgramIoError::Busy | ProgramIoError::NoSpace => SyscallError::BUSY,
        ProgramIoError::WouldBlock => SyscallError::WOULD_BLOCK,
        ProgramIoError::NotReadable => SyscallError::NOT_READABLE,
        ProgramIoError::NotWritable => SyscallError::NOT_WRITABLE,
        ProgramIoError::NotSeekable => SyscallError::NOT_SEEKABLE,
        ProgramIoError::NotDirectory => SyscallError::NOT_DIRECTORY,
    }
}

const fn syscall_error_from_vfs(error: VfsError) -> SyscallError {
    match error {
        VfsError::EmptyPath | VfsError::TooLong => SyscallError::INVALID_ARGUMENT,
        VfsError::NotFound => SyscallError::NOT_FOUND,
        VfsError::NotDirectory => SyscallError::NOT_DIRECTORY,
        VfsError::NotWritable => SyscallError::NOT_WRITABLE,
        VfsError::Busy => SyscallError::BUSY,
        VfsError::FileTooLarge => SyscallError::FILE_TOO_LARGE,
        VfsError::Io => SyscallError::IO,
    }
}

const fn syscall_error_from_program_argv(error: ProgramArgvBuildError) -> SyscallError {
    match error {
        ProgramArgvBuildError::TooManyArgs | ProgramArgvBuildError::ArgTooLong => {
            SyscallError::INVALID_ARGUMENT
        }
    }
}

const fn syscall_error_from_program_env(error: ProgramEnvBuildError) -> SyscallError {
    match error {
        ProgramEnvBuildError::TooManyVars
        | ProgramEnvBuildError::InvalidName
        | ProgramEnvBuildError::NameTooLong
        | ProgramEnvBuildError::ValueTooLong => SyscallError::INVALID_ARGUMENT,
    }
}

fn raw_open_at_dir_arg(dir: OpenAtDir) -> usize {
    dir.raw() as usize
}

const fn raw_seek_offset_arg(offset: usize) -> isize {
    offset as isize
}

const fn raw_seek_whence_arg(whence: usize) -> Option<SeekWhence> {
    if whence == SeekWhence::START.raw() as usize {
        Some(SeekWhence::START)
    } else if whence == SeekWhence::CURRENT.raw() as usize {
        Some(SeekWhence::CURRENT)
    } else if whence == SeekWhence::END.raw() as usize {
        Some(SeekWhence::END)
    } else {
        None
    }
}

/// # Safety
///
/// `ptr..ptr+len` must identify caller-owned writable memory for the duration
/// of this synchronous dispatch. Future trap entry must replace this harness
/// helper with architecture memory validation.
unsafe fn raw_syscall_read_buffer<'a>(
    ptr: usize,
    len: usize,
) -> Result<&'a mut [u8], SyscallError> {
    if len == 0 {
        // SAFETY: zero-length slices do not dereference the dangling address.
        return Ok(unsafe {
            core::slice::from_raw_parts_mut(NonNull::<u8>::dangling().as_ptr(), 0)
        });
    }
    if ptr == 0 {
        return Err(SyscallError::INVALID_ARGUMENT);
    }
    // SAFETY: raw syscall dispatch is synchronous. The current source-image
    // harness and linked-bin proof path pass host-valid buffers; future trap
    // entry must replace this with architecture memory validation.
    Ok(unsafe { core::slice::from_raw_parts_mut(ptr as *mut u8, len) })
}

/// # Safety
///
/// `ptr..ptr+len` must identify caller-owned readable memory for the duration
/// of this synchronous dispatch. Future trap entry must replace this harness
/// helper with architecture memory validation.
unsafe fn raw_syscall_write_buffer<'a>(ptr: usize, len: usize) -> Result<&'a [u8], SyscallError> {
    if len == 0 {
        // SAFETY: zero-length slices do not dereference the dangling address.
        return Ok(unsafe { core::slice::from_raw_parts(NonNull::<u8>::dangling().as_ptr(), 0) });
    }
    if ptr == 0 {
        return Err(SyscallError::INVALID_ARGUMENT);
    }
    // SAFETY: see `raw_syscall_read_buffer`; this is the read-only counterpart
    // for the same temporary synchronous dispatch path.
    Ok(unsafe { core::slice::from_raw_parts(ptr as *const u8, len) })
}

/// # Safety
///
/// `ptr..ptr+(argc * size_of::<ProcessArg>())` must identify caller-owned
/// readable memory for the duration of this synchronous dispatch. Future trap
/// entry must replace this harness helper with architecture memory validation.
unsafe fn raw_syscall_process_args<'a>(
    ptr: usize,
    argc: usize,
) -> Result<&'a [ProcessArg], SyscallError> {
    if argc == 0 {
        // SAFETY: zero-length slices do not dereference the dangling address.
        return Ok(unsafe {
            core::slice::from_raw_parts(NonNull::<ProcessArg>::dangling().as_ptr(), 0)
        });
    }
    if ptr == 0 {
        return Err(SyscallError::INVALID_ARGUMENT);
    }
    // SAFETY: raw syscall dispatch is synchronous and linked-bin callers pass a
    // live `ProcessArg` slice. Future trap entry must validate caller memory.
    Ok(unsafe { core::slice::from_raw_parts(ptr as *const ProcessArg, argc) })
}

/// # Safety
///
/// `ptr..ptr+(envc * size_of::<ProcessEnv>())` must identify caller-owned
/// readable memory for the duration of this synchronous dispatch. Future trap
/// entry must replace this harness helper with architecture memory validation.
unsafe fn raw_syscall_process_env<'a>(
    ptr: usize,
    envc: usize,
) -> Result<&'a [ProcessEnv], SyscallError> {
    if envc == 0 {
        // SAFETY: zero-length slices do not dereference the dangling address.
        return Ok(unsafe {
            core::slice::from_raw_parts(NonNull::<ProcessEnv>::dangling().as_ptr(), 0)
        });
    }
    if ptr == 0 {
        return Err(SyscallError::INVALID_ARGUMENT);
    }
    // SAFETY: raw syscall dispatch is synchronous and linked-bin callers pass a
    // live `ProcessEnv` slice. Future trap entry must validate caller memory.
    Ok(unsafe { core::slice::from_raw_parts(ptr as *const ProcessEnv, envc) })
}

/// # Safety
///
/// `ptr..ptr+len` must identify one caller-owned readable
/// [`ProcessSpawnRequest`] for the duration of this synchronous dispatch.
/// Future trap entry must replace this harness helper with architecture memory
/// validation.
unsafe fn raw_syscall_process_spawn_request<'a>(
    ptr: usize,
    len: usize,
) -> Result<&'a ProcessSpawnRequest, SyscallError> {
    if ptr == 0
        || len != size_of::<ProcessSpawnRequest>()
        || ptr % align_of::<ProcessSpawnRequest>() != 0
    {
        return Err(SyscallError::INVALID_ARGUMENT);
    }
    // SAFETY: raw syscall dispatch is synchronous and linked-bin callers pass a
    // live `ProcessSpawnRequest`. Future trap entry must validate caller memory.
    Ok(unsafe { &*(ptr as *const ProcessSpawnRequest) })
}

/// # Safety
///
/// `ptr..ptr+(2 * size_of::<i32>())` must identify caller-owned writable memory
/// for the duration of this synchronous dispatch. Future trap entry must
/// replace this harness helper with architecture memory validation.
unsafe fn raw_syscall_pipe_fds<'a>(
    ptr: usize,
    count: usize,
) -> Result<&'a mut [i32], SyscallError> {
    if count != 2 || ptr == 0 || ptr % align_of::<i32>() != 0 {
        return Err(SyscallError::INVALID_ARGUMENT);
    }
    // SAFETY: raw syscall dispatch is synchronous and linked-bin callers pass a
    // live two-element fd array. Future trap entry must validate caller memory.
    Ok(unsafe { core::slice::from_raw_parts_mut(ptr as *mut i32, count) })
}

/// # Safety
///
/// `ptr..ptr+size_of::<DumpSyncReport>()` must identify caller-owned writable
/// memory for the duration of this synchronous dispatch. Future trap entry must
/// replace this harness helper with architecture memory validation.
unsafe fn raw_syscall_dump_sync_report<'a>(
    ptr: usize,
    len: usize,
) -> Result<&'a mut DumpSyncReport, SyscallError> {
    if len != size_of::<DumpSyncReport>() || ptr == 0 || ptr % align_of::<DumpSyncReport>() != 0 {
        return Err(SyscallError::INVALID_ARGUMENT);
    }
    // SAFETY: raw syscall dispatch is synchronous and linked-bin callers pass a
    // live `DumpSyncReport`. Future trap entry must validate caller memory.
    Ok(unsafe { &mut *(ptr as *mut DumpSyncReport) })
}

/// # Safety
///
/// `ptr..ptr+size_of::<ProcessSleepReport>()` must identify caller-owned
/// writable memory for the duration of this synchronous dispatch. Future trap
/// entry must replace this harness helper with architecture memory validation.
unsafe fn raw_syscall_process_sleep_report<'a>(
    ptr: usize,
    len: usize,
) -> Result<&'a mut ProcessSleepReport, SyscallError> {
    if len != size_of::<ProcessSleepReport>()
        || ptr == 0
        || ptr % align_of::<ProcessSleepReport>() != 0
    {
        return Err(SyscallError::INVALID_ARGUMENT);
    }
    // SAFETY: raw syscall dispatch is synchronous and linked-bin callers pass a
    // live `ProcessSleepReport`. Future trap entry must validate caller memory.
    Ok(unsafe { &mut *(ptr as *mut ProcessSleepReport) })
}

/// # Safety
///
/// When present, `ptr..ptr+size_of::<ProcessControlReport>()` must identify
/// caller-owned writable memory for the duration of this synchronous dispatch.
/// Future trap entry must replace this harness helper with architecture memory
/// validation.
unsafe fn raw_syscall_optional_process_control_report<'a>(
    ptr: usize,
    len: usize,
) -> Result<Option<&'a mut ProcessControlReport>, SyscallError> {
    if ptr == 0 && len == 0 {
        return Ok(None);
    }
    if len != size_of::<ProcessControlReport>()
        || ptr == 0
        || ptr % align_of::<ProcessControlReport>() != 0
    {
        return Err(SyscallError::INVALID_ARGUMENT);
    }
    // SAFETY: raw syscall dispatch is synchronous and linked-bin callers pass a
    // live `ProcessControlReport`. Future trap entry must validate caller
    // memory.
    Ok(Some(unsafe { &mut *(ptr as *mut ProcessControlReport) }))
}

/// # Safety
///
/// `ptr..ptr+size_of::<ProcessControlReport>()` must identify caller-owned
/// writable memory for the duration of this synchronous dispatch. Future trap
/// entry must replace this harness helper with architecture memory validation.
unsafe fn raw_syscall_process_control_report<'a>(
    ptr: usize,
    len: usize,
) -> Result<&'a mut ProcessControlReport, SyscallError> {
    match unsafe { raw_syscall_optional_process_control_report(ptr, len) } {
        Ok(Some(report)) => Ok(report),
        Ok(None) => Err(SyscallError::INVALID_ARGUMENT),
        Err(error) => Err(error),
    }
}

/// # Safety
///
/// `ptr..ptr+size_of::<ProcessWaitReport>()` must identify caller-owned
/// writable memory for the duration of this synchronous dispatch when `ptr` is
/// nonzero. Future trap entry must replace this harness helper with
/// architecture memory validation.
unsafe fn raw_syscall_optional_process_wait_report<'a>(
    ptr: usize,
    len: usize,
) -> Result<Option<&'a mut ProcessWaitReport>, SyscallError> {
    if ptr == 0 && len == 0 {
        return Ok(None);
    }
    if len != size_of::<ProcessWaitReport>()
        || ptr == 0
        || ptr % align_of::<ProcessWaitReport>() != 0
    {
        return Err(SyscallError::INVALID_ARGUMENT);
    }
    // SAFETY: raw syscall dispatch is synchronous and linked-bin callers pass a
    // live `ProcessWaitReport`. Future trap entry must validate caller memory.
    Ok(Some(unsafe { &mut *(ptr as *mut ProcessWaitReport) }))
}

/// # Safety
///
/// `ptr..ptr+size_of::<ProcessTimedWaitReport>()` must identify caller-owned
/// writable memory for the duration of this synchronous dispatch. Future trap
/// entry must replace this harness helper with architecture memory validation.
unsafe fn raw_syscall_process_timed_wait_report<'a>(
    ptr: usize,
    len: usize,
) -> Result<&'a mut ProcessTimedWaitReport, SyscallError> {
    if len != size_of::<ProcessTimedWaitReport>()
        || ptr == 0
        || ptr % align_of::<ProcessTimedWaitReport>() != 0
    {
        return Err(SyscallError::INVALID_ARGUMENT);
    }
    // SAFETY: raw syscall dispatch is synchronous and linked-bin callers pass a
    // live `ProcessTimedWaitReport`. Future trap entry must validate caller
    // memory.
    Ok(unsafe { &mut *(ptr as *mut ProcessTimedWaitReport) })
}

/// # Safety
///
/// `ptr..ptr+size_of::<ServiceControlReport>()` must identify caller-owned
/// writable memory for the duration of this synchronous dispatch. Future trap
/// entry must replace this harness helper with architecture memory validation.
unsafe fn raw_syscall_service_control_report<'a>(
    ptr: usize,
    len: usize,
) -> Result<&'a mut ServiceControlReport, SyscallError> {
    if len != size_of::<ServiceControlReport>()
        || ptr == 0
        || ptr % align_of::<ServiceControlReport>() != 0
    {
        return Err(SyscallError::INVALID_ARGUMENT);
    }
    // SAFETY: raw syscall dispatch is synchronous and linked-bin callers pass a
    // live `ServiceControlReport`. Future trap entry must validate caller
    // memory.
    Ok(unsafe { &mut *(ptr as *mut ServiceControlReport) })
}

/// # Safety
///
/// `ptr..ptr+size_of::<SourceInstallReport>()` must identify caller-owned
/// writable memory for the duration of this synchronous dispatch. Future trap
/// entry must replace this harness helper with architecture memory validation.
unsafe fn raw_syscall_source_install_report<'a>(
    ptr: usize,
    len: usize,
) -> Result<&'a mut SourceInstallReport, SyscallError> {
    if len != size_of::<SourceInstallReport>()
        || ptr == 0
        || ptr % align_of::<SourceInstallReport>() != 0
    {
        return Err(SyscallError::INVALID_ARGUMENT);
    }
    // SAFETY: raw syscall dispatch is synchronous and linked-bin callers pass a
    // live `SourceInstallReport`. Future trap entry must validate caller
    // memory.
    Ok(unsafe { &mut *(ptr as *mut SourceInstallReport) })
}

fn write_dump_sync_report(report: &mut DumpSyncReport, status: dump::DumpSyncStatus) {
    report.clear();
    report.set_attempted(status.attempted);
    report.set_persistent_available(status.persistent_available);
    report.set_written(status.written);
    report.set_verified(status.verified);
    report.set_storage_capacity_bytes(status.storage_capacity_bytes);
    report.set_bytes_written(status.bytes_written);
    report.set_checksum(status.checksum);
    report.set_storage(status.storage);
    report.set_reason(status.reason);
}

fn write_process_sleep_report(
    report: &mut ProcessSleepReport,
    requested_ticks: usize,
    result: ProcessSleepResult,
) {
    let state = proc::process(result.child.pid)
        .map(|record| process_state_code(record.state))
        .unwrap_or(ProcessStateCode::BLOCKED);
    report.clear();
    report.set_requested_ticks(requested_ticks);
    report.set_pid(ProcessId::new(result.child.pid));
    report.set_state(state);
    report.set_wake_tick(result.wake_tick);
    report.set_path(result.child.program_path);
}

fn write_process_control_report(
    report: &mut ProcessControlReport,
    handle: ProcessHandle,
    fallback_state: ProcessStateCode,
) {
    let retained = proc::process(handle.pid);
    let state = retained
        .map(|record| process_state_code(record.state))
        .unwrap_or(fallback_state);
    report.clear();
    report.set_pid(ProcessId::new(handle.pid));
    report.set_state(state);
    report.set_path(handle.program_path);
    report.set_loader(handle.loader);
    report.set_entry_name(handle.entry_name);
    if let Some(record) = retained {
        report.set_body_format(record.artifact_body_format.as_str());
        report.set_body_inner(record.artifact_body_inner_format.as_str());
        if let Some(space) = mm::address_space(record.address_space_id) {
            report.set_body_bytes(space.text_bytes);
            report.set_body_checksum(space.text_checksum);
        }
    } else {
        report.set_body_format("none");
        report.set_body_inner("none");
    }
}

fn write_service_control_report(report: &mut ServiceControlReport, record: ServiceRecord) {
    report.clear();
    report.set_name(record.name);
    report.set_target(record.target);
    report.set_service_pid(record.service_pid);
    report.set_service_task_id(record.service_task_id);
    report.set_state(service_state_code(record.state));
    report.set_reason(service_reason_code(record.reason));
    report.set_result(ServiceControlResultCode::NONE);
    report.set_exit_code(0);
}

fn write_service_control_result_report(
    report: &mut ServiceControlReport,
    result: ProgramServiceControlResult,
) {
    write_service_control_report(report, result.record);
    report.set_result(service_control_result_code(result.launch_result));
    report.set_exit_code(result.launch_result.exit_code());
}

fn write_bin_source_install_report(
    report: &mut SourceInstallReport,
    record: BinSourceInstallRecord,
) {
    report.clear();
    report.set_namespace(SourceNamespaceCode::BIN);
    report.set_status(source_bin_install_status_code(record.status));
    report.set_origin(SourceInstallOriginCode::INSTALLED);
    report.set_name(record.name);
    report.set_path(record.path);
    report.set_bytes_len(record.bytes_len);
}

fn write_payload_source_install_report(
    report: &mut SourceInstallReport,
    record: PayloadSourceInstallRecord,
) {
    report.clear();
    report.set_namespace(SourceNamespaceCode::PAYLOAD);
    report.set_status(payload_source_install_status_code(record.status));
    report.set_origin(SourceInstallOriginCode::INSTALLED);
    report.set_name(record.name);
    report.set_path(record.path);
    report.set_bytes_len(record.bytes_len);
}

fn write_source_media_install_report(
    report: &mut SourceInstallReport,
    record: SourceMediaInstallRecord,
) {
    report.clear();
    report.set_namespace(source_namespace_code(record.namespace));
    report.set_status(SourceInstallStatusCode::NONE);
    report.set_origin(SourceInstallOriginCode::SOURCE_MEDIA);
    report.set_name(record.name);
    report.set_path(record.path);
    report.set_storage(record.storage);
    report.set_storage_capacity_bytes(record.storage_capacity_bytes);
    report.set_artifact_bytes_len(record.artifact_bytes_len);
    report.set_bytes_len(record.bytes_len);
    report.set_checksum(record.checksum);
}

fn write_process_wait_report(report: &mut ProcessWaitReport, wait: WaitRecord) {
    report.clear();
    report.set_child_pid(ProcessId::new(wait.child_pid));
    report.set_child_state(process_state_code(wait.child_state));
    report.set_exit_code(wait.exit_code);
    report.set_completed(wait.completed);
}

fn write_process_timed_wait_report(
    report: &mut ProcessTimedWaitReport,
    requested_ticks: usize,
    result: ProcessTimedWaitResult,
) {
    report.clear();
    report.set_requested_ticks(requested_ticks);
    report.set_child_pid(ProcessId::new(result.child_pid));
    report.set_child_state(process_state_code(result.child_state));
    report.set_exit_code(result.exit_code);
    report.set_completed(result.completed);
    report.set_timed_out(result.timed_out);
    report.set_tick_count(result.tick_count);
}

const fn process_state_code(state: proc::ProcessState) -> ProcessStateCode {
    match state {
        proc::ProcessState::Empty => ProcessStateCode::EMPTY,
        proc::ProcessState::New => ProcessStateCode::NEW,
        proc::ProcessState::Ready => ProcessStateCode::READY,
        proc::ProcessState::Running => ProcessStateCode::RUNNING,
        proc::ProcessState::Blocked => ProcessStateCode::BLOCKED,
        proc::ProcessState::Exited => ProcessStateCode::EXITED,
        proc::ProcessState::Failed => ProcessStateCode::FAILED,
        proc::ProcessState::Halted => ProcessStateCode::HALTED,
        proc::ProcessState::Reaped => ProcessStateCode::REAPED,
    }
}

const fn service_state_code(state: service::ServiceState) -> ServiceStateCode {
    match state {
        service::ServiceState::Empty => ServiceStateCode::EMPTY,
        service::ServiceState::Requested => ServiceStateCode::REQUESTED,
        service::ServiceState::Started => ServiceStateCode::STARTED,
        service::ServiceState::Exited => ServiceStateCode::EXITED,
        service::ServiceState::Stopped => ServiceStateCode::STOPPED,
        service::ServiceState::Failed => ServiceStateCode::FAILED,
    }
}

const fn service_reason_code(reason: service::ServiceReason) -> ServiceReasonCode {
    match reason {
        service::ServiceReason::None => ServiceReasonCode::NONE,
        service::ServiceReason::Requested => ServiceReasonCode::REQUESTED,
        service::ServiceReason::Running => ServiceReasonCode::RUNNING,
        service::ServiceReason::ProcessExited => ServiceReasonCode::PROCESS_EXITED,
        service::ServiceReason::ProcessFailed => ServiceReasonCode::PROCESS_FAILED,
        service::ServiceReason::ProcessKilled => ServiceReasonCode::PROCESS_KILLED,
        service::ServiceReason::OperatorStop => ServiceReasonCode::OPERATOR_STOP,
        service::ServiceReason::ExecLoadError => ServiceReasonCode::EXEC_LOAD_ERROR,
        service::ServiceReason::Halt => ServiceReasonCode::HALT,
        service::ServiceReason::StartError => ServiceReasonCode::START_ERROR,
    }
}

const fn service_control_result_code(result: PayloadLaunchResult) -> ServiceControlResultCode {
    match result {
        PayloadLaunchResult::Ready => ServiceControlResultCode::PAYLOAD_READY,
        PayloadLaunchResult::Resident => ServiceControlResultCode::PAYLOAD_RESIDENT,
        PayloadLaunchResult::NotConfigured => ServiceControlResultCode::PAYLOAD_NOT_CONFIGURED,
        PayloadLaunchResult::Failed => ServiceControlResultCode::PAYLOAD_FAILED,
        PayloadLaunchResult::ExitCode(_) => ServiceControlResultCode::PAYLOAD_EXIT_CODE,
    }
}

const fn source_namespace_code(namespace: SourceArtifactNamespace) -> SourceNamespaceCode {
    match namespace {
        SourceArtifactNamespace::Bin => SourceNamespaceCode::BIN,
        SourceArtifactNamespace::Payload => SourceNamespaceCode::PAYLOAD,
    }
}

const fn source_bin_install_status_code(status: BinSourceInstallStatus) -> SourceInstallStatusCode {
    match status {
        BinSourceInstallStatus::Ok => SourceInstallStatusCode::OK,
        BinSourceInstallStatus::Error => SourceInstallStatusCode::ERROR,
    }
}

const fn payload_source_install_status_code(
    status: PayloadSourceInstallStatus,
) -> SourceInstallStatusCode {
    match status {
        PayloadSourceInstallStatus::Ready => SourceInstallStatusCode::READY,
        PayloadSourceInstallStatus::Failed => SourceInstallStatusCode::FAILED,
    }
}

const fn source_bin_install_status_from_raw(raw: usize) -> Option<BinSourceInstallStatus> {
    match raw {
        RAW_SOURCE_INSTALL_STATUS_OK => Some(BinSourceInstallStatus::Ok),
        RAW_SOURCE_INSTALL_STATUS_ERROR => Some(BinSourceInstallStatus::Error),
        _ => None,
    }
}

const fn payload_source_install_status_from_raw(raw: usize) -> Option<PayloadSourceInstallStatus> {
    match raw {
        RAW_SOURCE_INSTALL_STATUS_READY => Some(PayloadSourceInstallStatus::Ready),
        RAW_SOURCE_INSTALL_STATUS_FAILED => Some(PayloadSourceInstallStatus::Failed),
        _ => None,
    }
}

fn static_init_service_request(name: &str, target: &str) -> Option<(&'static str, &'static str)> {
    if name == "shell" && target == "/bin/sh" {
        Some(("shell", "/bin/sh"))
    } else {
        None
    }
}

fn static_session_line_discipline(
    line_discipline: &str,
    pipe_mode: &str,
) -> Option<(&'static str, &'static str)> {
    if line_discipline == "argv-v1" && pipe_mode == "single-pipe" {
        Some(("argv-v1", "single-pipe"))
    } else {
        None
    }
}

fn argv_from_process_args(args: &[ProcessArg]) -> Result<ProgramArgvBuffer, SyscallError> {
    if args.is_empty() {
        return Err(SyscallError::INVALID_ARGUMENT);
    }

    let mut argv = ProgramArgvBuffer::empty();
    let mut index = 0usize;
    while index < args.len() {
        let arg = args[index];
        // SAFETY: the raw process-argv record points at caller-owned bytes for
        // this synchronous dispatch; the bytes are copied into `ProgramArgvBuffer`.
        let bytes = unsafe { raw_syscall_write_buffer(arg.ptr() as usize, arg.len()) }?;
        let text = core::str::from_utf8(bytes).map_err(|_| SyscallError::INVALID_ARGUMENT)?;
        argv.push(text).map_err(syscall_error_from_program_argv)?;
        index += 1;
    }
    Ok(argv)
}

fn env_from_process_env(vars: &[ProcessEnv]) -> Result<ProgramEnvBuffer, SyscallError> {
    let mut env = ProgramEnvBuffer::empty();
    let mut index = 0usize;
    while index < vars.len() {
        let var = vars[index];
        // SAFETY: raw process-env records point at caller-owned bytes for this
        // synchronous dispatch; bytes are copied into `ProgramEnvBuffer`.
        let name_bytes =
            unsafe { raw_syscall_write_buffer(var.name_ptr() as usize, var.name_len()) }?;
        // SAFETY: same synchronous raw-backend contract as the variable name.
        let value_bytes =
            unsafe { raw_syscall_write_buffer(var.value_ptr() as usize, var.value_len()) }?;
        let name = core::str::from_utf8(name_bytes).map_err(|_| SyscallError::INVALID_ARGUMENT)?;
        let value =
            core::str::from_utf8(value_bytes).map_err(|_| SyscallError::INVALID_ARGUMENT)?;
        env.push(name, value)
            .map_err(syscall_error_from_program_env)?;
        index += 1;
    }
    Ok(env)
}

/// Program-to-program exec failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProgramExecError {
    /// The syscall handle is not attached to a running process.
    NoCurrentProcess,
    /// The current process already owns retained syscall state.
    Busy,
    /// The exec request supplied no argv[0].
    EmptyArgv,
    /// The requested executable name or `/bin` path was not found.
    ProgramNotFound,
    /// The requested executable source artifact was not found.
    SourceNotFound,
    /// The requested executable image failed loader validation.
    InvalidImage,
    /// The supplied child stdin payload exceeded the bounded exec buffer.
    StdinTooLarge,
    /// The producer side of a bounded child pipe exceeded the pipe buffer.
    PipeTooLarge,
    /// No bounded process/task slot was available for a new executable.
    ProcessAdmissionFailed,
    /// Child descriptor setup failed before the scheduled image ran.
    FdSetupFailed,
    /// The parent could not begin a wait on the child.
    WaitFailed,
    /// The scheduler had no ready task after child spawn.
    SchedulerEmpty,
    /// The scheduler selected a process that had no pending image.
    MissingProgramImage,
    /// The bounded dispatch loop exhausted its step budget.
    DispatchBudgetExhausted,
    /// The child process could not be moved to the blocked state.
    BlockFailed,
    /// The current process image could not be replaced.
    ReplaceFailed,
}

impl ProgramExecError {
    /// Stable diagnostic word.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoCurrentProcess => "no-current-process",
            Self::Busy => "busy",
            Self::EmptyArgv => "empty-argv",
            Self::ProgramNotFound => "program-not-found",
            Self::SourceNotFound => "source-not-found",
            Self::InvalidImage => "invalid-image",
            Self::StdinTooLarge => "stdin-too-large",
            Self::PipeTooLarge => "pipe-too-large",
            Self::ProcessAdmissionFailed => "process-admission-failed",
            Self::FdSetupFailed => "fd-setup-failed",
            Self::WaitFailed => "wait-failed",
            Self::SchedulerEmpty => "scheduler-empty",
            Self::MissingProgramImage => "missing-program-image",
            Self::DispatchBudgetExhausted => "dispatch-budget-exhausted",
            Self::BlockFailed => "block-failed",
            Self::ReplaceFailed => "replace-failed",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ProgramChildFdSetup {
    stdin_from_parent_fd: Option<usize>,
    stdout_from_parent_fd: Option<usize>,
}

impl ProgramChildFdSetup {
    const fn standard() -> Self {
        Self {
            stdin_from_parent_fd: None,
            stdout_from_parent_fd: None,
        }
    }

    const fn pipe_stdout(stdout_from_parent_fd: usize) -> Self {
        Self {
            stdin_from_parent_fd: None,
            stdout_from_parent_fd: Some(stdout_from_parent_fd),
        }
    }

    const fn pipe_stdin(stdin_from_parent_fd: usize) -> Self {
        Self {
            stdin_from_parent_fd: Some(stdin_from_parent_fd),
            stdout_from_parent_fd: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProgramChildRunOutcome {
    Complete(ProgramStatus),
    Blocked(ProcessHandle),
}

impl ProgramChildRunOutcome {
    const fn status(self) -> ProgramStatus {
        match self {
            Self::Complete(status) => status,
            Self::Blocked(_) => ProgramStatus::Blocked,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PayloadSourceRunOutcome {
    Complete(PayloadLaunchResult),
    Blocked,
}

impl PayloadSourceRunOutcome {
    const fn syscall_status(self) -> SyscallStatus {
        match self {
            Self::Complete(result) => payload_syscall_status(result),
            Self::Blocked => SyscallStatus::Blocked,
        }
    }
}

/// Program process-control failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProgramProcessError {
    /// The syscall handle is not attached to a running process.
    NoCurrentProcess,
    /// The current process already owns retained syscall state.
    Busy,
    /// No retained process has the requested PID.
    ProcessNotFound,
    /// The target process state cannot be waited by this bounded syscall.
    NotWaitable,
    /// The target process exists but is not blocked.
    NotBlocked,
    /// The target process is protected from program-level termination.
    ProtectedProcess,
    /// The target process state cannot be killed through this syscall.
    NotKillable,
    /// The parent could not begin a wait on the target process.
    WaitFailed,
    /// The scheduler had no ready task while waiting.
    SchedulerEmpty,
    /// The bounded wait-dispatch loop exhausted its step budget.
    DispatchBudgetExhausted,
}

impl ProgramProcessError {
    /// Stable diagnostic word.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoCurrentProcess => "no-current-process",
            Self::Busy => "busy",
            Self::ProcessNotFound => "process-not-found",
            Self::NotWaitable => "not-waitable",
            Self::NotBlocked => "not-blocked",
            Self::ProtectedProcess => "protected-process",
            Self::NotKillable => "not-killable",
            Self::WaitFailed => "wait-failed",
            Self::SchedulerEmpty => "scheduler-empty",
            Self::DispatchBudgetExhausted => "dispatch-budget-exhausted",
        }
    }
}

const fn syscall_error_from_program_exec(error: ProgramExecError) -> SyscallError {
    match error {
        ProgramExecError::NoCurrentProcess => SyscallError::NO_CURRENT_PROCESS,
        ProgramExecError::Busy => SyscallError::BUSY,
        ProgramExecError::EmptyArgv => SyscallError::INVALID_ARGUMENT,
        ProgramExecError::ProgramNotFound | ProgramExecError::SourceNotFound => {
            SyscallError::NOT_FOUND
        }
        ProgramExecError::InvalidImage => SyscallError::INVALID_IMAGE,
        ProgramExecError::StdinTooLarge | ProgramExecError::PipeTooLarge => {
            SyscallError::FILE_TOO_LARGE
        }
        ProgramExecError::ProcessAdmissionFailed => SyscallError::BUSY,
        ProgramExecError::WaitFailed
        | ProgramExecError::SchedulerEmpty
        | ProgramExecError::MissingProgramImage
        | ProgramExecError::DispatchBudgetExhausted
        | ProgramExecError::BlockFailed
        | ProgramExecError::FdSetupFailed
        | ProgramExecError::ReplaceFailed => SyscallError::BUSY,
    }
}

const fn syscall_error_from_program_process(error: ProgramProcessError) -> SyscallError {
    match error {
        ProgramProcessError::NoCurrentProcess => SyscallError::NO_CURRENT_PROCESS,
        ProgramProcessError::Busy => SyscallError::BUSY,
        ProgramProcessError::ProcessNotFound => SyscallError::PROCESS_NOT_FOUND,
        ProgramProcessError::NotWaitable => SyscallError::NOT_WAITABLE,
        ProgramProcessError::ProtectedProcess => SyscallError::PROTECTED_PROCESS,
        ProgramProcessError::NotBlocked
        | ProgramProcessError::NotKillable
        | ProgramProcessError::WaitFailed
        | ProgramProcessError::SchedulerEmpty
        | ProgramProcessError::DispatchBudgetExhausted => SyscallError::BUSY,
    }
}

const fn syscall_error_from_program_service(error: ProgramServiceError) -> SyscallError {
    match error {
        ProgramServiceError::NoCurrentProcess => SyscallError::NO_CURRENT_PROCESS,
        ProgramServiceError::Busy => SyscallError::BUSY,
        ProgramServiceError::ServiceNotFound => SyscallError::NOT_FOUND,
        ProgramServiceError::ProtectedService => SyscallError::PROTECTED_PROCESS,
        ProgramServiceError::ProcessNotFound => SyscallError::PROCESS_NOT_FOUND,
        ProgramServiceError::NotStoppable | ProgramServiceError::AlreadyStarted => {
            SyscallError::BUSY
        }
        ProgramServiceError::UnsupportedTarget => SyscallError::UNSUPPORTED,
        ProgramServiceError::InvalidTarget => SyscallError::INVALID_ARGUMENT,
    }
}

const fn syscall_error_from_service_error(error: service::ServiceError) -> SyscallError {
    match error {
        service::ServiceError::Empty => SyscallError::INVALID_ARGUMENT,
        service::ServiceError::Unavailable => SyscallError::NO_CURRENT_PROCESS,
        service::ServiceError::Busy => SyscallError::BUSY,
        service::ServiceError::Full => SyscallError::BUSY,
    }
}

const fn syscall_error_from_program_source_install(
    error: ProgramSourceInstallError,
) -> SyscallError {
    match error {
        ProgramSourceInstallError::NoCurrentProcess => SyscallError::NO_CURRENT_PROCESS,
        ProgramSourceInstallError::Busy => SyscallError::BUSY,
        ProgramSourceInstallError::ProgramNotFound | ProgramSourceInstallError::PayloadNotFound => {
            SyscallError::NOT_FOUND
        }
        ProgramSourceInstallError::SourceMediaUnavailable => SyscallError::SOURCE_MEDIA_UNAVAILABLE,
        ProgramSourceInstallError::SourceMediaReadFailed => SyscallError::SOURCE_MEDIA_READ_FAILED,
        ProgramSourceInstallError::SourceMediaInvalid => SyscallError::INVALID_IMAGE,
        ProgramSourceInstallError::SourceMediaNamespaceMismatch => {
            SyscallError::SOURCE_MEDIA_NAMESPACE_MISMATCH
        }
        ProgramSourceInstallError::SourceMediaPathMismatch => {
            SyscallError::SOURCE_MEDIA_PATH_MISMATCH
        }
        ProgramSourceInstallError::UnsupportedImage => SyscallError::UNSUPPORTED,
        ProgramSourceInstallError::InstallFailed(SourceInstallError::EmptyPath) => {
            SyscallError::INVALID_ARGUMENT
        }
        ProgramSourceInstallError::InstallFailed(SourceInstallError::TooLarge) => {
            SyscallError::FILE_TOO_LARGE
        }
        ProgramSourceInstallError::InstallFailed(SourceInstallError::NoSlot) => SyscallError::BUSY,
    }
}

/// Program service-control failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProgramServiceError {
    /// The syscall handle is not attached to a running process.
    NoCurrentProcess,
    /// The current process already owns a retained raw syscall continuation.
    Busy,
    /// No retained service has the requested name.
    ServiceNotFound,
    /// The retained service process cannot be stopped by a program.
    ProtectedService,
    /// The retained service process is missing from the process table.
    ProcessNotFound,
    /// The retained service is not currently stoppable.
    NotStoppable,
    /// The retained service target is not a payload service target.
    UnsupportedTarget,
    /// The retained service already has a live service process.
    AlreadyStarted,
    /// The retained service target could not be represented as bounded argv.
    InvalidTarget,
}

impl ProgramServiceError {
    /// Stable diagnostic word.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoCurrentProcess => "no-current-process",
            Self::Busy => "busy",
            Self::ServiceNotFound => "service-not-found",
            Self::ProtectedService => "protected-service",
            Self::ProcessNotFound => "process-not-found",
            Self::NotStoppable => "not-stoppable",
            Self::UnsupportedTarget => "unsupported-target",
            Self::AlreadyStarted => "already-started",
            Self::InvalidTarget => "invalid-target",
        }
    }
}

/// Result of an operator service start/restart attempt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProgramServiceControlResult {
    /// Service row after the start/restart attempt.
    pub record: ServiceRecord,
    /// Payload launch result for the service target.
    pub launch_result: PayloadLaunchResult,
}

/// Runtime source install failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProgramSourceInstallError {
    /// The syscall handle is not attached to a running process.
    NoCurrentProcess,
    /// The current process already owns a retained raw syscall continuation.
    Busy,
    /// No `/bin` descriptor has the requested name or path.
    ProgramNotFound,
    /// No payload descriptor has the requested name.
    PayloadNotFound,
    /// No source-media target is installed.
    SourceMediaUnavailable,
    /// The source-media target failed to read a bounded artifact.
    SourceMediaReadFailed,
    /// The source-media bytes did not contain a valid checked artifact envelope.
    SourceMediaInvalid,
    /// The source-media artifact namespace does not match the requested install kind.
    SourceMediaNamespaceMismatch,
    /// The source-media artifact path does not match the requested descriptor.
    SourceMediaPathMismatch,
    /// The target executable is not a source-backed image.
    UnsupportedImage,
    /// The source overlay rejected the install request.
    InstallFailed(SourceInstallError),
}

impl ProgramSourceInstallError {
    /// Stable diagnostic word.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoCurrentProcess => "no-current-process",
            Self::Busy => "busy",
            Self::ProgramNotFound => "program-not-found",
            Self::PayloadNotFound => "payload-not-found",
            Self::SourceMediaUnavailable => "source-media-unavailable",
            Self::SourceMediaReadFailed => "source-media-read-failed",
            Self::SourceMediaInvalid => "source-media-invalid",
            Self::SourceMediaNamespaceMismatch => "source-media-namespace-mismatch",
            Self::SourceMediaPathMismatch => "source-media-path-mismatch",
            Self::UnsupportedImage => "unsupported-image",
            Self::InstallFailed(error) => error.as_str(),
        }
    }
}

/// Result of installing one `/bin` source artifact into the runtime overlay.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BinSourceInstallRecord {
    /// Canonical `/bin` program name.
    pub name: &'static str,
    /// Loader-visible source-store path.
    pub path: &'static str,
    /// Installed `/bin` source exit status.
    pub status: BinSourceInstallStatus,
    /// Encoded source-image byte length.
    pub bytes_len: usize,
}

/// Result of installing one payload source artifact into the runtime overlay.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PayloadSourceInstallRecord {
    /// Canonical payload name.
    pub name: &'static str,
    /// Loader-visible source-store path.
    pub path: &'static str,
    /// Installed payload-source exit status.
    pub status: PayloadSourceInstallStatus,
    /// Encoded source-image byte length.
    pub bytes_len: usize,
}

/// Result of installing source bytes read from source media.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceMediaInstallRecord {
    /// Canonical program or payload name.
    pub name: &'static str,
    /// Source artifact namespace.
    pub namespace: SourceArtifactNamespace,
    /// Loader-visible source-store path.
    pub path: &'static str,
    /// Source-media storage label.
    pub storage: &'static str,
    /// Source-media capacity in bytes.
    pub storage_capacity_bytes: usize,
    /// Complete checked source-media artifact bytes read.
    pub artifact_bytes_len: usize,
    /// Source payload bytes installed.
    pub bytes_len: usize,
    /// Verified checksum over the source payload.
    pub checksum: u32,
}

/// Result class for a cooperative scheduler yield request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SchedulerYieldStatus {
    /// No current process was attached to the syscall handle.
    Unavailable,
    /// The current process already owns a retained raw syscall continuation.
    Busy,
    /// The current process was not in the running state.
    NotRunning,
    /// The yield was handled, but no different ready task was available.
    NoPeer,
    /// The yield selected and ran a different ready task.
    Yielded,
    /// The scheduler/process transition failed.
    Failed,
}

impl SchedulerYieldStatus {
    /// Stable diagnostic word.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unavailable => "unavailable",
            Self::Busy => "busy",
            Self::NotRunning => "not-running",
            Self::NoPeer => "no-peer",
            Self::Yielded => "yielded",
            Self::Failed => "failed",
        }
    }
}

/// Structured result of a cooperative scheduler yield request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SchedulerYieldResult {
    /// Yield result class.
    pub status: SchedulerYieldStatus,
    /// Whether a different ready task was selected and run.
    pub yielded: bool,
    /// Process selected by the scheduler, or `0` when none was selected.
    pub selected_pid: usize,
    /// Task selected by the scheduler, or `0` when none was selected.
    pub selected_task_id: usize,
}

impl SchedulerYieldResult {
    const fn new(
        status: SchedulerYieldStatus,
        yielded: bool,
        selected_pid: usize,
        selected_task_id: usize,
    ) -> Self {
        Self {
            status,
            yielded,
            selected_pid,
            selected_task_id,
        }
    }
}

/// Result class for an explicit scheduler tick request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SchedulerTickStatus {
    /// No current process was attached to the syscall handle.
    Unavailable,
    /// The current process already owns a retained raw syscall continuation.
    Busy,
    /// The current process was not in the running state.
    NotRunning,
    /// The tick was charged to the running task.
    Ok,
    /// The scheduler/process transition failed.
    Failed,
}

impl SchedulerTickStatus {
    /// Stable diagnostic word.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unavailable => "unavailable",
            Self::Busy => "busy",
            Self::NotRunning => "not-running",
            Self::Ok => "ok",
            Self::Failed => "failed",
        }
    }
}

/// Structured result of an explicit scheduler tick request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SchedulerTickResult {
    /// Tick result class.
    pub status: SchedulerTickStatus,
    /// Whether a tick was recorded.
    pub ticked: bool,
    /// Task charged by the scheduler, or `0` when none was charged.
    pub task_id: usize,
    /// Global explicit scheduler tick count after the request.
    pub tick_count: usize,
    /// Explicit scheduler ticks charged to the selected task.
    pub task_ticks: usize,
    /// Number of sleeping processes woken by this tick.
    pub woken_count: usize,
}

impl SchedulerTickResult {
    const fn new(
        status: SchedulerTickStatus,
        ticked: bool,
        task_id: usize,
        tick_count: usize,
        task_ticks: usize,
        woken_count: usize,
    ) -> Self {
        Self {
            status,
            ticked,
            task_id,
            tick_count,
            task_ticks,
            woken_count,
        }
    }
}

/// Result class for a cooperative current-process sleep request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SchedulerSleepStatus {
    /// No current process was attached to the syscall handle.
    Unavailable,
    /// The current process already owns a retained raw syscall continuation.
    Busy,
    /// The current process was not in the running state.
    NotRunning,
    /// The current process slept until its scheduler tick deadline.
    Ok,
    /// The scheduler/process transition failed.
    Failed,
    /// The bounded dispatch/tick loop was exhausted before the process resumed.
    BudgetExhausted,
}

impl SchedulerSleepStatus {
    /// Stable diagnostic word.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unavailable => "unavailable",
            Self::Busy => "busy",
            Self::NotRunning => "not-running",
            Self::Ok => "ok",
            Self::Failed => "failed",
            Self::BudgetExhausted => "budget-exhausted",
        }
    }
}

/// Structured result of a cooperative current-process sleep request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SchedulerSleepResult {
    /// Sleep result class.
    pub status: SchedulerSleepStatus,
    /// Whether the current process blocked and later resumed.
    pub slept: bool,
    /// Process that requested sleep, or `0` when unavailable.
    pub pid: usize,
    /// Task that requested sleep, or `0` when unavailable.
    pub task_id: usize,
    /// Scheduler tick deadline requested by the sleep.
    pub wake_tick: usize,
    /// Global scheduler tick count when the syscall returned.
    pub tick_count: usize,
    /// Number of other ready processes dispatched while current was asleep.
    pub dispatched_count: usize,
    /// Number of sleeping processes woken while current was asleep.
    pub woken_count: usize,
}

impl SchedulerSleepResult {
    const fn new(
        status: SchedulerSleepStatus,
        slept: bool,
        pid: usize,
        task_id: usize,
        wake_tick: usize,
        tick_count: usize,
        dispatched_count: usize,
        woken_count: usize,
    ) -> Self {
        Self {
            status,
            slept,
            pid,
            task_id,
            wake_tick,
            tick_count,
            dispatched_count,
            woken_count,
        }
    }
}

/// Result of spawning a child that sleeps until a scheduler tick deadline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProcessSleepResult {
    /// Sleeping child process.
    pub child: ProcessHandle,
    /// Scheduler tick at or after which the child wakes.
    pub wake_tick: usize,
}

/// Result of a bounded tick-driven wait.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProcessTimedWaitResult {
    /// Child process identifier.
    pub child_pid: usize,
    /// Child state observed when the wait returned.
    pub child_state: proc::ProcessState,
    /// Child exit status observed when the wait returned.
    pub exit_code: i32,
    /// Whether the child completed before the deadline.
    pub completed: bool,
    /// Whether the wait returned because the deadline expired.
    pub timed_out: bool,
    /// Global scheduler tick count when the wait returned.
    pub tick_count: usize,
}

const fn process_record_handle(record: ProcessRecord) -> ProcessHandle {
    ProcessHandle {
        pid: record.pid,
        task_id: record.task_id,
        program_path: record.program_path,
        loader: record.loader,
        entry_name: record.entry_name,
    }
}

const fn process_state_is_waitable(state: proc::ProcessState) -> bool {
    matches!(
        state,
        proc::ProcessState::Ready
            | proc::ProcessState::Blocked
            | proc::ProcessState::Exited
            | proc::ProcessState::Failed
            | proc::ProcessState::Halted
    )
}

const fn process_is_service_ready_blocked(record: ProcessRecord) -> bool {
    matches!(record.state, proc::ProcessState::Blocked)
        && matches!(record.block_reason, sched::BlockReason::Service)
}

const fn process_state_is_readiness_waitable(record: ProcessRecord) -> bool {
    process_state_is_waitable(record.state) || process_is_service_ready_blocked(record)
}

const fn process_state_is_timed_waitable(state: proc::ProcessState) -> bool {
    matches!(
        state,
        proc::ProcessState::Ready
            | proc::ProcessState::Blocked
            | proc::ProcessState::Exited
            | proc::ProcessState::Failed
            | proc::ProcessState::Halted
    )
}

const fn process_state_is_complete(state: proc::ProcessState) -> bool {
    matches!(
        state,
        proc::ProcessState::Exited
            | proc::ProcessState::Failed
            | proc::ProcessState::Halted
            | proc::ProcessState::Reaped
    )
}

fn timed_wait_result(
    child: ProcessHandle,
    state: proc::ProcessState,
    exit_code: i32,
    completed: bool,
    timed_out: bool,
) -> ProcessTimedWaitResult {
    ProcessTimedWaitResult {
        child_pid: child.pid,
        child_state: state,
        exit_code,
        completed,
        timed_out,
        tick_count: sched::snapshot_scheduler().tick_count,
    }
}

fn wake_sleepers_due_with_events(tick_count: usize) -> usize {
    let mut woken = [proc::EMPTY_PROCESS_HANDLE; proc::MAX_PROCESSES];
    let woken_count = proc::wake_sleepers_due(tick_count, &mut woken);
    let mut index = 0usize;
    while index < woken_count {
        record_process_wake_event(woken[index]);
        index += 1;
    }
    woken_count
}

fn record_process_wake_event(handle: ProcessHandle) {
    let ctx = SyscallContext::from_process(handle);
    record_context(ctx, SyscallOp::ProcessWake, SyscallStatus::Ok);
    append_process_control_event(ctx, SyscallOp::ProcessWake);
}

fn wake_pipe_read_waiters(waiters: &[usize]) -> usize {
    let mut woken = 0usize;
    let mut index = 0usize;
    while index < waiters.len() {
        let pid = waiters[index];
        if pid != 0 {
            if let Some(record) = proc::process(pid) {
                if record.state == proc::ProcessState::Blocked
                    && record.block_reason == sched::BlockReason::PipeRead
                {
                    if let Some(handle) = proc::wake_process(pid) {
                        record_process_wake_event(handle);
                        woken += 1;
                    }
                }
            }
        }
        index += 1;
    }
    woken
}

fn wake_pipe_write_waiters(waiters: &[usize]) -> usize {
    let mut woken = 0usize;
    let mut index = 0usize;
    while index < waiters.len() {
        let pid = waiters[index];
        if pid != 0 {
            if let Some(record) = proc::process(pid) {
                if record.state == proc::ProcessState::Blocked
                    && record.block_reason == sched::BlockReason::PipeWrite
                {
                    if let Some(handle) = proc::wake_process(pid) {
                        record_process_wake_event(handle);
                        woken += 1;
                    }
                }
            }
        }
        index += 1;
    }
    woken
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ProgramStdinBuffer {
    bytes: [u8; MAX_PROGRAM_STDIN_BYTES],
    len: usize,
    cursor: usize,
    seeded: bool,
}

impl ProgramStdinBuffer {
    const fn empty() -> Self {
        Self {
            bytes: [0u8; MAX_PROGRAM_STDIN_BYTES],
            len: 0,
            cursor: 0,
            seeded: false,
        }
    }

    fn from_bytes(bytes: &[u8]) -> Self {
        let mut stdin = Self::empty();
        stdin.seeded = true;
        while stdin.len < bytes.len() && stdin.len < stdin.bytes.len() {
            stdin.bytes[stdin.len] = bytes[stdin.len];
            stdin.len += 1;
        }
        stdin
    }

    const fn is_seeded(&self) -> bool {
        self.seeded
    }

    fn seed(&mut self, bytes: &[u8]) {
        *self = Self::from_bytes(bytes);
    }

    fn read(&mut self, out: &mut [u8]) -> usize {
        self.seeded = true;
        let mut written = 0usize;
        while written < out.len() && self.cursor < self.len {
            out[written] = self.bytes[self.cursor];
            written += 1;
            self.cursor += 1;
        }
        written
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProgramVfsOpenKind {
    File,
    Directory,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProgramOpenAccess {
    ReadOnly,
    WriteOnly,
}

impl ProgramOpenAccess {
    const fn readable(self) -> bool {
        match self {
            Self::ReadOnly => true,
            Self::WriteOnly => false,
        }
    }

    const fn writable(self) -> bool {
        match self {
            Self::ReadOnly => false,
            Self::WriteOnly => true,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProgramOpenFileDescriptionKind {
    Vfs,
    Tty,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ProgramVfsFdState {
    open: bool,
    kind: ProgramVfsOpenKind,
    buffer_slot: usize,
    cursor: usize,
    len: usize,
    read_recorded: bool,
}

impl ProgramVfsFdState {
    const fn closed() -> Self {
        Self {
            open: false,
            kind: ProgramVfsOpenKind::File,
            buffer_slot: 0,
            cursor: 0,
            len: 0,
            read_recorded: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ProgramTtyFdState {
    open: bool,
    readable: bool,
    writable: bool,
    bytes: [u8; crate::rootd::ROOT_LINE_BYTES],
    cursor: usize,
    len: usize,
    eof: bool,
    read_recorded: bool,
}

impl ProgramTtyFdState {
    const fn closed() -> Self {
        Self {
            open: false,
            readable: false,
            writable: false,
            bytes: [0u8; crate::rootd::ROOT_LINE_BYTES],
            cursor: 0,
            len: 0,
            eof: false,
            read_recorded: false,
        }
    }

    const fn open(access: ProgramOpenAccess) -> Self {
        Self {
            open: true,
            readable: access.readable(),
            writable: access.writable(),
            bytes: [0u8; crate::rootd::ROOT_LINE_BYTES],
            cursor: 0,
            len: 0,
            eof: false,
            read_recorded: false,
        }
    }

    fn needs_refill(&self) -> bool {
        self.open && !self.eof && self.cursor >= self.len
    }

    fn refill(&mut self, bytes: &[u8]) {
        self.cursor = 0;
        self.len = 0;
        self.eof = bytes.is_empty();
        self.read_recorded = false;
        let mut index = 0usize;
        while index < bytes.len() && index < self.bytes.len() {
            self.bytes[index] = bytes[index];
            self.len += 1;
            index += 1;
        }
    }

    fn read(&mut self, out: &mut [u8]) -> (usize, bool) {
        let should_record = !self.read_recorded;
        let mut read = 0usize;
        while read < out.len() && self.cursor < self.len {
            out[read] = self.bytes[self.cursor];
            read += 1;
            self.cursor += 1;
        }
        self.read_recorded = true;
        (read, should_record)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProgramPipeEnd {
    Read,
    Write,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ProgramFdStatusFlags {
    nonblocking: bool,
}

impl ProgramFdStatusFlags {
    const fn empty() -> Self {
        Self { nonblocking: false }
    }

    fn from_file_status_flags(flags: FileStatusFlags) -> Result<Self, ProgramIoError> {
        if !flags.is_supported() {
            return Err(ProgramIoError::InvalidArgument);
        }
        Ok(Self {
            nonblocking: flags.is_nonblocking(),
        })
    }

    const fn is_nonblocking(self) -> bool {
        self.nonblocking
    }

    const fn to_file_status_flags(self) -> FileStatusFlags {
        if self.nonblocking {
            FileStatusFlags::NONBLOCK
        } else {
            FileStatusFlags::EMPTY
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ProgramPipeFdState {
    bytes: [u8; MAX_PROGRAM_PIPE_BYTES],
    read_cursor: usize,
    len: usize,
    readers: usize,
    writers: usize,
    read_status_flags: ProgramFdStatusFlags,
    write_status_flags: ProgramFdStatusFlags,
    read_waiters: [usize; MAX_PROGRAM_PIPE_WAITERS],
    read_waiter_count: usize,
    write_waiters: [usize; MAX_PROGRAM_PIPE_WAITERS],
    write_waiter_count: usize,
    read_recorded: bool,
    write_recorded: bool,
}

impl ProgramPipeFdState {
    const fn closed() -> Self {
        Self {
            bytes: [0u8; MAX_PROGRAM_PIPE_BYTES],
            read_cursor: 0,
            len: 0,
            readers: 0,
            writers: 0,
            read_status_flags: ProgramFdStatusFlags::empty(),
            write_status_flags: ProgramFdStatusFlags::empty(),
            read_waiters: [0; MAX_PROGRAM_PIPE_WAITERS],
            read_waiter_count: 0,
            write_waiters: [0; MAX_PROGRAM_PIPE_WAITERS],
            write_waiter_count: 0,
            read_recorded: false,
            write_recorded: false,
        }
    }

    const fn open_pair() -> Self {
        Self {
            bytes: [0u8; MAX_PROGRAM_PIPE_BYTES],
            read_cursor: 0,
            len: 0,
            readers: 1,
            writers: 1,
            read_status_flags: ProgramFdStatusFlags::empty(),
            write_status_flags: ProgramFdStatusFlags::empty(),
            read_waiters: [0; MAX_PROGRAM_PIPE_WAITERS],
            read_waiter_count: 0,
            write_waiters: [0; MAX_PROGRAM_PIPE_WAITERS],
            write_waiter_count: 0,
            read_recorded: false,
            write_recorded: false,
        }
    }

    fn unread_len(&self) -> usize {
        self.len.saturating_sub(self.read_cursor)
    }

    fn compact(&mut self) {
        if self.read_cursor == 0 {
            return;
        }
        let unread = self.unread_len();
        let mut index = 0usize;
        while index < unread {
            self.bytes[index] = self.bytes[self.read_cursor + index];
            index += 1;
        }
        self.read_cursor = 0;
        self.len = unread;
    }

    fn record_read_waiter(&mut self, pid: usize) -> Result<(), ProgramIoError> {
        if pid == 0 {
            return Err(ProgramIoError::InvalidArgument);
        }
        let mut index = 0usize;
        while index < self.read_waiter_count {
            if self.read_waiters[index] == pid {
                return Ok(());
            }
            index += 1;
        }
        if self.read_waiter_count < self.read_waiters.len() {
            self.read_waiters[self.read_waiter_count] = pid;
            self.read_waiter_count += 1;
            Ok(())
        } else {
            Err(ProgramIoError::NoSpace)
        }
    }

    fn record_write_waiter(&mut self, pid: usize) -> Result<(), ProgramIoError> {
        if pid == 0 {
            return Err(ProgramIoError::InvalidArgument);
        }
        let mut index = 0usize;
        while index < self.write_waiter_count {
            if self.write_waiters[index] == pid {
                return Ok(());
            }
            index += 1;
        }
        if self.write_waiter_count < self.write_waiters.len() {
            self.write_waiters[self.write_waiter_count] = pid;
            self.write_waiter_count += 1;
            Ok(())
        } else {
            Err(ProgramIoError::NoSpace)
        }
    }

    fn remove_read_waiter(&mut self, pid: usize) -> usize {
        if pid == 0 {
            return 0;
        }
        let mut removed = 0usize;
        let mut index = 0usize;
        while index < self.read_waiter_count {
            if self.read_waiters[index] == pid {
                let mut shift = index;
                while shift + 1 < self.read_waiter_count {
                    self.read_waiters[shift] = self.read_waiters[shift + 1];
                    shift += 1;
                }
                self.read_waiter_count -= 1;
                self.read_waiters[self.read_waiter_count] = 0;
                removed += 1;
            } else {
                index += 1;
            }
        }
        removed
    }

    fn remove_write_waiter(&mut self, pid: usize) -> usize {
        if pid == 0 {
            return 0;
        }
        let mut removed = 0usize;
        let mut index = 0usize;
        while index < self.write_waiter_count {
            if self.write_waiters[index] == pid {
                let mut shift = index;
                while shift + 1 < self.write_waiter_count {
                    self.write_waiters[shift] = self.write_waiters[shift + 1];
                    shift += 1;
                }
                self.write_waiter_count -= 1;
                self.write_waiters[self.write_waiter_count] = 0;
                removed += 1;
            } else {
                index += 1;
            }
        }
        removed
    }

    #[cfg(feature = "selftest")]
    fn read_waiter_count_for_pid(&self, pid: usize) -> usize {
        if pid == 0 {
            return 0;
        }
        let mut count = 0usize;
        let mut index = 0usize;
        while index < self.read_waiter_count {
            if self.read_waiters[index] == pid {
                count += 1;
            }
            index += 1;
        }
        count
    }

    #[cfg(feature = "selftest")]
    fn write_waiter_count_for_pid(&self, pid: usize) -> usize {
        if pid == 0 {
            return 0;
        }
        let mut count = 0usize;
        let mut index = 0usize;
        while index < self.write_waiter_count {
            if self.write_waiters[index] == pid {
                count += 1;
            }
            index += 1;
        }
        count
    }

    #[cfg(feature = "selftest")]
    fn fill_read_waiters_for_tests(&mut self, first_pid: usize) -> Result<usize, ProgramIoError> {
        let mut inserted = 0usize;
        let mut pid = first_pid.max(1);
        while self.read_waiter_count < self.read_waiters.len() {
            self.record_read_waiter(pid)?;
            inserted += 1;
            pid = pid.checked_add(1).ok_or(ProgramIoError::NoSpace)?;
        }
        Ok(inserted)
    }

    fn take_read_waiters(&mut self, out: &mut [usize]) -> usize {
        let copied = self.read_waiter_count.min(out.len());
        let mut index = 0usize;
        while index < copied {
            out[index] = self.read_waiters[index];
            index += 1;
        }
        index = 0;
        while index < self.read_waiter_count {
            self.read_waiters[index] = 0;
            index += 1;
        }
        self.read_waiter_count = 0;
        copied
    }

    fn take_write_waiters(&mut self, out: &mut [usize]) -> usize {
        let copied = self.write_waiter_count.min(out.len());
        let mut index = 0usize;
        while index < copied {
            out[index] = self.write_waiters[index];
            index += 1;
        }
        index = 0;
        while index < self.write_waiter_count {
            self.write_waiters[index] = 0;
            index += 1;
        }
        self.write_waiter_count = 0;
        copied
    }

    fn read(&mut self, out: &mut [u8]) -> Result<(usize, bool), ProgramIoError> {
        let should_record = !self.read_recorded;
        if out.is_empty() {
            self.read_recorded = true;
            return Ok((0, should_record));
        }
        if self.unread_len() == 0 {
            if self.writers > 0 {
                return Err(ProgramIoError::Busy);
            }
            self.read_recorded = true;
            return Ok((0, should_record));
        }
        let mut read = 0usize;
        while read < out.len() && self.read_cursor < self.len {
            out[read] = self.bytes[self.read_cursor];
            read += 1;
            self.read_cursor += 1;
        }
        if self.read_cursor == self.len {
            self.read_cursor = 0;
            self.len = 0;
        }
        self.read_recorded = true;
        Ok((read, should_record))
    }

    const fn status_flags(&self, end: ProgramPipeEnd) -> ProgramFdStatusFlags {
        match end {
            ProgramPipeEnd::Read => self.read_status_flags,
            ProgramPipeEnd::Write => self.write_status_flags,
        }
    }

    fn set_status_flags(&mut self, end: ProgramPipeEnd, flags: ProgramFdStatusFlags) {
        match end {
            ProgramPipeEnd::Read => {
                self.read_status_flags = flags;
            }
            ProgramPipeEnd::Write => {
                self.write_status_flags = flags;
            }
        }
    }

    fn write(&mut self, bytes: &[u8]) -> Result<(usize, bool), ProgramIoError> {
        if self.readers == 0 {
            return Err(ProgramIoError::NotReadable);
        }
        let should_record = !self.write_recorded;
        if bytes.is_empty() {
            self.write_recorded = true;
            return Ok((0, should_record));
        }
        self.compact();
        if self.len >= self.bytes.len() {
            return Err(ProgramIoError::Busy);
        }
        let mut written = 0usize;
        while written < bytes.len() && self.len < self.bytes.len() {
            self.bytes[self.len] = bytes[written];
            self.len += 1;
            written += 1;
        }
        self.write_recorded = true;
        Ok((written, should_record))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ProgramPipeSlot {
    open: bool,
    pipe: ProgramPipeFdState,
}

impl ProgramPipeSlot {
    const fn closed() -> Self {
        Self {
            open: false,
            pipe: ProgramPipeFdState::closed(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ProgramPipeWakeSet {
    read_waiters: [usize; MAX_PROGRAM_PIPE_WAITERS],
    read_waiter_count: usize,
    write_waiters: [usize; MAX_PROGRAM_PIPE_WAITERS],
    write_waiter_count: usize,
}

impl ProgramPipeWakeSet {
    const fn empty() -> Self {
        Self {
            read_waiters: [0; MAX_PROGRAM_PIPE_WAITERS],
            read_waiter_count: 0,
            write_waiters: [0; MAX_PROGRAM_PIPE_WAITERS],
            write_waiter_count: 0,
        }
    }

    fn wake(self) {
        let _ = wake_pipe_read_waiters(&self.read_waiters[..self.read_waiter_count]);
        let _ = wake_pipe_write_waiters(&self.write_waiters[..self.write_waiter_count]);
    }
}

struct ProgramPipeTable {
    slots: [ProgramPipeSlot; MAX_PROGRAM_VFS_OPEN_FILES],
}

impl ProgramPipeTable {
    const fn new() -> Self {
        Self {
            slots: [ProgramPipeSlot::closed(); MAX_PROGRAM_VFS_OPEN_FILES],
        }
    }

    fn reset(&mut self) {
        self.slots = [ProgramPipeSlot::closed(); MAX_PROGRAM_VFS_OPEN_FILES];
    }

    fn create_pipe(&mut self) -> Result<usize, ProgramIoError> {
        let mut slot = 0usize;
        while slot < self.slots.len() {
            if !self.slots[slot].open {
                self.slots[slot] = ProgramPipeSlot {
                    open: true,
                    pipe: ProgramPipeFdState::open_pair(),
                };
                return Ok(slot);
            }
            slot += 1;
        }
        Err(ProgramIoError::Busy)
    }

    fn slot_mut(&mut self, pipe_slot: usize) -> Result<&mut ProgramPipeSlot, ProgramIoError> {
        let Some(slot) = self.slots.get_mut(pipe_slot) else {
            return Err(ProgramIoError::BadFd);
        };
        if !slot.open {
            return Err(ProgramIoError::BadFd);
        }
        Ok(slot)
    }

    fn retain_endpoint(
        &mut self,
        pipe_slot: usize,
        end: ProgramPipeEnd,
    ) -> Result<(), ProgramIoError> {
        let slot = self.slot_mut(pipe_slot)?;
        match end {
            ProgramPipeEnd::Read => {
                slot.pipe.readers = slot.pipe.readers.saturating_add(1);
            }
            ProgramPipeEnd::Write => {
                slot.pipe.writers = slot.pipe.writers.saturating_add(1);
            }
        }
        Ok(())
    }

    fn release_endpoint(
        &mut self,
        pipe_slot: usize,
        end: ProgramPipeEnd,
    ) -> Result<ProgramPipeWakeSet, ProgramIoError> {
        let slot = self.slot_mut(pipe_slot)?;
        let mut wake = ProgramPipeWakeSet::empty();
        match end {
            ProgramPipeEnd::Read => {
                slot.pipe.readers = slot.pipe.readers.saturating_sub(1);
                if slot.pipe.readers == 0 {
                    wake.write_waiter_count = slot.pipe.take_write_waiters(&mut wake.write_waiters);
                }
            }
            ProgramPipeEnd::Write => {
                slot.pipe.writers = slot.pipe.writers.saturating_sub(1);
                if slot.pipe.writers == 0 {
                    wake.read_waiter_count = slot.pipe.take_read_waiters(&mut wake.read_waiters);
                }
            }
        }
        if slot.pipe.readers == 0 && slot.pipe.writers == 0 {
            *slot = ProgramPipeSlot::closed();
        }
        Ok(wake)
    }

    fn read(&mut self, pipe_slot: usize, out: &mut [u8]) -> Result<(usize, bool), ProgramIoError> {
        self.slot_mut(pipe_slot)?.pipe.read(out)
    }

    fn status_flags(
        &mut self,
        pipe_slot: usize,
        end: ProgramPipeEnd,
    ) -> Result<ProgramFdStatusFlags, ProgramIoError> {
        Ok(self.slot_mut(pipe_slot)?.pipe.status_flags(end))
    }

    fn set_status_flags(
        &mut self,
        pipe_slot: usize,
        end: ProgramPipeEnd,
        flags: ProgramFdStatusFlags,
    ) -> Result<(), ProgramIoError> {
        self.slot_mut(pipe_slot)?.pipe.set_status_flags(end, flags);
        Ok(())
    }

    fn write(
        &mut self,
        pipe_slot: usize,
        bytes: &[u8],
        read_waiters: &mut [usize],
    ) -> Result<(usize, bool, usize), ProgramIoError> {
        let slot = self.slot_mut(pipe_slot)?;
        let (written, should_record) = slot.pipe.write(bytes)?;
        let waiter_count = if written > 0 {
            slot.pipe.take_read_waiters(read_waiters)
        } else {
            0
        };
        Ok((written, should_record, waiter_count))
    }

    fn record_read_waiter(&mut self, pipe_slot: usize, pid: usize) -> Result<(), ProgramIoError> {
        self.slot_mut(pipe_slot)?.pipe.record_read_waiter(pid)
    }

    fn record_write_waiter(&mut self, pipe_slot: usize, pid: usize) -> Result<(), ProgramIoError> {
        self.slot_mut(pipe_slot)?.pipe.record_write_waiter(pid)
    }

    fn remove_read_waiter(&mut self, pid: usize) -> usize {
        let mut removed = 0usize;
        let mut index = 0usize;
        while index < self.slots.len() {
            if self.slots[index].open {
                removed += self.slots[index].pipe.remove_read_waiter(pid);
            }
            index += 1;
        }
        removed
    }

    fn remove_write_waiter(&mut self, pid: usize) -> usize {
        let mut removed = 0usize;
        let mut index = 0usize;
        while index < self.slots.len() {
            if self.slots[index].open {
                removed += self.slots[index].pipe.remove_write_waiter(pid);
            }
            index += 1;
        }
        removed
    }

    #[cfg(feature = "selftest")]
    fn read_waiter_count_for_pid(&self, pid: usize) -> usize {
        let mut count = 0usize;
        let mut index = 0usize;
        while index < self.slots.len() {
            if self.slots[index].open {
                count += self.slots[index].pipe.read_waiter_count_for_pid(pid);
            }
            index += 1;
        }
        count
    }

    #[cfg(feature = "selftest")]
    fn write_waiter_count_for_pid(&self, pid: usize) -> usize {
        let mut count = 0usize;
        let mut index = 0usize;
        while index < self.slots.len() {
            if self.slots[index].open {
                count += self.slots[index].pipe.write_waiter_count_for_pid(pid);
            }
            index += 1;
        }
        count
    }

    #[cfg(feature = "selftest")]
    fn fill_read_waiters_for_tests(
        &mut self,
        pipe_slot: usize,
        first_pid: usize,
    ) -> Result<usize, ProgramIoError> {
        self.slot_mut(pipe_slot)?
            .pipe
            .fill_read_waiters_for_tests(first_pid)
    }

    fn validate(&mut self, pipe_slot: usize) -> Result<(), ProgramIoError> {
        self.slot_mut(pipe_slot).map(|_| ())
    }

    fn take_write_waiters(
        &mut self,
        pipe_slot: usize,
        out: &mut [usize],
    ) -> Result<usize, ProgramIoError> {
        Ok(self.slot_mut(pipe_slot)?.pipe.take_write_waiters(out))
    }
}

struct ProgramPipeTableCell(UnsafeCell<ProgramPipeTable>);

// SAFETY: mutable access is serialized by `PROGRAM_PIPE_TABLE_LOCK`.
unsafe impl Sync for ProgramPipeTableCell {}

static PROGRAM_PIPE_TABLE: ProgramPipeTableCell =
    ProgramPipeTableCell(UnsafeCell::new(ProgramPipeTable::new()));
static PROGRAM_PIPE_TABLE_LOCK: AtomicBool = AtomicBool::new(false);

struct ProgramPipeTableGuard;

impl ProgramPipeTableGuard {
    fn acquire() -> Self {
        while PROGRAM_PIPE_TABLE_LOCK
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }
        Self
    }
}

impl Drop for ProgramPipeTableGuard {
    fn drop(&mut self) {
        PROGRAM_PIPE_TABLE_LOCK.store(false, Ordering::Release);
    }
}

fn with_program_pipe_table<R>(f: impl FnOnce(&mut ProgramPipeTable) -> R) -> R {
    let _guard = ProgramPipeTableGuard::acquire();
    // SAFETY: `PROGRAM_PIPE_TABLE_LOCK` serializes access.
    let table = unsafe { &mut *PROGRAM_PIPE_TABLE.0.get() };
    f(table)
}

fn reset_program_pipe_table() {
    with_program_pipe_table(ProgramPipeTable::reset);
}

fn create_program_pipe() -> Result<usize, ProgramIoError> {
    with_program_pipe_table(ProgramPipeTable::create_pipe)
}

fn retain_program_pipe_endpoint(
    pipe_slot: usize,
    end: ProgramPipeEnd,
) -> Result<(), ProgramIoError> {
    with_program_pipe_table(|table| table.retain_endpoint(pipe_slot, end))
}

fn release_program_pipe_endpoint(
    pipe_slot: usize,
    end: ProgramPipeEnd,
) -> Result<ProgramPipeWakeSet, ProgramIoError> {
    with_program_pipe_table(|table| table.release_endpoint(pipe_slot, end))
}

fn read_program_pipe(pipe_slot: usize, out: &mut [u8]) -> Result<(usize, bool), ProgramIoError> {
    with_program_pipe_table(|table| table.read(pipe_slot, out))
}

fn program_pipe_status_flags(
    pipe_slot: usize,
    end: ProgramPipeEnd,
) -> Result<ProgramFdStatusFlags, ProgramIoError> {
    with_program_pipe_table(|table| table.status_flags(pipe_slot, end))
}

fn set_program_pipe_status_flags(
    pipe_slot: usize,
    end: ProgramPipeEnd,
    flags: ProgramFdStatusFlags,
) -> Result<(), ProgramIoError> {
    with_program_pipe_table(|table| table.set_status_flags(pipe_slot, end, flags))
}

fn write_program_pipe(
    pipe_slot: usize,
    bytes: &[u8],
    read_waiters: &mut [usize],
) -> Result<(usize, bool, usize), ProgramIoError> {
    with_program_pipe_table(|table| table.write(pipe_slot, bytes, read_waiters))
}

fn record_program_pipe_read_waiter(pipe_slot: usize, pid: usize) -> Result<(), ProgramIoError> {
    with_program_pipe_table(|table| table.record_read_waiter(pipe_slot, pid))
}

fn record_program_pipe_write_waiter(pipe_slot: usize, pid: usize) -> Result<(), ProgramIoError> {
    with_program_pipe_table(|table| table.record_write_waiter(pipe_slot, pid))
}

fn release_program_pipe_read_waiters(pid: usize) -> usize {
    with_program_pipe_table(|table| table.remove_read_waiter(pid))
}

fn release_program_pipe_write_waiters(pid: usize) -> usize {
    with_program_pipe_table(|table| table.remove_write_waiter(pid))
}

#[cfg(feature = "selftest")]
pub(crate) fn program_pipe_read_waiter_count_for_pid(pid: usize) -> usize {
    with_program_pipe_table(|table| table.read_waiter_count_for_pid(pid))
}

#[cfg(feature = "selftest")]
pub(crate) fn program_pipe_write_waiter_count_for_pid(pid: usize) -> usize {
    with_program_pipe_table(|table| table.write_waiter_count_for_pid(pid))
}

fn take_program_pipe_write_waiters(
    pipe_slot: usize,
    out: &mut [usize],
) -> Result<usize, ProgramIoError> {
    with_program_pipe_table(|table| table.take_write_waiters(pipe_slot, out))
}

#[cfg(feature = "selftest")]
pub(crate) fn fill_process_pipe_read_waiters_for_tests(
    pid: usize,
    fd: usize,
    first_waiter_pid: usize,
) -> Result<usize, ProgramIoError> {
    let Some(endpoint) =
        with_existing_process_fd_table_mut(pid, |table| table.pipe_endpoint_for_fd(fd))
    else {
        return Err(ProgramIoError::BadFd);
    };
    let (pipe_slot, end) = endpoint?;
    if end != ProgramPipeEnd::Read {
        return Err(ProgramIoError::NotReadable);
    }
    with_program_pipe_table(|table| table.fill_read_waiters_for_tests(pipe_slot, first_waiter_pid))
}

fn validate_program_pipe(pipe_slot: usize) -> Result<(), ProgramIoError> {
    with_program_pipe_table(|table| table.validate(pipe_slot))
}

fn release_pipe_read_block(pid: usize) {
    release_program_pipe_read_waiters(pid);
    if matches!(
        proc::process(pid),
        Some(process)
            if process.state == proc::ProcessState::Blocked
                && process.block_reason == sched::BlockReason::PipeRead
    ) {
        let _ = proc::wake_process(pid);
        let _ = proc::run_process(pid);
    }
}

fn release_pipe_write_block(pid: usize) {
    release_program_pipe_write_waiters(pid);
    if matches!(
        proc::process(pid),
        Some(process)
            if process.state == proc::ProcessState::Blocked
                && process.block_reason == sched::BlockReason::PipeWrite
    ) {
        let _ = proc::wake_process(pid);
        let _ = proc::run_process(pid);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ProgramSyscallContinuation {
    pid: usize,
    context: SyscallContext,
    nr: SyscallNr,
    args: SyscallArgs,
    sleep_wake_tick: usize,
}

impl ProgramSyscallContinuation {
    const fn empty() -> Self {
        Self {
            pid: 0,
            context: SyscallContext {
                pid: 0,
                task_id: 0,
                program_path: "",
                loader: "",
                entry_name: "",
            },
            nr: SyscallNr::INVALID,
            args: SyscallArgs::EMPTY,
            sleep_wake_tick: 0,
        }
    }

    const fn is_empty(self) -> bool {
        self.pid == 0
    }

    const fn record(self) -> SyscallContinuationRecord {
        SyscallContinuationRecord {
            process_id: self.context.pid,
            task_id: self.context.task_id,
            program_path: self.context.program_path,
            loader: self.context.loader,
            entry_name: self.context.entry_name,
            nr: self.nr,
            op: syscall_op_for_continuation(self.nr),
            memory: syscall_memory_for_continuation(self.nr, self.args),
            args: self.args,
        }
    }
}

const fn syscall_op_for_continuation(nr: SyscallNr) -> SyscallOp {
    if nr.raw() == SyscallNr::READ.raw() {
        SyscallOp::FdRead
    } else if nr.raw() == SyscallNr::WRITE.raw() {
        SyscallOp::FdWrite
    } else if nr.raw() == SyscallNr::WAIT.raw() {
        SyscallOp::WaitBegin
    } else if nr.raw() == SyscallNr::PROCESS_WAIT_READY.raw() {
        SyscallOp::WaitReady
    } else if nr.raw() == SyscallNr::SLEEP.raw() {
        SyscallOp::ProcessSleep
    } else {
        SyscallOp::SyscallContinue
    }
}

const fn syscall_op_for_raw_dispatch(nr: SyscallNr, args: SyscallArgs) -> SyscallOp {
    if nr.raw() == SyscallNr::READ.raw() {
        SyscallOp::FdRead
    } else if nr.raw() == SyscallNr::WRITE.raw() {
        SyscallOp::FdWrite
    } else if nr.raw() == SyscallNr::OPEN_AT.raw() {
        SyscallOp::VfsOpen
    } else if nr.raw() == SyscallNr::CLOSE.raw() {
        SyscallOp::FdClose
    } else if nr.raw() == SyscallNr::LSEEK.raw() {
        SyscallOp::FdSeek
    } else if nr.raw() == SyscallNr::GETDENTS.raw() {
        SyscallOp::VfsList
    } else if nr.raw() == SyscallNr::GET_PID.raw() {
        SyscallOp::ProcessSelf
    } else if nr.raw() == SyscallNr::PROCESS_SELF.raw() {
        SyscallOp::ProcessSelf
    } else if nr.raw() == SyscallNr::GET_CWD.raw() {
        SyscallOp::SessionCwdGet
    } else if nr.raw() == SyscallNr::CHDIR.raw() {
        SyscallOp::SessionCwdSet
    } else if nr.raw() == SyscallNr::YIELD_NOW.raw() {
        SyscallOp::YieldNow
    } else if nr.raw() == SyscallNr::SLEEP.raw() {
        SyscallOp::ProcessSleep
    } else if nr.raw() == SyscallNr::SPAWN.raw() {
        SyscallOp::ExecLoad
    } else if nr.raw() == SyscallNr::EXECVE.raw() {
        SyscallOp::ExecReplace
    } else if nr.raw() == SyscallNr::WAIT.raw() {
        SyscallOp::WaitBegin
    } else if nr.raw() == SyscallNr::EXIT.raw() {
        SyscallOp::ProcessExitRequest
    } else if nr.raw() == SyscallNr::DUP.raw() || nr.raw() == SyscallNr::DUP_TO.raw() {
        SyscallOp::FdDuplicate
    } else if nr.raw() == SyscallNr::SCHED_TICK.raw() {
        SyscallOp::SchedulerTick
    } else if nr.raw() == SyscallNr::PIPE.raw() {
        SyscallOp::FdPipe
    } else if nr.raw() == SyscallNr::TERMINAL_CLEAR.raw() {
        SyscallOp::TtyClear
    } else if nr.raw() == SyscallNr::SYSTEM_HALT.raw() {
        SyscallOp::SystemHalt
    } else if nr.raw() == SyscallNr::PROVIDER_PROBE.raw() {
        SyscallOp::ProviderProbe
    } else if nr.raw() == SyscallNr::DUMP_SYNC.raw() {
        SyscallOp::DumpSync
    } else if nr.raw() == SyscallNr::PROCESS_WAKE.raw() {
        SyscallOp::ProcessWake
    } else if nr.raw() == SyscallNr::PROCESS_KILL.raw() {
        SyscallOp::ProcessKill
    } else if nr.raw() == SyscallNr::PROCESS_WAIT_TICKS.raw() {
        SyscallOp::WaitBegin
    } else if nr.raw() == SyscallNr::SERVICE_CONTROL.raw() {
        if args.a0 == RAW_SERVICE_CONTROL_OP_STOP {
            SyscallOp::ServiceStop
        } else if args.a0 == RAW_SERVICE_CONTROL_OP_START {
            SyscallOp::ServiceStart
        } else if args.a0 == RAW_SERVICE_CONTROL_OP_RESTART {
            SyscallOp::ServiceRestart
        } else if args.a0 == RAW_SERVICE_CONTROL_OP_REQUEST {
            SyscallOp::InitServiceStart
        } else {
            SyscallOp::ServiceStart
        }
    } else if nr.raw() == SyscallNr::PROCESS_WAIT_READY.raw() {
        SyscallOp::WaitReady
    } else if nr.raw() == SyscallNr::SESSION_CONTROL.raw() {
        if args.a0 == RAW_SESSION_CONTROL_OP_SHELL_START {
            SyscallOp::SessionShellStart
        } else if args.a0 == RAW_SESSION_CONTROL_OP_LINE_DISCIPLINE {
            SyscallOp::SessionLineDiscipline
        } else {
            SyscallOp::SnapshotSession
        }
    } else if nr.raw() == SyscallNr::SOURCE_CONTROL.raw() {
        SyscallOp::SourceInstall
    } else if nr.raw() == SyscallNr::FD_FLAGS_GET.raw() {
        SyscallOp::FdFlagsGet
    } else if nr.raw() == SyscallNr::FD_FLAGS_SET.raw() {
        SyscallOp::FdFlagsSet
    } else if nr.raw() == SyscallNr::TERMINAL_RAW_ENTER.raw() {
        SyscallOp::TtyRawEnter
    } else if nr.raw() == SyscallNr::TERMINAL_RAW_RESTORE.raw() {
        SyscallOp::TtyRawRestore
    } else if nr.raw() == SyscallNr::TERMINAL_RAW_RESTORE_PRIMARY.raw() {
        SyscallOp::TtyRawRestorePrimary
    } else if nr.raw() == SyscallNr::FD_STATUS_GET.raw() {
        SyscallOp::FdStatusGet
    } else if nr.raw() == SyscallNr::FD_STATUS_SET.raw() {
        SyscallOp::FdStatusSet
    } else {
        SyscallOp::SyscallContinue
    }
}

const fn syscall_memory_for_continuation(
    nr: SyscallNr,
    args: SyscallArgs,
) -> SyscallContinuationMemory {
    if nr.raw() == SyscallNr::READ.raw() {
        if args.a1 != 0 && args.a2 != 0 {
            SyscallContinuationMemory::RawReadBuffer
        } else {
            SyscallContinuationMemory::None
        }
    } else if nr.raw() == SyscallNr::WRITE.raw() {
        if args.a1 != 0 && args.a2 != 0 {
            SyscallContinuationMemory::RawWriteBuffer
        } else {
            SyscallContinuationMemory::None
        }
    } else if nr.raw() == SyscallNr::WAIT.raw() || nr.raw() == SyscallNr::PROCESS_WAIT_READY.raw() {
        if args.a1 != 0 && args.a2 != 0 {
            SyscallContinuationMemory::RawWaitReport
        } else {
            SyscallContinuationMemory::None
        }
    } else {
        SyscallContinuationMemory::None
    }
}

struct ProgramSyscallContinuationTable {
    continuations: [ProgramSyscallContinuation; proc::MAX_PROCESSES],
}

impl ProgramSyscallContinuationTable {
    const fn new() -> Self {
        Self {
            continuations: [ProgramSyscallContinuation::empty(); proc::MAX_PROCESSES],
        }
    }

    fn reset(&mut self) {
        self.continuations = [ProgramSyscallContinuation::empty(); proc::MAX_PROCESSES];
    }

    fn slot_index(&self, pid: usize) -> Option<usize> {
        if pid == 0 {
            return None;
        }
        let mut index = 0usize;
        while index < self.continuations.len() {
            if self.continuations[index].pid == pid {
                return Some(index);
            }
            index += 1;
        }
        None
    }

    fn free_slot(&self) -> Option<usize> {
        let mut index = 0usize;
        while index < self.continuations.len() {
            if self.continuations[index].is_empty() {
                return Some(index);
            }
            index += 1;
        }
        None
    }

    fn contains(&self, pid: usize) -> bool {
        self.slot_index(pid).is_some()
    }

    fn store(
        &mut self,
        context: SyscallContext,
        nr: SyscallNr,
        args: SyscallArgs,
    ) -> Result<(), ProgramIoError> {
        self.store_with_sleep_wake_tick(context, nr, args, 0)
    }

    fn store_with_sleep_wake_tick(
        &mut self,
        context: SyscallContext,
        nr: SyscallNr,
        args: SyscallArgs,
        sleep_wake_tick: usize,
    ) -> Result<(), ProgramIoError> {
        if self.contains(context.pid) {
            return Err(ProgramIoError::Busy);
        }
        let slot = self.free_slot().ok_or(ProgramIoError::Busy)?;
        self.continuations[slot] = ProgramSyscallContinuation {
            pid: context.pid,
            context,
            nr,
            args,
            sleep_wake_tick,
        };
        Ok(())
    }

    fn take(&mut self, pid: usize) -> Option<ProgramSyscallContinuation> {
        let slot = self.slot_index(pid)?;
        let continuation = self.continuations[slot];
        self.continuations[slot] = ProgramSyscallContinuation::empty();
        Some(continuation)
    }

    fn clear_matching(&mut self, pid: usize, nr: SyscallNr, args: SyscallArgs) {
        let Some(slot) = self.slot_index(pid) else {
            return;
        };
        let continuation = self.continuations[slot];
        if continuation.nr == nr && continuation.args == args {
            self.continuations[slot] = ProgramSyscallContinuation::empty();
        }
    }

    fn release_pid(&mut self, pid: usize) {
        if let Some(slot) = self.slot_index(pid) {
            self.continuations[slot] = ProgramSyscallContinuation::empty();
        }
    }

    fn wait_ready_parent_for_child(&self, child_pid: usize) -> Option<SyscallContext> {
        let mut index = 0usize;
        while index < self.continuations.len() {
            let continuation = self.continuations[index];
            if continuation.nr == SyscallNr::PROCESS_WAIT_READY && continuation.args.a0 == child_pid
            {
                return Some(continuation.context);
            }
            index += 1;
        }
        None
    }

    fn snapshot(&self, out: &mut [SyscallContinuationRecord]) -> usize {
        let mut written = 0usize;
        let mut index = 0usize;
        while index < self.continuations.len() && written < out.len() {
            let continuation = self.continuations[index];
            if !continuation.is_empty() {
                out[written] = continuation.record();
                written += 1;
            }
            index += 1;
        }
        written
    }
}

struct ProgramSyscallContinuationTableCell(UnsafeCell<ProgramSyscallContinuationTable>);

// SAFETY: mutable access is serialized by `PROGRAM_SYSCALL_CONTINUATION_LOCK`.
unsafe impl Sync for ProgramSyscallContinuationTableCell {}

static PROGRAM_SYSCALL_CONTINUATIONS: ProgramSyscallContinuationTableCell =
    ProgramSyscallContinuationTableCell(UnsafeCell::new(ProgramSyscallContinuationTable::new()));
static PROGRAM_SYSCALL_CONTINUATION_LOCK: AtomicBool = AtomicBool::new(false);

struct ProgramSyscallContinuationGuard;

impl ProgramSyscallContinuationGuard {
    fn acquire() -> Self {
        while PROGRAM_SYSCALL_CONTINUATION_LOCK
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }
        Self
    }
}

impl Drop for ProgramSyscallContinuationGuard {
    fn drop(&mut self) {
        PROGRAM_SYSCALL_CONTINUATION_LOCK.store(false, Ordering::Release);
    }
}

fn with_program_syscall_continuations<R>(
    f: impl FnOnce(&mut ProgramSyscallContinuationTable) -> R,
) -> R {
    let _guard = ProgramSyscallContinuationGuard::acquire();
    // SAFETY: `PROGRAM_SYSCALL_CONTINUATION_LOCK` serializes access.
    let table = unsafe { &mut *PROGRAM_SYSCALL_CONTINUATIONS.0.get() };
    f(table)
}

fn reset_program_syscall_continuations() {
    with_program_syscall_continuations(ProgramSyscallContinuationTable::reset);
}

fn store_program_syscall_continuation(
    context: SyscallContext,
    nr: SyscallNr,
    args: SyscallArgs,
) -> Result<(), ProgramIoError> {
    with_program_syscall_continuations(|table| table.store(context, nr, args))
}

fn store_program_sleep_syscall_continuation(
    context: SyscallContext,
    args: SyscallArgs,
    wake_tick: usize,
) -> Result<(), ProgramIoError> {
    with_program_syscall_continuations(|table| {
        table.store_with_sleep_wake_tick(context, SyscallNr::SLEEP, args, wake_tick)
    })
}

fn has_program_syscall_continuation(pid: usize) -> bool {
    with_program_syscall_continuations(|table| table.contains(pid))
}

fn take_program_syscall_continuation(pid: usize) -> Option<ProgramSyscallContinuation> {
    with_program_syscall_continuations(|table| table.take(pid))
}

fn clear_matching_program_syscall_continuation(pid: usize, nr: SyscallNr, args: SyscallArgs) {
    with_program_syscall_continuations(|table| table.clear_matching(pid, nr, args));
}

fn release_program_syscall_continuation(pid: usize) {
    with_program_syscall_continuations(|table| table.release_pid(pid));
}

fn wake_wait_ready_continuation_for_child(child_pid: usize) {
    let waiting =
        with_program_syscall_continuations(|table| table.wait_ready_parent_for_child(child_pid));
    if let Some(waiting) = waiting {
        let _ = proc::wake_process(waiting.pid);
    }
}

/// Copies active retained raw syscall continuations into `out`.
pub fn snapshot_syscall_continuations(out: &mut [SyscallContinuationRecord]) -> usize {
    with_program_syscall_continuations(|table| table.snapshot(out))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PayloadSourceContinuationStage {
    WaitingSpawnWaitBin,
    WaitingSpawnWaitPayload,
    WaitingPipeProducer,
    WaitingPipeConsumer,
}

#[derive(Clone, Copy, Debug)]
struct PayloadSourceContinuation {
    payload_pid: usize,
    context: SyscallContext,
    payload: LoadedPayloadProgram,
    next_offset: usize,
    stage: PayloadSourceContinuationStage,
    waiting_pid: usize,
    read_fd: usize,
    consumer: ProgramArgvBuffer,
}

impl PayloadSourceContinuation {
    const fn is_empty(self) -> bool {
        self.payload_pid == 0
    }
}

#[derive(Clone, Copy, Debug)]
struct PayloadSourceContinuationSlot {
    frame: Option<PayloadSourceContinuation>,
}

impl PayloadSourceContinuationSlot {
    const fn empty() -> Self {
        Self { frame: None }
    }
}

struct PayloadSourceContinuationTable {
    slots: [PayloadSourceContinuationSlot; proc::MAX_PROCESSES],
}

impl PayloadSourceContinuationTable {
    const fn new() -> Self {
        Self {
            slots: [PayloadSourceContinuationSlot::empty(); proc::MAX_PROCESSES],
        }
    }

    fn reset(&mut self) {
        self.slots = [PayloadSourceContinuationSlot::empty(); proc::MAX_PROCESSES];
    }

    fn slot_index(&self, payload_pid: usize) -> Option<usize> {
        if payload_pid == 0 {
            return None;
        }
        let mut index = 0usize;
        while index < self.slots.len() {
            if matches!(self.slots[index].frame, Some(frame) if frame.payload_pid == payload_pid) {
                return Some(index);
            }
            index += 1;
        }
        None
    }

    fn free_slot(&self) -> Option<usize> {
        let mut index = 0usize;
        while index < self.slots.len() {
            if self.slots[index].frame.is_none() {
                return Some(index);
            }
            index += 1;
        }
        None
    }

    fn can_store(&self, payload_pid: usize) -> bool {
        payload_pid != 0 && self.slot_index(payload_pid).is_none() && self.free_slot().is_some()
    }

    fn store(&mut self, frame: PayloadSourceContinuation) -> Result<(), ProgramExecError> {
        if frame.is_empty() || self.slot_index(frame.payload_pid).is_some() {
            return Err(ProgramExecError::Busy);
        }
        let slot = self.free_slot().ok_or(ProgramExecError::Busy)?;
        self.slots[slot].frame = Some(frame);
        Ok(())
    }

    fn take(&mut self, payload_pid: usize) -> Option<PayloadSourceContinuation> {
        let slot = self.slot_index(payload_pid)?;
        let frame = self.slots[slot].frame?;
        self.slots[slot] = PayloadSourceContinuationSlot::empty();
        Some(frame)
    }

    fn release_pid(&mut self, payload_pid: usize) {
        if let Some(slot) = self.slot_index(payload_pid) {
            self.slots[slot] = PayloadSourceContinuationSlot::empty();
        }
    }
}

struct PayloadSourceContinuationTableCell(UnsafeCell<PayloadSourceContinuationTable>);

// SAFETY: mutable access is serialized by `PAYLOAD_SOURCE_CONTINUATION_LOCK`.
unsafe impl Sync for PayloadSourceContinuationTableCell {}

static PAYLOAD_SOURCE_CONTINUATIONS: PayloadSourceContinuationTableCell =
    PayloadSourceContinuationTableCell(UnsafeCell::new(PayloadSourceContinuationTable::new()));
static PAYLOAD_SOURCE_CONTINUATION_LOCK: AtomicBool = AtomicBool::new(false);

struct PayloadSourceContinuationGuard;

impl PayloadSourceContinuationGuard {
    fn acquire() -> Self {
        while PAYLOAD_SOURCE_CONTINUATION_LOCK
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }
        Self
    }
}

impl Drop for PayloadSourceContinuationGuard {
    fn drop(&mut self) {
        PAYLOAD_SOURCE_CONTINUATION_LOCK.store(false, Ordering::Release);
    }
}

fn with_payload_source_continuations<R>(
    f: impl FnOnce(&mut PayloadSourceContinuationTable) -> R,
) -> R {
    let _guard = PayloadSourceContinuationGuard::acquire();
    // SAFETY: `PAYLOAD_SOURCE_CONTINUATION_LOCK` serializes access.
    let table = unsafe { &mut *PAYLOAD_SOURCE_CONTINUATIONS.0.get() };
    f(table)
}

fn reset_payload_source_continuations() {
    with_payload_source_continuations(PayloadSourceContinuationTable::reset);
}

fn store_payload_source_continuation(
    frame: PayloadSourceContinuation,
) -> Result<(), ProgramExecError> {
    with_payload_source_continuations(|table| table.store(frame))
}

fn can_store_payload_source_continuation(payload_pid: usize) -> bool {
    with_payload_source_continuations(|table| table.can_store(payload_pid))
}

fn take_payload_source_continuation(payload_pid: usize) -> Option<PayloadSourceContinuation> {
    with_payload_source_continuations(|table| table.take(payload_pid))
}

fn release_payload_source_continuation(payload_pid: usize) {
    with_payload_source_continuations(|table| table.release_pid(payload_pid));
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ProgramOpenFileDescription {
    kind: ProgramOpenFileDescriptionKind,
    state: ProgramVfsFdState,
    tty: ProgramTtyFdState,
    path: PathBuf,
    buffer: Option<NonNull<ProgramVfsFileBuffer>>,
    ref_count: usize,
}

impl ProgramOpenFileDescription {
    const fn closed() -> Self {
        Self {
            kind: ProgramOpenFileDescriptionKind::Vfs,
            state: ProgramVfsFdState::closed(),
            tty: ProgramTtyFdState::closed(),
            path: PathBuf::root(),
            buffer: None,
            ref_count: 0,
        }
    }

    fn is_open(&self) -> bool {
        match self.kind {
            ProgramOpenFileDescriptionKind::Vfs => self.state.open,
            ProgramOpenFileDescriptionKind::Tty => self.tty.open,
        }
    }
}

struct ProgramOpenFileTable {
    descriptions: [ProgramOpenFileDescription; MAX_PROGRAM_VFS_OPEN_FILES],
}

impl ProgramOpenFileTable {
    const fn new() -> Self {
        Self {
            descriptions: [ProgramOpenFileDescription::closed(); MAX_PROGRAM_VFS_OPEN_FILES],
        }
    }

    fn reset(&mut self) {
        let mut slot = 0usize;
        while slot < self.descriptions.len() {
            let description = self.descriptions[slot];
            if description.kind == ProgramOpenFileDescriptionKind::Vfs && description.state.open {
                release_program_vfs_file_buffer(description.state.buffer_slot);
            }
            self.descriptions[slot] = ProgramOpenFileDescription::closed();
            slot += 1;
        }
    }

    fn free_slot(&self) -> Option<usize> {
        let mut slot = 0usize;
        while slot < self.descriptions.len() {
            if !self.descriptions[slot].is_open() {
                return Some(slot);
            }
            slot += 1;
        }
        None
    }

    fn install_vfs_description(
        &mut self,
        kind: ProgramVfsOpenKind,
        path: PathBuf,
        buffer_slot: usize,
        buffer: NonNull<ProgramVfsFileBuffer>,
        len: usize,
    ) -> Result<usize, ProgramIoError> {
        let Some(slot) = self.free_slot() else {
            return Err(ProgramIoError::Busy);
        };
        self.descriptions[slot] = ProgramOpenFileDescription {
            kind: ProgramOpenFileDescriptionKind::Vfs,
            state: ProgramVfsFdState {
                open: true,
                kind,
                buffer_slot,
                cursor: 0,
                len,
                read_recorded: false,
            },
            tty: ProgramTtyFdState::closed(),
            path,
            buffer: Some(buffer),
            ref_count: 1,
        };
        Ok(slot)
    }

    fn install_tty_description(
        &mut self,
        path: PathBuf,
        access: ProgramOpenAccess,
    ) -> Result<usize, ProgramIoError> {
        let Some(slot) = self.free_slot() else {
            return Err(ProgramIoError::Busy);
        };
        self.descriptions[slot] = ProgramOpenFileDescription {
            kind: ProgramOpenFileDescriptionKind::Tty,
            state: ProgramVfsFdState::closed(),
            tty: ProgramTtyFdState::open(access),
            path,
            buffer: None,
            ref_count: 1,
        };
        Ok(slot)
    }

    fn description_mut(
        &mut self,
        slot: usize,
        kind: ProgramOpenFileDescriptionKind,
    ) -> Result<&mut ProgramOpenFileDescription, ProgramIoError> {
        let Some(description) = self.descriptions.get_mut(slot) else {
            return Err(ProgramIoError::BadFd);
        };
        if description.kind != kind || !description.is_open() {
            return Err(ProgramIoError::BadFd);
        }
        Ok(description)
    }

    fn description(
        &self,
        slot: usize,
        kind: ProgramOpenFileDescriptionKind,
    ) -> Result<&ProgramOpenFileDescription, ProgramIoError> {
        let Some(description) = self.descriptions.get(slot) else {
            return Err(ProgramIoError::BadFd);
        };
        if description.kind != kind || !description.is_open() {
            return Err(ProgramIoError::BadFd);
        }
        Ok(description)
    }

    fn retain(
        &mut self,
        slot: usize,
        kind: ProgramOpenFileDescriptionKind,
    ) -> Result<(), ProgramIoError> {
        let description = self.description_mut(slot, kind)?;
        description.ref_count = description.ref_count.saturating_add(1);
        Ok(())
    }

    fn release(
        &mut self,
        slot: usize,
        kind: ProgramOpenFileDescriptionKind,
    ) -> Result<(), ProgramIoError> {
        let description = self.description_mut(slot, kind)?;
        if description.ref_count > 1 {
            description.ref_count -= 1;
            return Ok(());
        }
        if description.kind == ProgramOpenFileDescriptionKind::Vfs {
            release_program_vfs_file_buffer(description.state.buffer_slot);
        }
        self.descriptions[slot] = ProgramOpenFileDescription::closed();
        Ok(())
    }

    fn validate(
        &self,
        slot: usize,
        kind: ProgramOpenFileDescriptionKind,
    ) -> Result<(), ProgramIoError> {
        self.description(slot, kind).map(|_| ())
    }

    fn directory_path(&self, slot: usize) -> Result<PathBuf, ProgramIoError> {
        let description = self.description(slot, ProgramOpenFileDescriptionKind::Vfs)?;
        if description.state.kind != ProgramVfsOpenKind::Directory {
            return Err(ProgramIoError::NotDirectory);
        }
        Ok(description.path)
    }

    fn read(
        &mut self,
        slot: usize,
        out: &mut [u8],
        kind: ProgramVfsOpenKind,
    ) -> Result<(usize, bool), ProgramIoError> {
        let description = self.description_mut(slot, ProgramOpenFileDescriptionKind::Vfs)?;
        if description.state.kind != kind {
            return Err(if kind == ProgramVfsOpenKind::Directory {
                ProgramIoError::NotDirectory
            } else {
                ProgramIoError::NotReadable
            });
        }
        let Some(buffer) = description.buffer else {
            return Err(ProgramIoError::BadFd);
        };
        let should_record = !description.state.read_recorded;
        if description.state.cursor >= description.state.len {
            description.state.read_recorded = true;
            return Ok((0, should_record));
        }
        // SAFETY: the open-file description owns the global file-buffer lock
        // until the final descriptor reference releases it.
        let read = unsafe { buffer.as_ref() }.read_at(description.state.cursor, out);
        description.state.cursor = description.state.cursor.saturating_add(read);
        description.state.read_recorded = true;
        Ok((read, should_record))
    }

    fn seek(
        &mut self,
        slot: usize,
        offset: isize,
        whence: SeekWhence,
    ) -> Result<usize, ProgramIoError> {
        let description = self.description_mut(slot, ProgramOpenFileDescriptionKind::Vfs)?;
        let base = if whence == SeekWhence::START {
            0
        } else if whence == SeekWhence::CURRENT {
            description.state.cursor
        } else if whence == SeekWhence::END {
            description.state.len
        } else {
            return Err(ProgramIoError::InvalidArgument);
        };
        let Some(next) = base.checked_add_signed(offset) else {
            return Err(ProgramIoError::InvalidArgument);
        };
        description.state.cursor = next;
        Ok(next)
    }

    fn read_tty(
        &mut self,
        slot: usize,
        out: &mut [u8],
    ) -> Result<Option<(usize, bool)>, ProgramIoError> {
        let description = self.description_mut(slot, ProgramOpenFileDescriptionKind::Tty)?;
        if !description.tty.readable {
            return Err(ProgramIoError::NotReadable);
        }
        if description.tty.needs_refill() {
            return Ok(None);
        }
        Ok(Some(description.tty.read(out)))
    }

    fn refill_tty(&mut self, slot: usize, bytes: &[u8]) -> Result<(), ProgramIoError> {
        let description = self.description_mut(slot, ProgramOpenFileDescriptionKind::Tty)?;
        description.tty.refill(bytes);
        Ok(())
    }

    fn validate_tty_write(&self, slot: usize) -> Result<(), ProgramIoError> {
        let description = self.description(slot, ProgramOpenFileDescriptionKind::Tty)?;
        if description.tty.writable {
            Ok(())
        } else {
            Err(ProgramIoError::NotWritable)
        }
    }
}

struct ProgramOpenFileTableCell(UnsafeCell<ProgramOpenFileTable>);

// SAFETY: mutable access is serialized by `PROGRAM_OPEN_FILE_TABLE_LOCK`.
unsafe impl Sync for ProgramOpenFileTableCell {}

static PROGRAM_OPEN_FILE_TABLE: ProgramOpenFileTableCell =
    ProgramOpenFileTableCell(UnsafeCell::new(ProgramOpenFileTable::new()));
static PROGRAM_OPEN_FILE_TABLE_LOCK: AtomicBool = AtomicBool::new(false);

struct ProgramOpenFileTableGuard;

impl ProgramOpenFileTableGuard {
    fn acquire() -> Self {
        while PROGRAM_OPEN_FILE_TABLE_LOCK
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }
        Self
    }
}

impl Drop for ProgramOpenFileTableGuard {
    fn drop(&mut self) {
        PROGRAM_OPEN_FILE_TABLE_LOCK.store(false, Ordering::Release);
    }
}

fn with_program_open_file_table<R>(f: impl FnOnce(&mut ProgramOpenFileTable) -> R) -> R {
    let _guard = ProgramOpenFileTableGuard::acquire();
    // SAFETY: `PROGRAM_OPEN_FILE_TABLE_LOCK` serializes access.
    let table = unsafe { &mut *PROGRAM_OPEN_FILE_TABLE.0.get() };
    f(table)
}

fn reset_program_open_file_table() {
    with_program_open_file_table(ProgramOpenFileTable::reset);
}

fn install_program_vfs_description(
    kind: ProgramVfsOpenKind,
    path: PathBuf,
    buffer_slot: usize,
    buffer: NonNull<ProgramVfsFileBuffer>,
    len: usize,
) -> Result<usize, ProgramIoError> {
    with_program_open_file_table(|table| {
        table.install_vfs_description(kind, path, buffer_slot, buffer, len)
    })
}

fn install_program_tty_description(
    path: PathBuf,
    access: ProgramOpenAccess,
) -> Result<usize, ProgramIoError> {
    with_program_open_file_table(|table| table.install_tty_description(path, access))
}

fn retain_program_open_file(
    slot: usize,
    kind: ProgramOpenFileDescriptionKind,
) -> Result<(), ProgramIoError> {
    with_program_open_file_table(|table| table.retain(slot, kind))
}

fn release_program_open_file(
    slot: usize,
    kind: ProgramOpenFileDescriptionKind,
) -> Result<(), ProgramIoError> {
    with_program_open_file_table(|table| table.release(slot, kind))
}

fn validate_program_open_file(
    slot: usize,
    kind: ProgramOpenFileDescriptionKind,
) -> Result<(), ProgramIoError> {
    with_program_open_file_table(|table| table.validate(slot, kind))
}

fn program_open_file_directory_path(slot: usize) -> Result<PathBuf, ProgramIoError> {
    with_program_open_file_table(|table| table.directory_path(slot))
}

fn read_program_open_file(
    slot: usize,
    out: &mut [u8],
    kind: ProgramVfsOpenKind,
) -> Result<(usize, bool), ProgramIoError> {
    with_program_open_file_table(|table| table.read(slot, out, kind))
}

fn seek_program_open_file(
    slot: usize,
    offset: isize,
    whence: SeekWhence,
) -> Result<usize, ProgramIoError> {
    with_program_open_file_table(|table| table.seek(slot, offset, whence))
}

fn read_program_tty(slot: usize, out: &mut [u8]) -> Result<Option<(usize, bool)>, ProgramIoError> {
    with_program_open_file_table(|table| table.read_tty(slot, out))
}

fn refill_program_tty(slot: usize, bytes: &[u8]) -> Result<(), ProgramIoError> {
    with_program_open_file_table(|table| table.refill_tty(slot, bytes))
}

fn validate_program_tty_write(slot: usize) -> Result<(), ProgramIoError> {
    with_program_open_file_table(|table| table.validate_tty_write(slot))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProgramFdDescriptor {
    Closed,
    Stdio(ProgramStream),
    Vfs(usize),
    Tty(usize),
    Pipe {
        pipe_slot: usize,
        end: ProgramPipeEnd,
    },
}

impl ProgramFdDescriptor {
    const fn is_open(self) -> bool {
        !matches!(self, Self::Closed)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ProgramFdDescriptorFlags {
    close_on_exec: bool,
}

impl ProgramFdDescriptorFlags {
    const fn empty() -> Self {
        Self {
            close_on_exec: false,
        }
    }

    const fn close_on_exec() -> Self {
        Self {
            close_on_exec: true,
        }
    }

    fn from_descriptor_flags(flags: DescriptorFlags) -> Result<Self, ProgramIoError> {
        if !flags.is_supported() {
            return Err(ProgramIoError::InvalidArgument);
        }
        Ok(Self {
            close_on_exec: flags.has_close_on_exec(),
        })
    }

    const fn to_descriptor_flags(self) -> DescriptorFlags {
        if self.close_on_exec {
            DescriptorFlags::CLOSE_ON_EXEC
        } else {
            DescriptorFlags::EMPTY
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ProgramFdTable {
    descriptors: [ProgramFdDescriptor; MAX_PROGRAM_FD_DESCRIPTORS],
    descriptor_flags: [ProgramFdDescriptorFlags; MAX_PROGRAM_FD_DESCRIPTORS],
    stdin: ProgramStdinBuffer,
    stdout_capture: Option<NonNull<ProgramStdoutCapture>>,
}

impl ProgramFdTable {
    const fn new() -> Self {
        Self {
            descriptors: [
                ProgramFdDescriptor::Stdio(ProgramStream::Stdin),
                ProgramFdDescriptor::Stdio(ProgramStream::Stdout),
                ProgramFdDescriptor::Stdio(ProgramStream::Stderr),
                ProgramFdDescriptor::Closed,
                ProgramFdDescriptor::Closed,
                ProgramFdDescriptor::Closed,
                ProgramFdDescriptor::Closed,
            ],
            descriptor_flags: [ProgramFdDescriptorFlags::empty(); MAX_PROGRAM_FD_DESCRIPTORS],
            stdin: ProgramStdinBuffer::empty(),
            stdout_capture: None,
        }
    }

    fn seed_stdin(&mut self, bytes: &[u8]) {
        self.stdin.seed(bytes);
    }

    fn seed_stdin_if_unseeded(&mut self, bytes: &[u8]) {
        if !self.stdin.is_seeded() {
            self.seed_stdin(bytes);
        }
    }

    fn set_stdout_capture(&mut self, capture: Option<NonNull<ProgramStdoutCapture>>) {
        self.stdout_capture = capture;
    }

    const fn stdout_capture(&self) -> Option<NonNull<ProgramStdoutCapture>> {
        self.stdout_capture
    }

    fn descriptor_for_fd(&self, fd: usize) -> Option<ProgramFdDescriptor> {
        let slot = program_descriptor_slot_for_fd(fd)?;
        let descriptor = self.descriptors[slot];
        if descriptor.is_open() {
            Some(descriptor)
        } else {
            None
        }
    }

    fn open_file_slot_for_fd(&self, fd: usize) -> Option<usize> {
        let slot = program_descriptor_slot_for_fd(fd)?;
        match self.descriptors[slot] {
            ProgramFdDescriptor::Vfs(open_file_slot) => Some(open_file_slot),
            ProgramFdDescriptor::Closed
            | ProgramFdDescriptor::Stdio(_)
            | ProgramFdDescriptor::Tty(_)
            | ProgramFdDescriptor::Pipe { .. } => None,
        }
    }

    fn free_descriptor_slot(&self) -> Option<usize> {
        let mut slot = 0usize;
        while slot < self.descriptors.len() {
            if self.descriptors[slot] == ProgramFdDescriptor::Closed {
                return Some(slot);
            }
            slot += 1;
        }
        None
    }

    fn free_descriptor_pair(&self) -> Option<(usize, usize)> {
        let first = self.free_descriptor_slot()?;
        let mut second = first + 1;
        while second < self.descriptors.len() {
            if self.descriptors[second] == ProgramFdDescriptor::Closed {
                return Some((first, second));
            }
            second += 1;
        }
        None
    }

    fn install_vfs_descriptor(
        &mut self,
        descriptor_slot: usize,
        open_file_slot: usize,
        flags: ProgramFdDescriptorFlags,
    ) {
        self.descriptors[descriptor_slot] = ProgramFdDescriptor::Vfs(open_file_slot);
        self.descriptor_flags[descriptor_slot] = flags;
    }

    fn install_tty_descriptor(
        &mut self,
        descriptor_slot: usize,
        open_file_slot: usize,
        flags: ProgramFdDescriptorFlags,
    ) {
        self.descriptors[descriptor_slot] = ProgramFdDescriptor::Tty(open_file_slot);
        self.descriptor_flags[descriptor_slot] = flags;
    }

    fn create_pipe(&mut self) -> Result<(usize, usize), ProgramIoError> {
        let Some((read_descriptor_slot, write_descriptor_slot)) = self.free_descriptor_pair()
        else {
            return Err(ProgramIoError::Busy);
        };
        let pipe_slot = create_program_pipe()?;

        self.descriptors[read_descriptor_slot] = ProgramFdDescriptor::Pipe {
            pipe_slot,
            end: ProgramPipeEnd::Read,
        };
        self.descriptor_flags[read_descriptor_slot] = ProgramFdDescriptorFlags::empty();
        self.descriptors[write_descriptor_slot] = ProgramFdDescriptor::Pipe {
            pipe_slot,
            end: ProgramPipeEnd::Write,
        };
        self.descriptor_flags[write_descriptor_slot] = ProgramFdDescriptorFlags::empty();

        Ok((
            program_fd_for_descriptor_slot(read_descriptor_slot),
            program_fd_for_descriptor_slot(write_descriptor_slot),
        ))
    }

    fn directory_path_for_fd(&self, fd: usize) -> Result<PathBuf, ProgramIoError> {
        let Some(descriptor_slot) = program_descriptor_slot_for_fd(fd) else {
            return Err(ProgramIoError::BadFd);
        };
        match self.descriptors[descriptor_slot] {
            ProgramFdDescriptor::Closed => Err(ProgramIoError::BadFd),
            ProgramFdDescriptor::Stdio(_) => Err(ProgramIoError::NotDirectory),
            ProgramFdDescriptor::Vfs(open_file_slot) => {
                program_open_file_directory_path(open_file_slot)
            }
            ProgramFdDescriptor::Tty(_) | ProgramFdDescriptor::Pipe { .. } => {
                Err(ProgramIoError::NotDirectory)
            }
        }
    }

    fn retain_descriptor(&mut self, descriptor: ProgramFdDescriptor) -> Result<(), ProgramIoError> {
        match descriptor {
            ProgramFdDescriptor::Closed => Err(ProgramIoError::BadFd),
            ProgramFdDescriptor::Stdio(_) => Ok(()),
            ProgramFdDescriptor::Vfs(open_file_slot) => {
                retain_program_open_file(open_file_slot, ProgramOpenFileDescriptionKind::Vfs)
            }
            ProgramFdDescriptor::Tty(open_file_slot) => {
                retain_program_open_file(open_file_slot, ProgramOpenFileDescriptionKind::Tty)
            }
            ProgramFdDescriptor::Pipe { pipe_slot, end } => {
                retain_program_pipe_endpoint(pipe_slot, end)
            }
        }
    }

    fn validate_descriptor(&self, descriptor: ProgramFdDescriptor) -> Result<(), ProgramIoError> {
        match descriptor {
            ProgramFdDescriptor::Closed => Err(ProgramIoError::BadFd),
            ProgramFdDescriptor::Stdio(_) => Ok(()),
            ProgramFdDescriptor::Vfs(open_file_slot) => {
                validate_program_open_file(open_file_slot, ProgramOpenFileDescriptionKind::Vfs)
            }
            ProgramFdDescriptor::Tty(open_file_slot) => {
                validate_program_open_file(open_file_slot, ProgramOpenFileDescriptionKind::Tty)
            }
            ProgramFdDescriptor::Pipe { pipe_slot, .. } => validate_program_pipe(pipe_slot),
        }
    }

    fn release_descriptor(
        &mut self,
        descriptor: ProgramFdDescriptor,
    ) -> Result<(), ProgramIoError> {
        match descriptor {
            ProgramFdDescriptor::Closed => Err(ProgramIoError::BadFd),
            ProgramFdDescriptor::Stdio(_) => Ok(()),
            ProgramFdDescriptor::Vfs(open_file_slot) => {
                release_program_open_file(open_file_slot, ProgramOpenFileDescriptionKind::Vfs)
            }
            ProgramFdDescriptor::Tty(open_file_slot) => {
                release_program_open_file(open_file_slot, ProgramOpenFileDescriptionKind::Tty)
            }
            ProgramFdDescriptor::Pipe { pipe_slot, end } => {
                let wake = release_program_pipe_endpoint(pipe_slot, end)?;
                wake.wake();
                Ok(())
            }
        }
    }

    fn duplicate_fd(&mut self, fd: usize) -> Result<usize, ProgramIoError> {
        let Some(descriptor_slot) = program_descriptor_slot_for_fd(fd) else {
            return Err(ProgramIoError::BadFd);
        };
        let descriptor = self.descriptors[descriptor_slot];
        if !descriptor.is_open() {
            return Err(ProgramIoError::BadFd);
        }
        self.validate_descriptor(descriptor)?;
        let Some(new_descriptor_slot) = self.free_descriptor_slot() else {
            return Err(ProgramIoError::Busy);
        };
        self.retain_descriptor(descriptor)?;
        self.descriptors[new_descriptor_slot] = descriptor;
        self.descriptor_flags[new_descriptor_slot] = ProgramFdDescriptorFlags::empty();
        Ok(program_fd_for_descriptor_slot(new_descriptor_slot))
    }

    fn duplicate_fd_to(&mut self, old_fd: usize, new_fd: usize) -> Result<usize, ProgramIoError> {
        let Some(old_descriptor_slot) = program_descriptor_slot_for_fd(old_fd) else {
            return Err(ProgramIoError::BadFd);
        };
        let Some(new_descriptor_slot) = program_descriptor_slot_for_fd(new_fd) else {
            return Err(ProgramIoError::BadFd);
        };
        let descriptor = self.descriptors[old_descriptor_slot];
        if !descriptor.is_open() {
            return Err(ProgramIoError::BadFd);
        }
        self.validate_descriptor(descriptor)?;
        if old_descriptor_slot == new_descriptor_slot {
            return Ok(new_fd);
        }

        if self.descriptors[new_descriptor_slot] == descriptor {
            self.descriptor_flags[new_descriptor_slot] = ProgramFdDescriptorFlags::empty();
            return Ok(new_fd);
        }
        if self.descriptors[new_descriptor_slot].is_open() {
            self.close_fd(new_fd)?;
        }
        self.retain_descriptor(descriptor)?;
        self.descriptors[new_descriptor_slot] = descriptor;
        self.descriptor_flags[new_descriptor_slot] = ProgramFdDescriptorFlags::empty();
        Ok(new_fd)
    }

    fn descriptor_flags_for_fd(&self, fd: usize) -> Result<DescriptorFlags, ProgramIoError> {
        let Some(descriptor_slot) = program_descriptor_slot_for_fd(fd) else {
            return Err(ProgramIoError::BadFd);
        };
        let descriptor = self.descriptors[descriptor_slot];
        if !descriptor.is_open() {
            return Err(ProgramIoError::BadFd);
        }
        self.validate_descriptor(descriptor)?;
        Ok(self.descriptor_flags[descriptor_slot].to_descriptor_flags())
    }

    fn set_descriptor_flags_for_fd(
        &mut self,
        fd: usize,
        flags: DescriptorFlags,
    ) -> Result<(), ProgramIoError> {
        let flags = ProgramFdDescriptorFlags::from_descriptor_flags(flags)?;
        let Some(descriptor_slot) = program_descriptor_slot_for_fd(fd) else {
            return Err(ProgramIoError::BadFd);
        };
        let descriptor = self.descriptors[descriptor_slot];
        if !descriptor.is_open() {
            return Err(ProgramIoError::BadFd);
        }
        self.validate_descriptor(descriptor)?;
        self.descriptor_flags[descriptor_slot] = flags;
        Ok(())
    }

    fn status_flags_for_fd(&self, fd: usize) -> Result<FileStatusFlags, ProgramIoError> {
        let Some(descriptor_slot) = program_descriptor_slot_for_fd(fd) else {
            return Err(ProgramIoError::BadFd);
        };
        let descriptor = self.descriptors[descriptor_slot];
        if !descriptor.is_open() {
            return Err(ProgramIoError::BadFd);
        }
        self.validate_descriptor(descriptor)?;
        match descriptor {
            ProgramFdDescriptor::Pipe { pipe_slot, end } => {
                Ok(program_pipe_status_flags(pipe_slot, end)?.to_file_status_flags())
            }
            ProgramFdDescriptor::Closed => Err(ProgramIoError::BadFd),
            ProgramFdDescriptor::Stdio(_)
            | ProgramFdDescriptor::Vfs(_)
            | ProgramFdDescriptor::Tty(_) => Ok(FileStatusFlags::EMPTY),
        }
    }

    fn set_status_flags_for_fd(
        &mut self,
        fd: usize,
        flags: FileStatusFlags,
    ) -> Result<(), ProgramIoError> {
        let flags = ProgramFdStatusFlags::from_file_status_flags(flags)?;
        let Some(descriptor_slot) = program_descriptor_slot_for_fd(fd) else {
            return Err(ProgramIoError::BadFd);
        };
        let descriptor = self.descriptors[descriptor_slot];
        if !descriptor.is_open() {
            return Err(ProgramIoError::BadFd);
        }
        self.validate_descriptor(descriptor)?;
        match descriptor {
            ProgramFdDescriptor::Pipe { pipe_slot, end } => {
                set_program_pipe_status_flags(pipe_slot, end, flags)
            }
            ProgramFdDescriptor::Closed => Err(ProgramIoError::BadFd),
            ProgramFdDescriptor::Stdio(_)
            | ProgramFdDescriptor::Vfs(_)
            | ProgramFdDescriptor::Tty(_) => {
                if flags == ProgramFdStatusFlags::empty() {
                    Ok(())
                } else {
                    Err(ProgramIoError::InvalidArgument)
                }
            }
        }
    }

    fn pipe_endpoint_for_fd(&self, fd: usize) -> Result<(usize, ProgramPipeEnd), ProgramIoError> {
        let Some(descriptor_slot) = program_descriptor_slot_for_fd(fd) else {
            return Err(ProgramIoError::BadFd);
        };
        match self.descriptors[descriptor_slot] {
            ProgramFdDescriptor::Pipe { pipe_slot, end } => Ok((pipe_slot, end)),
            ProgramFdDescriptor::Closed => Err(ProgramIoError::BadFd),
            ProgramFdDescriptor::Stdio(_)
            | ProgramFdDescriptor::Vfs(_)
            | ProgramFdDescriptor::Tty(_) => Err(ProgramIoError::BadFd),
        }
    }

    fn install_pipe_endpoint_at(
        &mut self,
        fd: usize,
        pipe_slot: usize,
        end: ProgramPipeEnd,
    ) -> Result<(), ProgramIoError> {
        let Some(descriptor_slot) = program_descriptor_slot_for_fd(fd) else {
            return Err(ProgramIoError::BadFd);
        };
        retain_program_pipe_endpoint(pipe_slot, end)?;
        if self.descriptors[descriptor_slot].is_open() {
            if let Err(error) = self.close_fd(fd) {
                let _ = release_program_pipe_endpoint(pipe_slot, end);
                return Err(error);
            }
        }
        self.descriptors[descriptor_slot] = ProgramFdDescriptor::Pipe { pipe_slot, end };
        self.descriptor_flags[descriptor_slot] = ProgramFdDescriptorFlags::empty();
        Ok(())
    }

    fn read_stdin(&mut self, out: &mut [u8]) -> usize {
        self.stdin.read(out)
    }

    fn read_pipe(&mut self, fd: usize, out: &mut [u8]) -> Result<(usize, bool), ProgramIoError> {
        let Some(descriptor_slot) = program_descriptor_slot_for_fd(fd) else {
            return Err(ProgramIoError::BadFd);
        };
        let pipe_slot = match self.descriptors[descriptor_slot] {
            ProgramFdDescriptor::Pipe {
                pipe_slot,
                end: ProgramPipeEnd::Read,
            } => pipe_slot,
            ProgramFdDescriptor::Pipe {
                end: ProgramPipeEnd::Write,
                ..
            } => return Err(ProgramIoError::NotReadable),
            ProgramFdDescriptor::Closed => return Err(ProgramIoError::BadFd),
            ProgramFdDescriptor::Stdio(_)
            | ProgramFdDescriptor::Vfs(_)
            | ProgramFdDescriptor::Tty(_) => {
                return Err(ProgramIoError::BadFd);
            }
        };
        let status = program_pipe_status_flags(pipe_slot, ProgramPipeEnd::Read)?;
        match read_program_pipe(pipe_slot, out) {
            Err(ProgramIoError::Busy) if status.is_nonblocking() => Err(ProgramIoError::WouldBlock),
            result => result,
        }
    }

    fn write_pipe(
        &mut self,
        fd: usize,
        bytes: &[u8],
        read_waiters: &mut [usize],
    ) -> Result<(usize, bool, usize), ProgramIoError> {
        let Some(descriptor_slot) = program_descriptor_slot_for_fd(fd) else {
            return Err(ProgramIoError::BadFd);
        };
        let pipe_slot = match self.descriptors[descriptor_slot] {
            ProgramFdDescriptor::Pipe {
                pipe_slot,
                end: ProgramPipeEnd::Write,
            } => pipe_slot,
            ProgramFdDescriptor::Pipe {
                end: ProgramPipeEnd::Read,
                ..
            } => return Err(ProgramIoError::NotWritable),
            ProgramFdDescriptor::Closed => return Err(ProgramIoError::BadFd),
            ProgramFdDescriptor::Stdio(_)
            | ProgramFdDescriptor::Vfs(_)
            | ProgramFdDescriptor::Tty(_) => {
                return Err(ProgramIoError::BadFd);
            }
        };
        let status = program_pipe_status_flags(pipe_slot, ProgramPipeEnd::Write)?;
        match write_program_pipe(pipe_slot, bytes, read_waiters) {
            Err(ProgramIoError::Busy) if status.is_nonblocking() => Err(ProgramIoError::WouldBlock),
            result => result,
        }
    }

    fn read_tty(
        &mut self,
        fd: usize,
        out: &mut [u8],
    ) -> Result<Option<(usize, bool)>, ProgramIoError> {
        let Some(descriptor_slot) = program_descriptor_slot_for_fd(fd) else {
            return Err(ProgramIoError::BadFd);
        };
        let open_file_slot = match self.descriptors[descriptor_slot] {
            ProgramFdDescriptor::Tty(open_file_slot) => open_file_slot,
            ProgramFdDescriptor::Closed => return Err(ProgramIoError::BadFd),
            ProgramFdDescriptor::Stdio(_)
            | ProgramFdDescriptor::Vfs(_)
            | ProgramFdDescriptor::Pipe { .. } => {
                return Err(ProgramIoError::BadFd);
            }
        };
        read_program_tty(open_file_slot, out)
    }

    fn refill_tty(&mut self, fd: usize, bytes: &[u8]) -> Result<(), ProgramIoError> {
        let Some(descriptor_slot) = program_descriptor_slot_for_fd(fd) else {
            return Err(ProgramIoError::BadFd);
        };
        let open_file_slot = match self.descriptors[descriptor_slot] {
            ProgramFdDescriptor::Tty(open_file_slot) => open_file_slot,
            ProgramFdDescriptor::Closed => return Err(ProgramIoError::BadFd),
            ProgramFdDescriptor::Stdio(_)
            | ProgramFdDescriptor::Vfs(_)
            | ProgramFdDescriptor::Pipe { .. } => {
                return Err(ProgramIoError::BadFd);
            }
        };
        refill_program_tty(open_file_slot, bytes)
    }

    fn close_fd(&mut self, fd: usize) -> Result<(), ProgramIoError> {
        let Some(descriptor_slot) = program_descriptor_slot_for_fd(fd) else {
            return Err(ProgramIoError::BadFd);
        };
        let descriptor = self.descriptors[descriptor_slot];
        self.descriptors[descriptor_slot] = ProgramFdDescriptor::Closed;
        self.descriptor_flags[descriptor_slot] = ProgramFdDescriptorFlags::empty();
        self.release_descriptor(descriptor)
    }

    fn close_all(&mut self) {
        let mut slot = 0usize;
        while slot < self.descriptors.len() {
            if self.descriptors[slot].is_open() {
                let _ = self.close_fd(program_fd_for_descriptor_slot(slot));
            }
            slot += 1;
        }
    }

    fn close_on_exec_descriptors(&mut self) {
        let mut slot = 0usize;
        while slot < self.descriptors.len() {
            if self.descriptor_flags[slot].close_on_exec && self.descriptors[slot].is_open() {
                let _ = self.close_fd(program_fd_for_descriptor_slot(slot));
            }
            slot += 1;
        }
    }

    fn inherit_from(&mut self, parent: &ProgramFdTable) -> Result<(), ProgramIoError> {
        self.close_all();
        *self = Self::new();
        self.close_all();
        self.stdin = parent.stdin;

        let mut slot = 0usize;
        while slot < parent.descriptors.len() {
            let descriptor = parent.descriptors[slot];
            if descriptor.is_open() {
                parent.validate_descriptor(descriptor)?;
                if let Err(error) = self.retain_descriptor(descriptor) {
                    self.close_all();
                    return Err(error);
                }
                self.descriptors[slot] = descriptor;
                self.descriptor_flags[slot] = parent.descriptor_flags[slot];
            }
            slot += 1;
        }

        self.stdout_capture = None;
        Ok(())
    }

    fn install_stdin_bytes(&mut self, bytes: &[u8]) -> Result<(), ProgramIoError> {
        if bytes.is_empty() {
            return Ok(());
        }
        if self.descriptors[0].is_open() {
            self.close_fd(0)?;
        }
        self.descriptors[0] = ProgramFdDescriptor::Stdio(ProgramStream::Stdin);
        self.descriptor_flags[0] = ProgramFdDescriptorFlags::empty();
        self.seed_stdin(bytes);
        Ok(())
    }

    fn read(
        &mut self,
        open_file_slot: usize,
        out: &mut [u8],
    ) -> Result<(usize, bool), ProgramIoError> {
        self.read_with_kind(open_file_slot, out, ProgramVfsOpenKind::File)
    }

    fn read_directory(
        &mut self,
        open_file_slot: usize,
        out: &mut [u8],
    ) -> Result<(usize, bool), ProgramIoError> {
        self.read_with_kind(open_file_slot, out, ProgramVfsOpenKind::Directory)
    }

    fn read_with_kind(
        &mut self,
        open_file_slot: usize,
        out: &mut [u8],
        kind: ProgramVfsOpenKind,
    ) -> Result<(usize, bool), ProgramIoError> {
        read_program_open_file(open_file_slot, out, kind)
    }

    fn seek(
        &mut self,
        open_file_slot: usize,
        offset: isize,
        whence: SeekWhence,
    ) -> Result<usize, ProgramIoError> {
        seek_program_open_file(open_file_slot, offset, whence)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ProgramProcessFdTableSlot {
    pid: usize,
    table: ProgramFdTable,
}

impl ProgramProcessFdTableSlot {
    const fn empty() -> Self {
        Self {
            pid: 0,
            table: ProgramFdTable::new(),
        }
    }
}

struct ProgramProcessFdTables {
    slots: [ProgramProcessFdTableSlot; proc::MAX_PROCESSES],
}

impl ProgramProcessFdTables {
    const fn new() -> Self {
        Self {
            slots: [ProgramProcessFdTableSlot::empty(); proc::MAX_PROCESSES],
        }
    }

    fn reset(&mut self) {
        let mut index = 0usize;
        while index < self.slots.len() {
            self.slots[index].table.close_all();
            self.slots[index] = ProgramProcessFdTableSlot::empty();
            index += 1;
        }
    }

    fn slot_index(&self, pid: usize) -> Option<usize> {
        if pid == 0 {
            return None;
        }
        let mut index = 0usize;
        while index < self.slots.len() {
            if self.slots[index].pid == pid {
                return Some(index);
            }
            index += 1;
        }
        None
    }

    fn ensure_slot_index(&mut self, pid: usize) -> Option<usize> {
        if let Some(index) = self.slot_index(pid) {
            return Some(index);
        }
        if pid == 0 {
            return None;
        }
        let mut index = 0usize;
        while index < self.slots.len() {
            if self.slots[index].pid == 0 {
                self.slots[index].pid = pid;
                self.slots[index].table = ProgramFdTable::new();
                return Some(index);
            }
            index += 1;
        }
        None
    }

    fn table_mut(&mut self, pid: usize) -> Option<&mut ProgramFdTable> {
        let index = self.ensure_slot_index(pid)?;
        Some(&mut self.slots[index].table)
    }

    fn existing_table_mut(&mut self, pid: usize) -> Option<&mut ProgramFdTable> {
        let index = self.slot_index(pid)?;
        Some(&mut self.slots[index].table)
    }

    fn release_pid(&mut self, pid: usize) {
        let mut index = 0usize;
        while index < self.slots.len() {
            if self.slots[index].pid == pid {
                self.slots[index].table.close_all();
                self.slots[index] = ProgramProcessFdTableSlot::empty();
                return;
            }
            index += 1;
        }
    }

    fn inherit_child(&mut self, parent_pid: usize, child_pid: usize) -> Result<(), ProgramIoError> {
        let Some(parent_index) = self.slot_index(parent_pid) else {
            return Ok(());
        };
        let Some(child_index) = self.ensure_slot_index(child_pid) else {
            return Err(ProgramIoError::Busy);
        };
        if parent_index == child_index {
            return Ok(());
        }
        let parent = self.slots[parent_index].table;
        self.slots[child_index].table.inherit_from(&parent)
    }

    fn install_child_stdin(
        &mut self,
        child_pid: usize,
        bytes: &[u8],
    ) -> Result<(), ProgramIoError> {
        let Some(child_index) = self.ensure_slot_index(child_pid) else {
            return Err(ProgramIoError::Busy);
        };
        self.slots[child_index].table.install_stdin_bytes(bytes)
    }
}

struct ProgramProcessFdTablesCell(UnsafeCell<ProgramProcessFdTables>);

// SAFETY: mutable access is serialized by `PROGRAM_PROCESS_FD_TABLES_LOCK`.
unsafe impl Sync for ProgramProcessFdTablesCell {}

static PROGRAM_PROCESS_FD_TABLES: ProgramProcessFdTablesCell =
    ProgramProcessFdTablesCell(UnsafeCell::new(ProgramProcessFdTables::new()));
static PROGRAM_PROCESS_FD_TABLES_LOCK: AtomicBool = AtomicBool::new(false);

struct ProgramProcessFdTablesGuard;

impl ProgramProcessFdTablesGuard {
    fn acquire() -> Self {
        while PROGRAM_PROCESS_FD_TABLES_LOCK
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }
        Self
    }
}

impl Drop for ProgramProcessFdTablesGuard {
    fn drop(&mut self) {
        PROGRAM_PROCESS_FD_TABLES_LOCK.store(false, Ordering::Release);
    }
}

fn with_process_fd_tables<R>(f: impl FnOnce(&mut ProgramProcessFdTables) -> R) -> R {
    let _guard = ProgramProcessFdTablesGuard::acquire();
    // SAFETY: `PROGRAM_PROCESS_FD_TABLES_LOCK` serializes access.
    let tables = unsafe { &mut *PROGRAM_PROCESS_FD_TABLES.0.get() };
    f(tables)
}

fn reset_process_fd_tables() {
    with_process_fd_tables(ProgramProcessFdTables::reset);
}

fn with_process_fd_table_mut<R>(pid: usize, f: impl FnOnce(&mut ProgramFdTable) -> R) -> Option<R> {
    with_process_fd_tables(|tables| tables.table_mut(pid).map(f))
}

fn with_existing_process_fd_table_mut<R>(
    pid: usize,
    f: impl FnOnce(&mut ProgramFdTable) -> R,
) -> Option<R> {
    with_process_fd_tables(|tables| tables.existing_table_mut(pid).map(f))
}

fn process_fd_descriptor(pid: usize, fd: usize) -> Option<ProgramFdDescriptor> {
    with_process_fd_tables(|tables| {
        tables
            .table_mut(pid)
            .and_then(|table| table.descriptor_for_fd(fd))
    })
}

fn process_stdout_capture(pid: usize) -> Option<NonNull<ProgramStdoutCapture>> {
    with_process_fd_tables(|tables| {
        tables
            .existing_table_mut(pid)
            .and_then(|table| table.stdout_capture())
    })
}

fn duplicate_process_pipe_fd_to(
    source_pid: usize,
    source_fd: usize,
    target_pid: usize,
    target_fd: usize,
) -> Result<(), ProgramIoError> {
    with_process_fd_tables(|tables| {
        let (pipe_slot, end) = {
            let Some(source) = tables.existing_table_mut(source_pid) else {
                return Err(ProgramIoError::BadFd);
            };
            source.pipe_endpoint_for_fd(source_fd)?
        };
        let Some(target) = tables.table_mut(target_pid) else {
            return Err(ProgramIoError::Busy);
        };
        target.install_pipe_endpoint_at(target_fd, pipe_slot, end)
    })
}

fn close_process_fd(pid: usize, fd: usize) -> Result<(), ProgramIoError> {
    with_process_fd_tables(|tables| {
        let Some(table) = tables.existing_table_mut(pid) else {
            return Err(ProgramIoError::BadFd);
        };
        table.close_fd(fd)
    })
}

fn release_process_fd_table(pid: usize) {
    with_process_fd_tables(|tables| tables.release_pid(pid));
}

fn inherit_process_fd_table(parent_pid: usize, child_pid: usize) -> Result<(), ProgramIoError> {
    with_process_fd_tables(|tables| tables.inherit_child(parent_pid, child_pid))
}

fn install_process_stdin_bytes(pid: usize, bytes: &[u8]) -> Result<(), ProgramIoError> {
    with_process_fd_tables(|tables| tables.install_child_stdin(pid, bytes))
}

fn process_context_for_pid(pid: usize) -> Option<SyscallContext> {
    proc::process(pid).map(|record| SyscallContext::from_process(process_record_handle(record)))
}

fn record_process_fd_op(pid: usize, op: SyscallOp, result: Result<(), ProgramIoError>) {
    let status = if result.is_ok() {
        SyscallStatus::Ok
    } else {
        SyscallStatus::Error
    };
    record_syscall(process_context_for_pid(pid), op, status);
}

pub(crate) fn create_process_pipe_fds(pid: usize) -> Result<(usize, usize), ProgramIoError> {
    let result = with_process_fd_table_mut(pid, ProgramFdTable::create_pipe)
        .ok_or(ProgramIoError::BadFd)
        .and_then(core::convert::identity);
    let status = if result.is_ok() {
        SyscallStatus::Ok
    } else {
        SyscallStatus::Error
    };
    record_syscall(process_context_for_pid(pid), SyscallOp::FdPipe, status);
    result
}

pub(crate) fn duplicate_process_pipe_fd_to_process(
    source_pid: usize,
    source_fd: usize,
    target_pid: usize,
    target_fd: usize,
) -> Result<(), ProgramIoError> {
    let result = duplicate_process_pipe_fd_to(source_pid, source_fd, target_pid, target_fd);
    record_process_fd_op(target_pid, SyscallOp::FdDuplicate, result);
    result
}

pub(crate) fn close_process_fd_for_pipeline(pid: usize, fd: usize) -> Result<(), ProgramIoError> {
    let result = close_process_fd(pid, fd);
    record_process_fd_op(pid, SyscallOp::FdClose, result);
    result
}

pub(crate) fn close_on_exec_process_fds(pid: usize) {
    let _ = with_existing_process_fd_table_mut(pid, ProgramFdTable::close_on_exec_descriptors);
}

fn inherit_process_fds_for_exec(parent_pid: usize, child_pid: usize) -> Result<(), ProgramIoError> {
    inherit_process_fd_table(parent_pid, child_pid)?;
    close_on_exec_process_fds(child_pid);
    Ok(())
}

fn complete_program_process(pid: usize, status: ProgramStatus) -> ProcessAdoptionSnapshot {
    if status == ProgramStatus::Blocked {
        return ProcessAdoptionSnapshot::empty();
    }
    program::release_bin_uapi_resume_frame(pid);
    release_program_syscall_continuation(pid);
    release_program_pipe_read_waiters(pid);
    release_program_pipe_write_waiters(pid);
    release_process_fd_table(pid);
    let adoption = exec::complete_program(pid, status);
    finish_retained_wait_for_child(pid);
    adoption
}

fn complete_payload_process(pid: usize, result: PayloadLaunchResult) -> ProcessAdoptionSnapshot {
    if result != PayloadLaunchResult::Resident {
        release_payload_source_continuation(pid);
        release_program_syscall_continuation(pid);
        release_program_pipe_read_waiters(pid);
        release_program_pipe_write_waiters(pid);
        release_process_fd_table(pid);
    }
    let adoption = exec::complete_payload(pid, result);
    if result != PayloadLaunchResult::Resident {
        finish_retained_wait_for_child(pid);
    }
    adoption
}

fn finish_retained_wait_for_child(child_pid: usize) {
    let _ = proc::complete_wait_for_child(child_pid);
}

/// Syscall context for one running image program.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SyscallContext {
    /// Current process identifier.
    pub pid: usize,
    /// Current task identifier.
    pub task_id: usize,
    /// Executable path.
    pub program_path: &'static str,
    /// Loader/source kind for the executable image.
    pub loader: &'static str,
    /// Stable executable entry name.
    pub entry_name: &'static str,
}

impl SyscallContext {
    pub(crate) const fn from_process(handle: ProcessHandle) -> Self {
        Self {
            pid: handle.pid,
            task_id: handle.task_id,
            program_path: handle.program_path,
            loader: handle.loader,
            entry_name: handle.entry_name,
        }
    }
}

/// Starts a root-shell `/bin` exec request and leaves it pending for scheduler dispatch.
#[must_use]
pub fn exec_bin_from_shell(program: LoadedProgram, argv: ProgramArgvBuffer) -> SyscallContext {
    exec_bin_from_shell_with_stdin(program, argv, &[])
}

/// Starts a root-shell `/bin` exec request with a bounded initial stdin payload.
#[must_use]
pub fn exec_bin_from_shell_with_stdin(
    program: LoadedProgram,
    argv: ProgramArgvBuffer,
    stdin: &[u8],
) -> SyscallContext {
    let handle = exec::spawn_bin_program_with_stdin(program, argv, stdin);
    let ctx = SyscallContext::from_process(handle);
    record_context(ctx, SyscallOp::ExecLoad, SyscallStatus::Ok);
    record_context(ctx, SyscallOp::ExecSpawn, SyscallStatus::Ok);
    ctx
}

fn try_exec_bin_from_shell_with_env_and_stdin(
    program: LoadedProgram,
    argv: ProgramArgvBuffer,
    env: ProgramEnvBuffer,
    stdin: &[u8],
) -> Result<SyscallContext, exec::ExecLoadError> {
    let Some(handle) = exec::try_spawn_bin_program_with_env_and_stdin(program, argv, env, stdin)
    else {
        record_syscall(None, SyscallOp::ExecSpawn, SyscallStatus::Error);
        return Err(exec::ExecLoadError::ProcessAdmissionFailed);
    };
    let ctx = SyscallContext::from_process(handle);
    record_context(ctx, SyscallOp::ExecLoad, SyscallStatus::Ok);
    record_context(ctx, SyscallOp::ExecSpawn, SyscallStatus::Ok);
    Ok(ctx)
}

/// Starts a rootd-owned `/bin` exec request with a bounded initial stdin payload.
#[must_use]
pub fn exec_bin_from_rootd_with_stdin(
    program: LoadedProgram,
    argv: ProgramArgvBuffer,
    stdin: &[u8],
) -> SyscallContext {
    let handle = exec::spawn_rootd_bin_program_with_stdin(program, argv, stdin);
    let ctx = SyscallContext::from_process(handle);
    record_context(ctx, SyscallOp::ExecLoad, SyscallStatus::Ok);
    record_context(ctx, SyscallOp::ExecSpawn, SyscallStatus::Ok);
    ctx
}

fn try_exec_bin_from_rootd_with_stdin(
    program: LoadedProgram,
    argv: ProgramArgvBuffer,
    stdin: &[u8],
) -> Result<SyscallContext, exec::ExecLoadError> {
    let Some(handle) = exec::try_spawn_rootd_bin_program_with_stdin(program, argv, stdin) else {
        record_syscall(None, SyscallOp::ExecSpawn, SyscallStatus::Error);
        return Err(exec::ExecLoadError::ProcessAdmissionFailed);
    };
    let ctx = SyscallContext::from_process(handle);
    record_context(ctx, SyscallOp::ExecLoad, SyscallStatus::Ok);
    record_context(ctx, SyscallOp::ExecSpawn, SyscallStatus::Ok);
    Ok(ctx)
}

/// Loads argv0 through exec and starts a root-shell `/bin` exec request.
pub fn exec_bin_from_shell_argv0(
    programs: &'static [ProgramDescriptor],
    source_store: ExecutableSourceStore,
    argv0: &str,
) -> Result<SyscallContext, exec::ExecLoadError> {
    let argv = match ProgramArgvBuffer::from_argv0(argv0) {
        Ok(argv) => argv,
        Err(_) => {
            record_syscall(None, SyscallOp::ExecLoad, SyscallStatus::Error);
            return Err(exec::ExecLoadError::EmptyArgv0);
        }
    };
    exec_bin_from_shell_argv_with_stdin(programs, source_store, argv, &[])
}

/// Loads argv0 through exec and starts a rootd-owned `/bin` exec request.
pub fn exec_bin_from_rootd_argv0(
    programs: &'static [ProgramDescriptor],
    source_store: ExecutableSourceStore,
    argv0: &str,
) -> Result<SyscallContext, exec::ExecLoadError> {
    let argv = match ProgramArgvBuffer::from_argv0(argv0) {
        Ok(argv) => argv,
        Err(_) => {
            record_syscall(None, SyscallOp::ExecLoad, SyscallStatus::Error);
            return Err(exec::ExecLoadError::EmptyArgv0);
        }
    };
    exec_bin_from_rootd_argv_with_stdin(programs, source_store, argv, &[])
}

fn exec_bin_from_rootd_argv_with_stdin(
    programs: &'static [ProgramDescriptor],
    source_store: ExecutableSourceStore,
    argv: ProgramArgvBuffer,
    stdin: &[u8],
) -> Result<SyscallContext, exec::ExecLoadError> {
    let Some(argv0) = argv.argv0() else {
        record_syscall(None, SyscallOp::ExecLoad, SyscallStatus::Error);
        return Err(exec::ExecLoadError::EmptyArgv0);
    };
    let program = match exec::load_bin_program(programs, source_store, argv0) {
        Ok(program) => program,
        Err(error) => {
            record_syscall(None, SyscallOp::ExecLoad, SyscallStatus::Error);
            return Err(error);
        }
    };
    try_exec_bin_from_rootd_with_stdin(program, argv, stdin)
}

/// Loads argv[0] through exec and starts a root-shell `/bin` exec request.
pub fn exec_bin_from_shell_argv(
    programs: &'static [ProgramDescriptor],
    source_store: ExecutableSourceStore,
    argv: ProgramArgvBuffer,
) -> Result<SyscallContext, exec::ExecLoadError> {
    exec_bin_from_shell_argv_with_stdin(programs, source_store, argv, &[])
}

/// Loads argv[0] through exec and starts a root-shell `/bin` exec request with stdin.
pub fn exec_bin_from_shell_argv_with_stdin(
    programs: &'static [ProgramDescriptor],
    source_store: ExecutableSourceStore,
    argv: ProgramArgvBuffer,
    stdin: &[u8],
) -> Result<SyscallContext, exec::ExecLoadError> {
    exec_bin_from_shell_argv_env_with_stdin(
        programs,
        source_store,
        argv,
        ProgramEnvBuffer::empty(),
        stdin,
    )
}

/// Loads argv[0] through exec and starts a root-shell `/bin` exec request with env and stdin.
pub(crate) fn exec_bin_from_shell_argv_env_with_stdin(
    programs: &'static [ProgramDescriptor],
    source_store: ExecutableSourceStore,
    argv: ProgramArgvBuffer,
    env: ProgramEnvBuffer,
    stdin: &[u8],
) -> Result<SyscallContext, exec::ExecLoadError> {
    let Some(argv0) = argv.argv0() else {
        record_syscall(None, SyscallOp::ExecLoad, SyscallStatus::Error);
        return Err(exec::ExecLoadError::EmptyArgv0);
    };
    let program = match exec::load_bin_program(programs, source_store, argv0) {
        Ok(program) => program,
        Err(error) => {
            record_syscall(None, SyscallOp::ExecLoad, SyscallStatus::Error);
            return Err(error);
        }
    };
    try_exec_bin_from_shell_with_env_and_stdin(program, argv, env, stdin)
}

/// Dispatches the next ready process and records the selected syscall context.
#[must_use]
pub fn dispatch_next_ready_program() -> Option<SyscallContext> {
    let dispatched = exec::dispatch_next_ready_process();
    let ctx = dispatched.map(SyscallContext::from_process)?;
    record_context(ctx, SyscallOp::SchedulerDispatch, SyscallStatus::Ok);
    append_scheduler_event(ctx, SyscallOp::SchedulerDispatch);
    record_context(ctx, SyscallOp::ProcessRun, process_run_status(dispatched, ctx.pid));
    Some(ctx)
}

/// Takes the pending invocation for a scheduler-selected process.
pub(crate) fn take_pending_exec(pid: usize) -> Option<exec::PendingProgramInvocation> {
    exec::take_pending_program(pid)
}

/// Runs a pending payload selected by rootd's scheduler loop.
#[must_use]
pub(crate) fn run_selected_pending_payload(
    daemon: &RootDaemon<'_>,
    session: &mut RootShellSession,
    ctx: SyscallContext,
) -> bool {
    let Some(pending) = exec::take_pending_payload(ctx.pid) else {
        return false;
    };
    let mut syscalls = ProgramSyscalls::new(daemon, session, Some(ctx));
    let _ = syscalls.run_selected_pending_payload_invocation(pending);
    true
}

/// Resumes a payload source image that blocked inside a retained interpreter operation.
#[must_use]
pub(crate) fn resume_payload_source_continuation(
    daemon: &RootDaemon<'_>,
    session: &mut RootShellSession,
    ctx: SyscallContext,
) -> bool {
    let Some(frame) = take_payload_source_continuation(ctx.pid) else {
        return false;
    };
    let payload = ProcessHandle {
        pid: ctx.pid,
        task_id: ctx.task_id,
        program_path: ctx.program_path,
        loader: ctx.loader,
        entry_name: ctx.entry_name,
    };
    let mut syscalls = ProgramSyscalls::new(daemon, session, Some(ctx));
    let outcome = syscalls.resume_payload_source_frame(frame);
    record_context(ctx, SyscallOp::PayloadRun, outcome.syscall_status());
    let PayloadSourceRunOutcome::Complete(result) = outcome else {
        return true;
    };
    if result == PayloadLaunchResult::Resident {
        append_payload_resident(payload);
        wake_wait_ready_continuation_for_child(payload.pid);
        return true;
    }
    let adoption = complete_payload_process(payload.pid, result);
    record_terminal_service_for_payload(ctx, result);
    record_context(ctx, SyscallOp::ProcessExit, payload_syscall_status(result));
    append_process_adoptions(ctx, adoption);
    append_payload_exit(payload, result);
    true
}

/// Replays a retained raw syscall continuation for a scheduler-selected process.
///
/// This is the first continuation-frame seed for blocking syscalls. It reuses
/// the temporary synchronous raw backend and therefore only replays scalar
/// arguments that were validated before the original block. Future trap entry
/// must replace the pointer assumptions with caller-memory validation.
#[must_use]
pub fn resume_syscall_continuation(
    daemon: &RootDaemon<'_>,
    session: &mut RootShellSession,
    ctx: SyscallContext,
) -> Option<SyscallRet> {
    let continuation = take_program_syscall_continuation(ctx.pid)?;
    let mut syscalls = ProgramSyscalls::new(daemon, session, Some(continuation.context));
    syscalls.replaying_continuation = Some(continuation.nr);
    syscalls.replaying_sleep_wake_tick = continuation.sleep_wake_tick;
    let ret = syscalls.dispatch_raw_syscall(continuation.nr, continuation.args);
    match ret.decode() {
        Ok(_) => {
            if let Some(blocked) = proc::block_user_resume_process(continuation.context.pid) {
                record_context(continuation.context, SyscallOp::SyscallContinue, SyscallStatus::Ok);
                let blocked_ctx = SyscallContext::from_process(blocked);
                record_context(blocked_ctx, SyscallOp::ProcessBlock, SyscallStatus::Blocked);
                append_process_control_event(blocked_ctx, SyscallOp::ProcessBlock);
            } else {
                record_context(
                    continuation.context,
                    SyscallOp::SyscallContinue,
                    SyscallStatus::Error,
                );
                record_context(continuation.context, SyscallOp::ProcessBlock, SyscallStatus::Error);
                return Some(SyscallRet::failure(SyscallError::IO));
            }
        }
        Err(SyscallError::BUSY) => {
            record_context(
                continuation.context,
                SyscallOp::SyscallContinue,
                SyscallStatus::Blocked,
            );
        }
        Err(_) => {
            record_context(continuation.context, SyscallOp::SyscallContinue, SyscallStatus::Error);
        }
    }
    Some(ret)
}

/// Releases a retained user-frame block for an explicit checked executable-body resume.
#[must_use]
pub(crate) fn resume_user_frame_process(ctx: SyscallContext) -> Option<SyscallContext> {
    let resumed = proc::run_user_resume_process(ctx.pid)?;
    let resumed_ctx = SyscallContext::from_process(resumed);
    record_context(resumed_ctx, SyscallOp::ProcessWake, SyscallStatus::Ok);
    append_process_control_event(resumed_ctx, SyscallOp::ProcessWake);
    record_context(resumed_ctx, SyscallOp::ProcessRun, SyscallStatus::Ok);
    Some(resumed_ctx)
}

/// Completes the current image program.
pub(crate) fn exit_current(ctx: SyscallContext, status: ProgramStatus) {
    if status == ProgramStatus::Blocked {
        return;
    }
    let adoption = complete_program_process(ctx.pid, status);
    record_terminal_service_for_program(ctx, status);
    record_context(ctx, SyscallOp::ProcessExit, process_exit_syscall_status(status));
    append_process_adoptions(ctx, adoption);
}

/// Typed syscall handle exposed to image-linked `/bin` program entries.
pub struct ProgramSyscalls<'daemon, 'session, 'rootd> {
    daemon: &'daemon RootDaemon<'rootd>,
    session: &'session mut RootShellSession,
    current: Option<SyscallContext>,
    process_bound: bool,
    stdio: ProgramStdio,
    raw_exit_status: Option<ProgramStatus>,
    raw_stdout_write_recorded: bool,
    fd_table: ProgramFdTable,
    vfs_render_capture: Option<NonNull<ProgramVfsFileBuffer>>,
    replaying_continuation: Option<SyscallNr>,
    replaying_sleep_wake_tick: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DirectCurrentAdmissionError {
    Unavailable,
    Busy(Option<SyscallContext>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SyscallRequest {
    KernelLogStats,
    DumpStatus,
    DumpSync,
    SchedulerSnapshot,
}

impl SyscallRequest {
    const fn op(self) -> SyscallOp {
        match self {
            Self::KernelLogStats => SyscallOp::KernelLogStats,
            Self::DumpStatus => SyscallOp::DumpStatus,
            Self::DumpSync => SyscallOp::DumpSync,
            Self::SchedulerSnapshot => SyscallOp::SchedulerSnapshot,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SyscallResult {
    KernelLogStats(klog::Stats),
    DumpStatus(dump::DumpStatus),
    DumpSync(dump::DumpSyncStatus),
    SchedulerSnapshot(SchedulerSnapshot),
}

impl<'daemon, 'session, 'rootd> ProgramSyscalls<'daemon, 'session, 'rootd> {
    /// Creates a syscall handle for one root-shell-launched program.
    #[must_use]
    pub fn new(
        daemon: &'daemon RootDaemon<'rootd>,
        session: &'session mut RootShellSession,
        current: Option<SyscallContext>,
    ) -> Self {
        Self::new_with_stdin_and_stdout_capture(daemon, session, current, &[], None)
    }

    /// Creates a syscall handle with a bounded initial stdin payload.
    #[must_use]
    pub fn new_with_stdin(
        daemon: &'daemon RootDaemon<'rootd>,
        session: &'session mut RootShellSession,
        current: Option<SyscallContext>,
        stdin: &[u8],
    ) -> Self {
        Self::new_with_stdin_and_stdout_capture(daemon, session, current, stdin, None)
    }

    /// Creates a syscall handle with stdin and optional stdout capture.
    #[must_use]
    pub fn new_with_stdin_and_stdout_capture(
        daemon: &'daemon RootDaemon<'rootd>,
        session: &'session mut RootShellSession,
        current: Option<SyscallContext>,
        stdin: &[u8],
        stdout_capture: Option<&ProgramStdoutCapture>,
    ) -> Self {
        let mut fd_table = ProgramFdTable::new();
        fd_table.seed_stdin(stdin);
        fd_table.set_stdout_capture(stdout_capture.map(NonNull::from));
        if let Some(current) = current {
            let capture = stdout_capture.map(NonNull::from);
            let _ = with_process_fd_table_mut(current.pid, |table| {
                if !stdin.is_empty() {
                    table.seed_stdin_if_unseeded(stdin);
                }
                table.set_stdout_capture(capture);
            });
        }
        Self {
            daemon,
            session,
            current,
            process_bound: current.is_some(),
            stdio: ProgramStdio::standard(),
            raw_exit_status: None,
            raw_stdout_write_recorded: false,
            fd_table,
            vfs_render_capture: None,
            replaying_continuation: None,
            replaying_sleep_wake_tick: 0,
        }
    }

    pub(crate) fn shares_linked_syscall_scope(&self, other: &ProgramSyscalls<'_, '_, '_>) -> bool {
        let self_daemon = self.daemon as *const RootDaemon<'_> as *const ();
        let other_daemon = other.daemon as *const RootDaemon<'_> as *const ();
        let self_session = self.session as *const RootShellSession as *const ();
        let other_session = other.session as *const RootShellSession as *const ();
        self_daemon == other_daemon && self_session == other_session
    }

    pub(crate) const fn current_context(&self) -> Option<SyscallContext> {
        self.current
    }

    const fn has_invalidated_process_context(&self) -> bool {
        self.process_bound && self.current.is_none()
    }

    fn current_has_retained_syscall_continuation(&self) -> bool {
        if self.replaying_continuation.is_some() {
            return false;
        }
        match self.current {
            Some(current) => has_program_syscall_continuation(current.pid),
            None => false,
        }
    }

    fn current_is_user_resume_blocked(&self) -> bool {
        if self.replaying_continuation.is_some() {
            return false;
        }
        let Some(current) = self.current else {
            return false;
        };
        matches!(
            proc::process(current.pid),
            Some(process)
                if process.state == proc::ProcessState::Blocked
                    && process.block_reason == sched::BlockReason::UserResume
        )
    }

    fn current_has_direct_backend_admission_block(&self) -> bool {
        self.current_has_retained_syscall_continuation() || self.current_is_user_resume_blocked()
    }

    fn require_direct_current_context(
        &self,
        op: SyscallOp,
    ) -> Result<SyscallContext, DirectCurrentAdmissionError> {
        if self.has_invalidated_process_context() {
            self.record(op, SyscallStatus::Error);
            return Err(DirectCurrentAdmissionError::Busy(None));
        }
        let Some(current) = self.current else {
            self.record(op, SyscallStatus::Unavailable);
            return Err(DirectCurrentAdmissionError::Unavailable);
        };
        if self.current_has_direct_backend_admission_block() {
            record_context(current, SyscallOp::SyscallContinue, SyscallStatus::Error);
            record_context(current, op, SyscallStatus::Error);
            return Err(DirectCurrentAdmissionError::Busy(Some(current)));
        }
        Ok(current)
    }

    fn ensure_no_retained_syscall_continuation_for_io(
        &self,
        op: SyscallOp,
    ) -> Result<(), ProgramIoError> {
        if self.current_has_direct_backend_admission_block() {
            self.record(SyscallOp::SyscallContinue, SyscallStatus::Error);
            self.record(op, SyscallStatus::Error);
            Err(ProgramIoError::Busy)
        } else {
            Ok(())
        }
    }

    fn ensure_direct_backend_vfs_admitted(&self, op: SyscallOp) -> Result<(), VfsError> {
        if self.has_invalidated_process_context() {
            self.record(op, SyscallStatus::Error);
            Err(VfsError::Busy)
        } else if self.current_has_direct_backend_admission_block() {
            self.record(SyscallOp::SyscallContinue, SyscallStatus::Error);
            self.record(op, SyscallStatus::Error);
            Err(VfsError::Busy)
        } else {
            Ok(())
        }
    }

    fn ensure_no_retained_syscall_continuation_for_exec(
        &self,
        op: SyscallOp,
    ) -> Result<(), ProgramExecError> {
        if self.current_has_direct_backend_admission_block() {
            self.record(SyscallOp::SyscallContinue, SyscallStatus::Error);
            self.record(op, SyscallStatus::Error);
            Err(ProgramExecError::Busy)
        } else {
            Ok(())
        }
    }

    fn ensure_no_retained_syscall_continuation_for_process_op(
        &self,
        op: SyscallOp,
    ) -> Result<(), ProgramProcessError> {
        if self.current_has_direct_backend_admission_block() {
            self.record(SyscallOp::SyscallContinue, SyscallStatus::Error);
            self.record(op, SyscallStatus::Error);
            Err(ProgramProcessError::Busy)
        } else {
            Ok(())
        }
    }

    fn ensure_no_retained_syscall_continuation_for_source_install(
        &self,
    ) -> Result<(), ProgramSourceInstallError> {
        if self.current_has_direct_backend_admission_block() {
            self.record(SyscallOp::SyscallContinue, SyscallStatus::Error);
            self.record(SyscallOp::SourceInstall, SyscallStatus::Error);
            Err(ProgramSourceInstallError::Busy)
        } else {
            Ok(())
        }
    }

    fn ensure_direct_backend_payload_launch_admitted(&self) -> Result<(), PayloadLaunchResult> {
        if self.has_invalidated_process_context() {
            self.record(SyscallOp::PayloadLaunch, SyscallStatus::Error);
            Err(PayloadLaunchResult::Failed)
        } else if self.current_has_direct_backend_admission_block() {
            self.record(SyscallOp::SyscallContinue, SyscallStatus::Error);
            self.record(SyscallOp::PayloadLaunch, SyscallStatus::Error);
            Err(PayloadLaunchResult::Failed)
        } else {
            Ok(())
        }
    }

    fn direct_backend_report_admitted(&self, op: SyscallOp) -> bool {
        if self.has_invalidated_process_context() {
            self.record(op, SyscallStatus::Error);
            false
        } else if self.current_has_direct_backend_admission_block() {
            self.record(SyscallOp::SyscallContinue, SyscallStatus::Error);
            self.record(op, SyscallStatus::Error);
            false
        } else {
            true
        }
    }

    fn direct_backend_session_admitted(&self, op: SyscallOp) -> bool {
        if self.has_invalidated_process_context() {
            self.record(op, SyscallStatus::Error);
            false
        } else if self.current_has_direct_backend_admission_block() {
            self.record(SyscallOp::SyscallContinue, SyscallStatus::Error);
            self.record(op, SyscallStatus::Error);
            false
        } else {
            true
        }
    }

    fn with_active_fd_table_mut<R>(
        &mut self,
        f: impl FnOnce(&mut ProgramFdTable) -> R,
    ) -> Result<R, ProgramIoError> {
        if let Some(current) = self.current {
            with_process_fd_table_mut(current.pid, f).ok_or(ProgramIoError::Busy)
        } else if self.has_invalidated_process_context() {
            Err(ProgramIoError::Busy)
        } else {
            Ok(f(&mut self.fd_table))
        }
    }

    fn with_existing_active_fd_table_mut<R>(
        &mut self,
        f: impl FnOnce(&mut ProgramFdTable) -> R,
    ) -> Option<R> {
        if let Some(current) = self.current {
            with_existing_process_fd_table_mut(current.pid, f)
        } else if self.has_invalidated_process_context() {
            None
        } else {
            Some(f(&mut self.fd_table))
        }
    }

    fn active_fd_descriptor(&self, fd: usize) -> Option<ProgramFdDescriptor> {
        if let Some(current) = self.current {
            process_fd_descriptor(current.pid, fd)
        } else if self.has_invalidated_process_context() {
            None
        } else {
            self.fd_table.descriptor_for_fd(fd)
        }
    }

    fn active_stdout_capture(&self) -> Option<NonNull<ProgramStdoutCapture>> {
        if let Some(current) = self.current {
            process_stdout_capture(current.pid)
        } else if self.has_invalidated_process_context() {
            None
        } else {
            self.fd_table.stdout_capture()
        }
    }

    fn session_cwd_path(&self) -> PathBuf {
        match vfs::normalize("/", self.session.cwd()) {
            Ok(path) => path,
            Err(_) => PathBuf::root(),
        }
    }

    fn open_at_base_path(&mut self, raw_dir: usize) -> Result<PathBuf, ProgramIoError> {
        if raw_dir == raw_open_at_dir_arg(OpenAtDir::session_cwd()) {
            return Ok(self.session_cwd_path());
        }
        match self.with_existing_active_fd_table_mut(|table| table.directory_path_for_fd(raw_dir)) {
            Some(result) => result,
            None => Err(ProgramIoError::BadFd),
        }
    }

    /// Returns the standard stream handles attached to this program.
    #[must_use]
    pub fn stdio(&self) -> ProgramStdio {
        if self.has_invalidated_process_context() {
            self.record(SyscallOp::StdioAttach, SyscallStatus::Error);
            return self.stdio;
        }
        if self.current_has_direct_backend_admission_block() {
            self.record(SyscallOp::SyscallContinue, SyscallStatus::Error);
            self.record(SyscallOp::StdioAttach, SyscallStatus::Error);
            return self.stdio;
        }
        self.record(SyscallOp::StdioAttach, SyscallStatus::Ok);
        self.stdio
    }

    /// Returns the standard input handle attached to this program.
    #[must_use]
    pub fn stdin(&self) -> ProgramStreamHandle {
        if self.has_invalidated_process_context() {
            self.record(SyscallOp::StdioAttach, SyscallStatus::Error);
            return self.stdio.stdin;
        }
        if self.current_has_direct_backend_admission_block() {
            self.record(SyscallOp::SyscallContinue, SyscallStatus::Error);
            self.record(SyscallOp::StdioAttach, SyscallStatus::Error);
        }
        self.stdio.stdin
    }

    /// Returns the standard output handle attached to this program.
    #[must_use]
    pub fn stdout(&self) -> ProgramStreamHandle {
        if self.has_invalidated_process_context() {
            self.record(SyscallOp::StdioAttach, SyscallStatus::Error);
            return self.stdio.stdout;
        }
        if self.current_has_direct_backend_admission_block() {
            self.record(SyscallOp::SyscallContinue, SyscallStatus::Error);
            self.record(SyscallOp::StdioAttach, SyscallStatus::Error);
        }
        self.stdio.stdout
    }

    /// Returns the standard error handle attached to this program.
    #[must_use]
    pub fn stderr(&self) -> ProgramStreamHandle {
        if self.has_invalidated_process_context() {
            self.record(SyscallOp::StdioAttach, SyscallStatus::Error);
            return self.stdio.stderr;
        }
        if self.current_has_direct_backend_admission_block() {
            self.record(SyscallOp::SyscallContinue, SyscallStatus::Error);
            self.record(SyscallOp::StdioAttach, SyscallStatus::Error);
        }
        self.stdio.stderr
    }

    /// Returns the current process record, if this handle is attached.
    #[must_use]
    pub fn process_self(&self) -> Option<ProcessRecord> {
        let Ok(current) = self.require_direct_current_context(SyscallOp::ProcessSelf) else {
            return None;
        };
        let record = proc::process(current.pid);
        self.record_result(SyscallOp::ProcessSelf, record.is_some());
        record
    }

    /// Dispatches one raw Reovim syscall transport request.
    ///
    /// This is the temporary kernel-side backend for the raw syscall spine
    /// before architecture trap entry exists.
    /// It accepts only transport types; filesystem/process semantics still live
    /// in domain uapi wrappers and the existing kernel object methods.
    #[must_use]
    pub fn dispatch_raw_syscall(&mut self, nr: SyscallNr, args: SyscallArgs) -> SyscallRet {
        if nr.is_invalid() {
            return SyscallRet::failure(SyscallError::INVALID_ARGUMENT);
        }
        if self.has_invalidated_process_context() {
            self.record(syscall_op_for_raw_dispatch(nr, args), SyscallStatus::Error);
            return SyscallRet::failure(SyscallError::BUSY);
        }
        if self.current_has_direct_backend_admission_block() {
            self.record(SyscallOp::SyscallContinue, SyscallStatus::Error);
            self.record(syscall_op_for_raw_dispatch(nr, args), SyscallStatus::Error);
            return SyscallRet::failure(SyscallError::BUSY);
        }
        match nr {
            SyscallNr::READ => self.dispatch_raw_read(args),
            SyscallNr::WRITE => self.dispatch_raw_write(args),
            SyscallNr::OPEN_AT => self.dispatch_raw_open_at(args),
            SyscallNr::CLOSE => self.dispatch_raw_close(args),
            SyscallNr::LSEEK => self.dispatch_raw_lseek(args),
            SyscallNr::GETDENTS => self.dispatch_raw_getdents(args),
            SyscallNr::DUP => self.dispatch_raw_dup(args),
            SyscallNr::DUP_TO => self.dispatch_raw_dup_to(args),
            SyscallNr::FD_FLAGS_GET => self.dispatch_raw_fd_flags_get(args),
            SyscallNr::FD_FLAGS_SET => self.dispatch_raw_fd_flags_set(args),
            SyscallNr::FD_STATUS_GET => self.dispatch_raw_fd_status_get(args),
            SyscallNr::FD_STATUS_SET => self.dispatch_raw_fd_status_set(args),
            SyscallNr::PIPE => self.dispatch_raw_pipe(args),
            SyscallNr::TERMINAL_CLEAR => self.dispatch_raw_terminal_clear(args),
            SyscallNr::TERMINAL_RAW_ENTER => self.dispatch_raw_terminal_raw_enter(args),
            SyscallNr::TERMINAL_RAW_RESTORE => self.dispatch_raw_terminal_raw_restore(args),
            SyscallNr::TERMINAL_RAW_RESTORE_PRIMARY => {
                self.dispatch_raw_terminal_raw_restore_primary(args)
            }
            SyscallNr::SYSTEM_HALT => self.dispatch_raw_system_halt(args),
            SyscallNr::PROVIDER_PROBE => self.dispatch_raw_provider_probe(args),
            SyscallNr::DUMP_SYNC => self.dispatch_raw_dump_sync(args),
            SyscallNr::SERVICE_CONTROL => self.dispatch_raw_service_control(args),
            SyscallNr::SESSION_CONTROL => self.dispatch_raw_session_control(args),
            SyscallNr::SOURCE_CONTROL => self.dispatch_raw_source_control(args),
            SyscallNr::GET_PID => self.dispatch_raw_get_pid(),
            SyscallNr::PROCESS_SELF => self.dispatch_raw_process_self(args),
            SyscallNr::GET_CWD => self.dispatch_raw_get_cwd(args),
            SyscallNr::CHDIR => self.dispatch_raw_chdir(args),
            SyscallNr::YIELD_NOW => self.dispatch_raw_yield_now(args),
            SyscallNr::SLEEP => self.dispatch_raw_sleep(args),
            SyscallNr::SCHED_TICK => self.dispatch_raw_scheduler_tick(args),
            SyscallNr::SPAWN => self.dispatch_raw_spawn(args),
            SyscallNr::EXECVE => self.dispatch_raw_execve(args),
            SyscallNr::WAIT => self.dispatch_raw_wait(args),
            SyscallNr::PROCESS_WAKE => self.dispatch_raw_process_wake(args),
            SyscallNr::PROCESS_KILL => self.dispatch_raw_process_kill(args),
            SyscallNr::PROCESS_WAIT_TICKS => self.dispatch_raw_process_wait_ticks(args),
            SyscallNr::PROCESS_WAIT_READY => self.dispatch_raw_process_wait_ready(args),
            SyscallNr::EXIT => self.dispatch_raw_exit(args),
            _ => SyscallRet::failure(SyscallError::UNSUPPORTED),
        }
    }

    fn dispatch_raw_read(&mut self, args: SyscallArgs) -> SyscallRet {
        if self.current_has_direct_backend_admission_block() {
            self.record(SyscallOp::SyscallContinue, SyscallStatus::Error);
            return SyscallRet::failure(SyscallError::BUSY);
        }
        // SAFETY: this is the temporary synchronous raw backend. The caller is
        // responsible for passing a valid writable buffer for `read`.
        let out = match unsafe { raw_syscall_read_buffer(args.a1, args.a2) } {
            Ok(out) => out,
            Err(error) => {
                self.record(SyscallOp::FdRead, SyscallStatus::Error);
                return SyscallRet::failure(error);
            }
        };
        match self.read_fd(args.a0, out) {
            Ok(read) => {
                if let Some(current) = self.current {
                    clear_matching_program_syscall_continuation(current.pid, SyscallNr::READ, args);
                }
                SyscallRet::success(read)
            }
            Err(error) => {
                if error == ProgramIoError::Busy {
                    if let Some(current) = self.current {
                        let stored =
                            store_program_syscall_continuation(current, SyscallNr::READ, args);
                        let status = if stored.is_ok() {
                            SyscallStatus::Blocked
                        } else {
                            release_pipe_read_block(current.pid);
                            SyscallStatus::Error
                        };
                        self.record(SyscallOp::SyscallContinue, status);
                    }
                }
                SyscallRet::failure(syscall_error_from_program_io(error))
            }
        }
    }

    fn dispatch_raw_write(&mut self, args: SyscallArgs) -> SyscallRet {
        // SAFETY: this is the temporary synchronous raw backend. The caller is
        // responsible for passing a valid readable buffer for `write`.
        let bytes = match unsafe { raw_syscall_write_buffer(args.a1, args.a2) } {
            Ok(bytes) => bytes,
            Err(error) => {
                self.record(SyscallOp::FdWrite, SyscallStatus::Error);
                return SyscallRet::failure(error);
            }
        };
        match self.write_fd(args.a0, bytes) {
            Ok(written) => {
                if let Some(current) = self.current {
                    clear_matching_program_syscall_continuation(
                        current.pid,
                        SyscallNr::WRITE,
                        args,
                    );
                }
                if self.active_fd_descriptor(args.a0)
                    == Some(ProgramFdDescriptor::Stdio(ProgramStream::Stdout))
                {
                    self.record_raw_stdout_write_once();
                }
                SyscallRet::success(written)
            }
            Err(error) => {
                if error == ProgramIoError::Busy {
                    if let Some(current) = self.current {
                        let stored =
                            store_program_syscall_continuation(current, SyscallNr::WRITE, args);
                        let status = if stored.is_ok() {
                            SyscallStatus::Blocked
                        } else {
                            release_pipe_write_block(current.pid);
                            SyscallStatus::Error
                        };
                        self.record(SyscallOp::SyscallContinue, status);
                    }
                }
                SyscallRet::failure(syscall_error_from_program_io(error))
            }
        }
    }

    fn dispatch_raw_open_at(&mut self, args: SyscallArgs) -> SyscallRet {
        if self.current.is_none() {
            self.record(SyscallOp::VfsOpen, SyscallStatus::Unavailable);
            return SyscallRet::failure(SyscallError::NO_CURRENT_PROCESS);
        }
        if args.a4 != 0 || args.a5 != 0 {
            self.record(SyscallOp::VfsOpen, SyscallStatus::Error);
            return SyscallRet::failure(SyscallError::INVALID_ARGUMENT);
        }
        let flags = OpenFlags::new(args.a3 as u32);
        if !flags.is_supported_open() {
            self.record(SyscallOp::VfsOpen, SyscallStatus::Error);
            return SyscallRet::failure(SyscallError::INVALID_ARGUMENT);
        }
        // SAFETY: this is the temporary synchronous raw backend. The caller is
        // responsible for passing a valid readable path buffer.
        let path = match unsafe { raw_syscall_write_buffer(args.a1, args.a2) } {
            Ok(path) => path,
            Err(error) => {
                self.record(SyscallOp::VfsOpen, SyscallStatus::Error);
                return SyscallRet::failure(error);
            }
        };
        let path = match core::str::from_utf8(path) {
            Ok(path) => path,
            Err(_) => {
                self.record(SyscallOp::VfsOpen, SyscallStatus::Error);
                return SyscallRet::failure(SyscallError::INVALID_ARGUMENT);
            }
        };
        let base = match self.open_at_base_path(args.a0) {
            Ok(base) => base,
            Err(error) => {
                self.record(SyscallOp::VfsOpen, SyscallStatus::Error);
                return SyscallRet::failure(syscall_error_from_program_io(error));
            }
        };
        let descriptor_flags = if flags.has_close_on_exec() {
            ProgramFdDescriptorFlags::close_on_exec()
        } else {
            ProgramFdDescriptorFlags::empty()
        };
        let open = if flags.is_read_directory() {
            if flags.is_write_only() {
                self.record(SyscallOp::VfsOpen, SyscallStatus::Error);
                return SyscallRet::failure(SyscallError::INVALID_ARGUMENT);
            }
            self.open_vfs_directory_path_at_with_descriptor_flags(base, path, descriptor_flags)
        } else if flags.is_write_only() {
            self.open_vfs_file_path_at_with_access_and_descriptor_flags(
                base,
                path,
                ProgramOpenAccess::WriteOnly,
                descriptor_flags,
            )
        } else {
            self.open_vfs_file_path_at_with_access_and_descriptor_flags(
                base,
                path,
                ProgramOpenAccess::ReadOnly,
                descriptor_flags,
            )
        };
        match open {
            Ok(fd) => SyscallRet::success(fd),
            Err(error) => SyscallRet::failure(syscall_error_from_vfs(error)),
        }
    }

    fn dispatch_raw_close(&mut self, args: SyscallArgs) -> SyscallRet {
        match self.close_fd(args.a0) {
            Ok(()) => SyscallRet::success(0),
            Err(error) => SyscallRet::failure(syscall_error_from_program_io(error)),
        }
    }

    fn dispatch_raw_lseek(&mut self, args: SyscallArgs) -> SyscallRet {
        let Some(whence) = raw_seek_whence_arg(args.a2) else {
            self.record(SyscallOp::FdSeek, SyscallStatus::Error);
            return SyscallRet::failure(SyscallError::INVALID_ARGUMENT);
        };
        match self.seek_fd(args.a0, raw_seek_offset_arg(args.a1), whence) {
            Ok(offset) => SyscallRet::success(offset),
            Err(error) => SyscallRet::failure(syscall_error_from_program_io(error)),
        }
    }

    fn dispatch_raw_getdents(&mut self, args: SyscallArgs) -> SyscallRet {
        // SAFETY: this is the temporary synchronous raw backend. The caller is
        // responsible for passing a valid writable buffer for `getdents`.
        let out = match unsafe { raw_syscall_read_buffer(args.a1, args.a2) } {
            Ok(out) => out,
            Err(error) => {
                self.record(SyscallOp::VfsList, SyscallStatus::Error);
                return SyscallRet::failure(error);
            }
        };
        match self.read_directory_fd(args.a0, out) {
            Ok(read) => SyscallRet::success(read),
            Err(error) => SyscallRet::failure(syscall_error_from_program_io(error)),
        }
    }

    fn dispatch_raw_dup(&mut self, args: SyscallArgs) -> SyscallRet {
        match self.duplicate_fd(args.a0) {
            Ok(fd) => SyscallRet::success(fd),
            Err(error) => SyscallRet::failure(syscall_error_from_program_io(error)),
        }
    }

    fn dispatch_raw_dup_to(&mut self, args: SyscallArgs) -> SyscallRet {
        if args.a2 != 0 || args.a3 != 0 || args.a4 != 0 || args.a5 != 0 {
            self.record(SyscallOp::FdDuplicate, SyscallStatus::Error);
            return SyscallRet::failure(SyscallError::INVALID_ARGUMENT);
        }
        match self.duplicate_fd_to(args.a0, args.a1) {
            Ok(fd) => SyscallRet::success(fd),
            Err(error) => SyscallRet::failure(syscall_error_from_program_io(error)),
        }
    }

    fn dispatch_raw_fd_flags_get(&mut self, args: SyscallArgs) -> SyscallRet {
        if args.a1 != 0 || args.a2 != 0 || args.a3 != 0 || args.a4 != 0 || args.a5 != 0 {
            self.record(SyscallOp::FdFlagsGet, SyscallStatus::Error);
            return SyscallRet::failure(SyscallError::INVALID_ARGUMENT);
        }
        match self.descriptor_flags(args.a0) {
            Ok(flags) => SyscallRet::success(flags.raw() as usize),
            Err(error) => SyscallRet::failure(syscall_error_from_program_io(error)),
        }
    }

    fn dispatch_raw_fd_flags_set(&mut self, args: SyscallArgs) -> SyscallRet {
        if args.a2 != 0 || args.a3 != 0 || args.a4 != 0 || args.a5 != 0 {
            self.record(SyscallOp::FdFlagsSet, SyscallStatus::Error);
            return SyscallRet::failure(SyscallError::INVALID_ARGUMENT);
        }
        if args.a1 > u32::MAX as usize {
            self.record(SyscallOp::FdFlagsSet, SyscallStatus::Error);
            return SyscallRet::failure(SyscallError::INVALID_ARGUMENT);
        }
        match self.set_descriptor_flags(args.a0, DescriptorFlags::new(args.a1 as u32)) {
            Ok(()) => SyscallRet::success(0),
            Err(error) => SyscallRet::failure(syscall_error_from_program_io(error)),
        }
    }

    fn dispatch_raw_fd_status_get(&mut self, args: SyscallArgs) -> SyscallRet {
        if args.a1 != 0 || args.a2 != 0 || args.a3 != 0 || args.a4 != 0 || args.a5 != 0 {
            self.record(SyscallOp::FdStatusGet, SyscallStatus::Error);
            return SyscallRet::failure(SyscallError::INVALID_ARGUMENT);
        }
        match self.status_flags(args.a0) {
            Ok(flags) => SyscallRet::success(flags.raw() as usize),
            Err(error) => SyscallRet::failure(syscall_error_from_program_io(error)),
        }
    }

    fn dispatch_raw_fd_status_set(&mut self, args: SyscallArgs) -> SyscallRet {
        if args.a2 != 0 || args.a3 != 0 || args.a4 != 0 || args.a5 != 0 {
            self.record(SyscallOp::FdStatusSet, SyscallStatus::Error);
            return SyscallRet::failure(SyscallError::INVALID_ARGUMENT);
        }
        if args.a1 > u32::MAX as usize {
            self.record(SyscallOp::FdStatusSet, SyscallStatus::Error);
            return SyscallRet::failure(SyscallError::INVALID_ARGUMENT);
        }
        match self.set_status_flags(args.a0, FileStatusFlags::new(args.a1 as u32)) {
            Ok(()) => SyscallRet::success(0),
            Err(error) => SyscallRet::failure(syscall_error_from_program_io(error)),
        }
    }

    fn dispatch_raw_pipe(&mut self, args: SyscallArgs) -> SyscallRet {
        if self.current.is_none() {
            self.record(SyscallOp::FdPipe, SyscallStatus::Unavailable);
            return SyscallRet::failure(SyscallError::NO_CURRENT_PROCESS);
        }
        if args.a2 != 0 || args.a3 != 0 || args.a4 != 0 || args.a5 != 0 {
            self.record(SyscallOp::FdPipe, SyscallStatus::Error);
            return SyscallRet::failure(SyscallError::INVALID_ARGUMENT);
        }
        // SAFETY: this is the temporary synchronous raw backend. The caller is
        // responsible for passing a valid writable two-fd array.
        let fds = match unsafe { raw_syscall_pipe_fds(args.a0, args.a1) } {
            Ok(fds) => fds,
            Err(error) => {
                self.record(SyscallOp::FdPipe, SyscallStatus::Error);
                return SyscallRet::failure(error);
            }
        };
        match self.pipe_fds() {
            Ok((read_fd, write_fd)) => {
                if read_fd > i32::MAX as usize || write_fd > i32::MAX as usize {
                    self.record(SyscallOp::FdPipe, SyscallStatus::Error);
                    return SyscallRet::failure(SyscallError::INVALID_ARGUMENT);
                }
                fds[0] = read_fd as i32;
                fds[1] = write_fd as i32;
                SyscallRet::success(0)
            }
            Err(error) => SyscallRet::failure(syscall_error_from_program_io(error)),
        }
    }

    fn dispatch_raw_terminal_clear(&self, args: SyscallArgs) -> SyscallRet {
        if self.current.is_none() {
            self.record(SyscallOp::TtyClear, SyscallStatus::Unavailable);
            return SyscallRet::failure(SyscallError::NO_CURRENT_PROCESS);
        }
        if args.a0 != PRIMARY_OUTPUT.raw() as usize
            || args.a1 != 0
            || args.a2 != 0
            || args.a3 != 0
            || args.a4 != 0
            || args.a5 != 0
        {
            self.record(SyscallOp::TtyClear, SyscallStatus::Error);
            return SyscallRet::failure(SyscallError::INVALID_ARGUMENT);
        }
        self.clear_console();
        SyscallRet::success(0)
    }

    fn dispatch_raw_terminal_raw_enter(&self, args: SyscallArgs) -> SyscallRet {
        if self.current.is_none() {
            self.record(SyscallOp::TtyRawEnter, SyscallStatus::Unavailable);
            return SyscallRet::failure(SyscallError::NO_CURRENT_PROCESS);
        }
        if args.a0 > u32::MAX as usize
            || args.a1 != 0
            || args.a2 != 0
            || args.a3 != 0
            || args.a4 != 0
            || args.a5 != 0
        {
            self.record(SyscallOp::TtyRawEnter, SyscallStatus::Error);
            return SyscallRet::failure(SyscallError::INVALID_ARGUMENT);
        }
        let request = RawModeRequest::new(TerminalId::new(args.a0 as u32));
        match crate::terminal::enter_raw_mode_token(request) {
            Ok(token) => {
                self.record(SyscallOp::TtyRawEnter, SyscallStatus::Ok);
                SyscallRet::success(token.raw() as usize)
            }
            Err(error) => {
                self.record(SyscallOp::TtyRawEnter, terminal_syscall_status(error));
                SyscallRet::failure(syscall_error_from_terminal(error))
            }
        }
    }

    fn dispatch_raw_terminal_raw_restore(&self, args: SyscallArgs) -> SyscallRet {
        if self.current.is_none() {
            self.record(SyscallOp::TtyRawRestore, SyscallStatus::Unavailable);
            return SyscallRet::failure(SyscallError::NO_CURRENT_PROCESS);
        }
        if args.a1 != 0 || args.a2 != 0 || args.a3 != 0 || args.a4 != 0 || args.a5 != 0 {
            self.record(SyscallOp::TtyRawRestore, SyscallStatus::Error);
            return SyscallRet::failure(SyscallError::INVALID_ARGUMENT);
        }
        match crate::terminal::restore_raw_mode(RawModeToken::new(args.a0 as u64)) {
            Ok(()) => {
                self.record(SyscallOp::TtyRawRestore, SyscallStatus::Ok);
                SyscallRet::success(0)
            }
            Err(error) => {
                self.record(SyscallOp::TtyRawRestore, terminal_syscall_status(error));
                SyscallRet::failure(syscall_error_from_terminal(error))
            }
        }
    }

    fn dispatch_raw_terminal_raw_restore_primary(&self, args: SyscallArgs) -> SyscallRet {
        if self.current.is_none() {
            self.record(SyscallOp::TtyRawRestorePrimary, SyscallStatus::Unavailable);
            return SyscallRet::failure(SyscallError::NO_CURRENT_PROCESS);
        }
        if args != SyscallArgs::EMPTY {
            self.record(SyscallOp::TtyRawRestorePrimary, SyscallStatus::Error);
            return SyscallRet::failure(SyscallError::INVALID_ARGUMENT);
        }
        crate::terminal::restore_primary_input_raw_mode();
        self.record(SyscallOp::TtyRawRestorePrimary, SyscallStatus::Ok);
        SyscallRet::success(0)
    }

    fn dispatch_raw_system_halt(&mut self, args: SyscallArgs) -> SyscallRet {
        if self.current.is_none() {
            self.record(SyscallOp::SystemHalt, SyscallStatus::Unavailable);
            return SyscallRet::failure(SyscallError::NO_CURRENT_PROCESS);
        }
        if args != SyscallArgs::EMPTY || self.raw_exit_status.is_some() {
            self.record(SyscallOp::SystemHalt, SyscallStatus::Error);
            return SyscallRet::failure(SyscallError::INVALID_ARGUMENT);
        }

        self.raw_exit_status = Some(ProgramStatus::Halt);
        self.record(SyscallOp::SystemHalt, SyscallStatus::Ok);
        SyscallRet::success(0)
    }

    fn dispatch_raw_provider_probe(&self, args: SyscallArgs) -> SyscallRet {
        if self.current.is_none() {
            self.record(SyscallOp::ProviderProbe, SyscallStatus::Unavailable);
            return SyscallRet::failure(SyscallError::NO_CURRENT_PROCESS);
        }
        if args.a2 != 0 || args.a3 != 0 || args.a4 != 0 || args.a5 != 0 || args.a1 == 0 {
            self.record(SyscallOp::ProviderProbe, SyscallStatus::Error);
            return SyscallRet::failure(SyscallError::INVALID_ARGUMENT);
        }
        // SAFETY: this is the temporary synchronous raw backend. The caller is
        // responsible for passing a valid readable UTF-8 target buffer.
        let target = match unsafe { raw_syscall_write_buffer(args.a0, args.a1) } {
            Ok(target) => target,
            Err(error) => {
                self.record(SyscallOp::ProviderProbe, SyscallStatus::Error);
                return SyscallRet::failure(error);
            }
        };
        let target = match core::str::from_utf8(target) {
            Ok(target) => target,
            Err(_) => {
                self.record(SyscallOp::ProviderProbe, SyscallStatus::Error);
                return SyscallRet::failure(SyscallError::INVALID_ARGUMENT);
            }
        };

        match self.run_hardware_probe(target) {
            Some(HardwareProbeResult::Handled) => SyscallRet::success(0),
            Some(HardwareProbeResult::UnknownTarget) => {
                SyscallRet::failure(SyscallError::NOT_FOUND)
            }
            None => SyscallRet::failure(SyscallError::UNSUPPORTED),
        }
    }

    fn dispatch_raw_dump_sync(&self, args: SyscallArgs) -> SyscallRet {
        if self.current.is_none() {
            self.record(SyscallOp::DumpSync, SyscallStatus::Unavailable);
            return SyscallRet::failure(SyscallError::NO_CURRENT_PROCESS);
        }
        if args.a2 != 0 || args.a3 != 0 || args.a4 != 0 || args.a5 != 0 {
            self.record(SyscallOp::DumpSync, SyscallStatus::Error);
            return SyscallRet::failure(SyscallError::INVALID_ARGUMENT);
        }
        // SAFETY: this is the temporary synchronous raw backend. The caller is
        // responsible for passing a live writable `DumpSyncReport`.
        let report = match unsafe { raw_syscall_dump_sync_report(args.a0, args.a1) } {
            Ok(report) => report,
            Err(error) => {
                self.record(SyscallOp::DumpSync, SyscallStatus::Error);
                return SyscallRet::failure(error);
            }
        };
        let status = dump::sync_with_context(
            self.dump_image_identity(),
            self.daemon.boot_info(),
            self.daemon.devices(),
        );
        self.record(SyscallOp::DumpSync, dump_sync_syscall_status(status));
        write_dump_sync_report(report, status);
        SyscallRet::success(0)
    }

    fn dispatch_raw_service_control(&mut self, args: SyscallArgs) -> SyscallRet {
        let op = match args.a0 {
            RAW_SERVICE_CONTROL_OP_STOP => SyscallOp::ServiceStop,
            RAW_SERVICE_CONTROL_OP_START => SyscallOp::ServiceStart,
            RAW_SERVICE_CONTROL_OP_RESTART => SyscallOp::ServiceRestart,
            RAW_SERVICE_CONTROL_OP_REQUEST => SyscallOp::InitServiceStart,
            _ => {
                self.record(SyscallOp::ServiceStop, SyscallStatus::Error);
                return SyscallRet::failure(SyscallError::INVALID_ARGUMENT);
            }
        };
        if self.current.is_none() {
            self.record(op, SyscallStatus::Unavailable);
            return SyscallRet::failure(SyscallError::NO_CURRENT_PROCESS);
        }
        if args.a0 == RAW_SERVICE_CONTROL_OP_REQUEST {
            return self.dispatch_raw_service_request(args);
        }
        if args.a2 == 0 || args.a5 != 0 {
            self.record(op, SyscallStatus::Error);
            return SyscallRet::failure(SyscallError::INVALID_ARGUMENT);
        }
        // SAFETY: this is the temporary synchronous raw backend. The caller is
        // responsible for passing a valid readable UTF-8 service-name buffer.
        let name = match unsafe { raw_syscall_write_buffer(args.a1, args.a2) } {
            Ok(name) => name,
            Err(error) => {
                self.record(op, SyscallStatus::Error);
                return SyscallRet::failure(error);
            }
        };
        let name = match core::str::from_utf8(name) {
            Ok(name) => name,
            Err(_) => {
                self.record(op, SyscallStatus::Error);
                return SyscallRet::failure(SyscallError::INVALID_ARGUMENT);
            }
        };
        // SAFETY: this is the temporary synchronous raw backend. The caller is
        // responsible for passing a live writable `ServiceControlReport`.
        let report = match unsafe { raw_syscall_service_control_report(args.a3, args.a4) } {
            Ok(report) => report,
            Err(error) => {
                self.record(op, SyscallStatus::Error);
                return SyscallRet::failure(error);
            }
        };

        match args.a0 {
            RAW_SERVICE_CONTROL_OP_STOP => match self.stop_service_by_name(name) {
                Ok(record) => {
                    write_service_control_report(report, record);
                    if let Some(current) = self.current {
                        append_service_lifecycle_event(current, SyscallOp::ServiceStop, record);
                    }
                    SyscallRet::success(record.service_pid)
                }
                Err(error) => SyscallRet::failure(syscall_error_from_program_service(error)),
            },
            RAW_SERVICE_CONTROL_OP_START => match self.start_service_by_name(name) {
                Ok(result) => {
                    write_service_control_result_report(report, result);
                    SyscallRet::success(result.record.service_pid)
                }
                Err(error) => SyscallRet::failure(syscall_error_from_program_service(error)),
            },
            RAW_SERVICE_CONTROL_OP_RESTART => match self.restart_service_by_name(name) {
                Ok(result) => {
                    write_service_control_result_report(report, result);
                    SyscallRet::success(result.record.service_pid)
                }
                Err(error) => SyscallRet::failure(syscall_error_from_program_service(error)),
            },
            _ => SyscallRet::failure(SyscallError::INVALID_ARGUMENT),
        }
    }

    fn dispatch_raw_service_request(&mut self, args: SyscallArgs) -> SyscallRet {
        if args.a2 == 0 || args.a4 == 0 || args.a5 != 0 {
            self.record(SyscallOp::InitServiceStart, SyscallStatus::Error);
            return SyscallRet::failure(SyscallError::INVALID_ARGUMENT);
        }
        // SAFETY: this is the temporary synchronous raw backend. The caller is
        // responsible for passing valid readable UTF-8 service strings.
        let name = match unsafe { raw_syscall_write_buffer(args.a1, args.a2) } {
            Ok(name) => name,
            Err(error) => {
                self.record(SyscallOp::InitServiceStart, SyscallStatus::Error);
                return SyscallRet::failure(error);
            }
        };
        // SAFETY: same synchronous raw-backend contract as the service name.
        let target = match unsafe { raw_syscall_write_buffer(args.a3, args.a4) } {
            Ok(target) => target,
            Err(error) => {
                self.record(SyscallOp::InitServiceStart, SyscallStatus::Error);
                return SyscallRet::failure(error);
            }
        };
        let name = match core::str::from_utf8(name) {
            Ok(name) => name,
            Err(_) => {
                self.record(SyscallOp::InitServiceStart, SyscallStatus::Error);
                return SyscallRet::failure(SyscallError::INVALID_ARGUMENT);
            }
        };
        let target = match core::str::from_utf8(target) {
            Ok(target) => target,
            Err(_) => {
                self.record(SyscallOp::InitServiceStart, SyscallStatus::Error);
                return SyscallRet::failure(SyscallError::INVALID_ARGUMENT);
            }
        };
        let Some((name, target)) = static_init_service_request(name, target) else {
            self.record(SyscallOp::InitServiceStart, SyscallStatus::Error);
            return SyscallRet::failure(SyscallError::INVALID_ARGUMENT);
        };

        match self.request_init_service(name, target) {
            Ok(record) => SyscallRet::success(record.service_pid),
            Err(error) => SyscallRet::failure(syscall_error_from_service_error(error)),
        }
    }

    fn dispatch_raw_source_control(&mut self, args: SyscallArgs) -> SyscallRet {
        let op = match args.a0 {
            RAW_SOURCE_CONTROL_OP_INSTALL_BIN_STATUS
            | RAW_SOURCE_CONTROL_OP_INSTALL_PAYLOAD_STATUS
            | RAW_SOURCE_CONTROL_OP_INSTALL_BIN_MEDIA
            | RAW_SOURCE_CONTROL_OP_INSTALL_PAYLOAD_MEDIA => SyscallOp::SourceInstall,
            _ => {
                self.record(SyscallOp::SourceInstall, SyscallStatus::Error);
                return SyscallRet::failure(SyscallError::INVALID_ARGUMENT);
            }
        };
        if self.current.is_none() {
            self.record(op, SyscallStatus::Unavailable);
            return SyscallRet::failure(SyscallError::NO_CURRENT_PROCESS);
        }
        if args.a2 == 0 {
            self.record(op, SyscallStatus::Error);
            return SyscallRet::failure(SyscallError::INVALID_ARGUMENT);
        }
        // SAFETY: this is the temporary synchronous raw backend. The caller is
        // responsible for passing a valid readable UTF-8 source name buffer.
        let name = match unsafe { raw_syscall_write_buffer(args.a1, args.a2) } {
            Ok(name) => name,
            Err(error) => {
                self.record(op, SyscallStatus::Error);
                return SyscallRet::failure(error);
            }
        };
        let name = match core::str::from_utf8(name) {
            Ok(name) => name,
            Err(_) => {
                self.record(op, SyscallStatus::Error);
                return SyscallRet::failure(SyscallError::INVALID_ARGUMENT);
            }
        };
        // SAFETY: this is the temporary synchronous raw backend. The caller is
        // responsible for passing a live writable `SourceInstallReport`.
        let report = match unsafe { raw_syscall_source_install_report(args.a4, args.a5) } {
            Ok(report) => report,
            Err(error) => {
                self.record(op, SyscallStatus::Error);
                return SyscallRet::failure(error);
            }
        };

        match args.a0 {
            RAW_SOURCE_CONTROL_OP_INSTALL_BIN_STATUS => {
                let Some(status) = source_bin_install_status_from_raw(args.a3) else {
                    self.record(op, SyscallStatus::Error);
                    return SyscallRet::failure(SyscallError::INVALID_ARGUMENT);
                };
                match self.install_bin_source_by_name(name, status) {
                    Ok(record) => {
                        write_bin_source_install_report(report, record);
                        SyscallRet::success(record.bytes_len)
                    }
                    Err(error) => {
                        SyscallRet::failure(syscall_error_from_program_source_install(error))
                    }
                }
            }
            RAW_SOURCE_CONTROL_OP_INSTALL_PAYLOAD_STATUS => {
                let Some(status) = payload_source_install_status_from_raw(args.a3) else {
                    self.record(op, SyscallStatus::Error);
                    return SyscallRet::failure(SyscallError::INVALID_ARGUMENT);
                };
                match self.install_payload_source_by_name(name, status) {
                    Ok(record) => {
                        write_payload_source_install_report(report, record);
                        SyscallRet::success(record.bytes_len)
                    }
                    Err(error) => {
                        SyscallRet::failure(syscall_error_from_program_source_install(error))
                    }
                }
            }
            RAW_SOURCE_CONTROL_OP_INSTALL_BIN_MEDIA => {
                if args.a3 != RAW_SOURCE_INSTALL_STATUS_NONE {
                    self.record(op, SyscallStatus::Error);
                    return SyscallRet::failure(SyscallError::INVALID_ARGUMENT);
                }
                match self.install_bin_source_from_media_by_name(name) {
                    Ok(record) => {
                        write_source_media_install_report(report, record);
                        SyscallRet::success(record.bytes_len)
                    }
                    Err(error) => {
                        SyscallRet::failure(syscall_error_from_program_source_install(error))
                    }
                }
            }
            RAW_SOURCE_CONTROL_OP_INSTALL_PAYLOAD_MEDIA => {
                if args.a3 != RAW_SOURCE_INSTALL_STATUS_NONE {
                    self.record(op, SyscallStatus::Error);
                    return SyscallRet::failure(SyscallError::INVALID_ARGUMENT);
                }
                match self.install_payload_source_from_media_by_name(name) {
                    Ok(record) => {
                        write_source_media_install_report(report, record);
                        SyscallRet::success(record.bytes_len)
                    }
                    Err(error) => {
                        SyscallRet::failure(syscall_error_from_program_source_install(error))
                    }
                }
            }
            _ => SyscallRet::failure(SyscallError::INVALID_ARGUMENT),
        }
    }

    fn dispatch_raw_session_control(&mut self, args: SyscallArgs) -> SyscallRet {
        let op = match args.a0 {
            RAW_SESSION_CONTROL_OP_SHELL_START => SyscallOp::SessionShellStart,
            RAW_SESSION_CONTROL_OP_LINE_DISCIPLINE => SyscallOp::SessionLineDiscipline,
            _ => {
                self.record(SyscallOp::SessionShellStart, SyscallStatus::Error);
                return SyscallRet::failure(SyscallError::INVALID_ARGUMENT);
            }
        };
        if self.current.is_none() {
            self.record(op, SyscallStatus::Unavailable);
            return SyscallRet::failure(SyscallError::NO_CURRENT_PROCESS);
        }

        match args.a0 {
            RAW_SESSION_CONTROL_OP_SHELL_START => {
                if args.a1 != 0 || args.a2 != 0 || args.a3 != 0 || args.a4 != 0 || args.a5 != 0 {
                    self.record(op, SyscallStatus::Error);
                    return SyscallRet::failure(SyscallError::INVALID_ARGUMENT);
                }
                self.request_shell_start();
                SyscallRet::success(0)
            }
            RAW_SESSION_CONTROL_OP_LINE_DISCIPLINE => {
                if args.a2 == 0 || args.a4 == 0 || args.a5 != 0 {
                    self.record(op, SyscallStatus::Error);
                    return SyscallRet::failure(SyscallError::INVALID_ARGUMENT);
                }
                // SAFETY: this is the temporary synchronous raw backend. The
                // caller is responsible for passing valid readable UTF-8
                // session-control strings.
                let line_discipline = match unsafe { raw_syscall_write_buffer(args.a1, args.a2) } {
                    Ok(line_discipline) => line_discipline,
                    Err(error) => {
                        self.record(op, SyscallStatus::Error);
                        return SyscallRet::failure(error);
                    }
                };
                // SAFETY: same synchronous raw-backend contract as the line
                // discipline string.
                let pipe_mode = match unsafe { raw_syscall_write_buffer(args.a3, args.a4) } {
                    Ok(pipe_mode) => pipe_mode,
                    Err(error) => {
                        self.record(op, SyscallStatus::Error);
                        return SyscallRet::failure(error);
                    }
                };
                let line_discipline = match core::str::from_utf8(line_discipline) {
                    Ok(line_discipline) => line_discipline,
                    Err(_) => {
                        self.record(op, SyscallStatus::Error);
                        return SyscallRet::failure(SyscallError::INVALID_ARGUMENT);
                    }
                };
                let pipe_mode = match core::str::from_utf8(pipe_mode) {
                    Ok(pipe_mode) => pipe_mode,
                    Err(_) => {
                        self.record(op, SyscallStatus::Error);
                        return SyscallRet::failure(SyscallError::INVALID_ARGUMENT);
                    }
                };
                let Some((line_discipline, pipe_mode)) =
                    static_session_line_discipline(line_discipline, pipe_mode)
                else {
                    self.record(op, SyscallStatus::Error);
                    return SyscallRet::failure(SyscallError::INVALID_ARGUMENT);
                };

                self.request_shell_line_discipline(line_discipline, pipe_mode);
                SyscallRet::success(0)
            }
            _ => SyscallRet::failure(SyscallError::INVALID_ARGUMENT),
        }
    }

    fn dispatch_raw_get_pid(&self) -> SyscallRet {
        let Some(current) = self.current else {
            self.record(SyscallOp::ProcessSelf, SyscallStatus::Unavailable);
            return SyscallRet::failure(SyscallError::NO_CURRENT_PROCESS);
        };
        self.record(SyscallOp::ProcessSelf, SyscallStatus::Ok);
        SyscallRet::success(current.pid)
    }

    fn dispatch_raw_process_self(&self, args: SyscallArgs) -> SyscallRet {
        if args.a2 != 0 || args.a3 != 0 || args.a4 != 0 || args.a5 != 0 {
            self.record(SyscallOp::ProcessSelf, SyscallStatus::Error);
            return SyscallRet::failure(SyscallError::INVALID_ARGUMENT);
        }
        let Some(current) = self.current else {
            self.record(SyscallOp::ProcessSelf, SyscallStatus::Unavailable);
            return SyscallRet::failure(SyscallError::NO_CURRENT_PROCESS);
        };
        // SAFETY: this is the temporary synchronous raw backend. The caller is
        // responsible for passing a live writable process-control report.
        let report = match unsafe { raw_syscall_process_control_report(args.a0, args.a1) } {
            Ok(report) => report,
            Err(error) => {
                self.record(SyscallOp::ProcessSelf, SyscallStatus::Error);
                return SyscallRet::failure(error);
            }
        };
        let Some(record) = proc::process(current.pid) else {
            self.record(SyscallOp::ProcessSelf, SyscallStatus::Error);
            return SyscallRet::failure(SyscallError::PROCESS_NOT_FOUND);
        };
        write_process_control_report(
            report,
            process_record_handle(record),
            process_state_code(record.state),
        );
        self.record(SyscallOp::ProcessSelf, SyscallStatus::Ok);
        SyscallRet::success(record.pid)
    }

    fn dispatch_raw_get_cwd(&self, args: SyscallArgs) -> SyscallRet {
        if self.current.is_none() {
            self.record(SyscallOp::SessionCwdGet, SyscallStatus::Unavailable);
            return SyscallRet::failure(SyscallError::NO_CURRENT_PROCESS);
        }
        // SAFETY: this is the temporary synchronous raw backend. The caller is
        // responsible for passing a valid writable buffer for `get_cwd`.
        let out = match unsafe { raw_syscall_read_buffer(args.a0, args.a1) } {
            Ok(out) => out,
            Err(error) => {
                self.record(SyscallOp::SessionCwdGet, SyscallStatus::Error);
                return SyscallRet::failure(error);
            }
        };
        let cwd = self.session.cwd().as_bytes();
        if out.len() < cwd.len() {
            self.record(SyscallOp::SessionCwdGet, SyscallStatus::Error);
            return SyscallRet::failure(SyscallError::FILE_TOO_LARGE);
        }
        out[..cwd.len()].copy_from_slice(cwd);
        self.record(SyscallOp::SessionCwdGet, SyscallStatus::Ok);
        SyscallRet::success(cwd.len())
    }

    fn dispatch_raw_chdir(&mut self, args: SyscallArgs) -> SyscallRet {
        if self.current.is_none() {
            self.record(SyscallOp::SessionCwdSet, SyscallStatus::Unavailable);
            return SyscallRet::failure(SyscallError::NO_CURRENT_PROCESS);
        }
        // SAFETY: this is the temporary synchronous raw backend. The caller is
        // responsible for passing a valid readable path buffer for `chdir`.
        let path = match unsafe { raw_syscall_write_buffer(args.a0, args.a1) } {
            Ok(path) => path,
            Err(error) => {
                self.record(SyscallOp::SessionCwdSet, SyscallStatus::Error);
                return SyscallRet::failure(error);
            }
        };
        let target = match core::str::from_utf8(path) {
            Ok(path) => path,
            Err(_) => {
                self.record(SyscallOp::SessionCwdSet, SyscallStatus::Error);
                return SyscallRet::failure(SyscallError::INVALID_ARGUMENT);
            }
        };
        let normalized = match self.normalize_path(target) {
            Ok(path) => path,
            Err(error) => {
                self.record(SyscallOp::SessionCwdSet, SyscallStatus::Error);
                return SyscallRet::failure(syscall_error_from_vfs(error));
            }
        };
        match self.lookup_path(normalized.as_str()) {
            Ok(Node::Directory(_)) => {
                self.set_cwd(normalized);
                SyscallRet::success(0)
            }
            Ok(Node::File(_)) => {
                self.record(SyscallOp::SessionCwdSet, SyscallStatus::Error);
                SyscallRet::failure(SyscallError::NOT_DIRECTORY)
            }
            Err(error) => {
                self.record(SyscallOp::SessionCwdSet, SyscallStatus::Error);
                SyscallRet::failure(syscall_error_from_vfs(error))
            }
        }
    }

    fn dispatch_raw_yield_now(&mut self, args: SyscallArgs) -> SyscallRet {
        if args != SyscallArgs::EMPTY {
            self.record(SyscallOp::YieldNow, SyscallStatus::Error);
            return SyscallRet::failure(SyscallError::INVALID_ARGUMENT);
        }
        let result = self.yield_now_result();
        match result.status {
            SchedulerYieldStatus::Yielded | SchedulerYieldStatus::NoPeer => {
                SyscallRet::success(usize::from(result.yielded))
            }
            SchedulerYieldStatus::Unavailable => {
                SyscallRet::failure(SyscallError::NO_CURRENT_PROCESS)
            }
            SchedulerYieldStatus::Busy => SyscallRet::failure(SyscallError::BUSY),
            SchedulerYieldStatus::NotRunning => SyscallRet::failure(SyscallError::INVALID_ARGUMENT),
            SchedulerYieldStatus::Failed => SyscallRet::failure(SyscallError::IO),
        }
    }

    fn dispatch_raw_sleep(&mut self, args: SyscallArgs) -> SyscallRet {
        if args.a0 == 0
            || args.a1 != 0
            || args.a2 != 0
            || args.a3 != 0
            || args.a4 != 0
            || args.a5 != 0
        {
            self.record(SyscallOp::ProcessSleep, SyscallStatus::Error);
            return SyscallRet::failure(SyscallError::INVALID_ARGUMENT);
        }
        if self.current_has_direct_backend_admission_block() {
            self.record(SyscallOp::SyscallContinue, SyscallStatus::Error);
            return SyscallRet::failure(SyscallError::BUSY);
        }

        if self.replaying_continuation == Some(SyscallNr::SLEEP) {
            return self.dispatch_replayed_raw_sleep(args);
        }

        self.store_raw_sleep_continuation(args, 0)
    }

    fn dispatch_replayed_raw_sleep(&mut self, args: SyscallArgs) -> SyscallRet {
        let wake_tick = self.replaying_sleep_wake_tick;
        if wake_tick != 0 && sched::snapshot_scheduler().tick_count < wake_tick {
            return self.store_raw_sleep_continuation(args, wake_tick);
        }

        let Some(current) = self.current else {
            record_syscall(self.current, SyscallOp::ProcessSleep, SyscallStatus::Unavailable);
            return SyscallRet::failure(SyscallError::NO_CURRENT_PROCESS);
        };
        match proc::process(current.pid) {
            Some(process) if process.state == proc::ProcessState::Running => {
                record_context(current, SyscallOp::ProcessSleep, SyscallStatus::Ok);
                SyscallRet::success(sched::snapshot_scheduler().tick_count)
            }
            Some(_) => {
                record_context(current, SyscallOp::ProcessSleep, SyscallStatus::Error);
                SyscallRet::failure(SyscallError::INVALID_ARGUMENT)
            }
            None => {
                record_context(current, SyscallOp::ProcessSleep, SyscallStatus::Error);
                SyscallRet::failure(SyscallError::IO)
            }
        }
    }

    fn store_raw_sleep_continuation(
        &mut self,
        args: SyscallArgs,
        existing_wake_tick: usize,
    ) -> SyscallRet {
        let Some(current) = self.current else {
            record_syscall(self.current, SyscallOp::ProcessSleep, SyscallStatus::Unavailable);
            return SyscallRet::failure(SyscallError::NO_CURRENT_PROCESS);
        };
        let Some(process) = proc::process(current.pid) else {
            record_context(current, SyscallOp::ProcessSleep, SyscallStatus::Error);
            return SyscallRet::failure(SyscallError::IO);
        };
        if process.state != proc::ProcessState::Running {
            record_context(current, SyscallOp::ProcessSleep, SyscallStatus::Error);
            return SyscallRet::failure(SyscallError::INVALID_ARGUMENT);
        }

        let wake_tick = if existing_wake_tick == 0 {
            sched::snapshot_scheduler()
                .tick_count
                .saturating_add(args.a0)
        } else {
            existing_wake_tick
        };
        let Some(sleeping) = proc::sleep_process_until(current.pid, wake_tick) else {
            record_context(current, SyscallOp::ProcessSleep, SyscallStatus::Error);
            return SyscallRet::failure(SyscallError::IO);
        };
        let sleeping_ctx = SyscallContext::from_process(sleeping);
        record_context(sleeping_ctx, SyscallOp::ProcessSleep, SyscallStatus::Ok);
        append_process_control_event(sleeping_ctx, SyscallOp::ProcessSleep);

        let stored = store_program_sleep_syscall_continuation(current, args, wake_tick);
        if stored.is_ok() {
            self.record(SyscallOp::SyscallContinue, SyscallStatus::Blocked);
            SyscallRet::failure(SyscallError::BUSY)
        } else {
            if let Some(handle) = proc::wake_process(current.pid) {
                record_process_wake_event(handle);
            }
            self.record(SyscallOp::SyscallContinue, SyscallStatus::Error);
            SyscallRet::failure(SyscallError::IO)
        }
    }

    fn dispatch_raw_scheduler_tick(&mut self, args: SyscallArgs) -> SyscallRet {
        if args != SyscallArgs::EMPTY {
            self.record(SyscallOp::SchedulerTick, SyscallStatus::Error);
            return SyscallRet::failure(SyscallError::INVALID_ARGUMENT);
        }
        let result = self.scheduler_tick_result();
        match result.status {
            SchedulerTickStatus::Ok => SyscallRet::success(result.tick_count),
            SchedulerTickStatus::Unavailable => {
                SyscallRet::failure(SyscallError::NO_CURRENT_PROCESS)
            }
            SchedulerTickStatus::Busy => SyscallRet::failure(SyscallError::BUSY),
            SchedulerTickStatus::NotRunning => SyscallRet::failure(SyscallError::INVALID_ARGUMENT),
            SchedulerTickStatus::Failed => SyscallRet::failure(SyscallError::IO),
        }
    }

    fn dispatch_raw_spawn(&mut self, args: SyscallArgs) -> SyscallRet {
        if self.current.is_none() {
            self.record(SyscallOp::ExecLoad, SyscallStatus::Unavailable);
            return SyscallRet::failure(SyscallError::NO_CURRENT_PROCESS);
        }
        if args.a2 == RAW_PROCESS_SPAWN_TARGET_BIN_WITH_REQUEST
            || args.a2 == RAW_PROCESS_SPAWN_TARGET_PAYLOAD_WITH_REQUEST
        {
            if args.a3 != 0 || args.a4 != 0 || args.a5 != 0 {
                self.record(SyscallOp::ExecLoad, SyscallStatus::Error);
                return SyscallRet::failure(SyscallError::INVALID_ARGUMENT);
            }
            let expected_target = if args.a2 == RAW_PROCESS_SPAWN_TARGET_PAYLOAD_WITH_REQUEST {
                RAW_PROCESS_SPAWN_TARGET_PAYLOAD
            } else {
                RAW_PROCESS_SPAWN_TARGET_BIN
            };
            // SAFETY: this is the temporary direct raw backend. The caller is
            // responsible for passing a valid readable process-spawn request.
            let request = match unsafe { raw_syscall_process_spawn_request(args.a0, args.a1) } {
                Ok(request) => *request,
                Err(error) => {
                    self.record(SyscallOp::ExecLoad, SyscallStatus::Error);
                    return SyscallRet::failure(error);
                }
            };
            return self.dispatch_raw_spawn_request(request, expected_target);
        }
        // SAFETY: this is the temporary direct raw backend. The caller is
        // responsible for passing a valid readable `ProcessArg` slice.
        let raw_args = match unsafe { raw_syscall_process_args(args.a0, args.a1) } {
            Ok(raw_args) => raw_args,
            Err(error) => {
                self.record(SyscallOp::ExecLoad, SyscallStatus::Error);
                return SyscallRet::failure(error);
            }
        };
        let argv = match argv_from_process_args(raw_args) {
            Ok(argv) => argv,
            Err(error) => {
                self.record(SyscallOp::ExecLoad, SyscallStatus::Error);
                return SyscallRet::failure(error);
            }
        };
        match args.a2 {
            RAW_PROCESS_SPAWN_TARGET_BIN => match args.a3 {
                RAW_PROCESS_SPAWN_MODE_READY => {
                    // SAFETY: this is the temporary synchronous raw backend.
                    // The caller may pass a live writable
                    // `ProcessControlReport` to request structured metadata.
                    let report = match unsafe {
                        raw_syscall_optional_process_control_report(args.a4, args.a5)
                    } {
                        Ok(report) => report,
                        Err(error) => {
                            self.record(SyscallOp::ExecLoad, SyscallStatus::Error);
                            return SyscallRet::failure(error);
                        }
                    };
                    match self.spawn_program_argv(argv) {
                        Ok(child) => {
                            if let Some(report) = report {
                                write_process_control_report(
                                    report,
                                    child,
                                    ProcessStateCode::READY,
                                );
                            }
                            SyscallRet::success(child.pid)
                        }
                        Err(error) => SyscallRet::failure(syscall_error_from_program_exec(error)),
                    }
                }
                RAW_PROCESS_SPAWN_MODE_BLOCKED => {
                    // SAFETY: this is the temporary synchronous raw backend.
                    // The caller may pass a live writable
                    // `ProcessControlReport` to request structured metadata.
                    let report = match unsafe {
                        raw_syscall_optional_process_control_report(args.a4, args.a5)
                    } {
                        Ok(report) => report,
                        Err(error) => {
                            self.record(SyscallOp::ExecLoad, SyscallStatus::Error);
                            return SyscallRet::failure(error);
                        }
                    };
                    match self.spawn_blocked_program_argv(argv) {
                        Ok(child) => {
                            if let Some(report) = report {
                                write_process_control_report(
                                    report,
                                    child,
                                    ProcessStateCode::BLOCKED,
                                );
                            }
                            SyscallRet::success(child.pid)
                        }
                        Err(error) => SyscallRet::failure(syscall_error_from_program_exec(error)),
                    }
                }
                RAW_PROCESS_SPAWN_MODE_SLEEPING => {
                    // SAFETY: this is the temporary synchronous raw backend.
                    // The caller is responsible for passing a live writable
                    // `ProcessSleepReport`.
                    let report = match unsafe { raw_syscall_process_sleep_report(args.a4, args.a5) }
                    {
                        Ok(report) => report,
                        Err(error) => {
                            self.record(SyscallOp::ExecLoad, SyscallStatus::Error);
                            return SyscallRet::failure(error);
                        }
                    };
                    let requested_ticks = report.requested_ticks();
                    if requested_ticks == 0 {
                        self.record(SyscallOp::ProcessSleep, SyscallStatus::Error);
                        return SyscallRet::failure(SyscallError::INVALID_ARGUMENT);
                    }
                    match self.spawn_sleeping_program_argv(argv, requested_ticks) {
                        Ok(result) => {
                            let child_pid = result.child.pid;
                            write_process_sleep_report(report, requested_ticks, result);
                            SyscallRet::success(child_pid)
                        }
                        Err(error) => SyscallRet::failure(syscall_error_from_program_exec(error)),
                    }
                }
                _ => {
                    self.record(SyscallOp::ExecLoad, SyscallStatus::Error);
                    SyscallRet::failure(SyscallError::INVALID_ARGUMENT)
                }
            },
            RAW_PROCESS_SPAWN_TARGET_BIN_WITH_ENV => match args.a3 {
                RAW_PROCESS_SPAWN_MODE_READY => {
                    // SAFETY: this is the temporary synchronous raw backend.
                    // The caller is responsible for passing a valid readable
                    // `ProcessEnv` slice in the non-reporting env slots.
                    let raw_env = match unsafe { raw_syscall_process_env(args.a4, args.a5) } {
                        Ok(raw_env) => raw_env,
                        Err(error) => {
                            self.record(SyscallOp::ExecLoad, SyscallStatus::Error);
                            return SyscallRet::failure(error);
                        }
                    };
                    let env = match env_from_process_env(raw_env) {
                        Ok(env) => env,
                        Err(error) => {
                            self.record(SyscallOp::ExecLoad, SyscallStatus::Error);
                            return SyscallRet::failure(error);
                        }
                    };
                    match self.spawn_program_argv_env(argv, env) {
                        Ok(child) => SyscallRet::success(child.pid),
                        Err(error) => SyscallRet::failure(syscall_error_from_program_exec(error)),
                    }
                }
                RAW_PROCESS_SPAWN_MODE_BLOCKED => {
                    // SAFETY: this is the temporary synchronous raw backend.
                    // The caller is responsible for passing a valid readable
                    // `ProcessEnv` slice in the non-reporting env slots.
                    let raw_env = match unsafe { raw_syscall_process_env(args.a4, args.a5) } {
                        Ok(raw_env) => raw_env,
                        Err(error) => {
                            self.record(SyscallOp::ExecLoad, SyscallStatus::Error);
                            return SyscallRet::failure(error);
                        }
                    };
                    let env = match env_from_process_env(raw_env) {
                        Ok(env) => env,
                        Err(error) => {
                            self.record(SyscallOp::ExecLoad, SyscallStatus::Error);
                            return SyscallRet::failure(error);
                        }
                    };
                    match self.spawn_blocked_program_argv_env(argv, env) {
                        Ok(child) => SyscallRet::success(child.pid),
                        Err(error) => SyscallRet::failure(syscall_error_from_program_exec(error)),
                    }
                }
                _ => {
                    self.record(SyscallOp::ExecLoad, SyscallStatus::Error);
                    SyscallRet::failure(SyscallError::INVALID_ARGUMENT)
                }
            },
            RAW_PROCESS_SPAWN_TARGET_PAYLOAD => {
                if args.a3 != RAW_PROCESS_SPAWN_MODE_READY || args.a4 != 0 || args.a5 != 0 {
                    self.record(SyscallOp::ExecLoad, SyscallStatus::Error);
                    return SyscallRet::failure(SyscallError::INVALID_ARGUMENT);
                }
                let launch_enabled = self.daemon.launch_enabled();
                match self.spawn_payload_argv(argv) {
                    Ok(child) => SyscallRet::success(child.pid),
                    Err(PayloadLaunchResult::NotConfigured) if !launch_enabled => {
                        SyscallRet::failure(SyscallError::UNSUPPORTED)
                    }
                    Err(PayloadLaunchResult::NotConfigured) => {
                        SyscallRet::failure(SyscallError::NOT_FOUND)
                    }
                    Err(PayloadLaunchResult::Failed) => {
                        SyscallRet::failure(SyscallError::INVALID_IMAGE)
                    }
                    Err(PayloadLaunchResult::Resident | PayloadLaunchResult::Ready) => {
                        SyscallRet::failure(SyscallError::IO)
                    }
                    Err(PayloadLaunchResult::ExitCode(_)) => SyscallRet::failure(SyscallError::IO),
                }
            }
            _ => {
                self.record(SyscallOp::ExecLoad, SyscallStatus::Error);
                SyscallRet::failure(SyscallError::INVALID_ARGUMENT)
            }
        }
    }

    fn dispatch_raw_spawn_request(
        &mut self,
        request: ProcessSpawnRequest,
        expected_target: usize,
    ) -> SyscallRet {
        if !request.flags().is_known() {
            self.record(SyscallOp::ExecLoad, SyscallStatus::Error);
            return SyscallRet::failure(SyscallError::INVALID_ARGUMENT);
        }
        if request.target().raw() != expected_target {
            self.record(SyscallOp::ExecLoad, SyscallStatus::Error);
            return SyscallRet::failure(SyscallError::INVALID_ARGUMENT);
        }
        if request.target().raw() == RAW_PROCESS_SPAWN_TARGET_PAYLOAD
            && (request.mode().raw() != RAW_PROCESS_SPAWN_MODE_READY
                || request.report_ptr() != 0
                || request.report_len() != 0)
        {
            self.record(SyscallOp::ExecLoad, SyscallStatus::Error);
            return SyscallRet::failure(SyscallError::INVALID_ARGUMENT);
        }
        // SAFETY: this is the temporary direct raw backend. The request points
        // at caller-owned process argv records for this synchronous dispatch.
        let raw_args = match unsafe {
            raw_syscall_process_args(request.argv_ptr() as usize, request.argc())
        } {
            Ok(raw_args) => raw_args,
            Err(error) => {
                self.record(SyscallOp::ExecLoad, SyscallStatus::Error);
                return SyscallRet::failure(error);
            }
        };
        let argv = match argv_from_process_args(raw_args) {
            Ok(argv) => argv,
            Err(error) => {
                self.record(SyscallOp::ExecLoad, SyscallStatus::Error);
                return SyscallRet::failure(error);
            }
        };
        // SAFETY: same temporary synchronous raw-backend contract as argv.
        let raw_env =
            match unsafe { raw_syscall_process_env(request.env_ptr() as usize, request.envc()) } {
                Ok(raw_env) => raw_env,
                Err(error) => {
                    self.record(SyscallOp::ExecLoad, SyscallStatus::Error);
                    return SyscallRet::failure(error);
                }
            };
        let env = match env_from_process_env(raw_env) {
            Ok(env) => env,
            Err(error) => {
                self.record(SyscallOp::ExecLoad, SyscallStatus::Error);
                return SyscallRet::failure(error);
            }
        };

        if request.target().raw() == RAW_PROCESS_SPAWN_TARGET_PAYLOAD {
            let launch_enabled = self.daemon.launch_enabled();
            return match self.spawn_payload_argv_env(argv, env) {
                Ok(child) => SyscallRet::success(child.pid),
                Err(PayloadLaunchResult::NotConfigured) if !launch_enabled => {
                    SyscallRet::failure(SyscallError::UNSUPPORTED)
                }
                Err(PayloadLaunchResult::NotConfigured) => {
                    SyscallRet::failure(SyscallError::NOT_FOUND)
                }
                Err(PayloadLaunchResult::Failed) => {
                    SyscallRet::failure(SyscallError::INVALID_IMAGE)
                }
                Err(PayloadLaunchResult::Resident | PayloadLaunchResult::Ready) => {
                    SyscallRet::failure(SyscallError::IO)
                }
                Err(PayloadLaunchResult::ExitCode(_)) => SyscallRet::failure(SyscallError::IO),
            };
        }

        match request.mode().raw() {
            RAW_PROCESS_SPAWN_MODE_READY => {
                // SAFETY: request carries an optional live process-control
                // report pointer for this synchronous dispatch.
                let report = match unsafe {
                    raw_syscall_optional_process_control_report(
                        request.report_ptr(),
                        request.report_len(),
                    )
                } {
                    Ok(report) => report,
                    Err(error) => {
                        self.record(SyscallOp::ExecLoad, SyscallStatus::Error);
                        return SyscallRet::failure(error);
                    }
                };
                match self.spawn_program_argv_env(argv, env) {
                    Ok(child) => {
                        if let Some(report) = report {
                            write_process_control_report(report, child, ProcessStateCode::READY);
                        }
                        SyscallRet::success(child.pid)
                    }
                    Err(error) => SyscallRet::failure(syscall_error_from_program_exec(error)),
                }
            }
            RAW_PROCESS_SPAWN_MODE_BLOCKED => {
                // SAFETY: request carries an optional live process-control
                // report pointer for this synchronous dispatch.
                let report = match unsafe {
                    raw_syscall_optional_process_control_report(
                        request.report_ptr(),
                        request.report_len(),
                    )
                } {
                    Ok(report) => report,
                    Err(error) => {
                        self.record(SyscallOp::ExecLoad, SyscallStatus::Error);
                        return SyscallRet::failure(error);
                    }
                };
                match self.spawn_blocked_program_argv_env(argv, env) {
                    Ok(child) => {
                        if let Some(report) = report {
                            write_process_control_report(report, child, ProcessStateCode::BLOCKED);
                        }
                        SyscallRet::success(child.pid)
                    }
                    Err(error) => SyscallRet::failure(syscall_error_from_program_exec(error)),
                }
            }
            RAW_PROCESS_SPAWN_MODE_SLEEPING => {
                // SAFETY: request carries a required live process-sleep report
                // pointer for this synchronous dispatch.
                let report = match unsafe {
                    raw_syscall_process_sleep_report(request.report_ptr(), request.report_len())
                } {
                    Ok(report) => report,
                    Err(error) => {
                        self.record(SyscallOp::ExecLoad, SyscallStatus::Error);
                        return SyscallRet::failure(error);
                    }
                };
                let requested_ticks = report.requested_ticks();
                if requested_ticks == 0 {
                    self.record(SyscallOp::ProcessSleep, SyscallStatus::Error);
                    return SyscallRet::failure(SyscallError::INVALID_ARGUMENT);
                }
                match self.spawn_sleeping_program_argv_env(argv, env, requested_ticks) {
                    Ok(result) => {
                        let child_pid = result.child.pid;
                        write_process_sleep_report(report, requested_ticks, result);
                        SyscallRet::success(child_pid)
                    }
                    Err(error) => SyscallRet::failure(syscall_error_from_program_exec(error)),
                }
            }
            _ => {
                self.record(SyscallOp::ExecLoad, SyscallStatus::Error);
                SyscallRet::failure(SyscallError::INVALID_ARGUMENT)
            }
        }
    }

    fn dispatch_raw_execve(&mut self, args: SyscallArgs) -> SyscallRet {
        if self.current.is_none() {
            self.record(SyscallOp::ExecLoad, SyscallStatus::Unavailable);
            return SyscallRet::failure(SyscallError::NO_CURRENT_PROCESS);
        }
        if self.current_has_direct_backend_admission_block() {
            self.record(SyscallOp::SyscallContinue, SyscallStatus::Error);
            return SyscallRet::failure(SyscallError::BUSY);
        }
        if args.a4 != 0 || args.a5 != 0 {
            self.record(SyscallOp::ExecLoad, SyscallStatus::Error);
            return SyscallRet::failure(SyscallError::INVALID_ARGUMENT);
        }
        // SAFETY: this is the temporary synchronous raw backend. The caller is
        // responsible for passing a valid readable `ProcessArg` slice.
        let raw_args = match unsafe { raw_syscall_process_args(args.a0, args.a1) } {
            Ok(raw_args) => raw_args,
            Err(error) => {
                self.record(SyscallOp::ExecLoad, SyscallStatus::Error);
                return SyscallRet::failure(error);
            }
        };
        let argv = match argv_from_process_args(raw_args) {
            Ok(argv) => argv,
            Err(error) => {
                self.record(SyscallOp::ExecLoad, SyscallStatus::Error);
                return SyscallRet::failure(error);
            }
        };
        // SAFETY: this is the temporary synchronous raw backend. The caller is
        // responsible for passing a valid readable `ProcessEnv` slice.
        let raw_env = match unsafe { raw_syscall_process_env(args.a2, args.a3) } {
            Ok(raw_env) => raw_env,
            Err(error) => {
                self.record(SyscallOp::ExecLoad, SyscallStatus::Error);
                return SyscallRet::failure(error);
            }
        };
        let env = match env_from_process_env(raw_env) {
            Ok(env) => env,
            Err(error) => {
                self.record(SyscallOp::ExecLoad, SyscallStatus::Error);
                return SyscallRet::failure(error);
            }
        };
        match self.admit_execve_program_argv_env(argv, env) {
            Ok(_) => SyscallRet::success(0),
            Err(error) => SyscallRet::failure(syscall_error_from_program_exec(error)),
        }
    }

    fn dispatch_raw_wait(&mut self, args: SyscallArgs) -> SyscallRet {
        if self.current_has_direct_backend_admission_block() {
            self.record(SyscallOp::SyscallContinue, SyscallStatus::Error);
            return SyscallRet::failure(SyscallError::BUSY);
        }
        if args.a3 != 0 || args.a4 != 0 || args.a5 != 0 {
            self.record(SyscallOp::WaitBegin, SyscallStatus::Error);
            return SyscallRet::failure(SyscallError::INVALID_ARGUMENT);
        }
        // SAFETY: this is the temporary synchronous raw backend. The caller may
        // pass a live writable `ProcessWaitReport`; scalar callers pass zeros.
        let report = match unsafe { raw_syscall_optional_process_wait_report(args.a1, args.a2) } {
            Ok(report) => report,
            Err(error) => {
                self.record(SyscallOp::WaitBegin, SyscallStatus::Error);
                return SyscallRet::failure(error);
            }
        };
        match self.wait_process_by_pid_inner(
            args.a0,
            self.replaying_continuation == Some(SyscallNr::WAIT),
        ) {
            Ok(wait) => {
                if let Some(report) = report {
                    write_process_wait_report(report, wait);
                }
                if let Some(current) = self.current {
                    clear_matching_program_syscall_continuation(current.pid, SyscallNr::WAIT, args);
                }
                if wait.exit_code < 0 || wait.exit_code > u8::MAX as i32 {
                    SyscallRet::failure(SyscallError::INVALID_ARGUMENT)
                } else {
                    SyscallRet::success(wait.exit_code as usize)
                }
            }
            Err(error) => {
                if error == ProgramProcessError::SchedulerEmpty {
                    if let Some(current) = self.current {
                        let stored =
                            store_program_syscall_continuation(current, SyscallNr::WAIT, args);
                        let status = if stored.is_ok() {
                            SyscallStatus::Blocked
                        } else {
                            SyscallStatus::Error
                        };
                        self.record(SyscallOp::SyscallContinue, status);
                    }
                }
                SyscallRet::failure(syscall_error_from_program_process(error))
            }
        }
    }

    fn dispatch_raw_process_wait_ready(&mut self, args: SyscallArgs) -> SyscallRet {
        if self.current_has_direct_backend_admission_block() {
            self.record(SyscallOp::SyscallContinue, SyscallStatus::Error);
            return SyscallRet::failure(SyscallError::BUSY);
        }
        if args.a3 != 0 || args.a4 != 0 || args.a5 != 0 {
            self.record(SyscallOp::WaitBegin, SyscallStatus::Error);
            return SyscallRet::failure(SyscallError::INVALID_ARGUMENT);
        }
        // SAFETY: this is the temporary synchronous raw backend. The caller must
        // pass a live writable `ProcessWaitReport` because scalar success cannot
        // distinguish resident readiness from exit code 0.
        let report = match unsafe { raw_syscall_optional_process_wait_report(args.a1, args.a2) } {
            Ok(Some(report)) => report,
            Ok(None) => {
                self.record(SyscallOp::WaitBegin, SyscallStatus::Error);
                return SyscallRet::failure(SyscallError::INVALID_ARGUMENT);
            }
            Err(error) => {
                self.record(SyscallOp::WaitBegin, SyscallStatus::Error);
                return SyscallRet::failure(error);
            }
        };
        match self.wait_process_by_pid_until_ready(
            args.a0,
            self.replaying_continuation == Some(SyscallNr::PROCESS_WAIT_READY),
        ) {
            Ok(wait) => {
                write_process_wait_report(report, wait);
                if let Some(current) = self.current {
                    clear_matching_program_syscall_continuation(
                        current.pid,
                        SyscallNr::PROCESS_WAIT_READY,
                        args,
                    );
                }
                if wait.completed && (wait.exit_code < 0 || wait.exit_code > u8::MAX as i32) {
                    SyscallRet::failure(SyscallError::INVALID_ARGUMENT)
                } else {
                    SyscallRet::success(wait.exit_code.max(0) as usize)
                }
            }
            Err(error) => {
                if error == ProgramProcessError::SchedulerEmpty {
                    if let Some(current) = self.current {
                        let stored = store_program_syscall_continuation(
                            current,
                            SyscallNr::PROCESS_WAIT_READY,
                            args,
                        );
                        let status = if stored.is_ok() {
                            SyscallStatus::Blocked
                        } else {
                            SyscallStatus::Error
                        };
                        self.record(SyscallOp::SyscallContinue, status);
                    }
                }
                SyscallRet::failure(syscall_error_from_program_process(error))
            }
        }
    }

    fn dispatch_raw_process_wake(&mut self, args: SyscallArgs) -> SyscallRet {
        if self.current.is_none() {
            self.record(SyscallOp::ProcessWake, SyscallStatus::Unavailable);
            return SyscallRet::failure(SyscallError::NO_CURRENT_PROCESS);
        }
        if args.a3 != 0 || args.a4 != 0 || args.a5 != 0 {
            self.record(SyscallOp::ProcessWake, SyscallStatus::Error);
            return SyscallRet::failure(SyscallError::INVALID_ARGUMENT);
        }
        // SAFETY: this is the temporary synchronous raw backend. The caller may
        // pass a live writable `ProcessControlReport` in a1/a2.
        let report = match unsafe { raw_syscall_optional_process_control_report(args.a1, args.a2) }
        {
            Ok(report) => report,
            Err(error) => {
                self.record(SyscallOp::ProcessWake, SyscallStatus::Error);
                return SyscallRet::failure(error);
            }
        };
        match self.wake_process_by_pid(args.a0) {
            Ok(handle) => {
                if let Some(report) = report {
                    write_process_control_report(report, handle, ProcessStateCode::READY);
                }
                SyscallRet::success(handle.pid)
            }
            Err(error) => SyscallRet::failure(syscall_error_from_program_process(error)),
        }
    }

    fn dispatch_raw_process_kill(&mut self, args: SyscallArgs) -> SyscallRet {
        if self.current.is_none() {
            self.record(SyscallOp::ProcessKill, SyscallStatus::Unavailable);
            return SyscallRet::failure(SyscallError::NO_CURRENT_PROCESS);
        }
        if args.a3 != 0 || args.a4 != 0 || args.a5 != 0 {
            self.record(SyscallOp::ProcessKill, SyscallStatus::Error);
            return SyscallRet::failure(SyscallError::INVALID_ARGUMENT);
        }
        // SAFETY: this is the temporary synchronous raw backend. The caller may
        // pass a live writable `ProcessControlReport` in a1/a2.
        let report = match unsafe { raw_syscall_optional_process_control_report(args.a1, args.a2) }
        {
            Ok(report) => report,
            Err(error) => {
                self.record(SyscallOp::ProcessKill, SyscallStatus::Error);
                return SyscallRet::failure(error);
            }
        };
        match self.kill_process_by_pid(args.a0) {
            Ok(handle) => {
                if let Some(report) = report {
                    write_process_control_report(report, handle, ProcessStateCode::FAILED);
                }
                SyscallRet::success(handle.pid)
            }
            Err(error) => SyscallRet::failure(syscall_error_from_program_process(error)),
        }
    }

    fn dispatch_raw_process_wait_ticks(&mut self, args: SyscallArgs) -> SyscallRet {
        if self.current.is_none() {
            self.record(SyscallOp::WaitBegin, SyscallStatus::Unavailable);
            return SyscallRet::failure(SyscallError::NO_CURRENT_PROCESS);
        }
        if args.a4 != 0 || args.a5 != 0 || args.a1 == 0 {
            self.record(SyscallOp::WaitBegin, SyscallStatus::Error);
            return SyscallRet::failure(SyscallError::INVALID_ARGUMENT);
        }
        // SAFETY: this is the temporary synchronous raw backend. The caller is
        // responsible for passing a live writable `ProcessTimedWaitReport`.
        let report = match unsafe { raw_syscall_process_timed_wait_report(args.a2, args.a3) } {
            Ok(report) => report,
            Err(error) => {
                self.record(SyscallOp::WaitBegin, SyscallStatus::Error);
                return SyscallRet::failure(error);
            }
        };
        match self.wait_process_by_pid_for_ticks(args.a0, args.a1) {
            Ok(result) => {
                let child_pid = result.child_pid;
                write_process_timed_wait_report(report, args.a1, result);
                SyscallRet::success(child_pid)
            }
            Err(error) => SyscallRet::failure(syscall_error_from_program_process(error)),
        }
    }

    fn dispatch_raw_exit(&mut self, args: SyscallArgs) -> SyscallRet {
        if self.current.is_none() {
            self.record(SyscallOp::ProcessExitRequest, SyscallStatus::Unavailable);
            return SyscallRet::failure(SyscallError::NO_CURRENT_PROCESS);
        }
        if args.a0 > u8::MAX as usize || self.raw_exit_status.is_some() {
            self.record(SyscallOp::ProcessExitRequest, SyscallStatus::Error);
            return SyscallRet::failure(SyscallError::INVALID_ARGUMENT);
        }

        let code = args.a0 as i32;
        self.raw_exit_status = Some(if code == 0 {
            ProgramStatus::Ok
        } else {
            ProgramStatus::ExitCode(code)
        });
        self.record(SyscallOp::ProcessExitRequest, SyscallStatus::Ok);
        SyscallRet::success(0)
    }

    /// Writes bytes to the active stdout/tty sink.
    pub fn stdout_bytes(&self, bytes: &[u8]) {
        let _ = self.write_fd(self.stdout().fd, bytes);
    }

    /// Writes one line to the active stdout/tty sink.
    pub fn stdout_line(&self, line: &str) {
        let _ = self.write_fd_line(self.stdout().fd, line);
    }

    /// Writes bytes to the active stderr/tty sink.
    ///
    /// Stderr currently shares the active console sink with stdout, but it has
    /// a distinct descriptor and retained syscall record for later TTY split.
    pub fn stderr_bytes(&self, bytes: &[u8]) {
        let _ = self.write_fd(self.stderr().fd, bytes);
    }

    /// Writes one line to the active stderr/tty sink.
    pub fn stderr_line(&self, line: &str) {
        let _ = self.write_fd_line(self.stderr().fd, line);
    }

    fn write_pipe_fd(&self, fd: usize, bytes: &[u8]) -> Result<usize, ProgramIoError> {
        let Some(current) = self.current else {
            self.record(SyscallOp::FdWrite, SyscallStatus::Error);
            return Err(ProgramIoError::BadFd);
        };
        let mut read_waiters = [0usize; MAX_PROGRAM_PIPE_WAITERS];
        let table_write = with_existing_process_fd_table_mut(current.pid, |table| {
            table.write_pipe(fd, bytes, &mut read_waiters)
        });
        match table_write {
            Some(Ok((written, should_record, waiter_count))) => {
                if should_record && !bytes.is_empty() {
                    self.record(SyscallOp::FdWrite, SyscallStatus::Ok);
                }
                let _ = wake_pipe_read_waiters(&read_waiters[..waiter_count]);
                Ok(written)
            }
            Some(Err(ProgramIoError::Busy)) => {
                let Some(endpoint) = with_existing_process_fd_table_mut(current.pid, |table| {
                    table.pipe_endpoint_for_fd(fd)
                }) else {
                    self.record(SyscallOp::FdWrite, SyscallStatus::Error);
                    return Err(ProgramIoError::BadFd);
                };
                let (pipe_slot, end) = match endpoint {
                    Ok(endpoint) => endpoint,
                    Err(error) => {
                        self.record(SyscallOp::FdWrite, SyscallStatus::Error);
                        return Err(error);
                    }
                };
                if end != ProgramPipeEnd::Write {
                    self.record(SyscallOp::FdWrite, SyscallStatus::Error);
                    return Err(ProgramIoError::NotWritable);
                }
                if let Err(error) = record_program_pipe_write_waiter(pipe_slot, current.pid) {
                    record_context(current, SyscallOp::ProcessBlock, SyscallStatus::Error);
                    self.record(SyscallOp::FdWrite, SyscallStatus::Error);
                    return Err(error);
                }
                if let Some(blocked) = proc::block_pipe_write_process(current.pid) {
                    let blocked_ctx = SyscallContext::from_process(blocked);
                    record_context(blocked_ctx, SyscallOp::ProcessBlock, SyscallStatus::Ok);
                    append_process_control_event(blocked_ctx, SyscallOp::ProcessBlock);
                } else {
                    record_context(current, SyscallOp::ProcessBlock, SyscallStatus::Error);
                    release_program_pipe_write_waiters(current.pid);
                    self.record(SyscallOp::FdWrite, SyscallStatus::Error);
                    return Err(ProgramIoError::NoSpace);
                }
                self.record(SyscallOp::FdWrite, SyscallStatus::Error);
                Err(ProgramIoError::Busy)
            }
            Some(Err(error)) => {
                self.record(SyscallOp::FdWrite, SyscallStatus::Error);
                Err(error)
            }
            None => {
                self.record(SyscallOp::FdWrite, SyscallStatus::Error);
                Err(ProgramIoError::BadFd)
            }
        }
    }

    fn write_tty_fd(&self, open_file_slot: usize, bytes: &[u8]) -> Result<usize, ProgramIoError> {
        match validate_program_tty_write(open_file_slot) {
            Ok(()) => {
                if !bytes.is_empty() {
                    self.record(SyscallOp::FdWrite, SyscallStatus::Ok);
                }
                self.daemon.write_bytes(bytes);
                Ok(bytes.len())
            }
            Err(error) => {
                self.record(SyscallOp::FdWrite, SyscallStatus::Error);
                Err(error)
            }
        }
    }

    fn write_tty_fd_line(
        &self,
        open_file_slot: usize,
        line: &str,
    ) -> Result<usize, ProgramIoError> {
        match validate_program_tty_write(open_file_slot) {
            Ok(()) => {
                self.record(SyscallOp::FdWrite, SyscallStatus::Ok);
                self.daemon.write_line(line);
                Ok(line.len().saturating_add(1))
            }
            Err(error) => {
                self.record(SyscallOp::FdWrite, SyscallStatus::Error);
                Err(error)
            }
        }
    }

    fn read_tty_fd(&mut self, fd: usize, out: &mut [u8]) -> Result<usize, ProgramIoError> {
        if out.is_empty() {
            self.record(SyscallOp::FdRead, SyscallStatus::Ok);
            return Ok(0);
        }

        let existing = self.with_existing_active_fd_table_mut(|table| table.read_tty(fd, out));
        match existing {
            Some(Ok(Some((read, should_record)))) => {
                if should_record {
                    self.record(SyscallOp::FdRead, SyscallStatus::Ok);
                }
                return Ok(read);
            }
            Some(Ok(None)) => {}
            Some(Err(error)) => {
                self.record(SyscallOp::FdRead, SyscallStatus::Error);
                return Err(error);
            }
            None => {
                self.record(SyscallOp::FdRead, SyscallStatus::Error);
                return Err(ProgramIoError::BadFd);
            }
        }

        let mut line = [0u8; crate::rootd::ROOT_LINE_BYTES];
        let len = self.daemon.read_tty_line(&mut line);
        let status = if len == 0 {
            SyscallStatus::Unavailable
        } else {
            SyscallStatus::Ok
        };
        self.record(SyscallOp::TtyReadLine, status);

        let refilled = self.with_existing_active_fd_table_mut(|table| {
            table.refill_tty(fd, &line[..len])?;
            table.read_tty(fd, out)
        });
        match refilled {
            Some(Ok(Some((read, should_record)))) => {
                if should_record {
                    self.record(SyscallOp::FdRead, SyscallStatus::Ok);
                }
                Ok(read)
            }
            Some(Ok(None)) => {
                self.record(SyscallOp::FdRead, SyscallStatus::Error);
                Err(ProgramIoError::BadFd)
            }
            Some(Err(error)) => {
                self.record(SyscallOp::FdRead, SyscallStatus::Error);
                Err(error)
            }
            None => {
                self.record(SyscallOp::FdRead, SyscallStatus::Error);
                Err(ProgramIoError::BadFd)
            }
        }
    }

    /// Writes bytes to a Reovim program fd.
    ///
    /// Stdout writes remain untraced per fragment to keep the bounded syscall
    /// ring useful during verbose commands. Stderr and failed writes retain
    /// `fd-write` rows because they are operator-visible diagnostics.
    pub fn write_fd(&self, fd: usize, bytes: &[u8]) -> Result<usize, ProgramIoError> {
        self.ensure_no_retained_syscall_continuation_for_io(SyscallOp::FdWrite)?;
        if fd == self.stdio.stdout.fd {
            if let Some(capture) = self.vfs_render_capture {
                // SAFETY: the VFS file buffer lock is held by this syscall
                // handle during rendering.
                return Ok(unsafe { capture.as_ref() }.write(bytes));
            }
        }
        match self.active_fd_descriptor(fd) {
            Some(ProgramFdDescriptor::Stdio(ProgramStream::Stdout)) => {
                if let Some(capture) = self.active_stdout_capture() {
                    // SAFETY: the capture pointer is created from a live reference
                    // in the same synchronous dispatch frame as this syscall
                    // handle.
                    return Ok(unsafe { capture.as_ref() }.write(bytes));
                }
                self.daemon.write_bytes(bytes);
                Ok(bytes.len())
            }
            Some(ProgramFdDescriptor::Stdio(ProgramStream::Stderr)) => {
                if !bytes.is_empty() {
                    self.record(SyscallOp::FdWrite, SyscallStatus::Ok);
                }
                self.daemon.write_bytes(bytes);
                Ok(bytes.len())
            }
            Some(ProgramFdDescriptor::Stdio(ProgramStream::Stdin))
            | Some(ProgramFdDescriptor::Vfs(_)) => {
                self.record(SyscallOp::FdWrite, SyscallStatus::Error);
                Err(ProgramIoError::NotWritable)
            }
            Some(ProgramFdDescriptor::Tty(open_file_slot)) => {
                self.write_tty_fd(open_file_slot, bytes)
            }
            Some(ProgramFdDescriptor::Pipe {
                end: ProgramPipeEnd::Write,
                ..
            }) => self.write_pipe_fd(fd, bytes),
            Some(ProgramFdDescriptor::Pipe {
                end: ProgramPipeEnd::Read,
                ..
            }) => {
                self.record(SyscallOp::FdWrite, SyscallStatus::Error);
                Err(ProgramIoError::NotWritable)
            }
            Some(ProgramFdDescriptor::Closed) | None => {
                self.record(SyscallOp::FdWrite, SyscallStatus::Error);
                Err(ProgramIoError::BadFd)
            }
        }
    }

    /// Writes one line to a Reovim program fd.
    pub fn write_fd_line(&self, fd: usize, line: &str) -> Result<usize, ProgramIoError> {
        self.ensure_no_retained_syscall_continuation_for_io(SyscallOp::FdWrite)?;
        if fd == self.stdio.stdout.fd {
            if let Some(capture) = self.vfs_render_capture {
                // SAFETY: the VFS file buffer lock is held by this syscall
                // handle during rendering.
                let capture = unsafe { capture.as_ref() };
                let mut written = capture.write(line.as_bytes());
                written += capture.write(b"\n");
                return Ok(written);
            }
        }
        match self.active_fd_descriptor(fd) {
            Some(ProgramFdDescriptor::Stdio(ProgramStream::Stdout)) => {
                if let Some(capture) = self.active_stdout_capture() {
                    // SAFETY: same dispatch-frame lifetime as `write_fd`.
                    let capture = unsafe { capture.as_ref() };
                    let mut written = capture.write(line.as_bytes());
                    written += capture.write(b"\n");
                    return Ok(written);
                }
                self.daemon.write_line(line);
                Ok(line.len().saturating_add(1))
            }
            Some(ProgramFdDescriptor::Stdio(ProgramStream::Stderr)) => {
                self.record(SyscallOp::FdWrite, SyscallStatus::Ok);
                self.daemon.write_line(line);
                Ok(line.len().saturating_add(1))
            }
            Some(ProgramFdDescriptor::Stdio(ProgramStream::Stdin))
            | Some(ProgramFdDescriptor::Vfs(_)) => {
                self.record(SyscallOp::FdWrite, SyscallStatus::Error);
                Err(ProgramIoError::NotWritable)
            }
            Some(ProgramFdDescriptor::Tty(open_file_slot)) => {
                self.write_tty_fd_line(open_file_slot, line)
            }
            Some(ProgramFdDescriptor::Pipe {
                end: ProgramPipeEnd::Write,
                ..
            }) => {
                let mut written = self.write_pipe_fd(fd, line.as_bytes())?;
                written += self.write_pipe_fd(fd, b"\n")?;
                Ok(written)
            }
            Some(ProgramFdDescriptor::Pipe {
                end: ProgramPipeEnd::Read,
                ..
            }) => {
                self.record(SyscallOp::FdWrite, SyscallStatus::Error);
                Err(ProgramIoError::NotWritable)
            }
            Some(ProgramFdDescriptor::Closed) | None => {
                self.record(SyscallOp::FdWrite, SyscallStatus::Error);
                Err(ProgramIoError::BadFd)
            }
        }
    }

    fn record_raw_stdout_write_once(&mut self) {
        if !self.raw_stdout_write_recorded {
            self.raw_stdout_write_recorded = true;
            self.record(SyscallOp::FdWrite, SyscallStatus::Ok);
        }
    }

    /// Reads bytes from a Reovim program fd.
    ///
    /// Default root-shell launches receive an empty stdin buffer, so reading fd
    /// 0 returns `Ok(0)` EOF unless a pipe or scheduled exec invocation seeded
    /// a bounded initial stdin payload. Interactive TTY input is exposed by
    /// opening `/dev/tty` and reading that descriptor; fd 0 remains the current
    /// program's stdin stream.
    pub fn read_fd(&mut self, fd: usize, out: &mut [u8]) -> Result<usize, ProgramIoError> {
        self.ensure_no_retained_syscall_continuation_for_io(SyscallOp::FdRead)?;
        match self.active_fd_descriptor(fd) {
            Some(ProgramFdDescriptor::Stdio(ProgramStream::Stdin)) => {
                let Some(read) =
                    self.with_existing_active_fd_table_mut(|table| table.read_stdin(out))
                else {
                    self.record(SyscallOp::FdRead, SyscallStatus::Error);
                    return Err(ProgramIoError::BadFd);
                };
                self.record(SyscallOp::FdRead, SyscallStatus::Ok);
                return Ok(read);
            }
            Some(ProgramFdDescriptor::Stdio(ProgramStream::Stdout | ProgramStream::Stderr)) => {
                self.record(SyscallOp::FdRead, SyscallStatus::Error);
                return Err(ProgramIoError::NotReadable);
            }
            Some(ProgramFdDescriptor::Vfs(_)) => {}
            Some(ProgramFdDescriptor::Tty(_)) => return self.read_tty_fd(fd, out),
            Some(ProgramFdDescriptor::Pipe {
                pipe_slot,
                end: ProgramPipeEnd::Read,
            }) => {
                let table_read =
                    self.with_existing_active_fd_table_mut(|table| table.read_pipe(fd, out));
                match table_read {
                    Some(Ok((read, should_record))) => {
                        if should_record {
                            self.record(SyscallOp::FdRead, SyscallStatus::Ok);
                        }
                        if read > 0 {
                            let mut write_waiters = [0usize; MAX_PROGRAM_PIPE_WAITERS];
                            if let Ok(waiter_count) =
                                take_program_pipe_write_waiters(pipe_slot, &mut write_waiters)
                            {
                                let _ = wake_pipe_write_waiters(&write_waiters[..waiter_count]);
                            }
                        }
                        return Ok(read);
                    }
                    Some(Err(error)) => {
                        if error == ProgramIoError::Busy {
                            if let Some(ctx) = self.current {
                                if let Err(error) =
                                    record_program_pipe_read_waiter(pipe_slot, ctx.pid)
                                {
                                    record_context(
                                        ctx,
                                        SyscallOp::ProcessBlock,
                                        SyscallStatus::Error,
                                    );
                                    self.record(SyscallOp::FdRead, SyscallStatus::Error);
                                    return Err(error);
                                } else if let Some(blocked) = proc::block_pipe_read_process(ctx.pid)
                                {
                                    let blocked_ctx = SyscallContext::from_process(blocked);
                                    record_context(
                                        blocked_ctx,
                                        SyscallOp::ProcessBlock,
                                        SyscallStatus::Ok,
                                    );
                                    append_process_control_event(
                                        blocked_ctx,
                                        SyscallOp::ProcessBlock,
                                    );
                                } else {
                                    record_context(
                                        ctx,
                                        SyscallOp::ProcessBlock,
                                        SyscallStatus::Error,
                                    );
                                    release_program_pipe_read_waiters(ctx.pid);
                                    self.record(SyscallOp::FdRead, SyscallStatus::Error);
                                    return Err(ProgramIoError::NoSpace);
                                }
                            }
                        }
                        self.record(SyscallOp::FdRead, SyscallStatus::Error);
                        return Err(error);
                    }
                    None => {
                        self.record(SyscallOp::FdRead, SyscallStatus::Error);
                        return Err(ProgramIoError::BadFd);
                    }
                }
            }
            Some(ProgramFdDescriptor::Pipe {
                end: ProgramPipeEnd::Write,
                ..
            }) => {
                self.record(SyscallOp::FdRead, SyscallStatus::Error);
                return Err(ProgramIoError::NotReadable);
            }
            Some(ProgramFdDescriptor::Closed) | None => {
                self.record(SyscallOp::FdRead, SyscallStatus::Error);
                return Err(ProgramIoError::BadFd);
            }
        }
        let table_read = self.with_existing_active_fd_table_mut(|table| {
            table
                .open_file_slot_for_fd(fd)
                .map(|slot| table.read(slot, out))
        });
        match table_read {
            Some(Some(Ok((read, should_record)))) => {
                if should_record {
                    self.record(SyscallOp::FdRead, SyscallStatus::Ok);
                }
                return Ok(read);
            }
            Some(Some(Err(error))) => {
                self.record(SyscallOp::FdRead, SyscallStatus::Error);
                return Err(error);
            }
            Some(None) | None => {}
        }

        self.record(SyscallOp::FdRead, SyscallStatus::Error);
        Err(ProgramIoError::BadFd)
    }

    /// Reads directory entry bytes from an opened Reovim directory fd.
    pub fn read_directory_fd(
        &mut self,
        fd: usize,
        out: &mut [u8],
    ) -> Result<usize, ProgramIoError> {
        self.ensure_no_retained_syscall_continuation_for_io(SyscallOp::VfsList)?;
        match self.active_fd_descriptor(fd) {
            Some(ProgramFdDescriptor::Vfs(_)) => {}
            Some(ProgramFdDescriptor::Stdio(_)) => {
                self.record(SyscallOp::VfsList, SyscallStatus::Error);
                return Err(ProgramIoError::NotDirectory);
            }
            Some(ProgramFdDescriptor::Tty(_)) | Some(ProgramFdDescriptor::Pipe { .. }) => {
                self.record(SyscallOp::VfsList, SyscallStatus::Error);
                return Err(ProgramIoError::NotDirectory);
            }
            Some(ProgramFdDescriptor::Closed) | None => {
                self.record(SyscallOp::VfsList, SyscallStatus::Error);
                return Err(ProgramIoError::BadFd);
            }
        }
        let table_read = self.with_existing_active_fd_table_mut(|table| {
            table
                .open_file_slot_for_fd(fd)
                .map(|slot| table.read_directory(slot, out))
        });
        match table_read {
            Some(Some(Ok((read, should_record)))) => {
                if should_record {
                    self.record(SyscallOp::VfsList, SyscallStatus::Ok);
                }
                Ok(read)
            }
            Some(Some(Err(error))) => {
                self.record(SyscallOp::VfsList, SyscallStatus::Error);
                Err(error)
            }
            Some(None) | None => {
                self.record(SyscallOp::VfsList, SyscallStatus::Error);
                Err(ProgramIoError::BadFd)
            }
        }
    }

    /// Seeks a Reovim program fd and returns the new opened-object offset.
    pub fn seek_fd(
        &mut self,
        fd: usize,
        offset: isize,
        whence: SeekWhence,
    ) -> Result<usize, ProgramIoError> {
        self.ensure_no_retained_syscall_continuation_for_io(SyscallOp::FdSeek)?;
        match self.active_fd_descriptor(fd) {
            Some(ProgramFdDescriptor::Stdio(_)) => {
                self.record(SyscallOp::FdSeek, SyscallStatus::Error);
                return Err(ProgramIoError::NotSeekable);
            }
            Some(ProgramFdDescriptor::Vfs(_)) => {}
            Some(ProgramFdDescriptor::Tty(_)) | Some(ProgramFdDescriptor::Pipe { .. }) => {
                self.record(SyscallOp::FdSeek, SyscallStatus::Error);
                return Err(ProgramIoError::NotSeekable);
            }
            Some(ProgramFdDescriptor::Closed) | None => {
                self.record(SyscallOp::FdSeek, SyscallStatus::Error);
                return Err(ProgramIoError::BadFd);
            }
        }
        let table_seek = self.with_existing_active_fd_table_mut(|table| {
            table
                .open_file_slot_for_fd(fd)
                .map(|slot| table.seek(slot, offset, whence))
        });
        match table_seek {
            Some(Some(Ok(offset))) => {
                self.record(SyscallOp::FdSeek, SyscallStatus::Ok);
                return Ok(offset);
            }
            Some(Some(Err(error))) => {
                self.record(SyscallOp::FdSeek, SyscallStatus::Error);
                return Err(error);
            }
            Some(None) | None => {}
        }

        self.record(SyscallOp::FdSeek, SyscallStatus::Error);
        Err(ProgramIoError::BadFd)
    }

    /// Closes an open Reovim program fd.
    pub fn close_fd(&mut self, fd: usize) -> Result<(), ProgramIoError> {
        self.ensure_no_retained_syscall_continuation_for_io(SyscallOp::FdClose)?;
        let table_close = self.with_existing_active_fd_table_mut(|table| table.close_fd(fd));
        match table_close {
            Some(Ok(())) => {
                self.record(SyscallOp::FdClose, SyscallStatus::Ok);
                return Ok(());
            }
            Some(Err(error)) => {
                self.record(SyscallOp::FdClose, SyscallStatus::Error);
                return Err(error);
            }
            None => {}
        }

        self.record(SyscallOp::FdClose, SyscallStatus::Error);
        Err(ProgramIoError::BadFd)
    }

    /// Duplicates a Reovim program fd.
    ///
    /// The duplicate descriptor shares the same open-file description and
    /// cursor. This is the current bounded equivalent of the fd-table behavior
    /// needed later by process-owned tables and fork/exec inheritance.
    pub fn duplicate_fd(&mut self, fd: usize) -> Result<usize, ProgramIoError> {
        self.ensure_no_retained_syscall_continuation_for_io(SyscallOp::FdDuplicate)?;
        let table_duplicate =
            self.with_existing_active_fd_table_mut(|table| table.duplicate_fd(fd));
        match table_duplicate {
            Some(Ok(new_fd)) => {
                self.record(SyscallOp::FdDuplicate, SyscallStatus::Ok);
                return Ok(new_fd);
            }
            Some(Err(error)) => {
                self.record(SyscallOp::FdDuplicate, SyscallStatus::Error);
                return Err(error);
            }
            None => {}
        }

        self.record(SyscallOp::FdDuplicate, SyscallStatus::Error);
        Err(ProgramIoError::BadFd)
    }

    /// Duplicates a Reovim program fd into the requested target fd.
    ///
    /// The target descriptor is closed first when open, and the replacement
    /// descriptor shares the source open-file description. Descriptor-local
    /// close-on-exec state is cleared on the target descriptor.
    pub fn duplicate_fd_to(
        &mut self,
        old_fd: usize,
        new_fd: usize,
    ) -> Result<usize, ProgramIoError> {
        self.ensure_no_retained_syscall_continuation_for_io(SyscallOp::FdDuplicate)?;
        let table_duplicate =
            self.with_existing_active_fd_table_mut(|table| table.duplicate_fd_to(old_fd, new_fd));
        match table_duplicate {
            Some(Ok(new_fd)) => {
                self.record(SyscallOp::FdDuplicate, SyscallStatus::Ok);
                return Ok(new_fd);
            }
            Some(Err(error)) => {
                self.record(SyscallOp::FdDuplicate, SyscallStatus::Error);
                return Err(error);
            }
            None => {}
        }

        self.record(SyscallOp::FdDuplicate, SyscallStatus::Error);
        Err(ProgramIoError::BadFd)
    }

    /// Reads descriptor-local flags for a Reovim program fd.
    pub fn descriptor_flags(&mut self, fd: usize) -> Result<DescriptorFlags, ProgramIoError> {
        self.ensure_no_retained_syscall_continuation_for_io(SyscallOp::FdFlagsGet)?;
        let table_flags =
            self.with_existing_active_fd_table_mut(|table| table.descriptor_flags_for_fd(fd));
        match table_flags {
            Some(Ok(flags)) => {
                self.record(SyscallOp::FdFlagsGet, SyscallStatus::Ok);
                return Ok(flags);
            }
            Some(Err(error)) => {
                self.record(SyscallOp::FdFlagsGet, SyscallStatus::Error);
                return Err(error);
            }
            None => {}
        }

        self.record(SyscallOp::FdFlagsGet, SyscallStatus::Error);
        Err(ProgramIoError::BadFd)
    }

    /// Replaces descriptor-local flags for a Reovim program fd.
    pub fn set_descriptor_flags(
        &mut self,
        fd: usize,
        flags: DescriptorFlags,
    ) -> Result<(), ProgramIoError> {
        self.ensure_no_retained_syscall_continuation_for_io(SyscallOp::FdFlagsSet)?;
        let table_set = self.with_existing_active_fd_table_mut(|table| {
            table.set_descriptor_flags_for_fd(fd, flags)
        });
        match table_set {
            Some(Ok(())) => {
                self.record(SyscallOp::FdFlagsSet, SyscallStatus::Ok);
                return Ok(());
            }
            Some(Err(error)) => {
                self.record(SyscallOp::FdFlagsSet, SyscallStatus::Error);
                return Err(error);
            }
            None => {}
        }

        self.record(SyscallOp::FdFlagsSet, SyscallStatus::Error);
        Err(ProgramIoError::BadFd)
    }

    /// Reads opened-object status flags for a Reovim program fd.
    pub fn status_flags(&mut self, fd: usize) -> Result<FileStatusFlags, ProgramIoError> {
        self.ensure_no_retained_syscall_continuation_for_io(SyscallOp::FdStatusGet)?;
        let table_flags =
            self.with_existing_active_fd_table_mut(|table| table.status_flags_for_fd(fd));
        match table_flags {
            Some(Ok(flags)) => {
                self.record(SyscallOp::FdStatusGet, SyscallStatus::Ok);
                return Ok(flags);
            }
            Some(Err(error)) => {
                self.record(SyscallOp::FdStatusGet, SyscallStatus::Error);
                return Err(error);
            }
            None => {}
        }

        self.record(SyscallOp::FdStatusGet, SyscallStatus::Error);
        Err(ProgramIoError::BadFd)
    }

    /// Replaces opened-object status flags for a Reovim program fd.
    pub fn set_status_flags(
        &mut self,
        fd: usize,
        flags: FileStatusFlags,
    ) -> Result<(), ProgramIoError> {
        self.ensure_no_retained_syscall_continuation_for_io(SyscallOp::FdStatusSet)?;
        let table_set = self
            .with_existing_active_fd_table_mut(|table| table.set_status_flags_for_fd(fd, flags));
        match table_set {
            Some(Ok(())) => {
                self.record(SyscallOp::FdStatusSet, SyscallStatus::Ok);
                return Ok(());
            }
            Some(Err(error)) => {
                self.record(SyscallOp::FdStatusSet, SyscallStatus::Error);
                return Err(error);
            }
            None => {}
        }

        self.record(SyscallOp::FdStatusSet, SyscallStatus::Error);
        Err(ProgramIoError::BadFd)
    }

    /// Creates a bounded pipe descriptor pair for the current process.
    ///
    /// The returned tuple is `(read_fd, write_fd)`. The pipe is a transitional
    /// nonblocking byte buffer backed by the process fd table; it proves
    /// endpoint lifetime and fd-shaped pipe I/O before the shell pipeline path
    /// is moved off stdout-capture seeding.
    pub fn pipe_fds(&mut self) -> Result<(usize, usize), ProgramIoError> {
        if self.has_invalidated_process_context() {
            self.record(SyscallOp::FdPipe, SyscallStatus::Error);
            return Err(ProgramIoError::Busy);
        }
        if self.current.is_none() {
            self.record(SyscallOp::FdPipe, SyscallStatus::Unavailable);
            return Err(ProgramIoError::BadFd);
        }
        self.ensure_no_retained_syscall_continuation_for_io(SyscallOp::FdPipe)?;
        let table_pipe = self.with_active_fd_table_mut(|table| table.create_pipe());
        match table_pipe {
            Ok(Ok(fds)) => {
                self.record(SyscallOp::FdPipe, SyscallStatus::Ok);
                Ok(fds)
            }
            Ok(Err(error)) | Err(error) => {
                self.record(SyscallOp::FdPipe, SyscallStatus::Error);
                Err(error)
            }
        }
    }

    /// Reads one interactive TTY line through the root-daemon line callback.
    pub fn read_tty_line(&mut self, out: &mut [u8]) -> usize {
        if !self.direct_backend_session_admitted(SyscallOp::TtyReadLine) {
            return 0;
        }
        let read = self.daemon.read_tty_line(out);
        let status = if read == 0 {
            SyscallStatus::Unavailable
        } else {
            SyscallStatus::Ok
        };
        self.record(SyscallOp::TtyReadLine, status);
        read
    }

    /// Returns the current shell session cwd.
    #[must_use]
    pub fn cwd(&self) -> &str {
        if !self.direct_backend_session_admitted(SyscallOp::SessionCwdGet) {
            return "";
        }
        self.record(SyscallOp::SessionCwdGet, SyscallStatus::Ok);
        self.session.cwd()
    }

    /// Sets the current shell session cwd.
    pub fn set_cwd(&mut self, path: PathBuf) {
        if !self.direct_backend_session_admitted(SyscallOp::SessionCwdSet) {
            return;
        }
        self.session.set_cwd(path);
        self.record(SyscallOp::SessionCwdSet, SyscallStatus::Ok);
    }

    /// Requests the `/bin` path used for the shell/session target.
    pub fn request_shell_target(&mut self, program: &'static str) {
        if !self.direct_backend_session_admitted(SyscallOp::SessionShellTarget) {
            return;
        }
        self.session.request_shell_target(program);
        self.record(SyscallOp::SessionShellTarget, SyscallStatus::Ok);
        if let Some(ctx) = self.current {
            klog::append_bytes(b"syscall path=");
            klog::append_bytes(ctx.program_path.as_bytes());
            klog::append_bytes(b" op=");
            klog::append_bytes(SyscallOp::SessionShellTarget.as_str().as_bytes());
            klog::append_bytes(b" status=ok target=");
            klog::append_bytes(program.as_bytes());
            klog::append_bytes(b" loader=");
            klog::append_bytes(ctx.loader.as_bytes());
            klog::append_bytes(b" entry_fn=");
            klog::append_bytes(ctx.entry_name.as_bytes());
            klog::append_bytes(b"\n");
            klog::append_event_with_source_context(
                "process",
                "syscall",
                "info",
                "session-shell-target",
                ctx.pid,
                ctx.task_id,
            );
        }
    }

    /// Requests a named init service target.
    ///
    /// The first boot service is `shell -> /bin/sh`; rootd still owns the
    /// privileged line loop, but init now records the service target through a
    /// typed syscall before rootd starts it.
    pub fn request_init_service(
        &mut self,
        name: &'static str,
        target: &'static str,
    ) -> Result<ServiceRecord, service::ServiceError> {
        let ctx = match self.require_direct_current_context(SyscallOp::InitServiceStart) {
            Ok(ctx) => ctx,
            Err(DirectCurrentAdmissionError::Unavailable) => {
                return Err(service::ServiceError::Unavailable);
            }
            Err(DirectCurrentAdmissionError::Busy(_)) => return Err(service::ServiceError::Busy),
        };
        match service::request_service(ctx.pid, ctx.task_id, name, target) {
            Ok(record) => {
                if name == "shell" {
                    self.session.request_shell_target(target);
                }
                record_context(ctx, SyscallOp::InitServiceStart, SyscallStatus::Ok);
                klog::append_bytes(b"syscall path=");
                klog::append_bytes(ctx.program_path.as_bytes());
                klog::append_bytes(b" op=");
                klog::append_bytes(SyscallOp::InitServiceStart.as_str().as_bytes());
                klog::append_bytes(b" status=ok service=");
                klog::append_bytes(name.as_bytes());
                klog::append_bytes(b" target=");
                klog::append_bytes(target.as_bytes());
                klog::append_bytes(b" loader=");
                klog::append_bytes(ctx.loader.as_bytes());
                klog::append_bytes(b" entry_fn=");
                klog::append_bytes(ctx.entry_name.as_bytes());
                klog::append_bytes(b"\n");
                klog::append_event_with_source_context(
                    "process",
                    "syscall",
                    "info",
                    "init-service-start",
                    ctx.pid,
                    ctx.task_id,
                );
                Ok(record)
            }
            Err(error) => {
                record_context(ctx, SyscallOp::InitServiceStart, SyscallStatus::Error);
                Err(error)
            }
        }
    }

    /// Publishes the current process as the running instance of a service.
    pub fn service_ready(
        &mut self,
        name: &'static str,
    ) -> Result<ServiceRecord, service::ServiceError> {
        let ctx = match self.require_direct_current_context(SyscallOp::ServiceReady) {
            Ok(ctx) => ctx,
            Err(DirectCurrentAdmissionError::Unavailable) => {
                return Err(service::ServiceError::Unavailable);
            }
            Err(DirectCurrentAdmissionError::Busy(_)) => return Err(service::ServiceError::Busy),
        };

        match service::request_service(ctx.pid, ctx.task_id, name, ctx.program_path) {
            Ok(_) => {
                let Some(record) = service::mark_started(name, ctx.pid, ctx.task_id) else {
                    record_context(ctx, SyscallOp::ServiceReady, SyscallStatus::Error);
                    return Err(service::ServiceError::Unavailable);
                };
                record_context(ctx, SyscallOp::ServiceReady, SyscallStatus::Ok);
                klog::append_bytes(b"syscall path=");
                klog::append_bytes(ctx.program_path.as_bytes());
                klog::append_bytes(b" op=");
                klog::append_bytes(SyscallOp::ServiceReady.as_str().as_bytes());
                klog::append_bytes(b" status=ok service=");
                klog::append_bytes(name.as_bytes());
                klog::append_bytes(b" target=");
                klog::append_bytes(ctx.program_path.as_bytes());
                klog::append_bytes(b" service_pid=");
                klog::append_usize_dec(record.service_pid);
                klog::append_bytes(b" service_task=");
                klog::append_usize_dec(record.service_task_id);
                klog::append_bytes(b" loader=");
                klog::append_bytes(ctx.loader.as_bytes());
                klog::append_bytes(b" entry_fn=");
                klog::append_bytes(ctx.entry_name.as_bytes());
                klog::append_bytes(b"\n");
                klog::append_event_with_source_context(
                    "process",
                    "syscall",
                    "info",
                    "service-ready",
                    ctx.pid,
                    ctx.task_id,
                );
                Ok(record)
            }
            Err(error) => {
                record_context(ctx, SyscallOp::ServiceReady, SyscallStatus::Error);
                Err(error)
            }
        }
    }

    /// Blocks the current process as a resident service after readiness was published.
    pub fn service_hold(&mut self) -> Result<ProcessHandle, ProgramProcessError> {
        let ctx = match self.require_direct_current_context(SyscallOp::ServiceHold) {
            Ok(ctx) => ctx,
            Err(DirectCurrentAdmissionError::Unavailable) => {
                return Err(ProgramProcessError::NoCurrentProcess);
            }
            Err(DirectCurrentAdmissionError::Busy(_)) => return Err(ProgramProcessError::Busy),
        };

        let Some(blocked) = proc::block_service_process(ctx.pid) else {
            record_context(ctx, SyscallOp::ServiceHold, SyscallStatus::Error);
            return Err(ProgramProcessError::WaitFailed);
        };

        let blocked_ctx = SyscallContext::from_process(blocked);
        record_context(blocked_ctx, SyscallOp::ServiceHold, SyscallStatus::Ok);
        record_context(blocked_ctx, SyscallOp::ProcessBlock, SyscallStatus::Ok);
        klog::append_bytes(b"syscall path=");
        klog::append_bytes(blocked.program_path.as_bytes());
        klog::append_bytes(b" op=");
        klog::append_bytes(SyscallOp::ServiceHold.as_str().as_bytes());
        klog::append_bytes(b" status=ok pid=");
        klog::append_usize_dec(blocked.pid);
        klog::append_bytes(b" task=");
        klog::append_usize_dec(blocked.task_id);
        klog::append_bytes(b" block=service loader=");
        klog::append_bytes(blocked.loader.as_bytes());
        klog::append_bytes(b" entry_fn=");
        klog::append_bytes(blocked.entry_name.as_bytes());
        klog::append_bytes(b"\n");
        append_process_control_event(blocked_ctx, SyscallOp::ServiceHold);
        Ok(blocked)
    }

    /// Requests startup of the interactive root shell/session loop.
    pub fn request_shell_start(&mut self) {
        if !self.direct_backend_session_admitted(SyscallOp::SessionShellStart) {
            return;
        }
        self.session.request_shell_start();
        self.record(SyscallOp::SessionShellStart, SyscallStatus::Ok);
        if let Some(ctx) = self.current {
            klog::append_bytes(b"syscall path=");
            klog::append_bytes(ctx.program_path.as_bytes());
            klog::append_bytes(b" op=");
            klog::append_bytes(SyscallOp::SessionShellStart.as_str().as_bytes());
            klog::append_bytes(b" status=ok loader=");
            klog::append_bytes(ctx.loader.as_bytes());
            klog::append_bytes(b" entry_fn=");
            klog::append_bytes(ctx.entry_name.as_bytes());
            klog::append_bytes(b"\n");
            klog::append_event_with_source_context(
                "process",
                "syscall",
                "info",
                "session-shell-start",
                ctx.pid,
                ctx.task_id,
            );
        }
    }

    /// Requests the command-line discipline used by the interactive shell loop.
    pub fn request_shell_line_discipline(
        &mut self,
        line_discipline: &'static str,
        pipe_mode: &'static str,
    ) {
        if !self.direct_backend_session_admitted(SyscallOp::SessionLineDiscipline) {
            return;
        }
        self.session
            .request_line_discipline(line_discipline, pipe_mode);
        self.record(SyscallOp::SessionLineDiscipline, SyscallStatus::Ok);
        if let Some(ctx) = self.current {
            klog::append_bytes(b"syscall path=");
            klog::append_bytes(ctx.program_path.as_bytes());
            klog::append_bytes(b" op=");
            klog::append_bytes(SyscallOp::SessionLineDiscipline.as_str().as_bytes());
            klog::append_bytes(b" status=ok line_discipline=");
            klog::append_bytes(line_discipline.as_bytes());
            klog::append_bytes(b" pipe_mode=");
            klog::append_bytes(pipe_mode.as_bytes());
            klog::append_bytes(b" loader=");
            klog::append_bytes(ctx.loader.as_bytes());
            klog::append_bytes(b" entry_fn=");
            klog::append_bytes(ctx.entry_name.as_bytes());
            klog::append_bytes(b"\n");
            klog::append_event_with_source_context(
                "process",
                "syscall",
                "info",
                "session-line-discipline",
                ctx.pid,
                ctx.task_id,
            );
        }
    }

    /// Normalizes a path against the current shell session cwd.
    pub fn normalize_path(&self, path: &str) -> Result<PathBuf, VfsError> {
        self.ensure_direct_backend_vfs_admitted(SyscallOp::VfsNormalize)?;
        self.normalize_path_from_base(self.session_cwd_path(), path)
    }

    fn normalize_path_from_base(&self, base: PathBuf, path: &str) -> Result<PathBuf, VfsError> {
        let result = vfs::normalize(base.as_str(), path);
        self.record_result(SyscallOp::VfsNormalize, result.is_ok());
        result
    }

    /// Looks up a normalized path in the kernel VFS.
    pub fn lookup_path(&self, path: &str) -> Result<Node, VfsError> {
        self.ensure_direct_backend_vfs_admitted(SyscallOp::VfsLookup)?;
        let result = vfs::lookup(path, self.daemon.devices(), self.daemon.programs());
        self.record_result(SyscallOp::VfsLookup, result.is_ok());
        result
    }

    /// Writes a kernel VFS directory listing, or a file basename, to stdout.
    pub fn write_vfs_listing(&self, target: &str) -> Result<(), VfsError> {
        let path = match self.normalize_path(target) {
            Ok(path) => path,
            Err(error) => {
                self.record(SyscallOp::VfsList, SyscallStatus::Error);
                return Err(error);
            }
        };
        match self.lookup_path(path.as_str()) {
            Ok(Node::Directory(directory)) => {
                self.record(SyscallOp::VfsList, SyscallStatus::Ok);
                self.write_directory(directory);
                Ok(())
            }
            Ok(Node::File(_)) => {
                self.record(SyscallOp::VfsList, SyscallStatus::Ok);
                self.stdout_line(Self::path_basename(path.as_str()));
                Ok(())
            }
            Err(error) => {
                self.record(SyscallOp::VfsList, SyscallStatus::Error);
                Err(error)
            }
        }
    }

    /// Opens a kernel VFS pseudo-file for descriptor-shaped reads.
    pub fn open_vfs_file_path(&mut self, target: &str) -> Result<usize, VfsError> {
        self.open_vfs_file_path_with_descriptor_flags(target, ProgramFdDescriptorFlags::empty())
    }

    fn open_vfs_file_path_with_descriptor_flags(
        &mut self,
        target: &str,
        descriptor_flags: ProgramFdDescriptorFlags,
    ) -> Result<usize, VfsError> {
        self.open_vfs_file_path_at_with_access_and_descriptor_flags(
            self.session_cwd_path(),
            target,
            ProgramOpenAccess::ReadOnly,
            descriptor_flags,
        )
    }

    fn open_vfs_file_path_at_with_access_and_descriptor_flags(
        &mut self,
        base: PathBuf,
        target: &str,
        access: ProgramOpenAccess,
        descriptor_flags: ProgramFdDescriptorFlags,
    ) -> Result<usize, VfsError> {
        self.ensure_direct_backend_vfs_admitted(SyscallOp::VfsOpen)?;
        let path = match self.normalize_path_from_base(base, target) {
            Ok(path) => path,
            Err(error) => {
                self.record(SyscallOp::VfsOpen, SyscallStatus::Error);
                return Err(error);
            }
        };
        match self.lookup_path(path.as_str()) {
            Ok(Node::File(file)) => {
                if file == vfs::File::DevTty {
                    let open_file_slot = match install_program_tty_description(path, access) {
                        Ok(slot) => slot,
                        Err(_) => {
                            self.record(SyscallOp::VfsOpen, SyscallStatus::Error);
                            return Err(VfsError::Busy);
                        }
                    };
                    let install = self.with_active_fd_table_mut(|table| {
                        let descriptor_slot = table.free_descriptor_slot()?;
                        table.install_tty_descriptor(
                            descriptor_slot,
                            open_file_slot,
                            descriptor_flags,
                        );
                        Some(program_fd_for_descriptor_slot(descriptor_slot))
                    });
                    let fd = match install {
                        Ok(Some(fd)) => fd,
                        Ok(None) | Err(_) => {
                            let _ = release_program_open_file(
                                open_file_slot,
                                ProgramOpenFileDescriptionKind::Tty,
                            );
                            self.record(SyscallOp::VfsOpen, SyscallStatus::Error);
                            return Err(VfsError::Busy);
                        }
                    };
                    self.record(SyscallOp::VfsOpen, SyscallStatus::Ok);
                    return Ok(fd);
                }
                if access.writable() {
                    self.record(SyscallOp::VfsOpen, SyscallStatus::Error);
                    return Err(VfsError::NotWritable);
                }
                let Some((buffer_slot, buffer)) = acquire_program_vfs_file_buffer() else {
                    self.record(SyscallOp::VfsOpen, SyscallStatus::Error);
                    return Err(VfsError::Busy);
                };
                // SAFETY: acquiring the buffer lock grants exclusive access.
                unsafe { buffer.as_ref() }.reset();
                self.vfs_render_capture = Some(buffer);
                (self.daemon.vfs_file_writer())(file, self);
                self.vfs_render_capture = None;
                // SAFETY: the buffer remains locked and owned by this handle.
                let buffer_ref = unsafe { buffer.as_ref() };
                if buffer_ref.truncated() {
                    release_program_vfs_file_buffer(buffer_slot);
                    self.record(SyscallOp::VfsOpen, SyscallStatus::Error);
                    self.record(SyscallOp::VfsRead, SyscallStatus::Error);
                    return Err(VfsError::FileTooLarge);
                }
                let len = buffer_ref.len();
                let open_file_slot = match install_program_vfs_description(
                    ProgramVfsOpenKind::File,
                    path,
                    buffer_slot,
                    buffer,
                    len,
                ) {
                    Ok(slot) => slot,
                    Err(_) => {
                        release_program_vfs_file_buffer(buffer_slot);
                        self.record(SyscallOp::VfsOpen, SyscallStatus::Error);
                        return Err(VfsError::Busy);
                    }
                };
                let install = self.with_active_fd_table_mut(|table| {
                    let descriptor_slot = table.free_descriptor_slot()?;
                    table.install_vfs_descriptor(descriptor_slot, open_file_slot, descriptor_flags);
                    Some(program_fd_for_descriptor_slot(descriptor_slot))
                });
                let fd = match install {
                    Ok(Some(fd)) => fd,
                    Ok(None) | Err(_) => {
                        let _ = release_program_open_file(
                            open_file_slot,
                            ProgramOpenFileDescriptionKind::Vfs,
                        );
                        self.record(SyscallOp::VfsOpen, SyscallStatus::Error);
                        return Err(VfsError::Busy);
                    }
                };
                self.record(SyscallOp::VfsOpen, SyscallStatus::Ok);
                self.record(SyscallOp::VfsRead, SyscallStatus::Ok);
                Ok(fd)
            }
            Ok(Node::Directory(_)) => {
                self.record(SyscallOp::VfsOpen, SyscallStatus::Error);
                Err(VfsError::NotDirectory)
            }
            Err(error) => {
                self.record(SyscallOp::VfsOpen, SyscallStatus::Error);
                Err(error)
            }
        }
    }

    /// Opens a kernel VFS pseudo-directory for descriptor-shaped iteration.
    pub fn open_vfs_directory_path(&mut self, target: &str) -> Result<usize, VfsError> {
        self.open_vfs_directory_path_with_descriptor_flags(
            target,
            ProgramFdDescriptorFlags::empty(),
        )
    }

    fn open_vfs_directory_path_with_descriptor_flags(
        &mut self,
        target: &str,
        descriptor_flags: ProgramFdDescriptorFlags,
    ) -> Result<usize, VfsError> {
        self.open_vfs_directory_path_at_with_descriptor_flags(
            self.session_cwd_path(),
            target,
            descriptor_flags,
        )
    }

    fn open_vfs_directory_path_at_with_descriptor_flags(
        &mut self,
        base: PathBuf,
        target: &str,
        descriptor_flags: ProgramFdDescriptorFlags,
    ) -> Result<usize, VfsError> {
        self.ensure_direct_backend_vfs_admitted(SyscallOp::VfsOpen)?;
        let path = match self.normalize_path_from_base(base, target) {
            Ok(path) => path,
            Err(error) => {
                self.record(SyscallOp::VfsOpen, SyscallStatus::Error);
                return Err(error);
            }
        };
        match self.lookup_path(path.as_str()) {
            Ok(Node::Directory(directory)) => {
                let Some((buffer_slot, buffer)) = acquire_program_vfs_file_buffer() else {
                    self.record(SyscallOp::VfsOpen, SyscallStatus::Error);
                    return Err(VfsError::Busy);
                };
                // SAFETY: acquiring the buffer lock grants exclusive access.
                unsafe { buffer.as_ref() }.reset();
                self.vfs_render_capture = Some(buffer);
                self.write_directory(directory);
                self.vfs_render_capture = None;
                // SAFETY: the buffer remains locked and owned by this handle.
                let buffer_ref = unsafe { buffer.as_ref() };
                if buffer_ref.truncated() {
                    release_program_vfs_file_buffer(buffer_slot);
                    self.record(SyscallOp::VfsOpen, SyscallStatus::Error);
                    self.record(SyscallOp::VfsList, SyscallStatus::Error);
                    return Err(VfsError::FileTooLarge);
                }
                let len = buffer_ref.len();
                let open_file_slot = match install_program_vfs_description(
                    ProgramVfsOpenKind::Directory,
                    path,
                    buffer_slot,
                    buffer,
                    len,
                ) {
                    Ok(slot) => slot,
                    Err(_) => {
                        release_program_vfs_file_buffer(buffer_slot);
                        self.record(SyscallOp::VfsOpen, SyscallStatus::Error);
                        return Err(VfsError::Busy);
                    }
                };
                let install = self.with_active_fd_table_mut(|table| {
                    let descriptor_slot = table.free_descriptor_slot()?;
                    table.install_vfs_descriptor(descriptor_slot, open_file_slot, descriptor_flags);
                    Some(program_fd_for_descriptor_slot(descriptor_slot))
                });
                let fd = match install {
                    Ok(Some(fd)) => fd,
                    Ok(None) | Err(_) => {
                        let _ = release_program_open_file(
                            open_file_slot,
                            ProgramOpenFileDescriptionKind::Vfs,
                        );
                        self.record(SyscallOp::VfsOpen, SyscallStatus::Error);
                        return Err(VfsError::Busy);
                    }
                };
                self.record(SyscallOp::VfsOpen, SyscallStatus::Ok);
                Ok(fd)
            }
            Ok(Node::File(_)) => {
                self.record(SyscallOp::VfsOpen, SyscallStatus::Error);
                Err(VfsError::NotDirectory)
            }
            Err(error) => {
                self.record(SyscallOp::VfsOpen, SyscallStatus::Error);
                Err(error)
            }
        }
    }

    /// Reads a kernel VFS pseudo-file and writes its content to stdout.
    pub fn write_vfs_file_path(&mut self, target: &str) -> Result<(), VfsError> {
        let fd = self.open_vfs_file_path(target)?;
        let mut buffer = [0u8; 64];
        loop {
            let read = match self.read_fd(fd, &mut buffer) {
                Ok(read) => read,
                Err(_) => {
                    let _ = self.close_fd(fd);
                    return Err(VfsError::Io);
                }
            };
            if read == 0 {
                let _ = self.close_fd(fd);
                return Ok(());
            }
            if self.write_fd(self.stdout().fd, &buffer[..read]).is_err() {
                let _ = self.close_fd(fd);
                return Err(VfsError::Io);
            }
        }
    }

    /// Writes image-owned `/bin/help` output to stdout/stderr.
    pub fn write_program_help(&mut self, program_name: Option<&str>) -> ProgramStatus {
        if !self.direct_backend_report_admitted(SyscallOp::ProgramHelp) {
            return ProgramStatus::Blocked;
        }
        let writer = self.daemon.program_help_writer();
        let status = writer(program_name, self);
        self.record_result(SyscallOp::ProgramHelp, matches!(status, ProgramStatus::Ok));
        status
    }

    /// Writes `/bin/<program>` metadata to stdout.
    pub fn write_program_file_metadata(&self, id: usize) {
        if !self.direct_backend_report_admitted(SyscallOp::ProgramHelp) {
            return;
        }
        let Some((_, entry)) = program::find_by_id(self.programs(), id) else {
            self.record(SyscallOp::ProgramHelp, SyscallStatus::Error);
            return;
        };
        self.stdout_bytes(b"program=");
        self.stdout_bytes(entry.name.as_bytes());
        self.stdout_bytes(b"\npath=");
        self.stdout_bytes(entry.path.as_bytes());
        self.stdout_bytes(b"\nsummary=");
        self.stdout_bytes(entry.summary.as_bytes());
        self.stdout_bytes(b"\ntype=bin\nloader=");
        self.stdout_bytes(entry.image_kind().as_str().as_bytes());
        self.stdout_bytes(b"\nentry_fn=");
        self.stdout_bytes(entry.entry_name.as_bytes());
        self.stdout_bytes(b"\n");
    }

    /// Writes `/dev/<device>` metadata to stdout.
    pub fn write_device_file_metadata(&self, index: usize) {
        if !self.direct_backend_report_admitted(SyscallOp::DeviceCatalog) {
            return;
        }
        let Some(device) = self.daemon.devices().get(index) else {
            self.record(SyscallOp::DeviceCatalog, SyscallStatus::Error);
            return;
        };
        self.record(SyscallOp::DeviceCatalog, SyscallStatus::Ok);
        self.write_device_row(device, index);
    }

    /// Writes the kernel VFS mount table to stdout.
    pub fn write_mount_table(&self) {
        if !self.direct_backend_report_admitted(SyscallOp::VfsMounts) {
            return;
        }
        self.record(SyscallOp::VfsMounts, SyscallStatus::Ok);
        for mount in vfs::mounts() {
            self.stdout_bytes(mount.source.as_bytes());
            self.stdout_bytes(b" on ");
            self.stdout_bytes(mount.target.as_bytes());
            self.stdout_bytes(b" type ");
            self.stdout_bytes(mount.fs_type.as_bytes());
            self.stdout_bytes(b" (");
            self.stdout_bytes(mount.flags.as_bytes());
            self.stdout_bytes(b")\n");
        }
    }

    /// Returns `/bin` program descriptors exposed to kernel programs.
    #[must_use]
    pub fn programs(&self) -> &'static [ProgramDescriptor] {
        if !self.direct_backend_report_admitted(SyscallOp::ProgramHelp) {
            return &EMPTY_PROGRAM_DESCRIPTORS;
        }
        self.daemon.programs()
    }

    /// Returns device inventory exposed to kernel programs.
    #[must_use]
    pub fn devices(&self) -> &[reovim_uapi_system::DeviceEntry] {
        if !self.direct_backend_report_admitted(SyscallOp::DeviceCatalog) {
            return &[];
        }
        self.record(SyscallOp::DeviceCatalog, SyscallStatus::Ok);
        self.daemon.devices()
    }

    /// Writes a boot-info summary to stdout.
    pub fn write_boot_info_summary(&self) {
        if !self.direct_backend_report_admitted(SyscallOp::BootInfo) {
            return;
        }
        let info = self.boot_info();
        self.stdout_line("boot_info:");
        self.write_kv_num("  ranges", info.memory.range_count() as u64);
        self.write_kv_num("  usable_bytes", info.memory.usable_bytes());
        self.write_kv_num("  cpu_count", info.cpu_count as u64);
        self.write_kv_num("  heap_total_bytes", info.heap_total_bytes);
        self.write_kv_num("  cpu_freq_hz", info.cpu_freq_hz);
        self.write_kv_num("  mem_freq_hz", info.mem_freq_hz);
        self.write_kv_num("  cache_line_bytes", info.cache_line_bytes as u64);
    }

    /// Writes the boot device inventory to stdout.
    pub fn write_device_inventory(&self) {
        if !self.direct_backend_report_admitted(SyscallOp::DeviceCatalog) {
            return;
        }
        let devices = self.devices();
        self.stdout_bytes(b"devices:\n");
        let mut index = 0usize;
        while index < devices.len() {
            self.write_device_row(&devices[index], index);
            index += 1;
        }
    }

    /// Returns boot facts exposed to kernel programs.
    #[must_use]
    pub fn boot_info(&self) -> reovim_uapi_system::BootInfo {
        if !self.direct_backend_report_admitted(SyscallOp::BootInfo) {
            return BootInfo::default();
        }
        self.record(SyscallOp::BootInfo, SyscallStatus::Ok);
        self.daemon.boot_info()
    }

    /// Returns the selected root profile name.
    #[must_use]
    pub fn profile_name(&self) -> &'static str {
        if !self.direct_backend_report_admitted(SyscallOp::BootProfile) {
            return "blocked";
        }
        self.record(SyscallOp::BootProfile, SyscallStatus::Ok);
        self.daemon.profile_name()
    }

    /// Returns the selected root-shell prompt.
    #[must_use]
    pub fn prompt(&self) -> &'static str {
        if !self.direct_backend_report_admitted(SyscallOp::SnapshotSession) {
            return "blocked";
        }
        self.daemon.prompt()
    }

    /// Returns whether payload launch is enabled for the selected profile.
    #[must_use]
    pub fn launch_enabled(&self) -> bool {
        if !self.direct_backend_report_admitted(SyscallOp::BootProfile) {
            return false;
        }
        self.record(SyscallOp::BootProfile, SyscallStatus::Ok);
        self.daemon.launch_enabled()
    }

    /// Returns compile-time image identity.
    #[must_use]
    pub fn boot_image(&self) -> BootImageSummary {
        if !self.direct_backend_report_admitted(SyscallOp::BootImage) {
            return blocked_boot_image();
        }
        self.record(SyscallOp::BootImage, SyscallStatus::Ok);
        self.daemon.boot_image()
    }

    /// Writes compile-time image identity in `/boot/image` format.
    pub fn write_boot_image(&self) {
        if !self.direct_backend_report_admitted(SyscallOp::BootImage) {
            return;
        }
        self.record(SyscallOp::BootImage, SyscallStatus::Ok);
        let image = self.daemon.boot_image();
        self.stdout_bytes(b"package=");
        self.stdout_bytes(image.package.as_bytes());
        self.stdout_bytes(b"\nversion=");
        self.stdout_bytes(image.version.as_bytes());
        self.stdout_bytes(b"\ntarget=");
        self.stdout_bytes(image.target.as_bytes());
        self.stdout_bytes(b"\nselected_profile=");
        self.stdout_bytes(image.selected_profile.as_bytes());
        self.stdout_bytes(b"\nprofile_request=");
        self.stdout_bytes(image.profile_request.as_bytes());
        self.stdout_bytes(b"\nbootline=");
        self.stdout_bytes(image.bootline.as_bytes());
        self.stdout_bytes(b"\nlaunch_profile_feature=");
        self.stdout_bytes(image.launch_profile_feature.as_bytes());
        self.stdout_bytes(b"\n");
    }

    /// Returns live console-input status.
    #[must_use]
    pub fn console_input(&self) -> ConsoleInputSummary {
        if !self.direct_backend_report_admitted(SyscallOp::ConsoleInput) {
            return blocked_console_input();
        }
        self.record(SyscallOp::ConsoleInput, SyscallStatus::Ok);
        self.daemon.console_input()
    }

    /// Writes the selected boot profile, launch policy, and input summary.
    pub fn write_boot_profile(&self) {
        if !self.direct_backend_report_admitted(SyscallOp::BootProfile) {
            return;
        }
        self.stdout_bytes(b"profile=");
        self.stdout_bytes(self.profile_name().as_bytes());
        self.stdout_bytes(b"\nlaunch=");
        self.write_enabled_disabled(self.launch_enabled());
        self.stdout_bytes(b"\npayloads=");
        self.write_u64_dec(self.payloads().len() as u64);
        self.stdout_bytes(b"\nprompt=");
        self.stdout_bytes(self.prompt().as_bytes());
        let input = self.console_input();
        self.stdout_bytes(b"\ninput=");
        self.stdout_bytes(input.source.as_bytes());
        self.stdout_bytes(b"\ninput_mode=");
        self.stdout_bytes(input.mode.as_bytes());
        self.stdout_bytes(b"\nusb_keyboard=");
        self.stdout_bytes(input_state_word(input.usb_keyboard));
        self.stdout_bytes(b"\n");
    }

    /// Writes launchable payload descriptors in `/boot/payloads` format.
    pub fn write_boot_payloads(&self) {
        if !self.direct_backend_report_admitted(SyscallOp::PayloadCatalog) {
            return;
        }
        self.record(SyscallOp::PayloadCatalog, SyscallStatus::Ok);
        let payloads = self.daemon.payloads();
        let mut media_payloads = [None; crate::rootd::MAX_MEDIA_PAYLOADS];
        let media_payload_count = crate::rootd::snapshot_media_payloads(&mut media_payloads);
        if payloads.is_empty() && media_payload_count == 0 {
            self.stdout_line("launch: no payloads registered");
            return;
        }

        self.stdout_line("launch: available payloads:");
        let mut index = 0usize;
        while index < payloads.len() {
            self.stdout_bytes(b"  ");
            self.stdout_bytes(payloads[index].name.as_bytes());
            self.stdout_bytes(b": ");
            self.stdout_bytes(payloads[index].summary.as_bytes());
            self.stdout_bytes(b" loader=");
            self.stdout_bytes(payloads[index].image_kind().as_str().as_bytes());
            self.stdout_bytes(b" entry_fn=");
            self.stdout_bytes(payloads[index].entry_name.as_bytes());
            self.stdout_bytes(b"\n");
            index += 1;
        }
        index = 0;
        while index < media_payload_count {
            if let Some(payload) = media_payloads[index] {
                self.stdout_bytes(b"  ");
                self.stdout_bytes(payload.name.as_bytes());
                self.stdout_bytes(b": ");
                self.stdout_bytes(payload.summary.as_bytes());
                self.stdout_bytes(b" loader=");
                self.stdout_bytes(payload.image_kind().as_str().as_bytes());
                self.stdout_bytes(b" entry_fn=");
                self.stdout_bytes(payload.entry_name.as_bytes());
                self.stdout_bytes(b"\n");
            }
            index += 1;
        }
    }

    /// Writes boot memory facts in `/boot/memory` format.
    pub fn write_boot_memory(&self) {
        if !self.direct_backend_report_admitted(SyscallOp::BootInfo) {
            return;
        }
        self.record(SyscallOp::BootInfo, SyscallStatus::Ok);
        let info = self.daemon.boot_info();
        self.write_kv_num("ranges", info.memory.range_count() as u64);
        self.write_kv_num("usable_bytes", info.memory.usable_bytes());
        self.write_kv_num("cpu_count", info.cpu_count as u64);
        self.write_kv_num("heap_total_bytes", info.heap_total_bytes);
        self.write_kv_num("cpu_freq_hz", info.cpu_freq_hz);
        self.write_kv_num("mem_freq_hz", info.mem_freq_hz);
        self.write_kv_num("cache_line_bytes", info.cache_line_bytes as u64);
    }

    /// Writes boot device inventory in `/boot/devices` format.
    pub fn write_boot_devices(&self) {
        if !self.direct_backend_report_admitted(SyscallOp::DeviceCatalog) {
            return;
        }
        self.record(SyscallOp::DeviceCatalog, SyscallStatus::Ok);
        let devices = self.daemon.devices();
        let mut index = 0usize;
        while index < devices.len() {
            self.write_device_row(&devices[index], index);
            index += 1;
        }
    }

    /// Writes boot image, profile, input, and next-action status to stdout.
    pub fn write_boot_status(&self) {
        if !self.direct_backend_report_admitted(SyscallOp::BootImage) {
            return;
        }
        let image = self.boot_image();
        let input = self.console_input();
        self.stdout_bytes(b"package=");
        self.stdout_bytes(image.package.as_bytes());
        self.stdout_bytes(b"\nversion=");
        self.stdout_bytes(image.version.as_bytes());
        self.stdout_bytes(b"\ntarget=");
        self.stdout_bytes(image.target.as_bytes());
        self.stdout_bytes(b"\nselected_profile=");
        self.stdout_bytes(image.selected_profile.as_bytes());
        self.stdout_bytes(b"\nprofile_request=");
        self.stdout_bytes(image.profile_request.as_bytes());
        self.stdout_bytes(b"\nbootline=");
        self.stdout_bytes(image.bootline.as_bytes());
        self.stdout_bytes(b"\nlaunch_profile_feature=");
        self.stdout_bytes(image.launch_profile_feature.as_bytes());
        self.stdout_bytes(b"\nprofile=");
        self.stdout_bytes(self.profile_name().as_bytes());
        self.stdout_bytes(b"\nlaunch=");
        self.write_enabled_disabled(self.launch_enabled());
        self.stdout_bytes(b"\npayloads=");
        self.write_u64_dec(self.payloads().len() as u64);
        self.stdout_bytes(b"\ninput=");
        self.stdout_bytes(input.source.as_bytes());
        self.stdout_bytes(b"\nsource_state=");
        self.stdout_bytes(input_state_word(input.source_state));
        self.stdout_bytes(b"\ninput_mode=");
        self.stdout_bytes(input.mode.as_bytes());
        self.stdout_bytes(b"\nusb_keyboard=");
        self.stdout_bytes(input_state_word(input.usb_keyboard));
        self.stdout_bytes(b"\nusb_keyboard_probe=");
        self.write_enabled_disabled(input.usb_keyboard_probe_enabled);
        self.stdout_bytes(b"\nusb_keyboard_poll_interval_ms=");
        self.write_u64_dec(input.usb_keyboard_poll_interval_ms as u64);
        self.stdout_bytes(b"\nusb_keyboard_pending_bytes=");
        self.write_u64_dec(input.usb_keyboard_pending_bytes as u64);
        self.stdout_bytes(b"\nusb_keyboard_last_poll=");
        self.stdout_bytes(input.usb_keyboard_last_poll.as_bytes());
        self.stdout_bytes(b"\nmanual_next=");
        self.stdout_bytes(manual_next_step(input));
        self.stdout_bytes(b"\n");
    }

    /// Writes the physical proof checklist and expected facts to stdout.
    pub fn write_boot_proof(&self) {
        if !self.direct_backend_report_admitted(SyscallOp::BootImage) {
            return;
        }
        self.stdout_line("proof:");
        self.stdout_line("/bin programs:");
        self.stdout_line("  proof");
        self.stdout_line("  cat /boot/proof");
        self.stdout_line("  help");
        self.stdout_line("  help clear");
        self.stdout_line("  help screentest");
        self.stdout_line("  help input");
        self.stdout_line("  help proof");
        self.stdout_line("  help pwd");
        self.stdout_line("  help ls");
        self.stdout_line("  help cd");
        self.stdout_line("  help cat");
        self.stdout_line("  help read");
        self.stdout_line("  help mount");
        self.stdout_line("  help device");
        self.stdout_line("  help dmesg");
        self.stdout_line("  help dump");
        self.stdout_line("  help sched");
        self.stdout_line("  help proc");
        self.stdout_line("  help status");
        self.stdout_line("  help probe");
        self.stdout_line("  help launch");
        self.stdout_line("  help reovim");
        self.stdout_line("  help hello");
        self.stdout_line("  hello");
        self.stdout_line("  help halt");
        self.stdout_line("  cat /boot/help");
        self.stdout_line("  clear");
        self.stdout_line("  screentest");
        self.stdout_line("  pwd");
        self.stdout_line("  ls /");
        self.stdout_line("  ls /boot");
        self.stdout_line("  ls /dev");
        self.stdout_line("  ls /log");
        self.stdout_line("  mount");
        self.stdout_line("  cat /boot/mounts");
        self.stdout_line("  device");
        self.stdout_line("  cat /boot/memory");
        self.stdout_line("  cat /boot/devices");
        self.stdout_line("  cd /dev");
        self.stdout_line("  pwd");
        self.stdout_line("  ls");
        self.stdout_line("  cat uart0");
        self.stdout_line("  cd /");
        self.stdout_line("  cat /boot/image");
        self.stdout_line("  status");
        self.stdout_line("  cat /boot/status");
        self.stdout_line("  input");
        self.stdout_line("  cat /boot/input");
        self.stdout_line("  probe help");
        self.stdout_line("  cat /boot/probes");
        self.stdout_line("  probe pcie");
        self.stdout_line("  probe usb-keyboard");
        self.stdout_line("  cat /boot/profile");
        self.stdout_line("  launch");
        self.stdout_line("  reovim");
        self.stdout_line("  dmesg --stats");
        self.stdout_line("  dump status");
        self.stdout_line("  dump snapshot");
        self.stdout_line("  dump sync");
        self.stdout_line("  cat /log/stats");
        self.stdout_line("  cat /log/events");
        self.stdout_line("  proc");
        self.stdout_line("  dmesg");
        self.stdout_line("  cat /log/dmesg");
        self.stdout_line("terminal:");
        self.stdout_line("  halt");
        self.stdout_line("expected:");
        self.write_boot_proof_image_facts();
        self.write_boot_proof_profile_facts();
        self.stdout_line("  bootline=absent");
        self.stdout_line("  source=usb-keyboard+uart-fallback");
        self.stdout_line("  source_state=ready");
        self.stdout_line("  mode=live");
        self.stdout_line("  input_mode=live");
        self.stdout_line("  usb_keyboard=ready");
        self.stdout_line("  usb_keyboard_probe=enabled");
        self.stdout_line("  usb_keyboard_last_poll=report-ready");
        self.stdout_line(
            "  dmesg contains input.usb_keyboard=ready source=usb-keyboard+uart-fallback last_poll=report-ready",
        );
        self.stdout_line(
            "  dmesg pairs input.line_source=usb-keyboard usb_bytes>0 fallback_bytes=0 line_bytes>0 before shell:<command>",
        );
        self.stdout_line(
            "  dmesg contains shell.session path=/bin/sh loader=linked-bin entry_fn=bin_sh line=<command>",
        );
        self.stdout_line("  dmesg contains exec.parent path=/bin/<command> ppid=2 parent_task=2");
        self.stdout_line("  manual_next=type-shell-command");
        self.stdout_line("  probe usb-keyboard reports manual_next=type-shell-command");
        self.stdout_line("  detailed help catalog available through /boot/help");
        self.stdout_line("  screentest includes erase-line mode diagnostics");
        self.stdout_line("  kernel log stats available through /log/stats");
        self.stdout_line("  kernel structured events available through /log/events");
        self.stdout_line("  dump status available through /bin/dump");
        self.stdout_line("  dump sync fails closed until persistent storage is available");
        self.stdout_line("  halt attempts dump sync before root daemon shutdown");
        self.stdout_line("  kernel log dropped_bytes=0");
        self.stdout_line("  retained dmesg has no [klog] dropped_bytes marker");
        self.stdout_line("  retained dmesg includes probe usb-keyboard manual_next output");
        self.stdout_line("  probe catalog available through /boot/probes");
        self.stdout_line("  probe targets include pcie");
        self.stdout_line("  probe targets include usb-keyboard");
        self.stdout_line("  probe targets include xhci-read-keyboard-report");
        self.stdout_line("  launch/reovim disabled in shell-only profile");
        self.stdout_line("  shell.status=ok");
        self.stdout_line("  shell.status=error for disabled payload programs");
        self.stdout_line("  halt typed last prints halt: ok and stops root daemon");
    }

    /// Writes live console-input diagnostics to stdout and records one stdin
    /// read through fd 0.
    pub fn write_boot_input(&mut self) {
        if !self.direct_backend_report_admitted(SyscallOp::ConsoleInput) {
            return;
        }
        let input = self.console_input();
        let mut stdin_buf = [0u8; 8];
        let stdin_read = self.read_fd(self.stdin().fd, &mut stdin_buf).unwrap_or(0);
        self.stdout_bytes(b"source=");
        self.stdout_bytes(input.source.as_bytes());
        self.stdout_bytes(b"\nsource_state=");
        self.stdout_bytes(input_state_word(input.source_state));
        self.stdout_bytes(b"\nmode=");
        self.stdout_bytes(input.mode.as_bytes());
        self.stdout_bytes(b"\nusb_keyboard=");
        self.stdout_bytes(input_state_word(input.usb_keyboard));
        self.stdout_bytes(b"\nusb_keyboard_pending_bytes=");
        self.write_u64_dec(input.usb_keyboard_pending_bytes as u64);
        self.stdout_bytes(b"\nusb_keyboard_probe=");
        self.write_enabled_disabled(input.usb_keyboard_probe_enabled);
        self.stdout_bytes(b"\nusb_keyboard_poll_interval_ms=");
        self.write_u64_dec(input.usb_keyboard_poll_interval_ms as u64);
        self.stdout_bytes(b"\nusb_keyboard_last_poll=");
        self.stdout_bytes(input.usb_keyboard_last_poll.as_bytes());
        self.stdout_bytes(b"\nprogram_stdin_read_bytes=");
        self.write_u64_dec(stdin_read as u64);
        self.stdout_bytes(b"\n");
    }

    /// Executes another `/bin` program as a child of the current program.
    ///
    /// This is a bounded run-to-completion child exec. It proves the typed
    /// exec/spawn/wait path for programs without claiming preemptive scheduling
    /// or media-backed executable loading.
    pub fn exec_program_and_wait(
        &mut self,
        argv0: &str,
    ) -> Result<ProgramStatus, ProgramExecError> {
        let argv = match ProgramArgvBuffer::from_argv0(argv0) {
            Ok(argv) => argv,
            Err(_) => return Err(ProgramExecError::EmptyArgv),
        };
        self.exec_program_argv_and_wait(argv)
    }

    /// Executes another `/bin` program with an owned argv buffer.
    ///
    /// This is the structured-argv form of child exec: the parent program owns
    /// argv construction before the child is admitted to exec/scheduler state.
    pub fn exec_program_argv_and_wait(
        &mut self,
        argv: ProgramArgvBuffer,
    ) -> Result<ProgramStatus, ProgramExecError> {
        self.exec_program_argv_with_stdin_and_wait(argv, &[])
    }

    /// Executes another `/bin` program with an owned argv buffer and bounded stdin.
    ///
    /// This is the structured child-exec form used by payload sources and
    /// pipes: the child receives stdin through the same descriptor-shaped
    /// fd-read path as other image programs.
    pub fn exec_program_argv_with_stdin_and_wait(
        &mut self,
        argv: ProgramArgvBuffer,
        stdin: &[u8],
    ) -> Result<ProgramStatus, ProgramExecError> {
        self.exec_program_argv_with_stdin_capture_setup_and_wait(
            argv,
            stdin,
            None,
            ProgramChildFdSetup::standard(),
        )
    }

    /// Admits a same-PID `execve` replacement without running its pending image.
    ///
    /// This is the direct-backend bridge toward final no-return `execve`
    /// semantics. On success the process and task image metadata have already
    /// been replaced, close-on-exec descriptors have already been closed, and
    /// the replacement image is retained in the pending exec queue. The old
    /// direct syscall frame is then cleared so callers cannot keep issuing
    /// syscalls as if the pre-exec image returned normally.
    pub fn admit_execve_program_argv(
        &mut self,
        argv: ProgramArgvBuffer,
    ) -> Result<SyscallContext, ProgramExecError> {
        self.admit_execve_program_argv_env(argv, ProgramEnvBuffer::empty())
    }

    /// Admits a same-PID `execve` replacement with retained env metadata.
    pub fn admit_execve_program_argv_env(
        &mut self,
        argv: ProgramArgvBuffer,
        env: ProgramEnvBuffer,
    ) -> Result<SyscallContext, ProgramExecError> {
        let replaced_ctx = self.admit_execve_program_replacement(argv, env)?;
        let Some(ready) = proc::ready_process(replaced_ctx.pid) else {
            let _ = take_pending_exec(replaced_ctx.pid);
            exit_current(replaced_ctx, ProgramStatus::Error);
            append_program_exec_log(replaced_ctx, ProgramStatus::Error);
            self.current = None;
            return Err(ProgramExecError::ReplaceFailed);
        };
        let ready_ctx = SyscallContext::from_process(ready);
        let _ = with_existing_process_fd_table_mut(replaced_ctx.pid, |table| {
            table.set_stdout_capture(None);
        });
        self.current = None;
        Ok(ready_ctx)
    }

    fn admit_execve_program_replacement(
        &mut self,
        argv: ProgramArgvBuffer,
        env: ProgramEnvBuffer,
    ) -> Result<SyscallContext, ProgramExecError> {
        let Some(current) = self.current else {
            self.record(SyscallOp::ExecLoad, SyscallStatus::Unavailable);
            return Err(ProgramExecError::NoCurrentProcess);
        };
        self.ensure_no_retained_syscall_continuation_for_exec(SyscallOp::ExecReplace)?;

        let Some(argv0) = argv.argv0() else {
            self.record(SyscallOp::ExecLoad, SyscallStatus::Error);
            return Err(ProgramExecError::EmptyArgv);
        };
        let program =
            match exec::load_bin_program(self.daemon.programs(), self.daemon.source_store(), argv0)
            {
                Ok(program) => program,
                Err(exec::ExecLoadError::EmptyArgv0) => {
                    self.record(SyscallOp::ExecLoad, SyscallStatus::Error);
                    return Err(ProgramExecError::EmptyArgv);
                }
                Err(exec::ExecLoadError::NotFound) => {
                    self.record(SyscallOp::ExecLoad, SyscallStatus::Error);
                    return Err(ProgramExecError::ProgramNotFound);
                }
                Err(exec::ExecLoadError::SourceNotFound) => {
                    self.record(SyscallOp::ExecLoad, SyscallStatus::Error);
                    return Err(ProgramExecError::SourceNotFound);
                }
                Err(exec::ExecLoadError::InvalidImage) => {
                    self.record(SyscallOp::ExecLoad, SyscallStatus::Error);
                    return Err(ProgramExecError::InvalidImage);
                }
                Err(exec::ExecLoadError::ProcessAdmissionFailed) => {
                    self.record(SyscallOp::ExecSpawn, SyscallStatus::Error);
                    return Err(ProgramExecError::ProcessAdmissionFailed);
                }
            };

        let current_handle = ProcessHandle {
            pid: current.pid,
            task_id: current.task_id,
            program_path: current.program_path,
            loader: current.loader,
            entry_name: current.entry_name,
        };
        let Some(replaced) =
            exec::replace_bin_program_with_env(current_handle, program, argv, env, &[])
        else {
            record_context(current, SyscallOp::ExecReplace, SyscallStatus::Error);
            return Err(ProgramExecError::ReplaceFailed);
        };
        let replaced_ctx = SyscallContext::from_process(replaced);
        self.current = Some(replaced_ctx);
        record_context(replaced_ctx, SyscallOp::ExecLoad, SyscallStatus::Ok);
        record_context(replaced_ctx, SyscallOp::ExecReplace, SyscallStatus::Ok);
        close_on_exec_process_fds(replaced_ctx.pid);
        Ok(replaced_ctx)
    }

    /// Replaces the current process image with another `/bin` program.
    ///
    /// This keeps the temporary direct-backend compatibility behavior: the
    /// current PID and task are retained, replacement admission is performed
    /// through the no-return seam, and the retained pending image is then run
    /// synchronously through the same linked/source program path. Future
    /// trap-backed entry should make successful `execve` no-return from the old
    /// image.
    pub fn execve_program_argv(
        &mut self,
        argv: ProgramArgvBuffer,
    ) -> Result<ProgramStatus, ProgramExecError> {
        self.execve_program_argv_env(argv, ProgramEnvBuffer::empty())
    }

    /// Replaces the current process image with another `/bin` program plus env.
    pub fn execve_program_argv_env(
        &mut self,
        argv: ProgramArgvBuffer,
        env: ProgramEnvBuffer,
    ) -> Result<ProgramStatus, ProgramExecError> {
        let replaced_ctx = self.admit_execve_program_replacement(argv, env)?;

        let Some(pending) = take_pending_exec(replaced_ctx.pid) else {
            exit_current(replaced_ctx, ProgramStatus::Error);
            append_program_exec_log(replaced_ctx, ProgramStatus::Error);
            self.current = None;
            return Err(ProgramExecError::MissingProgramImage);
        };
        let status = self.run_pending_program(replaced_ctx, pending);
        self.current = None;
        Ok(status)
    }

    fn exec_program_argv_with_stdin_capture_setup_and_wait(
        &mut self,
        argv: ProgramArgvBuffer,
        stdin: &[u8],
        stdout_capture: Option<&ProgramStdoutCapture>,
        fd_setup: ProgramChildFdSetup,
    ) -> Result<ProgramStatus, ProgramExecError> {
        self.exec_program_argv_with_stdin_capture_setup_and_wait_outcome(
            argv,
            stdin,
            stdout_capture,
            fd_setup,
        )
        .map(ProgramChildRunOutcome::status)
    }

    fn exec_program_argv_with_stdin_capture_setup_and_wait_outcome(
        &mut self,
        argv: ProgramArgvBuffer,
        stdin: &[u8],
        stdout_capture: Option<&ProgramStdoutCapture>,
        fd_setup: ProgramChildFdSetup,
    ) -> Result<ProgramChildRunOutcome, ProgramExecError> {
        if stdin.len() > MAX_PROGRAM_STDIN_BYTES {
            return Err(ProgramExecError::StdinTooLarge);
        }

        let Some(parent) = self.current else {
            self.record(SyscallOp::ExecLoad, SyscallStatus::Unavailable);
            return Err(ProgramExecError::NoCurrentProcess);
        };
        self.ensure_no_retained_syscall_continuation_for_exec(SyscallOp::ExecSpawn)?;

        let Some(argv0) = argv.argv0() else {
            self.record(SyscallOp::ExecLoad, SyscallStatus::Error);
            return Err(ProgramExecError::EmptyArgv);
        };
        let program =
            match exec::load_bin_program(self.daemon.programs(), self.daemon.source_store(), argv0)
            {
                Ok(program) => program,
                Err(exec::ExecLoadError::EmptyArgv0) => {
                    self.record(SyscallOp::ExecLoad, SyscallStatus::Error);
                    return Err(ProgramExecError::EmptyArgv);
                }
                Err(exec::ExecLoadError::NotFound) => {
                    self.record(SyscallOp::ExecLoad, SyscallStatus::Error);
                    return Err(ProgramExecError::ProgramNotFound);
                }
                Err(exec::ExecLoadError::SourceNotFound) => {
                    self.record(SyscallOp::ExecLoad, SyscallStatus::Error);
                    return Err(ProgramExecError::SourceNotFound);
                }
                Err(exec::ExecLoadError::InvalidImage) => {
                    self.record(SyscallOp::ExecLoad, SyscallStatus::Error);
                    return Err(ProgramExecError::InvalidImage);
                }
                Err(exec::ExecLoadError::ProcessAdmissionFailed) => {
                    self.record(SyscallOp::ExecSpawn, SyscallStatus::Error);
                    return Err(ProgramExecError::ProcessAdmissionFailed);
                }
            };

        let parent_handle = ProcessHandle {
            pid: parent.pid,
            task_id: parent.task_id,
            program_path: parent.program_path,
            loader: parent.loader,
            entry_name: parent.entry_name,
        };
        let Some(child) =
            exec::try_spawn_program_child_with_stdin(parent_handle, program, argv, stdin)
        else {
            record_context(parent, SyscallOp::ExecSpawn, SyscallStatus::Error);
            return Err(ProgramExecError::ProcessAdmissionFailed);
        };
        let child_ctx = SyscallContext::from_process(child);
        record_context(child_ctx, SyscallOp::ExecLoad, SyscallStatus::Ok);
        record_context(child_ctx, SyscallOp::ExecSpawn, SyscallStatus::Ok);
        record_context(child_ctx, SyscallOp::SpawnChild, SyscallStatus::Ok);

        if let Err(error) = inherit_process_fds_for_exec(parent.pid, child.pid)
            .and_then(|()| install_process_stdin_bytes(child.pid, stdin))
        {
            record_context(child_ctx, SyscallOp::FdDuplicate, SyscallStatus::Error);
            exec::discard_pending_program(child.pid);
            complete_program_process(child.pid, ProgramStatus::Error);
            record_context(child_ctx, SyscallOp::ProcessExit, SyscallStatus::Error);
            append_program_exec_log(child_ctx, ProgramStatus::Error);
            let _ = error;
            return Err(ProgramExecError::FdSetupFailed);
        }

        if let Err(error) = self.install_child_fd_setup(parent.pid, child.pid, fd_setup) {
            record_context(child_ctx, SyscallOp::FdDuplicate, SyscallStatus::Error);
            exec::discard_pending_program(child.pid);
            complete_program_process(child.pid, ProgramStatus::Error);
            record_context(child_ctx, SyscallOp::ProcessExit, SyscallStatus::Error);
            append_program_exec_log(child_ctx, ProgramStatus::Error);
            let _ = error;
            return Err(ProgramExecError::FdSetupFailed);
        }

        let Some(wait) = proc::begin_wait(parent.pid, child.pid) else {
            record_context(parent, SyscallOp::WaitBegin, SyscallStatus::Error);
            exec::discard_pending_program(child.pid);
            complete_program_process(child.pid, ProgramStatus::Error);
            record_context(child_ctx, SyscallOp::ProcessExit, SyscallStatus::Error);
            append_program_exec_log(child_ctx, ProgramStatus::Error);
            return Err(ProgramExecError::WaitFailed);
        };
        record_context(parent, SyscallOp::WaitBegin, SyscallStatus::Ok);
        append_wait_start(parent, child, wait);

        let mut steps = 0usize;
        while steps < sched::MAX_KERNEL_TASKS {
            let Some(ctx) = dispatch_next_ready_program() else {
                return Err(ProgramExecError::SchedulerEmpty);
            };

            if ctx.pid == child.pid {
                let status = self.run_selected_program_child_with_capture(ctx, stdout_capture)?;
                if status == ProgramStatus::Blocked {
                    return Ok(ProgramChildRunOutcome::Blocked(child));
                }
                self.finish_program_wait(parent, child);
                return Ok(ProgramChildRunOutcome::Complete(status));
            }

            if matches!(self.run_selected_non_wait_child(ctx), Some(ProgramStatus::Halt)) {
                return Ok(ProgramChildRunOutcome::Complete(ProgramStatus::Halt));
            }
            steps += 1;
        }

        Err(ProgramExecError::DispatchBudgetExhausted)
    }

    fn install_child_fd_setup(
        &self,
        parent_pid: usize,
        child_pid: usize,
        fd_setup: ProgramChildFdSetup,
    ) -> Result<(), ProgramIoError> {
        if let Some(stdin_fd) = fd_setup.stdin_from_parent_fd {
            duplicate_process_pipe_fd_to_process(parent_pid, stdin_fd, child_pid, 0)?;
        }
        if let Some(stdout_fd) = fd_setup.stdout_from_parent_fd {
            duplicate_process_pipe_fd_to_process(parent_pid, stdout_fd, child_pid, 1)?;
        }
        Ok(())
    }

    /// Executes a bounded producer-to-consumer child pipe under the current process.
    ///
    /// Both children are admitted through the normal exec/process/scheduler
    /// path. The producer's fd 1 and consumer's fd 0 are redirected to a
    /// process-owned pipe object shared through descriptor references.
    pub fn pipe_programs_argv_and_wait(
        &mut self,
        producer: ProgramArgvBuffer,
        consumer: ProgramArgvBuffer,
    ) -> Result<ProgramStatus, ProgramExecError> {
        let (read_fd, write_fd) = self
            .pipe_fds()
            .map_err(|_| ProgramExecError::FdSetupFailed)?;
        let producer_status = self.exec_program_argv_with_stdin_capture_setup_and_wait(
            producer,
            &[],
            None,
            ProgramChildFdSetup::pipe_stdout(write_fd),
        )?;
        let _ = self.close_fd(write_fd);
        if !producer_status.is_success() {
            let _ = self.close_fd(read_fd);
            return Ok(producer_status);
        }

        let consumer_status = self.exec_program_argv_with_stdin_capture_setup_and_wait(
            consumer,
            &[],
            None,
            ProgramChildFdSetup::pipe_stdin(read_fd),
        );
        let _ = self.close_fd(read_fd);
        consumer_status
    }

    fn run_payload_source_pipe_bin(
        &mut self,
        payload: LoadedPayloadProgram,
        next_offset: usize,
        producer: ProgramArgvBuffer,
        consumer: ProgramArgvBuffer,
    ) -> PayloadSourceRunOutcome {
        let Some(current) = self.current else {
            return PayloadSourceRunOutcome::Complete(PayloadLaunchResult::Failed);
        };
        if !can_store_payload_source_continuation(current.pid) {
            record_context(current, SyscallOp::PayloadRun, SyscallStatus::Error);
            return PayloadSourceRunOutcome::Complete(PayloadLaunchResult::Failed);
        }

        let (read_fd, write_fd) = match self.pipe_fds() {
            Ok(fds) => fds,
            Err(_) => return PayloadSourceRunOutcome::Complete(PayloadLaunchResult::Failed),
        };
        let producer_status = self.exec_program_argv_with_stdin_capture_setup_and_wait_outcome(
            producer,
            &[],
            None,
            ProgramChildFdSetup::pipe_stdout(write_fd),
        );
        let _ = self.close_fd(write_fd);
        match producer_status {
            Ok(ProgramChildRunOutcome::Complete(status)) if status.is_success() => {
                self.run_payload_source_pipe_consumer(payload, next_offset, read_fd, consumer)
            }
            Ok(ProgramChildRunOutcome::Complete(status)) => {
                let _ = self.close_fd(read_fd);
                PayloadSourceRunOutcome::Complete(payload_result_from_child_status(status))
            }
            Ok(ProgramChildRunOutcome::Blocked(producer_child)) => {
                let frame = PayloadSourceContinuation {
                    payload_pid: current.pid,
                    context: current,
                    payload,
                    next_offset,
                    stage: PayloadSourceContinuationStage::WaitingPipeProducer,
                    waiting_pid: producer_child.pid,
                    read_fd,
                    consumer,
                };
                if store_payload_source_continuation(frame).is_ok() {
                    PayloadSourceRunOutcome::Blocked
                } else {
                    let _ = self.close_fd(read_fd);
                    PayloadSourceRunOutcome::Complete(PayloadLaunchResult::Failed)
                }
            }
            Err(_) => {
                let _ = self.close_fd(read_fd);
                PayloadSourceRunOutcome::Complete(PayloadLaunchResult::Failed)
            }
        }
    }

    fn run_payload_source_pipe_consumer(
        &mut self,
        payload: LoadedPayloadProgram,
        next_offset: usize,
        read_fd: usize,
        consumer: ProgramArgvBuffer,
    ) -> PayloadSourceRunOutcome {
        let Some(current) = self.current else {
            let _ = self.close_fd(read_fd);
            return PayloadSourceRunOutcome::Complete(PayloadLaunchResult::Failed);
        };
        let consumer_status = self.exec_program_argv_with_stdin_capture_setup_and_wait_outcome(
            consumer,
            &[],
            None,
            ProgramChildFdSetup::pipe_stdin(read_fd),
        );
        let _ = self.close_fd(read_fd);
        match consumer_status {
            Ok(ProgramChildRunOutcome::Complete(status)) if status.is_success() => {
                self.run_payload_source_image_from_offset(payload, next_offset)
            }
            Ok(ProgramChildRunOutcome::Complete(status)) => {
                PayloadSourceRunOutcome::Complete(payload_result_from_child_status(status))
            }
            Ok(ProgramChildRunOutcome::Blocked(consumer_child)) => {
                let frame = PayloadSourceContinuation {
                    payload_pid: current.pid,
                    context: current,
                    payload,
                    next_offset,
                    stage: PayloadSourceContinuationStage::WaitingPipeConsumer,
                    waiting_pid: consumer_child.pid,
                    read_fd: 0,
                    consumer,
                };
                if store_payload_source_continuation(frame).is_ok() {
                    PayloadSourceRunOutcome::Blocked
                } else {
                    PayloadSourceRunOutcome::Complete(PayloadLaunchResult::Failed)
                }
            }
            Err(_) => PayloadSourceRunOutcome::Complete(PayloadLaunchResult::Failed),
        }
    }

    fn resume_payload_source_frame(
        &mut self,
        frame: PayloadSourceContinuation,
    ) -> PayloadSourceRunOutcome {
        let previous = self.current;
        self.current = Some(frame.context);
        let outcome = match frame.stage {
            PayloadSourceContinuationStage::WaitingSpawnWaitBin => {
                self.resume_payload_source_spawn_wait_bin(frame)
            }
            PayloadSourceContinuationStage::WaitingSpawnWaitPayload => {
                self.resume_payload_source_spawn_wait_payload(frame)
            }
            PayloadSourceContinuationStage::WaitingPipeProducer => {
                self.resume_payload_source_pipe_producer(frame)
            }
            PayloadSourceContinuationStage::WaitingPipeConsumer => {
                self.resume_payload_source_pipe_consumer(frame)
            }
        };
        self.current = previous;
        outcome
    }

    fn run_payload_source_spawn_wait_payload(
        &mut self,
        payload: LoadedPayloadProgram,
        next_offset: usize,
        argv: ProgramArgvBuffer,
    ) -> PayloadSourceRunOutcome {
        let Some(current) = self.current else {
            return PayloadSourceRunOutcome::Complete(PayloadLaunchResult::Failed);
        };
        if !can_store_payload_source_continuation(current.pid) {
            record_context(current, SyscallOp::PayloadRun, SyscallStatus::Error);
            return PayloadSourceRunOutcome::Complete(PayloadLaunchResult::Failed);
        }

        let child = match self.spawn_payload_argv(argv) {
            Ok(child) => child,
            Err(result) => return PayloadSourceRunOutcome::Complete(result),
        };
        match self.wait_process_by_pid(child.pid) {
            Ok(wait) => {
                let child_result = payload_result_from_wait_exit(wait);
                if child_result.is_success() {
                    self.run_payload_source_image_from_offset(payload, next_offset)
                } else {
                    PayloadSourceRunOutcome::Complete(child_result)
                }
            }
            Err(ProgramProcessError::SchedulerEmpty) => {
                let frame = PayloadSourceContinuation {
                    payload_pid: current.pid,
                    context: current,
                    payload,
                    next_offset,
                    stage: PayloadSourceContinuationStage::WaitingSpawnWaitPayload,
                    waiting_pid: child.pid,
                    read_fd: 0,
                    consumer: ProgramArgvBuffer::empty(),
                };
                if store_payload_source_continuation(frame).is_ok() {
                    PayloadSourceRunOutcome::Blocked
                } else {
                    PayloadSourceRunOutcome::Complete(PayloadLaunchResult::Failed)
                }
            }
            Err(_) => PayloadSourceRunOutcome::Complete(PayloadLaunchResult::Failed),
        }
    }

    fn run_payload_source_spawn_wait_bin(
        &mut self,
        payload: LoadedPayloadProgram,
        next_offset: usize,
        argv: ProgramArgvBuffer,
    ) -> PayloadSourceRunOutcome {
        let Some(current) = self.current else {
            return PayloadSourceRunOutcome::Complete(PayloadLaunchResult::Failed);
        };
        if !can_store_payload_source_continuation(current.pid) {
            record_context(current, SyscallOp::PayloadRun, SyscallStatus::Error);
            return PayloadSourceRunOutcome::Complete(PayloadLaunchResult::Failed);
        }

        match self.exec_program_argv_with_stdin_capture_setup_and_wait_outcome(
            argv,
            &[],
            None,
            ProgramChildFdSetup::standard(),
        ) {
            Ok(ProgramChildRunOutcome::Complete(status)) if status.is_success() => {
                self.run_payload_source_image_from_offset(payload, next_offset)
            }
            Ok(ProgramChildRunOutcome::Complete(status)) => {
                PayloadSourceRunOutcome::Complete(payload_result_from_child_status(status))
            }
            Ok(ProgramChildRunOutcome::Blocked(child)) => {
                let frame = PayloadSourceContinuation {
                    payload_pid: current.pid,
                    context: current,
                    payload,
                    next_offset,
                    stage: PayloadSourceContinuationStage::WaitingSpawnWaitBin,
                    waiting_pid: child.pid,
                    read_fd: 0,
                    consumer: ProgramArgvBuffer::empty(),
                };
                if store_payload_source_continuation(frame).is_ok() {
                    PayloadSourceRunOutcome::Blocked
                } else {
                    PayloadSourceRunOutcome::Complete(PayloadLaunchResult::Failed)
                }
            }
            Err(_) => PayloadSourceRunOutcome::Complete(PayloadLaunchResult::Failed),
        }
    }

    fn resume_payload_source_spawn_wait_bin(
        &mut self,
        frame: PayloadSourceContinuation,
    ) -> PayloadSourceRunOutcome {
        let Some(record) = proc::process(frame.waiting_pid) else {
            return PayloadSourceRunOutcome::Complete(PayloadLaunchResult::Failed);
        };
        let child = process_record_handle(record);
        let wait = match self.finish_wait_by_pid(frame.context, child) {
            Ok(wait) => wait,
            Err(_) => return PayloadSourceRunOutcome::Complete(PayloadLaunchResult::Failed),
        };
        let child_result = payload_result_from_wait_exit(wait);
        if !child_result.is_success() {
            return PayloadSourceRunOutcome::Complete(child_result);
        }
        self.run_payload_source_image_from_offset(frame.payload, frame.next_offset)
    }

    fn resume_payload_source_spawn_wait_payload(
        &mut self,
        frame: PayloadSourceContinuation,
    ) -> PayloadSourceRunOutcome {
        let Some(record) = proc::process(frame.waiting_pid) else {
            return PayloadSourceRunOutcome::Complete(PayloadLaunchResult::Failed);
        };
        let child = process_record_handle(record);
        let wait = match self.finish_wait_by_pid(frame.context, child) {
            Ok(wait) => wait,
            Err(_) => return PayloadSourceRunOutcome::Complete(PayloadLaunchResult::Failed),
        };
        let child_result = payload_result_from_wait_exit(wait);
        if !child_result.is_success() {
            return PayloadSourceRunOutcome::Complete(child_result);
        }
        self.run_payload_source_image_from_offset(frame.payload, frame.next_offset)
    }

    fn resume_payload_source_pipe_producer(
        &mut self,
        frame: PayloadSourceContinuation,
    ) -> PayloadSourceRunOutcome {
        let Some(record) = proc::process(frame.waiting_pid) else {
            let _ = self.close_fd(frame.read_fd);
            return PayloadSourceRunOutcome::Complete(PayloadLaunchResult::Failed);
        };
        let producer = process_record_handle(record);
        let wait = match self.finish_wait_by_pid(frame.context, producer) {
            Ok(wait) => wait,
            Err(_) => {
                let _ = self.close_fd(frame.read_fd);
                return PayloadSourceRunOutcome::Complete(PayloadLaunchResult::Failed);
            }
        };
        let producer_result = payload_result_from_wait_exit(wait);
        if !producer_result.is_success() {
            let _ = self.close_fd(frame.read_fd);
            return PayloadSourceRunOutcome::Complete(producer_result);
        }
        self.run_payload_source_pipe_consumer(
            frame.payload,
            frame.next_offset,
            frame.read_fd,
            frame.consumer,
        )
    }

    fn resume_payload_source_pipe_consumer(
        &mut self,
        frame: PayloadSourceContinuation,
    ) -> PayloadSourceRunOutcome {
        let Some(record) = proc::process(frame.waiting_pid) else {
            return PayloadSourceRunOutcome::Complete(PayloadLaunchResult::Failed);
        };
        let consumer = process_record_handle(record);
        let wait = match self.finish_wait_by_pid(frame.context, consumer) {
            Ok(wait) => wait,
            Err(_) => return PayloadSourceRunOutcome::Complete(PayloadLaunchResult::Failed),
        };
        let consumer_result = payload_result_from_wait_exit(wait);
        if !consumer_result.is_success() {
            return PayloadSourceRunOutcome::Complete(consumer_result);
        }
        self.run_payload_source_image_from_offset(frame.payload, frame.next_offset)
    }

    /// Spawns another `/bin` program as a child without waiting for it.
    ///
    /// The child is admitted into exec state and the scheduler ready queue, but
    /// its program body does not run until a later scheduler dispatch selects
    /// it.
    pub fn spawn_program_argv(
        &mut self,
        argv: ProgramArgvBuffer,
    ) -> Result<ProcessHandle, ProgramExecError> {
        self.spawn_program_argv_env(argv, ProgramEnvBuffer::empty())
    }

    /// Spawns another `/bin` program as a child with retained env metadata.
    pub fn spawn_program_argv_env(
        &mut self,
        argv: ProgramArgvBuffer,
        env: ProgramEnvBuffer,
    ) -> Result<ProcessHandle, ProgramExecError> {
        let Some(parent) = self.current else {
            self.record(SyscallOp::ExecLoad, SyscallStatus::Unavailable);
            return Err(ProgramExecError::NoCurrentProcess);
        };
        self.ensure_no_retained_syscall_continuation_for_exec(SyscallOp::ExecSpawn)?;

        let Some(argv0) = argv.argv0() else {
            self.record(SyscallOp::ExecLoad, SyscallStatus::Error);
            return Err(ProgramExecError::EmptyArgv);
        };
        let program =
            match exec::load_bin_program(self.daemon.programs(), self.daemon.source_store(), argv0)
            {
                Ok(program) => program,
                Err(exec::ExecLoadError::EmptyArgv0) => {
                    self.record(SyscallOp::ExecLoad, SyscallStatus::Error);
                    return Err(ProgramExecError::EmptyArgv);
                }
                Err(exec::ExecLoadError::NotFound) => {
                    self.record(SyscallOp::ExecLoad, SyscallStatus::Error);
                    return Err(ProgramExecError::ProgramNotFound);
                }
                Err(exec::ExecLoadError::SourceNotFound) => {
                    self.record(SyscallOp::ExecLoad, SyscallStatus::Error);
                    return Err(ProgramExecError::SourceNotFound);
                }
                Err(exec::ExecLoadError::InvalidImage) => {
                    self.record(SyscallOp::ExecLoad, SyscallStatus::Error);
                    return Err(ProgramExecError::InvalidImage);
                }
                Err(exec::ExecLoadError::ProcessAdmissionFailed) => {
                    self.record(SyscallOp::ExecSpawn, SyscallStatus::Error);
                    return Err(ProgramExecError::ProcessAdmissionFailed);
                }
            };

        let parent_handle = ProcessHandle {
            pid: parent.pid,
            task_id: parent.task_id,
            program_path: parent.program_path,
            loader: parent.loader,
            entry_name: parent.entry_name,
        };
        let Some(child) = exec::try_spawn_program_child_with_env_and_stdin(
            parent_handle,
            program,
            argv,
            env,
            &[],
        ) else {
            record_context(parent, SyscallOp::ExecSpawn, SyscallStatus::Error);
            return Err(ProgramExecError::ProcessAdmissionFailed);
        };
        let child_ctx = SyscallContext::from_process(child);
        record_context(child_ctx, SyscallOp::ExecLoad, SyscallStatus::Ok);
        record_context(child_ctx, SyscallOp::ExecSpawn, SyscallStatus::Ok);
        record_context(child_ctx, SyscallOp::SpawnChild, SyscallStatus::Ok);
        if let Err(error) = inherit_process_fds_for_exec(parent.pid, child.pid) {
            record_context(child_ctx, SyscallOp::FdDuplicate, SyscallStatus::Error);
            exec::discard_pending_program(child.pid);
            complete_program_process(child.pid, ProgramStatus::Error);
            record_context(child_ctx, SyscallOp::ProcessExit, SyscallStatus::Error);
            append_program_exec_log(child_ctx, ProgramStatus::Error);
            let _ = error;
            return Err(ProgramExecError::FdSetupFailed);
        }
        Ok(child)
    }

    /// Spawns another `/bin` program as a child, then blocks it before it can run.
    ///
    /// The pending executable image remains in exec state. A later wake makes
    /// the process scheduler-ready without rebuilding argv or reloading the
    /// image.
    pub fn spawn_blocked_program_argv(
        &mut self,
        argv: ProgramArgvBuffer,
    ) -> Result<ProcessHandle, ProgramExecError> {
        self.spawn_blocked_program_argv_env(argv, ProgramEnvBuffer::empty())
    }

    /// Spawns another `/bin` program with env, then blocks it before it can run.
    pub fn spawn_blocked_program_argv_env(
        &mut self,
        argv: ProgramArgvBuffer,
        env: ProgramEnvBuffer,
    ) -> Result<ProcessHandle, ProgramExecError> {
        let child = self.spawn_program_argv_env(argv, env)?;
        let child_ctx = SyscallContext::from_process(child);
        let Some(blocked) = proc::block_process(child.pid) else {
            record_context(child_ctx, SyscallOp::ProcessBlock, SyscallStatus::Error);
            exec::discard_pending_program(child.pid);
            complete_program_process(child.pid, ProgramStatus::Error);
            record_context(child_ctx, SyscallOp::ProcessExit, SyscallStatus::Error);
            append_program_exec_log(child_ctx, ProgramStatus::Error);
            return Err(ProgramExecError::BlockFailed);
        };
        record_context(
            SyscallContext::from_process(blocked),
            SyscallOp::ProcessBlock,
            SyscallStatus::Ok,
        );
        append_process_control_event(
            SyscallContext::from_process(blocked),
            SyscallOp::ProcessBlock,
        );
        Ok(blocked)
    }

    /// Spawns another `/bin` program as a child, then blocks it until a tick deadline.
    ///
    /// The pending executable image remains in exec state. Explicit scheduler
    /// ticks wake the child once `wake_tick` is reached, after which normal FIFO
    /// dispatch runs the retained image.
    pub fn spawn_sleeping_program_argv(
        &mut self,
        argv: ProgramArgvBuffer,
        ticks: usize,
    ) -> Result<ProcessSleepResult, ProgramExecError> {
        self.spawn_sleeping_program_argv_env(argv, ProgramEnvBuffer::empty(), ticks)
    }

    /// Spawns another `/bin` program with env, then blocks it until a tick deadline.
    pub fn spawn_sleeping_program_argv_env(
        &mut self,
        argv: ProgramArgvBuffer,
        env: ProgramEnvBuffer,
        ticks: usize,
    ) -> Result<ProcessSleepResult, ProgramExecError> {
        let child = self.spawn_program_argv_env(argv, env)?;
        let child_ctx = SyscallContext::from_process(child);
        let snapshot = sched::snapshot_scheduler();
        let wake_tick = snapshot.tick_count.saturating_add(ticks);
        let Some(sleeping) = proc::sleep_process_until(child.pid, wake_tick) else {
            record_context(child_ctx, SyscallOp::ProcessSleep, SyscallStatus::Error);
            exec::discard_pending_program(child.pid);
            complete_program_process(child.pid, ProgramStatus::Error);
            record_context(child_ctx, SyscallOp::ProcessExit, SyscallStatus::Error);
            append_program_exec_log(child_ctx, ProgramStatus::Error);
            return Err(ProgramExecError::BlockFailed);
        };
        let sleeping_ctx = SyscallContext::from_process(sleeping);
        record_context(sleeping_ctx, SyscallOp::ProcessSleep, SyscallStatus::Ok);
        append_process_control_event(sleeping_ctx, SyscallOp::ProcessSleep);
        Ok(ProcessSleepResult {
            child: sleeping,
            wake_tick,
        })
    }

    /// Waits for a retained process by PID.
    ///
    /// This is a Reovim process-control syscall, not POSIX `waitpid`: the
    /// target is selected by retained PID, may have been adopted by rootd, and
    /// must be ready or already complete in the current cooperative scheduler.
    pub fn wait_process_by_pid(&mut self, pid: usize) -> Result<WaitRecord, ProgramProcessError> {
        self.wait_process_by_pid_inner(pid, false)
    }

    fn wait_process_by_pid_inner(
        &mut self,
        pid: usize,
        allow_retained_completion: bool,
    ) -> Result<WaitRecord, ProgramProcessError> {
        let Some(parent) = self.current else {
            self.record(SyscallOp::WaitBegin, SyscallStatus::Unavailable);
            return Err(ProgramProcessError::NoCurrentProcess);
        };
        self.ensure_no_retained_syscall_continuation_for_process_op(SyscallOp::WaitBegin)?;
        if pid == proc::ROOTD_PID
            || pid == proc::SHELL_PID
            || Some(pid) == self.current.map(|ctx| ctx.pid)
        {
            self.record(SyscallOp::WaitBegin, SyscallStatus::Error);
            return Err(ProgramProcessError::ProtectedProcess);
        }

        let Some(child_record) = proc::process(pid) else {
            self.record(SyscallOp::WaitBegin, SyscallStatus::Error);
            return Err(ProgramProcessError::ProcessNotFound);
        };
        if allow_retained_completion {
            if let Some(wait) = proc::wait_record(parent.pid, pid) {
                if wait.completed {
                    let child = process_record_handle(child_record);
                    record_context(parent, SyscallOp::WaitEnd, SyscallStatus::Ok);
                    append_wait_end(parent, child, wait);
                    return Ok(wait);
                }
                let _ = proc::block_wait_child_process(parent.pid);
                return Err(ProgramProcessError::SchedulerEmpty);
            }
        }
        if !process_state_is_waitable(child_record.state) {
            self.record(SyscallOp::WaitBegin, SyscallStatus::Error);
            return Err(ProgramProcessError::NotWaitable);
        }

        let child = process_record_handle(child_record);
        let Some(wait) = proc::begin_wait(parent.pid, child.pid) else {
            record_context(parent, SyscallOp::WaitBegin, SyscallStatus::Error);
            return Err(ProgramProcessError::WaitFailed);
        };
        record_context(parent, SyscallOp::WaitBegin, SyscallStatus::Ok);
        append_wait_start(parent, child, wait);

        if process_state_is_complete(child_record.state) {
            return self.finish_wait_by_pid(parent, child);
        }

        let mut steps = 0usize;
        while steps < sched::MAX_KERNEL_TASKS {
            let Some(ctx) = dispatch_next_ready_program() else {
                return Err(ProgramProcessError::SchedulerEmpty);
            };

            if ctx.pid == child.pid {
                self.run_selected_non_wait_child(ctx);
                let record = proc::process(child.pid).unwrap_or(proc::EMPTY_PROCESS_RECORD);
                if process_state_is_complete(record.state) {
                    return self.finish_wait_by_pid(parent, child);
                }
                return Err(ProgramProcessError::SchedulerEmpty);
            }

            self.run_selected_non_wait_child(ctx);
            steps += 1;
        }

        let _ = proc::cancel_wait(parent.pid, child.pid);
        record_context(parent, SyscallOp::WaitEnd, SyscallStatus::Error);
        Err(ProgramProcessError::DispatchBudgetExhausted)
    }

    /// Waits for a retained process by PID until completion or service readiness.
    ///
    /// This is the process-domain readiness wait used by launch-style linked
    /// programs. It keeps ordinary wait completion semantics for exited children
    /// and returns an incomplete wait report when the child publishes readiness
    /// and blocks as a resident service.
    pub fn wait_process_by_pid_until_ready(
        &mut self,
        pid: usize,
        allow_retained_readiness: bool,
    ) -> Result<WaitRecord, ProgramProcessError> {
        let Some(parent) = self.current else {
            self.record(SyscallOp::WaitBegin, SyscallStatus::Unavailable);
            return Err(ProgramProcessError::NoCurrentProcess);
        };
        self.ensure_no_retained_syscall_continuation_for_process_op(SyscallOp::WaitBegin)?;
        if pid == proc::ROOTD_PID
            || pid == proc::SHELL_PID
            || Some(pid) == self.current.map(|ctx| ctx.pid)
        {
            self.record(SyscallOp::WaitBegin, SyscallStatus::Error);
            return Err(ProgramProcessError::ProtectedProcess);
        }

        let Some(child_record) = proc::process(pid) else {
            self.record(SyscallOp::WaitBegin, SyscallStatus::Error);
            return Err(ProgramProcessError::ProcessNotFound);
        };
        if allow_retained_readiness {
            if let Some(wait) = proc::wait_record(parent.pid, pid) {
                let child = process_record_handle(child_record);
                if wait.completed {
                    record_context(parent, SyscallOp::WaitEnd, SyscallStatus::Ok);
                    append_wait_end(parent, child, wait);
                    return Ok(wait);
                }
                if process_is_service_ready_blocked(child_record) {
                    return self.finish_ready_wait_by_pid(parent, child);
                }
                let _ = proc::block_wait_child_process(parent.pid);
                return Err(ProgramProcessError::SchedulerEmpty);
            }
        }
        if !process_state_is_readiness_waitable(child_record) {
            self.record(SyscallOp::WaitBegin, SyscallStatus::Error);
            return Err(ProgramProcessError::NotWaitable);
        }

        let child = process_record_handle(child_record);
        let Some(wait) = proc::begin_wait(parent.pid, child.pid) else {
            record_context(parent, SyscallOp::WaitBegin, SyscallStatus::Error);
            return Err(ProgramProcessError::WaitFailed);
        };
        record_context(parent, SyscallOp::WaitBegin, SyscallStatus::Ok);
        append_wait_start(parent, child, wait);

        if process_state_is_complete(child_record.state) {
            return self.finish_wait_by_pid(parent, child);
        }
        if process_is_service_ready_blocked(child_record) {
            return self.finish_ready_wait_by_pid(parent, child);
        }

        let mut steps = 0usize;
        while steps < sched::MAX_KERNEL_TASKS {
            let Some(ctx) = dispatch_next_ready_program() else {
                return Err(ProgramProcessError::SchedulerEmpty);
            };

            if ctx.pid == child.pid {
                self.run_selected_non_wait_child(ctx);
                let record = proc::process(child.pid).unwrap_or(proc::EMPTY_PROCESS_RECORD);
                if process_is_service_ready_blocked(record) {
                    return self.finish_ready_wait_by_pid(parent, child);
                }
                if process_state_is_complete(record.state) {
                    return self.finish_wait_by_pid(parent, child);
                }
                return Err(ProgramProcessError::SchedulerEmpty);
            }

            self.run_selected_non_wait_child(ctx);
            steps += 1;
        }

        let _ = proc::cancel_wait(parent.pid, child.pid);
        record_context(parent, SyscallOp::WaitEnd, SyscallStatus::Error);
        Err(ProgramProcessError::DispatchBudgetExhausted)
    }

    /// Waits for a retained process while advancing bounded scheduler ticks.
    ///
    /// This is a Reovim scheduler/process syscall: it can wait on retained
    /// ready or blocked work, wake sleeping children whose deadlines arrive,
    /// and return a structured timeout instead of exposing POSIX wait or sleep
    /// semantics.
    pub fn wait_process_by_pid_for_ticks(
        &mut self,
        pid: usize,
        ticks: usize,
    ) -> Result<ProcessTimedWaitResult, ProgramProcessError> {
        let Some(parent) = self.current else {
            self.record(SyscallOp::WaitBegin, SyscallStatus::Unavailable);
            return Err(ProgramProcessError::NoCurrentProcess);
        };
        self.ensure_no_retained_syscall_continuation_for_process_op(SyscallOp::WaitBegin)?;
        if pid == proc::ROOTD_PID
            || pid == proc::SHELL_PID
            || Some(pid) == self.current.map(|ctx| ctx.pid)
        {
            self.record(SyscallOp::WaitBegin, SyscallStatus::Error);
            return Err(ProgramProcessError::ProtectedProcess);
        }

        let Some(child_record) = proc::process(pid) else {
            self.record(SyscallOp::WaitBegin, SyscallStatus::Error);
            return Err(ProgramProcessError::ProcessNotFound);
        };
        if !process_state_is_timed_waitable(child_record.state) {
            self.record(SyscallOp::WaitBegin, SyscallStatus::Error);
            return Err(ProgramProcessError::NotWaitable);
        }

        let child = process_record_handle(child_record);
        let Some(wait) = proc::begin_wait(parent.pid, child.pid) else {
            record_context(parent, SyscallOp::WaitBegin, SyscallStatus::Error);
            return Err(ProgramProcessError::WaitFailed);
        };
        record_context(parent, SyscallOp::WaitBegin, SyscallStatus::Ok);
        append_wait_start(parent, child, wait);

        if process_state_is_complete(child_record.state) {
            let wait = self.finish_wait_by_pid(parent, child)?;
            return Ok(timed_wait_result(
                child,
                wait.child_state,
                wait.exit_code,
                wait.completed,
                false,
            ));
        }

        let deadline = sched::snapshot_scheduler().tick_count.saturating_add(ticks);
        let mut steps = 0usize;
        while steps < sched::MAX_KERNEL_TASKS {
            if let Some(child_record) = proc::process(child.pid) {
                if process_state_is_complete(child_record.state) {
                    let wait = self.finish_wait_by_pid(parent, child)?;
                    return Ok(timed_wait_result(
                        child,
                        wait.child_state,
                        wait.exit_code,
                        wait.completed,
                        false,
                    ));
                }
            }

            if let Some(ctx) = dispatch_next_ready_program() {
                if ctx.pid == child.pid {
                    self.run_selected_non_wait_child(ctx);
                    let wait = self.finish_wait_by_pid(parent, child)?;
                    return Ok(timed_wait_result(
                        child,
                        wait.child_state,
                        wait.exit_code,
                        wait.completed,
                        false,
                    ));
                }

                self.run_selected_non_wait_child(ctx);
                steps += 1;
                continue;
            }

            let snapshot = sched::snapshot_scheduler();
            if snapshot.tick_count >= deadline {
                let child_record = proc::process(child.pid).unwrap_or(child_record);
                let _ = proc::cancel_wait(parent.pid, child.pid);
                record_context(parent, SyscallOp::WaitTimeout, SyscallStatus::Ok);
                append_wait_timeout(parent, child, child_record);
                return Ok(timed_wait_result(
                    child,
                    child_record.state,
                    child_record.exit_code,
                    false,
                    true,
                ));
            }

            let snapshot = sched::tick_kernel_scheduler();
            record_context(parent, SyscallOp::SchedulerTick, SyscallStatus::Ok);
            let _ = wake_sleepers_due_with_events(snapshot.tick_count);
            steps += 1;
        }

        let _ = proc::cancel_wait(parent.pid, child.pid);
        record_context(parent, SyscallOp::WaitEnd, SyscallStatus::Error);
        Err(ProgramProcessError::DispatchBudgetExhausted)
    }

    /// Wakes a blocked process by PID and returns its process handle.
    pub fn wake_process_by_pid(
        &mut self,
        pid: usize,
    ) -> Result<ProcessHandle, ProgramProcessError> {
        let _current = match self.require_direct_current_context(SyscallOp::ProcessWake) {
            Ok(ctx) => ctx,
            Err(DirectCurrentAdmissionError::Unavailable) => {
                return Err(ProgramProcessError::NoCurrentProcess);
            }
            Err(DirectCurrentAdmissionError::Busy(_)) => return Err(ProgramProcessError::Busy),
        };
        let Some(record) = proc::process(pid) else {
            self.record(SyscallOp::ProcessWake, SyscallStatus::Error);
            return Err(ProgramProcessError::ProcessNotFound);
        };
        if record.state != proc::ProcessState::Blocked {
            self.record(SyscallOp::ProcessWake, SyscallStatus::Error);
            return Err(ProgramProcessError::NotBlocked);
        }
        if record.block_reason != sched::BlockReason::Operator {
            self.record(SyscallOp::ProcessWake, SyscallStatus::Error);
            return Err(ProgramProcessError::NotBlocked);
        }
        let Some(handle) = proc::wake_operator_blocked_process(pid) else {
            self.record(SyscallOp::ProcessWake, SyscallStatus::Error);
            return Err(ProgramProcessError::ProcessNotFound);
        };
        record_context(
            SyscallContext::from_process(handle),
            SyscallOp::ProcessWake,
            SyscallStatus::Ok,
        );
        append_process_control_event(SyscallContext::from_process(handle), SyscallOp::ProcessWake);
        Ok(handle)
    }

    /// Terminates a retained ready or blocked process by PID.
    pub fn kill_process_by_pid(
        &mut self,
        pid: usize,
    ) -> Result<ProcessHandle, ProgramProcessError> {
        let _current = match self.require_direct_current_context(SyscallOp::ProcessKill) {
            Ok(ctx) => ctx,
            Err(DirectCurrentAdmissionError::Unavailable) => {
                return Err(ProgramProcessError::NoCurrentProcess);
            }
            Err(DirectCurrentAdmissionError::Busy(_)) => return Err(ProgramProcessError::Busy),
        };
        let Some(record) = proc::process(pid) else {
            self.record(SyscallOp::ProcessKill, SyscallStatus::Error);
            return Err(ProgramProcessError::ProcessNotFound);
        };
        if pid == proc::ROOTD_PID
            || pid == proc::SHELL_PID
            || Some(pid) == self.current.map(|ctx| ctx.pid)
        {
            self.record(SyscallOp::ProcessKill, SyscallStatus::Error);
            return Err(ProgramProcessError::ProtectedProcess);
        }
        if !matches!(record.state, proc::ProcessState::Ready | proc::ProcessState::Blocked) {
            self.record(SyscallOp::ProcessKill, SyscallStatus::Error);
            return Err(ProgramProcessError::NotKillable);
        }

        let handle = ProcessHandle {
            pid: record.pid,
            task_id: record.task_id,
            program_path: record.program_path,
            loader: record.loader,
            entry_name: record.entry_name,
        };
        exec::discard_pending_program(pid);
        exec::discard_pending_payload(pid);
        let adoption = complete_program_process(pid, ProgramStatus::Error);
        let ctx = SyscallContext::from_process(handle);
        record_context(ctx, SyscallOp::ProcessKill, SyscallStatus::Ok);
        append_process_control_event(ctx, SyscallOp::ProcessKill);
        if let Some(service_record) =
            service::mark_failed_by_service_pid(pid, service::ServiceReason::ProcessKilled)
        {
            append_service_failure_event(ctx, service_record);
        }
        record_context(ctx, SyscallOp::ProcessExit, SyscallStatus::Error);
        append_process_adoptions(ctx, adoption);
        append_program_exec_log(ctx, ProgramStatus::Error);
        Ok(handle)
    }

    /// Explicitly stops a retained started service by service name.
    ///
    /// This is service lifecycle control, not a new process state: the backing
    /// process is terminated as failed, while the service row records operator
    /// intent as `stopped/operator-stop`.
    pub fn stop_service_by_name(
        &mut self,
        name: &str,
    ) -> Result<ServiceRecord, ProgramServiceError> {
        let current = match self.require_direct_current_context(SyscallOp::ServiceStop) {
            Ok(ctx) => ctx,
            Err(DirectCurrentAdmissionError::Unavailable) => {
                return Err(ProgramServiceError::NoCurrentProcess);
            }
            Err(DirectCurrentAdmissionError::Busy(_)) => return Err(ProgramServiceError::Busy),
        };
        let Some(record) = service::service(name) else {
            record_context(current, SyscallOp::ServiceStop, SyscallStatus::Error);
            return Err(ProgramServiceError::ServiceNotFound);
        };
        if record.state != service::ServiceState::Started || record.service_pid == 0 {
            record_context(current, SyscallOp::ServiceStop, SyscallStatus::Error);
            return Err(ProgramServiceError::NotStoppable);
        }
        let pid = record.service_pid;
        if pid == proc::ROOTD_PID
            || pid == proc::SHELL_PID
            || Some(pid) == self.current.map(|ctx| ctx.pid)
        {
            record_context(current, SyscallOp::ServiceStop, SyscallStatus::Error);
            return Err(ProgramServiceError::ProtectedService);
        }
        let Some(process) = proc::process(pid) else {
            record_context(current, SyscallOp::ServiceStop, SyscallStatus::Error);
            return Err(ProgramServiceError::ProcessNotFound);
        };
        if !matches!(process.state, proc::ProcessState::Ready | proc::ProcessState::Blocked) {
            record_context(current, SyscallOp::ServiceStop, SyscallStatus::Error);
            return Err(ProgramServiceError::NotStoppable);
        }

        let Some(stopped) = service::mark_stopped(record.name) else {
            record_context(current, SyscallOp::ServiceStop, SyscallStatus::Error);
            return Err(ProgramServiceError::ServiceNotFound);
        };
        let handle = ProcessHandle {
            pid: process.pid,
            task_id: process.task_id,
            program_path: process.program_path,
            loader: process.loader,
            entry_name: process.entry_name,
        };
        exec::discard_pending_program(pid);
        exec::discard_pending_payload(pid);
        let adoption = complete_program_process(pid, ProgramStatus::Error);
        let ctx = SyscallContext::from_process(handle);
        record_context(ctx, SyscallOp::ProcessKill, SyscallStatus::Ok);
        append_process_control_event(ctx, SyscallOp::ProcessKill);
        append_service_lifecycle_event(ctx, SyscallOp::ServiceStop, stopped);
        record_context(ctx, SyscallOp::ProcessExit, SyscallStatus::Error);
        append_process_adoptions(ctx, adoption);
        append_program_exec_log(ctx, ProgramStatus::Error);
        Ok(stopped)
    }

    /// Explicitly starts a retained payload service by service name.
    pub fn start_service_by_name(
        &mut self,
        name: &str,
    ) -> Result<ProgramServiceControlResult, ProgramServiceError> {
        self.start_service_by_name_with_op(name, SyscallOp::ServiceStart)
    }

    /// Explicitly restarts a retained payload service by service name.
    pub fn restart_service_by_name(
        &mut self,
        name: &str,
    ) -> Result<ProgramServiceControlResult, ProgramServiceError> {
        let _current = match self.require_direct_current_context(SyscallOp::ServiceRestart) {
            Ok(ctx) => ctx,
            Err(DirectCurrentAdmissionError::Unavailable) => {
                return Err(ProgramServiceError::NoCurrentProcess);
            }
            Err(DirectCurrentAdmissionError::Busy(_)) => return Err(ProgramServiceError::Busy),
        };
        if matches!(
            service::service(name).map(|record| record.state),
            Some(service::ServiceState::Started)
        ) {
            if let Err(error) = self.stop_service_by_name(name) {
                if let Some(ctx) = self.current {
                    record_context(ctx, SyscallOp::ServiceRestart, SyscallStatus::Error);
                } else {
                    self.record(SyscallOp::ServiceRestart, SyscallStatus::Unavailable);
                }
                return Err(error);
            }
        }
        self.start_service_by_name_with_op(name, SyscallOp::ServiceRestart)
    }

    fn start_service_by_name_with_op(
        &mut self,
        name: &str,
        op: SyscallOp,
    ) -> Result<ProgramServiceControlResult, ProgramServiceError> {
        let current = match self.require_direct_current_context(op) {
            Ok(ctx) => ctx,
            Err(DirectCurrentAdmissionError::Unavailable) => {
                return Err(ProgramServiceError::NoCurrentProcess);
            }
            Err(DirectCurrentAdmissionError::Busy(_)) => return Err(ProgramServiceError::Busy),
        };
        let Some(record) = service::service(name) else {
            record_context(current, op, SyscallStatus::Error);
            return Err(ProgramServiceError::ServiceNotFound);
        };
        if record.name == "shell" || record.target.as_bytes().starts_with(b"/bin/") {
            record_context(current, op, SyscallStatus::Error);
            return Err(ProgramServiceError::ProtectedService);
        }
        if !record.target.as_bytes().starts_with(b"/payload/") {
            record_context(current, op, SyscallStatus::Error);
            return Err(ProgramServiceError::UnsupportedTarget);
        }
        if record.state == service::ServiceState::Started {
            let Some(process) = proc::process(record.service_pid) else {
                record_context(current, op, SyscallStatus::Error);
                return Err(ProgramServiceError::ProcessNotFound);
            };
            if matches!(
                process.state,
                proc::ProcessState::New
                    | proc::ProcessState::Ready
                    | proc::ProcessState::Running
                    | proc::ProcessState::Blocked
            ) {
                record_context(current, op, SyscallStatus::Error);
                return Err(ProgramServiceError::AlreadyStarted);
            }
        }
        let Ok(argv) = ProgramArgvBuffer::from_argv0(record.target) else {
            record_context(current, op, SyscallStatus::Error);
            return Err(ProgramServiceError::InvalidTarget);
        };

        let launch_result = self.launch_payload_argv(argv);
        let status = payload_syscall_status(launch_result);
        let updated = service::service(name).unwrap_or(record);
        append_service_control_event(current, op, status, updated, launch_result);
        Ok(ProgramServiceControlResult {
            record: updated,
            launch_result,
        })
    }

    /// Installs a status-only `/bin` source image into the runtime source overlay.
    ///
    /// This resolves the active OS-image `/bin` descriptor first, then writes
    /// replacement bytes to that descriptor's source-store path. Later exec
    /// admission still has to load and validate the installed source artifact.
    pub fn install_bin_source_by_name(
        &mut self,
        name: &str,
        status: BinSourceInstallStatus,
    ) -> Result<BinSourceInstallRecord, ProgramSourceInstallError> {
        if self.current.is_none() {
            self.record(SyscallOp::SourceInstall, SyscallStatus::Unavailable);
            return Err(ProgramSourceInstallError::NoCurrentProcess);
        }
        self.ensure_no_retained_syscall_continuation_for_source_install()?;

        let Some((_, descriptor)) = program::resolve_argv0(self.daemon.programs(), name) else {
            self.record(SyscallOp::SourceInstall, SyscallStatus::Error);
            return Err(ProgramSourceInstallError::ProgramNotFound);
        };
        if descriptor.image_kind() != program::ProgramImageKind::SourceImage {
            self.record(SyscallOp::SourceInstall, SyscallStatus::Error);
            return Err(ProgramSourceInstallError::UnsupportedImage);
        }
        let path = descriptor.image.source_path();
        let bytes = status.source_bytes();

        match source_store::install_source(SourceArtifactNamespace::Bin, path, bytes) {
            Ok(()) => {
                self.record(SyscallOp::SourceInstall, SyscallStatus::Ok);
                Ok(BinSourceInstallRecord {
                    name: descriptor.name,
                    path,
                    status,
                    bytes_len: bytes.len(),
                })
            }
            Err(error) => {
                self.record(SyscallOp::SourceInstall, SyscallStatus::Error);
                Err(ProgramSourceInstallError::InstallFailed(error))
            }
        }
    }

    fn read_checked_source_media_artifact<'a>(
        &mut self,
        expected_namespace: SourceArtifactNamespace,
        expected_path: &'static str,
        out: &'a mut [u8; MAX_SOURCE_MEDIA_ARTIFACT_BYTES],
    ) -> Result<source_media::SourceMediaArtifactRead<'a>, ProgramSourceInstallError> {
        match source_media::read_checked_artifact(expected_namespace, expected_path, out) {
            Ok(read) => {
                self.record(SyscallOp::SourceMediaRead, SyscallStatus::Ok);
                Ok(read)
            }
            Err(source_media::SourceMediaReadError::Unavailable) => {
                self.record(SyscallOp::SourceMediaRead, SyscallStatus::Unavailable);
                Err(ProgramSourceInstallError::SourceMediaUnavailable)
            }
            Err(source_media::SourceMediaReadError::ReadFailed) => {
                self.record(SyscallOp::SourceMediaRead, SyscallStatus::Error);
                Err(ProgramSourceInstallError::SourceMediaReadFailed)
            }
            Err(source_media::SourceMediaReadError::Invalid) => {
                self.record(SyscallOp::SourceMediaRead, SyscallStatus::Ok);
                self.record(SyscallOp::SourceInstall, SyscallStatus::Error);
                Err(ProgramSourceInstallError::SourceMediaInvalid)
            }
            Err(source_media::SourceMediaReadError::NamespaceMismatch) => {
                self.record(SyscallOp::SourceMediaRead, SyscallStatus::Ok);
                self.record(SyscallOp::SourceInstall, SyscallStatus::Error);
                Err(ProgramSourceInstallError::SourceMediaNamespaceMismatch)
            }
            Err(source_media::SourceMediaReadError::PathMismatch) => {
                self.record(SyscallOp::SourceMediaRead, SyscallStatus::Ok);
                self.record(SyscallOp::SourceInstall, SyscallStatus::Error);
                Err(ProgramSourceInstallError::SourceMediaPathMismatch)
            }
        }
    }

    /// Installs source bytes for a `/bin` descriptor from the source-media block target.
    pub fn install_bin_source_from_media_by_name(
        &mut self,
        name: &str,
    ) -> Result<SourceMediaInstallRecord, ProgramSourceInstallError> {
        if self.current.is_none() {
            self.record(SyscallOp::SourceInstall, SyscallStatus::Unavailable);
            return Err(ProgramSourceInstallError::NoCurrentProcess);
        }
        self.ensure_no_retained_syscall_continuation_for_source_install()?;

        let Some((_, descriptor)) = program::resolve_argv0(self.daemon.programs(), name) else {
            self.record(SyscallOp::SourceInstall, SyscallStatus::Error);
            return Err(ProgramSourceInstallError::ProgramNotFound);
        };
        if descriptor.image_kind() != program::ProgramImageKind::SourceImage {
            self.record(SyscallOp::SourceInstall, SyscallStatus::Error);
            return Err(ProgramSourceInstallError::UnsupportedImage);
        }
        let path = descriptor.image.source_path();
        let mut bytes = [0u8; MAX_SOURCE_MEDIA_ARTIFACT_BYTES];
        let media = self.read_checked_source_media_artifact(
            SourceArtifactNamespace::Bin,
            path,
            &mut bytes,
        )?;

        match source_store::install_source(
            SourceArtifactNamespace::Bin,
            path,
            media.artifact.source_bytes,
        ) {
            Ok(()) => {
                self.record(SyscallOp::SourceInstall, SyscallStatus::Ok);
                Ok(SourceMediaInstallRecord {
                    name: descriptor.name,
                    namespace: SourceArtifactNamespace::Bin,
                    path,
                    storage: media.read.storage,
                    storage_capacity_bytes: media.read.capacity_bytes,
                    artifact_bytes_len: media.artifact_bytes_len,
                    bytes_len: media.artifact.source_bytes.len(),
                    checksum: media.artifact.checksum,
                })
            }
            Err(error) => {
                self.record(SyscallOp::SourceInstall, SyscallStatus::Error);
                Err(ProgramSourceInstallError::InstallFailed(error))
            }
        }
    }

    /// Installs a status-only payload source image into the runtime source overlay.
    ///
    /// This is the syscall path that lets an image `/bin` program change payload
    /// admission state without reaching into the source-store internals directly.
    pub fn install_payload_source_by_name(
        &mut self,
        name: &str,
        status: PayloadSourceInstallStatus,
    ) -> Result<PayloadSourceInstallRecord, ProgramSourceInstallError> {
        if self.current.is_none() {
            self.record(SyscallOp::SourceInstall, SyscallStatus::Unavailable);
            return Err(ProgramSourceInstallError::NoCurrentProcess);
        }
        self.ensure_no_retained_syscall_continuation_for_source_install()?;

        let Some((_, payload)) = self.daemon.payload_by_name(name) else {
            self.record(SyscallOp::SourceInstall, SyscallStatus::Error);
            return Err(ProgramSourceInstallError::PayloadNotFound);
        };
        let path = payload.image.source_path();
        let bytes = status.source_bytes();

        match source_store::install_source(SourceArtifactNamespace::Payload, path, bytes) {
            Ok(()) => {
                self.record(SyscallOp::SourceInstall, SyscallStatus::Ok);
                Ok(PayloadSourceInstallRecord {
                    name: payload.name,
                    path,
                    status,
                    bytes_len: bytes.len(),
                })
            }
            Err(error) => {
                self.record(SyscallOp::SourceInstall, SyscallStatus::Error);
                Err(ProgramSourceInstallError::InstallFailed(error))
            }
        }
    }

    /// Installs source bytes for a payload descriptor from the source-media block target.
    pub fn install_payload_source_from_media_by_name(
        &mut self,
        name: &str,
    ) -> Result<SourceMediaInstallRecord, ProgramSourceInstallError> {
        if self.current.is_none() {
            self.record(SyscallOp::SourceInstall, SyscallStatus::Unavailable);
            return Err(ProgramSourceInstallError::NoCurrentProcess);
        }
        self.ensure_no_retained_syscall_continuation_for_source_install()?;

        let Some((_, payload)) = self.daemon.payload_by_name(name) else {
            self.record(SyscallOp::SourceInstall, SyscallStatus::Error);
            return Err(ProgramSourceInstallError::PayloadNotFound);
        };
        let path = payload.image.source_path();
        let mut bytes = [0u8; MAX_SOURCE_MEDIA_ARTIFACT_BYTES];
        let media = self.read_checked_source_media_artifact(
            SourceArtifactNamespace::Payload,
            path,
            &mut bytes,
        )?;

        match source_store::install_source(
            SourceArtifactNamespace::Payload,
            path,
            media.artifact.source_bytes,
        ) {
            Ok(()) => {
                self.record(SyscallOp::SourceInstall, SyscallStatus::Ok);
                Ok(SourceMediaInstallRecord {
                    name: payload.name,
                    namespace: SourceArtifactNamespace::Payload,
                    path,
                    storage: media.read.storage,
                    storage_capacity_bytes: media.read.capacity_bytes,
                    artifact_bytes_len: media.artifact_bytes_len,
                    bytes_len: media.artifact.source_bytes.len(),
                    checksum: media.artifact.checksum,
                })
            }
            Err(error) => {
                self.record(SyscallOp::SourceInstall, SyscallStatus::Error);
                Err(ProgramSourceInstallError::InstallFailed(error))
            }
        }
    }

    /// Returns registered payload descriptors.
    #[must_use]
    pub fn payloads(&self) -> &[PayloadDescriptor] {
        if !self.direct_backend_report_admitted(SyscallOp::PayloadCatalog) {
            return &[];
        }
        self.record(SyscallOp::PayloadCatalog, SyscallStatus::Ok);
        self.daemon.payloads()
    }

    /// Launches a registered payload by name through the root supervisor.
    pub fn launch_payload_by_name(&mut self, name: &str) -> PayloadLaunchResult {
        let Ok(argv) = ProgramArgvBuffer::from_argv0(name) else {
            self.record(SyscallOp::PayloadLoad, SyscallStatus::Error);
            self.record(SyscallOp::PayloadLaunch, SyscallStatus::Error);
            return PayloadLaunchResult::Failed;
        };
        self.launch_payload_argv(argv)
    }

    /// Launches a registered payload with bounded argv metadata.
    pub fn launch_payload_argv(&mut self, argv: ProgramArgvBuffer) -> PayloadLaunchResult {
        let Some(name) = argv.argv0() else {
            self.record(SyscallOp::PayloadLoad, SyscallStatus::Error);
            self.record(SyscallOp::PayloadLaunch, SyscallStatus::Error);
            return PayloadLaunchResult::Failed;
        };
        if let Err(result) = self.ensure_direct_backend_payload_launch_admitted() {
            return result;
        }
        if !self.daemon.launch_enabled() {
            self.record(SyscallOp::PayloadLaunch, SyscallStatus::Unavailable);
            return PayloadLaunchResult::NotConfigured;
        }

        let payload = match exec::load_payload_by_name(
            self.daemon.payloads(),
            self.daemon.source_store(),
            name,
        ) {
            Ok(payload) => payload,
            Err(exec::ExecLoadError::InvalidImage) => {
                self.record(SyscallOp::PayloadLoad, SyscallStatus::Error);
                self.record(SyscallOp::PayloadLaunch, SyscallStatus::Error);
                return PayloadLaunchResult::Failed;
            }
            Err(_) => {
                self.record(SyscallOp::PayloadLoad, SyscallStatus::Error);
                self.record(SyscallOp::PayloadLaunch, SyscallStatus::Error);
                return PayloadLaunchResult::NotConfigured;
            }
        };
        self.record(SyscallOp::PayloadLoad, SyscallStatus::Ok);

        let Some(parent) = self.current else {
            let result = self.daemon.run_loaded_payload(payload);
            self.record_payload_launch(result);
            return result;
        };

        let parent_handle = ProcessHandle {
            pid: parent.pid,
            task_id: parent.task_id,
            program_path: parent.program_path,
            loader: parent.loader,
            entry_name: parent.entry_name,
        };
        let Some(child) = exec::try_spawn_payload_child_with_argv(parent_handle, payload, argv)
        else {
            record_context(parent, SyscallOp::SpawnChild, SyscallStatus::Error);
            self.record_payload_launch(PayloadLaunchResult::Failed);
            return PayloadLaunchResult::Failed;
        };
        let child_ctx = SyscallContext::from_process(child);
        record_context(child_ctx, SyscallOp::SpawnChild, SyscallStatus::Ok);
        if inherit_process_fds_for_exec(parent.pid, child.pid).is_err() {
            record_context(child_ctx, SyscallOp::FdDuplicate, SyscallStatus::Error);
            exec::discard_pending_payload(child.pid);
            complete_payload_process(child.pid, PayloadLaunchResult::Failed);
            record_terminal_service_for_payload(child_ctx, PayloadLaunchResult::Failed);
            record_context(child_ctx, SyscallOp::ProcessExit, SyscallStatus::Error);
            self.record_payload_launch(PayloadLaunchResult::Failed);
            return PayloadLaunchResult::Failed;
        }
        let Some(wait) = proc::begin_wait(parent.pid, child.pid) else {
            record_context(parent, SyscallOp::WaitBegin, SyscallStatus::Error);
            exec::discard_pending_payload(child.pid);
            complete_payload_process(child.pid, PayloadLaunchResult::Failed);
            record_terminal_service_for_payload(child_ctx, PayloadLaunchResult::Failed);
            record_context(child_ctx, SyscallOp::ProcessExit, SyscallStatus::Error);
            self.record_payload_launch(PayloadLaunchResult::Failed);
            return PayloadLaunchResult::Failed;
        };

        record_context(parent, SyscallOp::WaitBegin, SyscallStatus::Ok);
        append_payload_wait_start(parent, child, wait);
        let mut steps = 0usize;
        while steps < sched::MAX_KERNEL_TASKS {
            let Some(ctx) = dispatch_next_ready_program() else {
                let result = PayloadLaunchResult::Failed;
                exec::discard_pending_payload(child.pid);
                complete_payload_process(child.pid, result);
                record_terminal_service_for_payload(child_ctx, result);
                record_context(child_ctx, SyscallOp::ProcessExit, SyscallStatus::Error);
                self.finish_payload_wait(parent, child);
                self.record_payload_launch(result);
                return result;
            };

            if ctx.pid == child.pid {
                match self.run_selected_payload_child(parent, child) {
                    PayloadSourceRunOutcome::Complete(result) => {
                        if result == PayloadLaunchResult::Resident {
                            self.finish_payload_readiness_wait(parent, child);
                        } else {
                            self.finish_payload_wait(parent, child);
                        }
                        self.record_payload_launch(result);
                        return result;
                    }
                    PayloadSourceRunOutcome::Blocked => {
                        self.record_payload_launch(PayloadLaunchResult::Failed);
                        return PayloadLaunchResult::Failed;
                    }
                }
            }

            self.run_selected_non_wait_child(ctx);
            steps += 1;
        }

        let result = PayloadLaunchResult::Failed;
        exec::discard_pending_payload(child.pid);
        complete_payload_process(child.pid, result);
        record_terminal_service_for_payload(child_ctx, result);
        record_context(child_ctx, SyscallOp::ProcessExit, SyscallStatus::Error);
        self.finish_payload_wait(parent, child);
        self.record_payload_launch(result);
        result
    }

    fn spawn_payload_argv(
        &mut self,
        argv: ProgramArgvBuffer,
    ) -> Result<ProcessHandle, PayloadLaunchResult> {
        self.spawn_payload_argv_env(argv, ProgramEnvBuffer::empty())
    }

    fn spawn_payload_argv_env(
        &mut self,
        argv: ProgramArgvBuffer,
        env: ProgramEnvBuffer,
    ) -> Result<ProcessHandle, PayloadLaunchResult> {
        let Some(name) = argv.argv0() else {
            self.record(SyscallOp::PayloadLoad, SyscallStatus::Error);
            self.record(SyscallOp::PayloadLaunch, SyscallStatus::Error);
            return Err(PayloadLaunchResult::Failed);
        };
        self.ensure_direct_backend_payload_launch_admitted()?;
        if !self.daemon.launch_enabled() {
            self.record(SyscallOp::PayloadLaunch, SyscallStatus::Unavailable);
            return Err(PayloadLaunchResult::NotConfigured);
        }

        let payload = match exec::load_payload_by_name(
            self.daemon.payloads(),
            self.daemon.source_store(),
            name,
        ) {
            Ok(payload) => payload,
            Err(exec::ExecLoadError::InvalidImage) => {
                self.record(SyscallOp::PayloadLoad, SyscallStatus::Error);
                self.record(SyscallOp::PayloadLaunch, SyscallStatus::Error);
                return Err(PayloadLaunchResult::Failed);
            }
            Err(_) => {
                self.record(SyscallOp::PayloadLoad, SyscallStatus::Error);
                self.record(SyscallOp::PayloadLaunch, SyscallStatus::Error);
                return Err(PayloadLaunchResult::NotConfigured);
            }
        };
        self.record(SyscallOp::PayloadLoad, SyscallStatus::Ok);

        let Some(parent) = self.current else {
            self.record(SyscallOp::PayloadLaunch, SyscallStatus::Error);
            return Err(PayloadLaunchResult::Failed);
        };

        let parent_handle = ProcessHandle {
            pid: parent.pid,
            task_id: parent.task_id,
            program_path: parent.program_path,
            loader: parent.loader,
            entry_name: parent.entry_name,
        };
        let Some(child) =
            exec::try_spawn_payload_child_with_argv_env(parent_handle, payload, argv, env)
        else {
            record_context(parent, SyscallOp::SpawnChild, SyscallStatus::Error);
            self.record(SyscallOp::PayloadLaunch, SyscallStatus::Error);
            return Err(PayloadLaunchResult::Failed);
        };
        let child_ctx = SyscallContext::from_process(child);
        record_context(child_ctx, SyscallOp::SpawnChild, SyscallStatus::Ok);
        if inherit_process_fds_for_exec(parent.pid, child.pid).is_err() {
            record_context(child_ctx, SyscallOp::FdDuplicate, SyscallStatus::Error);
            exec::discard_pending_payload(child.pid);
            complete_payload_process(child.pid, PayloadLaunchResult::Failed);
            record_context(child_ctx, SyscallOp::ProcessExit, SyscallStatus::Error);
            self.record(SyscallOp::PayloadLaunch, SyscallStatus::Error);
            return Err(PayloadLaunchResult::Failed);
        }
        self.record(SyscallOp::PayloadLaunch, SyscallStatus::Ok);
        Ok(child)
    }

    fn run_selected_payload_child(
        &mut self,
        parent: SyscallContext,
        child: ProcessHandle,
    ) -> PayloadSourceRunOutcome {
        let outcome = match exec::take_pending_payload(child.pid) {
            Some(pending) => self.run_pending_payload(parent, pending.child(), pending.payload()),
            None => PayloadSourceRunOutcome::Complete(PayloadLaunchResult::Failed),
        };
        let child_ctx = SyscallContext::from_process(child);
        let PayloadSourceRunOutcome::Complete(result) = outcome else {
            return PayloadSourceRunOutcome::Blocked;
        };
        if result == PayloadLaunchResult::Resident {
            append_payload_resident(child);
            return PayloadSourceRunOutcome::Complete(result);
        }
        let adoption = complete_payload_process(child.pid, result);
        record_terminal_service_for_payload(child_ctx, result);
        record_context(child_ctx, SyscallOp::ProcessExit, payload_syscall_status(result));
        append_process_adoptions(child_ctx, adoption);
        append_payload_exit(child, result);
        PayloadSourceRunOutcome::Complete(result)
    }

    fn run_pending_payload(
        &mut self,
        parent: SyscallContext,
        child: ProcessHandle,
        payload: LoadedPayloadProgram,
    ) -> PayloadSourceRunOutcome {
        append_payload_start(parent, child, payload);
        let outcome = self.run_loaded_payload_as_current(child, payload);
        record_context(
            SyscallContext::from_process(child),
            SyscallOp::PayloadRun,
            outcome.syscall_status(),
        );
        outcome
    }

    fn run_loaded_payload_as_current(
        &mut self,
        child: ProcessHandle,
        payload: LoadedPayloadProgram,
    ) -> PayloadSourceRunOutcome {
        let previous = self.current;
        self.current = Some(SyscallContext::from_process(child));
        let result = self.run_payload_source_image(payload);
        self.current = previous;
        result
    }

    fn run_payload_source_image(
        &mut self,
        payload: LoadedPayloadProgram,
    ) -> PayloadSourceRunOutcome {
        if !self.daemon.launch_enabled() {
            return PayloadSourceRunOutcome::Complete(PayloadLaunchResult::NotConfigured);
        }

        let Some(offset) = crate::rootd::payload_source_op_start(payload.source_bytes()) else {
            return PayloadSourceRunOutcome::Complete(PayloadLaunchResult::Failed);
        };

        self.run_payload_source_image_from_offset(payload, offset)
    }

    fn run_payload_source_image_from_offset(
        &mut self,
        payload: LoadedPayloadProgram,
        mut offset: usize,
    ) -> PayloadSourceRunOutcome {
        let mut ops = 0usize;
        loop {
            match crate::rootd::next_payload_source_op(payload.source_bytes(), offset) {
                PayloadSourceStep::End => {
                    return PayloadSourceRunOutcome::Complete(PayloadLaunchResult::Ready);
                }
                PayloadSourceStep::Invalid => {
                    return PayloadSourceRunOutcome::Complete(PayloadLaunchResult::Failed);
                }
                PayloadSourceStep::Op { op, next } => {
                    ops += 1;
                    if ops > crate::rootd::payload_source_max_ops() {
                        return PayloadSourceRunOutcome::Complete(PayloadLaunchResult::Failed);
                    }
                    match op {
                        PayloadSourceOp::Noop => {}
                        PayloadSourceOp::ExitStatus(result) => {
                            if result != PayloadLaunchResult::Ready {
                                return PayloadSourceRunOutcome::Complete(result);
                            }
                        }
                        PayloadSourceOp::ExitCode(code) => {
                            return PayloadSourceRunOutcome::Complete(
                                PayloadLaunchResult::ExitCode(code),
                            );
                        }
                        PayloadSourceOp::ServiceReady(name) => {
                            if self.service_ready(name).is_err() {
                                return PayloadSourceRunOutcome::Complete(
                                    PayloadLaunchResult::Failed,
                                );
                            }
                        }
                        PayloadSourceOp::ServiceHold => {
                            if self.service_hold().is_err() {
                                return PayloadSourceRunOutcome::Complete(
                                    PayloadLaunchResult::Failed,
                                );
                            }
                            return PayloadSourceRunOutcome::Complete(
                                PayloadLaunchResult::Resident,
                            );
                        }
                        PayloadSourceOp::ExecPayload(argv) => {
                            match self.launch_payload_argv(argv) {
                                PayloadLaunchResult::Ready
                                | PayloadLaunchResult::Resident
                                | PayloadLaunchResult::ExitCode(0) => {}
                                result => return PayloadSourceRunOutcome::Complete(result),
                            }
                        }
                        PayloadSourceOp::SpawnPayload(argv) => {
                            if let Err(result) = self.spawn_payload_argv(argv) {
                                return PayloadSourceRunOutcome::Complete(result);
                            }
                        }
                        PayloadSourceOp::SpawnWaitPayload(argv) => {
                            match self.run_payload_source_spawn_wait_payload(payload, next, argv) {
                                PayloadSourceRunOutcome::Complete(result)
                                    if result.is_success() => {}
                                outcome => return outcome,
                            }
                        }
                        PayloadSourceOp::SpawnKillPayload(argv) => {
                            let child = match self.spawn_payload_argv(argv) {
                                Ok(child) => child,
                                Err(result) => return PayloadSourceRunOutcome::Complete(result),
                            };
                            if self.kill_process_by_pid(child.pid).is_err() {
                                return PayloadSourceRunOutcome::Complete(
                                    PayloadLaunchResult::Failed,
                                );
                            }
                        }
                        PayloadSourceOp::ExecBin(argv) => {
                            match self.exec_program_argv_and_wait(argv) {
                                Ok(status) if status.is_success() => {}
                                Ok(status) => {
                                    return PayloadSourceRunOutcome::Complete(
                                        payload_result_from_child_status(status),
                                    );
                                }
                                Err(_) => {
                                    return PayloadSourceRunOutcome::Complete(
                                        PayloadLaunchResult::Failed,
                                    );
                                }
                            }
                        }
                        PayloadSourceOp::ExecBinStdinHex { stdin_hex, argv } => {
                            let Some((stdin, stdin_len)) =
                                Self::payload_source_hex_to_stdin(stdin_hex)
                            else {
                                return PayloadSourceRunOutcome::Complete(
                                    PayloadLaunchResult::Failed,
                                );
                            };
                            match self
                                .exec_program_argv_with_stdin_and_wait(argv, &stdin[..stdin_len])
                            {
                                Ok(status) if status.is_success() => {}
                                Ok(status) => {
                                    return PayloadSourceRunOutcome::Complete(
                                        payload_result_from_child_status(status),
                                    );
                                }
                                Err(_) => {
                                    return PayloadSourceRunOutcome::Complete(
                                        PayloadLaunchResult::Failed,
                                    );
                                }
                            }
                        }
                        PayloadSourceOp::PipeBin { producer, consumer } => {
                            match self
                                .run_payload_source_pipe_bin(payload, next, producer, consumer)
                            {
                                PayloadSourceRunOutcome::Complete(result)
                                    if result.is_success() => {}
                                outcome => return outcome,
                            }
                        }
                        PayloadSourceOp::SpawnBin(argv) => {
                            if self.spawn_program_argv(argv).is_err() {
                                return PayloadSourceRunOutcome::Complete(
                                    PayloadLaunchResult::Failed,
                                );
                            }
                        }
                        PayloadSourceOp::SpawnWaitBin(argv) => {
                            match self.run_payload_source_spawn_wait_bin(payload, next, argv) {
                                PayloadSourceRunOutcome::Complete(result)
                                    if result.is_success() => {}
                                outcome => return outcome,
                            }
                        }
                        PayloadSourceOp::SpawnKillBin(argv) => {
                            let child = match self.spawn_program_argv(argv) {
                                Ok(child) => child,
                                Err(_) => {
                                    return PayloadSourceRunOutcome::Complete(
                                        PayloadLaunchResult::Failed,
                                    );
                                }
                            };
                            if self.kill_process_by_pid(child.pid).is_err() {
                                return PayloadSourceRunOutcome::Complete(
                                    PayloadLaunchResult::Failed,
                                );
                            }
                        }
                        PayloadSourceOp::SleepBin { ticks, argv } => {
                            if self.spawn_sleeping_program_argv(argv, ticks).is_err() {
                                return PayloadSourceRunOutcome::Complete(
                                    PayloadLaunchResult::Failed,
                                );
                            }
                        }
                        PayloadSourceOp::SleepWaitBin { ticks, argv } => {
                            let child = match self.spawn_sleeping_program_argv(argv, ticks) {
                                Ok(result) => result.child,
                                Err(_) => {
                                    return PayloadSourceRunOutcome::Complete(
                                        PayloadLaunchResult::Failed,
                                    );
                                }
                            };
                            match self.wait_process_by_pid_for_ticks(child.pid, ticks) {
                                Ok(wait)
                                    if wait.completed && !wait.timed_out && wait.exit_code == 0 => {
                                }
                                Ok(wait) => {
                                    return PayloadSourceRunOutcome::Complete(
                                        payload_result_from_timed_wait_exit(wait),
                                    );
                                }
                                Err(_) => {
                                    return PayloadSourceRunOutcome::Complete(
                                        PayloadLaunchResult::Failed,
                                    );
                                }
                            }
                        }
                        PayloadSourceOp::SchedulerTick => {
                            let result = self.scheduler_tick_result();
                            if result.status != SchedulerTickStatus::Ok {
                                return PayloadSourceRunOutcome::Complete(
                                    PayloadLaunchResult::Failed,
                                );
                            }
                        }
                        PayloadSourceOp::YieldNow => {
                            let result = self.yield_now_result();
                            if !matches!(
                                result.status,
                                SchedulerYieldStatus::Yielded | SchedulerYieldStatus::NoPeer
                            ) {
                                return PayloadSourceRunOutcome::Complete(
                                    PayloadLaunchResult::Failed,
                                );
                            }
                        }
                        PayloadSourceOp::ReadTtyLine => {
                            let mut line = [0u8; crate::rootd::ROOT_LINE_BYTES];
                            let len = self.read_tty_line(&mut line);
                            if len == 0 {
                                return PayloadSourceRunOutcome::Complete(
                                    PayloadLaunchResult::Failed,
                                );
                            }
                            self.stdout_bytes(&line[..len]);
                            self.stdout_bytes(b"\n");
                        }
                        PayloadSourceOp::WriteProcessSelf => {
                            self.write_current_process();
                        }
                        PayloadSourceOp::WriteProcessTable => {
                            if self.write_vfs_file_path("/proc/processes").is_err() {
                                return PayloadSourceRunOutcome::Complete(
                                    PayloadLaunchResult::Failed,
                                );
                            }
                        }
                        PayloadSourceOp::WriteServiceTable => {
                            if self.write_vfs_file_path("/proc/services").is_err() {
                                return PayloadSourceRunOutcome::Complete(
                                    PayloadLaunchResult::Failed,
                                );
                            }
                        }
                        PayloadSourceOp::WriteBootProfile => {
                            self.write_boot_profile();
                        }
                        PayloadSourceOp::WriteDeviceTable => {
                            self.write_device_inventory();
                        }
                        PayloadSourceOp::WriteExecTable => {
                            if self.write_vfs_file_path("/proc/execs").is_err() {
                                return PayloadSourceRunOutcome::Complete(
                                    PayloadLaunchResult::Failed,
                                );
                            }
                        }
                        PayloadSourceOp::WritePendingExecTable => {
                            if self.write_vfs_file_path("/proc/pending").is_err() {
                                return PayloadSourceRunOutcome::Complete(
                                    PayloadLaunchResult::Failed,
                                );
                            }
                        }
                        PayloadSourceOp::WriteSourceTable => {
                            if self.write_vfs_file_path("/proc/sources").is_err() {
                                return PayloadSourceRunOutcome::Complete(
                                    PayloadLaunchResult::Failed,
                                );
                            }
                        }
                        PayloadSourceOp::WriteSchedulerState => {
                            if self.write_vfs_file_path("/proc/scheduler").is_err() {
                                return PayloadSourceRunOutcome::Complete(
                                    PayloadLaunchResult::Failed,
                                );
                            }
                        }
                        PayloadSourceOp::WriteTaskTable => {
                            if self.write_vfs_file_path("/proc/tasks").is_err() {
                                return PayloadSourceRunOutcome::Complete(
                                    PayloadLaunchResult::Failed,
                                );
                            }
                        }
                        PayloadSourceOp::WriteWaitTable => {
                            if self.write_vfs_file_path("/proc/waits").is_err() {
                                return PayloadSourceRunOutcome::Complete(
                                    PayloadLaunchResult::Failed,
                                );
                            }
                        }
                        PayloadSourceOp::WriteSyscallTable => {
                            if self.write_vfs_file_path("/proc/syscalls").is_err() {
                                return PayloadSourceRunOutcome::Complete(
                                    PayloadLaunchResult::Failed,
                                );
                            }
                        }
                        PayloadSourceOp::WriteStdoutHex(encoded) => {
                            if self.write_payload_source_stdout_hex(encoded).is_none() {
                                return PayloadSourceRunOutcome::Complete(
                                    PayloadLaunchResult::Failed,
                                );
                            }
                        }
                        PayloadSourceOp::WriteStderrHex(encoded) => {
                            if self.write_payload_source_stderr_hex(encoded).is_none() {
                                return PayloadSourceRunOutcome::Complete(
                                    PayloadLaunchResult::Failed,
                                );
                            }
                        }
                        PayloadSourceOp::WriteVfsFile(path) => {
                            if self.write_vfs_file_path(path).is_err() {
                                return PayloadSourceRunOutcome::Complete(
                                    PayloadLaunchResult::Failed,
                                );
                            }
                        }
                    }
                    offset = next;
                }
            }
        }
    }

    fn write_payload_source_stdout_hex(&self, encoded: &[u8]) -> Option<()> {
        self.write_payload_source_hex(encoded, false)
    }

    fn write_payload_source_stderr_hex(&self, encoded: &[u8]) -> Option<()> {
        self.write_payload_source_hex(encoded, true)
    }

    fn write_payload_source_hex(&self, encoded: &[u8], stderr: bool) -> Option<()> {
        if encoded.len() % 2 != 0 {
            return None;
        }

        let mut index = 0usize;
        while index < encoded.len() {
            let high = parse_payload_source_hex_digit(encoded[index])?;
            let low = parse_payload_source_hex_digit(encoded[index + 1])?;
            let byte = [(high << 4) | low];
            if stderr {
                self.stderr_bytes(&byte);
            } else {
                self.stdout_bytes(&byte);
            }
            index += 2;
        }

        Some(())
    }

    fn payload_source_hex_to_stdin(
        encoded: &[u8],
    ) -> Option<([u8; MAX_PROGRAM_STDIN_BYTES], usize)> {
        if encoded.len() % 2 != 0 || encoded.len() > MAX_PROGRAM_STDIN_BYTES * 2 {
            return None;
        }

        let mut bytes = [0u8; MAX_PROGRAM_STDIN_BYTES];
        let mut index = 0usize;
        let mut len = 0usize;
        while index < encoded.len() {
            let high = parse_payload_source_hex_digit(encoded[index])?;
            let low = parse_payload_source_hex_digit(encoded[index + 1])?;
            bytes[len] = (high << 4) | low;
            len += 1;
            index += 2;
        }

        Some((bytes, len))
    }

    fn run_selected_program_child_with_capture(
        &mut self,
        ctx: SyscallContext,
        stdout_capture: Option<&ProgramStdoutCapture>,
    ) -> Result<ProgramStatus, ProgramExecError> {
        let Some(pending) = take_pending_exec(ctx.pid) else {
            exit_current(ctx, ProgramStatus::Error);
            append_program_exec_log(ctx, ProgramStatus::Error);
            return Err(ProgramExecError::MissingProgramImage);
        };
        Ok(self.run_pending_program_with_capture(ctx, pending, stdout_capture))
    }

    fn run_pending_program(
        &mut self,
        ctx: SyscallContext,
        pending: exec::PendingProgramInvocation,
    ) -> ProgramStatus {
        self.run_pending_program_with_capture(ctx, pending, None)
    }

    fn run_pending_program_with_capture(
        &mut self,
        ctx: SyscallContext,
        pending: exec::PendingProgramInvocation,
        stdout_capture: Option<&ProgramStdoutCapture>,
    ) -> ProgramStatus {
        let pending_ctx = SyscallContext::from_process(pending.handle());
        let status = execute_loaded_program_argv(
            self.daemon,
            self.session,
            pending.program(),
            pending.argv(),
            pending.env(),
            pending.stdin(),
            stdout_capture,
            Some(pending_ctx),
        )
        .status();
        if matches!(status, ProgramStatus::Replaced | ProgramStatus::Blocked) {
            return status;
        }
        exit_current(ctx, status);
        append_program_exec_log(ctx, status);
        status
    }

    fn run_selected_non_wait_child(&mut self, ctx: SyscallContext) -> Option<ProgramStatus> {
        if let Some(pending) = take_pending_exec(ctx.pid) {
            return Some(self.run_pending_program(ctx, pending));
        }

        if let Some(pending) = exec::take_pending_payload(ctx.pid) {
            self.run_selected_pending_payload_invocation(pending);
            return None;
        }

        if let Some(ret) = resume_syscall_continuation(self.daemon, self.session, ctx) {
            match ret.decode() {
                Ok(_) => {
                    if let Some(status) =
                        program::resume_bin_uapi_frame(self.daemon, self.session, ctx, ret)
                    {
                        if !matches!(status, ProgramStatus::Replaced | ProgramStatus::Blocked) {
                            exit_current(ctx, status);
                            append_program_exec_log(ctx, status);
                        }
                        return Some(status);
                    }
                    return None;
                }
                Err(SyscallError::BUSY) => return None,
                Err(_) => {
                    exit_current(ctx, ProgramStatus::Error);
                    append_program_exec_log(ctx, ProgramStatus::Error);
                    return Some(ProgramStatus::Error);
                }
            }
        }

        exit_current(ctx, ProgramStatus::Error);
        append_program_exec_log(ctx, ProgramStatus::Error);
        Some(ProgramStatus::Error)
    }

    fn run_selected_pending_payload_invocation(
        &mut self,
        pending: exec::PendingPayloadInvocation,
    ) -> PayloadSourceRunOutcome {
        let parent = SyscallContext::from_process(pending.parent());
        let child = pending.child();
        let outcome = self.run_pending_payload(parent, child, pending.payload());
        let child_ctx = SyscallContext::from_process(child);
        let PayloadSourceRunOutcome::Complete(result) = outcome else {
            return PayloadSourceRunOutcome::Blocked;
        };
        if result == PayloadLaunchResult::Resident {
            append_payload_resident(child);
            wake_wait_ready_continuation_for_child(child.pid);
            return PayloadSourceRunOutcome::Complete(result);
        }
        let adoption = complete_payload_process(child.pid, result);
        record_terminal_service_for_payload(child_ctx, result);
        record_context(child_ctx, SyscallOp::ProcessExit, payload_syscall_status(result));
        append_process_adoptions(child_ctx, adoption);
        append_payload_exit(child, result);
        let _ = self.try_finish_payload_wait(parent, child);
        PayloadSourceRunOutcome::Complete(result)
    }

    fn finish_program_wait(&self, parent: SyscallContext, child: ProcessHandle) {
        let _ = self.finish_wait_by_pid(parent, child);
    }

    fn finish_wait_by_pid(
        &self,
        parent: SyscallContext,
        child: ProcessHandle,
    ) -> Result<WaitRecord, ProgramProcessError> {
        if let Some(wait) = proc::finish_wait(parent.pid, child.pid) {
            record_context(parent, SyscallOp::WaitEnd, SyscallStatus::Ok);
            append_wait_end(parent, child, wait);
            Ok(wait)
        } else if let Some(wait) = proc::wait_record(parent.pid, child.pid) {
            if wait.completed {
                record_context(parent, SyscallOp::WaitEnd, SyscallStatus::Ok);
                append_wait_end(parent, child, wait);
                let _ = proc::run_process(parent.pid);
                Ok(wait)
            } else {
                record_context(parent, SyscallOp::WaitEnd, SyscallStatus::Error);
                Err(ProgramProcessError::WaitFailed)
            }
        } else {
            record_context(parent, SyscallOp::WaitEnd, SyscallStatus::Error);
            Err(ProgramProcessError::WaitFailed)
        }
    }

    fn finish_ready_wait_by_pid(
        &self,
        parent: SyscallContext,
        child: ProcessHandle,
    ) -> Result<WaitRecord, ProgramProcessError> {
        let child_record = proc::process(child.pid).unwrap_or(proc::EMPTY_PROCESS_RECORD);
        if !process_is_service_ready_blocked(child_record) {
            record_context(parent, SyscallOp::WaitReady, SyscallStatus::Error);
            return Err(ProgramProcessError::WaitFailed);
        }
        if proc::cancel_wait(parent.pid, child.pid).is_none() {
            record_context(parent, SyscallOp::WaitReady, SyscallStatus::Error);
            return Err(ProgramProcessError::WaitFailed);
        }
        record_context(parent, SyscallOp::WaitReady, SyscallStatus::Ok);
        append_payload_wait_ready(parent, child);
        Ok(WaitRecord {
            parent_pid: parent.pid,
            child_pid: child.pid,
            child_state: child_record.state,
            exit_code: child_record.exit_code,
            completed: false,
        })
    }

    fn finish_payload_wait(&self, parent: SyscallContext, child: ProcessHandle) {
        if !self.try_finish_payload_wait(parent, child) {
            record_context(parent, SyscallOp::WaitEnd, SyscallStatus::Error);
        }
    }

    fn finish_payload_readiness_wait(&self, parent: SyscallContext, child: ProcessHandle) {
        if proc::cancel_wait(parent.pid, child.pid).is_some() {
            record_context(parent, SyscallOp::WaitReady, SyscallStatus::Ok);
            append_payload_wait_ready(parent, child);
        } else {
            record_context(parent, SyscallOp::WaitReady, SyscallStatus::Error);
        }
    }

    fn try_finish_payload_wait(&self, parent: SyscallContext, child: ProcessHandle) -> bool {
        if let Some(wait) = proc::finish_wait(parent.pid, child.pid) {
            record_context(parent, SyscallOp::WaitEnd, SyscallStatus::Ok);
            append_payload_wait_end(parent, child, wait);
            true
        } else if let Some(wait) = proc::wait_record(parent.pid, child.pid) {
            if wait.completed {
                record_context(parent, SyscallOp::WaitEnd, SyscallStatus::Ok);
                append_payload_wait_end(parent, child, wait);
                let _ = proc::run_process(parent.pid);
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Runs a lower-provider hardware probe, if installed.
    pub fn run_hardware_probe(&self, target: &str) -> Option<HardwareProbeResult> {
        if !self.direct_backend_report_admitted(SyscallOp::ProviderProbe) {
            return None;
        }
        let result = self.daemon.run_hardware_probe(target);
        let status = match result {
            Some(HardwareProbeResult::Handled) => SyscallStatus::Ok,
            Some(HardwareProbeResult::UnknownTarget) => SyscallStatus::Error,
            None => SyscallStatus::Unavailable,
        };
        self.record(SyscallOp::ProviderProbe, status);
        result
    }

    /// Writes the provider probe catalog in `/boot/probes` format.
    pub fn write_probe_catalog(&self) {
        if !self.direct_backend_report_admitted(SyscallOp::ProviderProbe) {
            return;
        }
        match self.run_hardware_probe("help") {
            Some(HardwareProbeResult::Handled) => {}
            Some(HardwareProbeResult::UnknownTarget) => {
                self.stdout_line("probe targets:");
                self.stdout_line("  unknown");
            }
            None => {
                self.stdout_line("probe targets:");
                self.stdout_line("  none");
            }
        }
    }

    /// Writes retained kernel log bytes to stdout.
    pub fn write_kernel_log(&self) -> bool {
        if !self.direct_backend_report_admitted(SyscallOp::KernelLogRead) {
            return false;
        }
        let wrote = klog::write_to(|bytes| self.stdout_bytes(bytes));
        self.record(SyscallOp::KernelLogRead, SyscallStatus::Ok);
        wrote
    }

    /// Writes the operator dmesg view to stdout.
    pub fn write_kernel_log_view(&self) {
        if !self.direct_backend_report_admitted(SyscallOp::KernelLogRead) {
            return;
        }
        self.stdout_line("dmesg:");
        let wrote_kernel_log = self.write_kernel_log();
        if let Some(snapshot) = self.external_dmesg() {
            if wrote_kernel_log {
                self.stdout_line("external diagnostics:");
            }
            self.stdout_line(snapshot);
        } else if !wrote_kernel_log {
            self.stdout_line("(kernel log empty)");
        }
    }

    /// Returns retained kernel-log metadata.
    #[must_use]
    pub fn kernel_log_stats(&self) -> klog::Stats {
        if !self.direct_backend_report_admitted(SyscallOp::KernelLogStats) {
            return blocked_kernel_log_stats();
        }
        match self.dispatch(SyscallRequest::KernelLogStats) {
            SyscallResult::KernelLogStats(stats) => stats,
            _ => klog::stats(),
        }
    }

    /// Writes retained kernel-log metadata to stdout.
    pub fn write_kernel_log_stats(&self) {
        if !self.direct_backend_report_admitted(SyscallOp::KernelLogStats) {
            return;
        }
        let stats = self.kernel_log_stats();
        self.stdout_bytes(b"boot_id=");
        self.write_u64_dec(stats.identity.boot_id as u64);
        self.stdout_bytes(b"\nsession_id=");
        self.write_u64_dec(stats.identity.session_id as u64);
        self.stdout_bytes(b"\nidentity_source=");
        self.stdout_bytes(stats.identity.identity_source.as_bytes());
        self.stdout_bytes(b"\ncapacity_bytes=");
        self.write_u64_dec(stats.capacity_bytes as u64);
        self.stdout_bytes(b"\nretained_bytes=");
        self.write_u64_dec(stats.retained_bytes as u64);
        self.stdout_bytes(b"\ndropped_bytes=");
        self.write_u64_dec(stats.dropped_bytes as u64);
        self.stdout_bytes(b"\nnext_event_seq=");
        self.write_u64_dec(stats.next_event_seq as u64);
        self.stdout_bytes(b"\nretained_events=");
        self.write_u64_dec(stats.retained_events as u64);
        self.stdout_bytes(b"\n");
    }

    /// Copies retained structured kernel events into `out`, returning the count copied.
    pub fn snapshot_kernel_events(&self, out: &mut [klog::EventRecord]) -> usize {
        if !self.direct_backend_report_admitted(SyscallOp::SnapshotKernelEvents) {
            return 0;
        }
        let count = klog::snapshot_events(out);
        self.record(SyscallOp::SnapshotKernelEvents, SyscallStatus::Ok);
        count
    }

    /// Writes retained structured kernel events to stdout.
    pub fn write_kernel_event_table(&self) {
        if !self.direct_backend_report_admitted(SyscallOp::SnapshotKernelEvents) {
            return;
        }
        let mut records = [klog::EMPTY_EVENT_RECORD; klog::MAX_EVENTS];
        let count = klog::snapshot_events(&mut records);
        self.record(SyscallOp::SnapshotKernelEvents, SyscallStatus::Ok);
        self.stdout_line("events:");
        let mut index = 0usize;
        while index < count {
            let record = records[index];
            self.stdout_bytes(b"- seq=");
            self.write_u64_dec(record.seq as u64);
            self.stdout_bytes(b" boot=");
            self.write_u64_dec(record.boot_id as u64);
            self.stdout_bytes(b" session=");
            self.write_u64_dec(record.session_id as u64);
            self.stdout_bytes(b" source=");
            self.stdout_bytes(record.source.as_bytes());
            self.stdout_bytes(b" component=");
            self.stdout_bytes(record.component.as_bytes());
            self.stdout_bytes(b" severity=");
            self.stdout_bytes(record.severity.as_bytes());
            self.stdout_bytes(b" kind=");
            self.stdout_bytes(record.kind.as_bytes());
            self.stdout_bytes(b" pid=");
            self.write_u64_dec(record.process_id as u64);
            self.stdout_bytes(b" task=");
            self.write_u64_dec(record.task_id as u64);
            self.stdout_bytes(b"\n");
            index += 1;
        }
    }

    /// Returns the optional extra diagnostics snapshot.
    #[must_use]
    pub fn external_dmesg(&self) -> Option<&'static str> {
        if !self.direct_backend_report_admitted(SyscallOp::KernelLogRead) {
            return None;
        }
        self.daemon.dmesg_fn().map(|snapshot| snapshot())
    }

    /// Returns live dump status.
    #[must_use]
    pub fn dump_status(&self) -> dump::DumpStatus {
        if !self.direct_backend_report_admitted(SyscallOp::DumpStatus) {
            return blocked_dump_status();
        }
        match self.dispatch(SyscallRequest::DumpStatus) {
            SyscallResult::DumpStatus(status) => status,
            _ => dump::status(),
        }
    }

    /// Attempts to flush retained dump state to persistent storage.
    #[must_use]
    pub fn dump_sync(&self) -> dump::DumpSyncStatus {
        if !self.direct_backend_report_admitted(SyscallOp::DumpSync) {
            return blocked_dump_sync_status();
        }
        match self.dispatch(SyscallRequest::DumpSync) {
            SyscallResult::DumpSync(status) => status,
            _ => dump::sync(),
        }
    }

    /// Writes live dump status to stdout.
    pub fn write_dump_status(&self) {
        if !self.direct_backend_report_admitted(SyscallOp::DumpStatus) {
            return;
        }
        let status = self.dump_status();
        self.stdout_bytes(b"format=reovim-dump-v");
        self.write_u64_dec(status.format_version as u64);
        self.stdout_bytes(b"\nboot_id=");
        self.write_u64_dec(status.identity.boot_id as u64);
        self.stdout_bytes(b"\nsession_id=");
        self.write_u64_dec(status.identity.session_id as u64);
        self.stdout_bytes(b"\nidentity_source=");
        self.stdout_bytes(status.identity.identity_source.as_bytes());
        self.stdout_bytes(b"\npackage=");
        self.stdout_bytes(status.image.package.as_bytes());
        self.stdout_bytes(b"\nversion=");
        self.stdout_bytes(status.image.version.as_bytes());
        self.stdout_bytes(b"\ntarget=");
        self.stdout_bytes(status.image.target.as_bytes());
        self.stdout_bytes(b"\nselected_profile=");
        self.stdout_bytes(status.image.selected_profile.as_bytes());
        self.stdout_bytes(b"\nprofile_request=");
        self.stdout_bytes(status.image.profile_request.as_bytes());
        self.stdout_bytes(b"\nbootline=");
        self.stdout_bytes(status.image.bootline.as_bytes());
        self.stdout_bytes(b"\nlaunch_profile_feature=");
        self.stdout_bytes(status.image.launch_profile_feature.as_bytes());
        self.stdout_bytes(b"\nboot_memory_ranges=");
        self.write_u64_dec(status.boot_info.memory.range_count() as u64);
        self.stdout_bytes(b"\nboot_memory_usable_bytes=");
        self.write_u64_dec(status.boot_info.memory.usable_bytes());
        self.stdout_bytes(b"\nboot_cpu_count=");
        self.write_u64_dec(status.boot_info.cpu_count as u64);
        self.stdout_bytes(b"\nboot_heap_total_bytes=");
        self.write_u64_dec(status.boot_info.heap_total_bytes);
        self.stdout_bytes(b"\ndevice_records=");
        self.write_u64_dec(status.device_records as u64);
        self.stdout_bytes(b"\nproof_state=");
        self.stdout_bytes(status.proof_state.as_bytes());
        self.stdout_bytes(b"\npanic_state=");
        self.stdout_bytes(status.panic_state.as_bytes());
        self.stdout_bytes(b"\npanic_records=");
        self.write_u64_dec(status.panic_records as u64);
        self.stdout_bytes(b"\npersistent=");
        self.stdout_bytes(if status.persistent_available {
            b"available"
        } else {
            b"unavailable"
        });
        self.stdout_bytes(b"\nstorage=");
        self.stdout_bytes(status.storage.as_bytes());
        self.stdout_bytes(b"\nstorage_capacity_bytes=");
        self.write_u64_dec(status.storage_capacity_bytes as u64);
        self.stdout_bytes(b"\nlast_sync_attempted=");
        self.stdout_bytes(if status.last_sync.attempted {
            b"true"
        } else {
            b"false"
        });
        self.stdout_bytes(b"\nlast_sync_persistent=");
        self.stdout_bytes(if status.last_sync.persistent_available {
            b"available"
        } else {
            b"unavailable"
        });
        self.stdout_bytes(b"\nlast_sync_storage=");
        self.stdout_bytes(status.last_sync.storage.as_bytes());
        self.stdout_bytes(b"\nlast_sync_storage_capacity_bytes=");
        self.write_u64_dec(status.last_sync.storage_capacity_bytes as u64);
        self.stdout_bytes(b"\nlast_sync_status=");
        self.stdout_bytes(if status.last_sync.written {
            b"written"
        } else {
            b"not-written"
        });
        self.stdout_bytes(b"\nlast_sync_bytes=");
        self.write_u64_dec(status.last_sync.bytes_written as u64);
        self.stdout_bytes(b"\nlast_sync_checksum=");
        self.write_u64_dec(status.last_sync.checksum as u64);
        self.stdout_bytes(b"\nlast_sync_verified=");
        self.stdout_bytes(if status.last_sync.verified {
            b"true"
        } else {
            b"false"
        });
        self.stdout_bytes(b"\nlast_sync_reason=");
        self.stdout_bytes(status.last_sync.reason.as_bytes());
        self.stdout_bytes(b"\nklog_retained_bytes=");
        self.write_u64_dec(status.klog.retained_bytes as u64);
        self.stdout_bytes(b"\nklog_dropped_bytes=");
        self.write_u64_dec(status.klog.dropped_bytes as u64);
        self.stdout_bytes(b"\nklog_next_event_seq=");
        self.write_u64_dec(status.klog.next_event_seq as u64);
        self.stdout_bytes(b"\nevent_records=");
        self.write_u64_dec(status.event_records as u64);
        self.stdout_bytes(b"\nprocess_records=");
        self.write_u64_dec(status.process_records as u64);
        self.stdout_bytes(b"\nservice_records=");
        self.write_u64_dec(status.service_records as u64);
        self.stdout_bytes(b"\nexec_load_records=");
        self.write_u64_dec(status.exec_load_records as u64);
        self.stdout_bytes(b"\npending_exec_records=");
        self.write_u64_dec(status.pending_exec_records as u64);
        self.stdout_bytes(b"\nwait_records=");
        self.write_u64_dec(status.wait_records as u64);
        self.stdout_bytes(b"\ntask_records=");
        self.write_u64_dec(status.task_records as u64);
        self.stdout_bytes(b"\nsyscall_records=");
        self.write_u64_dec(status.syscall_records as u64);
        self.stdout_bytes(b"\nsyscall_continuation_records=");
        self.write_u64_dec(status.syscall_continuation_records as u64);
        self.stdout_bytes(b"\n");
    }

    /// Writes a bounded dump snapshot to stdout.
    pub fn write_dump_snapshot(&self) {
        if !self.direct_backend_report_admitted(SyscallOp::DumpStatus) {
            return;
        }
        self.stdout_line("dump:");
        let status = self.dump_status();
        match dump::encode_status_artifact(status) {
            Ok(artifact) => self.stdout_bytes(artifact.as_bytes()),
            Err(_) => self.stdout_line("snapshot=encode-error"),
        }
    }

    /// Attempts to flush retained dump state and writes the sync report.
    pub fn write_dump_sync(&self) {
        if !self.direct_backend_report_admitted(SyscallOp::DumpSync) {
            return;
        }
        let status = self.dump_sync();
        self.stdout_line("dump sync:");
        self.stdout_bytes(b"persistent=");
        self.stdout_bytes(if status.persistent_available {
            b"available"
        } else {
            b"unavailable"
        });
        self.stdout_bytes(b"\nattempted=");
        self.stdout_bytes(if status.attempted { b"true" } else { b"false" });
        self.stdout_bytes(b"\nstorage=");
        self.stdout_bytes(status.storage.as_bytes());
        self.stdout_bytes(b"\nstorage_capacity_bytes=");
        self.write_u64_dec(status.storage_capacity_bytes as u64);
        self.stdout_bytes(b"\nstatus=");
        self.stdout_bytes(if status.written {
            b"written"
        } else {
            b"not-written"
        });
        if status.persistent_available {
            self.stdout_bytes(b"\nbytes=");
            self.write_u64_dec(status.bytes_written as u64);
            self.stdout_bytes(b"\nchecksum=");
            self.write_u64_dec(status.checksum as u64);
            self.stdout_bytes(b"\nverified=");
            self.stdout_bytes(if status.verified { b"true" } else { b"false" });
        }
        self.stdout_bytes(b"\nreason=");
        self.stdout_bytes(status.reason.as_bytes());
        self.stdout_bytes(b"\n");
    }

    /// Copies retained process records into `out`, returning the count copied.
    pub fn snapshot_processes(&self, out: &mut [ProcessRecord]) -> usize {
        if !self.direct_backend_report_admitted(SyscallOp::SnapshotProcesses) {
            return 0;
        }
        let count = proc::snapshot(out);
        self.record(SyscallOp::SnapshotProcesses, SyscallStatus::Ok);
        count
    }

    /// Copies retained init-service records into `out`, returning the count copied.
    pub fn snapshot_services(&self, out: &mut [ServiceRecord]) -> usize {
        if !self.direct_backend_report_admitted(SyscallOp::SnapshotServices) {
            return 0;
        }
        let count = service::snapshot(out);
        self.record(SyscallOp::SnapshotServices, SyscallStatus::Ok);
        count
    }

    /// Copies retained executable load/admission records into `out`.
    pub fn snapshot_exec_loads(&self, out: &mut [exec::ExecLoadRecord]) -> usize {
        if !self.direct_backend_report_admitted(SyscallOp::SnapshotExecLoads) {
            return 0;
        }
        let count = exec::snapshot_loads(out);
        self.record(SyscallOp::SnapshotExecLoads, SyscallStatus::Ok);
        count
    }

    /// Copies retained address-space records into `out`.
    pub fn snapshot_address_spaces(&self, out: &mut [mm::AddressSpaceRecord]) -> usize {
        if !self.direct_backend_report_admitted(SyscallOp::SnapshotAddressSpaces) {
            return 0;
        }
        let count = mm::snapshot_address_spaces(out);
        self.record(SyscallOp::SnapshotAddressSpaces, SyscallStatus::Ok);
        count
    }

    /// Copies retained address-space page-table records into `out`.
    pub fn snapshot_address_space_page_tables(
        &self,
        out: &mut [mm::AddressSpacePageTableRecord],
    ) -> usize {
        if !self.direct_backend_report_admitted(SyscallOp::SnapshotAddressSpacePageTables) {
            return 0;
        }
        let count = mm::snapshot_address_space_page_tables(out);
        self.record(SyscallOp::SnapshotAddressSpacePageTables, SyscallStatus::Ok);
        count
    }

    /// Copies retained address-space page-entry records into `out`.
    pub fn snapshot_address_space_pages(&self, out: &mut [mm::AddressSpacePageRecord]) -> usize {
        if !self.direct_backend_report_admitted(SyscallOp::SnapshotAddressSpacePages) {
            return 0;
        }
        let count = mm::snapshot_address_space_pages(out);
        self.record(SyscallOp::SnapshotAddressSpacePages, SyscallStatus::Ok);
        count
    }

    /// Copies retained address-space memory-object records into `out`.
    pub fn snapshot_address_space_objects(
        &self,
        out: &mut [mm::AddressSpaceObjectRecord],
    ) -> usize {
        if !self.direct_backend_report_admitted(SyscallOp::SnapshotAddressSpaceObjects) {
            return 0;
        }
        let count = mm::snapshot_address_space_objects(out);
        self.record(SyscallOp::SnapshotAddressSpaceObjects, SyscallStatus::Ok);
        count
    }

    /// Copies retained pending executable invocation records into `out`.
    pub fn snapshot_pending_execs(&self, out: &mut [exec::PendingExecRecord]) -> usize {
        if !self.direct_backend_report_admitted(SyscallOp::SnapshotPendingExecs) {
            return 0;
        }
        let count = exec::snapshot_pending(out);
        self.record(SyscallOp::SnapshotPendingExecs, SyscallStatus::Ok);
        count
    }

    /// Copies executable source artifact records into `out`.
    pub fn snapshot_source_store(&self, out: &mut [SourceArtifactRecord]) -> usize {
        if !self.direct_backend_report_admitted(SyscallOp::SnapshotSourceStore) {
            return 0;
        }
        let count = self.daemon.source_store().snapshot(out);
        self.record(SyscallOp::SnapshotSourceStore, SyscallStatus::Ok);
        count
    }

    /// Copies retained wait records into `out`, returning the count copied.
    pub fn snapshot_waits(&self, out: &mut [WaitRecord]) -> usize {
        if !self.direct_backend_report_admitted(SyscallOp::SnapshotWaits) {
            return 0;
        }
        let count = proc::snapshot_waits(out);
        self.record(SyscallOp::SnapshotWaits, SyscallStatus::Ok);
        count
    }

    /// Writes retained process records to stdout.
    pub fn write_process_table(&self) {
        if !self.direct_backend_report_admitted(SyscallOp::SnapshotProcesses) {
            return;
        }
        let mut records = [proc::EMPTY_PROCESS_RECORD; proc::MAX_PROCESSES];
        let count = self.snapshot_processes(&mut records);
        self.stdout_line("processes:");
        let mut index = 0usize;
        while index < count {
            self.write_process_record(records[index]);
            index += 1;
        }
    }

    /// Writes retained service records to stdout.
    pub fn write_service_table(&self) {
        if !self.direct_backend_report_admitted(SyscallOp::SnapshotServices) {
            return;
        }
        let mut records = [EMPTY_SERVICE_RECORD; service::MAX_SERVICES];
        let count = self.snapshot_services(&mut records);
        self.stdout_line("services:");
        let mut index = 0usize;
        while index < count {
            let record = records[index];
            self.stdout_bytes(b"- seq=");
            self.write_u64_dec(record.seq as u64);
            self.stdout_bytes(b" name=");
            self.stdout_bytes(record.name.as_bytes());
            self.stdout_bytes(b" target=");
            self.stdout_bytes(record.target.as_bytes());
            self.stdout_bytes(b" state=");
            self.stdout_bytes(record.state.as_str().as_bytes());
            self.stdout_bytes(b" reason=");
            self.stdout_bytes(record.reason.as_str().as_bytes());
            self.stdout_bytes(b" owner_pid=");
            self.write_u64_dec(record.owner_pid as u64);
            self.stdout_bytes(b" owner_task=");
            self.write_u64_dec(record.owner_task_id as u64);
            self.stdout_bytes(b" service_pid=");
            self.write_u64_dec(record.service_pid as u64);
            self.stdout_bytes(b" service_task=");
            self.write_u64_dec(record.service_task_id as u64);
            self.stdout_bytes(b"\n");
            index += 1;
        }
    }

    /// Writes the current process record to stdout.
    pub fn write_current_process(&self) {
        if !self.direct_backend_report_admitted(SyscallOp::ProcessSelf) {
            return;
        }
        self.stdout_line("self:");
        let Some(record) = self.process_self() else {
            self.stderr_line("proc: current process unavailable");
            return;
        };
        self.write_process_record(record);
    }

    /// Writes the active interactive shell/session state to stdout.
    pub fn write_session_state(&self) {
        if !self.direct_backend_report_admitted(SyscallOp::SnapshotSession) {
            return;
        }
        self.record(SyscallOp::SnapshotSession, SyscallStatus::Ok);
        let shell = proc::process(proc::SHELL_PID);
        let input = self.console_input();
        self.stdout_line("session:");
        self.stdout_bytes(b"owner_path=");
        self.stdout_bytes(self.session.owner_path().as_bytes());
        self.stdout_bytes(b"\nowner_loader=");
        self.stdout_bytes(self.session.owner_loader().as_bytes());
        self.stdout_bytes(b"\nowner_entry_fn=");
        self.stdout_bytes(self.session.owner_entry_name().as_bytes());
        self.stdout_bytes(b"\nowner_pid=");
        self.write_u64_dec(shell.map_or(0, |record| record.pid) as u64);
        self.stdout_bytes(b"\nowner_task=");
        self.write_u64_dec(shell.map_or(0, |record| record.task_id) as u64);
        self.stdout_bytes(b"\ncwd=");
        self.stdout_bytes(self.session.cwd().as_bytes());
        self.stdout_bytes(b"\nshell_target=");
        self.stdout_bytes(
            self.session
                .shell_target_requested()
                .unwrap_or("none")
                .as_bytes(),
        );
        self.stdout_bytes(b"\nshell_started=");
        self.write_true_false_word(self.session.shell_start_requested());
        self.stdout_bytes(b"\nline_discipline=");
        self.stdout_bytes(self.session.line_discipline().as_bytes());
        self.stdout_bytes(b"\npipe_mode=");
        self.stdout_bytes(self.session.pipe_mode().as_bytes());
        self.stdout_bytes(b"\nline_loop_host=rootd\nprompt=");
        self.stdout_bytes(self.prompt().as_bytes());
        self.stdout_bytes(b"\ninput=");
        self.stdout_bytes(input.source.as_bytes());
        self.stdout_bytes(b"\ninput_mode=");
        self.stdout_bytes(input.mode.as_bytes());
        self.stdout_bytes(b"\nlaunch=");
        self.write_enabled_disabled(self.launch_enabled());
        self.stdout_bytes(b"\n");
    }

    /// Writes retained executable load/admission records to stdout.
    pub fn write_exec_load_table(&self) {
        if !self.direct_backend_report_admitted(SyscallOp::SnapshotExecLoads) {
            return;
        }
        let mut records = [exec::EMPTY_EXEC_LOAD_RECORD; exec::MAX_EXEC_LOAD_RECORDS];
        let count = self.snapshot_exec_loads(&mut records);
        self.stdout_line("execs:");
        let mut index = 0usize;
        while index < count {
            let record = records[index];
            self.stdout_bytes(b"- seq=");
            self.write_u64_dec(record.seq as u64);
            self.stdout_bytes(b" argv0=");
            self.stdout_bytes(record.argv0().as_bytes());
            self.stdout_bytes(b" status=");
            self.stdout_bytes(record.status.as_str().as_bytes());
            self.stdout_bytes(b" reason=");
            self.stdout_bytes(record.reason.as_str().as_bytes());
            self.stdout_bytes(b" path=");
            self.stdout_bytes(record.path.as_bytes());
            self.stdout_bytes(b" source=");
            self.stdout_bytes(record.source_path.as_bytes());
            self.stdout_bytes(b" loader=");
            self.stdout_bytes(record.loader.as_bytes());
            self.stdout_bytes(b" entry_fn=");
            self.stdout_bytes(record.entry_name.as_bytes());
            self.stdout_bytes(b" truncated=");
            self.write_true_false_word(record.argv0_truncated);
            self.stdout_bytes(b" kind=");
            self.stdout_bytes(record.kind.as_str().as_bytes());
            self.stdout_bytes(b" origin=");
            self.stdout_bytes(record.origin.as_str().as_bytes());
            self.stdout_bytes(b" artifact_format=");
            self.stdout_bytes(record.artifact_format.as_str().as_bytes());
            self.stdout_bytes(b" artifact_body_format=");
            self.stdout_bytes(record.artifact_body_format.as_str().as_bytes());
            self.stdout_bytes(b" artifact_body_inner=");
            self.stdout_bytes(record.artifact_body_inner_format.as_str().as_bytes());
            self.stdout_bytes(b" artifact_bytes=");
            self.write_u64_dec(record.artifact_bytes_len as u64);
            self.stdout_bytes(b" artifact_body_bytes=");
            self.write_u64_dec(record.artifact_body_bytes_len as u64);
            self.stdout_bytes(b" artifact_checksum=");
            self.write_u64_dec(record.artifact_checksum as u64);
            self.stdout_bytes(b"\n");
            index += 1;
        }
    }

    /// Writes retained address-space lifecycle records to stdout.
    pub fn write_address_space_table(&self) {
        if !self.direct_backend_report_admitted(SyscallOp::SnapshotAddressSpaces) {
            return;
        }
        let mut records = [mm::EMPTY_ADDRESS_SPACE_RECORD; mm::MAX_ADDRESS_SPACE_RECORDS];
        let count = self.snapshot_address_spaces(&mut records);
        self.stdout_line("address-spaces:");
        let mut index = 0usize;
        while index < count {
            let record = records[index];
            self.stdout_bytes(b"- id=");
            self.write_u64_dec(record.id as u64);
            self.stdout_bytes(b" owner_pid=");
            self.write_u64_dec(record.owner_pid as u64);
            self.stdout_bytes(b" image_generation=");
            self.write_u64_dec(record.image_generation as u64);
            self.stdout_bytes(b" state=");
            self.stdout_bytes(record.state.as_str().as_bytes());
            self.stdout_bytes(b" path=");
            self.stdout_bytes(record.program_path.as_bytes());
            self.stdout_bytes(b" loader=");
            self.stdout_bytes(record.loader.as_bytes());
            self.stdout_bytes(b" entry_fn=");
            self.stdout_bytes(record.entry_name.as_bytes());
            self.stdout_bytes(b" source=");
            self.stdout_bytes(record.source_path.as_bytes());
            self.stdout_bytes(b" text_bytes=");
            self.write_u64_dec(record.text_bytes as u64);
            self.stdout_bytes(b" text_checksum=");
            self.write_u64_dec(record.text_checksum as u64);
            self.stdout_bytes(b" stack_bytes=");
            self.write_u64_dec(record.stack_bytes as u64);
            self.stdout_bytes(b" regions=");
            self.write_u64_dec(record.region_count as u64);
            self.stdout_bytes(b" text_start=");
            self.write_u64_dec(record.text_start as u64);
            self.stdout_bytes(b" text_end=");
            self.write_u64_dec(record.text_end as u64);
            self.stdout_bytes(b" text_flags=");
            self.write_u64_dec(record.text_flags as u64);
            self.stdout_bytes(b" stack_start=");
            self.write_u64_dec(record.stack_start as u64);
            self.stdout_bytes(b" stack_end=");
            self.write_u64_dec(record.stack_end as u64);
            self.stdout_bytes(b" stack_flags=");
            self.write_u64_dec(record.stack_flags as u64);
            self.stdout_bytes(b" page_table=");
            self.write_u64_dec(record.page_table_id as u64);
            self.stdout_bytes(b" mapped_pages=");
            self.write_u64_dec(record.mapped_pages as u64);
            self.stdout_bytes(b" text_pages=");
            self.write_u64_dec(record.text_pages as u64);
            self.stdout_bytes(b" stack_pages=");
            self.write_u64_dec(record.stack_pages as u64);
            self.stdout_bytes(b"\n");
            index += 1;
        }
    }

    /// Writes retained address-space page-table records to stdout.
    pub fn write_address_space_page_table_table(&self) {
        if !self.direct_backend_report_admitted(SyscallOp::SnapshotAddressSpacePageTables) {
            return;
        }
        let mut records =
            [mm::EMPTY_ADDRESS_SPACE_PAGE_TABLE_RECORD; mm::MAX_ADDRESS_SPACE_PAGE_TABLE_RECORDS];
        let count = self.snapshot_address_space_page_tables(&mut records);
        self.stdout_line("address-space-page-tables:");
        let mut index = 0usize;
        while index < count {
            let record = records[index];
            self.stdout_bytes(b"- id=");
            self.write_u64_dec(record.id as u64);
            self.stdout_bytes(b" address_space=");
            self.write_u64_dec(record.address_space_id as u64);
            self.stdout_bytes(b" owner_pid=");
            self.write_u64_dec(record.owner_pid as u64);
            self.stdout_bytes(b" image_generation=");
            self.write_u64_dec(record.image_generation as u64);
            self.stdout_bytes(b" state=");
            self.stdout_bytes(record.state.as_str().as_bytes());
            self.stdout_bytes(b" root_table=");
            self.write_u64_dec(record.root_table_id as u64);
            self.stdout_bytes(b" source=");
            self.stdout_bytes(record.source_path.as_bytes());
            self.stdout_bytes(b" mapped_pages=");
            self.write_u64_dec(record.mapped_pages as u64);
            self.stdout_bytes(b" text_pages=");
            self.write_u64_dec(record.text_pages as u64);
            self.stdout_bytes(b" stack_pages=");
            self.write_u64_dec(record.stack_pages as u64);
            self.stdout_bytes(b" text_flags=");
            self.write_u64_dec(record.text_flags as u64);
            self.stdout_bytes(b" stack_flags=");
            self.write_u64_dec(record.stack_flags as u64);
            self.stdout_bytes(b"\n");
            index += 1;
        }
    }

    /// Writes retained address-space page-entry records to stdout.
    pub fn write_address_space_page_table_entry_table(&self) {
        if !self.direct_backend_report_admitted(SyscallOp::SnapshotAddressSpacePages) {
            return;
        }
        let mut records = [mm::EMPTY_ADDRESS_SPACE_PAGE_RECORD; mm::MAX_ADDRESS_SPACE_PAGE_RECORDS];
        let count = self.snapshot_address_space_pages(&mut records);
        self.stdout_line("address-space-pages:");
        let mut index = 0usize;
        while index < count {
            let record = records[index];
            self.stdout_bytes(b"- id=");
            self.write_u64_dec(record.id as u64);
            self.stdout_bytes(b" address_space=");
            self.write_u64_dec(record.address_space_id as u64);
            self.stdout_bytes(b" page_table=");
            self.write_u64_dec(record.page_table_id as u64);
            self.stdout_bytes(b" owner_pid=");
            self.write_u64_dec(record.owner_pid as u64);
            self.stdout_bytes(b" image_generation=");
            self.write_u64_dec(record.image_generation as u64);
            self.stdout_bytes(b" state=");
            self.stdout_bytes(record.state.as_str().as_bytes());
            self.stdout_bytes(b" kind=");
            self.stdout_bytes(record.kind.as_str().as_bytes());
            self.stdout_bytes(b" backing=");
            self.stdout_bytes(record.backing.as_str().as_bytes());
            self.stdout_bytes(b" source=");
            self.stdout_bytes(record.source_path.as_bytes());
            self.stdout_bytes(b" page_index=");
            self.write_u64_dec(record.page_index as u64);
            self.stdout_bytes(b" virtual_start=");
            self.write_u64_dec(record.virtual_start as u64);
            self.stdout_bytes(b" virtual_end=");
            self.write_u64_dec(record.virtual_end as u64);
            self.stdout_bytes(b" bytes=");
            self.write_u64_dec(record.bytes as u64);
            self.stdout_bytes(b" source_offset=");
            self.write_u64_dec(record.source_offset as u64);
            self.stdout_bytes(b" flags=");
            self.write_u64_dec(record.flags as u64);
            self.stdout_bytes(b"\n");
            index += 1;
        }
    }

    /// Writes retained address-space memory-object records to stdout.
    pub fn write_address_space_object_table(&self) {
        if !self.direct_backend_report_admitted(SyscallOp::SnapshotAddressSpaceObjects) {
            return;
        }
        let mut records =
            [mm::EMPTY_ADDRESS_SPACE_OBJECT_RECORD; mm::MAX_ADDRESS_SPACE_OBJECT_RECORDS];
        let count = self.snapshot_address_space_objects(&mut records);
        self.stdout_line("address-space-objects:");
        let mut index = 0usize;
        while index < count {
            let record = records[index];
            self.stdout_bytes(b"- id=");
            self.write_u64_dec(record.id as u64);
            self.stdout_bytes(b" address_space=");
            self.write_u64_dec(record.address_space_id as u64);
            self.stdout_bytes(b" owner_pid=");
            self.write_u64_dec(record.owner_pid as u64);
            self.stdout_bytes(b" image_generation=");
            self.write_u64_dec(record.image_generation as u64);
            self.stdout_bytes(b" state=");
            self.stdout_bytes(record.state.as_str().as_bytes());
            self.stdout_bytes(b" kind=");
            self.stdout_bytes(record.kind.as_str().as_bytes());
            self.stdout_bytes(b" backing=");
            self.stdout_bytes(record.backing.as_str().as_bytes());
            self.stdout_bytes(b" source=");
            self.stdout_bytes(record.source_path.as_bytes());
            self.stdout_bytes(b" start=");
            self.write_u64_dec(record.start as u64);
            self.stdout_bytes(b" end=");
            self.write_u64_dec(record.end as u64);
            self.stdout_bytes(b" bytes=");
            self.write_u64_dec(record.bytes as u64);
            self.stdout_bytes(b" checksum=");
            self.write_u64_dec(record.checksum as u64);
            self.stdout_bytes(b" flags=");
            self.write_u64_dec(record.flags as u64);
            self.stdout_bytes(b" page_count=");
            self.write_u64_dec(record.page_count as u64);
            self.stdout_bytes(b"\n");
            index += 1;
        }
    }

    /// Writes retained pending executable invocation records to stdout.
    pub fn write_pending_exec_table(&self) {
        if !self.direct_backend_report_admitted(SyscallOp::SnapshotPendingExecs) {
            return;
        }
        let mut records = [exec::EMPTY_PENDING_EXEC_RECORD; exec::MAX_PENDING_EXEC_RECORDS];
        let count = self.snapshot_pending_execs(&mut records);
        self.stdout_line("pending:");
        let mut index = 0usize;
        while index < count {
            let record = records[index];
            self.stdout_bytes(b"- pid=");
            self.write_u64_dec(record.pid as u64);
            self.stdout_bytes(b" ppid=");
            self.write_u64_dec(record.parent_pid as u64);
            self.stdout_bytes(b" task=");
            self.write_u64_dec(record.task_id as u64);
            self.stdout_bytes(b" path=");
            self.stdout_bytes(record.path.as_bytes());
            self.stdout_bytes(b" loader=");
            self.stdout_bytes(record.loader.as_bytes());
            self.stdout_bytes(b" entry_fn=");
            self.stdout_bytes(record.entry_name.as_bytes());
            self.stdout_bytes(b" kind=");
            self.stdout_bytes(record.kind.as_str().as_bytes());
            self.stdout_bytes(b" artifact_body_format=");
            self.stdout_bytes(record.artifact_body_format.as_str().as_bytes());
            self.stdout_bytes(b" artifact_body_inner=");
            self.stdout_bytes(record.artifact_body_inner_format.as_str().as_bytes());
            self.stdout_bytes(b" stdin_bytes=");
            self.write_u64_dec(record.stdin_len as u64);
            self.stdout_bytes(b"\n");
            index += 1;
        }
    }

    /// Writes executable source artifacts to stdout.
    pub fn write_source_store_table(&self) {
        if !self.direct_backend_report_admitted(SyscallOp::SnapshotSourceStore) {
            return;
        }
        let mut records = [EMPTY_SOURCE_ARTIFACT_RECORD; MAX_SOURCE_ARTIFACT_RECORDS];
        let count = self.snapshot_source_store(&mut records);
        self.stdout_line("sources:");
        let mut index = 0usize;
        while index < count {
            let record = records[index];
            self.stdout_bytes(b"- namespace=");
            self.stdout_bytes(record.namespace.as_str().as_bytes());
            self.stdout_bytes(b" path=");
            self.stdout_bytes(record.path.as_bytes());
            self.stdout_bytes(b" loader=");
            self.stdout_bytes(record.loader.as_bytes());
            self.stdout_bytes(b" bytes=");
            self.write_u64_dec(record.bytes_len as u64);
            self.stdout_bytes(b" origin=");
            self.stdout_bytes(record.origin.as_str().as_bytes());
            self.stdout_bytes(b"\n");
            index += 1;
        }
    }

    /// Writes executable source-media status and manifest rows to stdout.
    pub fn write_source_media_table(&self) {
        if !self.direct_backend_report_admitted(SyscallOp::SourceMediaSnapshot) {
            return;
        }
        self.record(SyscallOp::SourceMediaSnapshot, SyscallStatus::Ok);
        self.stdout_line("source-media:");
        let mut bytes = [0u8; MAX_SOURCE_MEDIA_CATALOG_BYTES];
        let mut entries = [EMPTY_SOURCE_MEDIA_CATALOG_ENTRY; MAX_SOURCE_MEDIA_CATALOG_RECORDS];
        let snapshot = source_media::snapshot_root(&mut bytes, &mut entries);
        let read = match snapshot {
            source_media::SourceMediaSnapshot::Unavailable { read }
            | source_media::SourceMediaSnapshot::ReadError { read }
            | source_media::SourceMediaSnapshot::Catalog { read, .. }
            | source_media::SourceMediaSnapshot::Artifact { read, .. }
            | source_media::SourceMediaSnapshot::InvalidCatalog { read, .. }
            | source_media::SourceMediaSnapshot::InvalidArtifact { read, .. } => read,
        };
        let read_status = match snapshot {
            source_media::SourceMediaSnapshot::Unavailable { .. } => SyscallStatus::Unavailable,
            source_media::SourceMediaSnapshot::ReadError { .. } => SyscallStatus::Error,
            source_media::SourceMediaSnapshot::Catalog { .. }
            | source_media::SourceMediaSnapshot::Artifact { .. }
            | source_media::SourceMediaSnapshot::InvalidCatalog { .. }
            | source_media::SourceMediaSnapshot::InvalidArtifact { .. } => SyscallStatus::Ok,
        };
        self.record(SyscallOp::SourceMediaRead, read_status);

        self.stdout_bytes(b"storage=");
        self.stdout_bytes(read.storage.as_bytes());
        self.stdout_bytes(b"\nstorage_capacity_bytes=");
        self.write_u64_dec(read.capacity_bytes as u64);
        self.stdout_bytes(b"\nroot_bytes=");
        self.write_u64_dec(read.bytes as u64);
        match snapshot {
            source_media::SourceMediaSnapshot::Unavailable { .. } => {
                self.stdout_bytes(b"\nstatus=unavailable\nreason=source-media-unavailable\n");
                self.write_exec_bundle_table();
                return;
            }
            source_media::SourceMediaSnapshot::ReadError { read } => {
                self.stdout_bytes(b"\nstatus=read-error\nreason=");
                self.stdout_bytes(read.reason.as_bytes());
                self.stdout_bytes(b"\n");
                self.write_exec_bundle_table();
                return;
            }
            _ => {}
        }

        match snapshot {
            source_media::SourceMediaSnapshot::Catalog {
                count, truncated, ..
            } => {
                self.stdout_bytes(b"\nstatus=ok\nformat=catalog\nentries=");
                self.write_u64_dec(count as u64);
                self.stdout_bytes(b"\ntruncated=");
                self.stdout_bytes(if truncated { b"true" } else { b"false" });
                self.stdout_bytes(b"\n");
                let mut index = 0usize;
                while index < count {
                    let entry = entries[index];
                    self.stdout_bytes(b"- namespace=");
                    self.stdout_bytes(entry.namespace.as_str().as_bytes());
                    self.stdout_bytes(b" path=");
                    self.stdout_bytes(entry.path.as_bytes());
                    self.stdout_bytes(b" offset=");
                    self.write_u64_dec(entry.offset as u64);
                    self.stdout_bytes(b" artifact_bytes=");
                    self.write_u64_dec(entry.artifact_bytes_len as u64);
                    self.stdout_bytes(b" checksum=");
                    self.write_u64_dec(entry.checksum as u64);
                    self.stdout_bytes(b"\n");
                    index += 1;
                }
            }
            source_media::SourceMediaSnapshot::Artifact { artifact, .. } => {
                self.stdout_bytes(b"\nstatus=ok\nformat=artifact\nentries=1\n");
                self.stdout_bytes(b"truncated=false\n");
                self.stdout_bytes(b"- namespace=");
                self.stdout_bytes(artifact.namespace.as_str().as_bytes());
                self.stdout_bytes(b" path=");
                self.stdout_bytes(artifact.path.as_bytes());
                self.stdout_bytes(b" offset=0 artifact_bytes=");
                self.write_u64_dec(read.bytes as u64);
                self.stdout_bytes(b" bytes=");
                self.write_u64_dec(artifact.source_bytes.len() as u64);
                self.stdout_bytes(b" checksum=");
                self.write_u64_dec(artifact.checksum as u64);
                self.stdout_bytes(b"\n");
            }
            source_media::SourceMediaSnapshot::InvalidCatalog { error, .. } => {
                self.stdout_bytes(b"\nstatus=invalid\nformat=catalog\nreason=");
                self.stdout_bytes(error.as_str().as_bytes());
                self.stdout_bytes(b"\n");
            }
            source_media::SourceMediaSnapshot::InvalidArtifact { error, .. } => {
                self.stdout_bytes(b"\nstatus=invalid\nformat=artifact\nreason=");
                self.stdout_bytes(error.as_str().as_bytes());
                self.stdout_bytes(b"\n");
            }
            source_media::SourceMediaSnapshot::Unavailable { .. }
            | source_media::SourceMediaSnapshot::ReadError { .. } => {}
        }
        self.write_exec_bundle_table();
    }

    fn write_exec_bundle_table(&self) {
        if !self.direct_backend_report_admitted(SyscallOp::ExecBundleSnapshot) {
            return;
        }
        self.record(SyscallOp::ExecBundleSnapshot, SyscallStatus::Ok);
        self.stdout_line("exec-bundle:");
        let mut bytes = [0u8; MAX_SOURCE_MEDIA_CATALOG_BYTES];
        let mut entries = [exec_bundle::EMPTY_EXEC_BUNDLE_CATALOG_ENTRY;
            exec_bundle::MAX_EXEC_BUNDLE_CATALOG_RECORDS];
        let snapshot = exec_bundle::snapshot_root(&mut bytes, &mut entries);
        let read = match snapshot {
            exec_bundle::ExecBundleSnapshot::Unavailable { read }
            | exec_bundle::ExecBundleSnapshot::ReadError { read }
            | exec_bundle::ExecBundleSnapshot::Catalog { read, .. }
            | exec_bundle::ExecBundleSnapshot::Artifact { read, .. }
            | exec_bundle::ExecBundleSnapshot::InvalidCatalog { read, .. }
            | exec_bundle::ExecBundleSnapshot::InvalidArtifact { read, .. } => read,
        };
        let read_status = match snapshot {
            exec_bundle::ExecBundleSnapshot::Unavailable { .. } => SyscallStatus::Unavailable,
            exec_bundle::ExecBundleSnapshot::ReadError { .. } => SyscallStatus::Error,
            exec_bundle::ExecBundleSnapshot::Catalog { .. }
            | exec_bundle::ExecBundleSnapshot::Artifact { .. }
            | exec_bundle::ExecBundleSnapshot::InvalidCatalog { .. }
            | exec_bundle::ExecBundleSnapshot::InvalidArtifact { .. } => SyscallStatus::Ok,
        };
        self.record(SyscallOp::ExecBundleRead, read_status);

        self.stdout_bytes(b"storage=");
        self.stdout_bytes(read.storage.as_bytes());
        self.stdout_bytes(b"\nstorage_capacity_bytes=");
        self.write_u64_dec(read.capacity_bytes as u64);
        self.stdout_bytes(b"\nroot_bytes=");
        self.write_u64_dec(read.bytes as u64);
        match snapshot {
            exec_bundle::ExecBundleSnapshot::Unavailable { .. } => {
                self.stdout_bytes(b"\nstatus=unavailable\nreason=exec-bundle-unavailable\n");
                return;
            }
            exec_bundle::ExecBundleSnapshot::ReadError { read } => {
                self.stdout_bytes(b"\nstatus=read-error\nreason=");
                self.stdout_bytes(read.reason.as_bytes());
                self.stdout_bytes(b"\n");
                return;
            }
            _ => {}
        }

        match snapshot {
            exec_bundle::ExecBundleSnapshot::Catalog {
                count, truncated, ..
            } => {
                self.stdout_bytes(b"\nstatus=ok\nformat=catalog\nentries=");
                self.write_u64_dec(count as u64);
                self.stdout_bytes(b"\ntruncated=");
                self.stdout_bytes(if truncated { b"true" } else { b"false" });
                self.stdout_bytes(b"\n");
                let mut index = 0usize;
                while index < count {
                    let entry = entries[index];
                    self.stdout_bytes(b"- namespace=");
                    self.stdout_bytes(entry.namespace.as_str().as_bytes());
                    self.stdout_bytes(b" path=");
                    self.stdout_bytes(entry.path.as_bytes());
                    self.stdout_bytes(b" offset=");
                    self.write_u64_dec(entry.offset as u64);
                    self.stdout_bytes(b" artifact_bytes=");
                    self.write_u64_dec(entry.artifact_bytes_len as u64);
                    self.stdout_bytes(b" checksum=");
                    self.write_u64_dec(entry.checksum as u64);
                    self.stdout_bytes(b"\n");
                    index += 1;
                }
            }
            exec_bundle::ExecBundleSnapshot::Artifact { artifact, .. } => {
                self.stdout_bytes(b"\nstatus=ok\nformat=artifact\nentries=1\n");
                self.stdout_bytes(b"truncated=false\n");
                self.stdout_bytes(b"- namespace=");
                self.stdout_bytes(artifact.namespace.as_str().as_bytes());
                self.stdout_bytes(b" path=");
                self.stdout_bytes(artifact.path.as_bytes());
                self.stdout_bytes(b" offset=0 artifact_bytes=");
                self.write_u64_dec(read.bytes as u64);
                self.stdout_bytes(b" bytes=");
                self.write_u64_dec(artifact.source_bytes.len() as u64);
                self.stdout_bytes(b" checksum=");
                self.write_u64_dec(artifact.checksum as u64);
                self.stdout_bytes(b"\n");
            }
            exec_bundle::ExecBundleSnapshot::InvalidCatalog { error, .. } => {
                self.stdout_bytes(b"\nstatus=invalid\nformat=catalog\nreason=");
                self.stdout_bytes(error.as_str().as_bytes());
                self.stdout_bytes(b"\n");
            }
            exec_bundle::ExecBundleSnapshot::InvalidArtifact { error, .. } => {
                self.stdout_bytes(b"\nstatus=invalid\nformat=artifact\nreason=");
                self.stdout_bytes(error.as_str().as_bytes());
                self.stdout_bytes(b"\n");
            }
            exec_bundle::ExecBundleSnapshot::Unavailable { .. }
            | exec_bundle::ExecBundleSnapshot::ReadError { .. } => {}
        }
    }

    /// Writes retained scheduler task records to stdout.
    pub fn write_task_table(&self) {
        if !self.direct_backend_report_admitted(SyscallOp::SnapshotTasks) {
            return;
        }
        let mut records = [sched::EMPTY_KERNEL_TASK_RECORD; sched::MAX_KERNEL_TASKS];
        let count = self.snapshot_tasks(&mut records);
        self.stdout_line("tasks:");
        let mut index = 0usize;
        while index < count {
            let record = records[index];
            self.stdout_bytes(b"- task=");
            self.write_u64_dec(record.task_id as u64);
            self.stdout_bytes(b" pid=");
            self.write_u64_dec(record.process_id as u64);
            self.stdout_bytes(b" parent_task=");
            self.write_u64_dec(record.parent_task_id as u64);
            self.stdout_bytes(b" state=");
            self.stdout_bytes(record.state.as_str().as_bytes());
            self.stdout_bytes(b" entry=");
            self.stdout_bytes(record.entry.as_bytes());
            self.stdout_bytes(b" block=");
            self.stdout_bytes(record.block_reason.as_str().as_bytes());
            self.stdout_bytes(b" wake_tick=");
            self.write_u64_dec(record.wake_tick as u64);
            self.stdout_bytes(b" runs=");
            self.write_u64_dec(record.run_count as u64);
            self.stdout_bytes(b" ticks=");
            self.write_u64_dec(record.runtime_ticks as u64);
            self.stdout_bytes(b"\n");
            index += 1;
        }
    }

    /// Writes retained process wait records to stdout.
    pub fn write_wait_table(&self) {
        if !self.direct_backend_report_admitted(SyscallOp::SnapshotWaits) {
            return;
        }
        let mut records = [proc::EMPTY_WAIT_RECORD; proc::MAX_WAITS];
        let count = self.snapshot_waits(&mut records);
        self.stdout_line("waits:");
        let mut index = 0usize;
        while index < count {
            let record = records[index];
            self.stdout_bytes(b"- parent_pid=");
            self.write_u64_dec(record.parent_pid as u64);
            self.stdout_bytes(b" child_pid=");
            self.write_u64_dec(record.child_pid as u64);
            self.stdout_bytes(b" child_state=");
            self.stdout_bytes(record.child_state.as_str().as_bytes());
            self.stdout_bytes(b" exit=");
            self.write_u64_dec(record.exit_code as u64);
            self.stdout_bytes(b" completed=");
            self.write_true_false_word(record.completed);
            self.stdout_bytes(b"\n");
            index += 1;
        }
    }

    /// Writes retained typed syscall records to stdout.
    pub fn write_syscall_table(&self) {
        if !self.direct_backend_report_admitted(SyscallOp::SnapshotSyscalls) {
            return;
        }
        let mut records = [EMPTY_SYSCALL_RECORD; MAX_SYSCALL_RECORDS];
        let count = self.snapshot_syscalls(&mut records);
        self.stdout_line("syscalls:");
        let mut index = 0usize;
        while index < count {
            let record = records[index];
            self.stdout_bytes(b"- seq=");
            self.write_u64_dec(record.seq as u64);
            self.stdout_bytes(b" pid=");
            self.write_u64_dec(record.process_id as u64);
            self.stdout_bytes(b" task=");
            self.write_u64_dec(record.task_id as u64);
            self.stdout_bytes(b" path=");
            self.stdout_bytes(record.program_path.as_bytes());
            self.stdout_bytes(b" op=");
            self.stdout_bytes(record.op.as_str().as_bytes());
            self.stdout_bytes(b" status=");
            self.stdout_bytes(record.status.as_str().as_bytes());
            self.stdout_bytes(b" loader=");
            self.stdout_bytes(record.loader.as_bytes());
            self.stdout_bytes(b" entry_fn=");
            self.stdout_bytes(record.entry_name.as_bytes());
            self.stdout_bytes(b"\n");
            index += 1;
        }
    }

    /// Writes active retained raw syscall continuation records to stdout.
    pub fn write_syscall_continuation_table(&self) {
        if !self.direct_backend_report_admitted(SyscallOp::SnapshotContinuations) {
            return;
        }
        let mut records = [EMPTY_SYSCALL_CONTINUATION_RECORD; MAX_SYSCALL_CONTINUATION_RECORDS];
        let count = self.snapshot_syscall_continuations(&mut records);
        self.stdout_line("continuations:");
        let mut index = 0usize;
        while index < count {
            let record = records[index];
            self.stdout_bytes(b"- pid=");
            self.write_u64_dec(record.process_id as u64);
            self.stdout_bytes(b" task=");
            self.write_u64_dec(record.task_id as u64);
            self.stdout_bytes(b" path=");
            self.stdout_bytes(record.program_path.as_bytes());
            self.stdout_bytes(b" nr=");
            self.write_u64_dec(record.nr.raw() as u64);
            self.stdout_bytes(b" op=");
            self.stdout_bytes(record.op.as_str().as_bytes());
            self.stdout_bytes(b" memory=");
            self.stdout_bytes(record.memory.as_str().as_bytes());
            self.stdout_bytes(b" a0=");
            self.write_u64_dec(record.args.a0 as u64);
            self.stdout_bytes(b" a1=");
            self.write_u64_dec(record.args.a1 as u64);
            self.stdout_bytes(b" a2=");
            self.write_u64_dec(record.args.a2 as u64);
            self.stdout_bytes(b" a3=");
            self.write_u64_dec(record.args.a3 as u64);
            self.stdout_bytes(b" a4=");
            self.write_u64_dec(record.args.a4 as u64);
            self.stdout_bytes(b" a5=");
            self.write_u64_dec(record.args.a5 as u64);
            self.stdout_bytes(b" loader=");
            self.stdout_bytes(record.loader.as_bytes());
            self.stdout_bytes(b" entry_fn=");
            self.stdout_bytes(record.entry_name.as_bytes());
            self.stdout_bytes(b"\n");
            index += 1;
        }
    }

    /// Copies retained scheduler task records into `out`, returning the count copied.
    pub fn snapshot_tasks(&self, out: &mut [KernelTaskRecord]) -> usize {
        if !self.direct_backend_report_admitted(SyscallOp::SnapshotTasks) {
            return 0;
        }
        let count = sched::snapshot_kernel_tasks(out);
        self.record(SyscallOp::SnapshotTasks, SyscallStatus::Ok);
        count
    }

    /// Returns scheduler run-queue metadata.
    #[must_use]
    pub fn scheduler_snapshot(&self) -> SchedulerSnapshot {
        if !self.direct_backend_report_admitted(SyscallOp::SchedulerSnapshot) {
            return blocked_scheduler_snapshot();
        }
        match self.dispatch(SyscallRequest::SchedulerSnapshot) {
            SyscallResult::SchedulerSnapshot(snapshot) => snapshot,
            _ => sched::snapshot_scheduler(),
        }
    }

    /// Writes scheduler run-queue metadata to stdout.
    pub fn write_scheduler_state(&self) {
        if !self.direct_backend_report_admitted(SyscallOp::SchedulerSnapshot) {
            return;
        }
        let snapshot = self.scheduler_snapshot();
        self.stdout_line("scheduler:");
        self.stdout_bytes(b"current_task=");
        self.write_u64_dec(snapshot.current_task_id as u64);
        self.stdout_bytes(b"\ncurrent_pid=");
        self.write_u64_dec(snapshot.current_process_id as u64);
        self.stdout_bytes(b"\nready_queue_len=");
        self.write_u64_dec(snapshot.ready_len as u64);
        self.stdout_bytes(b"\nnext_ready_task=");
        self.write_u64_dec(snapshot.next_ready_task_id() as u64);
        self.stdout_bytes(b"\nnext_ready_pid=");
        self.write_u64_dec(snapshot.next_ready_process_id() as u64);
        self.stdout_bytes(b"\ndispatch_count=");
        self.write_u64_dec(snapshot.dispatch_count as u64);
        self.stdout_bytes(b"\nyield_count=");
        self.write_u64_dec(snapshot.yield_count as u64);
        self.stdout_bytes(b"\ntick_count=");
        self.write_u64_dec(snapshot.tick_count as u64);
        self.stdout_bytes(b"\nready_queue:\n");
        let mut index = 0usize;
        while index < snapshot.ready_len {
            self.stdout_bytes(b"- task=");
            self.write_u64_dec(snapshot.ready_queue[index] as u64);
            self.stdout_bytes(b" pid=");
            self.write_u64_dec(snapshot.ready_process_queue[index] as u64);
            self.stdout_bytes(b"\n");
            index += 1;
        }
    }

    /// Cooperatively yields the current program task and returns a typed result.
    pub fn yield_now_result(&mut self) -> SchedulerYieldResult {
        let current = match self.require_direct_current_context(SyscallOp::YieldNow) {
            Ok(ctx) => ctx,
            Err(DirectCurrentAdmissionError::Unavailable) => {
                return SchedulerYieldResult::new(SchedulerYieldStatus::Unavailable, false, 0, 0);
            }
            Err(DirectCurrentAdmissionError::Busy(current)) => {
                return SchedulerYieldResult::new(
                    SchedulerYieldStatus::Busy,
                    false,
                    current.map_or(0, |ctx| ctx.pid),
                    current.map_or(0, |ctx| ctx.task_id),
                );
            }
        };

        let Some(record) = proc::process(current.pid) else {
            record_syscall(self.current, SyscallOp::YieldNow, SyscallStatus::Error);
            return SchedulerYieldResult::new(SchedulerYieldStatus::Failed, false, 0, 0);
        };
        if record.state != proc::ProcessState::Running {
            record_syscall(self.current, SyscallOp::YieldNow, SyscallStatus::Error);
            return SchedulerYieldResult::new(
                SchedulerYieldStatus::NotRunning,
                false,
                current.pid,
                current.task_id,
            );
        };

        let Some(selected) = proc::yield_process(current.pid) else {
            record_syscall(self.current, SyscallOp::YieldNow, SyscallStatus::Error);
            return SchedulerYieldResult::new(SchedulerYieldStatus::Failed, false, 0, 0);
        };

        record_syscall(self.current, SyscallOp::YieldNow, SyscallStatus::Ok);
        if selected.pid == current.pid {
            return SchedulerYieldResult::new(
                SchedulerYieldStatus::NoPeer,
                false,
                selected.pid,
                selected.task_id,
            );
        }

        let selected_ctx = SyscallContext::from_process(selected);
        record_context(selected_ctx, SyscallOp::SchedulerDispatch, SyscallStatus::Ok);
        append_scheduler_event(selected_ctx, SyscallOp::SchedulerDispatch);
        record_context(selected_ctx, SyscallOp::ProcessRun, SyscallStatus::Ok);
        let _ = self.run_selected_non_wait_child(selected_ctx);
        let _ = proc::run_process(current.pid);
        SchedulerYieldResult::new(
            SchedulerYieldStatus::Yielded,
            true,
            selected.pid,
            selected.task_id,
        )
    }

    /// Cooperatively yields the current program task.
    ///
    /// Returns `true` only when a different ready task was selected and run
    /// before this process was resumed.
    pub fn yield_now(&mut self) -> bool {
        self.yield_now_result().yielded
    }

    /// Blocks the current program task until an explicit scheduler tick deadline.
    ///
    /// This is cooperative Reovim scheduler control. While the current process
    /// is asleep, ready work can run and explicit scheduler ticks can wake due
    /// sleepers. The call returns only after the current process has been
    /// scheduler-dispatched back to running state.
    pub fn sleep_current_for_ticks_result(&mut self, ticks: usize) -> SchedulerSleepResult {
        let current = match self.require_direct_current_context(SyscallOp::ProcessSleep) {
            Ok(ctx) => ctx,
            Err(DirectCurrentAdmissionError::Unavailable) => {
                return SchedulerSleepResult::new(
                    SchedulerSleepStatus::Unavailable,
                    false,
                    0,
                    0,
                    0,
                    sched::snapshot_scheduler().tick_count,
                    0,
                    0,
                );
            }
            Err(DirectCurrentAdmissionError::Busy(current)) => {
                return SchedulerSleepResult::new(
                    SchedulerSleepStatus::Busy,
                    false,
                    current.map_or(0, |ctx| ctx.pid),
                    current.map_or(0, |ctx| ctx.task_id),
                    0,
                    sched::snapshot_scheduler().tick_count,
                    0,
                    0,
                );
            }
        };

        let Some(process) = proc::process(current.pid) else {
            record_context(current, SyscallOp::ProcessSleep, SyscallStatus::Error);
            return SchedulerSleepResult::new(
                SchedulerSleepStatus::Failed,
                false,
                current.pid,
                current.task_id,
                0,
                sched::snapshot_scheduler().tick_count,
                0,
                0,
            );
        };
        if process.state != proc::ProcessState::Running {
            record_context(current, SyscallOp::ProcessSleep, SyscallStatus::Error);
            return SchedulerSleepResult::new(
                SchedulerSleepStatus::NotRunning,
                false,
                current.pid,
                current.task_id,
                0,
                sched::snapshot_scheduler().tick_count,
                0,
                0,
            );
        }

        let wake_tick = sched::snapshot_scheduler().tick_count.saturating_add(ticks);
        let Some(sleeping) = proc::sleep_process_until(current.pid, wake_tick) else {
            record_context(current, SyscallOp::ProcessSleep, SyscallStatus::Error);
            return SchedulerSleepResult::new(
                SchedulerSleepStatus::Failed,
                false,
                current.pid,
                current.task_id,
                wake_tick,
                sched::snapshot_scheduler().tick_count,
                0,
                0,
            );
        };
        let sleeping_ctx = SyscallContext::from_process(sleeping);
        record_context(sleeping_ctx, SyscallOp::ProcessSleep, SyscallStatus::Ok);
        append_process_control_event(sleeping_ctx, SyscallOp::ProcessSleep);

        let mut dispatched_count = 0usize;
        let mut woken_count = 0usize;
        let mut steps = 0usize;
        let step_budget = sched::MAX_KERNEL_TASKS.saturating_mul(2);
        while steps < step_budget {
            if let Some(ctx) = dispatch_next_ready_program() {
                if ctx.pid == current.pid {
                    return SchedulerSleepResult::new(
                        SchedulerSleepStatus::Ok,
                        true,
                        current.pid,
                        current.task_id,
                        wake_tick,
                        sched::snapshot_scheduler().tick_count,
                        dispatched_count,
                        woken_count,
                    );
                }

                let _ = self.run_selected_non_wait_child(ctx);
                dispatched_count = dispatched_count.saturating_add(1);
                steps += 1;
                continue;
            }

            let snapshot = sched::snapshot_scheduler();
            if snapshot.tick_count >= wake_tick {
                let woken = wake_sleepers_due_with_events(snapshot.tick_count);
                woken_count = woken_count.saturating_add(woken);
                steps += 1;
                continue;
            }

            let snapshot = sched::tick_kernel_scheduler();
            record_context(current, SyscallOp::SchedulerTick, SyscallStatus::Ok);
            let woken = wake_sleepers_due_with_events(snapshot.tick_count);
            woken_count = woken_count.saturating_add(woken);
            steps += 1;
        }

        let _ = proc::wake_process(current.pid);
        let mut restore_steps = 0usize;
        while restore_steps < sched::MAX_KERNEL_TASKS {
            let Some(ctx) = dispatch_next_ready_program() else {
                break;
            };
            if ctx.pid == current.pid {
                break;
            }
            let _ = self.run_selected_non_wait_child(ctx);
            restore_steps += 1;
        }

        record_context(current, SyscallOp::ProcessSleep, SyscallStatus::Error);
        SchedulerSleepResult::new(
            SchedulerSleepStatus::BudgetExhausted,
            false,
            current.pid,
            current.task_id,
            wake_tick,
            sched::snapshot_scheduler().tick_count,
            dispatched_count,
            woken_count,
        )
    }

    /// Records one explicit scheduler tick for the current running program task.
    pub fn scheduler_tick_result(&self) -> SchedulerTickResult {
        let current = match self.require_direct_current_context(SyscallOp::SchedulerTick) {
            Ok(ctx) => ctx,
            Err(DirectCurrentAdmissionError::Unavailable) => {
                return SchedulerTickResult::new(
                    SchedulerTickStatus::Unavailable,
                    false,
                    0,
                    0,
                    0,
                    0,
                );
            }
            Err(DirectCurrentAdmissionError::Busy(current)) => {
                let snapshot = sched::snapshot_scheduler();
                return SchedulerTickResult::new(
                    SchedulerTickStatus::Busy,
                    false,
                    current.map_or(0, |ctx| ctx.task_id),
                    snapshot.tick_count,
                    0,
                    0,
                );
            }
        };

        let Some(process) = proc::process(current.pid) else {
            record_syscall(self.current, SyscallOp::SchedulerTick, SyscallStatus::Error);
            let snapshot = sched::snapshot_scheduler();
            return SchedulerTickResult::new(
                SchedulerTickStatus::Failed,
                false,
                current.task_id,
                snapshot.tick_count,
                0,
                0,
            );
        };

        if process.state != proc::ProcessState::Running {
            record_syscall(self.current, SyscallOp::SchedulerTick, SyscallStatus::Error);
            let snapshot = sched::snapshot_scheduler();
            return SchedulerTickResult::new(
                SchedulerTickStatus::NotRunning,
                false,
                current.task_id,
                snapshot.tick_count,
                0,
                0,
            );
        }

        let Some(task) = sched::tick_kernel_task(current.task_id) else {
            record_syscall(self.current, SyscallOp::SchedulerTick, SyscallStatus::Error);
            let snapshot = sched::snapshot_scheduler();
            return SchedulerTickResult::new(
                SchedulerTickStatus::Failed,
                false,
                current.task_id,
                snapshot.tick_count,
                0,
                0,
            );
        };

        record_syscall(self.current, SyscallOp::SchedulerTick, SyscallStatus::Ok);
        let snapshot = sched::snapshot_scheduler();
        let woken_count = wake_sleepers_due_with_events(snapshot.tick_count);
        SchedulerTickResult::new(
            SchedulerTickStatus::Ok,
            true,
            task.task_id,
            snapshot.tick_count,
            task.runtime_ticks,
            woken_count,
        )
    }

    /// Writes the result of one explicit scheduler tick to stdout.
    pub fn write_scheduler_tick(&self) {
        if !self.direct_backend_report_admitted(SyscallOp::SchedulerTick) {
            return;
        }
        let result = self.scheduler_tick_result();
        self.stdout_bytes(b"sched tick:\nticked=");
        if result.ticked {
            self.stdout_bytes(b"true");
        } else {
            self.stdout_bytes(b"false");
        }
        self.stdout_bytes(b"\nstatus=");
        self.stdout_bytes(result.status.as_str().as_bytes());
        self.stdout_bytes(b"\ntask=");
        self.write_u64_dec(result.task_id as u64);
        self.stdout_bytes(b"\ntick_count=");
        self.write_u64_dec(result.tick_count as u64);
        self.stdout_bytes(b"\ntask_ticks=");
        self.write_u64_dec(result.task_ticks as u64);
        self.stdout_bytes(b"\nwoken=");
        self.write_u64_dec(result.woken_count as u64);
        self.stdout_bytes(b"\n");
    }

    /// Copies retained typed syscall dispatch records into `out`.
    pub fn snapshot_syscalls(&self, out: &mut [SyscallRecord]) -> usize {
        if !self.direct_backend_report_admitted(SyscallOp::SnapshotSyscalls) {
            return 0;
        }
        self.record(SyscallOp::SnapshotSyscalls, SyscallStatus::Ok);
        snapshot_syscalls(out)
    }

    /// Copies active retained raw syscall continuation records into `out`.
    pub fn snapshot_syscall_continuations(&self, out: &mut [SyscallContinuationRecord]) -> usize {
        if !self.direct_backend_report_admitted(SyscallOp::SnapshotContinuations) {
            return 0;
        }
        self.record(SyscallOp::SnapshotContinuations, SyscallStatus::Ok);
        snapshot_syscall_continuations(out)
    }

    fn record(&self, op: SyscallOp, status: SyscallStatus) {
        record_syscall(self.current, op, status);
    }

    fn record_result(&self, op: SyscallOp, ok: bool) {
        self.record(
            op,
            if ok {
                SyscallStatus::Ok
            } else {
                SyscallStatus::Error
            },
        );
    }

    pub(crate) fn take_raw_exit_status(&mut self) -> Option<ProgramStatus> {
        self.raw_exit_status.take()
    }

    fn write_kv_num(&self, key: &str, value: u64) {
        self.stdout_bytes(key.as_bytes());
        self.stdout_bytes(b"=");
        self.write_u64_dec(value);
        self.stdout_bytes(b"\n");
    }

    fn write_device_row(&self, device: &reovim_uapi_system::DeviceEntry, index: usize) {
        self.stdout_bytes(b"- [");
        self.write_u64_dec(index as u64);
        self.stdout_bytes(b"] ");
        self.stdout_bytes(vfs::device_class_name(device.class).as_bytes());
        self.stdout_bytes(b" compat=");
        self.stdout_bytes(device.compatible.as_bytes());
        self.stdout_bytes(b" mmio=");
        self.write_u64_hex(device.mmio_base);
        self.stdout_bytes(b"/");
        self.write_u64_hex(device.mmio_len);
        self.stdout_bytes(b" irq=");
        self.write_u64_dec(device.irq as u64);
        self.stdout_bytes(b"\n");
    }

    fn write_directory(&self, directory: Directory) {
        match directory {
            Directory::Root => {
                self.stdout_line("bin");
                self.stdout_line("boot");
                self.stdout_line("dev");
                self.stdout_line("dump");
                self.stdout_line("log");
                self.stdout_line("proc");
            }
            Directory::Bin => {
                for entry in self.programs() {
                    self.stdout_line(entry.name);
                }
                let mut media_programs = [None; program::MAX_MEDIA_PROGRAMS];
                let count = program::snapshot_media_programs(&mut media_programs);
                let mut index = 0usize;
                while index < count {
                    if let Some(entry) = media_programs[index] {
                        self.stdout_line(entry.name);
                    }
                    index += 1;
                }
            }
            Directory::Boot => {
                self.stdout_line("devices");
                self.stdout_line("help");
                self.stdout_line("image");
                self.stdout_line("input");
                self.stdout_line("memory");
                self.stdout_line("mounts");
                self.stdout_line("payloads");
                self.stdout_line("probes");
                self.stdout_line("proof");
                self.stdout_line("profile");
                self.stdout_line("status");
            }
            Directory::Dev => {
                self.stdout_line("tty");
                let devices = self.devices();
                let mut index = 0usize;
                while index < devices.len() {
                    self.write_device_name(devices, index);
                    self.stdout_bytes(b"\n");
                    index += 1;
                }
            }
            Directory::Dump => {
                self.stdout_line("snapshot");
                self.stdout_line("status");
            }
            Directory::Log => {
                self.stdout_line("dmesg");
                self.stdout_line("events");
                self.stdout_line("stats");
            }
            Directory::Proc => {
                self.stdout_line("address-spaces");
                self.stdout_line("continuations");
                self.stdout_line("execs");
                self.stdout_line("media");
                self.stdout_line("memory-objects");
                self.stdout_line("page-tables");
                self.stdout_line("pages");
                self.stdout_line("pending");
                self.stdout_line("processes");
                self.stdout_line("self");
                self.stdout_line("session");
                self.stdout_line("services");
                self.stdout_line("scheduler");
                self.stdout_line("sources");
                self.stdout_line("syscalls");
                self.stdout_line("tasks");
                self.stdout_line("waits");
            }
        }
    }

    fn write_device_name(&self, devices: &[reovim_uapi_system::DeviceEntry], index: usize) {
        let (class, ordinal) = vfs::device_name_parts(devices, index);
        self.stdout_bytes(class.as_bytes());
        self.write_u64_dec(ordinal as u64);
    }

    fn path_basename(path: &str) -> &str {
        let bytes = path.as_bytes();
        let mut start = bytes.len();
        while start > 0 {
            if bytes[start - 1] == b'/' {
                break;
            }
            start -= 1;
        }
        &path[start..]
    }

    fn write_boot_proof_image_facts(&self) {
        let image = self.boot_image();
        self.stdout_bytes(b"  package=");
        self.stdout_bytes(image.package.as_bytes());
        self.stdout_bytes(b"\n  version=");
        self.stdout_bytes(image.version.as_bytes());
        self.stdout_bytes(b"\n  target=");
        self.stdout_bytes(image.target.as_bytes());
        self.stdout_bytes(b"\n  selected_profile=");
        self.stdout_bytes(image.selected_profile.as_bytes());
        self.stdout_bytes(b"\n  profile_request=");
        self.stdout_bytes(image.profile_request.as_bytes());
        self.stdout_bytes(b"\n  launch_profile_feature=");
        self.stdout_bytes(image.launch_profile_feature.as_bytes());
        self.stdout_bytes(b"\n");
    }

    fn write_boot_proof_profile_facts(&self) {
        self.stdout_bytes(b"  profile=");
        self.stdout_bytes(self.profile_name().as_bytes());
        self.stdout_bytes(b"\n  launch=");
        self.write_enabled_disabled(self.launch_enabled());
        self.stdout_bytes(b"\n  payloads=");
        self.write_u64_dec(self.payloads().len() as u64);
        self.stdout_bytes(b"\n");
    }

    fn write_process_record(&self, record: ProcessRecord) {
        self.stdout_bytes(b"- pid=");
        self.write_u64_dec(record.pid as u64);
        self.stdout_bytes(b" ppid=");
        self.write_u64_dec(record.parent_pid as u64);
        self.stdout_bytes(b" task=");
        self.write_u64_dec(record.task_id as u64);
        self.stdout_bytes(b" state=");
        self.stdout_bytes(record.state.as_str().as_bytes());
        self.stdout_bytes(b" path=");
        self.stdout_bytes(record.program_path.as_bytes());
        self.stdout_bytes(b" exit=");
        self.write_u64_dec(record.exit_code as u64);
        self.stdout_bytes(b" loader=");
        self.stdout_bytes(record.loader.as_bytes());
        self.stdout_bytes(b" entry_fn=");
        self.stdout_bytes(record.entry_name.as_bytes());
        self.stdout_bytes(b" block=");
        self.stdout_bytes(record.block_reason.as_str().as_bytes());
        self.stdout_bytes(b" argc=");
        self.write_u64_dec(record.argc as u64);
        self.stdout_bytes(b" argv0=");
        self.stdout_bytes(record.argv0().as_bytes());
        self.stdout_bytes(b" argv0_truncated=");
        self.write_true_false_word(record.argv_was_truncated(0));
        self.stdout_bytes(b" argv1=");
        self.stdout_bytes(record.argv1().as_bytes());
        self.stdout_bytes(b" argv1_truncated=");
        self.write_true_false_word(record.argv_was_truncated(1));
        let mut argv_index = 2usize;
        while argv_index < record.argc && argv_index < proc::MAX_PROCESS_ARGS {
            self.stdout_bytes(b" argv");
            self.write_u64_dec(argv_index as u64);
            self.stdout_bytes(b"=");
            self.stdout_bytes(record.argv(argv_index).as_bytes());
            self.stdout_bytes(b" argv");
            self.write_u64_dec(argv_index as u64);
            self.stdout_bytes(b"_truncated=");
            self.write_true_false_word(record.argv_was_truncated(argv_index));
            argv_index += 1;
        }
        self.stdout_bytes(b" envc=");
        self.write_u64_dec(record.envc as u64);
        let mut env_index = 0usize;
        while env_index < record.envc && env_index < proc::MAX_PROCESS_ENVS {
            self.stdout_bytes(b" env");
            self.write_u64_dec(env_index as u64);
            self.stdout_bytes(b"_name=");
            self.stdout_bytes(record.env_name(env_index).as_bytes());
            self.stdout_bytes(b" env");
            self.write_u64_dec(env_index as u64);
            self.stdout_bytes(b"_value=");
            self.stdout_bytes(record.env_value(env_index).as_bytes());
            self.stdout_bytes(b" env");
            self.write_u64_dec(env_index as u64);
            self.stdout_bytes(b"_truncated=");
            self.write_true_false_word(record.env_was_truncated(env_index));
            env_index += 1;
        }
        self.stdout_bytes(b" address_space=");
        self.write_u64_dec(record.address_space_id as u64);
        let address_space = mm::address_space(record.address_space_id);
        self.stdout_bytes(b" address_space_state=");
        let address_space_state = address_space.map_or("missing", |space| space.state.as_str());
        self.stdout_bytes(address_space_state.as_bytes());
        self.stdout_bytes(b" address_space_source=");
        self.stdout_bytes(
            address_space
                .map_or("", |space| space.source_path)
                .as_bytes(),
        );
        self.stdout_bytes(b" address_space_text_bytes=");
        self.write_u64_dec(address_space.map_or(0, |space| space.text_bytes) as u64);
        self.stdout_bytes(b" address_space_text_checksum=");
        self.write_u64_dec(address_space.map_or(0, |space| space.text_checksum as usize) as u64);
        self.stdout_bytes(b" address_space_stack_bytes=");
        self.write_u64_dec(address_space.map_or(0, |space| space.stack_bytes) as u64);
        self.stdout_bytes(b" address_space_regions=");
        self.write_u64_dec(address_space.map_or(0, |space| space.region_count) as u64);
        self.stdout_bytes(b" address_space_text_start=");
        self.write_u64_dec(address_space.map_or(0, |space| space.text_start) as u64);
        self.stdout_bytes(b" address_space_text_end=");
        self.write_u64_dec(address_space.map_or(0, |space| space.text_end) as u64);
        self.stdout_bytes(b" address_space_text_flags=");
        self.write_u64_dec(address_space.map_or(0, |space| space.text_flags as usize) as u64);
        self.stdout_bytes(b" address_space_stack_start=");
        self.write_u64_dec(address_space.map_or(0, |space| space.stack_start) as u64);
        self.stdout_bytes(b" address_space_stack_end=");
        self.write_u64_dec(address_space.map_or(0, |space| space.stack_end) as u64);
        self.stdout_bytes(b" address_space_stack_flags=");
        self.write_u64_dec(address_space.map_or(0, |space| space.stack_flags as usize) as u64);
        self.stdout_bytes(b" artifact_body_format=");
        self.stdout_bytes(record.artifact_body_format.as_str().as_bytes());
        self.stdout_bytes(b" artifact_body_inner=");
        self.stdout_bytes(record.artifact_body_inner_format.as_str().as_bytes());
        self.stdout_bytes(b" image_generation=");
        self.write_u64_dec(record.image_generation as u64);
        self.stdout_bytes(b"\n");
    }

    fn write_u64_dec(&self, mut value: u64) {
        let mut buf = [0u8; 20];
        let mut len = 0usize;
        if value == 0 {
            self.stdout_bytes(b"0");
            return;
        }
        while value > 0 && len < buf.len() {
            buf[len] = b'0' + (value % 10) as u8;
            value /= 10;
            len += 1;
        }
        while len > 0 {
            len -= 1;
            self.stdout_bytes(&buf[len..len + 1]);
        }
    }

    fn write_u64_hex(&self, value: u64) {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        self.stdout_bytes(b"0x");
        let mut started = false;
        let mut shift = 60u32;
        loop {
            let nibble = ((value >> shift) & 0xf) as usize;
            if nibble != 0 || started || shift == 0 {
                self.stdout_bytes(&HEX[nibble..nibble + 1]);
                started = true;
            }
            if shift == 0 {
                break;
            }
            shift -= 4;
        }
    }

    fn write_enabled_disabled(&self, value: bool) {
        self.stdout_bytes(if value { b"enabled" } else { b"disabled" });
    }

    fn write_true_false_word(&self, value: bool) {
        self.stdout_bytes(if value { b"true" } else { b"false" });
    }

    fn record_payload_launch(&self, result: PayloadLaunchResult) {
        self.record(SyscallOp::PayloadLaunch, payload_syscall_status(result));
    }

    fn dispatch(&self, request: SyscallRequest) -> SyscallResult {
        match request {
            SyscallRequest::KernelLogStats => {
                record_syscall(self.current, request.op(), SyscallStatus::Ok);
                SyscallResult::KernelLogStats(klog::stats())
            }
            SyscallRequest::DumpStatus => {
                record_syscall(self.current, request.op(), SyscallStatus::Ok);
                SyscallResult::DumpStatus(self.dump_status_snapshot())
            }
            SyscallRequest::DumpSync => {
                let status = dump::sync_with_context(
                    self.dump_image_identity(),
                    self.daemon.boot_info(),
                    self.daemon.devices(),
                );
                record_syscall(self.current, request.op(), dump_sync_syscall_status(status));
                SyscallResult::DumpSync(status)
            }
            SyscallRequest::SchedulerSnapshot => {
                record_syscall(self.current, request.op(), SyscallStatus::Ok);
                SyscallResult::SchedulerSnapshot(sched::snapshot_scheduler())
            }
        }
    }

    fn dump_status_snapshot(&self) -> dump::DumpStatus {
        dump::status_with_context(
            self.dump_image_identity(),
            self.daemon.boot_info(),
            self.daemon.devices(),
        )
    }

    fn dump_image_identity(&self) -> dump::DumpImageIdentity {
        let image = self.daemon.boot_image();
        dump::DumpImageIdentity::new(
            image.package,
            image.version,
            image.target,
            image.selected_profile,
            image.profile_request,
            image.bootline,
            image.launch_profile_feature,
        )
    }

    /// Clears the active framebuffer console.
    pub fn clear_console(&self) {
        if self.has_invalidated_process_context() {
            self.record(SyscallOp::TtyClear, SyscallStatus::Error);
            return;
        }
        if self.current_has_direct_backend_admission_block() {
            self.record(SyscallOp::SyscallContinue, SyscallStatus::Error);
            self.record(SyscallOp::TtyClear, SyscallStatus::Error);
            return;
        }
        crate::console::clear_screen();
        self.record(SyscallOp::TtyClear, SyscallStatus::Ok);
    }
}

impl Drop for ProgramSyscalls<'_, '_, '_> {
    fn drop(&mut self) {
        if let Some(current) = self.current {
            let _ = with_existing_process_fd_table_mut(current.pid, |table| {
                table.set_stdout_capture(None);
            });
        } else {
            self.fd_table.close_all();
        }
        self.vfs_render_capture = None;
    }
}

fn append_program_exec_log(ctx: SyscallContext, status: ProgramStatus) {
    klog::append_bytes(b"exec.path=");
    klog::append_bytes(ctx.program_path.as_bytes());
    klog::append_bytes(b" pid=");
    klog::append_usize_dec(ctx.pid);
    klog::append_bytes(b" task=");
    klog::append_usize_dec(ctx.task_id);
    klog::append_bytes(b" status=");
    klog::append_bytes(status.as_bytes());
    klog::append_bytes(b" loader=");
    klog::append_bytes(ctx.loader.as_bytes());
    klog::append_bytes(b" entry_fn=");
    klog::append_bytes(ctx.entry_name.as_bytes());
    klog::append_bytes(b"\n");
    append_program_parent_log(ctx);
    klog::append_event_with_context("proc", "info", "program-exit", ctx.pid, ctx.task_id);
}

fn append_program_parent_log(ctx: SyscallContext) {
    let parent_pid = proc::process(ctx.pid).map_or(0, |record| record.parent_pid);
    let parent_task = sched::task(ctx.task_id).map_or(0, |record| record.parent_task_id);
    klog::append_bytes(b"exec.parent path=");
    klog::append_bytes(ctx.program_path.as_bytes());
    klog::append_bytes(b" pid=");
    klog::append_usize_dec(ctx.pid);
    klog::append_bytes(b" ppid=");
    klog::append_usize_dec(parent_pid);
    klog::append_bytes(b" task=");
    klog::append_usize_dec(ctx.task_id);
    klog::append_bytes(b" parent_task=");
    klog::append_usize_dec(parent_task);
    klog::append_bytes(b"\n");
}

fn append_process_control_event(ctx: SyscallContext, op: SyscallOp) {
    klog::append_event_with_source_context(
        "process",
        "proc",
        "info",
        op.as_str(),
        ctx.pid,
        ctx.task_id,
    );
}

fn record_terminal_service_for_program(ctx: SyscallContext, status: ProgramStatus) {
    match status {
        ProgramStatus::Empty
        | ProgramStatus::Ok
        | ProgramStatus::Replaced
        | ProgramStatus::ExitCode(0) => {
            if let Some(record) = service::mark_exited_by_service_pid(ctx.pid) {
                append_service_lifecycle_event(ctx, SyscallOp::ServiceExited, record);
            }
        }
        ProgramStatus::Error | ProgramStatus::ExitCode(_) => {
            if let Some(record) =
                service::mark_failed_by_service_pid(ctx.pid, service::ServiceReason::ProcessFailed)
            {
                append_service_lifecycle_event(ctx, SyscallOp::ServiceFailed, record);
            }
        }
        ProgramStatus::Halt => {
            if let Some(record) =
                service::mark_failed_by_service_pid(ctx.pid, service::ServiceReason::Halt)
            {
                append_service_lifecycle_event(ctx, SyscallOp::ServiceFailed, record);
            }
        }
        ProgramStatus::Blocked => {}
    }
}

fn record_terminal_service_for_payload(ctx: SyscallContext, result: PayloadLaunchResult) {
    match result {
        PayloadLaunchResult::Ready | PayloadLaunchResult::ExitCode(0) => {
            if let Some(record) = service::mark_exited_by_service_pid(ctx.pid) {
                append_service_lifecycle_event(ctx, SyscallOp::ServiceExited, record);
            }
        }
        PayloadLaunchResult::Resident => {}
        PayloadLaunchResult::NotConfigured
        | PayloadLaunchResult::Failed
        | PayloadLaunchResult::ExitCode(_) => {
            if let Some(record) =
                service::mark_failed_by_service_pid(ctx.pid, service::ServiceReason::ProcessFailed)
            {
                append_service_lifecycle_event(ctx, SyscallOp::ServiceFailed, record);
            }
        }
    }
}

fn append_service_failure_event(ctx: SyscallContext, record: ServiceRecord) {
    append_service_lifecycle_event(ctx, SyscallOp::ServiceFailed, record);
}

fn append_service_lifecycle_event(ctx: SyscallContext, op: SyscallOp, record: ServiceRecord) {
    record_context(ctx, op, SyscallStatus::Ok);
    klog::append_bytes(b"syscall path=");
    klog::append_bytes(ctx.program_path.as_bytes());
    klog::append_bytes(b" op=");
    klog::append_bytes(op.as_str().as_bytes());
    klog::append_bytes(b" status=ok service=");
    klog::append_bytes(record.name.as_bytes());
    klog::append_bytes(b" target=");
    klog::append_bytes(record.target.as_bytes());
    klog::append_bytes(b" service_pid=");
    klog::append_usize_dec(record.service_pid);
    klog::append_bytes(b" service_task=");
    klog::append_usize_dec(record.service_task_id);
    klog::append_bytes(b" reason=");
    klog::append_bytes(record.reason.as_str().as_bytes());
    klog::append_bytes(b" state=");
    klog::append_bytes(record.state.as_str().as_bytes());
    klog::append_bytes(b" loader=");
    klog::append_bytes(ctx.loader.as_bytes());
    klog::append_bytes(b" entry_fn=");
    klog::append_bytes(ctx.entry_name.as_bytes());
    klog::append_bytes(b"\n");
    klog::append_event_with_source_context(
        "process",
        "syscall",
        "info",
        op.as_str(),
        ctx.pid,
        ctx.task_id,
    );
}

fn append_service_control_event(
    ctx: SyscallContext,
    op: SyscallOp,
    status: SyscallStatus,
    record: ServiceRecord,
    result: PayloadLaunchResult,
) {
    record_context(ctx, op, status);
    klog::append_bytes(b"syscall path=");
    klog::append_bytes(ctx.program_path.as_bytes());
    klog::append_bytes(b" op=");
    klog::append_bytes(op.as_str().as_bytes());
    klog::append_bytes(b" status=");
    klog::append_bytes(status.as_str().as_bytes());
    klog::append_bytes(b" service=");
    klog::append_bytes(record.name.as_bytes());
    klog::append_bytes(b" target=");
    klog::append_bytes(record.target.as_bytes());
    klog::append_bytes(b" service_pid=");
    klog::append_usize_dec(record.service_pid);
    klog::append_bytes(b" service_task=");
    klog::append_usize_dec(record.service_task_id);
    klog::append_bytes(b" reason=");
    klog::append_bytes(record.reason.as_str().as_bytes());
    klog::append_bytes(b" state=");
    klog::append_bytes(record.state.as_str().as_bytes());
    klog::append_bytes(b" result=");
    append_payload_result_word(result);
    klog::append_bytes(b" loader=");
    klog::append_bytes(ctx.loader.as_bytes());
    klog::append_bytes(b" entry_fn=");
    klog::append_bytes(ctx.entry_name.as_bytes());
    klog::append_bytes(b"\n");
    klog::append_event_with_source_context(
        "process",
        "syscall",
        "info",
        op.as_str(),
        ctx.pid,
        ctx.task_id,
    );
}

fn append_payload_result_word(result: PayloadLaunchResult) {
    match result {
        PayloadLaunchResult::Ready => klog::append_bytes(b"payload.ready"),
        PayloadLaunchResult::Resident => klog::append_bytes(b"payload.resident"),
        PayloadLaunchResult::NotConfigured => klog::append_bytes(b"payload.not_configured"),
        PayloadLaunchResult::Failed => klog::append_bytes(b"payload.failed"),
        PayloadLaunchResult::ExitCode(code) => {
            klog::append_bytes(b"payload.exit_code(");
            klog::append_usize_dec(code as usize);
            klog::append_bytes(b")");
        }
    }
}

fn blocked_kernel_log_stats() -> klog::Stats {
    klog::Stats {
        identity: klog::DiagnosticIdentity {
            boot_id: 0,
            session_id: 0,
            identity_source: "blocked",
        },
        capacity_bytes: 0,
        retained_bytes: 0,
        dropped_bytes: 0,
        next_event_seq: 0,
        retained_events: 0,
    }
}

fn blocked_boot_image() -> BootImageSummary {
    BootImageSummary::new(
        "blocked", "blocked", "blocked", "blocked", "blocked", "blocked", "blocked",
    )
}

fn blocked_console_input() -> ConsoleInputSummary {
    ConsoleInputSummary::with_usb_diagnostics(
        "blocked",
        "blocked",
        BootCheckState::Warn,
        BootCheckState::Warn,
        0,
        false,
        0,
        "blocked",
    )
}

fn blocked_scheduler_snapshot() -> SchedulerSnapshot {
    SchedulerSnapshot {
        current_task_id: usize::MAX,
        current_process_id: usize::MAX,
        ready_len: 0,
        ready_queue: [0; sched::MAX_KERNEL_TASKS],
        ready_process_queue: [0; sched::MAX_KERNEL_TASKS],
        dispatch_count: usize::MAX,
        yield_count: usize::MAX,
        tick_count: usize::MAX,
    }
}

const BLOCKED_DUMP_DEVICE_ENTRY: DeviceEntry = DeviceEntry {
    class: DeviceClass::Unknown,
    mmio_base: 0,
    mmio_len: 0,
    irq: u32::MAX,
    capacity_bytes: 0,
    compatible: "",
};

fn blocked_dump_sync_status() -> dump::DumpSyncStatus {
    dump::DumpSyncStatus {
        attempted: false,
        persistent_available: false,
        storage: "blocked",
        storage_capacity_bytes: 0,
        written: false,
        bytes_written: 0,
        checksum: 0,
        verified: false,
        reason: "blocked",
    }
}

fn blocked_dump_status() -> dump::DumpStatus {
    dump::DumpStatus {
        format_version: 0,
        identity: klog::DiagnosticIdentity {
            boot_id: 0,
            session_id: 0,
            identity_source: "blocked",
        },
        image: dump::DumpImageIdentity::new(
            "blocked", "blocked", "blocked", "blocked", "blocked", "blocked", "blocked",
        ),
        boot_info: BootInfo::default(),
        devices: [BLOCKED_DUMP_DEVICE_ENTRY; dump::MAX_DUMP_DEVICE_RECORDS],
        device_records: 0,
        proof_state: "blocked",
        panic_state: "blocked",
        panic_records: 0,
        panic_record: None,
        persistent_available: false,
        storage: "blocked",
        storage_capacity_bytes: 0,
        last_sync: blocked_dump_sync_status(),
        klog: blocked_kernel_log_stats(),
        event_records: 0,
        process_records: 0,
        service_records: 0,
        exec_load_records: 0,
        pending_exec_records: 0,
        wait_records: 0,
        task_records: 0,
        syscall_records: 0,
        syscall_continuation_records: 0,
    }
}

fn append_process_adoptions(parent: SyscallContext, adoption: ProcessAdoptionSnapshot) {
    let mut index = 0usize;
    while index < adoption.count {
        let child = adoption.children[index];
        let child_ctx = SyscallContext::from_process(child);
        record_context(child_ctx, SyscallOp::ProcessAdopt, SyscallStatus::Ok);
        klog::append_bytes(b"process.adopt old_parent_pid=");
        klog::append_usize_dec(parent.pid);
        klog::append_bytes(b" new_parent_pid=");
        klog::append_usize_dec(proc::ROOTD_PID);
        klog::append_bytes(b" child_pid=");
        klog::append_usize_dec(child.pid);
        klog::append_bytes(b" child_task=");
        klog::append_usize_dec(child.task_id);
        klog::append_bytes(b" path=");
        klog::append_bytes(child.program_path.as_bytes());
        klog::append_bytes(b"\n");
        append_process_control_event(child_ctx, SyscallOp::ProcessAdopt);
        index += 1;
    }
}

fn append_scheduler_event(ctx: SyscallContext, op: SyscallOp) {
    klog::append_event_with_source_context(
        "process",
        "sched",
        "info",
        op.as_str(),
        ctx.pid,
        ctx.task_id,
    );
}

fn append_wait_start(parent: SyscallContext, child: ProcessHandle, _wait: WaitRecord) {
    klog::append_bytes(b"wait.start parent_pid=");
    klog::append_usize_dec(parent.pid);
    klog::append_bytes(b" parent_task=");
    klog::append_usize_dec(parent.task_id);
    klog::append_bytes(b" child_pid=");
    klog::append_usize_dec(child.pid);
    klog::append_bytes(b" child_task=");
    klog::append_usize_dec(child.task_id);
    klog::append_bytes(b"\n");
    klog::append_event_with_context("proc", "info", "wait-start", parent.pid, parent.task_id);
}

fn append_wait_end(parent: SyscallContext, child: ProcessHandle, wait: WaitRecord) {
    klog::append_bytes(b"wait.end parent_pid=");
    klog::append_usize_dec(parent.pid);
    klog::append_bytes(b" child_pid=");
    klog::append_usize_dec(child.pid);
    klog::append_bytes(b" child_state=");
    klog::append_bytes(wait.child_state.as_str().as_bytes());
    klog::append_bytes(b" exit=");
    klog::append_usize_dec(wait.exit_code as usize);
    klog::append_bytes(b"\n");
    klog::append_event_with_context("proc", "info", "wait-end", parent.pid, parent.task_id);
}

fn append_wait_timeout(parent: SyscallContext, child: ProcessHandle, child_record: ProcessRecord) {
    klog::append_bytes(b"wait.timeout parent_pid=");
    klog::append_usize_dec(parent.pid);
    klog::append_bytes(b" child_pid=");
    klog::append_usize_dec(child.pid);
    klog::append_bytes(b" child_state=");
    klog::append_bytes(child_record.state.as_str().as_bytes());
    klog::append_bytes(b" exit=");
    klog::append_usize_dec(child_record.exit_code as usize);
    klog::append_bytes(b" tick_count=");
    klog::append_usize_dec(sched::snapshot_scheduler().tick_count);
    klog::append_bytes(b"\n");
    klog::append_event_with_context("proc", "info", "wait-timeout", parent.pid, parent.task_id);
}

fn parse_payload_source_hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn append_payload_start(
    parent: SyscallContext,
    child: ProcessHandle,
    payload: LoadedPayloadProgram,
) {
    klog::append_bytes(b"payload.start path=");
    klog::append_bytes(child.program_path.as_bytes());
    klog::append_bytes(b" loader=");
    klog::append_bytes(payload.image_kind.as_str().as_bytes());
    klog::append_bytes(b" entry_fn=");
    klog::append_bytes(payload.entry_name.as_bytes());
    klog::append_bytes(b" parent_pid=");
    klog::append_usize_dec(parent.pid);
    klog::append_bytes(b" parent_task=");
    klog::append_usize_dec(parent.task_id);
    klog::append_bytes(b" pid=");
    klog::append_usize_dec(child.pid);
    klog::append_bytes(b" task=");
    klog::append_usize_dec(child.task_id);
    klog::append_bytes(b"\n");
    klog::append_event_with_context("proc", "info", "payload-start", child.pid, child.task_id);
}

fn append_payload_wait_start(parent: SyscallContext, child: ProcessHandle, _wait: WaitRecord) {
    klog::append_bytes(b"wait.start parent_pid=");
    klog::append_usize_dec(parent.pid);
    klog::append_bytes(b" parent_task=");
    klog::append_usize_dec(parent.task_id);
    klog::append_bytes(b" child_pid=");
    klog::append_usize_dec(child.pid);
    klog::append_bytes(b" child_task=");
    klog::append_usize_dec(child.task_id);
    klog::append_bytes(b"\n");
    klog::append_event_with_context("proc", "info", "wait-start", parent.pid, parent.task_id);
}

fn append_payload_wait_end(parent: SyscallContext, child: ProcessHandle, wait: WaitRecord) {
    klog::append_bytes(b"wait.end parent_pid=");
    klog::append_usize_dec(parent.pid);
    klog::append_bytes(b" child_pid=");
    klog::append_usize_dec(child.pid);
    klog::append_bytes(b" child_state=");
    klog::append_bytes(wait.child_state.as_str().as_bytes());
    klog::append_bytes(b" exit=");
    klog::append_usize_dec(wait.exit_code as usize);
    klog::append_bytes(b"\n");
    klog::append_event_with_context("proc", "info", "wait-end", parent.pid, parent.task_id);
}

fn append_payload_wait_ready(parent: SyscallContext, child: ProcessHandle) {
    let child_record = proc::process(child.pid).unwrap_or(crate::proc::EMPTY_PROCESS_RECORD);
    klog::append_bytes(b"wait.ready parent_pid=");
    klog::append_usize_dec(parent.pid);
    klog::append_bytes(b" child_pid=");
    klog::append_usize_dec(child.pid);
    klog::append_bytes(b" child_state=");
    klog::append_bytes(child_record.state.as_str().as_bytes());
    klog::append_bytes(b" block=");
    klog::append_bytes(child_record.block_reason.as_str().as_bytes());
    klog::append_bytes(b"\n");
    klog::append_event_with_context("proc", "info", "wait-ready", parent.pid, parent.task_id);
}

fn append_payload_exit(child: ProcessHandle, result: PayloadLaunchResult) {
    klog::append_bytes(b"payload.exit path=");
    klog::append_bytes(child.program_path.as_bytes());
    klog::append_bytes(b" pid=");
    klog::append_usize_dec(child.pid);
    klog::append_bytes(b" task=");
    klog::append_usize_dec(child.task_id);
    klog::append_bytes(b" status=");
    klog::append_bytes(result.as_bytes());
    if let PayloadLaunchResult::ExitCode(code) = result {
        klog::append_bytes(b" exit=");
        klog::append_usize_dec(code as usize);
    }
    klog::append_bytes(b"\n");
    klog::append_event_with_context("proc", "info", "payload-exit", child.pid, child.task_id);
}

fn append_payload_resident(child: ProcessHandle) {
    klog::append_bytes(b"payload.resident path=");
    klog::append_bytes(child.program_path.as_bytes());
    klog::append_bytes(b" pid=");
    klog::append_usize_dec(child.pid);
    klog::append_bytes(b" task=");
    klog::append_usize_dec(child.task_id);
    klog::append_bytes(b" status=payload.resident block=service\n");
    klog::append_event_with_context("proc", "info", "payload-resident", child.pid, child.task_id);
}
