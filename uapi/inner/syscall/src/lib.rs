//! Raw Reovim syscall transport vocabulary.
//!
//! This crate is the small ABI spine below domain uapi leaves. It owns syscall
//! numbers, register-shaped argument packing, scalar return/error decoding, and
//! the temporary backend hook used before architecture trap entry exists.
//!
//! It deliberately does not own filesystem, process, scheduler, terminal, or
//! device semantics. Public operations such as read/write/open, spawn/exec/wait,
//! yield/sleep, and terminal control belong in their domain uapi leaves and may
//! lower through these raw transport types.
//!
//! There is intentionally no generic public `ioctl` transport number here.
//! Device or stream control must be modeled as typed domain control operations
//! with explicit op namespaces and typed payloads.

#![no_std]

/// Raw syscall number.
///
/// `0` is reserved invalid so zeroed argument blocks and missing dispatch paths
/// are easy to reject.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct SyscallNr(u32);

impl SyscallNr {
    /// Reserved invalid syscall number.
    pub const INVALID: Self = Self(0);
    /// Raw transport for domain filesystem read.
    pub const READ: Self = Self(1);
    /// Raw transport for domain filesystem write.
    pub const WRITE: Self = Self(2);
    /// Raw transport for domain filesystem open-at.
    pub const OPEN_AT: Self = Self(3);
    /// Raw transport for domain filesystem close.
    pub const CLOSE: Self = Self(4);
    /// Raw transport for domain filesystem seek.
    pub const LSEEK: Self = Self(5);
    /// Raw transport for domain directory iteration.
    pub const GETDENTS: Self = Self(6);
    /// Raw transport for process identity.
    pub const GET_PID: Self = Self(7);
    /// Raw transport for current-directory read.
    pub const GET_CWD: Self = Self(8);
    /// Raw transport for current-directory change.
    pub const CHDIR: Self = Self(9);
    /// Raw transport for cooperative scheduler yield.
    pub const YIELD_NOW: Self = Self(10);
    /// Raw transport for scheduler sleep.
    pub const SLEEP: Self = Self(11);
    /// Raw transport for process spawn.
    pub const SPAWN: Self = Self(12);
    /// Raw transport for replacing the current process image.
    pub const EXECVE: Self = Self(13);
    /// Raw transport for retained child wait.
    pub const WAIT: Self = Self(14);
    /// Raw transport for process exit.
    pub const EXIT: Self = Self(15);
    /// Raw transport for duplicating a descriptor into the next free slot.
    pub const DUP: Self = Self(16);
    /// Raw transport for charging one scheduler tick to the current process.
    pub const SCHED_TICK: Self = Self(17);
    /// Raw transport for duplicating a descriptor into a requested slot.
    pub const DUP_TO: Self = Self(18);
    /// Raw transport for creating a pipe descriptor pair.
    pub const PIPE: Self = Self(19);
    /// Raw transport for typed terminal clear control.
    pub const TERMINAL_CLEAR: Self = Self(20);
    /// Raw transport for typed system halt control.
    pub const SYSTEM_HALT: Self = Self(21);
    /// Raw transport for typed lower-provider probe control.
    pub const PROVIDER_PROBE: Self = Self(22);
    /// Raw transport for typed diagnostic dump sync control.
    pub const DUMP_SYNC: Self = Self(23);
    /// Raw transport for waking a retained blocked process.
    pub const PROCESS_WAKE: Self = Self(24);
    /// Raw transport for terminating a retained ready or blocked process.
    pub const PROCESS_KILL: Self = Self(25);
    /// Raw transport for retained child wait with a bounded scheduler tick deadline.
    pub const PROCESS_WAIT_TICKS: Self = Self(26);
    /// Raw transport for typed service lifecycle control.
    pub const SERVICE_CONTROL: Self = Self(27);
    /// Raw transport for retained child readiness wait.
    pub const PROCESS_WAIT_READY: Self = Self(28);
    /// Raw transport for typed shell session control.
    pub const SESSION_CONTROL: Self = Self(29);
    /// Raw transport for typed executable source-install control.
    pub const SOURCE_CONTROL: Self = Self(30);
    /// Raw transport for reading descriptor-local flags.
    pub const FD_FLAGS_GET: Self = Self(31);
    /// Raw transport for replacing descriptor-local flags.
    pub const FD_FLAGS_SET: Self = Self(32);
    /// Raw transport for typed terminal raw-mode entry.
    pub const TERMINAL_RAW_ENTER: Self = Self(33);
    /// Raw transport for typed terminal raw-mode restore by token.
    pub const TERMINAL_RAW_RESTORE: Self = Self(34);
    /// Raw transport for best-effort primary-input raw-mode restore.
    pub const TERMINAL_RAW_RESTORE_PRIMARY: Self = Self(35);
    /// Raw transport for reading opened-object status flags.
    pub const FD_STATUS_GET: Self = Self(36);
    /// Raw transport for replacing opened-object status flags.
    pub const FD_STATUS_SET: Self = Self(37);
    /// Raw transport for current retained-process metadata.
    pub const PROCESS_SELF: Self = Self(38);

