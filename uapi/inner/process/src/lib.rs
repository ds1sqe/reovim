//! Product-facing process syscall vocabulary.
//!
//! This leaf owns process-domain semantics such as process identity and exit
//! requests. The raw syscall leaf remains only the transport spine.

#![no_std]

use {
    core::mem::size_of,
    reovim_uapi_syscall::{RawSyscall, SyscallArgs, SyscallError, SyscallNr},
};

/// Maximum executable path bytes retained in a process-control report.
pub const PROCESS_REPORT_PATH_BYTES: usize = 64;
/// Maximum executable loader bytes retained in a process-control report.
pub const PROCESS_REPORT_LOADER_BYTES: usize = 32;
/// Maximum executable entry-name bytes retained in a process-control report.
pub const PROCESS_REPORT_ENTRY_NAME_BYTES: usize = 64;
/// Maximum executable body-format bytes retained in a process-control report.
pub const PROCESS_REPORT_BODY_FORMAT_BYTES: usize = 32;
/// Maximum checked executable body-contract bytes retained in a process-control report.
pub const PROCESS_REPORT_BODY_INNER_BYTES: usize = 32;

/// Product-facing process identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct ProcessId(usize);

impl ProcessId {
    /// Builds a process id from a system-kernel-issued scalar.
    #[must_use]
    pub const fn new(raw: usize) -> Self {
        Self(raw)
    }

    /// Returns the system-kernel-issued scalar.
    #[must_use]
    pub const fn raw(self) -> usize {
        self.0
    }
}

/// One borrowed process argument passed to spawn/exec-style process syscalls.
///
/// The raw transport receives a pointer to a contiguous slice of these records.
/// The system kernel copies the pointed-to UTF-8 bytes synchronously into its
/// bounded argv storage before the syscall returns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct ProcessArg {
    ptr: *const u8,
    len: usize,
}

impl ProcessArg {
    /// Builds a borrowed process argument from UTF-8 text.
    #[must_use]
    pub fn from_str(text: &str) -> Self {
        Self {
            ptr: text.as_ptr(),
            len: text.len(),
        }
    }

    /// Returns the borrowed byte pointer.
    #[must_use]
    pub const fn ptr(self) -> *const u8 {
        self.ptr
    }

    /// Returns the borrowed byte length.
    #[must_use]
    pub const fn len(self) -> usize {
        self.len
    }

    /// Returns whether this argument is empty.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.len == 0
    }
}

/// One borrowed process environment entry passed to spawn/exec-style process syscalls.
///
/// The raw transport receives a pointer to a contiguous slice of these records.
/// The system kernel copies the pointed-to UTF-8 bytes synchronously into its
/// bounded environment storage before the syscall returns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct ProcessEnv {
    name_ptr: *const u8,
    name_len: usize,
    value_ptr: *const u8,
    value_len: usize,
}

impl ProcessEnv {
    /// Builds a borrowed process environment entry from UTF-8 text.
    #[must_use]
    pub fn from_pair(name: &str, value: &str) -> Self {
        Self {
            name_ptr: name.as_ptr(),
            name_len: name.len(),
            value_ptr: value.as_ptr(),
            value_len: value.len(),
        }
    }

    /// Returns the borrowed variable-name byte pointer.
    #[must_use]
    pub const fn name_ptr(self) -> *const u8 {
        self.name_ptr
    }

    /// Returns the borrowed variable-name byte length.
    #[must_use]
    pub const fn name_len(self) -> usize {
        self.name_len
    }

    /// Returns the borrowed variable-value byte pointer.
    #[must_use]
    pub const fn value_ptr(self) -> *const u8 {
        self.value_ptr
    }

    /// Returns the borrowed variable-value byte length.
    #[must_use]
    pub const fn value_len(self) -> usize {
        self.value_len
    }
}

/// Raw process-spawn target owned by the process domain.
///
/// The raw syscall leaf carries this value as a scalar argument, but the public
/// semantic choice belongs here: a caller either spawns an image `/bin` program
/// or a configured `/payload` program.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct ProcessSpawnTarget(usize);

impl ProcessSpawnTarget {
    const ENV_FLAG: usize = 1usize << 8;
    const REQUEST_FLAG: usize = 1usize << 9;

    /// Spawn a `/bin` program from `argv[0]`.
    pub const BIN: Self = Self(0);
    /// Spawn a `/payload` program from `argv[0]`.
    pub const PAYLOAD: Self = Self(1);

    /// Returns the same target with env slots enabled for non-reporting calls.
    #[must_use]
    pub const fn with_env(self) -> Self {
        Self(self.0 | Self::ENV_FLAG)
    }

    /// Returns the same target with the structured-request transport enabled.
    #[must_use]
    pub const fn with_request(self) -> Self {
        Self(self.0 | Self::REQUEST_FLAG)
    }

    /// Returns the target without any process-domain transport flags.
    #[must_use]
    pub const fn base(self) -> Self {
        Self(self.0 & !(Self::ENV_FLAG | Self::REQUEST_FLAG))
    }

    /// Returns whether the env transport flag is set.
    #[must_use]
    pub const fn carries_env(self) -> bool {
        self.0 & Self::ENV_FLAG != 0
    }

    /// Returns whether the structured-request transport flag is set.
    #[must_use]
    pub const fn carries_request(self) -> bool {
        self.0 & Self::REQUEST_FLAG != 0
    }

    /// Returns the raw transport scalar.
    #[must_use]
    pub const fn raw(self) -> usize {
        self.0
    }
}

/// Structured process-spawn request for env plus report-bearing calls.
///
/// The raw syscall leaf carries a pointer to this record when the process
/// target has [`ProcessSpawnTarget::with_request`] set. The semantic owner is
/// still this process uapi leaf; raw transport only carries the pointer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct ProcessSpawnRequest {
    argv_ptr: *const ProcessArg,
    argc: usize,
    env_ptr: *const ProcessEnv,
    envc: usize,
    target: usize,
    mode: usize,
    flags: usize,
    report_ptr: usize,
    report_len: usize,
}

/// Extension flags for [`ProcessSpawnRequest`].
///
/// No nonzero flags are defined yet. Callers must pass [`Self::empty`] and the
/// kernel must reject unknown bits so this record can grow deliberately.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct ProcessSpawnFlags(usize);

impl ProcessSpawnFlags {
    /// No spawn request extension flags.
    #[must_use]
    pub const fn empty() -> Self {
        Self(0)
    }

    /// Builds flags from raw bits.
    #[must_use]
    pub const fn from_raw(raw: usize) -> Self {
        Self(raw)
    }

    /// Returns raw flag bits.
    #[must_use]
    pub const fn raw(self) -> usize {
        self.0
    }

    /// Returns whether this value contains only known flags.
    #[must_use]
    pub const fn is_known(self) -> bool {
        self.0 == 0
    }
}

