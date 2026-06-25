//! Typed Reovim syscall surface used by image-packaged programs.
//!
//! This is not a Linux syscall-number table. It is the kernel-local entry
//! vocabulary that `/bin` programs use to reach process, VFS, log, device, and
//! scheduler services, plus the retained child-context trace rows for scheduled
//! payload image execution.

use {
    crate::{
        dump, exec, exec_bundle, klog,
        proc::{self, ProcessHandle, ProcessRecord, WaitRecord},
        program::{
            self, BinSourceInstallStatus, LoadedProgram, MAX_PROGRAM_PIPE_BYTES,
            MAX_PROGRAM_STDIN_BYTES, ProgramArgvBuffer, ProgramDescriptor, ProgramStatus,
        },
        root_shell::{RootShellSession, execute_loaded_program_argv},
        rootd::{
            BootCheckState, BootImageSummary, ConsoleInputSummary, HardwareProbeResult,
            LoadedPayloadProgram, PayloadDescriptor, PayloadLaunchResult,
            PayloadSourceInstallStatus, RootDaemon,
        },
        sched::{self, KernelTaskRecord, SchedulerSnapshot},
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
        ptr::NonNull,
        sync::atomic::{AtomicBool, Ordering},
    },
};

pub use crate::program::ProgramArgv;

/// First non-stdio descriptor exposed by the current program ABI.
pub const PROGRAM_VFS_FILE_FD: usize = 3;
/// Maximum bytes readable from one opened VFS pseudo-file.
pub const MAX_PROGRAM_VFS_FILE_BYTES: usize = dump::MAX_DUMP_ARTIFACT_BYTES;

/// Bounded stdout capture used to feed one scheduled program's output into a
/// later program's stdin.
pub struct ProgramStdoutCapture {
    cell: UnsafeCell<ProgramStdoutCaptureState>,
}

struct ProgramVfsFileBuffer {
    cell: UnsafeCell<ProgramVfsFileBufferState>,
}

// SAFETY: mutable access is serialized by `PROGRAM_VFS_FILE_BUFFER_LOCK`.
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

static PROGRAM_VFS_FILE_BUFFER: ProgramVfsFileBuffer = ProgramVfsFileBuffer::new();
static PROGRAM_VFS_FILE_BUFFER_LOCK: AtomicBool = AtomicBool::new(false);

fn acquire_program_vfs_file_buffer() -> Option<NonNull<ProgramVfsFileBuffer>> {
    if PROGRAM_VFS_FILE_BUFFER_LOCK
        .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
        .is_err()
    {
        return None;
    }
    Some(NonNull::from(&PROGRAM_VFS_FILE_BUFFER))
}

fn release_program_vfs_file_buffer() {
    PROGRAM_VFS_FILE_BUFFER_LOCK.store(false, Ordering::Release);
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

/// System-kernel syscall operation recorded for `/bin` programs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyscallOp {
    /// Executable image metadata was loaded before spawn.
    ExecLoad,
    /// Root-shell exec spawned a `/bin` program process.
    ExecSpawn,
    /// Standard stream handles were attached to a program.
    StdioAttach,
    /// Program wrote to a file descriptor.
    FdWrite,
    /// Program read from a file descriptor.
    FdRead,
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
    /// Blocked process transitioned back to ready.
    ProcessWake,
    /// Retained process was explicitly terminated.
    ProcessKill,
    /// Scheduler selected a ready task for execution.
    SchedulerDispatch,
    /// Process exited.
    ProcessExit,
    /// Parent process began waiting for a child.
    WaitBegin,
    /// Parent process completed waiting for a child.
    WaitEnd,
    /// Current working directory query.
    SessionCwdGet,
    /// Current working directory update.
    SessionCwdSet,
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
    /// Executable load/admission snapshot query.
    SnapshotExecLoads,
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
    /// TTY/console line read request.
    TtyReadLine,
    /// TTY/console clear request.
    TtyClear,
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
            Self::StdioAttach => "stdio-attach",
            Self::FdWrite => "fd-write",
            Self::FdRead => "fd-read",
            Self::FdClose => "fd-close",
            Self::SpawnChild => "spawn-child",
            Self::ProcessRun => "process-run",
            Self::ProcessSelf => "process-self",
            Self::ProcessBlock => "process-block",
            Self::ProcessWake => "process-wake",
            Self::ProcessKill => "process-kill",
            Self::SchedulerDispatch => "scheduler-dispatch",
            Self::ProcessExit => "process-exit",
            Self::WaitBegin => "wait-begin",
            Self::WaitEnd => "wait-end",
            Self::SessionCwdGet => "session-cwd-get",
            Self::SessionCwdSet => "session-cwd-set",
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
            Self::SnapshotExecLoads => "snapshot-exec-loads",
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
            Self::TtyReadLine => "tty-read-line",
            Self::TtyClear => "tty-clear",
            Self::ProgramHelp => "program-help",
        }
    }
}

/// Result status for one typed syscall dispatch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyscallStatus {
    /// Dispatch completed normally.
    Ok,
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
    with_syscalls(SyscallTrace::reset);
}

/// Copies retained syscall dispatch records into `out`, returning the count.
pub fn snapshot_syscalls(out: &mut [SyscallRecord]) -> usize {
    with_syscalls(|trace| trace.snapshot(out))
}

fn record_syscall(current: Option<SyscallContext>, op: SyscallOp, status: SyscallStatus) {
    with_syscalls(|trace| trace.record(current, op, status));
}

fn record_context(ctx: SyscallContext, op: SyscallOp, status: SyscallStatus) {
    record_syscall(Some(ctx), op, status);
}

