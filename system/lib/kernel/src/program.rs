//! Generic `/bin` executable descriptor and dispatch ABI.
//!
//! The system kernel owns exec admission, process/scheduler bookkeeping, and
//! the syscall handle exposed to programs. Concrete operator behavior is not
//! owned by the shell or this crate: the OS image supplies a `/bin`
//! descriptor slice during boot.

use {
    crate::{
        rootd::PayloadSourceInstallStatus, source_store::ExecutableSourceStore,
        syscall::ProgramSyscalls,
    },
    core::{
        cell::UnsafeCell,
        slice,
        sync::atomic::{AtomicBool, Ordering},
    },
};

/// Maximum bytes attached to one program's initial stdin buffer.
pub const MAX_PROGRAM_STDIN_BYTES: usize = 128;
/// Maximum bytes captured from one program's stdout for a bounded pipe.
pub const MAX_PROGRAM_PIPE_BYTES: usize = MAX_PROGRAM_STDIN_BYTES;
/// Maximum argv entries passed to one image program.
pub const MAX_PROGRAM_ARGS: usize = 8;
/// Maximum bytes in one argv token.
pub const MAX_PROGRAM_ARG_BYTES: usize = 64;
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
            Self::Halt => b"halt",
        }
    }

    /// Numeric process exit code represented by this program status.
    #[must_use]
    pub const fn exit_code(self) -> i32 {
        match self {
            Self::Empty | Self::Ok | Self::Halt => 0,
            Self::Error => 1,
            Self::ExitCode(code) => code,
        }
    }

    /// Whether this status counts as successful completion.
    #[must_use]
    pub const fn is_success(self) -> bool {
        match self {
            Self::Ok => true,
            Self::Empty | Self::Error | Self::Halt => false,
            Self::ExitCode(code) => code == 0,
        }
    }
}

/// Source kind for a loaded `/bin` program.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProgramImageKind {
    /// Program body is interpreted from a bounded Reovim source image.
    SourceImage,
}

impl ProgramImageKind {
    /// Stable diagnostic word.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SourceImage => "source-image",
        }
    }
}

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
const SOURCE_BYTES_OP_REQUEST_SHELL_TARGET: &[u8] = b"request-shell-target ";
const SOURCE_BYTES_OP_REQUEST_ROOT_SHELL: &[u8] = b"request-root-shell";
const SOURCE_BYTES_OP_WRITE_KERNEL_LOG_VIEW: &[u8] = b"write-kernel-log-view";
const SOURCE_BYTES_OP_WRITE_KERNEL_LOG_STATS: &[u8] = b"write-kernel-log-stats";
const SOURCE_BYTES_OP_WRITE_DUMP_STATUS: &[u8] = b"write-dump-status";
const SOURCE_BYTES_OP_WRITE_DUMP_SNAPSHOT: &[u8] = b"write-dump-snapshot";
const SOURCE_BYTES_OP_WRITE_DUMP_SYNC: &[u8] = b"write-dump-sync";
const SOURCE_BYTES_OP_WRITE_SCHEDULER_STATE: &[u8] = b"write-scheduler-state";
const SOURCE_BYTES_OP_WRITE_SCHEDULER_TICK: &[u8] = b"write-scheduler-tick";
const SOURCE_BYTES_OP_WRITE_SCHEDULER_YIELD: &[u8] = b"write-scheduler-yield";
const SOURCE_BYTES_OP_WRITE_PROCESS_TABLE: &[u8] = b"write-process-table";
const SOURCE_BYTES_OP_WRITE_EXEC_LOAD_TABLE: &[u8] = b"write-exec-load-table";
const SOURCE_BYTES_OP_WRITE_PENDING_EXEC_TABLE: &[u8] = b"write-pending-exec-table";
const SOURCE_BYTES_OP_WRITE_PROCESS_SELF: &[u8] = b"write-process-self";
const SOURCE_BYTES_OP_WRITE_SOURCE_STORE_TABLE: &[u8] = b"write-source-store-table";
const SOURCE_BYTES_OP_WRITE_SOURCE_MEDIA_TABLE: &[u8] = b"write-source-media-table";
const SOURCE_BYTES_OP_WRITE_TASK_TABLE: &[u8] = b"write-task-table";
const SOURCE_BYTES_OP_WRITE_WAIT_TABLE: &[u8] = b"write-wait-table";
const SOURCE_BYTES_OP_WRITE_SYSCALL_TABLE: &[u8] = b"write-syscall-table";
const SOURCE_BYTES_OP_PROC_EXEC_ARGV_TAIL: &[u8] = b"proc-exec-argv-tail";
const SOURCE_BYTES_OP_PROC_SPAWN_ARGV_TAIL: &[u8] = b"proc-spawn-argv-tail";
const SOURCE_BYTES_OP_PROC_BLOCK_ARGV_TAIL: &[u8] = b"proc-block-argv-tail";
const SOURCE_BYTES_OP_PROC_WAIT_PID_ARG2: &[u8] = b"proc-wait-pid-arg2";
const SOURCE_BYTES_OP_PROC_WAKE_PID_ARG2: &[u8] = b"proc-wake-pid-arg2";
const SOURCE_BYTES_OP_PROC_KILL_PID_ARG2: &[u8] = b"proc-kill-pid-arg2";
const SOURCE_BYTES_OP_PROC_INSTALL_BIN_ARG2_ARG3: &[u8] = b"proc-install-bin-arg2-arg3";
const SOURCE_BYTES_OP_PROC_INSTALL_PAYLOAD_ARG2_ARG3: &[u8] = b"proc-install-payload-arg2-arg3";
const SOURCE_BYTES_OP_PROC_INSTALL_BIN_MEDIA_ARG2: &[u8] = b"proc-install-bin-media-arg2";
const SOURCE_BYTES_OP_PROC_INSTALL_PAYLOAD_MEDIA_ARG2: &[u8] = b"proc-install-payload-media-arg2";
const SOURCE_BYTES_OP_RUN_PROVIDER_PROBE_ARG1: &[u8] = b"run-provider-probe-arg1";
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