impl ProcessSpawnRequest {
    /// Builds a structured request without a report pointer.
    #[must_use]
    pub fn new(
        target: ProcessSpawnTarget,
        mode: ProcessSpawnMode,
        argv: &[ProcessArg],
        env: &[ProcessEnv],
    ) -> Self {
        Self {
            argv_ptr: argv.as_ptr(),
            argc: argv.len(),
            env_ptr: if env.is_empty() {
                core::ptr::null()
            } else {
                env.as_ptr()
            },
            envc: env.len(),
            target: target.base().raw(),
            mode: mode.raw(),
            flags: ProcessSpawnFlags::empty().raw(),
            report_ptr: 0,
            report_len: 0,
        }
    }

    /// Builds a structured request with a process-control report.
    #[must_use]
    pub fn with_process_report(
        target: ProcessSpawnTarget,
        mode: ProcessSpawnMode,
        argv: &[ProcessArg],
        env: &[ProcessEnv],
        report: &mut ProcessControlReport,
    ) -> Self {
        let mut request = Self::new(target, mode, argv, env);
        request.report_ptr = (report as *mut ProcessControlReport) as usize;
        request.report_len = size_of::<ProcessControlReport>();
        request
    }

    /// Builds a structured request with a sleep report.
    #[must_use]
    pub fn with_sleep_report(
        argv: &[ProcessArg],
        env: &[ProcessEnv],
        report: &mut ProcessSleepReport,
    ) -> Self {
        let mut request = Self::new(ProcessSpawnTarget::BIN, ProcessSpawnMode::SLEEPING, argv, env);
        request.report_ptr = (report as *mut ProcessSleepReport) as usize;
        request.report_len = size_of::<ProcessSleepReport>();
        request
    }

    /// Returns the process argv record pointer.
    #[must_use]
    pub const fn argv_ptr(self) -> *const ProcessArg {
        self.argv_ptr
    }

    /// Returns the process argv count.
    #[must_use]
    pub const fn argc(self) -> usize {
        self.argc
    }

    /// Returns the process env record pointer.
    #[must_use]
    pub const fn env_ptr(self) -> *const ProcessEnv {
        self.env_ptr
    }

    /// Returns the process env count.
    #[must_use]
    pub const fn envc(self) -> usize {
        self.envc
    }

    /// Returns the raw spawn target.
    #[must_use]
    pub const fn target(self) -> ProcessSpawnTarget {
        ProcessSpawnTarget(self.target)
    }

    /// Returns the raw spawn mode.
    #[must_use]
    pub const fn mode(self) -> ProcessSpawnMode {
        ProcessSpawnMode(self.mode)
    }

    /// Returns extension flags.
    #[must_use]
    pub const fn flags(self) -> ProcessSpawnFlags {
        ProcessSpawnFlags::from_raw(self.flags)
    }

    /// Replaces extension flags.
    ///
    /// This is intended for tests and future explicit extensions. Current
    /// production wrappers pass empty flags.
    pub fn set_flags(&mut self, flags: ProcessSpawnFlags) {
        self.flags = flags.raw();
    }

    /// Returns the optional report pointer.
    #[must_use]
    pub const fn report_ptr(self) -> usize {
        self.report_ptr
    }

    /// Returns the optional report byte length.
    #[must_use]
    pub const fn report_len(self) -> usize {
        self.report_len
    }
}

/// Raw process-spawn start mode owned by the process domain.
///
/// The raw syscall leaf carries this value as a scalar argument, but the public
/// semantic choice belongs here. A ready child enters the scheduler queue
/// immediately; a blocked child keeps its admitted executable image until a
/// process-domain wake makes it ready.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct ProcessSpawnMode(usize);

impl ProcessSpawnMode {
    /// Spawn the child as scheduler-ready work.
    pub const READY: Self = Self(0);
    /// Spawn the child and leave it blocked until an explicit wake.
    pub const BLOCKED: Self = Self(1);
    /// Spawn the child and leave it blocked until a scheduler tick deadline.
    pub const SLEEPING: Self = Self(2);

    /// Returns the raw transport scalar.
    #[must_use]
    pub const fn raw(self) -> usize {
        self.0
    }
}

/// Bounded Reovim process exit code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct ExitCode(u8);

impl ExitCode {
    /// Successful process completion.
    pub const SUCCESS: Self = Self(0);
    /// Generic failed process completion.
    pub const FAILURE: Self = Self(1);

    /// Builds a bounded Reovim exit code.
    #[must_use]
    pub const fn new(raw: u8) -> Self {
        Self(raw)
    }

    /// Returns the bounded scalar code.
    #[must_use]
    pub const fn raw(self) -> u8 {
        self.0
    }
}

/// Stable retained-process state code used by structured process reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct ProcessStateCode(usize);

impl ProcessStateCode {
    /// Empty or absent process slot.
    pub const EMPTY: Self = Self(0);
    /// Newly admitted process state.
    pub const NEW: Self = Self(1);
    /// Scheduler-ready process state.
    pub const READY: Self = Self(2);
    /// Currently running process state.
    pub const RUNNING: Self = Self(3);
    /// Blocked process state.
    pub const BLOCKED: Self = Self(4);
    /// Successful exited process state.
    pub const EXITED: Self = Self(5);
    /// Failed process state.
    pub const FAILED: Self = Self(6);
    /// Halted process state.
    pub const HALTED: Self = Self(7);
    /// Reaped child process state retained for diagnostics.
    pub const REAPED: Self = Self(8);

    /// Builds a process state code from a stable scalar.
    #[must_use]
    pub const fn new(raw: usize) -> Self {
        Self(raw)
    }

    /// Returns the stable scalar.
    #[must_use]
    pub const fn raw(self) -> usize {
        self.0
    }
}

/// Structured result of spawning a child blocked on a scheduler tick deadline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct ProcessSleepReport {
    requested_ticks: usize,
    pid: usize,
    state: usize,
    wake_tick: usize,
    path_len: usize,
    path_truncated: u8,
    path: [u8; PROCESS_REPORT_PATH_BYTES],
}