const fn process_exit_syscall_status(status: ProgramStatus) -> SyscallStatus {
    match status {
        ProgramStatus::Error => SyscallStatus::Error,
        ProgramStatus::ExitCode(code) => {
            if code == 0 {
                SyscallStatus::Ok
            } else {
                SyscallStatus::Error
            }
        }
        ProgramStatus::Empty | ProgramStatus::Ok | ProgramStatus::Halt => SyscallStatus::Ok,
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
        PayloadLaunchResult::NotConfigured => SyscallStatus::Unavailable,
        PayloadLaunchResult::Failed => SyscallStatus::Error,
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
    /// The fd exists but is not readable.
    NotReadable,
    /// The fd exists but is not writable.
    NotWritable,
}

impl ProgramIoError {
    /// Stable diagnostic word.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BadFd => "bad-fd",
            Self::NotReadable => "not-readable",
            Self::NotWritable => "not-writable",
        }
    }
}

/// Program-to-program exec failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProgramExecError {
    /// The syscall handle is not attached to a running process.
    NoCurrentProcess,
    /// The exec request supplied no argv[0].
    EmptyArgv,
    /// The requested executable name or `/bin` path was not found.
    ProgramNotFound,
    /// The requested executable source artifact was not found.
    SourceNotFound,
    /// The requested executable image failed loader validation.
    InvalidImage,
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
}

impl ProgramExecError {
    /// Stable diagnostic word.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoCurrentProcess => "no-current-process",
            Self::EmptyArgv => "empty-argv",
            Self::ProgramNotFound => "program-not-found",
            Self::SourceNotFound => "source-not-found",
            Self::InvalidImage => "invalid-image",
            Self::WaitFailed => "wait-failed",
            Self::SchedulerEmpty => "scheduler-empty",
            Self::MissingProgramImage => "missing-program-image",
            Self::DispatchBudgetExhausted => "dispatch-budget-exhausted",
            Self::BlockFailed => "block-failed",
        }
    }
}

/// Program process-control failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProgramProcessError {
    /// The syscall handle is not attached to a running process.
    NoCurrentProcess,
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

/// Runtime source install failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProgramSourceInstallError {
    /// The syscall handle is not attached to a running process.
    NoCurrentProcess,
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
    /// The source overlay rejected the install request.
    InstallFailed(SourceInstallError),
}

impl ProgramSourceInstallError {
    /// Stable diagnostic word.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoCurrentProcess => "no-current-process",
            Self::ProgramNotFound => "program-not-found",
            Self::PayloadNotFound => "payload-not-found",
            Self::SourceMediaUnavailable => "source-media-unavailable",
            Self::SourceMediaReadFailed => "source-media-read-failed",
            Self::SourceMediaInvalid => "source-media-invalid",
            Self::SourceMediaNamespaceMismatch => "source-media-namespace-mismatch",
            Self::SourceMediaPathMismatch => "source-media-path-mismatch",
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
}

impl SchedulerTickResult {
    const fn new(
        status: SchedulerTickStatus,
        ticked: bool,
        task_id: usize,
        tick_count: usize,
        task_ticks: usize,
    ) -> Self {
        Self {
            status,
            ticked,
            task_id,
            tick_count,
            task_ticks,
        }
    }
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
            | proc::ProcessState::Exited
            | proc::ProcessState::Failed
            | proc::ProcessState::Halted
    )
}

