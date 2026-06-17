//! `reovim-uapi-posix` — POSIX primitive newtypes and O_* constants.
//!
//! This crate is the single canonical source for the POSIX fd-vocabulary
//! types used across the uapi tier. It is pure-definition: no syscall logic,
//! no allocation, no platform policy.
//!
//! ## Types
//!
//! - [`OpenFlags`] — `openat(2)` flags word (wraps `i32`, Linux ABI shape).
//! - [`Mode`] — file permission bits (wraps `u32`, POSIX `mode_t` shape).
//! - [`Fd`] — file descriptor (wraps `i32`; non-negative values are valid fds).
//! - [`Errno`] — positive errno code (wraps `i32`; positive at this layer, never
//!   the negative `-errno` the raw syscall ABI returns).
//!
//! ## Canonical O_* constants
//!
//! The values are the Linux/x86-64 ABI values, mirrored exactly from
//! `arch/src/sys/wrap.rs` and confirmed identical in `arch/src/sys/none_aarch64/wrap.rs`.
//! Both targets document keeping Linux ABI values even where the target never
//! interprets them, so these constants are target-neutral vocabulary.
//!
//! ## AB3 layout precondition
//!
//! Compile-time size + align assertions for each newtype ensure that the uapi
//! types are byte-identical to their underlying ints. These must hold before
//! Phase 3 re-types the effectful-slot signatures in arch and kabi.
#![no_std]

// ---- OpenFlags ---------------------------------------------------------------

/// File-open flags word (the second argument to `openat(2)`).
///
/// A `#[repr(transparent)]` wrapper over `i32` — byte-identical to the
/// integer the Linux ABI uses in the `openat` register. The type is used for
/// flag composition via bitwise OR; the canonical flag constants below have
/// type [`OpenFlags`] so composition is type-checked.
///
/// ```rust
/// use reovim_uapi_posix::{OpenFlags, O_RDONLY};
///
/// let flags = O_RDONLY;
/// assert_eq!(flags, OpenFlags(0));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct OpenFlags(pub i32);

impl OpenFlags {
    /// The raw `i32` bit-pattern (the value passed to the syscall register).
    ///
    /// ```rust
    /// use reovim_uapi_posix::{OpenFlags, O_CREAT};
    ///
    /// assert_eq!(O_CREAT.bits(), 0o100);
    /// ```
    #[must_use]
    pub const fn bits(self) -> i32 {
        self.0
    }
}

impl core::ops::BitOr for OpenFlags {
    type Output = Self;

    /// Combines two flag words by bitwise OR.
    ///
    /// ```rust
    /// use reovim_uapi_posix::{OpenFlags, O_WRONLY, O_CREAT, O_CLOEXEC};
    ///
    /// let flags = O_WRONLY | O_CREAT | O_CLOEXEC;
    /// assert_eq!(flags.bits(), 0o1 | 0o100 | 0o2_000_000);
    /// ```
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

// AB3 layout precondition: OpenFlags must be byte-identical to i32.
const _: () = assert!(core::mem::size_of::<OpenFlags>() == core::mem::size_of::<i32>());
const _: () = assert!(core::mem::align_of::<OpenFlags>() == core::mem::align_of::<i32>());

// ---- Mode --------------------------------------------------------------------

/// File permission bits (the fourth argument to `openat(2)` when `O_CREAT` is set).
///
/// A `#[repr(transparent)]` wrapper over `u32` — the shape POSIX `mode_t`
/// takes on Linux (the kernel's `openat` mode register is 32 bits unsigned).
///
/// ```rust
/// use reovim_uapi_posix::Mode;
///
/// let m = Mode(0o644);
/// assert_eq!(m.bits(), 0o644);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct Mode(pub u32);

impl Mode {
    /// The raw `u32` permission bit-pattern.
    ///
    /// ```rust
    /// use reovim_uapi_posix::Mode;
    ///
    /// assert_eq!(Mode(0o600).bits(), 0o600);
    /// ```
    #[must_use]
    pub const fn bits(self) -> u32 {
        self.0
    }
}

// AB3 layout precondition: Mode must be byte-identical to u32.
const _: () = assert!(core::mem::size_of::<Mode>() == core::mem::size_of::<u32>());
const _: () = assert!(core::mem::align_of::<Mode>() == core::mem::align_of::<u32>());

// ---- Fd ----------------------------------------------------------------------

/// A file descriptor (the signed integer the kernel returns from `openat`,
/// `socket`, `accept`, and similar calls).
///
/// A `#[repr(transparent)]` wrapper over `i32`. Valid descriptors are
/// non-negative (`>= 0`). The sentinel value `-1` is the conventional
/// "no descriptor" marker, but this type does not enforce that invariant —
/// it only guarantees ABI shape.
///
/// ```rust
/// use reovim_uapi_posix::Fd;
///
/// let fd = Fd(3);
/// assert_eq!(fd.as_i32(), 3);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct Fd(pub i32);

impl Fd {
    /// The raw `i32` file descriptor value.
    ///
    /// ```rust
    /// use reovim_uapi_posix::Fd;
    ///
    /// assert_eq!(Fd(5).as_i32(), 5);
    /// ```
    #[must_use]
    pub const fn as_i32(self) -> i32 {
        self.0
    }
}

// AB3 layout precondition: Fd must be byte-identical to i32.
const _: () = assert!(core::mem::size_of::<Fd>() == core::mem::size_of::<i32>());
const _: () = assert!(core::mem::align_of::<Fd>() == core::mem::align_of::<i32>());

// ---- Errno -------------------------------------------------------------------

/// A positive errno code.
///
/// Carries the error number as a positive `i32`. The sign convention: the
/// Linux syscall ABI returns negative values (`-errno`) on failure; the layers
/// above the syscall boundary convert to a positive code and store it here.
///
/// ```rust
/// use reovim_uapi_posix::Errno;
///
/// // ENOENT is 2 on Linux.
/// let e = Errno(2);
/// assert_eq!(e.code(), 2);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct Errno(pub i32);

impl Errno {
    /// Builds an [`Errno`] from a positive errno code.
    ///
    /// The sign convention is the layer's, not the type's: callers above the
    /// syscall boundary have already converted the raw `-errno` return to a
    /// positive code before naming it here.
    ///
    /// ```rust
    /// use reovim_uapi_posix::Errno;
    ///
    /// assert_eq!(Errno::from_code(22).code(), 22);
    /// ```
    #[must_use]
    pub const fn from_code(code: i32) -> Self {
        Self(code)
    }