impl ProcessSleepReport {
    /// Empty report value used before a sleeping spawn request is issued.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            requested_ticks: 0,
            pid: 0,
            state: ProcessStateCode::EMPTY.raw(),
            wake_tick: 0,
            path_len: 0,
            path_truncated: 0,
            path: [0; PROCESS_REPORT_PATH_BYTES],
        }
    }

    /// Clears this report to the empty value.
    pub fn clear(&mut self) {
        *self = Self::empty();
    }

    /// Records the requested scheduler tick duration.
    pub fn set_requested_ticks(&mut self, ticks: usize) {
        self.requested_ticks = ticks;
    }

    /// Records the spawned child process id.
    pub fn set_pid(&mut self, pid: ProcessId) {
        self.pid = pid.raw();
    }

    /// Records the retained process state after sleeping spawn.
    pub fn set_state(&mut self, state: ProcessStateCode) {
        self.state = state.raw();
    }

    /// Records the scheduler tick deadline at or after which the child wakes.
    pub fn set_wake_tick(&mut self, tick: usize) {
        self.wake_tick = tick;
    }

    /// Records the executable path, truncating to the report boundary.
    pub fn set_path(&mut self, path: &str) {
        let bytes = path.as_bytes();
        let len = bytes.len().min(self.path.len());
        self.path[..len].copy_from_slice(&bytes[..len]);
        self.path_len = len;
        self.path_truncated = u8::from(len < bytes.len());
    }

    /// Returns the requested scheduler tick duration.
    #[must_use]
    pub const fn requested_ticks(self) -> usize {
        self.requested_ticks
    }

    /// Returns the spawned child process id.
    #[must_use]
    pub const fn pid(self) -> ProcessId {
        ProcessId::new(self.pid)
    }

    /// Returns the retained process state after sleeping spawn.
    #[must_use]
    pub const fn state(self) -> ProcessStateCode {
        ProcessStateCode::new(self.state)
    }

    /// Returns the scheduler tick deadline at or after which the child wakes.
    #[must_use]
    pub const fn wake_tick(self) -> usize {
        self.wake_tick
    }

    /// Returns the executable path bytes.
    #[must_use]
    pub fn path_bytes(&self) -> &[u8] {
        &self.path[..self.path_len]
    }

    /// Returns whether the executable path was truncated.
    #[must_use]
    pub const fn path_truncated(self) -> bool {
        self.path_truncated != 0
    }
}

/// Structured result of a retained process wait.
///
/// This is Reovim retained-process state, not a POSIX status word. The report
/// exists so linked `/bin` programs can format operator output without
/// scraping `/proc` or depending on source-image helper operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct ProcessWaitReport {
    child_pid: usize,
    child_state: usize,
    exit_code: i32,
    completed: u8,
}

impl ProcessWaitReport {
    /// Empty report value used before a wait request is issued.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            child_pid: 0,
            child_state: ProcessStateCode::EMPTY.raw(),
            exit_code: 0,
            completed: 0,
        }
    }

    /// Clears this report to the empty value.
    pub fn clear(&mut self) {
        *self = Self::empty();
    }

    /// Records the child process id observed by the wait.
    pub fn set_child_pid(&mut self, pid: ProcessId) {
        self.child_pid = pid.raw();
    }

    /// Records the child state observed when the wait returned.
    pub fn set_child_state(&mut self, state: ProcessStateCode) {
        self.child_state = state.raw();
    }

    /// Records the child exit code observed when the wait returned.
    pub fn set_exit_code(&mut self, code: i32) {
        self.exit_code = code;
    }

    /// Records whether the child completed.
    pub fn set_completed(&mut self, completed: bool) {
        self.completed = u8::from(completed);
    }

    /// Returns the child process id observed by the wait.
    #[must_use]
    pub const fn child_pid(self) -> ProcessId {
        ProcessId::new(self.child_pid)
    }

    /// Returns the child state observed when the wait returned.
    #[must_use]
    pub const fn child_state(self) -> ProcessStateCode {
        ProcessStateCode::new(self.child_state)
    }

    /// Returns the child exit code observed when the wait returned.
    #[must_use]
    pub const fn exit_code(self) -> i32 {
        self.exit_code
    }

    /// Returns whether the child completed.
    #[must_use]
    pub const fn completed(self) -> bool {
        self.completed != 0
    }
}

/// Structured result of a retained process wait with a scheduler tick deadline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct ProcessTimedWaitReport {
    requested_ticks: usize,
    child_pid: usize,
    child_state: usize,
    exit_code: i32,
    completed: u8,
    timed_out: u8,
    tick_count: usize,
}

impl ProcessTimedWaitReport {
    /// Empty report value used before a timed wait request is issued.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            requested_ticks: 0,
            child_pid: 0,
            child_state: ProcessStateCode::EMPTY.raw(),
            exit_code: 0,
            completed: 0,
            timed_out: 0,
            tick_count: 0,
        }
    }

    /// Clears this report to the empty value.
    pub fn clear(&mut self) {
        *self = Self::empty();
    }

    /// Records the requested scheduler tick duration.
    pub fn set_requested_ticks(&mut self, ticks: usize) {
        self.requested_ticks = ticks;
    }

    /// Records the child process id observed by the wait.
    pub fn set_child_pid(&mut self, pid: ProcessId) {
        self.child_pid = pid.raw();
    }

    /// Records the child state observed when the wait returned.
    pub fn set_child_state(&mut self, state: ProcessStateCode) {
        self.child_state = state.raw();
    }

    /// Records the child exit code observed when the wait returned.
    pub fn set_exit_code(&mut self, code: i32) {
        self.exit_code = code;
    }

    /// Records whether the child completed before the deadline.
    pub fn set_completed(&mut self, completed: bool) {
        self.completed = u8::from(completed);
    }

    /// Records whether the wait returned because the deadline expired.
    pub fn set_timed_out(&mut self, timed_out: bool) {
        self.timed_out = u8::from(timed_out);
    }

    /// Records the global scheduler tick count when the wait returned.
    pub fn set_tick_count(&mut self, tick_count: usize) {
        self.tick_count = tick_count;
    }

    /// Returns the requested scheduler tick duration.
    #[must_use]
    pub const fn requested_ticks(self) -> usize {
        self.requested_ticks
    }

    /// Returns the child process id observed by the wait.
    #[must_use]
    pub const fn child_pid(self) -> ProcessId {
        ProcessId::new(self.child_pid)
    }

    /// Returns the child state observed when the wait returned.
    #[must_use]
    pub const fn child_state(self) -> ProcessStateCode {
        ProcessStateCode::new(self.child_state)
    }

    /// Returns the child exit code observed when the wait returned.
    #[must_use]
    pub const fn exit_code(self) -> i32 {
        self.exit_code
    }

    /// Returns whether the child completed before the deadline.
    #[must_use]
    pub const fn completed(self) -> bool {
        self.completed != 0
    }

    /// Returns whether the wait returned because the deadline expired.
    #[must_use]
    pub const fn timed_out(self) -> bool {
        self.timed_out != 0
    }

    /// Returns the global scheduler tick count when the wait returned.
    #[must_use]
    pub const fn tick_count(self) -> usize {
        self.tick_count
    }
}