const fn process_state_is_complete(state: proc::ProcessState) -> bool {
    matches!(
        state,
        proc::ProcessState::Exited | proc::ProcessState::Failed | proc::ProcessState::Halted
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ProgramStdinBuffer {
    bytes: [u8; MAX_PROGRAM_STDIN_BYTES],
    len: usize,
    cursor: usize,
}

impl ProgramStdinBuffer {
    const fn empty() -> Self {
        Self {
            bytes: [0u8; MAX_PROGRAM_STDIN_BYTES],
            len: 0,
            cursor: 0,
        }
    }

    fn from_bytes(bytes: &[u8]) -> Self {
        let mut stdin = Self::empty();
        while stdin.len < bytes.len() && stdin.len < stdin.bytes.len() {
            stdin.bytes[stdin.len] = bytes[stdin.len];
            stdin.len += 1;
        }
        stdin
    }

    fn read(&mut self, out: &mut [u8]) -> usize {
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
struct ProgramVfsFdState {
    open: bool,
    cursor: usize,
    len: usize,
    read_recorded: bool,
}

impl ProgramVfsFdState {
    const fn closed() -> Self {
        Self {
            open: false,
            cursor: 0,
            len: 0,
            read_recorded: false,
        }
    }
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
    Ok(exec_bin_from_shell_with_stdin(program, argv, stdin))
}

/// Dispatches the next ready process and records the selected syscall context.
#[must_use]
pub fn dispatch_next_ready_program() -> Option<SyscallContext> {
    let dispatched = exec::dispatch_next_ready_process();
    let ctx = dispatched.map(SyscallContext::from_process)?;
    record_context(ctx, SyscallOp::SchedulerDispatch, SyscallStatus::Ok);
    record_context(ctx, SyscallOp::ProcessRun, process_run_status(dispatched, ctx.pid));
    Some(ctx)
}

/// Takes the pending invocation for a scheduler-selected process.
pub(crate) fn take_pending_exec(pid: usize) -> Option<exec::PendingProgramInvocation> {
    exec::take_pending_program(pid)
}

/// Completes the current image program.
pub(crate) fn exit_current(ctx: SyscallContext, status: ProgramStatus) {
    exec::complete_program(ctx.pid, status);
    record_context(ctx, SyscallOp::ProcessExit, process_exit_syscall_status(status));
}

/// Typed syscall handle exposed to image-linked `/bin` program entries.
pub struct ProgramSyscalls<'daemon, 'session, 'rootd> {
    daemon: &'daemon RootDaemon<'rootd>,
    session: &'session mut RootShellSession,
    current: Option<SyscallContext>,
    stdio: ProgramStdio,
    stdin: ProgramStdinBuffer,
    stdout_capture: Option<NonNull<ProgramStdoutCapture>>,
    vfs_file: ProgramVfsFdState,
    vfs_read_buffer: Option<NonNull<ProgramVfsFileBuffer>>,
    vfs_render_capture: Option<NonNull<ProgramVfsFileBuffer>>,
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
        Self {
            daemon,
            session,
            current,
            stdio: ProgramStdio::standard(),
            stdin: ProgramStdinBuffer::from_bytes(stdin),
            stdout_capture: stdout_capture.map(NonNull::from),
            vfs_file: ProgramVfsFdState::closed(),
            vfs_read_buffer: None,
            vfs_render_capture: None,
        }
    }

    /// Returns the standard stream handles attached to this program.
    #[must_use]
    pub fn stdio(&self) -> ProgramStdio {
        self.record(SyscallOp::StdioAttach, SyscallStatus::Ok);
        self.stdio
    }

    /// Returns the standard input handle attached to this program.
    #[must_use]
    pub const fn stdin(&self) -> ProgramStreamHandle {
        self.stdio.stdin
    }

    /// Returns the standard output handle attached to this program.
    #[must_use]
    pub const fn stdout(&self) -> ProgramStreamHandle {
        self.stdio.stdout
    }

    /// Returns the standard error handle attached to this program.
    #[must_use]
    pub const fn stderr(&self) -> ProgramStreamHandle {
        self.stdio.stderr
    }

    /// Returns the current process record, if this handle is attached.
    #[must_use]
    pub fn process_self(&self) -> Option<ProcessRecord> {
        let Some(current) = self.current else {
            self.record(SyscallOp::ProcessSelf, SyscallStatus::Unavailable);
            return None;
        };
        let record = proc::process(current.pid);
        self.record_result(SyscallOp::ProcessSelf, record.is_some());
        record
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

    /// Writes bytes to a Reovim program fd.
    ///
    /// Stdout writes remain untraced per fragment to keep the bounded syscall
    /// ring useful during verbose commands. Stderr and failed writes retain
    /// `fd-write` rows because they are operator-visible diagnostics.
    pub fn write_fd(&self, fd: usize, bytes: &[u8]) -> Result<usize, ProgramIoError> {
        if fd == self.stdout().fd {
            if let Some(capture) = self.vfs_render_capture {
                // SAFETY: the VFS file buffer lock is held by this syscall
                // handle during rendering.
                return Ok(unsafe { capture.as_ref() }.write(bytes));
            }
            if let Some(capture) = self.stdout_capture {
                // SAFETY: the capture pointer is created from a live reference
                // in the same synchronous dispatch frame as this syscall
                // handle.
                return Ok(unsafe { capture.as_ref() }.write(bytes));
            }
            self.daemon.write_bytes(bytes);
            return Ok(bytes.len());
        }
        if fd == self.stderr().fd {
            if !bytes.is_empty() {
                self.record(SyscallOp::FdWrite, SyscallStatus::Ok);
            }
            self.daemon.write_bytes(bytes);
            return Ok(bytes.len());
        }

        let error = if fd == self.stdin().fd {
            ProgramIoError::NotWritable
        } else if fd == PROGRAM_VFS_FILE_FD && self.vfs_file.open {
            ProgramIoError::NotWritable
        } else {
            ProgramIoError::BadFd
        };
        self.record(SyscallOp::FdWrite, SyscallStatus::Error);
        Err(error)
    }

    /// Writes one line to a Reovim program fd.
    pub fn write_fd_line(&self, fd: usize, line: &str) -> Result<usize, ProgramIoError> {
        if fd == self.stdout().fd {
            if let Some(capture) = self.vfs_render_capture {
                // SAFETY: the VFS file buffer lock is held by this syscall
                // handle during rendering.
                let capture = unsafe { capture.as_ref() };
                let mut written = capture.write(line.as_bytes());
                written += capture.write(b"\n");
                return Ok(written);
            }
            if let Some(capture) = self.stdout_capture {
                // SAFETY: same dispatch-frame lifetime as `write_fd`.
                let capture = unsafe { capture.as_ref() };
                let mut written = capture.write(line.as_bytes());
                written += capture.write(b"\n");
                return Ok(written);
            }
            self.daemon.write_line(line);
            return Ok(line.len().saturating_add(1));
        }
        if fd == self.stderr().fd {
            self.record(SyscallOp::FdWrite, SyscallStatus::Ok);
            self.daemon.write_line(line);
            return Ok(line.len().saturating_add(1));
        }

        let error = if fd == self.stdin().fd {
            ProgramIoError::NotWritable
        } else if fd == PROGRAM_VFS_FILE_FD && self.vfs_file.open {
            ProgramIoError::NotWritable
        } else {
            ProgramIoError::BadFd
        };
        self.record(SyscallOp::FdWrite, SyscallStatus::Error);
        Err(error)
    }

    fn record_vfs_fd_read_once(&mut self) {
        if !self.vfs_file.read_recorded {
            self.vfs_file.read_recorded = true;
            self.record(SyscallOp::FdRead, SyscallStatus::Ok);
        }
    }

    /// Reads bytes from a Reovim program fd.
    ///
    /// Default root-shell launches receive an empty stdin buffer, so reading fd
    /// 0 returns `Ok(0)` EOF unless a pipe or scheduled exec invocation seeded
    /// a bounded initial stdin payload. Interactive TTY input is intentionally
    /// exposed through [`ProgramSyscalls::read_tty_line`], not fd 0, until the
    /// stream/line discipline contract becomes blocking-capable.
    pub fn read_fd(&mut self, fd: usize, out: &mut [u8]) -> Result<usize, ProgramIoError> {
        if fd == self.stdin().fd {
            let read = self.stdin.read(out);
            self.record(SyscallOp::FdRead, SyscallStatus::Ok);
            return Ok(read);
        }
        if fd == PROGRAM_VFS_FILE_FD && self.vfs_file.open {
            let Some(buffer) = self.vfs_read_buffer else {
                self.record(SyscallOp::FdRead, SyscallStatus::Error);
                return Err(ProgramIoError::BadFd);
            };
            if self.vfs_file.cursor >= self.vfs_file.len {
                self.record_vfs_fd_read_once();
                return Ok(0);
            }
            // SAFETY: the open VFS fd owns the global file-buffer lock until close.
            let read = unsafe { buffer.as_ref() }.read_at(self.vfs_file.cursor, out);
            self.vfs_file.cursor = self.vfs_file.cursor.saturating_add(read);
            self.record_vfs_fd_read_once();
            return Ok(read);
        }

        let error = if fd == self.stdout().fd || fd == self.stderr().fd {
            ProgramIoError::NotReadable
        } else {
            ProgramIoError::BadFd
        };
        self.record(SyscallOp::FdRead, SyscallStatus::Error);
        Err(error)
    }

    /// Closes an open Reovim program fd.
    pub fn close_fd(&mut self, fd: usize) -> Result<(), ProgramIoError> {
        if fd == PROGRAM_VFS_FILE_FD && self.vfs_file.open {
            self.vfs_file = ProgramVfsFdState::closed();
            self.vfs_read_buffer = None;
            self.vfs_render_capture = None;
            release_program_vfs_file_buffer();
            self.record(SyscallOp::FdClose, SyscallStatus::Ok);
            return Ok(());
        }

        self.record(SyscallOp::FdClose, SyscallStatus::Error);
        Err(ProgramIoError::BadFd)
    }

    /// Reads one interactive TTY line through the root-daemon line callback.
    pub fn read_tty_line(&mut self, out: &mut [u8]) -> usize {
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
        self.record(SyscallOp::SessionCwdGet, SyscallStatus::Ok);
        self.session.cwd()
    }

    /// Sets the current shell session cwd.
    pub fn set_cwd(&mut self, path: PathBuf) {
        self.session.set_cwd(path);
        self.record(SyscallOp::SessionCwdSet, SyscallStatus::Ok);
    }

    /// Normalizes a path against the current shell session cwd.
    pub fn normalize_path(&self, path: &str) -> Result<PathBuf, VfsError> {
        let result = vfs::normalize(self.session.cwd(), path);
        self.record_result(SyscallOp::VfsNormalize, result.is_ok());
        result
    }

    /// Looks up a normalized path in the kernel VFS.
    pub fn lookup_path(&self, path: &str) -> Result<Node, VfsError> {
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
        if self.vfs_file.open {
            self.record(SyscallOp::VfsOpen, SyscallStatus::Error);
            return Err(VfsError::Busy);
        }

        let path = match self.normalize_path(target) {
            Ok(path) => path,
            Err(error) => {
                self.record(SyscallOp::VfsOpen, SyscallStatus::Error);
                return Err(error);
            }
        };
        match self.lookup_path(path.as_str()) {
            Ok(Node::File(file)) => {
                let Some(buffer) = acquire_program_vfs_file_buffer() else {
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
                    release_program_vfs_file_buffer();
                    self.record(SyscallOp::VfsOpen, SyscallStatus::Error);
                    self.record(SyscallOp::VfsRead, SyscallStatus::Error);
                    return Err(VfsError::FileTooLarge);
                }
                self.vfs_file = ProgramVfsFdState {
                    open: true,
                    cursor: 0,
                    len: buffer_ref.len(),
                    read_recorded: false,
                };
                self.vfs_read_buffer = Some(buffer);
                self.record(SyscallOp::VfsOpen, SyscallStatus::Ok);
                self.record(SyscallOp::VfsRead, SyscallStatus::Ok);
                Ok(PROGRAM_VFS_FILE_FD)
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
        let writer = self.daemon.program_help_writer();
        let status = writer(program_name, self);
        self.record_result(SyscallOp::ProgramHelp, matches!(status, ProgramStatus::Ok));
        status
    }

    /// Writes the kernel VFS mount table to stdout.
    pub fn write_mount_table(&self) {
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
        self.daemon.programs()
    }

    /// Returns device inventory exposed to kernel programs.
    #[must_use]
    pub fn devices(&self) -> &[reovim_uapi_system::DeviceEntry] {
        self.record(SyscallOp::DeviceCatalog, SyscallStatus::Ok);
        self.daemon.devices()
    }

    /// Writes a boot-info summary to stdout.
    pub fn write_boot_info_summary(&self) {
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
        self.record(SyscallOp::BootInfo, SyscallStatus::Ok);
        self.daemon.boot_info()
    }

    /// Returns the selected root profile name.
    #[must_use]
    pub fn profile_name(&self) -> &'static str {
        self.record(SyscallOp::BootProfile, SyscallStatus::Ok);
        self.daemon.profile_name()
    }

    /// Returns the selected root-shell prompt.
    #[must_use]
    pub fn prompt(&self) -> &'static str {
        self.daemon.prompt()
    }

    /// Returns whether payload launch is enabled for the selected profile.
    #[must_use]
    pub fn launch_enabled(&self) -> bool {
        self.record(SyscallOp::BootProfile, SyscallStatus::Ok);
        self.daemon.launch_enabled()
    }

    /// Returns compile-time image identity.
    #[must_use]
    pub fn boot_image(&self) -> BootImageSummary {
        self.record(SyscallOp::BootImage, SyscallStatus::Ok);
        self.daemon.boot_image()
    }

    /// Returns live console-input status.
    #[must_use]
    pub fn console_input(&self) -> ConsoleInputSummary {
        self.record(SyscallOp::ConsoleInput, SyscallStatus::Ok);
        self.daemon.console_input()
    }

    /// Writes boot image, profile, input, and next-action status to stdout.
    pub fn write_boot_status(&self) {
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
        self.stdout_line("  manual_next=type-shell-command");
        self.stdout_line("  probe usb-keyboard reports manual_next=type-shell-command");
        self.stdout_line("  detailed help catalog available through /boot/help");
        self.stdout_line("  screentest includes erase-line mode diagnostics");
        self.stdout_line("  kernel log stats available through /log/stats");
        self.stdout_line("  kernel structured events available through /log/events");
        self.stdout_line("  dump status available through /bin/dump");
        self.stdout_line("  dump sync fails closed until persistent storage is available");
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
        let Some(parent) = self.current else {
            self.record(SyscallOp::ExecLoad, SyscallStatus::Unavailable);
            return Err(ProgramExecError::NoCurrentProcess);
        };

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
            };

        let parent_handle = ProcessHandle {
            pid: parent.pid,
            task_id: parent.task_id,
            program_path: parent.program_path,
            loader: parent.loader,
            entry_name: parent.entry_name,
        };
        let child = exec::spawn_program_child_with_stdin(parent_handle, program, argv, &[]);
        let child_ctx = SyscallContext::from_process(child);
        record_context(child_ctx, SyscallOp::ExecLoad, SyscallStatus::Ok);
        record_context(child_ctx, SyscallOp::ExecSpawn, SyscallStatus::Ok);
        record_context(child_ctx, SyscallOp::SpawnChild, SyscallStatus::Ok);

        let Some(wait) = proc::begin_wait(parent.pid, child.pid) else {
            record_context(parent, SyscallOp::WaitBegin, SyscallStatus::Error);
            exec::discard_pending_program(child.pid);
            exec::complete_program(child.pid, ProgramStatus::Error);
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
                let status = self.run_selected_program_child(ctx)?;
                self.finish_program_wait(parent, child);
                return Ok(status);
            }

            if matches!(self.run_selected_non_wait_child(ctx), Some(ProgramStatus::Halt)) {
                return Ok(ProgramStatus::Halt);
            }
            steps += 1;
        }

        Err(ProgramExecError::DispatchBudgetExhausted)
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
        let Some(parent) = self.current else {
            self.record(SyscallOp::ExecLoad, SyscallStatus::Unavailable);
            return Err(ProgramExecError::NoCurrentProcess);
        };

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
            };

        let parent_handle = ProcessHandle {
            pid: parent.pid,
            task_id: parent.task_id,
            program_path: parent.program_path,
            loader: parent.loader,
            entry_name: parent.entry_name,
        };
        let child = exec::spawn_program_child_with_stdin(parent_handle, program, argv, &[]);
        let child_ctx = SyscallContext::from_process(child);
        record_context(child_ctx, SyscallOp::ExecLoad, SyscallStatus::Ok);
        record_context(child_ctx, SyscallOp::ExecSpawn, SyscallStatus::Ok);
        record_context(child_ctx, SyscallOp::SpawnChild, SyscallStatus::Ok);
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
        let child = self.spawn_program_argv(argv)?;
        let child_ctx = SyscallContext::from_process(child);
        let Some(blocked) = proc::block_process(child.pid) else {
            record_context(child_ctx, SyscallOp::ProcessBlock, SyscallStatus::Error);
            exec::discard_pending_program(child.pid);
            exec::complete_program(child.pid, ProgramStatus::Error);
            record_context(child_ctx, SyscallOp::ProcessExit, SyscallStatus::Error);
            append_program_exec_log(child_ctx, ProgramStatus::Error);
            return Err(ProgramExecError::BlockFailed);
        };
        record_context(
            SyscallContext::from_process(blocked),
            SyscallOp::ProcessBlock,
            SyscallStatus::Ok,
        );
        Ok(blocked)
    }

    /// Waits for a retained process by PID.
    ///
    /// This is a Reovim process-control syscall, not POSIX `waitpid`: the
    /// target is selected by retained PID, may have been adopted by rootd, and
    /// must be ready or already complete in the current cooperative scheduler.
    pub fn wait_process_by_pid(&mut self, pid: usize) -> Result<WaitRecord, ProgramProcessError> {
        let Some(parent) = self.current else {
            self.record(SyscallOp::WaitBegin, SyscallStatus::Unavailable);
            return Err(ProgramProcessError::NoCurrentProcess);
        };
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
                let _ = proc::cancel_wait(parent.pid, child.pid);
                record_context(parent, SyscallOp::WaitEnd, SyscallStatus::Error);
                return Err(ProgramProcessError::SchedulerEmpty);
            };

            if ctx.pid == child.pid {
                self.run_selected_non_wait_child(ctx);
                return self.finish_wait_by_pid(parent, child);
            }

            self.run_selected_non_wait_child(ctx);
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
        let Some(record) = proc::process(pid) else {
            self.record(SyscallOp::ProcessWake, SyscallStatus::Error);
            return Err(ProgramProcessError::ProcessNotFound);
        };
        if record.state != proc::ProcessState::Blocked {
            self.record(SyscallOp::ProcessWake, SyscallStatus::Error);
            return Err(ProgramProcessError::NotBlocked);
        }
        let Some(handle) = proc::wake_process(pid) else {
            self.record(SyscallOp::ProcessWake, SyscallStatus::Error);
            return Err(ProgramProcessError::ProcessNotFound);
        };
        record_context(
            SyscallContext::from_process(handle),
            SyscallOp::ProcessWake,
            SyscallStatus::Ok,
        );
        Ok(handle)
    }

    /// Terminates a retained ready or blocked process by PID.
    pub fn kill_process_by_pid(
        &mut self,
        pid: usize,
    ) -> Result<ProcessHandle, ProgramProcessError> {
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
        exec::complete_program(pid, ProgramStatus::Error);
        let ctx = SyscallContext::from_process(handle);
        record_context(ctx, SyscallOp::ProcessKill, SyscallStatus::Ok);
        record_context(ctx, SyscallOp::ProcessExit, SyscallStatus::Error);
        append_program_exec_log(ctx, ProgramStatus::Error);
        Ok(handle)
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

        let Some((_, descriptor)) = program::resolve_argv0(self.daemon.programs(), name) else {
            self.record(SyscallOp::SourceInstall, SyscallStatus::Error);
            return Err(ProgramSourceInstallError::ProgramNotFound);
        };
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

        let Some((_, descriptor)) = program::resolve_argv0(self.daemon.programs(), name) else {
            self.record(SyscallOp::SourceInstall, SyscallStatus::Error);
            return Err(ProgramSourceInstallError::ProgramNotFound);
        };
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
        self.record(SyscallOp::PayloadCatalog, SyscallStatus::Ok);
        self.daemon.payloads()
    }

    /// Launches a registered payload by name through the root supervisor.
    pub fn launch_payload_by_name(&mut self, name: &str) -> PayloadLaunchResult {
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
        let child = exec::spawn_payload_child(parent_handle, payload);
        let child_ctx = SyscallContext::from_process(child);
        record_context(child_ctx, SyscallOp::SpawnChild, SyscallStatus::Ok);
        let Some(wait) = proc::begin_wait(parent.pid, child.pid) else {
            record_context(parent, SyscallOp::WaitBegin, SyscallStatus::Error);
            exec::discard_pending_payload(child.pid);
            exec::complete_payload(child.pid, PayloadLaunchResult::Failed);
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
                exec::complete_payload(child.pid, result);
                record_context(child_ctx, SyscallOp::ProcessExit, SyscallStatus::Error);
                self.finish_payload_wait(parent, child);
                self.record_payload_launch(result);
                return result;
            };

            if ctx.pid == child.pid {
                let result = self.run_selected_payload_child(parent, child);
                self.finish_payload_wait(parent, child);
                self.record_payload_launch(result);
                return result;
            }

            self.run_selected_non_wait_child(ctx);
            steps += 1;
        }

        let result = PayloadLaunchResult::Failed;
        exec::discard_pending_payload(child.pid);
        exec::complete_payload(child.pid, result);
        record_context(child_ctx, SyscallOp::ProcessExit, SyscallStatus::Error);
        self.finish_payload_wait(parent, child);
        self.record_payload_launch(result);
        result
    }

    fn run_selected_payload_child(
        &self,
        parent: SyscallContext,
        child: ProcessHandle,
    ) -> PayloadLaunchResult {
        let result = match exec::take_pending_payload(child.pid) {
            Some(pending) => self.run_pending_payload(parent, pending.child(), pending.payload()),
            None => PayloadLaunchResult::Failed,
        };
        exec::complete_payload(child.pid, result);
        record_context(
            SyscallContext::from_process(child),
            SyscallOp::ProcessExit,
            payload_syscall_status(result),
        );
        append_payload_exit(child, result);
        result
    }

    fn run_pending_payload(
        &self,
        parent: SyscallContext,
        child: ProcessHandle,
        payload: LoadedPayloadProgram,
    ) -> PayloadLaunchResult {
        append_payload_start(parent, child, payload);
        let result = self.daemon.run_loaded_payload(payload);
        record_context(
            SyscallContext::from_process(child),
            SyscallOp::PayloadRun,
            payload_syscall_status(result),
        );
        result
    }

    fn run_selected_program_child(
        &mut self,
        ctx: SyscallContext,
    ) -> Result<ProgramStatus, ProgramExecError> {
        let Some(pending) = take_pending_exec(ctx.pid) else {
            exit_current(ctx, ProgramStatus::Error);
            append_program_exec_log(ctx, ProgramStatus::Error);
            return Err(ProgramExecError::MissingProgramImage);
        };
        Ok(self.run_pending_program(ctx, pending))
    }

    fn run_pending_program(
        &mut self,
        ctx: SyscallContext,
        pending: exec::PendingProgramInvocation,
    ) -> ProgramStatus {
        let pending_ctx = SyscallContext::from_process(pending.handle());
        let status = execute_loaded_program_argv(
            self.daemon,
            self.session,
            pending.program(),
            pending.argv(),
            pending.stdin(),
            None,
            Some(pending_ctx),
        )
        .status();
        exit_current(ctx, status);
        append_program_exec_log(ctx, status);
        status
    }

    fn run_selected_non_wait_child(&mut self, ctx: SyscallContext) -> Option<ProgramStatus> {
        if let Some(pending) = take_pending_exec(ctx.pid) {
            return Some(self.run_pending_program(ctx, pending));
        }

        if let Some(pending) = exec::take_pending_payload(ctx.pid) {
            let parent = SyscallContext::from_process(pending.parent());
            let child = pending.child();
            let result = self.run_pending_payload(parent, child, pending.payload());
            exec::complete_payload(child.pid, result);
            record_context(
                SyscallContext::from_process(child),
                SyscallOp::ProcessExit,
                payload_syscall_status(result),
            );
            append_payload_exit(child, result);
            self.finish_payload_wait(parent, child);
            return None;
        }

        exit_current(ctx, ProgramStatus::Error);
        append_program_exec_log(ctx, ProgramStatus::Error);
        Some(ProgramStatus::Error)
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
        } else {
            record_context(parent, SyscallOp::WaitEnd, SyscallStatus::Error);
            Err(ProgramProcessError::WaitFailed)
        }
    }

    fn finish_payload_wait(&self, parent: SyscallContext, child: ProcessHandle) {
        if let Some(wait) = proc::finish_wait(parent.pid, child.pid) {
            record_context(parent, SyscallOp::WaitEnd, SyscallStatus::Ok);
            append_payload_wait_end(parent, child, wait);
        } else {
            record_context(parent, SyscallOp::WaitEnd, SyscallStatus::Error);
        }
    }

    /// Runs a lower-provider hardware probe, if installed.
    pub fn run_hardware_probe(&self, target: &str) -> Option<HardwareProbeResult> {
        let result = self.daemon.run_hardware_probe(target);
        let status = match result {
            Some(HardwareProbeResult::Handled) => SyscallStatus::Ok,
            Some(HardwareProbeResult::UnknownTarget) => SyscallStatus::Error,
            None => SyscallStatus::Unavailable,
        };
        self.record(SyscallOp::ProviderProbe, status);
        result
    }

    /// Writes retained kernel log bytes to stdout.
    pub fn write_kernel_log(&self) -> bool {
        let wrote = klog::write_to(|bytes| self.stdout_bytes(bytes));
        self.record(SyscallOp::KernelLogRead, SyscallStatus::Ok);
        wrote
    }

    /// Writes the operator dmesg view to stdout.
    pub fn write_kernel_log_view(&self) {
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
        match self.dispatch(SyscallRequest::KernelLogStats) {
            SyscallResult::KernelLogStats(stats) => stats,
            _ => klog::stats(),
        }
    }

    /// Writes retained kernel-log metadata to stdout.
    pub fn write_kernel_log_stats(&self) {
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
        let count = klog::snapshot_events(out);
        self.record(SyscallOp::SnapshotKernelEvents, SyscallStatus::Ok);
        count
    }

    /// Returns the optional extra diagnostics snapshot.
    #[must_use]
    pub fn external_dmesg(&self) -> Option<&'static str> {
        self.daemon.dmesg_fn().map(|snapshot| snapshot())
    }

    /// Returns live dump status.
    #[must_use]
    pub fn dump_status(&self) -> dump::DumpStatus {
        match self.dispatch(SyscallRequest::DumpStatus) {
            SyscallResult::DumpStatus(status) => status,
            _ => dump::status(),
        }
    }

    /// Attempts to flush retained dump state to persistent storage.
    #[must_use]
    pub fn dump_sync(&self) -> dump::DumpSyncStatus {
        match self.dispatch(SyscallRequest::DumpSync) {
            SyscallResult::DumpSync(status) => status,
            _ => dump::sync(),
        }
    }

    /// Writes live dump status to stdout.
    pub fn write_dump_status(&self) {
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
        self.stdout_bytes(b"\n");
    }

    /// Writes a bounded dump snapshot to stdout.
    pub fn write_dump_snapshot(&self) {
        self.stdout_line("dump:");
        let status = self.dump_status();
        match dump::encode_status_artifact(status) {
            Ok(artifact) => self.stdout_bytes(artifact.as_bytes()),
            Err(_) => self.stdout_line("snapshot=encode-error"),
        }
    }

    /// Attempts to flush retained dump state and writes the sync report.
    pub fn write_dump_sync(&self) {
        let status = self.dump_sync();
        self.stdout_line("dump sync:");
        self.stdout_bytes(b"persistent=");
        self.stdout_bytes(if status.persistent_available {
            b"available"
        } else {
            b"unavailable"
        });
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
        let count = proc::snapshot(out);
        self.record(SyscallOp::SnapshotProcesses, SyscallStatus::Ok);
        count
    }

    /// Copies retained executable load/admission records into `out`.
    pub fn snapshot_exec_loads(&self, out: &mut [exec::ExecLoadRecord]) -> usize {
        let count = exec::snapshot_loads(out);
        self.record(SyscallOp::SnapshotExecLoads, SyscallStatus::Ok);
        count
    }

    /// Copies retained pending executable invocation records into `out`.
    pub fn snapshot_pending_execs(&self, out: &mut [exec::PendingExecRecord]) -> usize {
        let count = exec::snapshot_pending(out);
        self.record(SyscallOp::SnapshotPendingExecs, SyscallStatus::Ok);
        count
    }

    /// Copies executable source artifact records into `out`.
    pub fn snapshot_source_store(&self, out: &mut [SourceArtifactRecord]) -> usize {
        let count = self.daemon.source_store().snapshot(out);
        self.record(SyscallOp::SnapshotSourceStore, SyscallStatus::Ok);
        count
    }

    /// Copies retained wait records into `out`, returning the count copied.
    pub fn snapshot_waits(&self, out: &mut [WaitRecord]) -> usize {
        let count = proc::snapshot_waits(out);
        self.record(SyscallOp::SnapshotWaits, SyscallStatus::Ok);
        count
    }

    /// Writes retained process records to stdout.
    pub fn write_process_table(&self) {
        let mut records = [proc::EMPTY_PROCESS_RECORD; proc::MAX_PROCESSES];
        let count = self.snapshot_processes(&mut records);
        self.stdout_line("processes:");
        let mut index = 0usize;
        while index < count {
            self.write_process_record(records[index]);
            index += 1;
        }
    }

    /// Writes the current process record to stdout.
    pub fn write_current_process(&self) {
        self.stdout_line("self:");
        let Some(record) = self.process_self() else {
            self.stderr_line("proc: current process unavailable");
            return;
        };
        self.write_process_record(record);
    }

    /// Writes retained executable load/admission records to stdout.
    pub fn write_exec_load_table(&self) {
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
            self.stdout_bytes(b"\n");
            index += 1;
        }
    }

    /// Writes retained pending executable invocation records to stdout.
    pub fn write_pending_exec_table(&self) {
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
            self.stdout_bytes(b" stdin_bytes=");
            self.write_u64_dec(record.stdin_len as u64);
            self.stdout_bytes(b"\n");
            index += 1;
        }
    }

    /// Writes executable source artifacts to stdout.
    pub fn write_source_store_table(&self) {
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

    /// Copies retained scheduler task records into `out`, returning the count copied.
    pub fn snapshot_tasks(&self, out: &mut [KernelTaskRecord]) -> usize {
        let count = sched::snapshot_kernel_tasks(out);
        self.record(SyscallOp::SnapshotTasks, SyscallStatus::Ok);
        count
    }

    /// Returns scheduler run-queue metadata.
    #[must_use]
    pub fn scheduler_snapshot(&self) -> SchedulerSnapshot {
        match self.dispatch(SyscallRequest::SchedulerSnapshot) {
            SyscallResult::SchedulerSnapshot(snapshot) => snapshot,
            _ => sched::snapshot_scheduler(),
        }
    }

    /// Writes scheduler run-queue metadata to stdout.
    pub fn write_scheduler_state(&self) {
        let snapshot = self.scheduler_snapshot();
        self.stdout_line("scheduler:");
        self.stdout_bytes(b"current_task=");
        self.write_u64_dec(snapshot.current_task_id as u64);
        self.stdout_bytes(b"\nready_queue_len=");
        self.write_u64_dec(snapshot.ready_len as u64);
        self.stdout_bytes(b"\nnext_ready_task=");
        self.write_u64_dec(snapshot.next_ready_task_id() as u64);
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
            self.stdout_bytes(b"\n");
            index += 1;
        }
    }

    /// Cooperatively yields the current program task and returns a typed result.
    pub fn yield_now_result(&mut self) -> SchedulerYieldResult {
        let Some(current) = self.current else {
            record_syscall(self.current, SyscallOp::YieldNow, SyscallStatus::Unavailable);
            return SchedulerYieldResult::new(SchedulerYieldStatus::Unavailable, false, 0, 0);
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

    /// Records one explicit scheduler tick for the current running program task.
    pub fn scheduler_tick_result(&self) -> SchedulerTickResult {
        let Some(current) = self.current else {
            record_syscall(self.current, SyscallOp::SchedulerTick, SyscallStatus::Unavailable);
            return SchedulerTickResult::new(SchedulerTickStatus::Unavailable, false, 0, 0, 0);
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
            );
        };

        record_syscall(self.current, SyscallOp::SchedulerTick, SyscallStatus::Ok);
        let snapshot = sched::snapshot_scheduler();
        SchedulerTickResult::new(
            SchedulerTickStatus::Ok,
            true,
            task.task_id,
            snapshot.tick_count,
            task.runtime_ticks,
        )
    }

    /// Writes the result of one explicit scheduler tick to stdout.
    pub fn write_scheduler_tick(&self) {
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
        self.stdout_bytes(b"\n");
    }

    /// Copies retained typed syscall dispatch records into `out`.
    pub fn snapshot_syscalls(&self, out: &mut [SyscallRecord]) -> usize {
        record_syscall(self.current, SyscallOp::SnapshotSyscalls, SyscallStatus::Ok);
        snapshot_syscalls(out)
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
                self.stdout_line("probes");
                self.stdout_line("proof");
                self.stdout_line("profile");
                self.stdout_line("status");
            }
            Directory::Dev => {
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
                self.stdout_line("execs");
                self.stdout_line("media");
                self.stdout_line("pending");
                self.stdout_line("processes");
                self.stdout_line("self");
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
                record_syscall(
                    self.current,
                    request.op(),
                    if status.written {
                        SyscallStatus::Ok
                    } else if status.persistent_available {
                        SyscallStatus::Error
                    } else {
                        SyscallStatus::Unavailable
                    },
                );
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
        crate::console::clear_screen();
        self.record(SyscallOp::TtyClear, SyscallStatus::Ok);
    }
}