/// Executable body reference attached to one `/bin` descriptor.
#[derive(Clone, Copy, Debug)]
pub enum ProgramImage {
    /// Program source bytes are loaded from a source store path.
    SourcePath(&'static str),
}

impl ProgramImage {
    /// Loader/source kind for this image body.
    #[must_use]
    pub const fn kind(self) -> ProgramImageKind {
        match self {
            Self::SourcePath(_) => ProgramImageKind::SourceImage,
        }
    }

    /// Source-store path for this executable body.
    #[must_use]
    pub const fn source_path(self) -> &'static str {
        match self {
            Self::SourcePath(path) => path,
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
}

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

/// Loads argv[0] as either an absolute `/bin` path or a `/bin` basename.
pub fn load_argv0(
    programs: &'static [ProgramDescriptor],
    source_store: ExecutableSourceStore,
    argv0: &str,
) -> Result<Option<LoadedProgram>, ProgramLoadError> {
    let Some((catalog_index, descriptor)) = resolve_argv0(programs, argv0) else {
        return Ok(None);
    };
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
    }))
}

/// Validates a loaded `/bin` program image before process admission.
#[must_use]
pub fn validate_loaded_program(program: LoadedProgram) -> bool {
    validate_source_bytes(program.source_bytes)
}

impl LoadedProgram {
    /// Returns the loaded executable source bytes.
    #[must_use]
    pub const fn source_bytes(self) -> &'static [u8] {
        self.source_bytes
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
        || line == SOURCE_BYTES_OP_WRITE_PROCESS_TABLE
        || line == SOURCE_BYTES_OP_WRITE_EXEC_LOAD_TABLE
        || line == SOURCE_BYTES_OP_WRITE_PENDING_EXEC_TABLE
        || line == SOURCE_BYTES_OP_WRITE_PROCESS_SELF
        || line == SOURCE_BYTES_OP_WRITE_SOURCE_STORE_TABLE
        || line == SOURCE_BYTES_OP_WRITE_SOURCE_MEDIA_TABLE
        || line == SOURCE_BYTES_OP_WRITE_TASK_TABLE
        || line == SOURCE_BYTES_OP_WRITE_WAIT_TABLE
        || line == SOURCE_BYTES_OP_WRITE_SYSCALL_TABLE
        || line == SOURCE_BYTES_OP_PROC_EXEC_ARGV_TAIL
        || line == SOURCE_BYTES_OP_PROC_SPAWN_ARGV_TAIL
        || line == SOURCE_BYTES_OP_PROC_BLOCK_ARGV_TAIL
        || line == SOURCE_BYTES_OP_PROC_WAIT_PID_ARG2
        || line == SOURCE_BYTES_OP_PROC_WAKE_PID_ARG2
        || line == SOURCE_BYTES_OP_PROC_KILL_PID_ARG2
        || line == SOURCE_BYTES_OP_PROC_INSTALL_BIN_ARG2_ARG3
        || line == SOURCE_BYTES_OP_PROC_INSTALL_PAYLOAD_ARG2_ARG3
        || line == SOURCE_BYTES_OP_PROC_INSTALL_BIN_MEDIA_ARG2
        || line == SOURCE_BYTES_OP_PROC_INSTALL_PAYLOAD_MEDIA_ARG2
        || line == SOURCE_BYTES_OP_RUN_PROVIDER_PROBE_ARG1
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

fn parse_payload_source_install_status(value: &str) -> Option<PayloadSourceInstallStatus> {
    match value.as_bytes() {
        b"ready" => Some(PayloadSourceInstallStatus::Ready),
        b"failed" => Some(PayloadSourceInstallStatus::Failed),
        _ => None,
    }
}

fn parse_bin_source_install_status(value: &str) -> Option<BinSourceInstallStatus> {
    match value.as_bytes() {
        b"ok" => Some(BinSourceInstallStatus::Ok),
        b"error" => Some(BinSourceInstallStatus::Error),
        _ => None,
    }
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

fn build_source_tail_argv(
    argv: &ProgramArgv<'_>,
    start: usize,
    error_prefix: &[u8],
    missing_message: &str,
    syscalls: &mut ProgramSyscalls<'_, '_, '_>,
) -> Option<ProgramArgvBuffer> {
    if argv.argc() == start {
        syscalls.stderr_line(missing_message);
        return None;
    }

    let mut child_argv = ProgramArgvBuffer::empty();
    let mut index = start;
    while index < argv.argc() {
        if let Err(error) = child_argv.push(argv.arg(index).unwrap_or("")) {
            syscalls.stderr_bytes(error_prefix);
            syscalls.stderr_line(error.as_str());
            return None;
        }
        index += 1;
    }
    Some(child_argv)
}

fn run_proc_exec_tail(
    argv: &ProgramArgv<'_>,
    syscalls: &mut ProgramSyscalls<'_, '_, '_>,
) -> ProgramStatus {
    let Some(child_argv) =
        build_source_tail_argv(argv, 2, b"proc exec: ", "proc exec: missing program", syscalls)
    else {
        return ProgramStatus::Error;
    };

    match syscalls.exec_program_argv_and_wait(child_argv) {
        Ok(status) => status,
        Err(error) => {
            syscalls.stderr_bytes(b"proc exec: ");
            syscalls.stderr_line(error.as_str());
            ProgramStatus::Error
        }
    }
}

fn run_proc_spawn_tail(
    argv: &ProgramArgv<'_>,
    syscalls: &mut ProgramSyscalls<'_, '_, '_>,
) -> ProgramStatus {
    let Some(child_argv) =
        build_source_tail_argv(argv, 2, b"proc spawn: ", "proc spawn: missing program", syscalls)
    else {
        return ProgramStatus::Error;
    };

    match syscalls.spawn_program_argv(child_argv) {
        Ok(child) => {
            syscalls.stdout_line("proc spawn:");
            syscalls.stdout_bytes(b"pid=");
            source_write_u64_dec(syscalls, child.pid as u64);
            syscalls.stdout_bytes(b"\npath=");
            syscalls.stdout_bytes(child.program_path.as_bytes());
            syscalls.stdout_bytes(b"\nstate=ready\n");
            ProgramStatus::Ok
        }
        Err(error) => {
            syscalls.stderr_bytes(b"proc spawn: ");
            syscalls.stderr_line(error.as_str());
            ProgramStatus::Error
        }
    }
}

fn run_proc_block_tail(
    argv: &ProgramArgv<'_>,
    syscalls: &mut ProgramSyscalls<'_, '_, '_>,
) -> ProgramStatus {
    let Some(child_argv) =
        build_source_tail_argv(argv, 2, b"proc block: ", "proc block: missing program", syscalls)
    else {
        return ProgramStatus::Error;
    };

    match syscalls.spawn_blocked_program_argv(child_argv) {
        Ok(child) => {
            syscalls.stdout_line("proc block:");
            syscalls.stdout_bytes(b"pid=");
            source_write_u64_dec(syscalls, child.pid as u64);
            syscalls.stdout_bytes(b"\npath=");
            syscalls.stdout_bytes(child.program_path.as_bytes());
            syscalls.stdout_bytes(b"\nstate=blocked\n");
            ProgramStatus::Ok
        }
        Err(error) => {
            syscalls.stderr_bytes(b"proc block: ");
            syscalls.stderr_line(error.as_str());
            ProgramStatus::Error
        }
    }
}

fn run_proc_wake_pid_arg2(
    argv: &ProgramArgv<'_>,
    syscalls: &mut ProgramSyscalls<'_, '_, '_>,
) -> ProgramStatus {
    let Some(pid_text) = argv.arg(2) else {
        syscalls.stderr_line("proc wake: missing pid");
        return ProgramStatus::Error;
    };
    if argv.argc() > 3 {
        syscalls.stderr_line("proc wake: too many arguments");
        return ProgramStatus::Error;
    }

    let Some(pid) = parse_source_usize_bytes(pid_text.as_bytes()) else {
        syscalls.stderr_line("proc wake: invalid pid");
        return ProgramStatus::Error;
    };

    match syscalls.wake_process_by_pid(pid) {
        Ok(handle) => {
            syscalls.stdout_line("proc wake:");
            syscalls.stdout_bytes(b"pid=");
            source_write_u64_dec(syscalls, handle.pid as u64);
            syscalls.stdout_bytes(b"\npath=");
            syscalls.stdout_bytes(handle.program_path.as_bytes());
            syscalls.stdout_bytes(b"\nstate=ready\n");
            ProgramStatus::Ok
        }
        Err(error) => {
            syscalls.stderr_bytes(b"proc wake: ");
            syscalls.stderr_line(error.as_str());
            ProgramStatus::Error
        }
    }
}

fn run_proc_wait_pid_arg2(
    argv: &ProgramArgv<'_>,
    syscalls: &mut ProgramSyscalls<'_, '_, '_>,
) -> ProgramStatus {
    let Some(pid_text) = argv.arg(2) else {
        syscalls.stderr_line("proc wait: missing pid");
        return ProgramStatus::Error;
    };
    if argv.argc() > 3 {
        syscalls.stderr_line("proc wait: too many arguments");
        return ProgramStatus::Error;
    }

    let Some(pid) = parse_source_usize_bytes(pid_text.as_bytes()) else {
        syscalls.stderr_line("proc wait: invalid pid");
        return ProgramStatus::Error;
    };

    match syscalls.wait_process_by_pid(pid) {
        Ok(wait) => {
            syscalls.stdout_line("proc wait:");
            syscalls.stdout_bytes(b"pid=");
            source_write_u64_dec(syscalls, wait.child_pid as u64);
            syscalls.stdout_bytes(b"\nstate=");
            syscalls.stdout_bytes(wait.child_state.as_str().as_bytes());
            syscalls.stdout_bytes(b"\nexit=");
            source_write_u64_dec(syscalls, wait.exit_code as u64);
            syscalls.stdout_bytes(b"\ncompleted=");
            if wait.completed {
                syscalls.stdout_bytes(b"true\n");
            } else {
                syscalls.stdout_bytes(b"false\n");
            }
            ProgramStatus::Ok
        }
        Err(error) => {
            syscalls.stderr_bytes(b"proc wait: ");
            syscalls.stderr_line(error.as_str());
            ProgramStatus::Error
        }
    }
}

fn run_proc_kill_pid_arg2(
    argv: &ProgramArgv<'_>,
    syscalls: &mut ProgramSyscalls<'_, '_, '_>,
) -> ProgramStatus {
    let Some(pid_text) = argv.arg(2) else {
        syscalls.stderr_line("proc kill: missing pid");
        return ProgramStatus::Error;
    };
    if argv.argc() > 3 {
        syscalls.stderr_line("proc kill: too many arguments");
        return ProgramStatus::Error;
    }

    let Some(pid) = parse_source_usize_bytes(pid_text.as_bytes()) else {
        syscalls.stderr_line("proc kill: invalid pid");
        return ProgramStatus::Error;
    };

    match syscalls.kill_process_by_pid(pid) {
        Ok(handle) => {
            syscalls.stdout_line("proc kill:");
            syscalls.stdout_bytes(b"pid=");
            source_write_u64_dec(syscalls, handle.pid as u64);
            syscalls.stdout_bytes(b"\npath=");
            syscalls.stdout_bytes(handle.program_path.as_bytes());
            syscalls.stdout_bytes(b"\nstate=failed\n");
            ProgramStatus::Ok
        }
        Err(error) => {
            syscalls.stderr_bytes(b"proc kill: ");
            syscalls.stderr_line(error.as_str());
            ProgramStatus::Error
        }
    }
}

fn run_proc_install_payload_arg2_arg3(
    argv: &ProgramArgv<'_>,
    syscalls: &mut ProgramSyscalls<'_, '_, '_>,
) -> ProgramStatus {
    let Some(name) = argv.arg(2) else {
        syscalls.stderr_line("proc install-payload: missing payload");
        return ProgramStatus::Error;
    };
    let Some(status_text) = argv.arg(3) else {
        syscalls.stderr_line("proc install-payload: missing status");
        return ProgramStatus::Error;
    };
    if argv.argc() > 4 {
        syscalls.stderr_line("proc install-payload: too many arguments");
        return ProgramStatus::Error;
    }

    let Some(status) = parse_payload_source_install_status(status_text) else {
        syscalls.stderr_line("proc install-payload: invalid status");
        return ProgramStatus::Error;
    };

    match syscalls.install_payload_source_by_name(name, status) {
        Ok(record) => {
            syscalls.stdout_line("proc install-payload:");
            syscalls.stdout_bytes(b"name=");
            syscalls.stdout_bytes(record.name.as_bytes());
            syscalls.stdout_bytes(b"\nnamespace=payload\npath=");
            syscalls.stdout_bytes(record.path.as_bytes());
            syscalls.stdout_bytes(b"\nstatus=");
            syscalls.stdout_bytes(record.status.as_str().as_bytes());
            syscalls.stdout_bytes(b"\nbytes=");
            source_write_u64_dec(syscalls, record.bytes_len as u64);
            syscalls.stdout_bytes(b"\norigin=installed\n");
            ProgramStatus::Ok
        }
        Err(error) => {
            syscalls.stderr_bytes(b"proc install-payload: ");
            syscalls.stderr_line(error.as_str());
            ProgramStatus::Error
        }
    }
}

fn run_proc_install_bin_arg2_arg3(
    argv: &ProgramArgv<'_>,
    syscalls: &mut ProgramSyscalls<'_, '_, '_>,
) -> ProgramStatus {
    let Some(name) = argv.arg(2) else {
        syscalls.stderr_line("proc install-bin: missing program");
        return ProgramStatus::Error;
    };
    let Some(status_text) = argv.arg(3) else {
        syscalls.stderr_line("proc install-bin: missing status");
        return ProgramStatus::Error;
    };
    if argv.argc() > 4 {
        syscalls.stderr_line("proc install-bin: too many arguments");
        return ProgramStatus::Error;
    }

    let Some(status) = parse_bin_source_install_status(status_text) else {
        syscalls.stderr_line("proc install-bin: invalid status");
        return ProgramStatus::Error;
    };

    match syscalls.install_bin_source_by_name(name, status) {
        Ok(record) => {
            syscalls.stdout_line("proc install-bin:");
            syscalls.stdout_bytes(b"name=");
            syscalls.stdout_bytes(record.name.as_bytes());
            syscalls.stdout_bytes(b"\nnamespace=bin\npath=");
            syscalls.stdout_bytes(record.path.as_bytes());
            syscalls.stdout_bytes(b"\nstatus=");
            syscalls.stdout_bytes(record.status.as_str().as_bytes());
            syscalls.stdout_bytes(b"\nbytes=");
            source_write_u64_dec(syscalls, record.bytes_len as u64);
            syscalls.stdout_bytes(b"\norigin=installed\n");
            ProgramStatus::Ok
        }
        Err(error) => {
            syscalls.stderr_bytes(b"proc install-bin: ");
            syscalls.stderr_line(error.as_str());
            ProgramStatus::Error
        }
    }
}

fn write_source_media_install_record(
    prefix: &str,
    record: crate::syscall::SourceMediaInstallRecord,
    syscalls: &mut ProgramSyscalls<'_, '_, '_>,
) {
    syscalls.stdout_line(prefix);
    syscalls.stdout_bytes(b"name=");
    syscalls.stdout_bytes(record.name.as_bytes());
    syscalls.stdout_bytes(b"\nnamespace=");
    syscalls.stdout_bytes(record.namespace.as_str().as_bytes());
    syscalls.stdout_bytes(b"\npath=");
    syscalls.stdout_bytes(record.path.as_bytes());
    syscalls.stdout_bytes(b"\nstorage=");
    syscalls.stdout_bytes(record.storage.as_bytes());
    syscalls.stdout_bytes(b"\nstorage_capacity_bytes=");
    source_write_u64_dec(syscalls, record.storage_capacity_bytes as u64);
    syscalls.stdout_bytes(b"\nartifact_bytes=");
    source_write_u64_dec(syscalls, record.artifact_bytes_len as u64);
    syscalls.stdout_bytes(b"\nbytes=");
    source_write_u64_dec(syscalls, record.bytes_len as u64);
    syscalls.stdout_bytes(b"\nchecksum=");
    source_write_u64_dec(syscalls, record.checksum as u64);
    syscalls.stdout_bytes(b"\norigin=installed\nsource=source-media\n");
}

fn run_proc_install_bin_media_arg2(
    argv: &ProgramArgv<'_>,
    syscalls: &mut ProgramSyscalls<'_, '_, '_>,
) -> ProgramStatus {
    let Some(name) = argv.arg(2) else {
        syscalls.stderr_line("proc install-bin-media: missing program");
        return ProgramStatus::Error;
    };
    if argv.argc() > 3 {
        syscalls.stderr_line("proc install-bin-media: too many arguments");
        return ProgramStatus::Error;
    }

    match syscalls.install_bin_source_from_media_by_name(name) {
        Ok(record) => {
            write_source_media_install_record("proc install-bin-media:", record, syscalls);
            ProgramStatus::Ok
        }
        Err(error) => {
            syscalls.stderr_bytes(b"proc install-bin-media: ");
            syscalls.stderr_line(error.as_str());
            ProgramStatus::Error
        }
    }
}

fn run_proc_install_payload_media_arg2(
    argv: &ProgramArgv<'_>,
    syscalls: &mut ProgramSyscalls<'_, '_, '_>,
) -> ProgramStatus {
    let Some(name) = argv.arg(2) else {
        syscalls.stderr_line("proc install-payload-media: missing payload");
        return ProgramStatus::Error;
    };
    if argv.argc() > 3 {
        syscalls.stderr_line("proc install-payload-media: too many arguments");
        return ProgramStatus::Error;
    }

    match syscalls.install_payload_source_from_media_by_name(name) {
        Ok(record) => {
            write_source_media_install_record("proc install-payload-media:", record, syscalls);
            ProgramStatus::Ok
        }
        Err(error) => {
            syscalls.stderr_bytes(b"proc install-payload-media: ");
            syscalls.stderr_line(error.as_str());
            ProgramStatus::Error
        }
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

    if line == SOURCE_BYTES_OP_WRITE_PROCESS_TABLE {
        syscalls.write_process_table();
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

    if line == SOURCE_BYTES_OP_PROC_EXEC_ARGV_TAIL {
        return Some(run_proc_exec_tail(argv, syscalls));
    }

    if line == SOURCE_BYTES_OP_PROC_SPAWN_ARGV_TAIL {
        return Some(run_proc_spawn_tail(argv, syscalls));
    }

    if line == SOURCE_BYTES_OP_PROC_BLOCK_ARGV_TAIL {
        return Some(run_proc_block_tail(argv, syscalls));
    }

    if line == SOURCE_BYTES_OP_PROC_WAIT_PID_ARG2 {
        return Some(run_proc_wait_pid_arg2(argv, syscalls));
    }

    if line == SOURCE_BYTES_OP_PROC_WAKE_PID_ARG2 {
        return Some(run_proc_wake_pid_arg2(argv, syscalls));
    }

    if line == SOURCE_BYTES_OP_PROC_KILL_PID_ARG2 {
        return Some(run_proc_kill_pid_arg2(argv, syscalls));
    }

    if line == SOURCE_BYTES_OP_PROC_INSTALL_BIN_ARG2_ARG3 {
        return Some(run_proc_install_bin_arg2_arg3(argv, syscalls));
    }

    if line == SOURCE_BYTES_OP_PROC_INSTALL_PAYLOAD_ARG2_ARG3 {
        return Some(run_proc_install_payload_arg2_arg3(argv, syscalls));
    }

    if line == SOURCE_BYTES_OP_PROC_INSTALL_BIN_MEDIA_ARG2 {
        return Some(run_proc_install_bin_media_arg2(argv, syscalls));
    }

    if line == SOURCE_BYTES_OP_PROC_INSTALL_PAYLOAD_MEDIA_ARG2 {
        return Some(run_proc_install_payload_media_arg2(argv, syscalls));
    }

    if line == SOURCE_BYTES_OP_RUN_PROVIDER_PROBE_ARG1 {
        return Some(run_provider_probe_arg1(argv, syscalls));
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

fn run_provider_probe_arg1(
    argv: &ProgramArgv<'_>,
    syscalls: &mut ProgramSyscalls<'_, '_, '_>,
) -> ProgramStatus {
    let Some(target) = argv.arg(1) else {
        syscalls.stderr_line("probe: missing target, try `probe help`");
        return ProgramStatus::Error;
    };

    match syscalls.run_hardware_probe(target) {
        Some(crate::rootd::HardwareProbeResult::Handled) => ProgramStatus::Ok,
        Some(crate::rootd::HardwareProbeResult::UnknownTarget) => {
            syscalls.stderr_bytes(b"probe: unknown target: ");
            syscalls.stderr_bytes(target.as_bytes());
            syscalls.stderr_bytes(b"\n");
            ProgramStatus::Error
        }
        None => {
            syscalls.stderr_line("probe: no lower probe provider");
            ProgramStatus::Error
        }
    }
}

fn run_source_launch_payload_arg1_or_list(
    argv: &ProgramArgv<'_>,
    syscalls: &mut ProgramSyscalls<'_, '_, '_>,
) -> ProgramStatus {
    if argv.argc() == 1 {
        write_source_payload_list(syscalls);
        return ProgramStatus::Ok;
    }

    if argv.argc() > 2 {
        syscalls.stderr_line("launch: too many arguments");
        return ProgramStatus::Error;
    }

    let payload = argv.arg(1).unwrap_or("");
    if payload.is_empty() {
        syscalls.stderr_line("launch: no payload name");
        return ProgramStatus::Error;
    }

    let result = syscalls.launch_payload_by_name(payload);
    syscalls.stdout_bytes(b"launch ");
    syscalls.stdout_bytes(payload.as_bytes());
    syscalls.stdout_bytes(b": ");
    write_source_payload_status(syscalls, result);
    payload_result_status(result)
}

fn run_source_launch_payload_name(
    payload_name: &[u8],
    syscalls: &mut ProgramSyscalls<'_, '_, '_>,
) -> Option<ProgramStatus> {
    let Ok(payload_name) = core::str::from_utf8(payload_name) else {
        return None;
    };
    let result = syscalls.launch_payload_by_name(payload_name);
    write_source_payload_status(syscalls, result);
    Some(payload_result_status(result))
}

fn write_source_payload_list(syscalls: &ProgramSyscalls<'_, '_, '_>) {
    let payloads = syscalls.payloads();
    if payloads.is_empty() {
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
}

fn write_source_payload_status(
    syscalls: &ProgramSyscalls<'_, '_, '_>,
    result: crate::rootd::PayloadLaunchResult,
) {
    match result {
        crate::rootd::PayloadLaunchResult::Ready => syscalls.stdout_bytes(b"payload.ready"),
        crate::rootd::PayloadLaunchResult::NotConfigured => {
            syscalls.stdout_bytes(b"payload.not_configured")
        }
        crate::rootd::PayloadLaunchResult::Failed => syscalls.stdout_bytes(b"payload.failed"),
    }
    syscalls.stdout_bytes(b"\n");
}

fn payload_result_status(result: crate::rootd::PayloadLaunchResult) -> ProgramStatus {
    match result {
        crate::rootd::PayloadLaunchResult::Ready => ProgramStatus::Ok,
        crate::rootd::PayloadLaunchResult::NotConfigured
        | crate::rootd::PayloadLaunchResult::Failed => ProgramStatus::Error,
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

/// Runs one loaded `/bin` program through the typed syscall handle.
pub(crate) fn run_loaded_program(
    program: LoadedProgram,
    argv: &ProgramArgv<'_>,
    syscalls: &mut ProgramSyscalls<'_, '_, '_>,
) -> ProgramStatus {
    let _stdio = syscalls.stdio();
    run_source_bytes(program.source_bytes(), argv, syscalls)
}

#[cfg(feature = "selftest")]
#[path = "program_tests.rs"]
mod tests;