/// Structured result of a process-control operation that returns retained state.
///
/// This report is intentionally narrow: it carries the process id, stable state
/// code, bounded executable path, and bounded executable image identity needed
/// by real `/bin` programs that format operator output. Body byte count and
/// checksum identify the admitted executable body without exposing provider
/// internals. It does not expose scheduler internals or a POSIX process status
/// word.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct ProcessControlReport {
    pid: usize,
    state: usize,
    path_len: usize,
    path_truncated: u8,
    path: [u8; PROCESS_REPORT_PATH_BYTES],
    loader_len: usize,
    loader_truncated: u8,
    loader: [u8; PROCESS_REPORT_LOADER_BYTES],
    entry_name_len: usize,
    entry_name_truncated: u8,
    entry_name: [u8; PROCESS_REPORT_ENTRY_NAME_BYTES],
    body_format_len: usize,
    body_format_truncated: u8,
    body_format: [u8; PROCESS_REPORT_BODY_FORMAT_BYTES],
    body_inner_len: usize,
    body_inner_truncated: u8,
    body_inner: [u8; PROCESS_REPORT_BODY_INNER_BYTES],
    body_bytes: usize,
    body_checksum: u32,
}

impl ProcessControlReport {
    /// Empty report value used before a process-control request is issued.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            pid: 0,
            state: ProcessStateCode::EMPTY.raw(),
            path_len: 0,
            path_truncated: 0,
            path: [0; PROCESS_REPORT_PATH_BYTES],
            loader_len: 0,
            loader_truncated: 0,
            loader: [0; PROCESS_REPORT_LOADER_BYTES],
            entry_name_len: 0,
            entry_name_truncated: 0,
            entry_name: [0; PROCESS_REPORT_ENTRY_NAME_BYTES],
            body_format_len: 0,
            body_format_truncated: 0,
            body_format: [0; PROCESS_REPORT_BODY_FORMAT_BYTES],
            body_inner_len: 0,
            body_inner_truncated: 0,
            body_inner: [0; PROCESS_REPORT_BODY_INNER_BYTES],
            body_bytes: 0,
            body_checksum: 0,
        }
    }

    /// Clears this report to the empty value.
    pub fn clear(&mut self) {
        *self = Self::empty();
    }

    /// Records the process id.
    pub fn set_pid(&mut self, pid: ProcessId) {
        self.pid = pid.raw();
    }

    /// Records the retained process state.
    pub fn set_state(&mut self, state: ProcessStateCode) {
        self.state = state.raw();
    }

    /// Records the executable path, truncating to the report boundary.
    pub fn set_path(&mut self, path: &str) {
        let bytes = path.as_bytes();
        let len = bytes.len().min(self.path.len());
        self.path[..len].copy_from_slice(&bytes[..len]);
        self.path_len = len;
        self.path_truncated = u8::from(len < bytes.len());
    }

    /// Records the executable loader/source kind, truncating to the report boundary.
    pub fn set_loader(&mut self, loader: &str) {
        let bytes = loader.as_bytes();
        let len = bytes.len().min(self.loader.len());
        self.loader[..len].copy_from_slice(&bytes[..len]);
        self.loader_len = len;
        self.loader_truncated = u8::from(len < bytes.len());
    }

    /// Records the stable executable entry name, truncating to the report boundary.
    pub fn set_entry_name(&mut self, entry_name: &str) {
        let bytes = entry_name.as_bytes();
        let len = bytes.len().min(self.entry_name.len());
        self.entry_name[..len].copy_from_slice(&bytes[..len]);
        self.entry_name_len = len;
        self.entry_name_truncated = u8::from(len < bytes.len());
    }

    /// Records the executable body format, truncating to the report boundary.
    pub fn set_body_format(&mut self, body_format: &str) {
        let bytes = body_format.as_bytes();
        let len = bytes.len().min(self.body_format.len());
        self.body_format[..len].copy_from_slice(&bytes[..len]);
        self.body_format_len = len;
        self.body_format_truncated = u8::from(len < bytes.len());
    }

    /// Records the checked executable body contract, truncating to the report boundary.
    pub fn set_body_inner(&mut self, body_inner: &str) {
        let bytes = body_inner.as_bytes();
        let len = bytes.len().min(self.body_inner.len());
        self.body_inner[..len].copy_from_slice(&bytes[..len]);
        self.body_inner_len = len;
        self.body_inner_truncated = u8::from(len < bytes.len());
    }

    /// Records the retained executable body byte count.
    pub const fn set_body_bytes(&mut self, body_bytes: usize) {
        self.body_bytes = body_bytes;
    }

    /// Records the retained executable body checksum.
    pub const fn set_body_checksum(&mut self, body_checksum: u32) {
        self.body_checksum = body_checksum;
    }

    /// Returns the process id.
    #[must_use]
    pub const fn pid(self) -> ProcessId {
        ProcessId::new(self.pid)
    }

    /// Returns the retained process state.
    #[must_use]
    pub const fn state(self) -> ProcessStateCode {
        ProcessStateCode::new(self.state)
    }

    /// Returns the executable path bytes.
    #[must_use]
    pub fn path_bytes(&self) -> &[u8] {
        &self.path[..self.path_len]
    }

    /// Returns whether the executable path was truncated.
    #[must_use]
    pub const fn path_truncated(self) -> bool {
        self.path_truncated != 0
    }

    /// Returns the executable loader/source kind bytes.
    #[must_use]
    pub fn loader_bytes(&self) -> &[u8] {
        &self.loader[..self.loader_len]
    }

    /// Returns whether the executable loader/source kind was truncated.
    #[must_use]
    pub const fn loader_truncated(self) -> bool {
        self.loader_truncated != 0
    }

    /// Returns the executable entry-name bytes.
    #[must_use]
    pub fn entry_name_bytes(&self) -> &[u8] {
        &self.entry_name[..self.entry_name_len]
    }

    /// Returns whether the executable entry name was truncated.
    #[must_use]
    pub const fn entry_name_truncated(self) -> bool {
        self.entry_name_truncated != 0
    }

    /// Returns the executable body-format bytes.
    #[must_use]
    pub fn body_format_bytes(&self) -> &[u8] {
        &self.body_format[..self.body_format_len]
    }

    /// Returns whether the executable body format was truncated.
    #[must_use]
    pub const fn body_format_truncated(self) -> bool {
        self.body_format_truncated != 0
    }

    /// Returns the checked executable body-contract bytes.
    #[must_use]
    pub fn body_inner_bytes(&self) -> &[u8] {
        &self.body_inner[..self.body_inner_len]
    }

    /// Returns whether the checked executable body contract was truncated.
    #[must_use]
    pub const fn body_inner_truncated(self) -> bool {
        self.body_inner_truncated != 0
    }

    /// Returns the retained executable body byte count.
    #[must_use]
    pub const fn body_bytes(self) -> usize {
        self.body_bytes
    }

    /// Returns the retained executable body checksum.
    #[must_use]
    pub const fn body_checksum(self) -> u32 {
        self.body_checksum
    }
}

/// Product-facing process syscall error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct ProcessError(i32);

