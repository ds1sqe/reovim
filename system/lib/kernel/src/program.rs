//! Generic `/bin` executable descriptor and dispatch ABI.
//!
//! The system kernel owns exec admission, process/scheduler bookkeeping, and
//! the syscall handle exposed to programs. Concrete operator behavior is not
//! owned by the shell or this crate: the OS image supplies a `/bin`
//! descriptor slice during boot.

use {
    crate::{
        exec_body::{self, ExecBodyInnerFormat},
        proc,
        root_shell::RootShellSession,
        rootd, sched,
        source_store::ExecutableSourceStore,
        syscall::{ProgramSyscalls, SyscallContext},
    },
    core::{
        cell::UnsafeCell,
        ptr::NonNull,
        slice,
        sync::atomic::{AtomicBool, Ordering},
    },
    reovim_uapi_fs::{FsError, OpenAtDir, OpenFlags, RawFd, SyscallFdControl},
    reovim_uapi_process::{ExitCode, ProcessArg, ProcessEnv, ProcessError, SyscallProcessControl},
    reovim_uapi_sched::{SchedulerError, SchedulerTicks, SyscallSchedulerControl},
    reovim_uapi_syscall::{RawSyscall, SyscallArgs, SyscallError, SyscallNr, SyscallRet},
};

/// Maximum bytes attached to one program's initial stdin buffer.
pub const MAX_PROGRAM_STDIN_BYTES: usize = 128;
/// Maximum bytes captured from one program's stdout for a bounded pipe.
pub const MAX_PROGRAM_PIPE_BYTES: usize = MAX_PROGRAM_STDIN_BYTES;
/// Maximum argv entries passed to one image program.
pub const MAX_PROGRAM_ARGS: usize = 8;
/// Maximum bytes in one argv token.
pub const MAX_PROGRAM_ARG_BYTES: usize = 64;
/// Maximum environment entries passed to one image program.
pub const MAX_PROGRAM_ENVS: usize = 8;
/// Maximum bytes in one environment variable name.
pub const MAX_PROGRAM_ENV_NAME_BYTES: usize = 32;
/// Maximum bytes in one environment variable value.
pub const MAX_PROGRAM_ENV_VALUE_BYTES: usize = 64;
/// Maximum media-discovered `/bin` descriptors retained by exec.
pub const MAX_MEDIA_PROGRAMS: usize = 4;

const MEDIA_PROGRAM_ID_BASE: usize = 10_000;
const MAX_MEDIA_PROGRAM_NAME_BYTES: usize = 32;
pub(crate) const MAX_MEDIA_PROGRAM_PATH_BYTES: usize = MAX_MEDIA_PROGRAM_NAME_BYTES + 5;
const MAX_MEDIA_PROGRAM_ENTRY_BYTES: usize = MAX_MEDIA_PROGRAM_NAME_BYTES + 4;
const MEDIA_PROGRAM_SUMMARY: &str = "provider-discovered /bin program";

/// Borrowed argv for one image-program invocation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProgramArgv<'a> {
    args: [&'a str; MAX_PROGRAM_ARGS],
    argc: usize,
}

impl<'a> ProgramArgv<'a> {
    /// Creates an empty argv.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            args: [""; MAX_PROGRAM_ARGS],
            argc: 0,
        }
    }

    /// Appends one argv entry.
    pub(crate) fn push(&mut self, arg: &'a str) -> bool {
        if self.argc >= self.args.len() {
            return false;
        }
        self.args[self.argc] = arg;
        self.argc += 1;
        true
    }

    /// Number of argv entries.
    #[must_use]
    pub const fn argc(&self) -> usize {
        self.argc
    }

    /// Returns argv[index].
    #[must_use]
    pub fn arg(&self, index: usize) -> Option<&'a str> {
        if index < self.argc {
            Some(self.args[index])
        } else {
            None
        }
    }
}

/// Error while building an owned argv buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProgramArgvBuildError {
    /// The argv buffer already contains the maximum number of entries.
    TooManyArgs,
    /// One argv entry exceeded [`MAX_PROGRAM_ARG_BYTES`].
    ArgTooLong,
}

impl ProgramArgvBuildError {
    /// Stable diagnostic word.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TooManyArgs => "too-many-args",
            Self::ArgTooLong => "arg-too-long",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ProgramArg {
    bytes: [u8; MAX_PROGRAM_ARG_BYTES],
    len: usize,
}

impl ProgramArg {
    const fn empty() -> Self {
        Self {
            bytes: [0u8; MAX_PROGRAM_ARG_BYTES],
            len: 0,
        }
    }

    fn write(&mut self, arg: &str) -> Result<(), ProgramArgvBuildError> {
        if arg.len() > self.bytes.len() {
            return Err(ProgramArgvBuildError::ArgTooLong);
        }
        self.len = 0;
        let bytes = arg.as_bytes();
        while self.len < bytes.len() {
            self.bytes[self.len] = bytes[self.len];
            self.len += 1;
        }
        Ok(())
    }

    fn as_str(&self) -> &str {
        // Program argv originates from the root-shell tokenizer or existing
        // program argv, both already represented as `str`.
        unsafe { core::str::from_utf8_unchecked(&self.bytes[..self.len]) }
    }
}

/// Owned, bounded argv payload retained across exec admission and scheduling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProgramArgvBuffer {
    args: [ProgramArg; MAX_PROGRAM_ARGS],
    argc: usize,
}

impl ProgramArgvBuffer {
    /// Creates an empty owned argv buffer.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            args: [ProgramArg::empty(); MAX_PROGRAM_ARGS],
            argc: 0,
        }
    }

    /// Appends one argv entry.
    pub fn push(&mut self, arg: &str) -> Result<(), ProgramArgvBuildError> {
        if self.argc >= self.args.len() {
            return Err(ProgramArgvBuildError::TooManyArgs);
        }
        self.args[self.argc].write(arg)?;
        self.argc += 1;
        Ok(())
    }

    /// Builds a one-entry argv buffer.
    pub fn from_argv0(argv0: &str) -> Result<Self, ProgramArgvBuildError> {
        let mut argv = Self::empty();
        argv.push(argv0)?;
        Ok(argv)
    }

    /// Number of argv entries.
    #[must_use]
    pub const fn argc(&self) -> usize {
        self.argc
    }

    /// Returns argv[index].
    #[must_use]
    pub fn arg(&self, index: usize) -> Option<&str> {
        if index < self.argc {
            Some(self.args[index].as_str())
        } else {
            None
        }
    }

    /// Returns argv[0], if present.
    #[must_use]
    pub fn argv0(&self) -> Option<&str> {
        self.arg(0)
    }

    /// Creates a borrowed argv view for synchronous program entry dispatch.
    #[must_use]
    pub fn borrowed(&self) -> ProgramArgv<'_> {
        let mut argv = ProgramArgv::empty();
        let mut index = 0usize;
        while index < self.argc {
            let _ = argv.push(self.args[index].as_str());
            index += 1;
        }
        argv
    }
}

/// Borrowed environment variable for one image-program invocation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProgramEnvVar<'a> {
    name: &'a str,
    value: &'a str,
}

impl<'a> ProgramEnvVar<'a> {
    const fn empty() -> Self {
        Self {
            name: "",
            value: "",
        }
    }

    const fn new(name: &'a str, value: &'a str) -> Self {
        Self { name, value }
    }
}

/// Borrowed environment for one image-program invocation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProgramEnv<'a> {
    vars: [ProgramEnvVar<'a>; MAX_PROGRAM_ENVS],
    envc: usize,
}

impl<'a> ProgramEnv<'a> {
    /// Creates an empty environment.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            vars: [ProgramEnvVar::empty(); MAX_PROGRAM_ENVS],
            envc: 0,
        }
    }

    /// Appends one environment entry.
    pub(crate) fn push(&mut self, name: &'a str, value: &'a str) -> bool {
        if self.envc >= self.vars.len() {
            return false;
        }
        self.vars[self.envc] = ProgramEnvVar::new(name, value);
        self.envc += 1;
        true
    }

    /// Number of environment entries.
    #[must_use]
    pub const fn envc(&self) -> usize {
        self.envc
    }

    /// Returns the first value matching `name`.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&'a str> {
        let mut index = 0usize;
        while index < self.envc {
            let var = self.vars[index];
            if var.name == name {
                return Some(var.value);
            }
            index += 1;
        }
        None
    }
}

/// Error while building an owned environment buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProgramEnvBuildError {
    /// The environment buffer already contains the maximum number of entries.
    TooManyVars,
    /// The variable name is empty or contains unsupported bytes.
    InvalidName,
    /// One variable name exceeded [`MAX_PROGRAM_ENV_NAME_BYTES`].
    NameTooLong,
    /// One variable value exceeded [`MAX_PROGRAM_ENV_VALUE_BYTES`].
    ValueTooLong,
}

impl ProgramEnvBuildError {
    /// Stable diagnostic word.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TooManyVars => "too-many-vars",
            Self::InvalidName => "invalid-name",
            Self::NameTooLong => "name-too-long",
            Self::ValueTooLong => "value-too-long",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ProgramEnvEntry {
    name: [u8; MAX_PROGRAM_ENV_NAME_BYTES],
    name_len: usize,
    value: [u8; MAX_PROGRAM_ENV_VALUE_BYTES],
    value_len: usize,
}

impl ProgramEnvEntry {
    const fn empty() -> Self {
        Self {
            name: [0u8; MAX_PROGRAM_ENV_NAME_BYTES],
            name_len: 0,
            value: [0u8; MAX_PROGRAM_ENV_VALUE_BYTES],
            value_len: 0,
        }
    }

    fn write(&mut self, name: &str, value: &str) -> Result<(), ProgramEnvBuildError> {
        if !program_env_name_is_valid(name.as_bytes()) {
            return Err(ProgramEnvBuildError::InvalidName);
        }
        if name.len() > self.name.len() {
            return Err(ProgramEnvBuildError::NameTooLong);
        }
        if value.len() > self.value.len() {
            return Err(ProgramEnvBuildError::ValueTooLong);
        }
        self.name_len = 0;
        let name_bytes = name.as_bytes();
        while self.name_len < name_bytes.len() {
            self.name[self.name_len] = name_bytes[self.name_len];
            self.name_len += 1;
        }
        self.value_len = 0;
        let value_bytes = value.as_bytes();
        while self.value_len < value_bytes.len() {
            self.value[self.value_len] = value_bytes[self.value_len];
            self.value_len += 1;
        }
        Ok(())
    }

    fn name(&self) -> &str {
        // Environment names are validated as ASCII before storage.
        unsafe { core::str::from_utf8_unchecked(&self.name[..self.name_len]) }
    }

    fn value(&self) -> &str {
        // Environment values originate from shell tokens, already represented as `str`.
        unsafe { core::str::from_utf8_unchecked(&self.value[..self.value_len]) }
    }
}

/// Owned, bounded environment payload retained across exec admission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProgramEnvBuffer {
    vars: [ProgramEnvEntry; MAX_PROGRAM_ENVS],
    envc: usize,
}

impl ProgramEnvBuffer {
    /// Creates an empty owned environment buffer.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            vars: [ProgramEnvEntry::empty(); MAX_PROGRAM_ENVS],
            envc: 0,
        }
    }

    /// Appends one environment variable.
    pub fn push(&mut self, name: &str, value: &str) -> Result<(), ProgramEnvBuildError> {
        if self.envc >= self.vars.len() {
            return Err(ProgramEnvBuildError::TooManyVars);
        }
        self.vars[self.envc].write(name, value)?;
        self.envc += 1;
        Ok(())
    }

    /// Number of environment entries.
    #[must_use]
    pub const fn envc(&self) -> usize {
        self.envc
    }

    /// Returns the first value matching `name`.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&str> {
        let mut index = 0usize;
        while index < self.envc {
            if self.vars[index].name() == name {
                return Some(self.vars[index].value());
            }
            index += 1;
        }
        None
    }

    /// Returns the variable name at `index`.
    #[must_use]
    pub fn name(&self, index: usize) -> Option<&str> {
        if index < self.envc {
            Some(self.vars[index].name())
        } else {
            None
        }
    }

    /// Returns the variable value at `index`.
    #[must_use]
    pub fn value(&self, index: usize) -> Option<&str> {
        if index < self.envc {
            Some(self.vars[index].value())
        } else {
            None
        }
    }

    /// Creates a borrowed environment view for synchronous program entry.
    #[must_use]
    pub fn borrowed(&self) -> ProgramEnv<'_> {
        let mut env = ProgramEnv::empty();
        let mut index = 0usize;
        while index < self.envc {
            let var = &self.vars[index];
            let _ = env.push(var.name(), var.value());
            index += 1;
        }
        env
    }
}

/// Returns whether an environment variable name is accepted by the shell/entry ABI.
#[must_use]
pub fn program_env_name_is_valid(name: &[u8]) -> bool {
    if name.is_empty() || name.len() > MAX_PROGRAM_ENV_NAME_BYTES {
        return false;
    }
    let mut index = 0usize;
    while index < name.len() {
        let byte = name[index];
        let ok = byte == b'_' || byte.is_ascii_alphanumeric();
        if !ok || (index == 0 && byte.is_ascii_digit()) {
            return false;
        }
        index += 1;
    }
    true
}

/// Execution status returned by one `/bin` program.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProgramStatus {
    /// No program was present in the parsed line.
    Empty,
    /// The program completed normally.
    Ok,
    /// The program was rejected or reported a failure.
    Error,
    /// The program returned a numeric Reovim exit code.
    ExitCode(i32),
    /// The current image was replaced by another program image.
    Replaced,
    /// The process is blocked with retained syscall state.
    Blocked,
    /// The program requested root-daemon shutdown.
    Halt,
}

impl ProgramStatus {
    /// Stable text used in `klog` records.
    #[must_use]
    pub const fn as_bytes(self) -> &'static [u8] {
        match self {
            Self::Empty => b"empty",
            Self::Ok => b"ok",
            Self::Error => b"error",
            Self::ExitCode(_) => b"exit-code",
            Self::Replaced => b"replaced",
            Self::Blocked => b"blocked",
            Self::Halt => b"halt",
        }
    }

    /// Numeric process exit code represented by this program status.
    #[must_use]
    pub const fn exit_code(self) -> i32 {
        match self {
            Self::Empty | Self::Ok | Self::Blocked | Self::Halt => 0,
            Self::Error => 1,
            Self::ExitCode(code) => code,
            Self::Replaced => 0,
        }
    }

    /// Whether this status counts as successful completion.
    #[must_use]
    pub const fn is_success(self) -> bool {
        match self {
            Self::Ok | Self::Replaced => true,
            Self::Empty | Self::Error | Self::Blocked | Self::Halt => false,
            Self::ExitCode(code) => code == 0,
        }
    }
}

/// Source kind for a loaded `/bin` program.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProgramImageKind {
    /// Program body is interpreted from a bounded Reovim source image.
    SourceImage,
    /// Program body is a checked Reovim executable wrapper.
    ReovimExecBody,
    /// Program body is linked into the OS image as real code.
    LinkedBin,
}

impl ProgramImageKind {
    /// Stable diagnostic word.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SourceImage => "source-image",
            Self::ReovimExecBody => "reovim-exec-body",
            Self::LinkedBin => "linked-bin",
        }
    }
}

/// Linked `/bin` entry point.
///
/// The entry receives borrowed argv plus the raw syscall transport hook. Real
/// program semantics should be reached through domain uapi wrappers over that
/// hook, not by calling [`ProgramSyscalls`] convenience methods directly.
pub type LinkedProgramEntry = for<'argv> fn(&ProgramArgv<'argv>, RawSyscall) -> ProgramStatus;

const SOURCE_BYTES_MAGIC: &[u8] = b"reovim-source-v1";
const SOURCE_BYTES_OP_REJECT_ARGC_GREATER: &[u8] = b"reject-argc-greater ";
const SOURCE_BYTES_OP_REJECT_ARGC_GREATER_UNLESS_ARG1: &[u8] = b"reject-argc-greater-unless-arg1 ";
const SOURCE_BYTES_OP_WRITE_CWD_LINE: &[u8] = b"write-cwd-line";
const SOURCE_BYTES_OP_WRITE_VFS_LISTING_ARG1_OR_CWD: &[u8] = b"write-vfs-listing-arg1-or-cwd";
const SOURCE_BYTES_OP_SET_CWD_ARG1_OR_ROOT: &[u8] = b"set-cwd-arg1-or-root";
const SOURCE_BYTES_OP_WRITE_STDIN_OR_VFS_FILES_ARGV_TAIL: &[u8] =
    b"write-stdin-or-vfs-files-argv-tail";