    /// Builds a raw syscall number from a bridge-issued scalar.
    #[must_use]
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }

    /// Returns the scalar syscall number.
    #[must_use]
    pub const fn raw(self) -> u32 {
        self.0
    }

    /// Returns whether this is the reserved invalid number.
    #[must_use]
    pub const fn is_invalid(self) -> bool {
        self.0 == Self::INVALID.0
    }
}

/// Register-shaped syscall arguments.
///
/// The raw transport carries only scalar words. Larger domain calls should pass
/// a pointer to a `repr(C)` argument record owned by the semantic uapi leaf.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[repr(C)]
pub struct SyscallArgs {
    /// First scalar argument.
    pub a0: usize,
    /// Second scalar argument.
    pub a1: usize,
    /// Third scalar argument.
    pub a2: usize,
    /// Fourth scalar argument.
    pub a3: usize,
    /// Fifth scalar argument.
    pub a4: usize,
    /// Sixth scalar argument.
    pub a5: usize,
}

impl SyscallArgs {
    /// Empty argument block.
    pub const EMPTY: Self = Self::new([0; Self::COUNT]);
    /// Number of scalar argument slots.
    pub const COUNT: usize = 6;

    /// Builds an argument block from scalar words.
    #[must_use]
    pub const fn new(args: [usize; Self::COUNT]) -> Self {
        Self {
            a0: args[0],
            a1: args[1],
            a2: args[2],
            a3: args[3],
            a4: args[4],
            a5: args[5],
        }
    }

    /// Returns the scalar word at `index`, or `None` when out of range.
    #[must_use]
    pub const fn get(self, index: usize) -> Option<usize> {
        match index {
            0 => Some(self.a0),
            1 => Some(self.a1),
            2 => Some(self.a2),
            3 => Some(self.a3),
            4 => Some(self.a4),
            5 => Some(self.a5),
            _ => None,
        }
    }
}

/// Raw syscall transport error.
///
/// Error codes are Reovim syscall-transport scalars. They are not POSIX errno
/// constants, even when a future bridge maps them to provider errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct SyscallError(i32);

impl SyscallError {
    /// Unsupported syscall or transport operation.
    pub const UNSUPPORTED: Self = Self(1);
    /// Invalid syscall number or argument block.
    pub const INVALID_ARGUMENT: Self = Self(2);
    /// File descriptor is not attached to the current process.
    pub const BAD_DESCRIPTOR: Self = Self(3);
    /// File descriptor exists but is not readable.
    pub const NOT_READABLE: Self = Self(4);
    /// File descriptor exists but is not writable.
    pub const NOT_WRITABLE: Self = Self(5);
    /// The syscall requires an attached current process context.
    pub const NO_CURRENT_PROCESS: Self = Self(6);
    /// Path lookup did not find an object.
    pub const NOT_FOUND: Self = Self(7);
    /// Path lookup resolved to a directory where a file was required.
    pub const NOT_DIRECTORY: Self = Self(8);
    /// The requested kernel object is already in use by this bounded backend.
    pub const BUSY: Self = Self(9);
    /// The target object could not fit in the bounded syscall buffer.
    pub const FILE_TOO_LARGE: Self = Self(10);
    /// The kernel object reported an I/O failure.
    pub const IO: Self = Self(11);
    /// File descriptor exists but does not support seek.
    pub const NOT_SEEKABLE: Self = Self(12);
    /// No retained process exists for the requested process identifier.
    pub const PROCESS_NOT_FOUND: Self = Self(13);
    /// The retained process state cannot be waited by this operation.
    pub const NOT_WAITABLE: Self = Self(14);
    /// The target process is protected from this operation.
    pub const PROTECTED_PROCESS: Self = Self(15);
    /// The requested executable image failed validation.
    pub const INVALID_IMAGE: Self = Self(16);
    /// No source-media target is installed.
    pub const SOURCE_MEDIA_UNAVAILABLE: Self = Self(17);
    /// The source-media target failed to read a bounded artifact.
    pub const SOURCE_MEDIA_READ_FAILED: Self = Self(18);
    /// Source-media artifact namespace does not match the requested install kind.
    pub const SOURCE_MEDIA_NAMESPACE_MISMATCH: Self = Self(19);
    /// Source-media artifact path does not match the requested descriptor.
    pub const SOURCE_MEDIA_PATH_MISMATCH: Self = Self(20);
    /// The opened object has no data ready and the caller requested nonblocking behavior.
    pub const WOULD_BLOCK: Self = Self(21);