impl ProcessError {
    /// The requested arguments are not valid for this operation.
    pub const INVALID_ARGUMENT: Self = Self(SyscallError::INVALID_ARGUMENT.code());
    /// The current profile or backend does not support the requested operation.
    pub const UNSUPPORTED: Self = Self(SyscallError::UNSUPPORTED.code());
    /// The current process blocked with retained replayable syscall state.
    pub const BUSY: Self = Self(SyscallError::BUSY.code());
    /// The requested process target was not found.
    pub const NOT_FOUND: Self = Self(SyscallError::NOT_FOUND.code());
    /// The requested retained process is not waitable by this operation.
    pub const NOT_WAITABLE: Self = Self(SyscallError::NOT_WAITABLE.code());
    /// The requested retained process is protected from this operation.
    pub const PROTECTED_PROCESS: Self = Self(SyscallError::PROTECTED_PROCESS.code());
    /// The requested executable image failed validation.
    pub const INVALID_IMAGE: Self = Self(SyscallError::INVALID_IMAGE.code());

    /// Builds a process error from a bridge/provider code.
    #[must_use]
    pub const fn new(code: i32) -> Self {
        Self(code)
    }

    /// Returns the diagnostic bridge/provider code.
    #[must_use]
    pub const fn code(self) -> i32 {
        self.0
    }
}

/// Process control backed by the raw Reovim syscall transport.
///
/// This adapter keeps process semantics in `uapi::process`: callers request
/// process identity, current-image replacement, child process spawn, retained
/// child wait, retained timed wait, retained process wake/kill, or process
/// exit, and only this wrapper packs raw syscall numbers and scalar arguments.
#[derive(Debug, Clone, Copy)]
pub struct SyscallProcessControl {
    raw: RawSyscall,
}

impl SyscallProcessControl {
    /// Creates a process control adapter over the raw syscall transport.
    #[must_use]
    pub const fn new(raw: RawSyscall) -> Self {
        Self { raw }
    }

    /// Returns the current process id.
    ///
    /// # Errors
    ///
    /// Returns [`ProcessError`] when no current process context exists or the raw
    /// syscall transport reports failure.
    pub fn get_pid(self) -> Result<ProcessId, ProcessError> {
        self.raw
            .invoke(SyscallNr::GET_PID, SyscallArgs::EMPTY)
            .decode()
            .map(ProcessId::new)
            .map_err(process_error_from_syscall)
    }

    /// Returns retained metadata for the current process.
    ///
    /// This is the typed process-domain form of the current-process report. It
    /// uses raw syscall transport only as a carrier and avoids making
    /// `/proc/self` scraping a control API.
    ///
    /// # Errors
    ///
    /// Returns [`ProcessError`] when no current process context exists, the
    /// retained process row is missing, or the raw transport rejects the report
    /// buffer.
    pub fn self_report(self) -> Result<ProcessControlReport, ProcessError> {
        let mut report = ProcessControlReport::empty();
        let pid = self
            .raw
            .invoke(
                SyscallNr::PROCESS_SELF,
                SyscallArgs::new([
                    (&mut report as *mut ProcessControlReport) as usize,
                    size_of::<ProcessControlReport>(),
                    0,
                    0,
                    0,
                    0,
                ]),
            )
            .decode()
            .map_err(process_error_from_syscall)?;
        if report.pid().raw() == pid {
            Ok(report)
        } else {
            Err(ProcessError::INVALID_ARGUMENT)
        }
    }

    /// Spawns a child process from a bounded argv slice.
    ///
    /// `argv[0]` is the executable name or absolute `/bin/...` path. The kernel
    /// copies all argument bytes into retained process/exec state before this
    /// call returns.
    ///
    /// # Errors
    ///
    /// Returns [`ProcessError`] when there is no current process, `argv` is
    /// empty, executable lookup fails, image validation fails, or the bounded
    /// process/exec tables reject the child.
    pub fn spawn(self, argv: &[ProcessArg]) -> Result<ProcessId, ProcessError> {
        self.spawn_with_target_and_mode(ProcessSpawnTarget::BIN, ProcessSpawnMode::READY, argv)
    }

    /// Spawns a child process from bounded argv and env slices.
    ///
    /// This is the env-carrying form of [`Self::spawn`]. It remains a Reovim
    /// process-domain ABI; it does not expose a public POSIX environment face.
    ///
    /// # Errors
    ///
    /// Returns [`ProcessError`] when there is no current process, `argv` is
    /// empty, executable lookup fails, env validation fails, image validation
    /// fails, or the bounded process/exec tables reject the child.
    pub fn spawn_with_env(
        self,
        argv: &[ProcessArg],
        env: &[ProcessEnv],
    ) -> Result<ProcessId, ProcessError> {
        self.spawn_with_target_mode_and_env(
            ProcessSpawnTarget::BIN,
            ProcessSpawnMode::READY,
            argv,
            env,
        )
    }

    /// Spawns a child process and returns retained process metadata.
    ///
    /// This is the report-returning form of [`Self::spawn`], used by real
    /// `/bin` programs that need to format operator-facing pid/path/state
    /// output without calling source-image-only helper operations.
    ///
    /// # Errors
    ///
    /// Returns [`ProcessError`] when there is no current process, `argv` is
    /// empty, executable lookup fails, image validation fails, or the bounded
    /// process/exec tables reject the child.
    pub fn spawn_report(self, argv: &[ProcessArg]) -> Result<ProcessControlReport, ProcessError> {
        self.spawn_with_target_and_mode_report(
            ProcessSpawnTarget::BIN,
            ProcessSpawnMode::READY,
            argv,
        )
    }

    /// Spawns a child process with env and returns retained process metadata.
    ///
    /// This uses the structured process-spawn request record so env and the
    /// report pointer do not compete for raw syscall argument slots.
    ///
    /// # Errors
    ///
    /// Returns [`ProcessError`] when there is no current process, `argv` is
    /// empty, executable lookup fails, env validation fails, image validation
    /// fails, or the bounded process/exec tables reject the child.
    pub fn spawn_report_with_env(
        self,
        argv: &[ProcessArg],
        env: &[ProcessEnv],
    ) -> Result<ProcessControlReport, ProcessError> {
        let mut report = ProcessControlReport::empty();
        let request = ProcessSpawnRequest::with_process_report(
            ProcessSpawnTarget::BIN,
            ProcessSpawnMode::READY,
            argv,
            env,
            &mut report,
        );
        let pid = self.spawn_with_request(&request)?;
        if report.pid().raw() == pid.raw() {
            Ok(report)
        } else {
            Err(ProcessError::INVALID_ARGUMENT)
        }
    }

    /// Spawns a blocked child process from a bounded argv slice.
    ///
    /// `argv[0]` is the executable name or absolute `/bin/...` path. The child is
    /// admitted into exec/process/scheduler state but does not run until a later
    /// [`Self::wake`] succeeds for the returned process id.
    ///
    /// # Errors
    ///
    /// Returns [`ProcessError`] when there is no current process, `argv` is
    /// empty, executable lookup fails, image validation fails, or the bounded
    /// process/exec tables reject the child.
    pub fn spawn_blocked(self, argv: &[ProcessArg]) -> Result<ProcessId, ProcessError> {
        self.spawn_with_target_and_mode(ProcessSpawnTarget::BIN, ProcessSpawnMode::BLOCKED, argv)
    }