const SOURCE_BYTES_OP_WRITE_TTY_LINE: &[u8] = b"write-tty-line";
const SOURCE_BYTES_OP_WRITE_HELP_ARG1_OR_CATALOG: &[u8] = b"write-help-arg1-or-catalog";
const SOURCE_BYTES_OP_WRITE_MOUNT_TABLE: &[u8] = b"write-mount-table";
const SOURCE_BYTES_OP_WRITE_BOOT_INFO_SUMMARY: &[u8] = b"write-boot-info-summary";
const SOURCE_BYTES_OP_WRITE_DEVICE_INVENTORY: &[u8] = b"write-device-inventory";
const SOURCE_BYTES_OP_WRITE_BOOT_INPUT: &[u8] = b"write-boot-input";
const SOURCE_BYTES_OP_WRITE_BOOT_STATUS: &[u8] = b"write-boot-status";
const SOURCE_BYTES_OP_WRITE_BOOT_PROOF: &[u8] = b"write-boot-proof";
const SOURCE_BYTES_OP_REQUEST_SERVICE: &[u8] = b"request-service ";
const SOURCE_BYTES_OP_REQUEST_SHELL_TARGET: &[u8] = b"request-shell-target ";
const SOURCE_BYTES_OP_REQUEST_LINE_DISCIPLINE: &[u8] = b"request-line-discipline ";
const SOURCE_BYTES_OP_REQUEST_ROOT_SHELL: &[u8] = b"request-root-shell";
const SOURCE_BYTES_OP_WRITE_KERNEL_LOG_VIEW: &[u8] = b"write-kernel-log-view";
const SOURCE_BYTES_OP_WRITE_KERNEL_LOG_STATS: &[u8] = b"write-kernel-log-stats";
const SOURCE_BYTES_OP_WRITE_DUMP_STATUS: &[u8] = b"write-dump-status";
const SOURCE_BYTES_OP_WRITE_DUMP_SNAPSHOT: &[u8] = b"write-dump-snapshot";
const SOURCE_BYTES_OP_WRITE_DUMP_SYNC: &[u8] = b"write-dump-sync";
const SOURCE_BYTES_OP_WRITE_SCHEDULER_STATE: &[u8] = b"write-scheduler-state";
const SOURCE_BYTES_OP_WRITE_SCHEDULER_TICK: &[u8] = b"write-scheduler-tick";
const SOURCE_BYTES_OP_WRITE_SCHEDULER_YIELD: &[u8] = b"write-scheduler-yield";
const SOURCE_BYTES_OP_WRITE_SCHEDULER_SLEEP_ARG2: &[u8] = b"write-scheduler-sleep-arg2";
const SOURCE_BYTES_OP_WRITE_PROCESS_TABLE: &[u8] = b"write-process-table";
const SOURCE_BYTES_OP_WRITE_SESSION_STATE: &[u8] = b"write-session-state";
const SOURCE_BYTES_OP_WRITE_SERVICE_TABLE: &[u8] = b"write-service-table";
const SOURCE_BYTES_OP_WRITE_EXEC_LOAD_TABLE: &[u8] = b"write-exec-load-table";
const SOURCE_BYTES_OP_WRITE_PENDING_EXEC_TABLE: &[u8] = b"write-pending-exec-table";
const SOURCE_BYTES_OP_WRITE_PROCESS_SELF: &[u8] = b"write-process-self";
const SOURCE_BYTES_OP_WRITE_SOURCE_STORE_TABLE: &[u8] = b"write-source-store-table";
const SOURCE_BYTES_OP_WRITE_SOURCE_MEDIA_TABLE: &[u8] = b"write-source-media-table";
const SOURCE_BYTES_OP_WRITE_TASK_TABLE: &[u8] = b"write-task-table";
const SOURCE_BYTES_OP_WRITE_WAIT_TABLE: &[u8] = b"write-wait-table";
const SOURCE_BYTES_OP_WRITE_SYSCALL_TABLE: &[u8] = b"write-syscall-table";
const SOURCE_BYTES_OP_REQUIRE_LAUNCH_ENABLED: &[u8] = b"require-launch-enabled ";
const SOURCE_BYTES_OP_LAUNCH_PAYLOAD_ARG1_OR_LIST: &[u8] = b"launch-payload-arg1-or-list";
const SOURCE_BYTES_OP_LAUNCH_PAYLOAD_NAME: &[u8] = b"launch-payload-name ";
const SOURCE_BYTES_OP_CLEAR_CONSOLE: &[u8] = b"clear-console";
const SOURCE_BYTES_OP_WRITE_STDOUT_HEX: &[u8] = b"write-stdout-hex ";
const SOURCE_BYTES_OP_EXIT_STATUS: &[u8] = b"exit-status ";
const SOURCE_BYTES_OP_EXIT_CODE: &[u8] = b"exit-code ";
const SOURCE_BYTES_OP_DISPATCH_ARG1: &[u8] = b"dispatch-arg1 ";
const SOURCE_BYTES_DISPATCH_DEFAULT: &[u8] = b"default";
const SOURCE_BYTES_DISPATCH_CASE: &[u8] = b"case ";
const SOURCE_BYTES_END_DISPATCH_ARG1: &[u8] = b"end-dispatch-arg1";
const SOURCE_BYTES_MAX_OPS: usize = 64;
const BIN_UAPI_BODY_MAGIC: &[u8] = b"reovim-bin-uapi-v1";
const BIN_UAPI_OP_OPEN_READONLY_WRITE_STDOUT_ENV_OR_ARG1_OR: &[u8] =
    b"open-readonly-write-stdout-env-or-arg1-or ";
const BIN_UAPI_OP_OPEN_READONLY_WRITE_STDOUT_ARG1_OR: &[u8] =
    b"open-readonly-write-stdout-arg1-or ";
const BIN_UAPI_OP_OPEN_READONLY_WRITE_STDOUT: &[u8] = b"open-readonly-write-stdout ";
const BIN_UAPI_OP_EXEC_BIN_ENV: &[u8] = b"exec-bin-env ";
const BIN_UAPI_OP_SLEEP_TICKS: &[u8] = b"sleep-ticks ";
const BIN_UAPI_OP_SPAWN_SLEEP_BIN_ENV: &[u8] = b"spawn-sleep-bin-env ";
const BIN_UAPI_OP_SPAWN_BIN_ENV: &[u8] = b"spawn-bin-env ";
const BIN_UAPI_OP_SPAWN_WAIT_BIN_ENV: &[u8] = b"spawn-wait-bin-env ";
const BIN_UAPI_OP_SPAWN_WAIT_BIN: &[u8] = b"spawn-wait-bin ";
const BIN_UAPI_OP_SCHEDULER_TICK: &[u8] = b"scheduler-tick";
const BIN_UAPI_OP_YIELD_NOW: &[u8] = b"yield-now";
const BIN_UAPI_OP_WRITE_STDOUT_HEX: &[u8] = b"write-stdout-hex ";
const BIN_UAPI_OP_EXIT_STATUS: &[u8] = b"exit-status ";
const BIN_UAPI_OP_EXIT_CODE: &[u8] = b"exit-code ";
const BIN_UAPI_MAX_OPS: usize = 32;
const BIN_UAPI_MAX_PATH_BYTES: usize = MAX_PROGRAM_ARG_BYTES;
const MAX_BIN_UAPI_RESUME_FRAMES: usize = proc::MAX_PROCESSES;

/// Executable body reference attached to one `/bin` descriptor.
#[derive(Clone, Copy, Debug)]
pub enum ProgramImage {
    /// Program source bytes are loaded from a source store path.
    SourcePath(&'static str),
    /// Program entry is linked into the OS image as code.
    Linked(LinkedProgramEntry),
}

impl ProgramImage {
    /// Loader/source kind for this image body.
    #[must_use]
    pub const fn kind(self) -> ProgramImageKind {
        match self {
            Self::SourcePath(_) => ProgramImageKind::SourceImage,
            Self::Linked(_) => ProgramImageKind::LinkedBin,
        }
    }

    /// Source-store path for this executable body.
    #[must_use]
    pub const fn source_path(self) -> &'static str {
        match self {
            Self::SourcePath(path) => path,
            Self::Linked(_) => "",
        }
    }

    /// Linked entry function, when this image is linked code.
    #[must_use]
    pub const fn linked_entry(self) -> Option<LinkedProgramEntry> {
        match self {
            Self::SourcePath(_) => None,
            Self::Linked(entry) => Some(entry),
        }
    }
}

/// Byte artifact available to the executable loader.
#[derive(Clone, Copy, Debug)]
pub struct ProgramSourceArtifact {
    /// Loader-visible artifact path.
    pub path: &'static str,
    /// Encoded source kind.
    pub kind: ProgramImageKind,
    /// Encoded source-image bytes.
    pub bytes: &'static [u8],
}

/// Status-only `/bin` source image variants installable through the runtime
/// source overlay.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinSourceInstallStatus {
    /// Installed `/bin` image prints success text and exits ok.
    Ok,
    /// Installed `/bin` image prints failure text and exits error.
    Error,
}

impl BinSourceInstallStatus {
    /// Stable diagnostic word.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Error => "error",
        }
    }

    /// Byte-backed source image installed for this status.
    #[must_use]
    pub const fn source_bytes(self) -> &'static [u8] {
        match self {
            Self::Ok => BIN_SOURCE_INSTALL_OK_IMAGE,
            Self::Error => BIN_SOURCE_INSTALL_ERROR_IMAGE,
        }
    }
}

const BIN_SOURCE_INSTALL_OK_IMAGE: &[u8] =
    b"reovim-source-v1\nwrite-stdout-hex 696e7374616c6c65642d62696e2e6f6b0a\nexit-status ok\n";
const BIN_SOURCE_INSTALL_ERROR_IMAGE: &[u8] =
    b"reovim-source-v1\nwrite-stdout-hex 696e7374616c6c65642d62696e2e6572726f720a\nexit-status error\n";

/// Stable image program descriptor.
#[derive(Clone, Copy, Debug)]
pub struct ProgramDescriptor {
    /// Image-local executable identifier. The kernel treats this as opaque.
    pub id: usize,
    /// Basename exposed inside `/bin` and accepted as a shell lookup alias.
    pub name: &'static str,
    /// Absolute kernel VFS path for the image-packaged program.
    pub path: &'static str,
    /// One-line operator summary.
    pub summary: &'static str,
    /// Executable body for this `/bin` program.
    pub image: ProgramImage,
    /// Stable name for the executable entry.
    pub entry_name: &'static str,
}

impl ProgramDescriptor {
    /// Loader/source kind for this executable descriptor.
    #[must_use]
    pub const fn image_kind(self) -> ProgramImageKind {
        self.image.kind()
    }

    /// Loader-visible source/artifact path for diagnostics.
    #[must_use]
    pub const fn source_path(self) -> &'static str {
        match self.image {
            ProgramImage::SourcePath(path) => path,
            ProgramImage::Linked(_) => self.path,
        }
    }
}

static EMPTY_PROGRAM_DESCRIPTOR: ProgramDescriptor = ProgramDescriptor {
    id: 0,
    name: "",
    path: "",
    summary: "",
    image: ProgramImage::SourcePath(""),
    entry_name: "",
};

/// Failure while installing a media-discovered `/bin` descriptor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaProgramInstallError {
    /// The media path is not a single-component `/bin/<name>` path.
    InvalidPath,
    /// The `/bin` basename does not fit the bounded descriptor table.
    NameTooLong,
    /// All bounded media descriptor slots are occupied.
    NoSlot,
}

#[derive(Clone, Copy)]
struct MediaProgramSlot {
    name: [u8; MAX_MEDIA_PROGRAM_NAME_BYTES],
    name_len: usize,
    path: [u8; MAX_MEDIA_PROGRAM_PATH_BYTES],
    path_len: usize,
    entry_name: [u8; MAX_MEDIA_PROGRAM_ENTRY_BYTES],
    entry_name_len: usize,
    descriptor: ProgramDescriptor,
    occupied: bool,
}

impl MediaProgramSlot {
    const fn empty() -> Self {
        Self {
            name: [0u8; MAX_MEDIA_PROGRAM_NAME_BYTES],
            name_len: 0,
            path: [0u8; MAX_MEDIA_PROGRAM_PATH_BYTES],
            path_len: 0,
            entry_name: [0u8; MAX_MEDIA_PROGRAM_ENTRY_BYTES],
            entry_name_len: 0,
            descriptor: ProgramDescriptor {
                id: 0,
                name: "",
                path: "",
                summary: "",
                image: ProgramImage::SourcePath(""),
                entry_name: "",
            },
            occupied: false,
        }
    }

    fn clear(&mut self) {
        *self = Self::empty();
    }

    fn write(&mut self, index: usize, path: &str) -> Result<(), MediaProgramInstallError> {
        let name = media_program_name_from_path(path)?;
        if name.len() > self.name.len() {
            return Err(MediaProgramInstallError::NameTooLong);
        }
        if path.len() > self.path.len() {
            return Err(MediaProgramInstallError::NameTooLong);
        }

        self.name_len = name.len();
        self.name[..self.name_len].copy_from_slice(name.as_bytes());
        self.path_len = path.len();
        self.path[..self.path_len].copy_from_slice(path.as_bytes());
        self.entry_name_len = write_media_entry_name(name, &mut self.entry_name)?;

        let name = slot_str(self.name.as_ptr(), self.name_len);
        let path = slot_str(self.path.as_ptr(), self.path_len);
        let entry_name = slot_str(self.entry_name.as_ptr(), self.entry_name_len);
        self.descriptor = ProgramDescriptor {
            id: MEDIA_PROGRAM_ID_BASE + index,
            name,
            path,
            summary: MEDIA_PROGRAM_SUMMARY,
            image: ProgramImage::SourcePath(path),
            entry_name,
        };
        self.occupied = true;
        Ok(())
    }
}

struct MediaProgramTable {
    slots: [MediaProgramSlot; MAX_MEDIA_PROGRAMS],
}

impl MediaProgramTable {
    const fn new() -> Self {
        Self {
            slots: [MediaProgramSlot::empty(); MAX_MEDIA_PROGRAMS],
        }
    }

    fn reset(&mut self) {
        let mut index = 0usize;
        while index < self.slots.len() {
            self.slots[index].clear();
            index += 1;
        }
    }

    fn find_path(&self, path: &str) -> Option<usize> {
        let mut index = 0usize;
        while index < self.slots.len() {
            if self.slots[index].occupied && self.slots[index].descriptor.path == path {
                return Some(index);
            }
            index += 1;
        }
        None
    }

    fn find_name(&self, name: &str) -> Option<usize> {
        let mut index = 0usize;
        while index < self.slots.len() {
            if self.slots[index].occupied && self.slots[index].descriptor.name == name {
                return Some(index);
            }
            index += 1;
        }
        None
    }

    fn install(&mut self, path: &str) -> Result<usize, MediaProgramInstallError> {
        if let Some(index) = self.find_path(path) {
            return Ok(index);
        }
        let mut index = 0usize;
        while index < self.slots.len() {
            if !self.slots[index].occupied {
                self.slots[index].write(index, path)?;
                return Ok(index);
            }
            index += 1;
        }
        Err(MediaProgramInstallError::NoSlot)
    }
}

struct MediaProgramCell(UnsafeCell<MediaProgramTable>);

// SAFETY: mutable access is serialized by `MEDIA_PROGRAM_LOCK`.
unsafe impl Sync for MediaProgramCell {}

static MEDIA_PROGRAMS: MediaProgramCell =
    MediaProgramCell(UnsafeCell::new(MediaProgramTable::new()));
static MEDIA_PROGRAM_LOCK: AtomicBool = AtomicBool::new(false);

struct MediaProgramGuard;

impl MediaProgramGuard {
    fn acquire() -> Self {
        while MEDIA_PROGRAM_LOCK
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }
        Self
    }
}

impl Drop for MediaProgramGuard {
    fn drop(&mut self) {
        MEDIA_PROGRAM_LOCK.store(false, Ordering::Release);
    }
}

fn with_media_programs<R>(f: impl FnOnce(&mut MediaProgramTable) -> R) -> R {
    let _guard = MediaProgramGuard::acquire();
    // SAFETY: `MEDIA_PROGRAM_LOCK` serializes access to the media descriptor table.
    let table = unsafe { &mut *MEDIA_PROGRAMS.0.get() };
    f(table)
}

fn media_program_descriptor(index: usize) -> &'static ProgramDescriptor {
    // SAFETY: media descriptor slots live for the program lifetime. Tests reset
    // between isolated cases; loaded programs must not outlive a reset.
    unsafe { &(*MEDIA_PROGRAMS.0.get()).slots[index].descriptor }
}

fn slot_str(ptr: *const u8, len: usize) -> &'static str {
    // SAFETY: callers only pass bytes previously copied from Rust `str` values
    // or ASCII-generated entry names into static media descriptor storage.
    unsafe { core::str::from_utf8_unchecked(slice::from_raw_parts(ptr, len)) }
}

fn media_program_name_from_path(path: &str) -> Result<&str, MediaProgramInstallError> {
    let Some(name) = path.strip_prefix("/bin/") else {
        return Err(MediaProgramInstallError::InvalidPath);
    };
    if name.is_empty() || name.as_bytes().contains(&b'/') {
        return Err(MediaProgramInstallError::InvalidPath);
    }
    let mut index = 0usize;
    let bytes = name.as_bytes();
    while index < bytes.len() {
        let byte = bytes[index];
        if !(byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_') {
            return Err(MediaProgramInstallError::InvalidPath);
        }
        index += 1;
    }
    Ok(name)
}

fn write_media_entry_name(
    name: &str,
    out: &mut [u8; MAX_MEDIA_PROGRAM_ENTRY_BYTES],
) -> Result<usize, MediaProgramInstallError> {
    if name.len() + 4 > out.len() {
        return Err(MediaProgramInstallError::NameTooLong);
    }
    out[0] = b'b';
    out[1] = b'i';
    out[2] = b'n';
    out[3] = b'_';
    let mut index = 0usize;
    while index < name.len() {
        let byte = name.as_bytes()[index];
        out[index + 4] = if byte == b'-' { b'_' } else { byte };
        index += 1;
    }
    Ok(name.len() + 4)
}

/// Clears media-discovered `/bin` descriptors.
pub fn reset_media_programs() {
    with_media_programs(MediaProgramTable::reset);
}