impl Drop for ProgramSyscalls<'_, '_, '_> {
    fn drop(&mut self) {
        if self.vfs_file.open {
            self.vfs_file = ProgramVfsFdState::closed();
            self.vfs_read_buffer = None;
            self.vfs_render_capture = None;
            release_program_vfs_file_buffer();
        }
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
    klog::append_event_with_context("proc", "info", "program-exit", ctx.pid, ctx.task_id);
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

fn payload_result_bytes(result: PayloadLaunchResult) -> &'static [u8] {
    match result {
        PayloadLaunchResult::Ready => b"payload.ready",
        PayloadLaunchResult::NotConfigured => b"payload.not_configured",
        PayloadLaunchResult::Failed => b"payload.failed",
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

fn append_payload_exit(child: ProcessHandle, result: PayloadLaunchResult) {
    klog::append_bytes(b"payload.exit path=");
    klog::append_bytes(child.program_path.as_bytes());
    klog::append_bytes(b" pid=");
    klog::append_usize_dec(child.pid);
    klog::append_bytes(b" task=");
    klog::append_usize_dec(child.task_id);
    klog::append_bytes(b" status=");
    klog::append_bytes(payload_result_bytes(result));
    klog::append_bytes(b"\n");
    klog::append_event_with_context("proc", "info", "payload-exit", child.pid, child.task_id);
}