    /// Spawns a blocked child process from bounded argv and env slices.
    ///
    /// This is the env-carrying form of [`Self::spawn_blocked`]. Report-returning
    /// blocked spawn remains argv-only until the process-domain request record
    /// is deliberately widened.
    ///
    /// # Errors
    ///
    /// Returns [`ProcessError`] when there is no current process, `argv` is
    /// empty, executable lookup fails, env validation fails, image validation
    /// fails, or the bounded process/exec tables reject the child.
    pub fn spawn_blocked_with_env(
        self,
        argv: &[ProcessArg],
        env: &[ProcessEnv],
    ) -> Result<ProcessId, ProcessError> {
        self.spawn_with_target_mode_and_env(
            ProcessSpawnTarget::BIN,
            ProcessSpawnMode::BLOCKED,
            argv,
            env,
        )
    }

    /// Spawns a blocked child process and returns retained process metadata.
    ///
    /// This is the report-returning form of [`Self::spawn_blocked`].
    ///
    /// # Errors
    ///
    /// Returns [`ProcessError`] when there is no current process, `argv` is
    /// empty, executable lookup fails, image validation fails, or the bounded
    /// process/exec tables reject the child.
    pub fn spawn_blocked_report(
        self,
        argv: &[ProcessArg],
    ) -> Result<ProcessControlReport, ProcessError> {
        self.spawn_with_target_and_mode_report(
            ProcessSpawnTarget::BIN,
            ProcessSpawnMode::BLOCKED,
            argv,
        )
    }

    /// Spawns a blocked child process with env and returns retained metadata.
    ///
    /// This is the env-carrying form of [`Self::spawn_blocked_report`].
    ///
    /// # Errors
    ///
    /// Returns [`ProcessError`] when there is no current process, `argv` is
    /// empty, executable lookup fails, env validation fails, image validation
    /// fails, or the bounded process/exec tables reject the child.
    pub fn spawn_blocked_report_with_env(
        self,
        argv: &[ProcessArg],
        env: &[ProcessEnv],
    ) -> Result<ProcessControlReport, ProcessError> {
        let mut report = ProcessControlReport::empty();
        let request = ProcessSpawnRequest::with_process_report(
            ProcessSpawnTarget::BIN,
            ProcessSpawnMode::BLOCKED,
            argv,
            env,
            &mut report,
        );
        let pid = self.spawn_with_request(&request)?;
        if report.pid().raw() == pid.raw() {
            Ok(report)
        } else {
            Err(ProcessError::INVALID_ARGUMENT)
        }
    }

    /// Spawns a child process blocked until a scheduler tick deadline.
    ///
    /// `argv[0]` is the executable name or absolute `/bin/...` path. The
    /// returned report includes the child pid and absolute wake tick. This is a
    /// Reovim process/scheduler control, not POSIX `sleep`.
    ///
    /// # Errors
    ///
    /// Returns [`ProcessError`] when `ticks` is zero, there is no current
    /// process, executable lookup fails, image validation fails, or the bounded
    /// process/exec/scheduler tables reject the child.
    pub fn spawn_sleeping(
        self,
        argv: &[ProcessArg],
        ticks: usize,
    ) -> Result<ProcessSleepReport, ProcessError> {
        if ticks == 0 {
            return Err(ProcessError::INVALID_ARGUMENT);
        }
        let mut report = ProcessSleepReport::empty();
        report.set_requested_ticks(ticks);
        let pid = self
            .raw
            .invoke(
                SyscallNr::SPAWN,
                SyscallArgs::new([
                    argv.as_ptr() as usize,
                    argv.len(),
                    ProcessSpawnTarget::BIN.raw(),
                    ProcessSpawnMode::SLEEPING.raw(),
                    (&mut report as *mut ProcessSleepReport) as usize,
                    size_of::<ProcessSleepReport>(),
                ]),
            )
            .decode()
            .map_err(process_error_from_syscall)?;
        if report.pid().raw() == pid {
            Ok(report)
        } else {
            Err(ProcessError::INVALID_ARGUMENT)
        }
    }

    /// Spawns a child with env and blocks it until a scheduler tick deadline.
    ///
    /// This is the env-carrying form of [`Self::spawn_sleeping`] and uses the
    /// structured process-spawn request record.
    ///
    /// # Errors
    ///
    /// Returns [`ProcessError`] when `ticks` is zero, there is no current
    /// process, executable lookup fails, env validation fails, image validation
    /// fails, or the bounded process/exec/scheduler tables reject the child.
    pub fn spawn_sleeping_with_env(
        self,
        argv: &[ProcessArg],
        env: &[ProcessEnv],
        ticks: usize,
    ) -> Result<ProcessSleepReport, ProcessError> {
        if ticks == 0 {
            return Err(ProcessError::INVALID_ARGUMENT);
        }
        let mut report = ProcessSleepReport::empty();
        report.set_requested_ticks(ticks);
        let request = ProcessSpawnRequest::with_sleep_report(argv, env, &mut report);
        let pid = self.spawn_with_request(&request)?;
        if report.pid().raw() == pid.raw() {
            Ok(report)
        } else {
            Err(ProcessError::INVALID_ARGUMENT)
        }
    }

    /// Spawns a configured payload child process from a bounded argv slice.
    ///
    /// `argv[0]` is the payload name, not a `/bin` path. The kernel copies all
    /// argument bytes into retained process/exec state before this call returns.
    ///
    /// # Errors
    ///
    /// Returns [`ProcessError`] when there is no current process, launch is not
    /// enabled for the current profile, `argv` is empty, payload lookup fails,
    /// image validation fails, or the bounded process/exec tables reject the
    /// child.
    pub fn spawn_payload(self, argv: &[ProcessArg]) -> Result<ProcessId, ProcessError> {
        self.spawn_with_target_and_mode(ProcessSpawnTarget::PAYLOAD, ProcessSpawnMode::READY, argv)
    }

    /// Spawns a configured payload child process from bounded argv and env slices.
    ///
    /// This is the env-carrying form of [`Self::spawn_payload`]. It uses the
    /// structured spawn request because payload env does not have a scalar
    /// report-free compatibility form in the raw transport.
    ///
    /// # Errors
    ///
    /// Returns [`ProcessError`] when there is no current process, launch is not
    /// enabled for the current profile, `argv` is empty, payload lookup fails,
    /// env validation fails, image validation fails, or the bounded
    /// process/exec tables reject the child.
    pub fn spawn_payload_with_env(
        self,
        argv: &[ProcessArg],
        env: &[ProcessEnv],
    ) -> Result<ProcessId, ProcessError> {
        if env.is_empty() {
            return self.spawn_payload(argv);
        }
        let request = ProcessSpawnRequest::new(
            ProcessSpawnTarget::PAYLOAD,
            ProcessSpawnMode::READY,
            argv,
            env,
        );
        self.spawn_with_request(&request)
    }

