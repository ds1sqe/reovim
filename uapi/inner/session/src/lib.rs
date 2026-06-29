//! Product-facing shell session syscall vocabulary.
//!
//! This leaf owns session-domain controls such as selecting the root-shell line
//! discipline and requesting session startup. The raw syscall leaf remains only
//! the transport spine.

#![no_std]

use reovim_uapi_syscall::{RawSyscall, SyscallArgs, SyscallError, SyscallNr};

/// Shell-session control operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct SessionControlOp(usize);

impl SessionControlOp {
    /// Request startup of the interactive shell/session loop.
    pub const SHELL_START: Self = Self(0);
    /// Request the command-line discipline for the interactive shell loop.
    pub const LINE_DISCIPLINE: Self = Self(1);

    /// Builds a control op from a raw transport scalar.
    #[must_use]
    pub const fn new(raw: usize) -> Self {
        Self(raw)
    }

    /// Returns the raw transport scalar.
    #[must_use]
    pub const fn raw(self) -> usize {
        self.0
    }
}

/// Product-facing session syscall error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct SessionError(i32);

impl SessionError {
    /// The requested arguments are not valid for this operation.
    pub const INVALID_ARGUMENT: Self = Self(SyscallError::INVALID_ARGUMENT.code());
    /// This kernel build does not support the requested operation.
    pub const UNSUPPORTED: Self = Self(SyscallError::UNSUPPORTED.code());
    /// The syscall requires an attached current process context.
    pub const NO_CURRENT_PROCESS: Self = Self(SyscallError::NO_CURRENT_PROCESS.code());
    /// The requested session operation is already in use.
    pub const BUSY: Self = Self(SyscallError::BUSY.code());

    /// Builds a session error from a transport code.
    #[must_use]
    pub const fn new(code: i32) -> Self {
        Self(code)
    }

    /// Returns the stable transport code.
    #[must_use]
    pub const fn code(self) -> i32 {
        self.0
    }
}

/// Shell-session control backed by the raw Reovim syscall transport.
#[derive(Debug, Clone, Copy)]
pub struct SyscallSessionControl {
    raw: RawSyscall,
}

impl SyscallSessionControl {
    /// Creates a session control adapter over the raw syscall transport.
    #[must_use]
    pub const fn new(raw: RawSyscall) -> Self {
        Self { raw }
    }

    /// Requests the command-line discipline used by the interactive shell loop.
    ///
    /// This is session policy, not terminal device control. The current system
    /// kernel supports `argv-v1` with `single-pipe`.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError`] when either string is empty, there is no current
    /// process, or the kernel rejects the requested discipline.
    pub fn request_line_discipline(
        self,
        line_discipline: &str,
        pipe_mode: &str,
    ) -> Result<(), SessionError> {
        if line_discipline.is_empty() || pipe_mode.is_empty() {
            return Err(SessionError::INVALID_ARGUMENT);
        }
        self.raw
            .invoke(
                SyscallNr::SESSION_CONTROL,
                SyscallArgs::new([
                    SessionControlOp::LINE_DISCIPLINE.raw(),
                    line_discipline.as_ptr().addr(),
                    line_discipline.len(),
                    pipe_mode.as_ptr().addr(),
                    pipe_mode.len(),
                    0,
                ]),
            )
            .decode()
            .map(|_| ())
            .map_err(session_error_from_syscall)
    }

    /// Requests startup of the interactive shell/session loop.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError`] when no current process context exists or the
    /// kernel rejects the startup request.
    pub fn request_shell_start(self) -> Result<(), SessionError> {
        self.raw
            .invoke(
                SyscallNr::SESSION_CONTROL,
                SyscallArgs::new([SessionControlOp::SHELL_START.raw(), 0, 0, 0, 0, 0]),
            )
            .decode()
            .map(|_| ())
            .map_err(session_error_from_syscall)
    }
}

fn session_error_from_syscall(error: SyscallError) -> SessionError {
    SessionError::new(error.code())
}