/// Builds a checked `/bin/<name>` media path from an argv0 token.
///
/// Absolute argv0 values must already be under `/bin`; basename argv0 values
/// are converted to `/bin/<argv0>`.
pub fn media_program_path_from_argv0<'a>(
    argv0: &str,
    out: &'a mut [u8; MAX_MEDIA_PROGRAM_PATH_BYTES],
) -> Option<&'a str> {
    let len = if argv0.as_bytes().first() == Some(&b'/') {
        if argv0.len() > out.len() {
            return None;
        }
        out[..argv0.len()].copy_from_slice(argv0.as_bytes());
        argv0.len()
    } else {
        let prefix = b"/bin/";
        if prefix.len() + argv0.len() > out.len() {
            return None;
        }
        out[..prefix.len()].copy_from_slice(prefix);
        out[prefix.len()..prefix.len() + argv0.len()].copy_from_slice(argv0.as_bytes());
        prefix.len() + argv0.len()
    };
    // SAFETY: bytes were copied from Rust `str` values plus the ASCII `/bin/` prefix.
    let path = unsafe { core::str::from_utf8_unchecked(&out[..len]) };
    if media_program_name_from_path(path).is_err() {
        return None;
    }
    Some(path)
}

/// Installs or returns a media-discovered `/bin` descriptor.
pub fn install_media_program(
    path: &str,
) -> Result<&'static ProgramDescriptor, MediaProgramInstallError> {
    let index = with_media_programs(|programs| programs.install(path))?;
    Ok(media_program_descriptor(index))
}

fn find_media_by_bin_basename(name: &str) -> Option<(usize, &'static ProgramDescriptor)> {
    let index = with_media_programs(|programs| programs.find_name(name))?;
    Some((MEDIA_PROGRAM_ID_BASE + index, media_program_descriptor(index)))
}

fn find_media_by_path(path: &str) -> Option<(usize, &'static ProgramDescriptor)> {
    let index = with_media_programs(|programs| programs.find_path(path))?;
    Some((MEDIA_PROGRAM_ID_BASE + index, media_program_descriptor(index)))
}

fn find_media_by_id(id: usize) -> Option<(usize, &'static ProgramDescriptor)> {
    if id < MEDIA_PROGRAM_ID_BASE {
        return None;
    }
    let index = id - MEDIA_PROGRAM_ID_BASE;
    if index >= MAX_MEDIA_PROGRAMS {
        return None;
    }
    let exists = with_media_programs(|programs| programs.slots[index].occupied);
    if exists {
        Some((id, media_program_descriptor(index)))
    } else {
        None
    }
}

/// Snapshots currently installed media-discovered `/bin` descriptors.
///
/// Image descriptors remain supplied by the active OS image. This helper
/// exposes the bounded runtime descriptor extension so `/bin`, help, and VFS
/// surfaces do not hide source-media-admitted executables.
pub fn snapshot_media_programs(out: &mut [Option<&'static ProgramDescriptor>]) -> usize {
    let count = with_media_programs(|programs| {
        let mut written = 0usize;
        let mut index = 0usize;
        while index < programs.slots.len() && written < out.len() {
            if programs.slots[index].occupied {
                out[written] = Some(media_program_descriptor(index));
                written += 1;
            }
            index += 1;
        }
        written
    });
    let mut index = count;
    while index < out.len() {
        out[index] = None;
        index += 1;
    }
    count
}

/// Loaded executable object handed from the shell frontend to proc/syscall.
#[derive(Clone, Copy, Debug)]
pub struct LoadedProgram {
    /// Index in the current image program catalog.
    pub catalog_index: usize,
    /// Stable descriptor for the executable.
    pub descriptor: &'static ProgramDescriptor,
    /// Loader/source kind for this executable object.
    pub image_kind: ProgramImageKind,
    /// Loader-visible artifact path that supplied the executable bytes.
    pub source_path: &'static str,
    source_bytes: &'static [u8],
    linked_entry: Option<LinkedProgramEntry>,
}

fn find_by_bin_basename(
    programs: &'static [ProgramDescriptor],
    name: &str,
) -> Option<(usize, &'static ProgramDescriptor)> {
    let mut index = 0usize;
    while index < programs.len() {
        if programs[index].name == name {
            return Some((index, &programs[index]));
        }
        index += 1;
    }
    None
}

/// Finds a program by absolute `/bin` path.
#[must_use]
pub fn find_by_path(
    programs: &'static [ProgramDescriptor],
    path: &str,
) -> Option<(usize, &'static ProgramDescriptor)> {
    let mut index = 0usize;
    while index < programs.len() {
        if programs[index].path == path {
            return Some((index, &programs[index]));
        }
        index += 1;
    }
    find_media_by_path(path)
}

/// Finds a program by basename inside `/bin`.
#[must_use]
pub fn find_by_bin_name(
    programs: &'static [ProgramDescriptor],
    name: &str,
) -> Option<(usize, &'static ProgramDescriptor)> {
    find_by_bin_basename(programs, name).or_else(|| find_media_by_bin_basename(name))
}

/// Finds a program by stable descriptor identifier.
#[must_use]
pub fn find_by_id(
    programs: &'static [ProgramDescriptor],
    id: usize,
) -> Option<(usize, &'static ProgramDescriptor)> {
    let mut index = 0usize;
    while index < programs.len() {
        if programs[index].id == id {
            return Some((index, &programs[index]));
        }
        index += 1;
    }
    find_media_by_id(id)
}

/// Resolves argv[0] as either an absolute program path or a `/bin` basename.
#[must_use]
pub fn resolve_argv0(
    programs: &'static [ProgramDescriptor],
    argv0: &str,
) -> Option<(usize, &'static ProgramDescriptor)> {
    if argv0.as_bytes().first() == Some(&b'/') {
        find_by_path(programs, argv0)
    } else {
        find_by_bin_name(programs, argv0)
    }
}

/// Source-store failure while loading a resolved `/bin` descriptor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProgramLoadError {
    /// Descriptor source path was not present in the source store.
    SourceNotFound,
    /// Source bytes failed loader validation.
    InvalidImage,
}

struct LinkedRawSyscallCell(UnsafeCell<Option<NonNull<()>>>);

// SAFETY: mutable access is serialized by `LINKED_RAW_SYSCALL_LOCK`.
unsafe impl Sync for LinkedRawSyscallCell {}

static LINKED_RAW_SYSCALLS: LinkedRawSyscallCell = LinkedRawSyscallCell(UnsafeCell::new(None));
static LINKED_RAW_SYSCALL_LOCK: AtomicBool = AtomicBool::new(false);

struct LinkedRawSyscallGuard {
    previous: Option<NonNull<()>>,
    owns_lock: bool,
}

impl LinkedRawSyscallGuard {
    fn enter(syscalls: &mut ProgramSyscalls<'_, '_, '_>) -> Option<Self> {
        if LINKED_RAW_SYSCALL_LOCK
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            // SAFETY: the lock is held until this outer guard drops, and linked
            // entries execute synchronously inside the current program dispatch
            // frame.
            unsafe {
                *LINKED_RAW_SYSCALLS.0.get() = Some(NonNull::from(syscalls).cast());
            }
            return Some(Self {
                previous: None,
                owns_lock: true,
            });
        }

        // A linked program may run another linked child while handling a raw
        // syscall (for example, timed wait dispatching a woken child). Permit
        // that nested install only for the same rootd/session execution scope
        // and restore the parent handle when the child returns.
        // SAFETY: when the lock is held, the cell either contains the current
        // synchronous linked syscall handle or `None` while an outer guard is
        // unwinding. We never release the lock for nested entries.
        let previous = unsafe { *LINKED_RAW_SYSCALLS.0.get() };
        let previous_ptr = previous?;
        // SAFETY: the pointer was installed by an active guard and the lock is
        // still held by that guard's synchronous call chain.
        let previous_syscalls = unsafe {
            &*(previous_ptr.as_ptr() as *const ProgramSyscalls<'static, 'static, 'static>)
        };
        if !previous_syscalls.shares_linked_syscall_scope(syscalls) {
            return None;
        }
        // SAFETY: the parent guard owns the lock; this nested guard only swaps
        // the active handle until it drops.
        unsafe {
            *LINKED_RAW_SYSCALLS.0.get() = Some(NonNull::from(syscalls).cast());
        }
        Some(Self {
            previous,
            owns_lock: false,
        })
    }
}

impl Drop for LinkedRawSyscallGuard {
    fn drop(&mut self) {
        // SAFETY: outer guards exclusively own the lock; nested guards only
        // restore the previous handle under that lock.
        unsafe {
            *LINKED_RAW_SYSCALLS.0.get() = self.previous;
        }
        if self.owns_lock {
            LINKED_RAW_SYSCALL_LOCK.store(false, Ordering::Release);
        }
    }
}

fn linked_raw_syscall(nr: SyscallNr, args: SyscallArgs) -> SyscallRet {
    // SAFETY: reads the pointer installed by `LinkedRawSyscallGuard`. The
    // pointed syscall handle is valid only for the synchronous entry call.
    let Some(ptr) = (unsafe { *LINKED_RAW_SYSCALLS.0.get() }) else {
        return SyscallRet::failure(SyscallError::NO_CURRENT_PROCESS);
    };
    // SAFETY: `LinkedRawSyscallGuard::enter` stored a live `ProgramSyscalls`
    // pointer and holds the lock for the full duration of this call path.
    let syscalls =
        unsafe { &mut *(ptr.as_ptr() as *mut ProgramSyscalls<'static, 'static, 'static>) };
    syscalls.dispatch_raw_syscall(nr, args)
}

/// Loads argv[0] as either an absolute `/bin` path or a `/bin` basename.
pub fn load_argv0(
    programs: &'static [ProgramDescriptor],
    source_store: ExecutableSourceStore,
    argv0: &str,
) -> Result<Option<LoadedProgram>, ProgramLoadError> {
    let Some((catalog_index, descriptor)) = resolve_argv0(programs, argv0) else {
        return Ok(None);
    };
    if let Some(entry) = descriptor.image.linked_entry() {
        return Ok(Some(LoadedProgram {
            catalog_index,
            descriptor,
            image_kind: descriptor.image_kind(),
            source_path: descriptor.source_path(),
            source_bytes: &[],
            linked_entry: Some(entry),
        }));
    }

    let source_path = descriptor.image.source_path();
    let Some(source) = source_store.find_program(source_path) else {
        return Err(ProgramLoadError::SourceNotFound);
    };
    if source.kind != descriptor.image_kind() || !validate_source_bytes(source.bytes) {
        return Err(ProgramLoadError::InvalidImage);
    }
    Ok(Some(LoadedProgram {
        catalog_index,
        descriptor,
        image_kind: descriptor.image_kind(),
        source_path,
        source_bytes: source.bytes,
        linked_entry: None,
    }))
}

/// Loads argv[0] from exec-owned provider bytes instead of the source overlay.
pub(crate) fn load_argv0_source_bytes(
    programs: &'static [ProgramDescriptor],
    argv0: &str,
    source_bytes: &'static [u8],
) -> Result<Option<LoadedProgram>, ProgramLoadError> {
    let Some((catalog_index, descriptor)) = resolve_argv0(programs, argv0) else {
        return Ok(None);
    };
    load_descriptor_source_bytes(catalog_index, descriptor, source_bytes).map(Some)
}

/// Loads argv[0] from a checked exec-owned executable body.
pub(crate) fn load_argv0_exec_body_bytes(
    programs: &'static [ProgramDescriptor],
    argv0: &str,
    exec_body_bytes: &'static [u8],
) -> Result<Option<LoadedProgram>, ProgramLoadError> {
    let Some((catalog_index, descriptor)) = resolve_argv0(programs, argv0) else {
        return Ok(None);
    };
    load_descriptor_exec_body_bytes(catalog_index, descriptor, exec_body_bytes).map(Some)
}

pub(crate) fn load_descriptor_source_bytes(
    catalog_index: usize,
    descriptor: &'static ProgramDescriptor,
    source_bytes: &'static [u8],
) -> Result<LoadedProgram, ProgramLoadError> {
    if descriptor.image_kind() != ProgramImageKind::SourceImage
        || !validate_source_bytes(source_bytes)
    {
        return Err(ProgramLoadError::InvalidImage);
    }
    Ok(LoadedProgram {
        catalog_index,
        descriptor,
        image_kind: descriptor.image_kind(),
        source_path: descriptor.source_path(),
        source_bytes,
        linked_entry: None,
    })
}

pub(crate) fn load_descriptor_exec_body_bytes(
    catalog_index: usize,
    descriptor: &'static ProgramDescriptor,
    exec_body_bytes: &'static [u8],
) -> Result<LoadedProgram, ProgramLoadError> {
    let Ok(body) = exec_body::parse_exec_body(exec_body_bytes) else {
        return Err(ProgramLoadError::InvalidImage);
    };
    if descriptor.image_kind() != ProgramImageKind::SourceImage || !validate_bin_exec_body(body) {
        return Err(ProgramLoadError::InvalidImage);
    }
    Ok(LoadedProgram {
        catalog_index,
        descriptor,
        image_kind: ProgramImageKind::ReovimExecBody,
        source_path: descriptor.source_path(),
        source_bytes: exec_body_bytes,
        linked_entry: None,
    })
}

/// Validates a loaded `/bin` program image before process admission.
#[must_use]
pub fn validate_loaded_program(program: LoadedProgram) -> bool {
    match program.image_kind {
        ProgramImageKind::SourceImage => validate_source_bytes(program.source_bytes),
        ProgramImageKind::ReovimExecBody => {
            matches!(
                exec_body::parse_exec_body(program.source_bytes),
                Ok(body)
                    if validate_bin_exec_body(body)
            )
        }
        ProgramImageKind::LinkedBin => program.linked_entry.is_some(),
    }
}

impl LoadedProgram {
    /// Returns the loaded executable source bytes.
    #[must_use]
    pub const fn source_bytes(self) -> &'static [u8] {
        self.source_bytes
    }
}

#[derive(Clone, Copy, Debug)]
struct BinUapiResumeFrame {
    pid: usize,
    context: SyscallContext,
    program: LoadedProgram,
    argv: ProgramArgvBuffer,
    env: ProgramEnvBuffer,
    stdin: [u8; MAX_PROGRAM_STDIN_BYTES],
    stdin_len: usize,
    write_byte: u8,
    offset: usize,
    action: BinUapiResumeAction,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BinUapiResumeAction {
    ContinueFromOffset,
    ContinueFileCopyAfterRead { file_fd: i32 },
    ContinueFileCopyAfterWrite { file_fd: i32 },
    ExitWithWaitStatus,
}

impl BinUapiResumeFrame {
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
            program: LoadedProgram {
                catalog_index: 0,
                descriptor: &EMPTY_PROGRAM_DESCRIPTOR,
                image_kind: ProgramImageKind::SourceImage,
                source_path: "",
                source_bytes: &[],
                linked_entry: None,
            },
            argv: ProgramArgvBuffer::empty(),
            env: ProgramEnvBuffer::empty(),
            stdin: [0u8; MAX_PROGRAM_STDIN_BYTES],
            stdin_len: 0,
            write_byte: 0,
            offset: 0,
            action: BinUapiResumeAction::ContinueFromOffset,
        }
    }

    const fn is_empty(self) -> bool {
        self.pid == 0
    }

    fn new(
        context: SyscallContext,
        program: LoadedProgram,
        argv: ProgramArgvBuffer,
        env: ProgramEnvBuffer,
        stdin: &[u8],
        offset: usize,
        action: BinUapiResumeAction,
    ) -> Self {
        let mut frame = Self {
            pid: context.pid,
            context,
            program,
            argv,
            env,
            stdin: [0u8; MAX_PROGRAM_STDIN_BYTES],
            stdin_len: 0,
            write_byte: 0,
            offset,
            action,
        };
        while frame.stdin_len < stdin.len() && frame.stdin_len < frame.stdin.len() {
            frame.stdin[frame.stdin_len] = stdin[frame.stdin_len];
            frame.stdin_len += 1;
        }
        frame
    }

    fn stdin(&self) -> &[u8] {
        &self.stdin[..self.stdin_len]
    }
}

struct BinUapiResumeFrameTable {
    frames: [BinUapiResumeFrame; MAX_BIN_UAPI_RESUME_FRAMES],
}

impl BinUapiResumeFrameTable {
    const fn new() -> Self {
        Self {
            frames: [BinUapiResumeFrame::empty(); MAX_BIN_UAPI_RESUME_FRAMES],
        }
    }

    fn reset(&mut self) {
        self.frames = [BinUapiResumeFrame::empty(); MAX_BIN_UAPI_RESUME_FRAMES];
    }

    fn slot_index(&self, pid: usize) -> Option<usize> {
        if pid == 0 {
            return None;
        }
        let mut index = 0usize;
        while index < self.frames.len() {
            if self.frames[index].pid == pid {
                return Some(index);
            }
            index += 1;
        }
        None
    }

    fn reclaim_stale(&mut self) {
        let mut index = 0usize;
        while index < self.frames.len() {
            let frame = self.frames[index];
            if !frame.is_empty()
                && !matches!(
                    proc::process(frame.pid),
                    Some(process)
                        if matches!(
                            process.state,
                            proc::ProcessState::Ready
                                | proc::ProcessState::Running
                                | proc::ProcessState::Blocked
                        )
                )
            {
                self.frames[index] = BinUapiResumeFrame::empty();
            }
            index += 1;
        }
    }

    fn free_slot(&mut self) -> Option<usize> {
        self.reclaim_stale();
        let mut index = 0usize;
        while index < self.frames.len() {
            if self.frames[index].is_empty() {
                return Some(index);
            }
            index += 1;
        }
        None
    }