    fn spawn_with_target_and_mode(
        self,
        target: ProcessSpawnTarget,
        mode: ProcessSpawnMode,
        argv: &[ProcessArg],
    ) -> Result<ProcessId, ProcessError> {
        self.raw
            .invoke(
                SyscallNr::SPAWN,
                SyscallArgs::new([
                    argv.as_ptr() as usize,
                    argv.len(),
                    target.raw(),
                    mode.raw(),
                    0,
                    0,
                ]),
            )
            .decode()
            .map(ProcessId::new)
            .map_err(process_error_from_syscall)
    }

    fn spawn_with_target_mode_and_env(
        self,
        target: ProcessSpawnTarget,
        mode: ProcessSpawnMode,
        argv: &[ProcessArg],
        env: &[ProcessEnv],
    ) -> Result<ProcessId, ProcessError> {
        if env.is_empty() {
            return self.spawn_with_target_and_mode(target, mode, argv);
        }
        self.raw
            .invoke(
                SyscallNr::SPAWN,
                SyscallArgs::new([
                    argv.as_ptr() as usize,
                    argv.len(),
                    target.with_env().raw(),
                    mode.raw(),
                    env.as_ptr() as usize,
                    env.len(),
                ]),
            )
            .decode()
            .map(ProcessId::new)
            .map_err(process_error_from_syscall)
    }

    fn spawn_with_target_and_mode_report(
        self,
        target: ProcessSpawnTarget,
        mode: ProcessSpawnMode,
        argv: &[ProcessArg],
    ) -> Result<ProcessControlReport, ProcessError> {
        let mut report = ProcessControlReport::empty();
        let pid = self
            .raw
            .invoke(
                SyscallNr::SPAWN,
                SyscallArgs::new([
                    argv.as_ptr() as usize,
                    argv.len(),
                    target.raw(),
                    mode.raw(),
                    (&mut report as *mut ProcessControlReport) as usize,
                    size_of::<ProcessControlReport>(),
                ]),
            )
            .decode()
            .map_err(process_error_from_syscall)?;
        if report.pid().raw() == pid {
            Ok(report)
        } else {
            Err(ProcessError::INVALID_ARGUMENT)
        }
    }

    fn spawn_with_request(self, request: &ProcessSpawnRequest) -> Result<ProcessId, ProcessError> {
        self.raw
            .invoke(
                SyscallNr::SPAWN,
                SyscallArgs::new([
                    (request as *const ProcessSpawnRequest) as usize,
                    size_of::<ProcessSpawnRequest>(),
                    request.target().with_request().raw(),
                    0,
                    0,
                    0,
                ]),
            )
            .decode()
            .map(ProcessId::new)
            .map_err(process_error_from_syscall)
    }

    /// Replaces the current process image from a bounded argv slice.
    ///
    /// `argv[0]` is the executable name or absolute `/bin/...` path. This is
    /// Reovim process image replacement, not POSIX branding. The current direct
    /// backend returns success after replacement admission so linked proof
    /// programs can stop the old image frame; trap-backed entry should make
    /// success no-return.
    ///
    /// # Errors
    ///
    /// Returns [`ProcessError`] when there is no current process, `argv` is
    /// empty, executable lookup fails, image validation fails, replacement
    /// admission fails, or the backend reports an invalid success scalar.
    pub fn execve(self, argv: &[ProcessArg]) -> Result<ExitCode, ProcessError> {
        self.execve_with_env(argv, &[])
    }

    /// Replaces the current process image from bounded argv and env slices.
    ///
    /// This is the env-carrying form of [`Self::execve`]. The current direct
    /// backend still returns after replacement admission; future trap-backed
    /// entry should make successful replacement no-return.
    ///
    /// # Errors
    ///
    /// Returns [`ProcessError`] when there is no current process, `argv` is
    /// empty, executable lookup fails, env validation fails, image validation
    /// fails, replacement admission fails, or the backend reports an invalid
    /// success scalar.
    pub fn execve_with_env(
        self,
        argv: &[ProcessArg],
        env: &[ProcessEnv],
    ) -> Result<ExitCode, ProcessError> {
        self.raw
            .invoke(
                SyscallNr::EXECVE,
                SyscallArgs::new([
                    argv.as_ptr() as usize,
                    argv.len(),
                    if env.is_empty() {
                        0
                    } else {
                        env.as_ptr() as usize
                    },
                    env.len(),
                    0,
                    0,
                ]),
            )
            .decode()
            .and_then(|code| {
                if code <= u8::MAX as usize {
                    Ok(ExitCode::new(code as u8))
                } else {
                    Err(SyscallError::INVALID_ARGUMENT)
                }
            })
            .map_err(process_error_from_syscall)
    }

    /// Waits for a retained child process by process id and returns its exit code.
    ///
    /// This is Reovim process wait, not POSIX `waitpid`: the target is selected
    /// by retained process id and the cooperative scheduler may run ready work to
    /// let the target finish.
    ///
    /// # Errors
    ///
    /// Returns [`ProcessError`] when there is no current process, the target does
    /// not exist, the target is protected, or the target state is not waitable.
    pub fn wait(self, pid: ProcessId) -> Result<ExitCode, ProcessError> {
        self.raw
            .invoke(SyscallNr::WAIT, SyscallArgs::new([pid.raw(), 0, 0, 0, 0, 0]))
            .decode()
            .and_then(|code| {
                if code <= u8::MAX as usize {
                    Ok(ExitCode::new(code as u8))
                } else {
                    Err(SyscallError::INVALID_ARGUMENT)
                }
            })
            .map_err(process_error_from_syscall)
    }

    /// Waits for a retained child process and returns a structured wait report.
    ///
    /// This uses the same raw `WAIT` transport as [`Self::wait`], passing an
    /// optional typed report buffer owned by this process leaf.
    ///
    /// # Errors
    ///
    /// Returns [`ProcessError`] when there is no current process, the target does
    /// not exist, the target is protected, or the target state is not waitable.
    pub fn wait_report(self, pid: ProcessId) -> Result<ProcessWaitReport, ProcessError> {
        let mut report = ProcessWaitReport::empty();
        report.set_child_pid(pid);
        self.raw
            .invoke(
                SyscallNr::WAIT,
                SyscallArgs::new([
                    pid.raw(),
                    (&mut report as *mut ProcessWaitReport) as usize,
                    size_of::<ProcessWaitReport>(),
                    0,
                    0,
                    0,
                ]),
            )
            .decode()
            .map(|_| report)
            .map_err(process_error_from_syscall)
    }