    /// The positive errno code.
    ///
    /// ```rust
    /// use reovim_uapi_posix::Errno;
    ///
    /// assert_eq!(Errno(9).code(), 9);
    /// ```
    #[must_use]
    pub const fn code(self) -> i32 {
        self.0
    }
}

// AB3 layout precondition: Errno must be byte-identical to i32.
const _: () = assert!(core::mem::size_of::<Errno>() == core::mem::size_of::<i32>());
const _: () = assert!(core::mem::align_of::<Errno>() == core::mem::align_of::<i32>());

// ---- O_* constants -----------------------------------------------------------
// Canonical Linux/x86-64 values, mirrored from arch/src/sys/wrap.rs (lines
// 70-86) and verified identical in arch/src/sys/none_aarch64/wrap.rs (lines
// 46-54). The none_aarch64 target explicitly documents keeping the Linux ABI
// values even where the target never interprets them, so these are
// target-neutral vocabulary constants.

/// `O_RDONLY` — open for reading only.
///
/// ```rust
/// use reovim_uapi_posix::{OpenFlags, O_RDONLY};
///
/// assert_eq!(O_RDONLY, OpenFlags(0));
/// ```
pub const O_RDONLY: OpenFlags = OpenFlags(0);

/// `O_WRONLY` — open for writing only.
///
/// ```rust
/// use reovim_uapi_posix::{OpenFlags, O_WRONLY};
///
/// assert_eq!(O_WRONLY, OpenFlags(0o1));
/// ```
pub const O_WRONLY: OpenFlags = OpenFlags(0o1);

/// `O_CREAT` — create the file if it does not exist.
///
/// ```rust
/// use reovim_uapi_posix::{OpenFlags, O_CREAT};
///
/// assert_eq!(O_CREAT, OpenFlags(0o100));
/// ```
pub const O_CREAT: OpenFlags = OpenFlags(0o100);

/// `O_TRUNC` — truncate the file to zero length on open.
///
/// ```rust
/// use reovim_uapi_posix::{OpenFlags, O_TRUNC};
///
/// assert_eq!(O_TRUNC, OpenFlags(0o1000));
/// ```
pub const O_TRUNC: OpenFlags = OpenFlags(0o1000);

/// `O_CLOEXEC` — close the descriptor on `exec(2)`.
///
/// ```rust
/// use reovim_uapi_posix::{OpenFlags, O_CLOEXEC};
///
/// assert_eq!(O_CLOEXEC, OpenFlags(0o2_000_000));
/// ```
pub const O_CLOEXEC: OpenFlags = OpenFlags(0o2_000_000);