    fn store(&mut self, frame: BinUapiResumeFrame) -> bool {
        if self.slot_index(frame.pid).is_some() {
            return false;
        }
        let Some(slot) = self.free_slot() else {
            return false;
        };
        self.frames[slot] = frame;
        true
    }

    fn store_byte(&mut self, mut frame: BinUapiResumeFrame, byte: u8) -> Option<*mut u8> {
        if self.slot_index(frame.pid).is_some() {
            return None;
        }
        let slot = self.free_slot()?;
        frame.write_byte = byte;
        self.frames[slot] = frame;
        Some(&mut self.frames[slot].write_byte as *mut u8)
    }

    fn take(&mut self, pid: usize) -> Option<BinUapiResumeFrame> {
        let slot = self.slot_index(pid)?;
        let frame = self.frames[slot];
        self.frames[slot] = BinUapiResumeFrame::empty();
        Some(frame)
    }

    fn release(&mut self, pid: usize) {
        if let Some(slot) = self.slot_index(pid) {
            self.frames[slot] = BinUapiResumeFrame::empty();
        }
    }
}

struct BinUapiResumeFrameCell(UnsafeCell<BinUapiResumeFrameTable>);

// SAFETY: mutable access is serialized by `BIN_UAPI_RESUME_FRAME_LOCK`.
unsafe impl Sync for BinUapiResumeFrameCell {}

static BIN_UAPI_RESUME_FRAMES: BinUapiResumeFrameCell =
    BinUapiResumeFrameCell(UnsafeCell::new(BinUapiResumeFrameTable::new()));
static BIN_UAPI_RESUME_FRAME_LOCK: AtomicBool = AtomicBool::new(false);

struct BinUapiResumeFrameGuard;

impl BinUapiResumeFrameGuard {
    fn acquire() -> Self {
        while BIN_UAPI_RESUME_FRAME_LOCK
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }
        Self
    }
}

impl Drop for BinUapiResumeFrameGuard {
    fn drop(&mut self) {
        BIN_UAPI_RESUME_FRAME_LOCK.store(false, Ordering::Release);
    }
}

fn with_bin_uapi_resume_frames<R>(f: impl FnOnce(&mut BinUapiResumeFrameTable) -> R) -> R {
    let _guard = BinUapiResumeFrameGuard::acquire();
    // SAFETY: `BIN_UAPI_RESUME_FRAME_LOCK` serializes access.
    let frames = unsafe { &mut *BIN_UAPI_RESUME_FRAMES.0.get() };
    f(frames)
}

pub(crate) fn reset_bin_uapi_resume_frames() {
    with_bin_uapi_resume_frames(BinUapiResumeFrameTable::reset);
}

pub(crate) fn release_bin_uapi_resume_frame(pid: usize) {
    with_bin_uapi_resume_frames(|frames| frames.release(pid));
}

fn store_bin_uapi_resume_frame(
    context: SyscallContext,
    program: LoadedProgram,
    argv: ProgramArgvBuffer,
    env: ProgramEnvBuffer,
    stdin: &[u8],
    offset: usize,
    action: BinUapiResumeAction,
) -> bool {
    with_bin_uapi_resume_frames(|frames| {
        frames.store(BinUapiResumeFrame::new(context, program, argv, env, stdin, offset, action))
    })
}

fn store_bin_uapi_write_byte_frame(
    context: SyscallContext,
    program: LoadedProgram,
    argv: ProgramArgvBuffer,
    env: ProgramEnvBuffer,
    stdin: &[u8],
    offset: usize,
    byte: u8,
    action: BinUapiResumeAction,
) -> Option<&'static [u8]> {
    let ptr = with_bin_uapi_resume_frames(|frames| {
        frames.store_byte(
            BinUapiResumeFrame::new(context, program, argv, env, stdin, offset, action),
            byte,
        )
    })?;
    // SAFETY: `ptr` points into the static resume-frame table. The slot is
    // retained until the raw write succeeds, errors, or the process exits.
    Some(unsafe { core::slice::from_raw_parts(ptr as *const u8, 1) })
}

fn store_bin_uapi_read_byte_frame(
    context: SyscallContext,
    program: LoadedProgram,
    argv: ProgramArgvBuffer,
    env: ProgramEnvBuffer,
    stdin: &[u8],
    offset: usize,
    file: RawFd,
) -> Option<&'static mut [u8]> {
    let ptr = with_bin_uapi_resume_frames(|frames| {
        frames.store_byte(
            BinUapiResumeFrame::new(
                context,
                program,
                argv,
                env,
                stdin,
                offset,
                BinUapiResumeAction::ContinueFileCopyAfterRead {
                    file_fd: file.raw(),
                },
            ),
            0,
        )
    })?;
    // SAFETY: `ptr` points into the static resume-frame table. The slot is
    // retained until the raw read succeeds, errors, or the process exits.
    Some(unsafe { core::slice::from_raw_parts_mut(ptr, 1) })
}

fn take_bin_uapi_resume_frame(pid: usize) -> Option<BinUapiResumeFrame> {
    with_bin_uapi_resume_frames(|frames| frames.take(pid))
}

pub(crate) fn resume_bin_uapi_frame(
    daemon: &rootd::RootDaemon<'_>,
    session: &mut RootShellSession,
    ctx: SyscallContext,
    continuation_ret: SyscallRet,
) -> Option<ProgramStatus> {
    let frame = take_bin_uapi_resume_frame(ctx.pid)?;
    let resumed_ctx = match crate::syscall::resume_user_frame_process(frame.context) {
        Some(ctx) => ctx,
        None => return Some(ProgramStatus::Error),
    };
    let mut syscalls =
        ProgramSyscalls::new_with_stdin(daemon, session, Some(resumed_ctx), frame.stdin());
    match frame.action {
        BinUapiResumeAction::ContinueFromOffset => Some(run_loaded_program_from_offset(
            frame.program,
            &frame.argv,
            &frame.env,
            frame.stdin(),
            &mut syscalls,
            frame.offset,
        )),
        BinUapiResumeAction::ContinueFileCopyAfterRead { file_fd } => {
            let code = match continuation_ret.decode() {
                Ok(code) => code,
                _ => return Some(ProgramStatus::Error),
            };
            let status = {
                let Some(_guard) = LinkedRawSyscallGuard::enter(&mut syscalls) else {
                    return Some(ProgramStatus::Error);
                };
                resume_bin_uapi_file_copy_after_read(
                    SyscallFdControl::new(RawSyscall::new(linked_raw_syscall)),
                    resumed_ctx,
                    frame.program,
                    &frame.argv,
                    &frame.env,
                    frame.stdin(),
                    frame.offset,
                    RawFd::new(file_fd),
                    frame.write_byte,
                    code,
                )
            };
            if status == ProgramStatus::Ok {
                Some(run_loaded_program_from_offset(
                    frame.program,
                    &frame.argv,
                    &frame.env,
                    frame.stdin(),
                    &mut syscalls,
                    frame.offset,
                ))
            } else {
                Some(status)
            }
        }
        BinUapiResumeAction::ContinueFileCopyAfterWrite { file_fd } => {
            let code = match continuation_ret.decode() {
                Ok(code) => code,
                _ => return Some(ProgramStatus::Error),
            };
            if code != 1 {
                return Some(ProgramStatus::Error);
            }
            let status = {
                let Some(_guard) = LinkedRawSyscallGuard::enter(&mut syscalls) else {
                    return Some(ProgramStatus::Error);
                };
                exec_body_uapi_copy_opened_file(
                    SyscallFdControl::new(RawSyscall::new(linked_raw_syscall)),
                    Some(resumed_ctx),
                    frame.program,
                    &frame.argv,
                    &frame.env,
                    frame.stdin(),
                    frame.offset,
                    RawFd::new(file_fd),
                )
            };
            if status == ProgramStatus::Ok {
                Some(run_loaded_program_from_offset(
                    frame.program,
                    &frame.argv,
                    &frame.env,
                    frame.stdin(),
                    &mut syscalls,
                    frame.offset,
                ))
            } else {
                Some(status)
            }
        }
        BinUapiResumeAction::ExitWithWaitStatus => {
            let code = match continuation_ret.decode() {
                Ok(code) if code <= u8::MAX as usize => code as u8,
                _ => return Some(ProgramStatus::Error),
            };
            let Some(_guard) = LinkedRawSyscallGuard::enter(&mut syscalls) else {
                return Some(ProgramStatus::Error);
            };
            Some(exec_body_uapi_exit(
                SyscallProcessControl::new(RawSyscall::new(linked_raw_syscall)),
                code,
            ))
        }
    }
}