    /// Waits for a retained child until it either completes or reaches readiness.
    ///
    /// This is a Reovim process readiness wait, not POSIX `waitpid`. It is used
    /// by launch-style user programs that need to distinguish a completed child
    /// from a resident child that published readiness and blocked as a service.
    ///
    /// # Errors
    ///
    /// Returns [`ProcessError`] when there is no current process, the target does
    /// not exist, the target is protected, or the target state is not readiness
    /// waitable.
    pub fn wait_ready_report(self, pid: ProcessId) -> Result<ProcessWaitReport, ProcessError> {
        let mut report = ProcessWaitReport::empty();
        report.set_child_pid(pid);
        self.raw
            .invoke(
                SyscallNr::PROCESS_WAIT_READY,
                SyscallArgs::new([
                    pid.raw(),
                    (&mut report as *mut ProcessWaitReport) as usize,
                    size_of::<ProcessWaitReport>(),
                    0,
                    0,
                    0,
                ]),
            )
            .decode()
            .map(|_| report)
            .map_err(process_error_from_syscall)
    }

    /// Waits for a retained process while advancing bounded scheduler ticks.
    ///
    /// The returned report says whether the child completed or whether the
    /// deadline expired, plus the observed child state and global tick count.
    ///
    /// # Errors
    ///
    /// Returns [`ProcessError`] when `ticks` is zero, there is no current
    /// process, the target does not exist, is protected, or the target state is
    /// not waitable by the timed wait operation.
    pub fn wait_for_ticks(
        self,
        pid: ProcessId,
        ticks: usize,
    ) -> Result<ProcessTimedWaitReport, ProcessError> {
        if ticks == 0 {
            return Err(ProcessError::INVALID_ARGUMENT);
        }
        let mut report = ProcessTimedWaitReport::empty();
        report.set_child_pid(pid);
        report.set_requested_ticks(ticks);
        let child_pid = self
            .raw
            .invoke(
                SyscallNr::PROCESS_WAIT_TICKS,
                SyscallArgs::new([
                    pid.raw(),
                    ticks,
                    (&mut report as *mut ProcessTimedWaitReport) as usize,
                    size_of::<ProcessTimedWaitReport>(),
                    0,
                    0,
                ]),
            )
            .decode()
            .map_err(process_error_from_syscall)?;
        if report.child_pid().raw() == child_pid {
            Ok(report)
        } else {
            Err(ProcessError::INVALID_ARGUMENT)
        }
    }

    /// Wakes an operator-blocked process by process id.
    ///
    /// This is Reovim process control, not a signal API. The target must be a
    /// retained process blocked by the process-control `block` operation, and
    /// the kernel returns the same process id after it is made scheduler-ready.
    /// Wait, pipe-read, sleep, and service wait channels are woken only by their
    /// owning kernel events.
    ///
    /// # Errors
    ///
    /// Returns [`ProcessError`] when the target does not exist, is not
    /// operator-blocked, or the raw transport reports failure.
    pub fn wake(self, pid: ProcessId) -> Result<ProcessId, ProcessError> {
        self.raw
            .invoke(SyscallNr::PROCESS_WAKE, SyscallArgs::new([pid.raw(), 0, 0, 0, 0, 0]))
            .decode()
            .map(ProcessId::new)
            .map_err(process_error_from_syscall)
    }

    /// Wakes an operator-blocked process and returns retained process metadata.
    ///
    /// This is the report-returning form of [`Self::wake`].
    ///
    /// # Errors
    ///
    /// Returns [`ProcessError`] when the target does not exist, is not
    /// operator-blocked, or the raw transport reports failure.
    pub fn wake_report(self, pid: ProcessId) -> Result<ProcessControlReport, ProcessError> {
        let mut report = ProcessControlReport::empty();
        let returned_pid = self
            .raw
            .invoke(
                SyscallNr::PROCESS_WAKE,
                SyscallArgs::new([
                    pid.raw(),
                    (&mut report as *mut ProcessControlReport) as usize,
                    size_of::<ProcessControlReport>(),
                    0,
                    0,
                    0,
                ]),
            )
            .decode()
            .map_err(process_error_from_syscall)?;
        if report.pid().raw() == returned_pid {
            Ok(report)
        } else {
            Err(ProcessError::INVALID_ARGUMENT)
        }
    }

    /// Terminates a retained ready or blocked process by process id.
    ///
    /// This is Reovim process control. Root/system processes and the current
    /// process are protected; ordinary targets complete as failed and keep their
    /// retained process record for diagnostics.
    ///
    /// # Errors
    ///
    /// Returns [`ProcessError`] when the target does not exist, is protected, is
    /// not killable, or the raw transport reports failure.
    pub fn kill(self, pid: ProcessId) -> Result<ProcessId, ProcessError> {
        self.raw
            .invoke(SyscallNr::PROCESS_KILL, SyscallArgs::new([pid.raw(), 0, 0, 0, 0, 0]))
            .decode()
            .map(ProcessId::new)
            .map_err(process_error_from_syscall)
    }

    /// Terminates a retained ready or blocked process and returns metadata.
    ///
    /// This is the report-returning form of [`Self::kill`].
    ///
    /// # Errors
    ///
    /// Returns [`ProcessError`] when the target does not exist, is protected, is
    /// not killable, or the raw transport reports failure.
    pub fn kill_report(self, pid: ProcessId) -> Result<ProcessControlReport, ProcessError> {
        let mut report = ProcessControlReport::empty();
        let returned_pid = self
            .raw
            .invoke(
                SyscallNr::PROCESS_KILL,
                SyscallArgs::new([
                    pid.raw(),
                    (&mut report as *mut ProcessControlReport) as usize,
                    size_of::<ProcessControlReport>(),
                    0,
                    0,
                    0,
                ]),
            )
            .decode()
            .map_err(process_error_from_syscall)?;
        if report.pid().raw() == returned_pid {
            Ok(report)
        } else {
            Err(ProcessError::INVALID_ARGUMENT)
        }
    }

    /// Requests current process exit.
    ///
    /// The temporary direct backend returns after recording the request so the
    /// system-kernel dispatcher can finalize process state through its existing
    /// single owner. Future trap-backed entry may make this operation no-return.
    ///
    /// # Errors
    ///
    /// Returns [`ProcessError`] when no current process context exists or the raw
    /// syscall transport rejects the exit request.
    pub fn exit(self, code: ExitCode) -> Result<(), ProcessError> {
        self.raw
            .invoke(SyscallNr::EXIT, SyscallArgs::new([code.raw() as usize, 0, 0, 0, 0, 0]))
            .decode()
            .map(|_| ())
            .map_err(process_error_from_syscall)
    }
}

fn process_error_from_syscall(error: SyscallError) -> ProcessError {
    ProcessError::new(error.code())
}