    /// Builds an error from a stable positive Reovim transport code.
    #[must_use]
    pub const fn new(code: i32) -> Self {
        Self(code)
    }

    /// Returns the stable Reovim transport code.
    #[must_use]
    pub const fn code(self) -> i32 {
        self.0
    }

    const fn positive_code(self) -> i32 {
        if self.0 > 0 {
            self.0
        } else {
            Self::INVALID_ARGUMENT.0
        }
    }
}

/// Raw syscall return scalar.
///
/// Non-negative values are success. Negative values encode
/// [`SyscallError::code`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct SyscallRet(i64);

impl SyscallRet {
    /// Builds a raw return from its encoded scalar.
    #[must_use]
    pub const fn raw(raw: i64) -> Self {
        Self(raw)
    }

    /// Encodes a successful scalar return.
    #[must_use]
    pub const fn success(value: usize) -> Self {
        Self(value as i64)
    }

    /// Encodes a failed return.
    #[must_use]
    pub const fn failure(error: SyscallError) -> Self {
        Self(-(error.positive_code() as i64))
    }

    /// Returns the encoded scalar.
    #[must_use]
    pub const fn value(self) -> i64 {
        self.0
    }

    /// Returns whether this return encodes failure.
    #[must_use]
    pub const fn is_failure(self) -> bool {
        self.0 < 0
    }

    /// Decodes the scalar return.
    ///
    /// # Errors
    ///
    /// Returns [`SyscallError`] when the encoded value is negative.
    pub const fn decode(self) -> Result<usize, SyscallError> {
        if self.0 < 0 {
            Err(SyscallError((-self.0) as i32))
        } else {
            Ok(self.0 as usize)
        }
    }
}

/// Raw syscall backend function.
pub type RawSyscallFn = fn(SyscallNr, SyscallArgs) -> SyscallRet;

/// Raw syscall backend hook.
///
/// Domain uapi wrappers may carry one of these until architecture-specific trap
/// entry exists. The hook still uses raw transport types only; it does not own
/// domain semantics.
#[derive(Debug, Clone, Copy)]
pub struct RawSyscall {
    /// Invokes one raw syscall.
    pub invoke_fn: RawSyscallFn,
}

impl RawSyscall {
    /// Creates a backend that rejects every syscall as unsupported.
    #[must_use]
    pub const fn unsupported() -> Self {
        Self::new(unsupported_syscall)
    }

    /// Creates a raw syscall backend hook.
    #[must_use]
    pub const fn new(invoke_fn: RawSyscallFn) -> Self {
        Self { invoke_fn }
    }

    /// Invokes one raw syscall.
    #[must_use]
    pub fn invoke(self, nr: SyscallNr, args: SyscallArgs) -> SyscallRet {
        (self.invoke_fn)(nr, args)
    }
}

impl Default for RawSyscall {
    fn default() -> Self {
        Self::unsupported()
    }
}

fn unsupported_syscall(_: SyscallNr, _: SyscallArgs) -> SyscallRet {
    SyscallRet::failure(SyscallError::UNSUPPORTED)
}