fn next_source_line(bytes: &'static [u8], offset: usize) -> Option<(&'static [u8], usize)> {
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

fn parse_source_usize(line: &[u8], offset: &mut usize) -> Option<usize> {
    let mut value = 0usize;
    let mut saw_digit = false;
    while *offset < line.len() {
        let byte = line[*offset];
        if !byte.is_ascii_digit() {
            break;
        }
        saw_digit = true;
        value = value.checked_mul(10)?;
        value = value.checked_add((byte - b'0') as usize)?;
        *offset += 1;
    }
    if saw_digit { Some(value) } else { None }
}

fn parse_source_hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn parse_source_status(value: &[u8]) -> Option<ProgramStatus> {
    match value {
        b"ok" => Some(ProgramStatus::Ok),
        b"error" => Some(ProgramStatus::Error),
        b"halt" => Some(ProgramStatus::Halt),
        _ => None,
    }
}

fn parse_source_exit_code(value: &[u8]) -> Option<i32> {
    let mut offset = 0usize;
    let code = parse_source_usize(value, &mut offset)?;
    if offset != value.len() || code > 255 {
        return None;
    }
    Some(code as i32)
}

fn source_hex_is_valid(encoded: &[u8]) -> bool {
    if encoded.len() % 2 != 0 {
        return false;
    }

    let mut index = 0usize;
    while index < encoded.len() {
        if parse_source_hex_digit(encoded[index]).is_none()
            || parse_source_hex_digit(encoded[index + 1]).is_none()
        {
            return false;
        }
        index += 2;
    }

    true
}

fn validate_reject_argc_greater(line: &[u8]) -> bool {
    let mut offset = SOURCE_BYTES_OP_REJECT_ARGC_GREATER.len();
    parse_source_usize(line, &mut offset).is_some() && offset < line.len() && line[offset] == b' '
}

fn validate_reject_argc_greater_unless_arg1(line: &[u8]) -> bool {
    let mut offset = SOURCE_BYTES_OP_REJECT_ARGC_GREATER_UNLESS_ARG1.len();
    if parse_source_usize(line, &mut offset).is_none()
        || offset >= line.len()
        || line[offset] != b' '
    {
        return false;
    }
    offset += 1;
    let allow_start = offset;
    while offset < line.len() && line[offset] != b' ' {
        offset += 1;
    }
    allow_start < offset && offset < line.len()
}

fn validate_request_shell_target(line: &[u8]) -> bool {
    let program = &line[SOURCE_BYTES_OP_REQUEST_SHELL_TARGET.len()..];
    if program.is_empty() || program.len() > MAX_PROGRAM_ARG_BYTES {
        return false;
    }
    if core::str::from_utf8(program).is_err() {
        return false;
    }
    let mut index = 0usize;
    while index < program.len() {
        if matches!(program[index], b' ' | b'\t' | b'\r' | b'\n') {
            return false;
        }
        index += 1;
    }
    true
}

fn parse_service_request(line: &'static [u8]) -> Option<(&'static str, &'static str)> {
    let rest = &line[SOURCE_BYTES_OP_REQUEST_SERVICE.len()..];
    let split = rest.iter().position(|&byte| byte == b' ')?;
    if split == 0 || split + 1 >= rest.len() {
        return None;
    }
    let name = &rest[..split];
    let target = &rest[split + 1..];
    if name.len() > MAX_PROGRAM_ARG_BYTES || target.len() > MAX_PROGRAM_ARG_BYTES {
        return None;
    }
    if target
        .iter()
        .any(|&byte| byte == b' ' || byte == b'\t' || byte == b'\r' || byte == b'\n')
    {
        return None;
    }
    let name = core::str::from_utf8(name).ok()?;
    let target = core::str::from_utf8(target).ok()?;
    Some((name, target))
}

fn parse_line_discipline_request(line: &'static [u8]) -> Option<(&'static str, &'static str)> {
    let rest = &line[SOURCE_BYTES_OP_REQUEST_LINE_DISCIPLINE.len()..];
    let split = rest.iter().position(|&byte| byte == b' ')?;
    if split == 0 || split + 1 >= rest.len() {
        return None;
    }
    let discipline = &rest[..split];
    let pipe_mode = &rest[split + 1..];
    if pipe_mode
        .iter()
        .any(|&byte| byte == b' ' || byte == b'\t' || byte == b'\r' || byte == b'\n')
    {
        return None;
    }
    if discipline != b"argv-v1" || pipe_mode != b"single-pipe" {
        return None;
    }
    let discipline = core::str::from_utf8(discipline).ok()?;
    let pipe_mode = core::str::from_utf8(pipe_mode).ok()?;
    Some((discipline, pipe_mode))
}

fn validate_source_byte_op(line: &'static [u8]) -> bool {
    if line.is_empty() {
        return true;
    }

    if line == SOURCE_BYTES_OP_WRITE_CWD_LINE
        || line == SOURCE_BYTES_OP_WRITE_VFS_LISTING_ARG1_OR_CWD
        || line == SOURCE_BYTES_OP_SET_CWD_ARG1_OR_ROOT
        || line == SOURCE_BYTES_OP_WRITE_STDIN_OR_VFS_FILES_ARGV_TAIL
        || line == SOURCE_BYTES_OP_WRITE_TTY_LINE
        || line == SOURCE_BYTES_OP_CLEAR_CONSOLE
        || line == SOURCE_BYTES_OP_WRITE_MOUNT_TABLE
        || line == SOURCE_BYTES_OP_WRITE_BOOT_INFO_SUMMARY
        || line == SOURCE_BYTES_OP_WRITE_DEVICE_INVENTORY
        || line == SOURCE_BYTES_OP_WRITE_BOOT_INPUT
        || line == SOURCE_BYTES_OP_WRITE_BOOT_STATUS
        || line == SOURCE_BYTES_OP_WRITE_BOOT_PROOF
        || line == SOURCE_BYTES_OP_REQUEST_ROOT_SHELL
        || line == SOURCE_BYTES_OP_WRITE_KERNEL_LOG_VIEW
        || line == SOURCE_BYTES_OP_WRITE_KERNEL_LOG_STATS
        || line == SOURCE_BYTES_OP_WRITE_DUMP_STATUS
        || line == SOURCE_BYTES_OP_WRITE_DUMP_SNAPSHOT
        || line == SOURCE_BYTES_OP_WRITE_DUMP_SYNC
        || line == SOURCE_BYTES_OP_WRITE_SCHEDULER_STATE
        || line == SOURCE_BYTES_OP_WRITE_SCHEDULER_TICK
        || line == SOURCE_BYTES_OP_WRITE_SCHEDULER_YIELD
        || line == SOURCE_BYTES_OP_WRITE_SCHEDULER_SLEEP_ARG2
        || line == SOURCE_BYTES_OP_WRITE_PROCESS_TABLE
        || line == SOURCE_BYTES_OP_WRITE_SESSION_STATE
        || line == SOURCE_BYTES_OP_WRITE_SERVICE_TABLE
        || line == SOURCE_BYTES_OP_WRITE_EXEC_LOAD_TABLE
        || line == SOURCE_BYTES_OP_WRITE_PENDING_EXEC_TABLE
        || line == SOURCE_BYTES_OP_WRITE_PROCESS_SELF
        || line == SOURCE_BYTES_OP_WRITE_SOURCE_STORE_TABLE
        || line == SOURCE_BYTES_OP_WRITE_SOURCE_MEDIA_TABLE
        || line == SOURCE_BYTES_OP_WRITE_TASK_TABLE
        || line == SOURCE_BYTES_OP_WRITE_WAIT_TABLE
        || line == SOURCE_BYTES_OP_WRITE_SYSCALL_TABLE
        || line == SOURCE_BYTES_OP_LAUNCH_PAYLOAD_ARG1_OR_LIST
        || line == SOURCE_BYTES_OP_WRITE_HELP_ARG1_OR_CATALOG
    {
        return true;
    }

    if line.starts_with(SOURCE_BYTES_OP_REQUIRE_LAUNCH_ENABLED) {
        return true;
    }

    if line.starts_with(SOURCE_BYTES_OP_LAUNCH_PAYLOAD_NAME) {
        return core::str::from_utf8(&line[SOURCE_BYTES_OP_LAUNCH_PAYLOAD_NAME.len()..]).is_ok();
    }

    if line.starts_with(SOURCE_BYTES_OP_REQUEST_SHELL_TARGET) {
        return validate_request_shell_target(line);
    }

    if line.starts_with(SOURCE_BYTES_OP_REQUEST_SERVICE) {
        return parse_service_request(line).is_some();
    }

    if line.starts_with(SOURCE_BYTES_OP_REQUEST_LINE_DISCIPLINE) {
        return parse_line_discipline_request(line).is_some();
    }

    if line.starts_with(SOURCE_BYTES_OP_WRITE_STDOUT_HEX) {
        return source_hex_is_valid(&line[SOURCE_BYTES_OP_WRITE_STDOUT_HEX.len()..]);
    }

    if line.starts_with(SOURCE_BYTES_OP_EXIT_STATUS) {
        return parse_source_status(&line[SOURCE_BYTES_OP_EXIT_STATUS.len()..]).is_some();
    }

    if line.starts_with(SOURCE_BYTES_OP_EXIT_CODE) {
        return parse_source_exit_code(&line[SOURCE_BYTES_OP_EXIT_CODE.len()..]).is_some();
    }

    if line.starts_with(SOURCE_BYTES_OP_REJECT_ARGC_GREATER) {
        return validate_reject_argc_greater(line);
    }

    if line.starts_with(SOURCE_BYTES_OP_REJECT_ARGC_GREATER_UNLESS_ARG1) {
        return validate_reject_argc_greater_unless_arg1(line);
    }

    false
}

fn validate_source_arg1_dispatch(bytes: &'static [u8], mut offset: usize) -> Option<usize> {
    let mut scanned = 0usize;

    while let Some((line, next)) = next_source_line(bytes, offset) {
        scanned += 1;
        if scanned > SOURCE_BYTES_MAX_OPS {
            return None;
        }

        if line == SOURCE_BYTES_END_DISPATCH_ARG1 {
            return Some(next);
        }

        if line == SOURCE_BYTES_DISPATCH_DEFAULT || line.starts_with(SOURCE_BYTES_DISPATCH_CASE) {
            offset = next;
            continue;
        }

        if !validate_source_byte_op(line) {
            return None;
        }

        offset = next;
    }

    None
}

fn validate_source_bytes(bytes: &'static [u8]) -> bool {
    let Some((header, mut offset)) = next_source_line(bytes, 0) else {
        return false;
    };
    if header != SOURCE_BYTES_MAGIC {
        return false;
    }

    let mut ops = 0usize;
    while let Some((line, next)) = next_source_line(bytes, offset) {
        ops += 1;
        if ops > SOURCE_BYTES_MAX_OPS {
            return false;
        }
        if line.starts_with(SOURCE_BYTES_OP_DISPATCH_ARG1) {
            let Some(next) = validate_source_arg1_dispatch(bytes, next) else {
                return false;
            };
            offset = next;
            continue;
        }
        if !validate_source_byte_op(line) {
            return false;
        }
        offset = next;
    }

    true
}

fn validate_bin_exec_body(body: exec_body::ExecBody<'static>) -> bool {
    match body.inner {
        ExecBodyInnerFormat::BinSourceImage => validate_source_bytes(body.bytes),
        ExecBodyInnerFormat::BinUapiV1 => validate_bin_uapi_body(body.bytes),
        ExecBodyInnerFormat::PayloadSourceImage => false,
    }
}

fn next_bin_uapi_line(bytes: &[u8], offset: usize) -> Option<(&[u8], usize)> {
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

fn validate_bin_uapi_body_op(line: &[u8]) -> bool {
    if line.is_empty() {
        return true;
    }
    if line.starts_with(BIN_UAPI_OP_OPEN_READONLY_WRITE_STDOUT_ENV_OR_ARG1_OR) {
        let Some((name, path)) = split_bin_uapi_env_or(
            &line[BIN_UAPI_OP_OPEN_READONLY_WRITE_STDOUT_ENV_OR_ARG1_OR.len()..],
        ) else {
            return false;
        };
        return program_env_name_is_valid(name) && validate_bin_uapi_path(path);
    }
    if line.starts_with(BIN_UAPI_OP_OPEN_READONLY_WRITE_STDOUT_ARG1_OR) {
        return validate_bin_uapi_path(
            &line[BIN_UAPI_OP_OPEN_READONLY_WRITE_STDOUT_ARG1_OR.len()..],
        );
    }
    if line.starts_with(BIN_UAPI_OP_OPEN_READONLY_WRITE_STDOUT) {
        return validate_bin_uapi_path(&line[BIN_UAPI_OP_OPEN_READONLY_WRITE_STDOUT.len()..]);
    }
    if line.starts_with(BIN_UAPI_OP_EXEC_BIN_ENV) {
        return parse_bin_uapi_process_env_and_args(&line[BIN_UAPI_OP_EXEC_BIN_ENV.len()..])
            .is_some();
    }
    if line.starts_with(BIN_UAPI_OP_SLEEP_TICKS) {
        return parse_bin_uapi_ticks(&line[BIN_UAPI_OP_SLEEP_TICKS.len()..]).is_some();
    }
    if line.starts_with(BIN_UAPI_OP_SPAWN_SLEEP_BIN_ENV) {
        return parse_bin_uapi_sleep_ticks_env_and_args(
            &line[BIN_UAPI_OP_SPAWN_SLEEP_BIN_ENV.len()..],
        )
        .is_some();
    }
    if line.starts_with(BIN_UAPI_OP_SPAWN_BIN_ENV) {
        return parse_bin_uapi_process_env_and_args(&line[BIN_UAPI_OP_SPAWN_BIN_ENV.len()..])
            .is_some();
    }
    if line.starts_with(BIN_UAPI_OP_SPAWN_WAIT_BIN_ENV) {
        return parse_bin_uapi_process_env_and_args(&line[BIN_UAPI_OP_SPAWN_WAIT_BIN_ENV.len()..])
            .is_some();
    }
    if line.starts_with(BIN_UAPI_OP_SPAWN_WAIT_BIN) {
        return parse_bin_uapi_process_args(&line[BIN_UAPI_OP_SPAWN_WAIT_BIN.len()..]).is_some();
    }
    if line == BIN_UAPI_OP_SCHEDULER_TICK {
        return true;
    }
    if line == BIN_UAPI_OP_YIELD_NOW {
        return true;
    }
    if line.starts_with(BIN_UAPI_OP_WRITE_STDOUT_HEX) {
        return source_hex_is_valid(&line[BIN_UAPI_OP_WRITE_STDOUT_HEX.len()..]);
    }
    if line.starts_with(BIN_UAPI_OP_EXIT_STATUS) {
        return matches!(&line[BIN_UAPI_OP_EXIT_STATUS.len()..], b"ok" | b"error");
    }
    if line.starts_with(BIN_UAPI_OP_EXIT_CODE) {
        return parse_source_exit_code(&line[BIN_UAPI_OP_EXIT_CODE.len()..]).is_some();
    }
    false
}

fn validate_bin_uapi_path(path: &[u8]) -> bool {
    if path.is_empty()
        || path.len() > BIN_UAPI_MAX_PATH_BYTES
        || core::str::from_utf8(path).is_err()
    {
        return false;
    }

    let mut index = 0usize;
    while index < path.len() {
        if matches!(path[index], b' ' | b'\t' | b'\r' | b'\n') {
            return false;
        }
        index += 1;
    }

    true
}

fn parse_bin_uapi_process_args(bytes: &[u8]) -> Option<([ProcessArg; MAX_PROGRAM_ARGS], usize)> {
    let empty = ProcessArg::from_str("");
    let mut args = [empty; MAX_PROGRAM_ARGS];
    let mut argc = 0usize;
    let mut offset = 0usize;

    while offset < bytes.len() {
        while offset < bytes.len() && bytes[offset] == b' ' {
            offset += 1;
        }
        if offset >= bytes.len() {
            break;
        }
        if argc >= MAX_PROGRAM_ARGS {
            return None;
        }

        let start = offset;
        while offset < bytes.len() && bytes[offset] != b' ' {
            if matches!(bytes[offset], b'\t' | b'\r' | b'\n') {
                return None;
            }
            offset += 1;
        }
        let len = offset - start;
        if len == 0 || len > MAX_PROGRAM_ARG_BYTES {
            return None;
        }
        let text = core::str::from_utf8(&bytes[start..offset]).ok()?;
        args[argc] = ProcessArg::from_str(text);
        argc += 1;
    }

    if argc == 0 { None } else { Some((args, argc)) }
}

fn take_bin_uapi_token(bytes: &[u8]) -> Option<(&[u8], &[u8])> {
    let mut offset = 0usize;
    while offset < bytes.len() && bytes[offset] == b' ' {
        offset += 1;
    }
    if offset >= bytes.len() {
        return None;
    }

    let start = offset;
    while offset < bytes.len() && bytes[offset] != b' ' {
        if matches!(bytes[offset], b'\t' | b'\r' | b'\n') {
            return None;
        }
        offset += 1;
    }
    if offset == start {
        return None;
    }
    Some((&bytes[start..offset], &bytes[offset..]))
}

fn parse_bin_uapi_ticks(bytes: &[u8]) -> Option<usize> {
    let ticks = parse_source_usize_bytes(bytes)?;
    if ticks == 0 || ticks > sched::MAX_KERNEL_TASKS {
        return None;
    }
    Some(ticks)
}

fn parse_bin_uapi_process_env_and_args(
    bytes: &[u8],
) -> Option<(ProcessEnv, [ProcessArg; MAX_PROGRAM_ARGS], usize)> {
    let (name, rest) = take_bin_uapi_token(bytes)?;
    if !program_env_name_is_valid(name) {
        return None;
    }
    let (value, argv_bytes) = take_bin_uapi_token(rest)?;
    if value.len() > MAX_PROGRAM_ENV_VALUE_BYTES {
        return None;
    }

    let name = core::str::from_utf8(name).ok()?;
    let value = core::str::from_utf8(value).ok()?;
    let (args, argc) = parse_bin_uapi_process_args(argv_bytes)?;
    Some((ProcessEnv::from_pair(name, value), args, argc))
}

fn parse_bin_uapi_sleep_ticks_env_and_args(
    bytes: &[u8],
) -> Option<(usize, ProcessEnv, [ProcessArg; MAX_PROGRAM_ARGS], usize)> {
    let (ticks, rest) = take_bin_uapi_token(bytes)?;
    let ticks = parse_source_usize_bytes(ticks)?;
    if ticks == 0 || ticks > sched::MAX_KERNEL_TASKS {
        return None;
    }
    let (env, args, argc) = parse_bin_uapi_process_env_and_args(rest)?;
    Some((ticks, env, args, argc))
}

fn split_bin_uapi_env_or(line: &[u8]) -> Option<(&[u8], &[u8])> {
    let mut split = 0usize;
    while split < line.len() && line[split] != b' ' && line[split] != b'\t' {
        split += 1;
    }
    if split == 0 || split >= line.len() {
        return None;
    }
    let mut path = split;
    while path < line.len() && matches!(line[path], b' ' | b'\t') {
        path += 1;
    }
    if path >= line.len() {
        return None;
    }
    Some((&line[..split], &line[path..]))
}

fn validate_bin_uapi_body(bytes: &[u8]) -> bool {
    let Some((header, mut offset)) = next_bin_uapi_line(bytes, 0) else {
        return false;
    };
    if header != BIN_UAPI_BODY_MAGIC {
        return false;
    }

    let mut ops = 0usize;
    while let Some((line, next)) = next_bin_uapi_line(bytes, offset) {
        ops += 1;
        if ops > BIN_UAPI_MAX_OPS || !validate_bin_uapi_body_op(line) {
            return false;
        }
        offset = next;
    }

    true
}

fn parse_source_usize_bytes(bytes: &[u8]) -> Option<usize> {
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

fn source_arg1_matches_csv(argv: &ProgramArgv<'_>, csv: &[u8]) -> bool {
    let Some(arg1) = argv.arg(1) else {
        return false;
    };
    let arg1 = arg1.as_bytes();
    let mut start = 0usize;
    let mut index = 0usize;
    while index <= csv.len() {
        if index == csv.len() || csv[index] == b',' {
            if &csv[start..index] == arg1 {
                return true;
            }
            start = index + 1;
        }
        index += 1;
    }
    false
}

fn write_source_stdout_hex(
    encoded: &[u8],
    syscalls: &mut ProgramSyscalls<'_, '_, '_>,
) -> Option<()> {
    if encoded.len() % 2 != 0 {
        return None;
    }

    let mut index = 0usize;
    while index < encoded.len() {
        let high = parse_source_hex_digit(encoded[index])?;
        let low = parse_source_hex_digit(encoded[index + 1])?;
        let byte = (high << 4) | low;
        syscalls.stdout_bytes(&[byte]);
        index += 2;
    }

    Some(())
}

fn source_write_u64_dec(syscalls: &mut ProgramSyscalls<'_, '_, '_>, mut value: u64) {
    let mut buf = [0u8; 20];
    let mut len = 0usize;
    if value == 0 {
        syscalls.stdout_bytes(b"0");
        return;
    }
    while value > 0 && len < buf.len() {
        buf[len] = b'0' + (value % 10) as u8;
        value /= 10;
        len += 1;
    }
    while len > 0 {
        len -= 1;
        syscalls.stdout_bytes(&buf[len..len + 1]);
    }
}

fn run_source_scheduler_sleep_arg2(
    argv: &ProgramArgv<'_>,
    syscalls: &mut ProgramSyscalls<'_, '_, '_>,
) -> ProgramStatus {
    let Some(ticks_text) = argv.arg(2) else {
        syscalls.stderr_line("sched sleep: missing ticks");
        return ProgramStatus::Error;
    };
    if argv.argc() > 3 {
        syscalls.stderr_line("sched: too many arguments");
        return ProgramStatus::Error;
    }

    let Some(ticks) = parse_source_usize_bytes(ticks_text.as_bytes()) else {
        syscalls.stderr_line("sched sleep: invalid ticks");
        return ProgramStatus::Error;
    };
    if ticks == 0 || ticks > sched::MAX_KERNEL_TASKS {
        syscalls.stderr_line("sched sleep: invalid ticks");
        return ProgramStatus::Error;
    }

    let result = syscalls.sleep_current_for_ticks_result(ticks);
    syscalls.stdout_line("sched sleep:");
    syscalls.stdout_bytes(b"slept=");
    syscalls.stdout_bytes(if result.slept { b"true" } else { b"false" });
    syscalls.stdout_bytes(b"\nstatus=");
    syscalls.stdout_bytes(result.status.as_str().as_bytes());
    syscalls.stdout_bytes(b"\npid=");
    source_write_u64_dec(syscalls, result.pid as u64);
    syscalls.stdout_bytes(b"\ntask=");
    source_write_u64_dec(syscalls, result.task_id as u64);
    syscalls.stdout_bytes(b"\nwake_tick=");
    source_write_u64_dec(syscalls, result.wake_tick as u64);
    syscalls.stdout_bytes(b"\ntick_count=");
    source_write_u64_dec(syscalls, result.tick_count as u64);
    syscalls.stdout_bytes(b"\ndispatched=");
    source_write_u64_dec(syscalls, result.dispatched_count as u64);
    syscalls.stdout_bytes(b"\nwoken=");
    source_write_u64_dec(syscalls, result.woken_count as u64);
    syscalls.stdout_bytes(b"\n");

    if result.status == crate::syscall::SchedulerSleepStatus::Ok {
        ProgramStatus::Ok
    } else {
        ProgramStatus::Error
    }
}

fn run_source_byte_op(
    line: &'static [u8],
    argv: &ProgramArgv<'_>,
    syscalls: &mut ProgramSyscalls<'_, '_, '_>,
) -> Option<ProgramStatus> {
    if line.is_empty() {
        return Some(ProgramStatus::Ok);
    }

    if line == SOURCE_BYTES_OP_WRITE_CWD_LINE {
        syscalls.stdout_line(syscalls.cwd());
        return Some(ProgramStatus::Ok);
    }

    if line == SOURCE_BYTES_OP_WRITE_VFS_LISTING_ARG1_OR_CWD {
        return Some(run_source_vfs_listing_arg1_or_cwd(argv, syscalls));
    }

    if line == SOURCE_BYTES_OP_SET_CWD_ARG1_OR_ROOT {
        return Some(run_source_set_cwd_arg1_or_root(argv, syscalls));
    }

    if line == SOURCE_BYTES_OP_WRITE_STDIN_OR_VFS_FILES_ARGV_TAIL {
        return Some(run_source_stdin_or_vfs_files_argv_tail(argv, syscalls));
    }

    if line == SOURCE_BYTES_OP_WRITE_TTY_LINE {
        return Some(run_source_write_tty_line(syscalls));
    }

    if line == SOURCE_BYTES_OP_CLEAR_CONSOLE {
        syscalls.clear_console();
        return Some(ProgramStatus::Ok);
    }

    if line == SOURCE_BYTES_OP_WRITE_MOUNT_TABLE {
        syscalls.write_mount_table();
        return Some(ProgramStatus::Ok);
    }

    if line == SOURCE_BYTES_OP_WRITE_BOOT_INFO_SUMMARY {
        syscalls.write_boot_info_summary();
        return Some(ProgramStatus::Ok);
    }

    if line == SOURCE_BYTES_OP_WRITE_DEVICE_INVENTORY {
        syscalls.write_device_inventory();
        return Some(ProgramStatus::Ok);
    }

    if line == SOURCE_BYTES_OP_WRITE_BOOT_INPUT {
        syscalls.write_boot_input();
        return Some(ProgramStatus::Ok);
    }

    if line == SOURCE_BYTES_OP_WRITE_BOOT_STATUS {
        syscalls.write_boot_status();
        return Some(ProgramStatus::Ok);
    }

    if line == SOURCE_BYTES_OP_WRITE_BOOT_PROOF {
        syscalls.write_boot_proof();
        return Some(ProgramStatus::Ok);
    }

    if line.starts_with(SOURCE_BYTES_OP_REQUEST_SHELL_TARGET) {
        let program =
            core::str::from_utf8(&line[SOURCE_BYTES_OP_REQUEST_SHELL_TARGET.len()..]).ok()?;
        syscalls.request_shell_target(program);
        return Some(ProgramStatus::Ok);
    }

    if line.starts_with(SOURCE_BYTES_OP_REQUEST_SERVICE) {
        let (name, target) = parse_service_request(line)?;
        return Some(match syscalls.request_init_service(name, target) {
            Ok(_) => ProgramStatus::Ok,
            Err(_) => ProgramStatus::Error,
        });
    }

    if line.starts_with(SOURCE_BYTES_OP_REQUEST_LINE_DISCIPLINE) {
        let (line_discipline, pipe_mode) = parse_line_discipline_request(line)?;
        syscalls.request_shell_line_discipline(line_discipline, pipe_mode);
        return Some(ProgramStatus::Ok);
    }

    if line == SOURCE_BYTES_OP_REQUEST_ROOT_SHELL {
        syscalls.request_shell_start();
        return Some(ProgramStatus::Ok);
    }

    if line == SOURCE_BYTES_OP_WRITE_KERNEL_LOG_VIEW {
        syscalls.write_kernel_log_view();
        return Some(ProgramStatus::Ok);
    }

    if line == SOURCE_BYTES_OP_WRITE_KERNEL_LOG_STATS {
        syscalls.write_kernel_log_stats();
        return Some(ProgramStatus::Ok);
    }

    if line == SOURCE_BYTES_OP_WRITE_DUMP_STATUS {
        syscalls.write_dump_status();
        return Some(ProgramStatus::Ok);
    }

    if line == SOURCE_BYTES_OP_WRITE_DUMP_SNAPSHOT {
        syscalls.write_dump_snapshot();
        return Some(ProgramStatus::Ok);
    }

    if line == SOURCE_BYTES_OP_WRITE_DUMP_SYNC {
        syscalls.write_dump_sync();
        return Some(ProgramStatus::Ok);
    }

    if line == SOURCE_BYTES_OP_WRITE_SCHEDULER_STATE {
        syscalls.write_scheduler_state();
        return Some(ProgramStatus::Ok);
    }

    if line == SOURCE_BYTES_OP_WRITE_SCHEDULER_TICK {
        syscalls.write_scheduler_tick();
        return Some(ProgramStatus::Ok);
    }

    if line == SOURCE_BYTES_OP_WRITE_SCHEDULER_YIELD {
        let result = syscalls.yield_now_result();
        syscalls.stdout_bytes(b"sched yield:\nyielded=");
        if result.yielded {
            syscalls.stdout_bytes(b"true");
        } else {
            syscalls.stdout_bytes(b"false");
        }
        syscalls.stdout_bytes(b"\nstatus=");
        syscalls.stdout_bytes(result.status.as_str().as_bytes());
        syscalls.stdout_bytes(b"\nselected_pid=");
        source_write_u64_dec(syscalls, result.selected_pid as u64);
        syscalls.stdout_bytes(b"\nselected_task=");
        source_write_u64_dec(syscalls, result.selected_task_id as u64);
        syscalls.stdout_bytes(b"\n");
        return Some(ProgramStatus::Ok);
    }

    if line == SOURCE_BYTES_OP_WRITE_SCHEDULER_SLEEP_ARG2 {
        return Some(run_source_scheduler_sleep_arg2(argv, syscalls));
    }

    if line == SOURCE_BYTES_OP_WRITE_PROCESS_TABLE {
        syscalls.write_process_table();
        return Some(ProgramStatus::Ok);
    }

    if line == SOURCE_BYTES_OP_WRITE_SESSION_STATE {
        syscalls.write_session_state();
        return Some(ProgramStatus::Ok);
    }

    if line == SOURCE_BYTES_OP_WRITE_SERVICE_TABLE {
        syscalls.write_service_table();
        return Some(ProgramStatus::Ok);
    }

    if line == SOURCE_BYTES_OP_WRITE_EXEC_LOAD_TABLE {
        syscalls.write_exec_load_table();
        return Some(ProgramStatus::Ok);
    }

    if line == SOURCE_BYTES_OP_WRITE_PENDING_EXEC_TABLE {
        syscalls.write_pending_exec_table();
        return Some(ProgramStatus::Ok);
    }

    if line == SOURCE_BYTES_OP_WRITE_PROCESS_SELF {
        syscalls.write_current_process();
        return Some(ProgramStatus::Ok);
    }

    if line == SOURCE_BYTES_OP_WRITE_SOURCE_STORE_TABLE {
        syscalls.write_source_store_table();
        return Some(ProgramStatus::Ok);
    }

    if line == SOURCE_BYTES_OP_WRITE_SOURCE_MEDIA_TABLE {
        syscalls.write_source_media_table();
        return Some(ProgramStatus::Ok);
    }

    if line == SOURCE_BYTES_OP_WRITE_TASK_TABLE {
        syscalls.write_task_table();
        return Some(ProgramStatus::Ok);
    }

    if line == SOURCE_BYTES_OP_WRITE_WAIT_TABLE {
        syscalls.write_wait_table();
        return Some(ProgramStatus::Ok);
    }

    if line == SOURCE_BYTES_OP_WRITE_SYSCALL_TABLE {
        syscalls.write_syscall_table();
        return Some(ProgramStatus::Ok);
    }

    if line == SOURCE_BYTES_OP_LAUNCH_PAYLOAD_ARG1_OR_LIST {
        return Some(run_source_launch_payload_arg1_or_list(argv, syscalls));
    }

    if line == SOURCE_BYTES_OP_WRITE_HELP_ARG1_OR_CATALOG {
        return Some(syscalls.write_program_help(argv.arg(1)));
    }

    if line.starts_with(SOURCE_BYTES_OP_REQUIRE_LAUNCH_ENABLED) {
        let message = &line[SOURCE_BYTES_OP_REQUIRE_LAUNCH_ENABLED.len()..];
        if !syscalls.launch_enabled() {
            syscalls.stderr_bytes(message);
            syscalls.stderr_bytes(b"\n");
            return Some(ProgramStatus::Error);
        }
        return Some(ProgramStatus::Ok);
    }

    if line.starts_with(SOURCE_BYTES_OP_LAUNCH_PAYLOAD_NAME) {
        return Some(run_source_launch_payload_name(
            &line[SOURCE_BYTES_OP_LAUNCH_PAYLOAD_NAME.len()..],
            argv,
            syscalls,
        )?);
    }

    if line.starts_with(SOURCE_BYTES_OP_WRITE_STDOUT_HEX) {
        write_source_stdout_hex(&line[SOURCE_BYTES_OP_WRITE_STDOUT_HEX.len()..], syscalls)?;
        return Some(ProgramStatus::Ok);
    }

    if line.starts_with(SOURCE_BYTES_OP_EXIT_STATUS) {
        return parse_source_status(&line[SOURCE_BYTES_OP_EXIT_STATUS.len()..]);
    }

    if line.starts_with(SOURCE_BYTES_OP_EXIT_CODE) {
        return Some(ProgramStatus::ExitCode(parse_source_exit_code(
            &line[SOURCE_BYTES_OP_EXIT_CODE.len()..],
        )?));
    }

    if line.starts_with(SOURCE_BYTES_OP_REJECT_ARGC_GREATER) {
        let mut offset = SOURCE_BYTES_OP_REJECT_ARGC_GREATER.len();
        let max_argc = parse_source_usize(line, &mut offset)?;
        if offset >= line.len() || line[offset] != b' ' {
            return None;
        }
        let message = &line[offset + 1..];
        if argv.argc() > max_argc {
            syscalls.stderr_bytes(message);
            syscalls.stderr_bytes(b"\n");
            return Some(ProgramStatus::Error);
        }
        return Some(ProgramStatus::Ok);
    }

    if line.starts_with(SOURCE_BYTES_OP_REJECT_ARGC_GREATER_UNLESS_ARG1) {
        let mut offset = SOURCE_BYTES_OP_REJECT_ARGC_GREATER_UNLESS_ARG1.len();
        let max_argc = parse_source_usize(line, &mut offset)?;
        if offset >= line.len() || line[offset] != b' ' {
            return None;
        }
        offset += 1;
        let allow_start = offset;
        while offset < line.len() && line[offset] != b' ' {
            offset += 1;
        }
        if offset >= line.len() {
            return None;
        }
        let allowed_arg1 = &line[allow_start..offset];
        let message = &line[offset + 1..];
        if argv.argc() > max_argc && !source_arg1_matches_csv(argv, allowed_arg1) {
            syscalls.stderr_bytes(message);
            syscalls.stderr_bytes(b"\n");
            return Some(ProgramStatus::Error);
        }
        return Some(ProgramStatus::Ok);
    }

    None
}

fn run_source_vfs_listing_arg1_or_cwd(
    argv: &ProgramArgv<'_>,
    syscalls: &mut ProgramSyscalls<'_, '_, '_>,
) -> ProgramStatus {
    let target = if argv.argc() == 1 {
        syscalls.cwd()
    } else {
        argv.arg(1).unwrap_or("")
    };
    match syscalls.write_vfs_listing(target) {
        Ok(()) => ProgramStatus::Ok,
        Err(error) => {
            write_source_path_error(syscalls, "ls", target, error);
            ProgramStatus::Error
        }
    }
}

fn run_source_set_cwd_arg1_or_root(
    argv: &ProgramArgv<'_>,
    syscalls: &mut ProgramSyscalls<'_, '_, '_>,
) -> ProgramStatus {
    let target = if argv.argc() == 1 {
        "/"
    } else {
        argv.arg(1).unwrap_or("/")
    };
    let path = match syscalls.normalize_path(target) {
        Ok(path) => path,
        Err(error) => {
            write_source_path_error(syscalls, "cd", target, error);
            return ProgramStatus::Error;
        }
    };
    match syscalls.lookup_path(path.as_str()) {
        Ok(crate::vfs::Node::Directory(_)) => {
            syscalls.set_cwd(path);
            ProgramStatus::Ok
        }
        Ok(crate::vfs::Node::File(_)) => {
            write_source_path_error(syscalls, "cd", target, crate::vfs::VfsError::NotDirectory);
            ProgramStatus::Error
        }
        Err(error) => {
            write_source_path_error(syscalls, "cd", target, error);
            ProgramStatus::Error
        }
    }
}

fn run_source_stdin_or_vfs_files_argv_tail(
    argv: &ProgramArgv<'_>,
    syscalls: &mut ProgramSyscalls<'_, '_, '_>,
) -> ProgramStatus {
    if argv.argc() == 1 {
        return run_source_copy_stdin_to_stdout(syscalls);
    }

    let mut status = ProgramStatus::Ok;
    let mut index = 1usize;
    while index < argv.argc() {
        let target = argv.arg(index).unwrap_or("");
        if let Err(error) = run_source_copy_vfs_file_to_stdout(target, syscalls) {
            write_source_path_error(syscalls, "cat", target, error);
            status = ProgramStatus::Error;
        }
        index += 1;
    }
    status
}

fn run_source_copy_vfs_file_to_stdout(
    target: &str,
    syscalls: &mut ProgramSyscalls<'_, '_, '_>,
) -> Result<(), crate::vfs::VfsError> {
    let fd = syscalls.open_vfs_file_path(target)?;
    let mut buffer = [0u8; 64];
    loop {
        let read = match syscalls.read_fd(fd, &mut buffer) {
            Ok(read) => read,
            Err(_) => {
                let _ = syscalls.close_fd(fd);
                return Err(crate::vfs::VfsError::Io);
            }
        };
        if read == 0 {
            let _ = syscalls.close_fd(fd);
            return Ok(());
        }
        if syscalls
            .write_fd(syscalls.stdout().fd, &buffer[..read])
            .is_err()
        {
            let _ = syscalls.close_fd(fd);
            return Err(crate::vfs::VfsError::Io);
        }
    }
}

fn run_source_copy_stdin_to_stdout(syscalls: &mut ProgramSyscalls<'_, '_, '_>) -> ProgramStatus {
    let mut buffer = [0u8; 64];
    loop {
        let read = match syscalls.read_fd(syscalls.stdin().fd, &mut buffer) {
            Ok(read) => read,
            Err(_) => {
                syscalls.stderr_line("cat: stdin read failed");
                return ProgramStatus::Error;
            }
        };
        if read == 0 {
            return ProgramStatus::Ok;
        }
        if syscalls
            .write_fd(syscalls.stdout().fd, &buffer[..read])
            .is_err()
        {
            syscalls.stderr_line("cat: stdout write failed");
            return ProgramStatus::Error;
        }
    }
}

fn run_source_write_tty_line(syscalls: &mut ProgramSyscalls<'_, '_, '_>) -> ProgramStatus {
    let mut line = [0u8; crate::rootd::ROOT_LINE_BYTES];
    let len = syscalls.read_tty_line(&mut line);
    if len == 0 {
        syscalls.stderr_line("read: input eof");
        return ProgramStatus::Error;
    }
    syscalls.stdout_bytes(&line[..len]);
    syscalls.stdout_bytes(b"\n");
    ProgramStatus::Ok
}

fn run_source_launch_payload_arg1_or_list(
    argv: &ProgramArgv<'_>,
    syscalls: &mut ProgramSyscalls<'_, '_, '_>,
) -> ProgramStatus {
    if argv.argc() == 1 {
        write_source_payload_list(syscalls);
        return ProgramStatus::Ok;
    }

    let payload = argv.arg(1).unwrap_or("");
    if payload.is_empty() {
        syscalls.stderr_line("launch: no payload name");
        return ProgramStatus::Error;
    }

    let Some(payload_argv) = payload_argv_from_name_and_tail(payload, argv, 2) else {
        syscalls.stderr_line("launch: invalid payload argv");
        return ProgramStatus::Error;
    };
    let result = syscalls.launch_payload_argv(payload_argv);
    syscalls.stdout_bytes(b"launch ");
    syscalls.stdout_bytes(payload.as_bytes());
    syscalls.stdout_bytes(b": ");
    write_source_payload_status(syscalls, result);
    payload_result_status(result)
}

fn run_source_launch_payload_name(
    payload_name: &[u8],
    argv: &ProgramArgv<'_>,
    syscalls: &mut ProgramSyscalls<'_, '_, '_>,
) -> Option<ProgramStatus> {
    let Ok(payload_name) = core::str::from_utf8(payload_name) else {
        return None;
    };
    let Some(payload_argv) = payload_argv_from_name_and_tail(payload_name, argv, 1) else {
        syscalls.stderr_line("payload launch: invalid argv");
        return Some(ProgramStatus::Error);
    };
    let result = syscalls.launch_payload_argv(payload_argv);
    write_source_payload_status(syscalls, result);
    Some(payload_result_status(result))
}

fn payload_argv_from_name_and_tail(
    name: &str,
    argv: &ProgramArgv<'_>,
    tail_start: usize,
) -> Option<ProgramArgvBuffer> {
    let mut payload_argv = ProgramArgvBuffer::empty();
    payload_argv.push(name).ok()?;
    let mut index = tail_start;
    while index < argv.argc() {
        payload_argv.push(argv.arg(index)?).ok()?;
        index += 1;
    }
    Some(payload_argv)
}

fn write_source_payload_list(syscalls: &ProgramSyscalls<'_, '_, '_>) {
    let payloads = syscalls.payloads();
    let mut media_payloads = [None; rootd::MAX_MEDIA_PAYLOADS];
    let media_payload_count = rootd::snapshot_media_payloads(&mut media_payloads);
    if payloads.is_empty() && media_payload_count == 0 {
        syscalls.stdout_line("launch: no payloads registered");
        return;
    }

    syscalls.stdout_line("launch: available payloads:");
    let mut index = 0usize;
    while index < payloads.len() {
        syscalls.stdout_bytes(b"  ");
        syscalls.stdout_bytes(payloads[index].name.as_bytes());
        syscalls.stdout_bytes(b": ");
        syscalls.stdout_bytes(payloads[index].summary.as_bytes());
        syscalls.stdout_bytes(b" loader=");
        syscalls.stdout_bytes(payloads[index].image_kind().as_str().as_bytes());
        syscalls.stdout_bytes(b" entry_fn=");
        syscalls.stdout_bytes(payloads[index].entry_name.as_bytes());
        syscalls.stdout_bytes(b"\n");
        index += 1;
    }
    index = 0;
    while index < media_payload_count {
        if let Some(payload) = media_payloads[index] {
            syscalls.stdout_bytes(b"  ");
            syscalls.stdout_bytes(payload.name.as_bytes());
            syscalls.stdout_bytes(b": ");
            syscalls.stdout_bytes(payload.summary.as_bytes());
            syscalls.stdout_bytes(b" loader=");
            syscalls.stdout_bytes(payload.image_kind().as_str().as_bytes());
            syscalls.stdout_bytes(b" entry_fn=");
            syscalls.stdout_bytes(payload.entry_name.as_bytes());
            syscalls.stdout_bytes(b"\n");
        }
        index += 1;
    }
}

fn write_source_payload_status(
    syscalls: &ProgramSyscalls<'_, '_, '_>,
    result: crate::rootd::PayloadLaunchResult,
) {
    match result {
        crate::rootd::PayloadLaunchResult::Ready => syscalls.stdout_bytes(b"payload.ready"),
        crate::rootd::PayloadLaunchResult::Resident => syscalls.stdout_bytes(b"payload.resident"),
        crate::rootd::PayloadLaunchResult::NotConfigured => {
            syscalls.stdout_bytes(b"payload.not_configured")
        }
        crate::rootd::PayloadLaunchResult::Failed => syscalls.stdout_bytes(b"payload.failed"),
        crate::rootd::PayloadLaunchResult::ExitCode(code) => {
            syscalls.stdout_bytes(b"payload.exit_code(");
            write_source_u32_dec(syscalls, code as u32);
            syscalls.stdout_bytes(b")");
        }
    }
    syscalls.stdout_bytes(b"\n");
}

fn payload_result_status(result: crate::rootd::PayloadLaunchResult) -> ProgramStatus {
    match result {
        crate::rootd::PayloadLaunchResult::Ready | crate::rootd::PayloadLaunchResult::Resident => {
            ProgramStatus::Ok
        }
        crate::rootd::PayloadLaunchResult::NotConfigured
        | crate::rootd::PayloadLaunchResult::Failed => ProgramStatus::Error,
        crate::rootd::PayloadLaunchResult::ExitCode(code) => ProgramStatus::ExitCode(code as i32),
    }
}

fn write_source_u32_dec(syscalls: &ProgramSyscalls<'_, '_, '_>, mut value: u32) {
    let mut buf = [0u8; 10];
    let mut len = 0usize;
    loop {
        buf[len] = b'0' + (value % 10) as u8;
        len += 1;
        value /= 10;
        if value == 0 {
            break;
        }
    }

    while len > 0 {
        len -= 1;
        syscalls.stdout_bytes(&buf[len..len + 1]);
    }
}

fn write_source_path_error(
    syscalls: &ProgramSyscalls<'_, '_, '_>,
    program_name: &str,
    path: &str,
    error: crate::vfs::VfsError,
) {
    syscalls.stderr_bytes(program_name.as_bytes());
    syscalls.stderr_bytes(b": ");
    if !path.is_empty() {
        syscalls.stderr_bytes(path.as_bytes());
        syscalls.stderr_bytes(b": ");
    }
    match error {
        crate::vfs::VfsError::EmptyPath => syscalls.stderr_bytes(b"empty path"),
        crate::vfs::VfsError::TooLong => syscalls.stderr_bytes(b"path too long"),
        crate::vfs::VfsError::NotFound => syscalls.stderr_bytes(b"no such file or directory"),
        crate::vfs::VfsError::NotDirectory => syscalls.stderr_bytes(b"not a directory"),
        crate::vfs::VfsError::NotWritable => syscalls.stderr_bytes(b"not writable"),
        crate::vfs::VfsError::Busy => syscalls.stderr_bytes(b"file busy"),
        crate::vfs::VfsError::FileTooLarge => syscalls.stderr_bytes(b"file too large"),
        crate::vfs::VfsError::Io => syscalls.stderr_bytes(b"I/O error"),
    }
    syscalls.stderr_bytes(b"\n");
}

fn run_source_arg1_dispatch(
    bytes: &'static [u8],
    mut offset: usize,
    argv: &ProgramArgv<'_>,
    syscalls: &mut ProgramSyscalls<'_, '_, '_>,
    unknown_message: &'static [u8],
) -> Option<(ProgramStatus, usize)> {
    let selected_arg = argv.arg(1);
    let mut selected = false;
    let mut execute_branch = false;
    let mut scanned = 0usize;

    while let Some((line, next)) = next_source_line(bytes, offset) {
        scanned += 1;
        if scanned > SOURCE_BYTES_MAX_OPS {
            return None;
        }

        if line == SOURCE_BYTES_END_DISPATCH_ARG1 {
            if selected_arg.is_some() && !selected {
                syscalls.stderr_bytes(unknown_message);
                syscalls.stderr_bytes(b"\n");
                return Some((ProgramStatus::Error, next));
            }
            return Some((ProgramStatus::Ok, next));
        }

        if line == SOURCE_BYTES_DISPATCH_DEFAULT {
            execute_branch = selected_arg.is_none();
            selected = selected || execute_branch;
            offset = next;
            continue;
        }

        if line.starts_with(SOURCE_BYTES_DISPATCH_CASE) {
            let case_arg = &line[SOURCE_BYTES_DISPATCH_CASE.len()..];
            execute_branch = selected_arg
                .map(|arg| arg.as_bytes() == case_arg)
                .unwrap_or(false);
            selected = selected || execute_branch;
            offset = next;
            continue;
        }

        if execute_branch {
            let status = run_source_byte_op(line, argv, syscalls)?;
            if status != ProgramStatus::Ok {
                return Some((status, next));
            }
        }

        offset = next;
    }

    None
}

fn exec_body_uapi_write_all(fd_control: SyscallFdControl, fd: RawFd, bytes: &[u8]) -> bool {
    let mut offset = 0usize;
    while offset < bytes.len() {
        let remaining = bytes.len() - offset;
        let Ok(written) = fd_control.write(fd, &bytes[offset..]) else {
            return false;
        };
        if written == 0 || written > remaining {
            return false;
        }
        offset += written;
    }
    true
}

fn exec_body_uapi_stderr_line(fd_control: SyscallFdControl, line: &[u8]) {
    let _ = exec_body_uapi_write_all(fd_control, RawFd::stderr(), line);
    let _ = exec_body_uapi_write_all(fd_control, RawFd::stderr(), b"\n");
}

fn store_bin_uapi_write_byte_continue_frame(
    current: Option<SyscallContext>,
    program: LoadedProgram,
    argv: &ProgramArgvBuffer,
    env: &ProgramEnvBuffer,
    stdin: &[u8],
    resume_offset: usize,
    byte: u8,
) -> Option<&'static [u8]> {
    let current = current?;
    store_bin_uapi_write_byte_frame(
        current,
        program,
        *argv,
        *env,
        stdin,
        resume_offset,
        byte,
        BinUapiResumeAction::ContinueFromOffset,
    )
}

fn write_bin_uapi_stdout_hex(
    encoded: &[u8],
    fd_control: SyscallFdControl,
    current: Option<SyscallContext>,
    program: LoadedProgram,
    argv: &ProgramArgvBuffer,
    env: &ProgramEnvBuffer,
    stdin: &[u8],
    resume_offset: usize,
) -> ProgramStatus {
    if encoded.len() % 2 != 0 {
        return ProgramStatus::Error;
    }

    let mut index = 0usize;
    while index < encoded.len() {
        let Some(high) = parse_source_hex_digit(encoded[index]) else {
            return ProgramStatus::Error;
        };
        let Some(low) = parse_source_hex_digit(encoded[index + 1]) else {
            return ProgramStatus::Error;
        };
        let Some(byte) = store_bin_uapi_write_byte_continue_frame(
            current,
            program,
            argv,
            env,
            stdin,
            resume_offset,
            (high << 4) | low,
        ) else {
            return ProgramStatus::Error;
        };
        match fd_control.write(RawFd::stdout(), byte) {
            Ok(1) => {
                if let Some(current) = current {
                    release_bin_uapi_resume_frame(current.pid);
                }
            }
            Ok(_) => {
                if let Some(current) = current {
                    release_bin_uapi_resume_frame(current.pid);
                }
                return ProgramStatus::Error;
            }
            Err(FsError::BUSY) => return ProgramStatus::Blocked,
            Err(_) => {
                if let Some(current) = current {
                    release_bin_uapi_resume_frame(current.pid);
                }
                return ProgramStatus::Error;
            }
        }
        index += 2;
    }

    ProgramStatus::Ok
}

fn close_bin_uapi_file_copy(fd_control: SyscallFdControl, file: RawFd) -> ProgramStatus {
    if fd_control.close(file).is_ok() {
        ProgramStatus::Ok
    } else {
        ProgramStatus::Error
    }
}

fn exec_body_uapi_write_file_copy_byte(
    fd_control: SyscallFdControl,
    current: SyscallContext,
    program: LoadedProgram,
    argv: &ProgramArgvBuffer,
    env: &ProgramEnvBuffer,
    stdin: &[u8],
    resume_offset: usize,
    file: RawFd,
    byte: u8,
) -> ProgramStatus {
    let Some(bytes) = store_bin_uapi_write_byte_frame(
        current,
        program,
        *argv,
        *env,
        stdin,
        resume_offset,
        byte,
        BinUapiResumeAction::ContinueFileCopyAfterWrite {
            file_fd: file.raw(),
        },
    ) else {
        let _ = fd_control.close(file);
        return ProgramStatus::Error;
    };

    match fd_control.write(RawFd::stdout(), bytes) {
        Ok(1) => {
            release_bin_uapi_resume_frame(current.pid);
            ProgramStatus::Ok
        }
        Ok(_) => {
            release_bin_uapi_resume_frame(current.pid);
            let _ = fd_control.close(file);
            ProgramStatus::Error
        }
        Err(FsError::BUSY) => ProgramStatus::Blocked,
        Err(_) => {
            release_bin_uapi_resume_frame(current.pid);
            let _ = fd_control.close(file);
            ProgramStatus::Error
        }
    }
}

fn resume_bin_uapi_file_copy_after_read(
    fd_control: SyscallFdControl,
    current: SyscallContext,
    program: LoadedProgram,
    argv: &ProgramArgvBuffer,
    env: &ProgramEnvBuffer,
    stdin: &[u8],
    resume_offset: usize,
    file: RawFd,
    byte: u8,
    read: usize,
) -> ProgramStatus {
    match read {
        0 => close_bin_uapi_file_copy(fd_control, file),
        1 => {
            let status = exec_body_uapi_write_file_copy_byte(
                fd_control,
                current,
                program,
                argv,
                env,
                stdin,
                resume_offset,
                file,
                byte,
            );
            if status == ProgramStatus::Ok {
                exec_body_uapi_copy_opened_file(
                    fd_control,
                    Some(current),
                    program,
                    argv,
                    env,
                    stdin,
                    resume_offset,
                    file,
                )
            } else {
                status
            }
        }
        _ => {
            let _ = fd_control.close(file);
            ProgramStatus::Error
        }
    }
}

fn exec_body_uapi_copy_opened_file_without_frame(
    fd_control: SyscallFdControl,
    file: RawFd,
) -> ProgramStatus {
    let mut buf = [0u8; 32];
    loop {
        let Ok(read) = fd_control.read(file, &mut buf) else {
            let _ = fd_control.close(file);
            return ProgramStatus::Error;
        };
        if read == 0 {
            return close_bin_uapi_file_copy(fd_control, file);
        }
        if !exec_body_uapi_write_all(fd_control, RawFd::stdout(), &buf[..read]) {
            let _ = fd_control.close(file);
            return ProgramStatus::Error;
        }
    }
}

fn exec_body_uapi_copy_opened_file(
    fd_control: SyscallFdControl,
    current: Option<SyscallContext>,
    program: LoadedProgram,
    argv: &ProgramArgvBuffer,
    env: &ProgramEnvBuffer,
    stdin: &[u8],
    resume_offset: usize,
    file: RawFd,
) -> ProgramStatus {
    let Some(current) = current else {
        return exec_body_uapi_copy_opened_file_without_frame(fd_control, file);
    };

    loop {
        let Some(out) = store_bin_uapi_read_byte_frame(
            current,
            program,
            *argv,
            *env,
            stdin,
            resume_offset,
            file,
        ) else {
            let _ = fd_control.close(file);
            return ProgramStatus::Error;
        };
        let read = fd_control.read(file, out);
        match read {
            Ok(0) => {
                release_bin_uapi_resume_frame(current.pid);
                return close_bin_uapi_file_copy(fd_control, file);
            }
            Ok(1) => {
                let Some(frame) = take_bin_uapi_resume_frame(current.pid) else {
                    let _ = fd_control.close(file);
                    return ProgramStatus::Error;
                };
                let status = exec_body_uapi_write_file_copy_byte(
                    fd_control,
                    current,
                    program,
                    argv,
                    env,
                    stdin,
                    resume_offset,
                    file,
                    frame.write_byte,
                );
                match status {
                    ProgramStatus::Ok => continue,
                    _ => return status,
                }
            }
            Ok(_) => {
                release_bin_uapi_resume_frame(current.pid);
                let _ = fd_control.close(file);
                return ProgramStatus::Error;
            }
            Err(FsError::BUSY) => return ProgramStatus::Blocked,
            Err(_) => {
                release_bin_uapi_resume_frame(current.pid);
                let _ = fd_control.close(file);
                return ProgramStatus::Error;
            }
        }
    }
}

fn exec_body_uapi_open_readonly_write_stdout(
    fd_control: SyscallFdControl,
    current: Option<SyscallContext>,
    program: LoadedProgram,
    argv: &ProgramArgvBuffer,
    env: &ProgramEnvBuffer,
    stdin: &[u8],
    resume_offset: usize,
    path: &[u8],
) -> ProgramStatus {
    let Ok(file) = fd_control.open_at(OpenAtDir::session_cwd(), path, OpenFlags::read_only())
    else {
        return ProgramStatus::Error;
    };

    exec_body_uapi_copy_opened_file(
        fd_control,
        current,
        program,
        argv,
        env,
        stdin,
        resume_offset,
        file,
    )
}

fn exec_body_uapi_exit(process: SyscallProcessControl, code: u8) -> ProgramStatus {
    if process.exit(ExitCode::new(code)).is_err() {
        return ProgramStatus::Error;
    }
    if code == 0 {
        ProgramStatus::Ok
    } else {
        ProgramStatus::ExitCode(code as i32)
    }
}

fn exec_body_uapi_spawn_wait_bin(
    fd: SyscallFdControl,
    process: SyscallProcessControl,
    current: Option<SyscallContext>,
    program: LoadedProgram,
    argv_buf: &ProgramArgvBuffer,
    env_buf: &ProgramEnvBuffer,
    stdin: &[u8],
    resume_offset: usize,
    argv_bytes: &[u8],
) -> ProgramStatus {
    let Some((args, argc)) = parse_bin_uapi_process_args(argv_bytes) else {
        exec_body_uapi_stderr_line(fd, b"error: invalid executable body");
        return ProgramStatus::Error;
    };

    let pid = match process.spawn(&args[..argc]) {
        Ok(pid) => pid,
        Err(ProcessError::BUSY) => return ProgramStatus::Blocked,
        Err(_) => {
            exec_body_uapi_stderr_line(fd, b"error: executable body spawn failed");
            return ProgramStatus::Error;
        }
    };

    let Some(current) = current else {
        exec_body_uapi_stderr_line(fd, b"error: executable body wait missing process");
        return ProgramStatus::Error;
    };
    if !store_bin_uapi_resume_frame(
        current,
        program,
        *argv_buf,
        *env_buf,
        stdin,
        resume_offset,
        BinUapiResumeAction::ExitWithWaitStatus,
    ) {
        exec_body_uapi_stderr_line(fd, b"error: executable body wait frame full");
        return ProgramStatus::Error;
    }
    match process.wait(pid) {
        Ok(code) => {
            release_bin_uapi_resume_frame(current.pid);
            exec_body_uapi_exit(process, code.raw())
        }
        Err(ProcessError::BUSY) => ProgramStatus::Blocked,
        Err(_) => {
            release_bin_uapi_resume_frame(current.pid);
            exec_body_uapi_stderr_line(fd, b"error: executable body wait failed");
            ProgramStatus::Error
        }
    }
}

fn exec_body_uapi_spawn_wait_bin_env(
    fd: SyscallFdControl,
    process: SyscallProcessControl,
    current: Option<SyscallContext>,
    program: LoadedProgram,
    argv_buf: &ProgramArgvBuffer,
    env_buf: &ProgramEnvBuffer,
    stdin: &[u8],
    resume_offset: usize,
    line: &[u8],
) -> ProgramStatus {
    let Some((env, args, argc)) = parse_bin_uapi_process_env_and_args(line) else {
        exec_body_uapi_stderr_line(fd, b"error: invalid executable body");
        return ProgramStatus::Error;
    };
    let envs = [env];

    let pid = match process.spawn_with_env(&args[..argc], &envs) {
        Ok(pid) => pid,
        Err(ProcessError::BUSY) => return ProgramStatus::Blocked,
        Err(_) => {
            exec_body_uapi_stderr_line(fd, b"error: executable body spawn failed");
            return ProgramStatus::Error;
        }
    };

    let Some(current) = current else {
        exec_body_uapi_stderr_line(fd, b"error: executable body wait missing process");
        return ProgramStatus::Error;
    };
    if !store_bin_uapi_resume_frame(
        current,
        program,
        *argv_buf,
        *env_buf,
        stdin,
        resume_offset,
        BinUapiResumeAction::ExitWithWaitStatus,
    ) {
        exec_body_uapi_stderr_line(fd, b"error: executable body wait frame full");
        return ProgramStatus::Error;
    }
    match process.wait(pid) {
        Ok(code) => {
            release_bin_uapi_resume_frame(current.pid);
            exec_body_uapi_exit(process, code.raw())
        }
        Err(ProcessError::BUSY) => ProgramStatus::Blocked,
        Err(_) => {
            release_bin_uapi_resume_frame(current.pid);
            exec_body_uapi_stderr_line(fd, b"error: executable body wait failed");
            ProgramStatus::Error
        }
    }
}

fn exec_body_uapi_spawn_bin_env(
    fd: SyscallFdControl,
    process: SyscallProcessControl,
    line: &[u8],
) -> ProgramStatus {
    let Some((env, args, argc)) = parse_bin_uapi_process_env_and_args(line) else {
        exec_body_uapi_stderr_line(fd, b"error: invalid executable body");
        return ProgramStatus::Error;
    };
    let envs = [env];

    match process.spawn_with_env(&args[..argc], &envs) {
        Ok(_) => ProgramStatus::Ok,
        Err(ProcessError::BUSY) => ProgramStatus::Blocked,
        Err(_) => {
            exec_body_uapi_stderr_line(fd, b"error: executable body spawn failed");
            ProgramStatus::Error
        }
    }
}

fn exec_body_uapi_spawn_sleep_bin_env(
    fd: SyscallFdControl,
    process: SyscallProcessControl,
    line: &[u8],
) -> ProgramStatus {
    let Some((ticks, env, args, argc)) = parse_bin_uapi_sleep_ticks_env_and_args(line) else {
        exec_body_uapi_stderr_line(fd, b"error: invalid executable body");
        return ProgramStatus::Error;
    };
    let envs = [env];

    match process.spawn_sleeping_with_env(&args[..argc], &envs, ticks) {
        Ok(_) => ProgramStatus::Ok,
        Err(ProcessError::BUSY) => ProgramStatus::Blocked,
        Err(_) => {
            exec_body_uapi_stderr_line(fd, b"error: executable body sleep spawn failed");
            ProgramStatus::Error
        }
    }
}

fn exec_body_uapi_exec_bin_env(
    fd: SyscallFdControl,
    process: SyscallProcessControl,
    line: &[u8],
) -> ProgramStatus {
    let Some((env, args, argc)) = parse_bin_uapi_process_env_and_args(line) else {
        exec_body_uapi_stderr_line(fd, b"error: invalid executable body");
        return ProgramStatus::Error;
    };
    let envs = [env];

    match process.execve_with_env(&args[..argc], &envs) {
        Ok(_) => ProgramStatus::Replaced,
        Err(ProcessError::BUSY) => ProgramStatus::Blocked,
        Err(ProcessError::NOT_FOUND) => {
            exec_body_uapi_stderr_line(fd, b"error: executable body exec not found");
            ProgramStatus::Error
        }
        Err(ProcessError::INVALID_IMAGE) => {
            exec_body_uapi_stderr_line(fd, b"error: executable body exec invalid image");
            ProgramStatus::Error
        }
        Err(_) => {
            exec_body_uapi_stderr_line(fd, b"error: executable body exec failed");
            ProgramStatus::Error
        }
    }
}

fn exec_body_uapi_scheduler_tick(
    fd: SyscallFdControl,
    sched: SyscallSchedulerControl,
) -> ProgramStatus {
    match sched.tick_current() {
        Ok(_) => ProgramStatus::Ok,
        Err(error) if error == SchedulerError::BUSY => ProgramStatus::Blocked,
        Err(_) => {
            exec_body_uapi_stderr_line(fd, b"error: executable body scheduler tick failed");
            ProgramStatus::Error
        }
    }
}

fn exec_body_uapi_sleep_ticks(
    fd: SyscallFdControl,
    sched: SyscallSchedulerControl,
    current: Option<SyscallContext>,
    program: LoadedProgram,
    argv: &ProgramArgvBuffer,
    env: &ProgramEnvBuffer,
    stdin: &[u8],
    resume_offset: usize,
    line: &[u8],
) -> ProgramStatus {
    let Some(ticks) = parse_bin_uapi_ticks(line) else {
        exec_body_uapi_stderr_line(fd, b"error: invalid executable body");
        return ProgramStatus::Error;
    };
    let Some(current) = current else {
        exec_body_uapi_stderr_line(fd, b"error: executable body sleep missing process");
        return ProgramStatus::Error;
    };
    if !store_bin_uapi_resume_frame(
        current,
        program,
        *argv,
        *env,
        stdin,
        resume_offset,
        BinUapiResumeAction::ContinueFromOffset,
    ) {
        exec_body_uapi_stderr_line(fd, b"error: executable body sleep frame full");
        return ProgramStatus::Error;
    }

    match sched.sleep_for_ticks(SchedulerTicks::new(ticks)) {
        Ok(_) => {
            release_bin_uapi_resume_frame(current.pid);
            ProgramStatus::Ok
        }
        Err(error) if error == SchedulerError::BUSY => ProgramStatus::Blocked,
        Err(_) => {
            release_bin_uapi_resume_frame(current.pid);
            exec_body_uapi_stderr_line(fd, b"error: executable body sleep failed");
            ProgramStatus::Error
        }
    }
}

fn exec_body_uapi_yield_now(fd: SyscallFdControl, sched: SyscallSchedulerControl) -> ProgramStatus {
    match sched.yield_now() {
        Ok(_) => ProgramStatus::Ok,
        Err(error) if error == SchedulerError::BUSY => ProgramStatus::Blocked,
        Err(_) => {
            exec_body_uapi_stderr_line(fd, b"error: executable body yield failed");
            ProgramStatus::Error
        }
    }
}

fn run_bin_uapi_body(
    bytes: &[u8],
    program: LoadedProgram,
    argv_buf: &ProgramArgvBuffer,
    env_buf: &ProgramEnvBuffer,
    stdin: &[u8],
    raw: RawSyscall,
    current: Option<SyscallContext>,
    start_offset: usize,
) -> ProgramStatus {
    let argv = argv_buf.borrowed();
    let env = env_buf.borrowed();
    let fd = SyscallFdControl::new(raw);
    let process = SyscallProcessControl::new(raw);
    let sched = SyscallSchedulerControl::new(raw);
    let mut offset = if start_offset == 0 {
        let Some((header, offset)) = next_bin_uapi_line(bytes, 0) else {
            exec_body_uapi_stderr_line(fd, b"error: invalid executable body");
            return ProgramStatus::Error;
        };
        if header != BIN_UAPI_BODY_MAGIC {
            exec_body_uapi_stderr_line(fd, b"error: invalid executable body");
            return ProgramStatus::Error;
        }
        offset
    } else if start_offset <= bytes.len() {
        start_offset
    } else {
        exec_body_uapi_stderr_line(fd, b"error: invalid executable body");
        return ProgramStatus::Error;
    };

    let mut ops = 0usize;
    while let Some((line, next)) = next_bin_uapi_line(bytes, offset) {
        ops += 1;
        if ops > BIN_UAPI_MAX_OPS {
            exec_body_uapi_stderr_line(fd, b"error: executable body too large");
            return ProgramStatus::Error;
        }
        if line.is_empty() {
            offset = next;
            continue;
        }
        if line.starts_with(BIN_UAPI_OP_OPEN_READONLY_WRITE_STDOUT_ENV_OR_ARG1_OR) {
            let Some((name, default_path)) = split_bin_uapi_env_or(
                &line[BIN_UAPI_OP_OPEN_READONLY_WRITE_STDOUT_ENV_OR_ARG1_OR.len()..],
            ) else {
                exec_body_uapi_stderr_line(fd, b"error: invalid executable body");
                return ProgramStatus::Error;
            };
            let Ok(name) = core::str::from_utf8(name) else {
                exec_body_uapi_stderr_line(fd, b"error: invalid executable body");
                return ProgramStatus::Error;
            };
            let path = env
                .get(name)
                .or_else(|| argv.arg(1))
                .map_or(default_path, |value| value.as_bytes());
            if !program_env_name_is_valid(name.as_bytes()) || !validate_bin_uapi_path(path) {
                exec_body_uapi_stderr_line(fd, b"error: executable body fd copy failed");
                return ProgramStatus::Error;
            }
            let status = exec_body_uapi_open_readonly_write_stdout(
                fd, current, program, argv_buf, env_buf, stdin, next, path,
            );
            if status == ProgramStatus::Error {
                exec_body_uapi_stderr_line(fd, b"error: executable body fd copy failed");
            }
            if status != ProgramStatus::Ok {
                return status;
            }
            offset = next;
            continue;
        }
        if line.starts_with(BIN_UAPI_OP_OPEN_READONLY_WRITE_STDOUT_ARG1_OR) {
            let default_path = &line[BIN_UAPI_OP_OPEN_READONLY_WRITE_STDOUT_ARG1_OR.len()..];
            let path = argv.arg(1).map_or(default_path, |arg| arg.as_bytes());
            if !validate_bin_uapi_path(path) {
                exec_body_uapi_stderr_line(fd, b"error: executable body fd copy failed");
                return ProgramStatus::Error;
            }
            let status = exec_body_uapi_open_readonly_write_stdout(
                fd, current, program, argv_buf, env_buf, stdin, next, path,
            );
            if status == ProgramStatus::Error {
                exec_body_uapi_stderr_line(fd, b"error: executable body fd copy failed");
            }
            if status != ProgramStatus::Ok {
                return status;
            }
            offset = next;
            continue;
        }
        if line.starts_with(BIN_UAPI_OP_OPEN_READONLY_WRITE_STDOUT) {
            let path = &line[BIN_UAPI_OP_OPEN_READONLY_WRITE_STDOUT.len()..];
            if !validate_bin_uapi_path(path) {
                exec_body_uapi_stderr_line(fd, b"error: executable body fd copy failed");
                return ProgramStatus::Error;
            }
            let status = exec_body_uapi_open_readonly_write_stdout(
                fd, current, program, argv_buf, env_buf, stdin, next, path,
            );
            if status == ProgramStatus::Error {
                exec_body_uapi_stderr_line(fd, b"error: executable body fd copy failed");
            }
            if status != ProgramStatus::Ok {
                return status;
            }
            offset = next;
            continue;
        }
        if line.starts_with(BIN_UAPI_OP_EXEC_BIN_ENV) {
            return exec_body_uapi_exec_bin_env(
                fd,
                process,
                &line[BIN_UAPI_OP_EXEC_BIN_ENV.len()..],
            );
        }
        if line.starts_with(BIN_UAPI_OP_SLEEP_TICKS) {
            let status = exec_body_uapi_sleep_ticks(
                fd,
                sched,
                current,
                program,
                argv_buf,
                env_buf,
                stdin,
                next,
                &line[BIN_UAPI_OP_SLEEP_TICKS.len()..],
            );
            if status != ProgramStatus::Ok {
                return status;
            }
            offset = next;
            continue;
        }
        if line.starts_with(BIN_UAPI_OP_SPAWN_SLEEP_BIN_ENV) {
            let status = exec_body_uapi_spawn_sleep_bin_env(
                fd,
                process,
                &line[BIN_UAPI_OP_SPAWN_SLEEP_BIN_ENV.len()..],
            );
            if status != ProgramStatus::Ok {
                return status;
            }
            offset = next;
            continue;
        }
        if line.starts_with(BIN_UAPI_OP_SPAWN_BIN_ENV) {
            let status =
                exec_body_uapi_spawn_bin_env(fd, process, &line[BIN_UAPI_OP_SPAWN_BIN_ENV.len()..]);
            if status != ProgramStatus::Ok {
                return status;
            }
            offset = next;
            continue;
        }
        if line.starts_with(BIN_UAPI_OP_SPAWN_WAIT_BIN_ENV) {
            let status = exec_body_uapi_spawn_wait_bin_env(
                fd,
                process,
                current,
                program,
                argv_buf,
                env_buf,
                stdin,
                next,
                &line[BIN_UAPI_OP_SPAWN_WAIT_BIN_ENV.len()..],
            );
            if status != ProgramStatus::Ok {
                return status;
            }
            offset = next;
            continue;
        }
        if line.starts_with(BIN_UAPI_OP_SPAWN_WAIT_BIN) {
            let status = exec_body_uapi_spawn_wait_bin(
                fd,
                process,
                current,
                program,
                argv_buf,
                env_buf,
                stdin,
                next,
                &line[BIN_UAPI_OP_SPAWN_WAIT_BIN.len()..],
            );
            if status != ProgramStatus::Ok {
                return status;
            }
            offset = next;
            continue;
        }
        if line == BIN_UAPI_OP_SCHEDULER_TICK {
            let status = exec_body_uapi_scheduler_tick(fd, sched);
            if status != ProgramStatus::Ok {
                return status;
            }
            offset = next;
            continue;
        }
        if line == BIN_UAPI_OP_YIELD_NOW {
            let status = exec_body_uapi_yield_now(fd, sched);
            if status != ProgramStatus::Ok {
                return status;
            }
            offset = next;
            continue;
        }
        if line.starts_with(BIN_UAPI_OP_WRITE_STDOUT_HEX) {
            let status = write_bin_uapi_stdout_hex(
                &line[BIN_UAPI_OP_WRITE_STDOUT_HEX.len()..],
                fd,
                current,
                program,
                argv_buf,
                env_buf,
                stdin,
                next,
            );
            if status == ProgramStatus::Error {
                exec_body_uapi_stderr_line(fd, b"error: executable body write failed");
            }
            if status != ProgramStatus::Ok {
                return status;
            }
            offset = next;
            continue;
        }
        if line.starts_with(BIN_UAPI_OP_EXIT_STATUS) {
            return match &line[BIN_UAPI_OP_EXIT_STATUS.len()..] {
                b"ok" => exec_body_uapi_exit(process, ExitCode::SUCCESS.raw()),
                b"error" => exec_body_uapi_exit(process, ExitCode::FAILURE.raw()),
                _ => {
                    exec_body_uapi_stderr_line(fd, b"error: invalid executable body");
                    ProgramStatus::Error
                }
            };
        }
        if line.starts_with(BIN_UAPI_OP_EXIT_CODE) {
            let Some(code) = parse_source_exit_code(&line[BIN_UAPI_OP_EXIT_CODE.len()..]) else {
                exec_body_uapi_stderr_line(fd, b"error: invalid executable body");
                return ProgramStatus::Error;
            };
            return exec_body_uapi_exit(process, code as u8);
        }
        exec_body_uapi_stderr_line(fd, b"error: invalid executable body");
        return ProgramStatus::Error;
    }

    ProgramStatus::Ok
}

fn run_source_bytes(
    bytes: &'static [u8],
    argv: &ProgramArgv<'_>,
    syscalls: &mut ProgramSyscalls<'_, '_, '_>,
) -> ProgramStatus {
    let Some((header, mut offset)) = next_source_line(bytes, 0) else {
        syscalls.stderr_line("error: invalid source image");
        return ProgramStatus::Error;
    };
    if header != SOURCE_BYTES_MAGIC {
        syscalls.stderr_line("error: invalid source image");
        return ProgramStatus::Error;
    }

    let mut ops = 0usize;
    while let Some((line, next)) = next_source_line(bytes, offset) {
        ops += 1;
        if ops > SOURCE_BYTES_MAX_OPS {
            syscalls.stderr_line("error: source image too large");
            return ProgramStatus::Error;
        }
        if line.starts_with(SOURCE_BYTES_OP_DISPATCH_ARG1) {
            let Some((status, next)) = run_source_arg1_dispatch(
                bytes,
                next,
                argv,
                syscalls,
                &line[SOURCE_BYTES_OP_DISPATCH_ARG1.len()..],
            ) else {
                syscalls.stderr_line("error: invalid source image");
                return ProgramStatus::Error;
            };
            if status != ProgramStatus::Ok {
                return status;
            }
            offset = next;
            continue;
        }
        let Some(status) = run_source_byte_op(line, argv, syscalls) else {
            syscalls.stderr_line("error: invalid source image");
            return ProgramStatus::Error;
        };
        if status != ProgramStatus::Ok {
            return status;
        }
        offset = next;
    }

    ProgramStatus::Ok
}

fn run_loaded_program_from_offset(
    program: LoadedProgram,
    argv_buf: &ProgramArgvBuffer,
    env_buf: &ProgramEnvBuffer,
    stdin: &[u8],
    syscalls: &mut ProgramSyscalls<'_, '_, '_>,
    start_offset: usize,
) -> ProgramStatus {
    let _stdio = syscalls.stdio();
    let argv = argv_buf.borrowed();
    match program.image_kind {
        ProgramImageKind::SourceImage => run_source_bytes(program.source_bytes(), &argv, syscalls),
        ProgramImageKind::ReovimExecBody => {
            let Ok(body) = exec_body::parse_exec_body(program.source_bytes()) else {
                syscalls.stderr_line("error: invalid executable body");
                return ProgramStatus::Error;
            };
            match body.inner {
                ExecBodyInnerFormat::BinSourceImage => {
                    run_source_bytes(body.bytes, &argv, syscalls)
                }
                ExecBodyInnerFormat::BinUapiV1 => {
                    let Some(_guard) = LinkedRawSyscallGuard::enter(syscalls) else {
                        syscalls.stderr_line("error: executable body syscall backend busy");
                        return ProgramStatus::Error;
                    };
                    let returned = run_bin_uapi_body(
                        body.bytes,
                        program,
                        argv_buf,
                        env_buf,
                        stdin,
                        RawSyscall::new(linked_raw_syscall),
                        syscalls.current_context(),
                        start_offset,
                    );
                    syscalls.take_raw_exit_status().unwrap_or(returned)
                }
                ExecBodyInnerFormat::PayloadSourceImage => {
                    syscalls.stderr_line("error: invalid executable body");
                    ProgramStatus::Error
                }
            }
        }
        ProgramImageKind::LinkedBin => {
            let Some(entry) = program.linked_entry else {
                syscalls.stderr_line("error: invalid linked image");
                return ProgramStatus::Error;
            };
            let Some(_guard) = LinkedRawSyscallGuard::enter(syscalls) else {
                syscalls.stderr_line("error: linked syscall backend busy");
                return ProgramStatus::Error;
            };
            let returned = entry(&argv, RawSyscall::new(linked_raw_syscall));
            syscalls.take_raw_exit_status().unwrap_or(returned)
        }
    }
}

/// Runs one loaded `/bin` program through the typed syscall handle.
pub(crate) fn run_loaded_program(
    program: LoadedProgram,
    argv: &ProgramArgvBuffer,
    env: &ProgramEnvBuffer,
    stdin: &[u8],
    syscalls: &mut ProgramSyscalls<'_, '_, '_>,
) -> ProgramStatus {
    run_loaded_program_from_offset(program, argv, env, stdin, syscalls, 0)
}

#[cfg(feature = "selftest")]
#[path = "program_tests.rs"]
mod tests;
